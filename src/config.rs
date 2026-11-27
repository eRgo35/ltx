use std::{
    env::current_dir,
    error::Error,
    fs::read_to_string,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

// Every section and key is optional; missing ones take the defaults below.

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
#[allow(dead_code)] // reserved for a future metadata command
pub struct Project {
    pub name: String,
    pub author: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Document {
    pub entry: PathBuf,
    pub engine: String,
    /// Directory for build outputs, relative to the project root.
    /// Unset means the project root itself.
    pub output: Option<PathBuf>,
}

impl Document {
    /// `output`, checked to be a plain subdirectory of the project, since
    /// `ltx clean` deletes it.
    pub fn output_dir(&self) -> Result<Option<&Path>, String> {
        let Some(dir) = &self.output else {
            return Ok(None);
        };
        let plain = dir.components().all(|c| matches!(c, Component::Normal(_)));
        if dir.as_os_str().is_empty() || !plain {
            return Err(format!(
                "document.output must be a subdirectory of the project, got `{}`",
                dir.display()
            ));
        }
        Ok(Some(dir))
    }
}

impl Default for Document {
    fn default() -> Self {
        Document {
            entry: PathBuf::from("main.tex"),
            engine: String::from("xelatex"),
            output: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Bib {
    pub engine: String,
}

impl Default for Bib {
    fn default() -> Self {
        Bib {
            engine: String::from("biber"),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
#[allow(dead_code)]
pub struct Config {
    pub project: Project,
    pub document: Document,
    pub bib: Bib,
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
    use super::{Config, Document};
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
    fn output_dir_must_stay_inside_the_project() {
        let doc = |output: &str| Document {
            output: Some(output.into()),
            ..Document::default()
        };
        assert_eq!(doc("build").output_dir(), Ok(Some(Path::new("build"))));
        assert_eq!(doc("out/pdf").output_dir(), Ok(Some(Path::new("out/pdf"))));
        for bad in ["", ".", "..", "../x", "build/../..", "/tmp/x"] {
            assert!(doc(bad).output_dir().is_err(), "{bad:?}");
        }
        assert_eq!(Document::default().output_dir(), Ok(None));
    }

    #[test]
    fn every_section_is_optional() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.project.name, "");
        assert_eq!(cfg.document.entry, Path::new("main.tex"));
        assert_eq!(cfg.bib.engine, "biber");

        let cfg: Config = toml::from_str("[document]\nentry = \"thesis.tex\"").unwrap();
        assert_eq!(cfg.document.entry, Path::new("thesis.tex"));
        assert_eq!(cfg.document.engine, "xelatex");
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
