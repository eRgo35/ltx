use std::{
    error::Error,
    path::Path,
    sync::mpsc::{channel, Receiver, RecvTimeoutError},
    time::Duration,
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{
    commands::{build_command, preflight, EngineKind},
    config::load_config,
};

/// Quiet period after the last relevant change before rebuilding.
const DEBOUNCE: Duration = Duration::from_millis(300);

type WatchEvent = notify::Result<Event>;

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
            Ok(event) => {
                if !is_relevant_event(event) {
                    continue;
                }
                // One editor save often emits several events: wait for quiet.
                if !wait_for_quiet(&rx) {
                    break;
                }
                println!("Change detected, rebuilding...");
                if let Err(e) = build_command(main_file) {
                    eprintln!("Build error: {:?}", e);
                }
            }
            Err(e) => {
                eprintln!("Watch channel closed: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Drains events until none arrive for `DEBOUNCE`. Returns false if the
/// channel closed.
fn wait_for_quiet(rx: &Receiver<WatchEvent>) -> bool {
    loop {
        match rx.recv_timeout(DEBOUNCE) {
            Ok(event) => {
                is_relevant_event(event);
            }
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

fn is_relevant_event(event: WatchEvent) -> bool {
    match event {
        Ok(event) => {
            matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
                && event.paths.iter().any(|p| is_relevant(p))
        }
        Err(e) => {
            eprintln!("Watch error: {:?}", e);
            false
        }
    }
}

fn is_relevant(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str())
        && (ext == "tex" || ext == "bib")
    {
        return true;
    }
    // Anything under images/ at any depth
    path.components().any(|c| c.as_os_str() == "images")
}
