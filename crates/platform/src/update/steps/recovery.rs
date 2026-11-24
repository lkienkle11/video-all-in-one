//! Crash recovery for the macOS bundle swap, and the plain-text error report the
//! update helper leaves for the app when it cannot finish.
use crate::update::install_error;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use vaio_core::update::UpdateError;

/// Written before the first rename of a swap; removed when the swap is settled.
pub const MARKER_FILE: &str = "update-recovery.txt";
/// Left next to the settings file when an update fails; the UI will show it later.
pub const ERROR_FILE: &str = "update-error.txt";

/// A swap that may have been interrupted: the app bundle and where the old one went.
#[derive(Debug, PartialEq, Eq)]
pub struct Marker {
    pub target: PathBuf,
    pub backup: PathBuf,
}

/// What to do about a leftover marker.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RecoveryAction {
    /// The app is in place: the swap finished or never started.
    Nothing,
    /// The app is missing but the old bundle is there: move it back.
    Restore,
    /// Neither exists: nothing can be restored, tell the user.
    Report,
}

pub fn decide(target_exists: bool, backup_exists: bool) -> RecoveryAction {
    match (target_exists, backup_exists) {
        (true, _) => RecoveryAction::Nothing,
        (false, true) => RecoveryAction::Restore,
        (false, false) => RecoveryAction::Report,
    }
}

fn write_synced(path: &Path, text: &str) -> Result<(), UpdateError> {
    let mut file = fs::File::create(path).map_err(install_error)?;
    file.write_all(text.as_bytes()).map_err(install_error)?;
    file.sync_all().map_err(install_error)
}

#[cfg(any(target_os = "macos", test))]
pub fn write_marker(data_dir: &Path, marker: &Marker) -> Result<(), UpdateError> {
    let (target, backup) = (
        marker.target.to_string_lossy(),
        marker.backup.to_string_lossy(),
    );
    if target.contains('\n') || backup.contains('\n') {
        return Err(install_error("update path contains a line break"));
    }
    fs::create_dir_all(data_dir).map_err(install_error)?;
    write_synced(
        &data_dir.join(MARKER_FILE),
        &format!("{target}\n{backup}\n"),
    )
}

pub fn read_marker(data_dir: &Path) -> Option<Marker> {
    let text = fs::read_to_string(data_dir.join(MARKER_FILE)).ok()?;
    let mut lines = text.lines();
    let (target, backup) = (PathBuf::from(lines.next()?), PathBuf::from(lines.next()?));
    if lines.next().is_some() || !target.is_absolute() || !backup.is_absolute() {
        return None;
    }
    Some(Marker { target, backup })
}

pub fn clear_marker(data_dir: &Path) {
    let _ = fs::remove_file(data_dir.join(MARKER_FILE));
}

/// Writes `update-error.txt` in `data_dir`: what failed and where the old bundle is.
pub fn write_error_file(data_dir: &Path, what: &str, old_bundle: &Path) -> Result<(), UpdateError> {
    fs::create_dir_all(data_dir).map_err(install_error)?;
    let text = format!(
        "The update could not be completed.\n\nWhat failed: {what}\nThe previous app version is at: {}\n",
        old_bundle.display()
    );
    write_synced(&data_dir.join(ERROR_FILE), &text)
}

#[cfg(any(target_os = "macos", test))]
pub fn clear_error_file(data_dir: &Path) {
    let _ = fs::remove_file(data_dir.join(ERROR_FILE));
}

