//! Turns TeX logs into short, cargo-style diagnostics.
//!
//! Expects logs written with `-file-line-error` and with line wrapping
//! disabled (`max_print_line`), see `build.rs`.

use std::{fs, path::Path};

use colored::Colorize;

#[derive(Debug, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    /// Source file as written in the log, relative to the project root.
    pub file: Option<String>,
    pub line: Option<usize>,
    /// Source text up to the point where TeX stopped (from the `l.N` line).
    pub context: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn message(message: &str) -> Self {
        Diagnostic {
            message: message.to_owned(),
            file: None,
            line: None,
            context: None,
            help: None,
        }
    }
}

/// Errors from a TeX log, in the order they occurred.
pub fn parse_errors(log: &str) -> Vec<Diagnostic> {
    let lines: Vec<&str> = log.lines().collect();
    let mut errors = Vec::new();
    let mut aborted = None;

    for (i, line) in lines.iter().enumerate() {
        let (file, number, message) = if let Some((file, number, message)) = split_file_line(line) {
            (Some(file), Some(number), message)
        } else if let Some(message) = line.strip_prefix("! ") {
            (None, None, message)
        } else {
            continue;
        };

        let message = clean_message(message);
        if message == "Emergency stop" {
            // A consequence of an earlier error; the reason follows as `*** (...)`.
            aborted = lines[i + 1..]
                .iter()
                .take(5)
                .find_map(|l| l.strip_prefix("*** ("))
                .map(|r| r.trim_end_matches(')').to_owned());
            continue;
        }

        errors.push(Diagnostic {
            message,
            file: file.map(|f| f.strip_prefix("./").unwrap_or(f).to_owned()),
            line: number,
            context: find_context(&lines[i + 1..], number),
            help: None,
        });
    }

    // Errors inside a .aux file come from a stale one written by an earlier
    // build (e.g. before a package was removed); they cascade, so show one.
    let aux = |d: &Diagnostic| d.file.as_deref().is_some_and(|f| f.ends_with(".aux"));
    if let Some(first) = errors.iter().position(aux) {
        let file = errors[first].file.clone().unwrap_or_default();
        let n = errors.iter().filter(|d| aux(d)).count();
        errors.retain(|d| !aux(d));
        let mut d = Diagnostic::message(&format!(
            "`{file}` left over from an earlier build is invalid ({n} errors)"
        ));
        d.help = Some("run `ltx clean`, then build again".to_owned());
        errors.insert(first.min(errors.len()), d);
    }

    if errors.is_empty()
        && let Some(reason) = aborted
    {
        errors.push(Diagnostic::message(&reason));
    }
    errors
}

/// Warnings worth a user's attention: undefined references and citations,
/// duplicate labels. Layout noise (over/underfull boxes) is left out.
pub fn parse_warnings(log: &str) -> Vec<Diagnostic> {
    let mut warnings: Vec<Diagnostic> = Vec::new();
    for line in log.lines() {
        let Some((_, message)) = line.split_once("Warning: ") else {
            continue;
        };
        let relevant = message.starts_with("Reference ")
            || message.starts_with("Citation ")
            || (message.starts_with("Label ") && message.contains("multiply defined"));
        if !relevant {
            continue;
        }
        let d = Diagnostic::message(&clean_message(message));
        if !warnings.contains(&d) {
            warnings.push(d);
        }
    }
    warnings
}

/// Error lines from a biber `.blg` file.
pub fn parse_biber_errors(blg: &str) -> Vec<Diagnostic> {
    // biber parses a temporary UTF-8 copy of the .bib and reports that path.
    // With a single data source we can point at the real file instead.
    let sources: Vec<&str> = blg
        .lines()
        .filter_map(|l| l.split_once("Found BibTeX data source '"))
        .filter_map(|(_, rest)| rest.strip_suffix('\''))
        .collect();

    blg.lines()
        .filter_map(|l| l.split_once("ERROR - ").map(|(_, m)| m.trim()))
        .map(
            |message| match (sources.as_slice(), bib_syntax_error(message)) {
                ([source], Some((line, rest))) => Diagnostic {
                    message: rest.to_owned(),
                    file: Some((*source).to_owned()),
                    line: Some(line),
                    context: None,
                    help: None,
                },
                _ => Diagnostic::message(message),
            },
        )
        .collect()
}

/// `BibTeX subsystem: /tmp/x.utf8, line 14, syntax error: ...` -> (14, "syntax error: ...")
fn bib_syntax_error(message: &str) -> Option<(usize, &str)> {
    let rest = message.strip_prefix("BibTeX subsystem: ")?;
    let (_, rest) = rest.split_once(", line ")?;
    let (line, rest) = rest.split_once(", ")?;
    Some((line.parse().ok()?, rest))
}

