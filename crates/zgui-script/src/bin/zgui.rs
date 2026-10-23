//! `zgui`: developer tools for zgui-script.
//!
//! `zgui check [--nonstrict] <file-or-dir>...` type checks Luau scripts
//! with luau-lsp and the bundled `ui`/`anime` definitions, printing
//! `file:line:col` findings; it exits 1 when there are errors.
use std::{path::PathBuf, process::ExitCode};
use zgui_script::check::{CheckError, Options, check};

const USAGE: &str = "usage: zgui check [--nonstrict] <file-or-dir>...

Type checks zgui Luau scripts with luau-lsp, the bundled ui/anime types,
the standard platform and the new type solver. Strict mode applies unless
a .luaurc (languageMode) or a --!nonstrict comment says otherwise.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => {}
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    let mut options = Options::default();
    let mut paths = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--nonstrict" | "--non-strict" => options.strict = false,
            "--strict" => options.strict = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with('-') => {
                eprintln!("zgui check: unknown option {flag}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            path => paths.push(PathBuf::from(path)),
        }
    }
    if paths.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match check(&paths, &options) {
        Ok(report) => {
            for d in &report.diagnostics {
                println!("{d}");
            }
            let (errors, warnings) = (report.errors(), report.warnings());
            let plural =
                |n: usize, what: &str| format!("{n} {what}{}", if n == 1 { "" } else { "s" });
            eprintln!(
                "{}, {} in {}",
                plural(errors, "error"),
                plural(warnings, "warning"),
                plural(report.files, "file")
            );
            if errors > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error @ CheckError::NotInstalled) => {
            eprintln!("zgui check: {error}");
            ExitCode::from(3)
        }
        Err(error) => {
            eprintln!("zgui check: {error}");
            ExitCode::from(2)
        }
    }
}