/// Settles an interrupted swap. Call it before starting an update helper and at app start.
pub fn recover(data_dir: &Path) -> Option<RecoveryAction> {
    let marker = match read_marker(data_dir) {
        Some(marker) => marker,
        None => {
            clear_marker(data_dir); // a malformed marker must not linger
            return None;
        }
    };
    let exists = |path: &Path| path.symlink_metadata().is_ok();
    let action = decide(exists(&marker.target), exists(&marker.backup));
    match action {
        RecoveryAction::Nothing => {}
        RecoveryAction::Restore => {
            if let Err(error) = fs::rename(&marker.backup, &marker.target) {
                // Keep the marker so the next start tries again.
                let _ = write_error_file(
                    data_dir,
                    &format!("could not restore the previous app: {error}"),
                    &marker.backup,
                );
                return Some(action);
            }
        }
        RecoveryAction::Report => {
            let _ = write_error_file(
                data_dir,
                "the app and its previous version are both missing",
                &marker.backup,
            );
        }
    }
    clear_marker(data_dir);
    Some(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::steps::test_dir::TempDir;

    #[test]
    fn recovery_decision_covers_every_combination() {
        assert_eq!(decide(true, true), RecoveryAction::Nothing);
        assert_eq!(decide(true, false), RecoveryAction::Nothing);
        assert_eq!(decide(false, true), RecoveryAction::Restore);
        assert_eq!(decide(false, false), RecoveryAction::Report);
    }

    #[test]
    fn marker_round_trips_and_is_cleared() {
        let dir = TempDir::new();
        let marker = Marker {
            target: "/Apps/Video.app".into(),
            backup: "/Apps/.vaio-update-1/previous.app".into(),
        };
        assert_eq!(read_marker(&dir.0), None);
        write_marker(&dir.0, &marker).unwrap();
        assert_eq!(read_marker(&dir.0), Some(marker));
        clear_marker(&dir.0);
        assert_eq!(read_marker(&dir.0), None);
    }

    #[test]
    fn marker_with_relative_or_missing_paths_is_ignored() {
        let dir = TempDir::new();
        for body in ["", "/a\n", "rel\n/b\n", "/a\nrel\n", "/a\n/b\n/c\n"] {
            fs::write(dir.0.join(MARKER_FILE), body).unwrap();
            assert_eq!(read_marker(&dir.0), None, "{body:?}");
        }
        let newline = Marker {
            target: "/a\nb".into(),
            backup: "/c".into(),
        };
        assert!(write_marker(&dir.0, &newline).is_err());
    }

    #[test]
    fn error_file_names_the_failure_and_the_old_bundle() {
        let dir = TempDir::new();
        write_error_file(
            &dir.0,
            "restore failed",
            Path::new("/Apps/old/previous.app"),
        )
        .unwrap();
        let text = fs::read_to_string(dir.0.join(ERROR_FILE)).unwrap();
        assert!(text.contains("restore failed"));
        assert!(text.contains("/Apps/old/previous.app"));
        clear_error_file(&dir.0);
        assert!(!dir.0.join(ERROR_FILE).exists());
    }

    #[test]
    fn error_file_is_created_in_a_missing_data_directory() {
        let dir = TempDir::new();
        let nested = dir.0.join("data");
        write_error_file(&nested, "failed", Path::new("/old")).unwrap();
        assert!(nested.join(ERROR_FILE).is_file());
    }

    fn bundle(path: &Path, body: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join("binary"), body).unwrap();
    }

    #[test]
    fn missing_target_is_restored_from_the_backup() {
        let dir = TempDir::new();
        let (target, backup) = (dir.0.join("Video.app"), dir.0.join("previous.app"));
        bundle(&backup, "old");
        write_marker(
            &dir.0,
            &Marker {
                target: target.clone(),
                backup: backup.clone(),
            },
        )
        .unwrap();
        assert_eq!(recover(&dir.0), Some(RecoveryAction::Restore));
        assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "old");
        assert_eq!(read_marker(&dir.0), None);
        assert!(!dir.0.join(ERROR_FILE).exists());
    }

    #[test]
    fn present_target_is_left_alone_and_marker_cleared() {
        let dir = TempDir::new();
        let (target, backup) = (dir.0.join("Video.app"), dir.0.join("previous.app"));
        bundle(&target, "new");
        bundle(&backup, "old");
        write_marker(
            &dir.0,
            &Marker {
                target: target.clone(),
                backup,
            },
        )
        .unwrap();
        assert_eq!(recover(&dir.0), Some(RecoveryAction::Nothing));
        assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "new");
        assert_eq!(read_marker(&dir.0), None);
    }

    #[test]
    fn nothing_to_restore_is_reported_in_the_error_file() {
        let dir = TempDir::new();
        write_marker(
            &dir.0,
            &Marker {
                target: dir.0.join("Video.app"),
                backup: dir.0.join("previous.app"),
            },
        )
        .unwrap();
        assert_eq!(recover(&dir.0), Some(RecoveryAction::Report));
        assert!(dir.0.join(ERROR_FILE).is_file());
        assert_eq!(read_marker(&dir.0), None);
    }

    #[test]
    fn no_marker_means_no_recovery() {
        let dir = TempDir::new();
        assert_eq!(recover(&dir.0), None);
        assert!(!dir.0.join(ERROR_FILE).exists());
    }
}
