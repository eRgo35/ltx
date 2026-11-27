use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

use colored::Colorize;

const CITATION_PREFIXES: &[&str] = &[
    "\\cite",
    "\\citep",
    "\\citet",
    "\\nocite",
    "\\footcite",
    "\\textcite",
];

/// Scans every `*.tex` file under `dir` (recursively, skipping hidden
/// directories such as `.git`) for citation prefixes followed by a non-word
/// byte (e.g. `{`, end-of-file, whitespace). Unreadable files and
/// directories are skipped.
pub fn citations_present(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // file_type() does not follow symlinks, so loops cannot recurse forever.
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if !hidden && citations_present(&path) {
                return true;
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("tex") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if scan_bytes(&bytes) {
            return true;
        }
    }
    false
}

fn scan_bytes(bytes: &[u8]) -> bool {
    bytes
        .split(|&b| b == b'\n')
        .any(|line| scan_line(strip_comment(line)))
}

/// Cuts a line at the first `%` that starts a comment (not `\%`).
fn strip_comment(line: &[u8]) -> &[u8] {
    let mut backslashes = 0;
    for (i, &b) in line.iter().enumerate() {
        match b {
            b'\\' => backslashes += 1,
            b'%' if backslashes % 2 == 0 => return &line[..i],
            _ => backslashes = 0,
        }
    }
    line
}

fn scan_line(bytes: &[u8]) -> bool {
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

/// Probes `engine --version` and returns its output. On spawn failure or
/// non-zero exit, prints a yellow warning naming the engine and the
/// platform-appropriate install hint, and returns `None`. Never aborts.
pub fn preflight(engine: &str) -> Option<String> {
    let version = version_output(engine);
    if version.is_none() {
        eprintln!(
            "{} {} not found or failed --version probe.",
            "⚠".bold().yellow(),
            engine.bold()
        );
        eprintln!("  {}", platform_hint());
        eprintln!("  Continuing — the real error will surface when ltx invokes the engine.");
    }
    version
}

/// Output of `program --version`, or `None` if it cannot be run or fails.
pub fn version_output(program: &str) -> Option<String> {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

#[cfg(target_os = "linux")]
pub fn platform_hint() -> &'static str {
    "Install TeX Live (provides xelatex, biber): https://tug.org/texlive/"
}
#[cfg(target_os = "macos")]
pub fn platform_hint() -> &'static str {
    "Install MacTeX (provides xelatex, biber): https://tug.org/mactex/"
}
#[cfg(target_os = "windows")]
pub fn platform_hint() -> &'static str {
    "Install MiKTeX (provides xelatex, biber): https://miktex.org/"
}

#[cfg(test)]
mod tests {
    use super::{citations_present, scan_bytes};
    use std::fs;

    #[test]
    fn finds_citations_in_subdirectories_but_not_hidden_ones() {
        let root = std::env::temp_dir().join(format!("ltx-cite-test-{}", std::process::id()));
        let chapters = root.join("chapters/part1");
        let hidden = root.join(".git");
        fs::create_dir_all(&chapters).unwrap();
        fs::create_dir_all(&hidden).unwrap();
        fs::write(root.join("main.tex"), "\\include{chapters/part1/intro}").unwrap();
        fs::write(hidden.join("old.tex"), "\\cite{a}").unwrap();
        assert!(!citations_present(&root), "hidden directories are skipped");

        fs::write(chapters.join("intro.tex"), "see \\cite{a}").unwrap();
        assert!(citations_present(&root));
        fs::remove_dir_all(&root).unwrap();
    }

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
    fn ignores_commented_out_citations() {
        assert!(!scan_bytes(b"% \\cite{a}\ntext"));
        assert!(!scan_bytes(b"text % see \\cite{a}"));
    }

    #[test]
    fn escaped_percent_does_not_start_a_comment() {
        assert!(scan_bytes(b"50\\% of \\cite{a}"));
        // `\\` is a line break, so the `%` after it starts a comment.
        assert!(!scan_bytes(b"a\\\\% \\cite{a}"));
    }

    #[test]
    fn ignores_documents_without_citations() {
        assert!(!scan_bytes(b"\\section{Intro} plain text"));
        assert!(!scan_bytes(b""));
    }
}
