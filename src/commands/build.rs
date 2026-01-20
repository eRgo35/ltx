use std::{error::Error, path::PathBuf, process::Command};

use crate::config::load_config;

pub fn build_command(main_file: &PathBuf) -> Result<(), Box<dyn Error>> {
    let (config, _cwd) = load_config()?;

    let engine = &config.document.engine;
    let entry = config
        .document
        .entry
        .to_str()
        .unwrap_or(main_file.to_str().unwrap_or("main.tex"));

    let status = Command::new(engine).arg(entry).status()?;

    if !status.success() {
        return Err(format!("Build failed with exit code: {}", status).into());
    }

    Ok(())
}
