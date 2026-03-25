//! mullande - A large model Agent system in Rust

mod cli;
mod config;
mod workspace;
mod memory;
mod agent;
mod performance;
mod logging;
mod tools;

fn main() -> anyhow::Result<()> {
    cli::main()
}
