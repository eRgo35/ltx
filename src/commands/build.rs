use std::{error::Error, path::PathBuf, process::Command};

use crate::{
    commands::{citations_present, preflight, EngineKind},
    config::load_config,
};

pub fn build_command(main_file: &PathBuf) -> Result<(), Box<dyn Error>> {
    let (config, cwd) = load_config()?;

    let engine = config.document.engine.clone();
    let bib_engine = config.bib.engine.clone();

    let entry = config
        .document
        .entry
        .to_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            main_file
                .to_str()
                .unwrap_or("main.tex")
                .to_owned()
        });

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
        let status = Command::new(&bib_engine).current_dir(&cwd).status()?;
        if !status.success() {
            return Err(format!("Build failed ({}): exit code {}", bib_engine, status).into());
        }

        // Pass 2: xelatex
        let status = Command::new(&engine).arg(&entry).current_dir(&cwd).status()?;
        if !status.success() {
            return Err(format!("Build failed (xelatex) with exit code: {}", status).into());
        }
    }

    Ok(())
}
