//! `toknt` — a fast, lightweight CLI that prints an accurate, model-specific
//! token count for files, globs, directories, inline `-t`, or piped stdin.
//!
//! This binary is a thin product surface over the `toknt` library: it parses
//! flags, routes input, formats output, and provides the cache/registry
//! subcommands. All counting, model resolution, and accuracy labeling live in
//! the library and are reused verbatim — the CLI never reshapes the core.

use std::process::ExitCode;

use clap::Parser;

mod cli;

fn main() -> ExitCode {
    // clap exits with code 2 on a parse/usage error before we get here.
    let cli = cli::Cli::parse();
    match cli::dispatch(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            // A fatal CLI-boundary error (no model, no input, missing file, a
            // failed subcommand). The message is already specific & actionable.
            eprintln!("toknt: {err:#}");
            ExitCode::from(1)
        }
    }
}
