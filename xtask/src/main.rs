mod check_structure;
mod icons;
mod lang_share;
mod libmpv;
mod package;
mod release;
mod structure_map;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use release::{check_release, keygen, latest_json, set_version, sign};
use std::path::PathBuf;

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
    /// Renders the app icons from the SVG sources into `assets/icons/generated`.
    Icons {
        /// Fail instead of writing when a committed icon is out of date.
        #[arg(long)]
        check: bool,
    },
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
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    Sign {
        #[arg(long)]
        file: PathBuf,
    },
    /// Builds and packs our own LGPL libmpv (see docs/RELEASING.md, "Our own libmpv").
    Libmpv {
        #[command(subcommand)]
        command: libmpv::Command,
    },
    /// Builds the installer and the update artifact of one OS from a built production binary.
    Package {
        #[arg(long, value_enum)]
        target: package::Target,
        /// Defaults to the workspace version.
        #[arg(long)]
        version: Option<String>,
        /// Output folder, default `target/package`.
        #[arg(long)]
        out: Option<PathBuf>,
        /// The production executable, default `target/release/vaio-desktop`.
        #[arg(long)]
        binary: Option<PathBuf>,
        /// Windows only: the folder with libmpv-2.dll and the DLLs it needs.
        #[arg(long)]
        libs: Option<PathBuf>,
        /// macOS and Ubuntu: the folder of our own libmpv build (`cargo xtask libmpv build`).
        #[arg(long)]
        libmpv_dir: Option<PathBuf>,
        /// The CPU of the executable (`aarch64` or `x86_64`) when it differs from this machine's,
        /// for a binary cross-compiled on another Mac; it decides the platform key of the package.
        #[arg(long)]
        arch: Option<String>,
        /// Local testing only: lets GPL or non-free libraries into the package, with a warning.
        #[arg(long = "allow-gpl-for-local-testing")]
        allow_gpl: bool,
    },
    LatestJson {
        #[arg(long)]
        version: String,
        #[arg(long)]
        notes: PathBuf,
        #[arg(long)]
        assets: PathBuf,
        #[arg(long)]
        base_url: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Base64 of the 32-byte Ed25519 public key that every artifact signature must match.
        #[arg(long)]
        public_key: String,
        /// Platform keys that must have an artifact, comma separated.
        #[arg(long, value_delimiter = ',')]
        require: Vec<String>,
    },
}

fn main() -> Result<()> {
    let root = std::env::current_dir()?;
    match Cli::parse().command {
        Command::Icons { check } => icons::run(&root, check),
        Command::LangShare { min } => lang_share::run(&root, min),
        Command::SetVersion { version } => set_version::set_version(&root, &version),
        Command::CheckRelease { tag, latest } => check_release::check_release(&root, &tag, &latest),
        Command::Package {
            target,
            version,
            out,
            binary,
            libs,
            libmpv_dir,
            arch,
            allow_gpl,
        } => package::run(
            &root,
            &package::Options {
                target,
                version,
                out,
                binary,
                libs,
                libmpv_dir,
                arch,
                allow_gpl,
            },
        ),
        Command::Libmpv { command } => libmpv::run(command),
        Command::Keygen { out } => keygen::run(&out),
        Command::Sign { file } => sign::run(&file),
        Command::LatestJson {
            version,
            notes,
            assets,
            base_url,
            out,
            public_key,
            require,
        } => {
            let key = latest_json::decode_public_key(&public_key)?;
            latest_json::run(
                &version,
                &notes,
                &assets,
                &base_url,
                out.as_deref(),
                &key,
                &require,
            )
        }
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
