use colored::Colorize;
use inquire::Text;

use crate::messages;

fn get_git_command() -> String {
    std::env::var("LTX_GIT_OVERRIDE").unwrap_or_else(|_| "git".to_string())
}

pub fn new_command(project_name: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut project_name = match project_name {
        Some(name) if !name.trim().is_empty() => name.trim().to_string(),
        _ => interactive_prompt()?,
    };

    let current_dir = std::env::current_dir()?;
    let project_path = current_dir.join(&project_name);
    if project_path.exists() {
        if project_path.read_dir()?.next().is_some() {
            return Err(format!(
                "Project directory '{}' is not empty.",
                project_path.display()
            )
            .into());
        }

        project_name = project_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("my-latex-project")
            .to_string();
    }
    let author_name = get_author_name();
    let date = chrono::Utc::now().format("%B %Y").to_string();

    messages::print_creating_message(&project_path);

    create_project_structure(&project_name, &author_name, &date, &project_path)?;

    messages::print_created_message(&project_name, &project_path);

    Ok(())
}

fn interactive_prompt() -> Result<String, Box<dyn std::error::Error>> {
    loop {
        let project_name = Text::new("What is your project named?")
            .with_default("my-latex-project")
            .with_help_message("Press Enter to use the default name")
            .prompt()?;

        if project_name.contains(' ') {
            eprintln!("{} Project name cannot contain spaces", "✖".bold().red());
            continue;
        }

        if !project_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            eprintln!(
                "{} Project name can only contain alphanumeric characters, hyphens, and underscores",
                "✖".bold().red()
            );
            continue;
        }

        return Ok(project_name);
    }
}

fn create_project_structure(
    project_name: &str,
    author_name: &str,
    date: &str,
    project_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(project_path)?;
    std::fs::create_dir_all(project_path.join("images"))?;

    let main_tex = include_str!("../../templates/main.tex");
    let bibliography_bib = include_str!("../../templates/bibliography.bib");
    let ltx_toml = include_str!("../../templates/ltx.toml");
    let readme_md = include_str!("../../templates/README.md");

    let main_tex = main_tex
        .replace("{project_name}", project_name)
        .replace("{author_name}", author_name)
        .replace("{date}", date);

    let bibliography_bib = bibliography_bib
        .replace("{project_name}", project_name)
        .replace("{author_name}", author_name)
        .replace("{date}", date);

    let ltx_toml = ltx_toml
        .replace("{project_name}", project_name)
        .replace("{author_name}", author_name);

    let readme_md = readme_md.replace("{project_name}", project_name);

    std::fs::write(project_path.join("main.tex"), main_tex)?;
    std::fs::write(project_path.join("bibliography.bib"), bibliography_bib)?;
    std::fs::write(project_path.join("ltx.toml"), ltx_toml)?;
    std::fs::write(project_path.join("README.md"), readme_md)?;

    if let Err(e) = std::process::Command::new(get_git_command())
        .arg("init")
        .current_dir(project_path)
        .status()
    {
        eprintln!(
            "{} Failed to initialize git repository: {}",
            "✖".bold().red(),
            e
        );
    }

    Ok(())
}

fn get_author_name() -> String {
    std::process::Command::new(get_git_command())
        .arg("config")
        .arg("user.name")
        .output()
        .ok()
        .and_then(|output| {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if name.is_empty() { None } else { Some(name) }
        })
        .unwrap_or_else(|| {
            std::env::var("USER")
                .ok()
                .and_then(|output| {
                    let name = output.trim().to_string();
                    if name.is_empty() { None } else { Some(name) }
                })
                .unwrap_or_else(|| "John Smith".to_string())
        })
}
