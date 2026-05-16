use std::process::ExitCode;

mod cli;
mod commands;
mod config;
mod core;
mod error;
mod fsops;
mod sources;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
