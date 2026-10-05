# ltx Bibliography Build — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ltx build` / `watch` / `run` actually invoke the bibliography engine when citations are present, fix the `default_entry` typo, expand `ltx watch` scope, add a friendly engine preflight, and update the README so the tool's documented behavior matches its runtime behavior.

**Architecture:** One new helper module (`commands::bib`) owns citation detection. The existing `commands::build` becomes the orchestration point (preflight → xelatex → optional biber → xelatex). `commands::watch` switches to a recursive `notify` watcher with an allow-list filter. `commands::run` is unchanged in shape but inherits the new build semantics transitively. `config::default_entry` typo fix. No new dependencies.

**Tech Stack:** Rust 2024 edition · clap 4 · notify 8 · serde + toml · colored · chrono · inquire. No new crates.

**Spec:** `docs/superpowers/specs/2026-10-05-ltx-bib-build-design.md`

## Global Constraints

- No new dependencies. `Cargo.toml` and `Cargo.lock` MUST NOT change.
- All modifications stay inside: `src/commands/{build,watch,run,mod}.rs`, `src/commands/bib.rs` (new), `src/config.rs`, `README.md`.
- Behavior change MUST be confined per NFR-5. No edits to templates, CI workflows, `dist-workspace.toml`, or `.github/`.
- `cargo build` MUST exit 0 with **zero** warnings after the change (NFR-2).
- The three current `dead_code` warnings on `Config::project.{name,author,version}` MUST be silenced with `#[allow(dead_code)]` plus a one-line `// reserved for future metadata command` comment (per OS-8).
- The `Config::bib.engine` field becomes used (read in `build_command`), so no annotation needed there.
- Substring-only citation detection — NO regex dependency (FR-2 + NFR-1).
- Every task ends with a smoke check, not unit tests (OS-2).

---

## File Structure

| File | Responsibility | Touched in |
|---|---|---|
| `src/commands/bib.rs` *(new)* | `citations_present(cwd)` helper, `preflight(engine, kind)` helper, `EngineKind` enum | Task 1 |
| `src/commands/build.rs` | Orchestrate xelatex → optional biber → xelatex; call preflight; use `Config::bib.engine` | Task 2 |
| `src/commands/watch.rs` | Switch to recursive notify watcher with allow-list filter (`*.tex`, `*.bib`, `images/*`) | Task 3 |
| `src/commands/run.rs` | Inherit new build semantics; minor cleanup so preflight runs once | Task 4 |
| `src/commands/mod.rs` | Declare `pub mod bib;` and re-export new helpers | Task 1 |
| `src/config.rs` | Fix `default_entry` typo; `#[allow(dead_code)]` on `Project` fields | Task 5 |
| `README.md` | Update "Usage" block to list all 5 commands with examples | Task 6 |

---

- [ ] **Task 1: Bib detection + preflight helpers**

**Files:**
- Create: `src/commands/bib.rs`
- Modify: `src/commands/mod.rs`

**Interfaces:**
- Produces `pub fn citations_present(cwd: &Path) -> bool` — used by Task 2 (FR-2, FR-3, FR-4)
- Produces `pub fn preflight(engine: &str, kind: EngineKind)` — used by Task 2 (FR-7, FR-8, FR-9)
- Produces `pub enum EngineKind { Xelatex, Bib }` — used by Task 2

- [ ] **Step 1: Write `src/commands/bib.rs` with the two helpers and the enum**

