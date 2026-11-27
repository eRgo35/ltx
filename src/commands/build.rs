use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::Instant,
};

use colored::Colorize;

use crate::{
    commands::{
        citations_present,
        diagnostics::{self, Diagnostic},
        preflight,
    },
    config::load_config,
};

/// Builds the document and returns the path of the produced PDF.
/// `main_file` overrides `document.entry` from `ltx.toml` when given.
/// Engine output is hidden unless `verbose`; errors and relevant warnings
/// are read from the log and printed cargo-style.
pub fn build_command(main_file: Option<&Path>, verbose: bool) -> Result<PathBuf, Box<dyn Error>> {
    let started = Instant::now();
    let (config, cwd) = load_config()?;

    let out = config.document.output_dir()?.map(Path::to_path_buf);
    let entry = main_file
        .map(Path::to_path_buf)
        .unwrap_or(config.document.entry);

    // Outputs are named after the entry's stem (jobname).
    let jobname = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid entry file: {}", entry.display()))?
        .to_owned();

    if let Some(out) = &out {
        mirror_tex_dirs(&cwd, &cwd, &cwd.join(out))?;
    }

    // Preflight: warn (do not fail) when engines are missing.
    let version = preflight(&config.document.engine);
    let miktex = version.is_some_and(|v| v.contains("MiKTeX"));

    let job = Job {
        engine: config.document.engine,
        entry,
        jobname,
        cwd,
        out,
        miktex,
        verbose,
    };

    let mut pass = 1;
    job.latex(pass)?;

    // Optional pass: biber when any .tex file cites, or when biblatex asks
    // for it (e.g. citations hidden in a macro from a .sty).
    if citations_present(&job.cwd) || log_requests_biber(&read_lossy(&job.output("log"))) {
        let _ = preflight(&config.bib.engine);
        job.biber(&config.bib.engine)?;
        pass += 1;
        job.latex(pass)?;
    }

    // One extra pass when LaTeX reports stale cross-references / TOC.
    let log = job.output("log");
    if log_requests_rerun(&read_lossy(&log)) {
        pass += 1;
        job.latex(pass)?;
    }

    let warnings = diagnostics::parse_warnings(&read_lossy(&log));
    for w in &warnings {
        eprintln!("{}\n", diagnostics::render("warning", w, &job.cwd));
    }
    let pdf = job.output("pdf");
    if !warnings.is_empty() {
        eprintln!(
            "{}{} `{}` generated {}",
            "warning".yellow().bold(),
            ":".bold(),
            job.pdf_name(),
            count(warnings.len(), "warning")
        );
    }
    status(
        "Finished",
        &format!(
            "`{}` in {:.2}s",
            job.pdf_name(),
            started.elapsed().as_secs_f64()
        ),
    );
    Ok(pdf)
}

struct Job {
    engine: String,
    entry: PathBuf,
    jobname: String,
    cwd: PathBuf,
    /// Output directory relative to `cwd`; `None` writes into `cwd`.
    out: Option<PathBuf>,
    miktex: bool,
    verbose: bool,
}

impl Job {
    fn latex(&self, pass: u32) -> Result<(), Box<dyn Error>> {
        let label = if pass == 1 {
            format!("{} ({})", self.entry.display(), self.engine)
        } else {
            format!("{} ({}, pass {pass})", self.entry.display(), self.engine)
        };
        status("Compiling", &label);

        let mut cmd = Command::new(&self.engine);
        cmd.args(["-interaction=nonstopmode", "-file-line-error"]);
        if let Some(out) = &self.out {
            cmd.arg(format!("-output-directory={}", out.display()));
        }
        // Unwrapped, untruncated log lines keep the log parseable. TeX Live
        // reads these from the environment, MiKTeX only from its options.
        if self.miktex {
            cmd.args([
                "-max-print-line=10000",
                "-error-line=254",
                "-half-error-line=238",
            ]);
        }
        cmd.arg(&self.entry)
            .env("max_print_line", "10000")
            .env("error_line", "254")
            .env("half_error_line", "238");
        let (success, output) = self.run(cmd)?;
        if success {
            return Ok(());
        }

        let mut errors = diagnostics::parse_errors(&read_lossy(&self.output("log")));
        if errors.is_empty() {
            errors = fallback_errors(output.as_ref(), "fatal", &self.engine);
        }
        self.fail(&errors, "log")
    }

    fn biber(&self, bib_engine: &str) -> Result<(), Box<dyn Error>> {
        // biber takes the control file's path without extension.
        let control = self.relative(&self.jobname);
        status("Running", &format!("{bib_engine} {}", control.display()));

        let mut cmd = Command::new(bib_engine);
        cmd.arg(&control);
        let (success, output) = self.run(cmd)?;
        if success {
            return Ok(());
        }

        let mut errors = diagnostics::parse_biber_errors(&read_lossy(&self.output("blg")));
        if errors.is_empty() {
            errors = fallback_errors(output.as_ref(), "ERROR", bib_engine);
        }
        self.fail(&errors, "blg")
    }

