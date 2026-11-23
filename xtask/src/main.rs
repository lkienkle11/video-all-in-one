mod check_release;
mod check_structure;
mod lang_share;
mod set_version;
mod structure_map;

use anyhow::{bail, Context, Result};
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
    CheckStructure,
    StructureMap {
        #[arg(long, required = true)]
        write: bool,
    },
    SetVersion {
        version: String,
    },
    CheckRelease {
        #[arg(long)]
        tag: String,
        #[arg(long)]
        latest: String,
    },
}

fn main() -> Result<()> {
    let root = std::env::current_dir()?;
    match Cli::parse().command {
        Command::LangShare { min } => lang_share::run(&root, min),
        Command::SetVersion { version } => set_version::set_version(&root, &version),
        Command::CheckRelease { tag, latest } => check_release::check_release(&root, &tag, &latest),
        Command::CheckStructure => {
            let output = std::process::Command::new("cargo")
                .args(["metadata", "--format-version", "1", "--no-deps"])
                .current_dir(&root)
                .output()
                .context("running cargo metadata")?;
            if !output.status.success() {
                bail!(
                    "cargo metadata failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let metadata = String::from_utf8(output.stdout)?;
            let map = structure_map::parse_map(&std::fs::read_to_string(
                root.join("docs/architecture.md"),
            )?)?;
            let violations = check_structure::check(&root, &metadata, &map);
            for violation in &violations {
                println!(
                    "{:?} {} {}",
                    violation.rule,
                    violation.path.display(),
                    violation.detail
                );
            }
            if !violations.is_empty() {
                std::process::exit(1);
            }
            Ok(())
        }
        Command::StructureMap { write: _ } => {
            let path = root.join("docs/architecture.md");
            let markdown = std::fs::read_to_string(&path)?;
            let updated = structure_map::render(&markdown, &check_structure::directories(&root))?;
            std::fs::write(path, updated)?;
            Ok(())
        }
    }
}
