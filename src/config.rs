use std::{env::current_dir, error::Error, fs::read_to_string, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
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
pub struct Config {
    pub project: Project,
    pub document: Document,
    pub bib: Bib,
}

fn default_entry() -> PathBuf {
    PathBuf::from("main.text")
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
