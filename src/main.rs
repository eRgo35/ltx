use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;

mod cli;
mod commands;
mod messages;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::New { project_name }) => commands::new_command(project_name),
        Some(Commands::Build { main_file }) => commands::build_command(main_file),
        Some(Commands::Watch { main_file }) => commands::watch_command(main_file),
        Some(Commands::Clean) => commands::clean_command(),
        None => Ok(()),
    };

    if let Err(err) = result {
        eprintln!("{} {}", "✖".bold().red(), err.to_string());
        std::process::exit(1);
    }
}
