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
        Some(Commands::Build { main_file }) => commands::build_command(main_file.as_deref()),
        Some(Commands::Watch { main_file }) => commands::watch_command(main_file.as_deref()),
        Some(Commands::Clean) => commands::clean_command(),
        Some(Commands::Run { main_file }) => commands::run_command(main_file.as_deref()),
        None => Ok(()),
    };

    if let Err(err) = result {
        eprintln!("{} {}", "✖".bold().red(), err);
        std::process::exit(1);
    }
}
