use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;

mod cli;
mod commands;
mod config;
mod messages;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::New { project_name }) => commands::new_command(project_name),
        Some(Commands::Build { main_file, verbose }) => {
            commands::build_command(main_file.as_deref(), verbose).map(|_| ())
        }
        Some(Commands::Watch { main_file, verbose }) => {
            commands::watch_command(main_file.as_deref(), verbose)
        }
        Some(Commands::Clean) => commands::clean_command(),
        Some(Commands::Run { main_file, verbose }) => {
            commands::run_command(main_file.as_deref(), verbose)
        }
        None => Ok(()),
    };

    if let Err(err) = result {
        eprintln!("{}{} {}", "error".red().bold(), ":".bold(), err);
        std::process::exit(1);
    }
}
