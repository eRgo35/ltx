use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

use crate::commands::build_command;

pub fn run_command(main_file: &PathBuf) -> Result<(), Box<dyn Error>> {
    if let Err(e) = build_command(main_file) {
        eprintln!("Build error: {:?}", e);
    }

    let mut pdf_file = main_file.clone();
    pdf_file.set_extension("pdf");

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(&pdf_file).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", "start", pdf_file.to_str().unwrap()])
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(&pdf_file).spawn()?;
    }

    Ok(())
}
