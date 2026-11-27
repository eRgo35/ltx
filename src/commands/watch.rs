use std::{
    error::Error,
    path::Path,
    sync::mpsc::channel,
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{
    commands::{build_command, preflight, EngineKind},
    config::load_config,
};

pub fn watch_command(main_file: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let (config, cwd) = load_config()?;
    let engine = config.document.engine.clone();

    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, notify::Config::default())?;
    watcher.watch(&cwd, RecursiveMode::Recursive)?;

    // Best-effort preflight so the user sees a warning before the first build.
    let _ = preflight(&engine, EngineKind::Xelatex);

    println!(
        "Watching {} for changes (.tex, .bib, images/*)...",
        cwd.display()
    );

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if event.paths.iter().any(|p| is_relevant(p)) {
                        println!("Change detected, rebuilding...");
                        if let Err(e) = build_command(main_file) {
                            eprintln!("Build error: {:?}", e);
                        }
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Watch error: {:?}", e),
            Err(e) => {
                eprintln!("Watch channel closed: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

fn is_relevant(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext == "tex" || ext == "bib" {
            return true;
        }
    }
    // Anything under images/ at any depth
    path.components().any(|c| c.as_os_str() == "images")
}
