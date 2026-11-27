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

    let entry = main_file
        .map(Path::to_path_buf)
        .unwrap_or(config.document.entry);

    // The engine writes outputs to cwd, named after the entry's stem (jobname).
    let jobname = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid entry file: {}", entry.display()))?
        .to_owned();

    let job = Job {
        engine: config.document.engine,
        entry,
        jobname,
        cwd,
        verbose,
    };

    // Preflight: warn (do not fail) when engines are missing.
    preflight(&job.engine);

    let mut pass = 1;
    job.latex(pass)?;

    // Optional pass: biber when citations are detected in cwd/*.tex
    if citations_present(&job.cwd) {
        preflight(&config.bib.engine);
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
        &format!("`{}` in {:.2}s", job.pdf_name(), started.elapsed().as_secs_f64()),
    );
    Ok(pdf)
}

struct Job {
    engine: String,
    entry: PathBuf,
    jobname: String,
    cwd: PathBuf,
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
        cmd.args(["-interaction=nonstopmode", "-file-line-error"])
            .arg(&self.entry)
            // Unwrapped, untruncated log lines keep the log parseable (TeX Live).
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
        status("Running", &format!("{bib_engine} {}", self.jobname));

        let mut cmd = Command::new(bib_engine);
        cmd.arg(&self.jobname);
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

    fn output(&self, ext: &str) -> PathBuf {
        self.cwd.join(format!("{}.{ext}", self.jobname))
    }

    fn pdf_name(&self) -> String {
        format!("{}.pdf", self.jobname)
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

fn log_requests_rerun(log: &str) -> bool {
    log.contains("Rerun to get") || log.contains("Please rerun LaTeX")
}

#[cfg(test)]
mod tests {
    use super::log_requests_rerun;

    #[test]
    fn rerun_on_stale_cross_references() {
        let log = "LaTeX Warning: Label(s) may have changed. Rerun to get cross-references right.";
        assert!(log_requests_rerun(log));
    }

    #[test]
    fn rerun_on_biblatex_request() {
        assert!(log_requests_rerun("Package biblatex Warning: Please rerun LaTeX."));
    }

    #[test]
    fn no_rerun_on_clean_log() {
        assert!(!log_requests_rerun("Output written on main.pdf (1 page)."));
    }

    #[test]
    fn no_rerun_when_only_biber_is_requested() {
        // Needs biber, not another LaTeX pass; handled by the citation path.
        assert!(!log_requests_rerun("Please (re)run Biber on the file: main"));
    }
}
