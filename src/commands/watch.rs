use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{error::Error, path::PathBuf, sync::mpsc::channel};

use crate::commands::build_command;

pub fn watch_command(main_file: &PathBuf) -> Result<(), Box<dyn Error>> {
    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx, notify::Config::default())?;

    watcher.watch(&main_file, RecursiveMode::NonRecursive)?;

    println!(
        "Watching {} for changes...",
        main_file.to_str().unwrap_or("unknown")
    );

    loop {
        match rx.recv() {
            Ok(event) => {
                if let Ok(event) = event {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        println!("Change detected, rebuilding...");
                        if let Err(e) = build_command(&main_file) {
                            eprintln!("Build error: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
    }
}
