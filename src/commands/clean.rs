use std::fs;

pub fn clean_command() -> Result<(), Box<dyn std::error::Error>> {
    let exts = [
        "aux",
        "log",
        "out",
        "bbl",
        "blg",
        "toc",
        "lof",
        "lot",
        "fls",
        "fdb_latexmk",
        "synctex.gz",
    ];

    let current_dir = std::env::current_dir()?;
    for entry in fs::read_dir(&current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.contains(&ext) {
                fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}
