use std::{env::current_dir, error::Error};

use colored::Colorize;

use crate::{
    commands::{
        bib::{platform_hint, version_output},
        new::get_git_command,
    },
    config::{Config, load_config},
};

/// Checks everything `ltx build` and `ltx new` depend on and reports each
/// item on one line. Fails when something a build needs is missing.
pub fn doctor_command() -> Result<(), Box<dyn Error>> {
    let mut problems = 0;
    let manifest = current_dir()?.join("ltx.toml");

    // Without ltx.toml (e.g. outside a project) check the default engines.
    let config = if manifest.exists() {
        match load_config() {
            Ok((config, _)) => Some(config),
            Err(e) => {
                fail("ltx.toml", &e.to_string(), None);
                problems += 1;
                None
            }
        }
    } else {
        None
    };
    let defaults = Config::default();
    let cfg = config.as_ref().unwrap_or(&defaults);

    for engine in [&cfg.document.engine, &cfg.bib.engine] {
        match version_output(engine) {
            Some(version) => pass(engine, first_line(&version)),
            None => {
                fail(
                    engine,
                    "not found or `--version` failed",
                    Some(platform_hint()),
                );
                problems += 1;
            }
        }
    }

    let git = get_git_command();
    match version_output(&git) {
        Some(version) => pass(&git, first_line(&version)),
        None => warn(&git, "not found; `ltx new` cannot initialize a repository"),
    }

    match &config {
        Some(config) => {
            let entry = &config.document.entry;
            let output = config.document.output_dir();
            if !entry.exists() {
                fail(
                    "ltx.toml",
                    &format!("entry `{}` does not exist", entry.display()),
                    None,
                );
                problems += 1;
            } else if let Err(e) = output {
                fail("ltx.toml", &e, None);
                problems += 1;
            } else {
                let output = match output {
                    Ok(Some(dir)) => format!("{}/", dir.display()),
                    _ => "project root".to_owned(),
                };
                pass(
                    "ltx.toml",
                    &format!("entry {}, output {}", entry.display(), output),
                );
            }
        }
        None if !manifest.exists() => warn(
            "ltx.toml",
            "not found in the current directory; checked default engines",
        ),
        None => {} // already reported as a parse error
    }

    match problems {
        0 => Ok(()),
        1 => Err("1 problem found".into()),
        n => Err(format!("{n} problems found").into()),
    }
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

fn pass(name: &str, detail: &str) {
    eprintln!("  {} {:<10} {}", "✓".green().bold(), name.bold(), detail);
}

fn warn(name: &str, detail: &str) {
    eprintln!("  {} {:<10} {}", "⚠".yellow().bold(), name.bold(), detail);
}

fn fail(name: &str, detail: &str, hint: Option<&str>) {
    eprintln!("  {} {:<10} {}", "✗".red().bold(), name.bold(), detail);
    if let Some(hint) = hint {
        eprintln!("    {hint}");
    }
}

#[cfg(test)]
mod tests {
    use super::first_line;

    #[test]
    fn first_line_skips_blank_lines() {
        assert_eq!(
            first_line("\n  biber version: 2.21\nmore"),
            "biber version: 2.21"
        );
        assert_eq!(first_line(""), "");
    }
}
