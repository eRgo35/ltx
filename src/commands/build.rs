use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    commands::{citations_present, preflight, EngineKind},
    config::load_config,
};

/// Builds the document and returns the path of the produced PDF.
/// `main_file` overrides `document.entry` from `ltx.toml` when given.
pub fn build_command(main_file: Option<&Path>) -> Result<PathBuf, Box<dyn Error>> {
    let (config, cwd) = load_config()?;

    let engine = config.document.engine.clone();
    let bib_engine = config.bib.engine.clone();

    let entry = main_file
        .map(Path::to_path_buf)
        .unwrap_or(config.document.entry);

    // The engine writes outputs to cwd, named after the entry's stem (jobname).
    let jobname = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid entry file: {}", entry.display()))?
        .to_owned();

    // Preflight: warn (do not fail) when engines are missing.
    preflight(&engine, EngineKind::Xelatex)?;

    // Pass 1: xelatex
    let status = Command::new(&engine).arg(&entry).current_dir(&cwd).status()?;
    if !status.success() {
        return Err(format!("Build failed (xelatex) with exit code: {}", status).into());
    }

    // Optional pass: biber when citations are detected in cwd/*.tex
    if citations_present(&cwd) {
        preflight(&bib_engine, EngineKind::Bib)?;
        let status = Command::new(&bib_engine)
            .arg(&jobname)
            .current_dir(&cwd)
            .status()?;
        if !status.success() {
            return Err(format!("Build failed ({}): exit code {}", bib_engine, status).into());
        }

        // Pass 2: xelatex
        let status = Command::new(&engine).arg(&entry).current_dir(&cwd).status()?;
        if !status.success() {
            return Err(format!("Build failed (xelatex) with exit code: {}", status).into());
        }
    }

    Ok(cwd.join(format!("{jobname}.pdf")))
}
