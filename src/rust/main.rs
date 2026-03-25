//! mullande - A large model Agent system in Rust

mod cli;
mod config;
mod workspace;
mod agent;
mod performance;
mod logging;

fn main() -> anyhow::Result<()> {
    cli::main()
}
