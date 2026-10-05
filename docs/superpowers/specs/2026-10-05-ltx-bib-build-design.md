# ltx Bibliography Build — Design

**Author:** omp (assistant) on behalf of `mike`
**Date:** 2026-10-05
**Status:** Draft
**Reviewers:** `mike`
**Spec target:** `ltx` CLI v0.1.x → next minor

---

## 1. Context

`ltx` is a Rust CLI that wraps a small LaTeX project lifecycle (`new`, `build`, `watch`, `clean`, `run`). The repo ships:

- Five clap subcommands (`src/cli.rs`)
- A `Config` struct (`src/config.rs`) with a `[bib] engine = "biber"` field that no command actually consults
- A `build_command` (`src/commands/build.rs`) that shells `xelatex` once and stops — meaning `\` from any bibliography file the template ships (`templates/bibliography.bib`) is never processed, even when a project cites it
- A template (`templates/main.tex`) referencing the missing `report` / `book` source, and a typo in `default_entry()` returning `"main.text"` instead of `"main.tex"`
- Three active `dead_code` warnings from `Project`/`Bib` fields no command reads

The README advertises bibliography management; in practice `ltx build` produces PDFs without ever resolving citations. The user calls this "unfinished" and approved wiring real bib processing as the smallest MVP that finishes the project.

This spec covers: real bibliography execution in `build` / `run` / `watch`, the entry-file typo fix, watch scope expansion, an engine preflight warning when engines are missing, and README updates. It does not cover new commands, new flags, or tests (intentional, see §9).

## 2. Functional Requirements

### Bib processing

- **FR-1.** `ltx build` MUST invoke `xelatex` and, when citations are present in any `.tex` file, additionally invoke the bib engine specified by `ltx.toml`'s `[bib].engine` (default `biber`) between two `xelatex` runs. The full pass sequence MUST be `xelatex → biber (optional) → xelatex`.
- **FR-2.** A `.tex` file MUST be considered to contain citations when at least one byte substring matches a citation prefix followed by either `{` or an ASCII letter that is **not** `[A-Za-z]` (i.e. a non-word character, including `{`, end-of-file, whitespace, punctuation). Recognised prefixes: `\cite`, `\citep`, `\citet`, `\nocite`, `\footcite`, `\textcite`. The check MUST be a case-sensitive byte-level substring search against the file's contents; no regex dependency is allowed. This rule prevents false positives on commands such as `\citecolor{...}` while still accepting `\cite{...}`, `\citep`, `\nocite*`, and trailing-of-line citations.
- **FR-3.** Citation detection MUST scan every `*.tex` file directly in the current working directory (non-recursive). Files in subdirectories MUST NOT be scanned.
- **FR-4.** If reading a `.tex` file fails (permission, IO), citation detection MUST treat that file as having no citations (fail-closed) and continue scanning the remaining files. A scan error MUST NOT abort the build.
- **FR-5.** If no `.tex` file in cwd contains a citation substring, the bib engine MUST NOT be invoked, regardless of whether `.bib` files exist.
- **FR-6.** The bib engine invocation MUST pass the engine's executable name as argv[0] with no additional arguments. Output MUST stream to the user's terminal as it would if the user ran the engine directly.

### Engine preflight

- **FR-7.** Before the first invocation of `xelatex`, `ltx build` and `ltx run` MUST attempt `Command::new("xelatex").arg("--version")`. If the command cannot be spawned (engine not in PATH) or exits non-zero, `ltx` MUST print a yellow warning on stderr naming the missing engine and the platform-appropriate install hint, then continue. The build itself will still surface the real failure from the subsequent `Command::new(...).status()`.
- **FR-8.** When FR-1 elects to invoke the bib engine, the preflight MUST additionally probe that engine via `Command::new(<engine>).arg("--version")` and follow the same warning policy as FR-7.
- **FR-9.** Preflight warnings MUST include a per-platform install hint:
  - Linux: `Install TeX Live (provides xelatex, biber): https://tug.org/texlive/`
  - macOS: `Install MacTeX (provides xelatex, biber): https://tug.org/mactex/`
  - Windows: `Install MiKTeX (provides xelatex, biber): https://miktex.org/`

### Config fix

- **FR-10.** `Config::default_entry()` in `src/config.rs` MUST return `PathBuf::from("main.tex")`. The current `"main.text"` value is a bug that breaks any project relying on the default entry.

### Watch scope

- **FR-11.** `ltx watch` MUST watch the current working directory recursively for any path ending in `.tex`, `.bib`, or contained in an `images/` directory at any depth. Edits to such paths MUST trigger a rebuild.
- **FR-12.** `ltx watch` MUST continue to invoke `build_command` (which carries FR-1's bib logic). Watch MUST NOT duplicate build logic.
- **FR-13.** `ltx watch` MUST continue to print `Watching ... for changes...` on startup, naming the entry file (existing behavior preserved).

### Run command

- **FR-14.** `ltx run` MUST call `build_command` first; if the build fails, the viewer MUST still be launched (existing behavior preserved) and the underlying build error MUST be reported on stderr.

### Documentation

- **FR-15.** `README.md` MUST list all five subcommands (`new`, `build`, `watch`, `clean`, `run`) with one-line descriptions and an example invocation for each. The "Usage" code block MUST be updated from its current "new + help only" content.

## 3. Non-Functional Requirements

- **NFR-1.** No new runtime dependencies. The implementation MUST use only crates already present in `Cargo.toml` (`chrono`, `clap`, `colored`, `inquire`, `notify`, `serde`, `toml`).
- **NFR-2.** `cargo build` MUST succeed with zero warnings after the change. The three current `dead_code` warnings on `Project.name/author/version` and `Bib.engine` MUST be resolved either by use or by `#[allow(dead_code)]` with a single-line comment explaining the reservation.
- **NFR-3.** The citation-detection helper MUST NOT allocate more than one `String` per scanned `.tex` file (read once into a buffer; substring search in place).
- **NFR-4.** Preflight probes MUST time out gracefully: stderr/stdout from `--version` MUST be discarded; only the exit status matters.
- **NFR-5.** Behavior change MUST be confined to `src/commands/{build,watch,run}.rs`, `src/config.rs`, and `README.md`. No other file (templates, CI workflows, `Cargo.toml`, `dist-workspace.toml`) MUST be modified.

## 4. Acceptance Criteria

- **AC-1.** Given a project created by `ltx new demo` with the unmodified template, when `cd demo && ltx build` is invoked with no citations in any `.tex` file, then `xelatex` is invoked exactly once, `biber` is invoked zero times, and `main.pdf` is produced.
  - Covers FR-1, FR-5.
- **AC-2.** Given the same project with `\cite{sample}` added to `main.tex`, when `ltx build` is invoked, then the process tree executes `xelatex`, then `biber`, then `xelatex`, in that order; `biber` is invoked exactly once; `main.pdf` references the `sample` bibliography entry.
  - Covers FR-1, FR-2, FR-6.
- **AC-3.** Given a `.tex` file in a subdirectory (`chapters/intro.tex`) that contains `\cite{foo}`, when `ltx build` is invoked from the project root, then `biber` is NOT invoked (subdirectory `.tex` files are out of scope per FR-3).
  - Covers FR-3.
- **AC-4.** Given `xelatex` not in PATH, when `ltx build` is invoked, then a yellow warning naming `xelatex` and the Linux TeX Live install hint is printed to stderr before the build proceeds; the final exit code is non-zero (the underlying `Command::status()` reports the missing engine).
  - Covers FR-7, FR-9.
- **AC-5.** Given citations present and `biber` not in PATH, when `ltx build` is invoked, then a yellow warning naming `biber` is printed in addition to the `xelatex` warning; the build proceeds and exits non-zero.
  - Covers FR-8, FR-9.
- **AC-6.** Given the default `ltx.toml` produced by `ltx new`, when `load_config()` returns a `Config`, then `Document.entry` is `PathBuf::from("main.tex")` (not `main.text`).
  - Covers FR-10.
- **AC-7.** Given `ltx watch demo` running in a project with `bibliography.bib` and an `images/` directory, when the user edits `bibliography.bib` or a file under `images/`, then `build_command` is invoked.
  - Covers FR-11, FR-12.
- **AC-8.** Given the project builds cleanly, when `cargo build` is invoked, then it exits 0 with no warnings.
  - Covers NFR-2.
- **AC-9.** Given a fresh checkout, when `cargo build` is invoked, then the resolved dependency tree is byte-identical to `Cargo.lock` (no new crates).
  - Covers NFR-1.
- **AC-10.** Given the README's "Usage" block, when read by a new user, then it lists `ltx new`, `ltx build`, `ltx watch`, `ltx clean`, and `ltx run` with example invocations.
  - Covers FR-15.

## 5. Edge Cases

- **EC-1.** `.tex` file has no read permission. Citation scan treats it as "no citations"; build proceeds without biber. (FR-4)
- **EC-2.** `.tex` file is empty (zero bytes). Citation scan returns false; biber not invoked. (FR-2, FR-5)
- **EC-3.** `.tex` file is a symlink to a regular file. Read proceeds normally; symlink target is read once. (FR-4)
- **EC-4.** `ltx.toml` is malformed. `load_config` returns its existing error; preflight and build are not reached. (existing behavior preserved)
- **EC-5.** `ltx.toml` is missing `[bib].engine`. The default `biber` is used per `default_bib_engine`. (existing behavior preserved)
- **EC-6.** User's PATH contains `xelatex` but the binary fails `--version` (e.g., a wrapper that exits 7). Preflight warns; build proceeds and surfaces the real error from the actual run. (FR-7)
- **EC-7.** `xelatex` is found but `biber` is not, with citations present. Both warnings printed; build surfaces biber's missing-binary error. (FR-7, FR-8)
- **EC-8.** Build run from a subdirectory of the project (e.g., `cd chapters && ltx build`). `load_config` returns its existing "ltx.toml not found" error. (existing behavior preserved)
- **EC-9.** `images/` directory does not exist. Watch still works for `*.tex` and `*.bib`; the missing directory is silently ignored. (FR-11)
- **EC-10.** `.bib` file is present but no `.tex` cites it. Citation scan finds nothing; biber not invoked; the `.bib` file is ignored. (FR-5)

## 6. API Contracts

### Internal Rust API

```rust
// src/commands/bib.rs (new)
/// Returns true if any *.tex file directly in `cwd` contains a citation
/// prefix (case-sensitive substring match against the file bytes).
///
/// On any IO error reading a single file, that file is treated as having
/// no citations and the scan continues.
pub fn citations_present(cwd: &std::path::Path) -> bool;
```

```rust
// src/commands/build.rs (modified)
/// Preflight-probes the given engine by invoking `<engine> --version`,
/// discarding output. On spawn failure or non-zero exit, prints a yellow
/// warning naming the engine and the platform-appropriate install hint
/// (Linux / macOS / Windows). Never aborts.
fn preflight(engine: &str, missing_kind: EngineKind);

pub enum EngineKind {
    Xelatex,
    Bib,
}
```

### CLI Surface (unchanged shape, expanded behavior)

```
ltx build [-f <entry>]
ltx watch [-f <entry>]
ltx run   [-f <entry>]
```

No new flags. No new subcommands.

### Observed Process Tree (when citations present)

```
ltx build
  └── xelatex main.tex
  └── biber
  └── xelatex main.tex
```

## 7. Data Models

| Entity | Field | Type | Constraints |
|---|---|---|---|
| `Config.document.entry` | entry | `PathBuf` | Defaults to `"main.tex"`. Was `"main.text"` — fixed. |
| `Config.document.engine` | engine | `String` | Defaults to `"xelatex"`. Read by build. |
| `Config.bib.engine` | bib engine | `String` | Defaults to `"biber"`. Read by build when citations present. |
| `Config.project.{name,author,version}` | metadata | `String` | Reserved for a future metadata command; currently unused → `#[allow(dead_code)]` with comment. |

## 8. Out of Scope

- **OS-1.** New CLI flags, new subcommands, or new `ltx.toml` keys. (Reason: smallest MVP; the user explicitly approved no surface changes.)
- **OS-2.** Automated tests (unit or integration). (Reason: smallest MVP; smoke verification covers acceptance. A test harness adds dependencies on `tempfile` and CI work that dwarfs the change.)
- **OS-3.** Recursive citation detection into subdirectories. (Reason: scoped per clarifying question; subdirectory `.tex` files are uncommon in ltx's single-document project model.)
- **OS-4.** Configurable pass count or pass customization. (Reason: `xelatex → biber → xelatex` is the standard minimum.)
- **OS-5.** Persistent preflight cache between invocations. (Reason: preflight is two `Command::status()` calls — cheap enough to repeat.)
- **OS-6.** Engine path override via `LTX_ENGINE` env var. (Reason: no current precedent for xelatex override; `LTX_GIT_OVERRIDE` already exists for `new` only.)
- **OS-7.** Updating `Cargo.toml`, `dist-workspace.toml`, or `.github/workflows/*.yml`. (Reason: NFR-5; only code + README change.)
- **OS-8.** Resolving the three current `dead_code` warnings by deleting the fields. (Reason: `Project` fields are reserved for future metadata; deleting would break the next thing.)

---

## 9. Acceptance Checklist (operator-facing)

For the human or CI agent executing the plan:

1. `cargo build` exits 0 with zero warnings.
2. `cargo build --release` exits 0 (sanity).
3. `target/debug/ltx new demo && cd demo` succeeds; project tree matches template.
4. Add `\cite{sample}` to `demo/main.tex`. Run `target/debug/ltx build`. Observe `xelatex → biber → xelatex` in stdout. Verify `demo/main.pdf` contains the bibliography entry.
5. Run `target/debug/ltx watch` in `demo/`; touch `demo/bibliography.bib`; observe rebuild.
6. Inspect `README.md` "Usage" section: all five commands present with examples.
7. `git diff --stat` shows changes only in `src/commands/{build,watch,run,bib}.rs`, `src/config.rs`, `README.md`, and (expected) `Cargo.lock` unchanged.