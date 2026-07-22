#![forbid(unsafe_code)]
//! Repository maintenance tasks for sim-stream-host.

mod tooling;

fn main() {
    if let Err(err) = tooling::run(std::env::args().collect()) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
