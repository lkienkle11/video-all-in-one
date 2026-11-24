//! Atomic JSON persistence for the user's settings.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use vaio_core::i18n::Language;
use vaio_core::settings::Settings;

#[derive(Clone)]
pub struct SettingsStore {
    path: Option<PathBuf>,
}

impl SettingsStore {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            path: Some(dir.as_ref().join("settings.json")),
        }
    }
    /// Disables disk persistence; the view retains the chosen settings in memory.
    pub fn in_memory() -> Self {
        Self { path: None }
    }
    /// Reads from this store's data directory.
    pub fn read(&self) -> Settings {
        self.path.as_ref().map(Self::read_path).unwrap_or_default()
    }
    /// Missing, unreadable or invalid settings always mean first-launch defaults.
    pub fn load(dir: impl AsRef<Path>) -> Settings {
        Self::read_path(dir.as_ref().join("settings.json"))
    }
    fn read_path(path: impl AsRef<Path>) -> Settings {
        let language = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("language")
                    .and_then(|value| value.as_str())
                    .and_then(Language::from_code)
            });
        Settings { language }
    }

    /// Writes a complete temporary file before replacing the live settings.
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(
            &serde_json::json!({"language": settings.language.map(|language| language.code())}),
        )?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temp = path.with_extension(format!(
            "json.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()
        })();
        drop(file);
        let result = result.and_then(|_| fs::rename(&temp, path));
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vaio_core::i18n::Language;

    struct Directory(PathBuf);
    impl std::ops::Deref for Directory {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }
    impl AsRef<Path> for Directory {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
            let _ = fs::remove_file(&self.0);
        }
    }
    fn directory() -> Directory {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vaio-store-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Directory(dir)
    }

    #[test]
    fn missing_empty_corrupt_unknown_and_wrong_type_use_defaults() {
        let dir = directory();
        assert_eq!(SettingsStore::load(&dir), Settings::default());
        for text in [
            "",
            "{broken",
            "{}",
            "[]",
            "{\"language\":\"fr\"}",
            "{\"language\":7}",
            "{\"language\":null}",
        ] {
            std::fs::write(dir.join("settings.json"), text).unwrap();
            assert_eq!(SettingsStore::load(&dir), Settings::default(), "{text}");
        }
    }

    #[test]
    fn save_creates_directory_and_replaces_settings_atomically() {
        let root = directory();
        let dir = root.join("data");
        let store = SettingsStore::new(&dir);
        for language in [Some(Language::English), Some(Language::Vietnamese), None] {
            let settings = Settings { language };
            store.save(&settings).unwrap();
            assert_eq!(SettingsStore::load(&dir), settings);
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        }
    }

    #[test]
    fn failed_save_leaves_the_existing_file_unchanged() {
        let root = directory();
        let dir = root.join("data");
        let store = SettingsStore::new(&dir);
        store
            .save(&Settings {
                language: Some(Language::English),
            })
            .unwrap();
        let original = std::fs::read(dir.join("settings.json")).unwrap();
        // A file in place of the data directory makes every temporary path unwritable.
        let backup = root.join("original");
        std::fs::rename(&dir, &backup).unwrap();
        std::fs::write(&dir, "blocked").unwrap();
        assert!(store
            .save(&Settings {
                language: Some(Language::Vietnamese)
            })
            .is_err());
        assert_eq!(std::fs::read(&dir).unwrap(), b"blocked");
        assert_eq!(
            std::fs::read(backup.join("settings.json")).unwrap(),
            original
        );
    }

    #[test]
    fn failed_save_removes_temporary_file() {
        let dir = directory();
        std::fs::create_dir(dir.join("settings.json")).unwrap();
        assert!(SettingsStore::new(&dir).save(&Settings::default()).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_saves_use_unique_temp_files() {
        let dir = directory();
        std::fs::create_dir(dir.join("settings.json.tmp")).unwrap();
        let store = SettingsStore::new(&dir);
        store
            .save(&Settings {
                language: Some(Language::Vietnamese),
            })
            .unwrap();
        assert_eq!(store.read().language, Some(Language::Vietnamese));
    }
}
