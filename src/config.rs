use std::{env::current_dir, error::Error, fs::read_to_string, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // reserved for a future metadata command
pub struct Project {
    pub name: String,
    pub author: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    #[serde(default = "default_entry")]
    pub entry: PathBuf,
    #[serde(default = "default_engine")]
    pub engine: String,
}

#[derive(Debug, Deserialize)]
pub struct Bib {
    #[serde(default = "default_bib_engine")]
    pub engine: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    pub project: Project,
    pub document: Document,
    pub bib: Bib,
}

fn default_entry() -> PathBuf {
    PathBuf::from("main.tex")
}

fn default_engine() -> String {
    String::from("xelatex")
}

fn default_bib_engine() -> String {
    String::from("biber")
}

pub fn load_config() -> Result<(Config, PathBuf), Box<dyn Error>> {
    let cwd = current_dir()?;
    let manifest_path = cwd.join("ltx.toml");
    if !manifest_path.exists() {
        return Err("ltx.toml not found in current directory".into());
    }

    let content = read_to_string(&manifest_path)?;
    let cfg: Config = toml::from_str(&content)?;

    Ok((cfg, cwd))
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::path::Path;

    #[test]
    fn parses_project_template() {
        let toml = include_str!("../templates/ltx.toml")
            .replace("{project_name}", "demo")
            .replace("{author_name}", "Author");
        let cfg: Config = toml::from_str(&toml).unwrap();
        assert_eq!(cfg.project.name, "demo");
        assert_eq!(cfg.document.entry, Path::new("main.tex"));
        assert_eq!(cfg.document.engine, "xelatex");
        assert_eq!(cfg.bib.engine, "biber");
    }

    #[test]
    fn document_and_bib_keys_have_defaults() {
        let toml = r#"
            [project]
            name = "demo"
            author = "Author"
            version = "0.1.0"
            [document]
            [bib]
        "#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.document.entry, Path::new("main.tex"));
        assert_eq!(cfg.document.engine, "xelatex");
        assert_eq!(cfg.bib.engine, "biber");
    }

    #[test]
    fn custom_entry_and_engines_are_read() {
        let toml = r#"
            [project]
            name = "demo"
            author = "Author"
            version = "0.1.0"
            [document]
            entry = "thesis.tex"
            engine = "lualatex"
            [bib]
            engine = "bibtex"
        "#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.document.entry, Path::new("thesis.tex"));
        assert_eq!(cfg.document.engine, "lualatex");
        assert_eq!(cfg.bib.engine, "bibtex");
    }
}
