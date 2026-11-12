//! `folio` — render Markdown with Folio's engine from the command line.
//!
//! Mainly a development and integration tool: the desktop app links the same
//! `folio-core` crate.

use folio_core::theme::{scaffold_theme, ThemeSet, DEFAULT_DARK, DEFAULT_LIGHT};
use folio_core::{render, RenderOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
folio — a quiet Markdown reader

USAGE:
    folio render <FILE|-> [OPTIONS]
    folio themes list [--dir <DIR>]
    folio themes scaffold <ID>
    folio themes json [--dir <DIR>]
    folio --version
    folio --help

RENDER OPTIONS:
    --out <FILE>              Write HTML to a file instead of stdout
    --json                    Print { html, meta } as JSON (used by the app/tests)
    --no-highlight            Disable syntax highlighting
    --no-smart-punctuation    Keep straight quotes and ASCII dashes
    --hard-breaks             Treat single newlines as <br>
    --localize                Resolve relative links/images against the file

THEMES:
    --dir <DIR>               Load additional theme JSON files from DIR

EXAMPLES:
    folio render notes.md --json | jq .meta.stats
    folio render notes.md --out notes.html
    folio themes scaffold my-theme > ~/.config/folio/themes/my-theme.json
";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            ExitCode::from(code)
        }
    }
}

fn run(args: Vec<String>) -> Result<(), (u8, String)> {
    let Some(command) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return Ok(());
    };
    match command {
        "--version" | "-V" | "version" => {
            println!("folio {}", folio_core::VERSION);
            Ok(())
        }
        "--help" | "-h" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        "render" => render_command(&args[1..]),
        "themes" => themes_command(&args[1..]),
        other => Err((2, format!("unknown command: {other}\n\n{USAGE}"))),
    }
}

struct RenderArgs {
    input: PathBuf,
    out: Option<PathBuf>,
    json: bool,
    highlight: bool,
    smart: bool,
    hard_breaks: bool,
    localize: bool,
}

fn render_command(args: &[String]) -> Result<(), (u8, String)> {
    let mut parsed = RenderArgs {
        input: PathBuf::new(),
        out: None,
        json: false,
        highlight: true,
        smart: true,
        hard_breaks: false,
        localize: false,
    };
    let mut input: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--json" => parsed.json = true,
            "--no-highlight" => parsed.highlight = false,
            "--no-smart-punctuation" => parsed.smart = false,
            "--hard-breaks" => parsed.hard_breaks = true,
            "--localize" => parsed.localize = true,
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| (2, "--out needs a file path".to_string()))?;
                parsed.out = Some(PathBuf::from(value));
            }
            flag if flag.starts_with('-') && flag != "-" => {
                return Err((2, format!("unknown option: {flag}\n\n{USAGE}")));
            }
            path => {
                if input.is_some() {
                    return Err((2, "only one input file is supported".to_string()));
                }
                input = Some(PathBuf::from(path));
            }
        }
        index += 1;
    }
    let Some(input) = input else {
        return Err((2, format!("missing input file\n\n{USAGE}")));
    };
    parsed.input = input;

    let (source, base_dir) = if parsed.input.as_os_str() == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| (1, format!("cannot read stdin: {e}")))?;
        (source, None)
    } else {
        let path = &parsed.input;
        let source = std::fs::read_to_string(path)
            .map_err(|e| (1, format!("cannot read {}: {e}", path.display())))?;
        (source, path.parent().map(Path::to_path_buf))
    };

    let options = RenderOptions {
        base_dir: if parsed.localize { base_dir } else { None },
        localize: parsed.localize,
        highlight: parsed.highlight,
        smart_punctuation: parsed.smart,
        hard_breaks: parsed.hard_breaks,
    };
    let rendered = render(&source, &options);

    let output = if parsed.json {
        serde_json::to_string_pretty(&rendered)
            .map_err(|e| (1, format!("cannot serialize document: {e}")))?
    } else {
        rendered.html
    };

    match parsed.out {
        Some(path) => {
            std::fs::write(&path, output.as_bytes())
                .map_err(|e| (1, format!("cannot write {}: {e}", path.display())))?;
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(output.as_bytes())
                .map_err(|e| (1, format!("cannot write output: {e}")))?;
            if !output.ends_with('\n') {
                let _ = stdout.write_all(b"\n");
            }
        }
    }
    Ok(())
}

fn themes_command(args: &[String]) -> Result<(), (u8, String)> {
    let Some(action) = args.first().map(String::as_str) else {
        return Err((2, format!("missing themes action\n\n{USAGE}")));
    };
    match action {
        "list" => {
            let dirs = theme_dirs(&args[1..])?;
            let set = ThemeSet::builtin().with_user_dirs(&dirs);
            println!("{:<16} {:<7} {:<20} COUNTERPART", "ID", "MODE", "NAME");
            for theme in set.all() {
                println!(
                    "{:<16} {:<7} {:<20} {}",
                    theme.id,
                    theme.appearance.as_str(),
                    theme.name,
                    theme.counterpart.as_deref().unwrap_or("—")
                );
            }
            if !set.errors().is_empty() {
                eprintln!("\nSkipped themes:");
                for error in set.errors() {
                    eprintln!("  {error}");
                }
            }
            Ok(())
        }
        "scaffold" => {
            let id = args.get(1).ok_or_else(|| {
                (
                    2,
                    "themes scaffold needs an id, e.g. paper-blue".to_string(),
                )
            })?;
            let text = scaffold_theme(id).map_err(|e| (2, e))?;
            println!("{text}");
            Ok(())
        }
        "json" => {
            let dirs = theme_dirs(&args[1..])?;
            let set = ThemeSet::builtin().with_user_dirs(&dirs);
            let catalog = set.catalog();
            let text = serde_json::to_string_pretty(&catalog)
                .map_err(|e| (1, format!("cannot serialize themes: {e}")))?;
            println!("{text}");
            if !set.errors().is_empty() {
                for error in set.errors() {
                    eprintln!("theme skipped: {error}");
                }
            }
            Ok(())
        }
        "default" => {
            println!("{DEFAULT_LIGHT}");
            Ok(())
        }
        "default-dark" => {
            println!("{DEFAULT_DARK}");
            Ok(())
        }
        other => Err((2, format!("unknown themes action: {other}\n\n{USAGE}"))),
    }
}

fn theme_dirs(args: &[String]) -> Result<Vec<PathBuf>, (u8, String)> {
    let mut dirs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dir" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| (2, "--dir needs a directory".to_string()))?;
                dirs.push(PathBuf::from(value));
            }
            other => return Err((2, format!("unknown option: {other}"))),
        }
        index += 1;
    }
    Ok(dirs)
}
