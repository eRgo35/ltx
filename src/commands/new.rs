use std::fmt;

use colored::Colorize;
use inquire::{Select, Text};

use crate::messages;

pub fn get_git_command() -> String {
    std::env::var("LTX_GIT_OVERRIDE").unwrap_or_else(|_| "git".to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Template {
    Article,
    Report,
    Beamer,
    Thesis,
    Letter,
}

impl Template {
    const ALL: [Template; 5] = [
        Template::Article,
        Template::Report,
        Template::Beamer,
        Template::Thesis,
        Template::Letter,
    ];

    fn name(self) -> &'static str {
        match self {
            Template::Article => "article",
            Template::Report => "report",
            Template::Beamer => "beamer",
            Template::Thesis => "thesis",
            Template::Letter => "letter",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Template::Article => "Short document with sections",
            Template::Report => "Chapters, table of contents and bibliography",
            Template::Beamer => "Presentation slides",
            Template::Thesis => "Title page, abstract, chapters in chapters/, bibliography",
            Template::Letter => "Formal letter",
        }
    }

    /// Files specific to this template, as (path in the project, contents).
    fn files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Template::Article => &[("main.tex", include_str!("../../templates/article/main.tex"))],
            Template::Report => &[("main.tex", include_str!("../../templates/report/main.tex"))],
            Template::Beamer => &[("main.tex", include_str!("../../templates/beamer/main.tex"))],
            Template::Thesis => &[
                ("main.tex", include_str!("../../templates/thesis/main.tex")),
                (
                    "chapters/introduction.tex",
                    include_str!("../../templates/thesis/chapters/introduction.tex"),
                ),
                (
                    "chapters/conclusion.tex",
                    include_str!("../../templates/thesis/chapters/conclusion.tex"),
                ),
            ],
            Template::Letter => &[("main.tex", include_str!("../../templates/letter/main.tex"))],
        }
    }

    fn has_bibliography(self) -> bool {
        self != Template::Letter
    }
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:<8} {}", self.name(), self.description())
    }
}

/// Creates a project from `template`. Without a project name, asks for the
/// name and, unless `template` is given, for the template too.
pub fn new_command(
    project_name: Option<String>,
    template: Option<Template>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut project_name, template) = match project_name {
        Some(name) if !name.trim().is_empty() => (
            name.trim().to_string(),
            template.unwrap_or(Template::Article),
        ),
        _ => {
            let name = interactive_prompt()?;
            let template = match template {
                Some(t) => t,
                None => Select::new("Which template?", Template::ALL.to_vec()).prompt()?,
            };
            (name, template)
        }
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

    create_project_structure(template, &project_name, &author_name, &date, &project_path)?;

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
    template: Template,
    project_name: &str,
    author_name: &str,
    date: &str,
    project_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(project_path)?;
    std::fs::create_dir_all(project_path.join("images"))?;

    let fill = |text: &str| {
        text.replace("{project_name}", project_name)
            .replace("{author_name}", author_name)
            .replace("{date}", date)
    };

    let mut files: Vec<(&str, &str)> = template.files().to_vec();
    files.push(("ltx.toml", include_str!("../../templates/ltx.toml")));
    files.push(("README.md", include_str!("../../templates/README.md")));
    if template.has_bibliography() {
        files.push((
            "bibliography.bib",
            include_str!("../../templates/bibliography.bib"),
        ));
    }
    for (path, contents) in files {
        let path = project_path.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, fill(contents))?;
    }

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

#[cfg(test)]
mod tests {
    use super::Template;

    #[test]
    fn every_template_has_a_main_document() {
        for t in Template::ALL {
            let main = t.files().iter().find(|(p, _)| *p == "main.tex");
            assert!(
                main.is_some_and(|(_, c)| c.contains("\\documentclass")),
                "{t:?}"
            );
        }
    }

    #[test]
    fn templates_use_only_known_placeholders() {
        for t in Template::ALL {
            for (path, contents) in t.files() {
                let filled = contents
                    .replace("{project_name}", "")
                    .replace("{author_name}", "")
                    .replace("{date}", "");
                for leftover in ["{project", "{author", "{date"] {
                    assert!(!filled.contains(leftover), "{t:?} {path}");
                }
            }
        }
    }

    #[test]
    fn only_documents_that_cite_get_a_bibliography() {
        for t in Template::ALL {
            let cites = t.files().iter().any(|(_, c)| c.contains("\\cite"));
            assert!(!cites || t.has_bibliography(), "{t:?} cites without a .bib");
        }
        assert!(!Template::Letter.has_bibliography());
    }
}