/// Renders a diagnostic like rustc/cargo. `root` is used to read the source
/// line for the snippet; when it cannot be read, the snippet is omitted.
pub fn render(level: &str, d: &Diagnostic, root: &Path) -> String {
    let label = match level {
        "error" => level.red().bold(),
        _ => level.yellow().bold(),
    };
    let mut out = format!("{}{} {}", label, ":".bold(), d.message.bold());
    out += &snippet(d, root);
    if let Some(help) = &d.help {
        out += &format!("\n{}{} {}", "help".cyan().bold(), ":".bold(), help);
    }
    out
}

/// The `--> file:line` part with the source line, when there is one.
fn snippet(d: &Diagnostic, root: &Path) -> String {
    let mut out = String::new();
    let (Some(file), Some(line)) = (&d.file, d.line) else {
        return out;
    };
    let gutter = line.to_string().len();
    let pad = " ".repeat(gutter);
    let bar = "|".blue().bold();
    out += &format!("\n{}{} {}:{}", pad, "-->".blue().bold(), file, line);

    let Some(source) = read_line(&root.join(file), line) else {
        return out;
    };
    out += &format!("\n{pad} {bar}");
    out += &format!("\n{} {bar} {}", line.to_string().blue().bold(), source);
    if let Some((start, len)) = d.context.as_deref().and_then(|c| underline(&source, c)) {
        out += &format!(
            "\n{pad} {bar} {}{}",
            " ".repeat(start),
            "^".repeat(len).red().bold()
        );
    }
    out
}

/// Splits `./main.tex:12: message` into its parts.
fn split_file_line(line: &str) -> Option<(&str, usize, &str)> {
    // Scan for `:<digits>: ` so that Windows drive letters (`C:\`) are skipped.
    let bytes = line.as_bytes();
    for (i, _) in line.match_indices(':') {
        let digits = bytes[i + 1..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let end = i + 1 + digits;
        if digits > 0 && line[end..].starts_with(": ") {
            let file = &line[..i];
            if file.is_empty() || file.contains(' ') && !file.contains('/') {
                return None;
            }
            return Some((file, line[i + 1..end].parse().ok()?, &line[end + 2..]));
        }
    }
    None
}

/// Finds the `l.N text` line that belongs to an error.
fn find_context(following: &[&str], number: Option<usize>) -> Option<String> {
    for line in following.iter().take(12) {
        if split_file_line(line).is_some() || line.starts_with("! ") {
            return None; // reached the next error
        }
        if let Some(rest) = line.strip_prefix("l.") {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            let n: usize = rest[..digits].parse().ok()?;
            if number.is_some_and(|expected| expected != n) {
                return None;
            }
            return Some(rest[digits..].strip_prefix(' ').unwrap_or("").to_owned());
        }
    }
    None
}

fn clean_message(message: &str) -> String {
    let message = message.strip_prefix("LaTeX Error: ").unwrap_or(message);
    message.trim().trim_end_matches('.').to_owned()
}

fn read_line(path: &Path, line: usize) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let source = text.lines().nth(line.checked_sub(1)?)?;
    Some(source.replace('\t', "    "))
}

