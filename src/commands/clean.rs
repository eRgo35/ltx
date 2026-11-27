use std::fs;

pub fn clean_command() -> Result<(), Box<dyn std::error::Error>> {
    // Matched against the end of the file name, so multi-dot suffixes work.
    let suffixes = [
        ".aux",
        ".log",
        ".out",
        ".bbl",
        ".blg",
        ".bcf",
        ".run.xml",
        ".toc",
        ".lof",
        ".lot",
        ".fls",
        ".fdb_latexmk",
        ".synctex.gz",
        ".xdv",
    ];

    let current_dir = std::env::current_dir()?;
    for entry in fs::read_dir(&current_dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && suffixes.iter().any(|s| name.ends_with(s))
        {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}
