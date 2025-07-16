# ltx

A command-line tool for managing LaTeX projects

## What is ltx?

`ltx` is a command-line tool designed to simplify the management of LaTeX projects. It provides commands for creating new projects, managing bibliographies, and more, all while integrating with Git for version control.

## Features

- **Project Creation**: Easily create new LaTeX projects with a predefined structure.
- **Git Integration**: Automatically initializes a Git repository for your project.
- More features to be added in the future.

## Installation

```bash
makepkg -si # For Arch Linux users
```

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
  new   Initialize a new LaTeX project
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```
