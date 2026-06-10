mod app;
mod border;
mod drawing;
mod events;
mod ipc;
mod logging;
mod parser;
mod settings;
mod sys;
mod windows;

use std::process::ExitCode;

fn main() -> ExitCode {
    match app::run(std::env::args().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
