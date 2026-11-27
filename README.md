# ltx

A command-line tool for managing LaTeX projects

## What is ltx?

`ltx` is a command-line tool designed to simplify the management of LaTeX projects. It provides commands for creating new projects, managing bibliographies, and more, all while integrating with Git for version control.

## Features

- **Project Creation**: Easily create new LaTeX projects with a predefined structure.
- **Git Integration**: Automatically initializes a Git repository for your project.
- **Build**: Runs `xelatex`, adds a `biber` pass when the document cites sources, and reruns when LaTeX asks for it (TOC, cross-references).
- **Watch**: Rebuilds on changes to `.tex`, `.bib` and files under `images/`.
- **Clean / Run**: Removes intermediate files; builds and opens the PDF.
- **Configuration**: Entry file and engines are set in `ltx.toml` (`--main-file` overrides the entry).

## Installation

Prebuilt binaries for Linux, macOS and Windows are on the [releases page](https://github.com/eRgo35/ltx/releases).

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/eRgo35/ltx/releases/latest/download/ltx-installer.sh | sh

# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/eRgo35/ltx/releases/latest/download/ltx-installer.ps1 | iex"

# From source
cargo install --git https://github.com/eRgo35/ltx
```

`ltx build` requires a TeX distribution providing `xelatex` and `biber` (TeX Live, MacTeX or MiKTeX).

## Building

To build the project, you can use the following command:

```bash
cargo build --release
```

## Usage

```txt
$ ltx --help
A command-line tool for managing LaTeX projects

Usage: ltx [COMMAND]

Commands:
  new    Initialize a new LaTeX project
  build  Build the LaTeX document
  watch  Watch for changes and rebuild the LaTeX document
  clean  Clean build artifacts
  run    Run the compiled document
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```
