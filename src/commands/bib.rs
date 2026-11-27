use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Stdio},
};

use colored::Colorize;

const CITATION_PREFIXES: &[&str] = &[
    "\\cite", "\\citep", "\\citet", "\\nocite", "\\footcite", "\\textcite",
];

pub enum EngineKind {
    Xelatex,
    Bib,
}

/// Scans every `*.tex` file directly in `cwd` (non-recursive) for citation
/// prefixes followed by a non-word byte (e.g. `{`, end-of-file, whitespace).
/// Returns false on any IO error reading a single file (fail-closed).
pub fn citations_present(cwd: &Path) -> bool {
    let entries = match fs::read_dir(cwd) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tex") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue, // fail-closed per FR-4
        };
        if scan_bytes(&bytes) {
            return true;
        }
    }
    false
}

fn scan_bytes(bytes: &[u8]) -> bool {
    for prefix in CITATION_PREFIXES {
        let needle = prefix.as_bytes();
        let mut start = 0;
        while let Some(idx) = find_subseq(&bytes[start..], needle) {
            let after = start + idx + needle.len();
            if after >= bytes.len() || !is_ascii_letter(bytes[after]) {
                return true;
            }
            start = after;
        }
    }
    false
}

fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn is_ascii_letter(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

/// Probes `engine --version`. On spawn failure or non-zero exit, prints a
/// yellow warning naming the engine and the platform-appropriate install
/// hint. Never aborts.
pub fn preflight(engine: &str, kind: EngineKind) -> Result<(), Box<dyn Error>> {
    let status = Command::new(engine)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let bad = match status {
        Ok(s) => !s.success(),
        Err(_) => true,
    };

    if bad {
        let label = match kind {
            EngineKind::Xelatex => "xelatex",
            EngineKind::Bib => engine, // biber / bibtex / ...
        };
        eprintln!(
            "{} {} not found or failed --version probe.",
            "⚠".bold().yellow(),
            label.bold()
        );
        eprintln!("  {}", platform_hint());
        eprintln!("  {}", "Continuing — the real error will surface when ltx invokes the engine.");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_hint() -> &'static str {
    "Install TeX Live (provides xelatex, biber): https://tug.org/texlive/"
}
#[cfg(target_os = "macos")]
fn platform_hint() -> &'static str {
    "Install MacTeX (provides xelatex, biber): https://tug.org/mactex/"
}
#[cfg(target_os = "windows")]
fn platform_hint() -> &'static str {
    "Install MiKTeX (provides xelatex, biber): https://miktex.org/"
}

#[cfg(test)]
mod tests {
    use super::scan_bytes;

    #[test]
    fn detects_each_citation_command() {
        for src in [
            "\\cite{a}",
            "\\citep{a}",
            "\\citet{a}",
            "\\nocite{*}",
            "\\footcite{a}",
            "\\textcite{a}",
        ] {
            assert!(scan_bytes(src.as_bytes()), "{src}");
        }
    }

    #[test]
    fn detects_optional_args_and_starred_form() {
        assert!(scan_bytes(b"see \\cite[p.~3]{a}"));
        assert!(scan_bytes(b"\\cite*{a}"));
    }

    #[test]
    fn detects_citation_at_end_of_input() {
        assert!(scan_bytes(b"text \\cite"));
    }

    #[test]
    fn ignores_longer_commands_sharing_a_prefix() {
        assert!(!scan_bytes(b"\\citation{a} \\citestyle{b}"));
    }

    #[test]
    fn ignores_documents_without_citations() {
        assert!(!scan_bytes(b"\\section{Intro} plain text"));
        assert!(!scan_bytes(b""));
    }
}