/// Column and width (in chars) of the token TeX stopped at: the trailing
/// control sequence of `context`, or its last character.
fn underline(source: &str, context: &str) -> Option<(usize, usize)> {
    // TeX prints a tab as `^^I` in the log.
    let context = context.replace("^^I", "\t").replace('\t', "    ");
    let context = context.trim_end();
    if context.is_empty() || !source.starts_with(context) {
        return None;
    }
    let end = context.chars().count();
    let tail: String = context
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '@')
        .collect();
    let width = if !tail.is_empty() && context[..context.len() - tail.len()].ends_with('\\') {
        tail.len() + 1
    } else {
        1
    };
    Some((end - width, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
(./e.tex
./e.tex:5: Undefined control sequence.
l.5 Hello \\foo
              \\ world.
The control sequence at the end of the top line

LaTeX Warning: Reference `nope' on page 1 undefined on input line 5.
LaTeX Warning: Reference `nope' on page 1 undefined on input line 5.
LaTeX Warning: Citation `x' on page 1 undefined on input line 5.
Overfull \\hbox (12.0pt too wide) in paragraph at lines 7--8
./e.tex:8: LaTeX Error: \\begin{itemize} on input line 7 ended by \\end{enumerate}.

See the LaTeX manual or LaTeX Companion for explanation.
l.8 \\end{enumerate}
LaTeX Warning: Label `a' multiply defined.
LaTeX Warning: There were undefined references.
";

    #[test]
    fn parses_errors_with_location_and_context() {
        let errors = parse_errors(LOG);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].message, "Undefined control sequence");
        assert_eq!(errors[0].file.as_deref(), Some("e.tex"));
        assert_eq!(errors[0].line, Some(5));
        assert_eq!(errors[0].context.as_deref(), Some("Hello \\foo"));
        assert_eq!(
            errors[1].message,
            "\\begin{itemize} on input line 7 ended by \\end{enumerate}"
        );
        assert_eq!(errors[1].context.as_deref(), Some("\\end{enumerate}"));
    }

    #[test]
    fn stale_aux_errors_collapse_into_one() {
        let log = "./main.aux:2: Undefined control sequence.\nl.2 \\abx@aux\n\
                   ./main.aux:3: Undefined control sequence.\nl.3 \\abx@aux\n\
                   ./main.tex:3: Undefined control sequence.\nl.3 \\bad\n";
        let errors = parse_errors(log);
        assert_eq!(errors.len(), 2);
        assert_eq!(
            errors[0].message,
            "`main.aux` left over from an earlier build is invalid (2 errors)"
        );
        assert!(errors[0].help.is_some());
        assert_eq!(errors[1].file.as_deref(), Some("main.tex"));
    }

    #[test]
    fn emergency_stop_after_a_real_error_is_dropped() {
        let log = "! LaTeX Error: File `nosuchpkg.sty' not found.\n\
                   ./p.tex:3: Emergency stop.\nl.3 \\begin\n\
                   *** (cannot \\read from terminal in nonstop modes)\n";
        let errors = parse_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "File `nosuchpkg.sty' not found");
        assert_eq!(errors[0].file, None);
    }

    #[test]
    fn lone_emergency_stop_reports_the_abort_reason() {
        let log = "! Emergency stop.\n<*> t.tex\n\n*** (job aborted, no legal \\end found)\n";
        let errors = parse_errors(log);
        assert_eq!(
            errors,
            vec![Diagnostic::message("job aborted, no legal \\end found")]
        );
    }

    #[test]
    fn warnings_are_filtered_and_deduplicated() {
        let messages: Vec<String> = parse_warnings(LOG).into_iter().map(|d| d.message).collect();
        assert_eq!(
            messages,
            [
                "Reference `nope' on page 1 undefined on input line 5",
                "Citation `x' on page 1 undefined on input line 5",
                "Label `a' multiply defined",
            ]
        );
    }

    #[test]
    fn parses_biber_errors() {
        let blg = "[0] Config.pm:307> INFO - This is Biber 2.19\n\
                   [12] Utils.pm:411> ERROR - Cannot find 'main.bcf'!\n";
        assert_eq!(
            parse_biber_errors(blg),
            vec![Diagnostic::message("Cannot find 'main.bcf'!")]
        );
    }

    #[test]
    fn biber_syntax_error_points_at_the_single_bib_file() {
        let blg = "[9] bibtex.pm:1519> INFO - Found BibTeX data source 'refs.bib'\n\
                   [10] Utils.pm:480> ERROR - BibTeX subsystem: /tmp/biber_tmp_x/ab.utf8, line 14, syntax error: at end of input\n";
        let errors = parse_biber_errors(blg);
        assert_eq!(errors[0].file.as_deref(), Some("refs.bib"));
        assert_eq!(errors[0].line, Some(14));
        assert_eq!(errors[0].message, "syntax error: at end of input");
    }

    #[test]
    fn split_file_line_handles_paths() {
        assert_eq!(
            split_file_line("./a.tex:3: msg"),
            Some(("./a.tex", 3, "msg"))
        );
        assert_eq!(
            split_file_line("C:\\p\\a.tex:12: msg"),
            Some(("C:\\p\\a.tex", 12, "msg"))
        );
        assert_eq!(split_file_line("Package x Warning: on input line 3."), None);
        assert_eq!(split_file_line("Some text: 12: not a location"), None);
    }

    #[test]
    fn underline_marks_control_sequence_or_last_char() {
        assert_eq!(underline("Hello \\foo world", "Hello \\foo"), Some((6, 4)));
        assert_eq!(underline("a} b", "a}"), Some((1, 1)));
        assert_eq!(underline("Zażółć \\x y", "Zażółć \\x"), Some((7, 2)));
        assert_eq!(underline("other line", "Hello"), None);
        assert_eq!(
            underline("    Hello \\foo", "^^IHello \\foo"),
            Some((10, 4))
        );
    }
}
