use std::fs;

use crate::config::load_config;

// Matched against the end of the file name, so multi-dot suffixes work.
const ARTIFACT_SUFFIXES: &[&str] = &[
    ".aux",
    ".log",
    ".out",
    ".bbl",
    ".blg",
    ".bcf",
    ".run.xml",
    ".toc",
    ".lof",
    ".lot",
    ".fls",
    ".fdb_latexmk",
    ".synctex.gz",
    ".xdv",
];

/// Removes the output directory (when `document.output` is set) and any
/// intermediate files in the project root, e.g. from builds before it was set.
pub fn clean_command() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;

    // Works without ltx.toml too; a present but invalid one is an error.
    if current_dir.join("ltx.toml").exists() {
        let (config, _) = load_config()?;
        if let Some(out) = config.document.output_dir()? {
            let out = current_dir.join(out);
            if out.is_dir() {
                fs::remove_dir_all(&out)?;
            }
        }
    }

    for entry in fs::read_dir(&current_dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && is_artifact(name)
        {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn is_artifact(name: &str) -> bool {
    ARTIFACT_SUFFIXES.iter().any(|s| name.ends_with(s))
}

#[cfg(test)]
mod tests {
    use super::is_artifact;

    #[test]
    fn matches_single_suffixes() {
        for name in ["main.aux", "main.log", "main.bcf", "main.xdv", "main.toc"] {
            assert!(is_artifact(name), "{name}");
        }
    }

    #[test]
    fn matches_multi_dot_suffixes() {
        assert!(is_artifact("main.synctex.gz"));
        assert!(is_artifact("main.run.xml"));
    }

    #[test]
    fn keeps_sources_and_outputs() {
        for name in [
            "main.tex",
            "main.pdf",
            "bibliography.bib",
            "ltx.toml",
            "README.md",
            "figure.gz",
            "data.xml",
        ] {
            assert!(!is_artifact(name), "{name}");
        }
    }
}
