//! Small building blocks of the install flow, each tested on its own: private
//! verified copies, the root-side `.deb` step, bundle path rules and crash recovery.

#[cfg(any(target_os = "macos", test))]
pub(super) mod bundle_rules;
#[cfg(any(target_os = "linux", test))]
pub(super) mod deb;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux", test))]
pub(super) mod private_copy;
pub mod recovery;

/// Lowercase hexadecimal spelling of a digest.
#[cfg(any(target_os = "linux", test))]
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
pub(super) mod test_dir {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    /// A fresh temporary directory removed when dropped (permissions restored first).
    pub struct TempDir(pub PathBuf);
    impl TempDir {
        pub fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "vaio-steps-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
            }
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
