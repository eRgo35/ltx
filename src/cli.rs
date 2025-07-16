use clap::{Parser, Subcommand};

const NAME: Option<&str> = option_env!("CARGO_PKG_NAME");
const AUTHOR: Option<&str> = option_env!("CARGO_PKG_AUTHORS");
const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
const ABOUT: Option<&str> = option_env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser)]
#[command(
    name = NAME.unwrap_or("ltx"),
    author = AUTHOR.unwrap_or("Michał Czyż <mike@c2yz.com>"),
    version = VERSION.unwrap_or("unknown"),
    about = ABOUT.unwrap_or("A command-line tool for managing LaTeX projects"),
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(aliases = ["n", "init"], about = "Initialize a new LaTeX project")]
    New { project_name: Option<String> },

    #[command(alias = "b", about = "Build the LaTeX document")]
    Build {
        #[arg(short, long, default_value = "main.tex")]
        main_file: String,
    },

    #[command(
        alias = "w",
        about = "Watch for changes and rebuild the LaTeX document"
    )]
    Watch {
        #[arg(short, long, default_value = "main.tex")]
        main_file: String,
    },

    #[command(alias = "c", about = "Clean build artifacts")]
    Clean,
}
