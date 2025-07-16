use colored::Colorize;

pub fn print_creating_message(project_path: &std::path::Path) {
    println!();
    println!(
        "{} Creating a new LaTeX project in {}",
        "✓".bold().green(),
        project_path.display().to_string().bold()
    );
    println!();
}

pub fn print_created_message(project_name: &str, project_path: &std::path::Path) {
    println!(
        "{} Success! Created {} at {}",
        "✓".bold().green(),
        project_name.bold(),
        project_path.display().to_string().bold()
    );
    println!();
    println!("Inside that directory, you can run several commands:");
    println!();
    println!("  {} Build the LaTeX document", "ltx build".bold().cyan());
    println!(
        "  {} Watch for changes and rebuild",
        "ltx watch".bold().cyan()
    );
    println!("  {} Clean build artifacts", "ltx clean".bold().cyan());
    println!();
    println!("We suggest that you begin by typing:");
    println!();
    println!("  {} {}", "cd".bold().cyan(), project_name.bold());
    println!(
        "  {} {}",
        "ltx build".bold().cyan(),
        "# Build your LaTeX document".dimmed()
    );
}
