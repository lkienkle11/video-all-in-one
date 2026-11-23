mod lang_share;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    LangShare {
        #[arg(long)]
        min: f64,
    },
}

fn run_lang_share(root: &std::path::Path, min: f64) -> Result<()> {
    let share = lang_share::rust_share(root)?;
    println!("Rust: {share:.1}%");
    lang_share::check_minimum(share, min)
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::LangShare { min } => run_lang_share(&std::env::current_dir()?, min),
    }
}
