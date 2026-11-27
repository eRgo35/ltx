use std::error::Error;
use std::path::Path;
use std::process::Command;

use crate::commands::build_command;

pub fn run_command(main_file: Option<&Path>, verbose: bool) -> Result<(), Box<dyn Error>> {
    let pdf_file = build_command(main_file, verbose)?;

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(&pdf_file).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        // Empty "" is the window title; otherwise `start` treats a quoted path as the title.
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&pdf_file)
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(&pdf_file).spawn()?;
    }

    Ok(())
}