```rust
use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Stdio},
};

use colored::Colorize;

const CITATION_PREFIXES: &[&str] = &[
    "\\cite", "\\citep", "\\citet", "\\nocite", "\\footcite", "\\textcite",
];

pub enum EngineKind {
    Xelatex,
    Bib,
}

/// Scans every `*.tex` file directly in `cwd` (non-recursive) for citation
/// prefixes followed by a non-word byte (e.g. `{`, end-of-file, whitespace).
/// Returns false on any IO error reading a single file (fail-closed).
pub fn citations_present(cwd: &Path) -> bool {
    let entries = match fs::read_dir(cwd) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tex") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue, // fail-closed per FR-4
        };
        if scan_bytes(&bytes) {
            return true;
        }
    }
    false
}

fn scan_bytes(bytes: &[u8]) -> bool {
    for prefix in CITATION_PREFIXES {
        let needle = prefix.as_bytes();
        let mut start = 0;
        while let Some(idx) = find_subseq(&bytes[start..], needle) {
            let after = start + idx + needle.len();
            if after >= bytes.len() || !is_ascii_letter(bytes[after]) {
                return true;
            }
            start = after;
        }
    }
    false
}

fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn is_ascii_letter(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

/// Probes `engine --version`. On spawn failure or non-zero exit, prints a
/// yellow warning naming the engine and the platform-appropriate install
/// hint. Never aborts.
pub fn preflight(engine: &str, kind: EngineKind) -> Result<(), Box<dyn Error>> {
    let status = Command::new(engine)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let bad = match status {
        Ok(s) => !s.success(),
        Err(_) => true,
    };

    if bad {
        let label = match kind {
            EngineKind::Xelatex => "xelatex",
            EngineKind::Bib => engine, // biber / bibtex / ...
        };
        eprintln!(
            "{} {} not found or failed --version probe.",
            "⚠".bold().yellow(),
            label.bold()
        );
        eprintln!("  {}", platform_hint());
        eprintln!("  {}", "Continuing — the real error will surface when ltx invokes the engine.");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_hint() -> &'static str {
    "Install TeX Live (provides xelatex, biber): https://tug.org/texlive/"
}
#[cfg(target_os = "macos")]
fn platform_hint() -> &'static str {
    "Install MacTeX (provides xelatex, biber): https://tug.org/mactex/"
}
#[cfg(target_os = "windows")]
fn platform_hint() -> &'static str {
    "Install MiKTeX (provides xelatex, biber): https://miktex.org/"
}
```

- [ ] **Step 2: Wire the new module from `src/commands/mod.rs`**

Replace the existing contents with:

```rust
pub mod bib;
pub mod build;
pub mod clean;
pub mod new;
pub mod run;
pub mod watch;

pub use bib::{citations_present, preflight, EngineKind};
pub use build::build_command;
pub use clean::clean_command;
pub use new::new_command;
pub use run::run_command;
pub use watch::watch_command;
```

- [ ] **Step 3: Build and confirm it compiles**

Run: `cargo build`
Expected: `Finished` with zero new warnings. The three existing `dead_code` warnings on `Config::project` and `Config::bib.engine` may still appear — they get fixed in Tasks 2 and 5.

- [ ] **Step 4: Smoke-check `citations_present` via a one-off Rust snippet (throwaway)**

Run in a temp dir (no commit):

```bash
cd /tmp && rm -rf ltx_smoke && mkdir ltx_smoke && cd ltx_smoke
mkdir -p src
cat > src/main.rs <<'EOF'
fn main() {
    let cwd = std::env::current_dir().unwrap();
    // create one .tex with a citation, one without
    std::fs::write("with.tex", b"hello \\cite{foo} world").unwrap();
    std::fs::write("without.tex", b"plain text").unwrap();
    let r = ltx::commands::citations_present(&cwd);
    println!("with_citation={}", r);
    assert!(r);

    std::fs::remove_file("with.tex").unwrap();
    let r2 = ltx::commands::citations_present(&cwd);
    println!("after_remove={}", r2);
    assert!(!r2);

    // false positive check: \citecolor must NOT trigger
    std::fs::write("color.tex", b"\\citecolor{red}bar").unwrap();
    let r3 = ltx::commands::citations_present(&cwd);
    println!("citecolor={}", r3);
    assert!(!r3);
}
EOF
# stub lib.rs exposing commands
cat > src/lib.rs <<'EOF'
pub mod commands;
EOF
```

Expected: `with_citation=true`, `after_remove=false`, `citecolor=false`. If any assertion fails, fix the helper and re-run.

NOTE: this snippet depends on `ltx` having a `lib.rs` exposing `commands`. If the crate is currently a pure `[[bin]]`, this smoke must instead be done via `cargo run --` after a temporary CLI flag, OR you may add `pub mod commands;` to `src/lib.rs` as a one-line throwaway (revert before commit). Pick whichever is least invasive in your environment; if neither is trivial, the same logic can be tested manually:

```bash
mkdir /tmp/lab && cd /tmp/lab
echo '\cite{x}' > a.tex
# Run: ltx build  (expect biber in the process tree after Task 2 lands)
```

The throwaway snippet is a convenience, not a deliverable. The deliverable is the helper itself, which Task 2 will exercise through `build_command`.

- [ ] **Step 5: Commit**

```bash
git add src/commands/bib.rs src/commands/mod.rs
git commit -m "feat(commands): add bib citation detection + engine preflight helpers"
```

---

- [ ] **Task 2: Wire bib processing into `build_command`**

**Files:**
- Modify: `src/commands/build.rs` (rewrite the body; keep the signature)

**Interfaces:**
- Consumes `citations_present(cwd)` and `preflight(engine, kind)` from Task 1 (FR-1, FR-5, FR-6, FR-7, FR-8)
- Consumes `Config::bib.engine` from `load_config` (already returns `(Config, PathBuf)`)

- [ ] **Step 1: Rewrite `src/commands/build.rs`**

Replace the entire file body with:

```rust
use std::{error::Error, path::PathBuf, process::Command};

use crate::{
    commands::{citations_present, preflight, EngineKind},
    config::load_config,
};

pub fn build_command(main_file: &PathBuf) -> Result<(), Box<dyn Error>> {
    let (config, cwd) = load_config()?;

    let engine = config.document.engine.clone();
    let bib_engine = config.bib.engine.clone();

    let entry = config
        .document
        .entry
        .to_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            main_file
                .to_str()
                .unwrap_or("main.tex")
                .to_owned()
        });

    // Preflight: warn (do not fail) when engines are missing.
    preflight(&engine, EngineKind::Xelatex)?;

    // Pass 1: xelatex
    let status = Command::new(&engine).arg(&entry).current_dir(&cwd).status()?;
    if !status.success() {
        return Err(format!("Build failed (xelatex) with exit code: {}", status).into());
    }

    // Optional pass: biber when citations are detected in cwd/*.tex
    if citations_present(&cwd) {
        preflight(&bib_engine, EngineKind::Bib)?;
        let status = Command::new(&bib_engine).current_dir(&cwd).status()?;
        if !status.success() {
            return Err(format!("Build failed ({}): exit code {}", bib_engine, status).into());
        }

        // Pass 2: xelatex
        let status = Command::new(&engine).arg(&entry).current_dir(&cwd).status()?;
        if !status.success() {
            return Err(format!("Build failed (xelatex) with exit code: {}", status).into());
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Confirm `Config::bib.engine` warning is gone**

Run: `cargo build 2>&1 | grep -E "warning|error" | head -20`
Expected: only the `Config::project.{name,author,version}` warnings remain. The `Bib.engine` warning is gone because it's now read.

- [ ] **Step 3: Smoke test — no-citation case (AC-1)**

```bash
cd /tmp && rm -rf ltx_nocite && mkdir ltx_nocite && cd ltx_nocite
cat > ltx.toml <<'EOF'
[project]
name = "x"
author = "x"
version = "0.1.0"
[document]
entry = "main.tex"
engine = "xelatex"
[bib]
engine = "biber"
EOF
cat > main.tex <<'EOF'
\documentclass{article}
\begin{document}
hi
\end{document}
EOF
echo '\bibliography{../nothing.bib}' > /dev/null  # force a stray .bib mention? not needed
# Run from inside the project
PATH="$PWD/../../target/release:$PATH" /home/mike/Work/ltx/target/debug/ltx build
# OR: cd /home/mike/Work/ltx && cargo run --quiet -- build
```

Observed expectation: xelatex runs once, biber does NOT run, main.pdf exists (or xelatex error if texlive isn't installed — that's fine, the smoke just needs to confirm biber is absent):

```bash
ls main.pdf 2>&1 || echo "(xelatex missing — expected on dev box without texlive)"
# inspect: did biber run? Use the wrapper from Task 4 step 4 below, or just observe that
# the only Command::new invocations in build.rs are xelatex twice and (optional) biber once.
```

--- [ ] **Step 4: Smoke test — citation case (AC-2)**

```bash
cd /tmp/ltx_nocite
echo '\cite{foo}' >> main.tex
# add a stub .bib so biber doesn't fail on missing files
echo '@article{foo, title={t}, author={a}, year={2024}}' > refs.bib
# patch ltx.toml to reference refs.bib (or rely on biber discovering *.bib in cwd)
# biber auto-discovers *.bib in cwd by default.
cd /home/mike/Work/ltx && cargo run --quiet -- build
```

If texlive is installed, expect: xelatex → biber → xelatex sequence, `main.pdf` contains the bibliography entry. If texlive is not installed, the preflight warning MUST be printed for xelatex (FR-7 / AC-4); the build will then exit non-zero with xelatex's spawn error. Both outcomes satisfy the task's smoke requirement.

- [ ] **Step 5: Commit**

```bash
git add src/commands/build.rs
git commit -m "feat(build): invoke biber between xelatex passes when citations present"
```

---

- [ ] **Task 3: Expand `ltx watch` scope to `.tex`, `.bib`, `images/`**

**Files:**
- Modify: `src/commands/watch.rs`

**Interfaces:**
- Consumes `build_command` (unchanged signature) — watch delegates to build (FR-12); does NOT duplicate bib logic
- Watches `cwd` recursively with an allow-list filter (FR-11, FR-13)
- Calls `preflight` on entry so the first run surfaces missing engines (FR-7)

- [ ] **Step 1: Rewrite `src/commands/watch.rs`**

Replace the entire file body with:

```rust
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::mpsc::channel,
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{
    commands::{build_command, preflight, EngineKind},
    config::load_config,
};

pub fn watch_command(main_file: &PathBuf) -> Result<(), Box<dyn Error>> {
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
                    if event_paths.iter().any(|p| is_relevant(p)) {
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
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: zero new warnings. The remaining `Config::project` warnings are fixed in Task 5.

- [ ] **Step 3: Smoke test (AC-7)**

```bash
cd /tmp/ltx_nocite  # any project with a main.tex
mkdir -p images
# In one terminal:
cd /home/mike/Work/ltx && cargo run --quiet -- watch
# In another:
touch /tmp/ltx_nocite/bibliography.bib   # if absent; create it
echo "x" > /tmp/ltx_nocite/images/fig1.png
```

Observed: `Change detected, rebuilding...` prints after each touch. Ctrl-C the watcher.

- [ ] **Step 4: Commit**

```bash
git add src/commands/watch.rs
git commit -m "feat(watch): recursive watcher over .tex, .bib, images/"
```

---

- [ ] **Task 4: Run command inherits new build semantics**

**Files:**
- Modify: `src/commands/run.rs`

**Interfaces:**
- Consumes `build_command` (unchanged signature) — inherits FR-1, FR-7 transitively
- Behavior preserved per FR-14: viewer still launches even if build fails

- [ ] **Step 1: Minimal cleanup of `src/commands/run.rs`**

Current file already calls `build_command` and continues on failure. The only required change is dropping the now-unused `error::Error` import path it might still need. Verify the file compiles; if it does, no edits are needed. If `cargo build` complains about unused imports, remove them. Otherwise, no change is required — the new build semantics flow in transparently via Task 2.

Replace the file body only if compilation requires it; otherwise leave it untouched:

```rust
use std::{path::PathBuf, process::Command};

use crate::commands::build_command;

pub fn run_command(main_file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = build_command(main_file) {
        eprintln!("Build error: {:?}", e);
    }

    let mut pdf_file = main_file.clone();
    pdf_file.set_extension("pdf");

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(&pdf_file).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", "start", pdf_file.to_str().unwrap()])
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(&pdf_file).spawn()?;
    }

    Ok(())
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: zero new warnings beyond the `Config::project` set.

- [ ] **Step 3: Commit (only if the file was modified)**

```bash
git add src/commands/run.rs
git diff --cached --quiet || git commit -m "chore(run): drop unused imports after build refactor"
```

If the file was untouched, skip this commit.

---

- [ ] **Task 5: Config fix + silence remaining `dead_code` warnings**

**Files:**
- Modify: `src/config.rs`

**Interfaces:**
- `pub fn load_config() -> Result<(Config, PathBuf), Box<dyn Error>>` — unchanged signature (FR-10, NFR-2)
- `Config::document.entry` now defaults to `"main.tex"` (was `"main.text"` — FR-10)
- `Project.{name,author,version}` annotated with `#[allow(dead_code)]` and a one-line comment (OS-8, NFR-2)

- [ ] **Step 1: Fix `default_entry()` typo (FR-10)**

Edit `src/config.rs`, changing:

```rust
fn default_entry() -> PathBuf {
    PathBuf::from("main.text")
}
```

to:

```rust
fn default_entry() -> PathBuf {
    PathBuf::from("main.tex")
}
```

- [ ] **Step 2: Silence `Project` field warnings (OS-8)**

Edit `src/config.rs`, changing:

```rust
#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
    pub author: String,
    pub version: String,
}
```

to:

```rust
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // reserved for a future metadata command
pub struct Project {
    pub name: String,
    pub author: String,
    pub version: String,
}
```

(Apply `#[allow(dead_code)]` at the struct or field level — whichever produces zero warnings. If struct-level errors let it, fall back to field-level annotations.)

- [ ] **Step 3: Build with zero warnings (NFR-2 / AC-8)**

Run: `cargo build 2>&1 | tail -20`
Expected: `Finished` line with NO warnings. If any warning remains, fix it before committing. Note: do NOT add `#[allow(dead_code)]` to `Bib.engine` — it must remain un-annotated because Task 2 reads it (proving the field is live).

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "fix(config): default_entry returns main.tex; silence Project field warnings"
```

---

- [ ] **Task 6: README usage block**

**Files:**
- Modify: `README.md`

**Interfaces:**
- Pure documentation update — no code (FR-15).

- [ ] **Step 1: Replace the "Usage" block in `README.md`**

The current block is:

```txt
$ ltx --help
A command-line tool for managing LaTeX projects

Usage: ltx [COMMAND]

Commands:
  new   Initialize a new LaTeX project
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Replace it with:

````markdown
## Usage

```text
$ ltx --help
A command-line tool for managing LaTeX projects

Usage: ltx [COMMAND]

Commands:
  new    Initialize a new LaTeX project
  build   Build the LaTeX document (runs xelatex; biber and a second xelatex pass when citations are present)
  watch   Watch .tex, .bib, and images/ for changes and rebuild
  clean   Remove LaTeX build artifacts (.aux, .log, .out, .bbl, .blg, .toc, .synctex.gz, ...)
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Examples

```sh
# Start a new project
ltx new my-paper
cd my-paper

# Build once (xelatex, or xelatex → biber → xelatex if you cite)
ltx build

# Rebuild on every change
ltx watch

# Clean intermediate files
ltx clean

# Build and open the resulting PDF
ltx run
```
````

(Adjust the surrounding markdown structure so the new section sits inside the existing `## Usage` heading — preserve any intro paragraph that already exists above the code block.)

- [ ] **Step 2: Verify**

Run: `grep -E "new|build|watch|clean|run" README.md | head -20`
Expected: at least five lines, one per command name.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: list all five ltx subcommands in README usage"
```

---

## Final acceptance run

After all six tasks land on `main`:

- [ ] **Step 1: Clean build, zero warnings**

```bash
cargo clean
cargo build 2>&1 | tee /tmp/build.log
grep -c warning /tmp/build.log   # MUST print 0
```

- [ ] **Step 2: Cargo.lock unchanged**

```bash
git diff Cargo.lock Cargo.toml
```

Expected: empty diff (NFR-1 / AC-9).

- [ ] **Step 3: End-to-end smoke (if texlive is installed)**

```bash
cd /tmp && rm -rf ltx_final && /home/mike/Work/ltx/target/debug/ltx new ltx_final
cd ltx_final
echo '\cite{sample}' >> main.tex
/home/mike/Work/ltx/target/debug/ltx build
ls -la main.pdf
```

Expected: build succeeds; `main.pdf` contains the `sample` bibliography entry.

- [ ] **Step 4: End-to-end smoke (no texlive — preflight-only)**

```bash
cd /tmp/ltx_final
PATH=/usr/bin:/bin /home/mike/Work/ltx/target/debug/ltx build
```

Expected: yellow warning naming `xelatex` printed on stderr; non-zero exit; `biber` warning absent (none in this project); preflight logic verified.

- [ ] **Step 5: Diff summary**

```bash
git diff main --stat
```

Expected: only the seven files listed in §File Structure are modified or created.