    /// Runs `cmd` in the project root without stdin. Output is captured
    /// unless verbose, in which case it goes straight to the terminal.
    fn run(&self, mut cmd: Command) -> Result<(bool, Option<Output>), Box<dyn Error>> {
        let program = cmd.get_program().to_string_lossy().into_owned();
        cmd.current_dir(&self.cwd).stdin(Stdio::null());
        let result = if self.verbose {
            cmd.status().map(|s| (s.success(), None))
        } else {
            cmd.output().map(|o| (o.status.success(), Some(o)))
        };
        result.map_err(|e| format!("failed to run `{program}`: {e}").into())
    }

    fn fail(&self, errors: &[Diagnostic], log_ext: &str) -> Result<(), Box<dyn Error>> {
        for e in errors {
            eprintln!("{}\n", diagnostics::render("error", e, &self.cwd));
        }
        eprintln!(
            "{} full log: {}",
            "note:".bold(),
            self.output(log_ext).display()
        );
        Err(format!(
            "could not build `{}` ({})",
            self.pdf_name(),
            count(errors.len(), "error")
        )
        .into())
    }

    /// Absolute path of a build output, e.g. `output("log")`.
    fn output(&self, ext: &str) -> PathBuf {
        self.cwd
            .join(self.relative(&format!("{}.{ext}", self.jobname)))
    }

    /// `name` inside the output directory, relative to the project root.
    fn relative(&self, name: &str) -> PathBuf {
        match &self.out {
            Some(out) => out.join(name),
            None => PathBuf::from(name),
        }
    }

    fn pdf_name(&self) -> String {
        self.relative(&format!("{}.pdf", self.jobname))
            .display()
            .to_string()
    }
}

/// When the log has nothing parseable: lines mentioning `marker` from the
/// captured output, or a generic message naming the program.
fn fallback_errors(output: Option<&Output>, marker: &str, program: &str) -> Vec<Diagnostic> {
    let lines: Vec<Diagnostic> = output
        .map(|o| {
            let text = String::from_utf8_lossy(&o.stdout).into_owned()
                + &String::from_utf8_lossy(&o.stderr);
            text.lines()
                .filter(|l| l.contains(marker))
                .map(|l| Diagnostic::message(l.trim()))
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec![Diagnostic::message(&format!("`{program}` failed"))]
    } else {
        lines
    }
}

/// Creates `out` and, inside it, every directory under `dir` that holds
/// `.tex` files: `\include{chapters/intro}` writes `chapters/intro.aux` into
/// the output directory, and TeX cannot create directories itself.
fn mirror_tex_dirs(root: &Path, dir: &Path, out: &Path) -> std::io::Result<()> {
    fs::create_dir_all(out)?;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        // file_type() does not follow symlinks, so loops cannot recurse forever.
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        if !entry.file_type()?.is_dir() || hidden || path == out {
            continue;
        }
        let has_tex = fs::read_dir(&path)?
            .flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "tex"));
        if has_tex && let Ok(rel) = path.strip_prefix(root) {
            fs::create_dir_all(out.join(rel))?;
        }
        mirror_tex_dirs(root, &path, out)?;
    }
    Ok(())
}

/// A right-aligned, cargo-style progress line on stderr.
fn status(verb: &str, message: &str) {
    eprintln!("{} {}", format!("{verb:>12}").green().bold(), message);
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn read_lossy(path: &Path) -> String {
    // Logs are not guaranteed UTF-8.
    fs::read(path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

fn log_requests_biber(log: &str) -> bool {
    log.contains("Please (re)run Biber")
}

fn log_requests_rerun(log: &str) -> bool {
    log.contains("Rerun to get") || log.contains("Please rerun LaTeX")
}

#[cfg(test)]
mod tests {
    use super::{log_requests_biber, log_requests_rerun};

    #[test]
    fn biber_on_biblatex_request() {
        assert!(log_requests_biber(
            "Package biblatex Warning: Please (re)run Biber on the file:\n(biblatex) main"
        ));
        assert!(!log_requests_biber("Output written on main.pdf (1 page)."));
    }

    #[test]
    fn rerun_on_stale_cross_references() {
        let log = "LaTeX Warning: Label(s) may have changed. Rerun to get cross-references right.";
        assert!(log_requests_rerun(log));
    }

    #[test]
    fn rerun_on_biblatex_request() {
        assert!(log_requests_rerun(
            "Package biblatex Warning: Please rerun LaTeX."
        ));
    }

    #[test]
    fn no_rerun_on_clean_log() {
        assert!(!log_requests_rerun("Output written on main.pdf (1 page)."));
    }

    #[test]
    fn no_rerun_when_only_biber_is_requested() {
        // Needs biber, not another LaTeX pass; handled by the citation path.
        assert!(!log_requests_rerun(
            "Please (re)run Biber on the file: main"
        ));
    }
}
