use super::{install_error, InstallOutcome};
use std::{path::Path, process::Command};
use vaio_core::update::UpdateError;

pub(super) fn reveal(path: &Path) -> Result<InstallOutcome, UpdateError> {
    let path = path.canonicalize().map_err(install_error)?;
    let status = reveal_command(std::env::consts::OS, &path)
        .status()
        .map_err(install_error)?;
    if !status.success() {
        return Err(install_error("file manager could not reveal the update"));
    }
    Ok(InstallOutcome::OpenedInstaller(path))
}

pub(super) fn reveal_command(os: &str, path: &Path) -> Command {
    match os {
        "macos" => {
            let mut command = Command::new("open");
            command.arg("-R").arg(path);
            command
        }
        "windows" => {
            let mut command = Command::new("explorer.exe");
            command.arg("/select,").arg(path);
            command
        }
        _ => {
            let mut command = Command::new("xdg-open");
            command.arg(path.parent().unwrap_or(Path::new("/")));
            command
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_reveals_file_without_executing_it() {
        let path = Path::new("/tmp/installer ; dangerous.exe");
        for (os, program, args) in [
            (
                "macos",
                "open",
                vec!["-R", "/tmp/installer ; dangerous.exe"],
            ),
            (
                "windows",
                "explorer.exe",
                vec!["/select,", "/tmp/installer ; dangerous.exe"],
            ),
            ("linux", "xdg-open", vec!["/tmp"]),
        ] {
            let command = reveal_command(os, path);
            assert_eq!(command.get_program(), program);
            assert_eq!(command.get_args().collect::<Vec<_>>(), args);
        }
    }
}
