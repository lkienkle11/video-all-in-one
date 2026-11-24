use super::{
    apply::{parse, settle, swap_bundle, wait_for_exit, DebRequest, HelperRequest},
    install_linux, install_macos, install_windows,
    steps::recovery::{read_marker, ERROR_FILE},
    verify_staged,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use vaio_core::update::{StagedFile, UpdateError};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "vaio-install-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn bundle(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("binary"), body).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn apply_argument_parser_rejects_malformed_requests() {
    let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
    assert!(parse(&args(&[])).unwrap().is_none());
    let Some(HelperRequest::Update(parsed)) = parse(&args(&[
        "--apply-update",
        "/tmp/new.app",
        "/Applications/Video.app",
        "123",
        "/data",
    ]))
    .unwrap() else {
        panic!("expected an update request")
    };
    assert_eq!(parsed.staged, Path::new("/tmp/new.app"));
    assert_eq!(parsed.target, Path::new("/Applications/Video.app"));
    assert_eq!(parsed.pid, 123);
    assert_eq!(parsed.data_dir, Path::new("/data"));
    for values in [
        vec!["--apply-update"],
        vec!["--apply-update", "/new", "/target", "123"],
        vec!["--apply-update", "relative", "/target", "123", "/data"],
        vec!["--apply-update", "/new", "/target", "0", "/data"],
        vec!["--apply-update", "/new", "/target", "bad", "/data"],
        vec!["--apply-update", "/same", "/same", "123", "/data"],
        vec!["--apply-update", "/new", "/target", "123", "data"],
        vec!["--apply-update", "/new", "/target", "123", "/data", "extra"],
    ] {
        assert!(parse(&args(&values)).is_err(), "{values:?}");
    }
}

#[test]
fn helper_mode_starts_only_from_argument_zero() {
    let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
    for values in [
        vec!["other", "--apply-update", "/new", "/target", "123", "/data"],
        vec!["/file", "--apply-deb", "/x", &"a".repeat(64)],
        vec!["--apply-updates"],
        vec!["x--apply-deb"],
    ] {
        assert!(parse(&args(&values)).unwrap().is_none(), "{values:?}");
    }
}

#[test]
fn deb_helper_arguments_are_validated() {
    let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
    let hash = "AB".repeat(32);
    let Some(HelperRequest::Deb(DebRequest { path, sha256_hex })) =
        parse(&args(&["--apply-deb", "/tmp/update.deb", &hash])).unwrap()
    else {
        panic!("expected a deb request")
    };
    assert_eq!(path, Path::new("/tmp/update.deb"));
    assert_eq!(sha256_hex, hash.to_lowercase());
    let short = "a".repeat(63);
    for values in [
        vec!["--apply-deb"],
        vec!["--apply-deb", "/tmp/update.deb"],
        vec!["--apply-deb", "relative.deb", &hash],
        vec!["--apply-deb", "/tmp/update.deb", &short],
        vec!["--apply-deb", "/tmp/update.deb", &hash, "extra"],
    ] {
        assert!(parse(&args(&values)).is_err(), "{values:?}");
    }
}

#[test]
fn parent_wait_has_a_deadline_and_propagates_probe_errors() {
    assert!(wait_for_exit(Duration::ZERO, || Ok(true)).is_err());
    assert!(wait_for_exit(Duration::ZERO, || Ok(false)).is_ok());
    assert!(
        wait_for_exit(Duration::from_secs(1), || Err(UpdateError::Install(
            "probe failed".into()
        )))
        .is_err()
    );
}

#[test]
fn successful_swap_preserves_old_bundle_as_backup() {
    let fixture = Fixture::new();
    let staged = fixture.bundle("new.app", "new");
    let target = fixture.bundle("target.app", "old");
    let backup = fixture.0.join("backup.app");
    swap_bundle(&staged, &target, &backup, &fixture.0, || {
        assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "new");
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read_to_string(backup.join("binary")).unwrap(), "old");
}

#[test]
fn failed_relaunch_restores_old_bundle() {
    let fixture = Fixture::new();
    let staged = fixture.bundle("new.app", "new");
    let target = fixture.bundle("target.app", "old");
    let backup = fixture.0.join("backup.app");
    assert!(swap_bundle(&staged, &target, &backup, &fixture.0, || Err(
        UpdateError::Install("launch failed".into())
    ))
    .is_err());
    assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "old");
    assert_eq!(fs::read_to_string(staged.join("binary")).unwrap(), "new");
    assert!(!backup.exists());
}

#[test]
fn failed_move_restores_old_bundle_and_existing_backup_is_never_overwritten() {
    let fixture = Fixture::new();
    let target = fixture.bundle("target.app", "old");
    let backup = fixture.0.join("backup.app");
    assert!(swap_bundle(
        &fixture.0.join("missing.app"),
        &target,
        &backup,
        &fixture.0,
        || panic!("must not launch")
    )
    .is_err());
    assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "old");
    let staged = fixture.bundle("new.app", "new");
    fs::create_dir(&backup).unwrap();
    fs::write(backup.join("keep"), "keep").unwrap();
    assert!(
        swap_bundle(&staged, &target, &backup, &fixture.0, || panic!(
            "must not launch"
        ))
        .is_err()
    );
    assert_eq!(fs::read_to_string(backup.join("keep")).unwrap(), "keep");
    assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "old");
}

#[test]
fn installer_commands_keep_paths_as_single_arguments() {
    let path = Path::new("/tmp/-installer with spaces;echo.exe");
    let windows = install_windows::installer_command(path);
    assert_eq!(windows.get_program(), path);
    assert_eq!(windows.get_args().collect::<Vec<_>>(), ["/S"]);
    let linux = install_linux::installer_command(Path::new("/opt/app/video"), path, &[0xab; 32]);
    assert_eq!(linux.get_program(), "pkexec");
    assert_eq!(
        linux.get_args().collect::<Vec<_>>(),
        [
            "/opt/app/video",
            "--apply-deb",
            "/tmp/-installer with spaces;echo.exe",
            &"ab".repeat(32)
        ]
    );
    let extract = install_macos::extract_command(path, Path::new("/tmp/update folder"));
    assert_eq!(extract.get_program(), "/usr/bin/ditto");
    assert_eq!(
        extract.get_args().collect::<Vec<_>>(),
        [
            "-x",
            "-k",
            "/tmp/-installer with spaces;echo.exe",
            "/tmp/update folder"
        ]
    );
    let helper = install_macos::helper_command(
        Path::new("/app/bin"),
        Path::new("/new app"),
        Path::new("/old app"),
        123,
        Path::new("/data dir"),
    );
    assert_eq!(helper.get_program(), "/app/bin");
    assert_eq!(
        helper.get_args().collect::<Vec<_>>(),
        ["--apply-update", "/new app", "/old app", "123", "/data dir"]
    );
}

#[test]
fn macos_bundle_is_derived_from_contents_macos_executable() {
    assert_eq!(
        install_macos::bundle_for_executable(Path::new(
            "/Applications/Video.app/Contents/MacOS/video"
        ))
        .unwrap(),
        Path::new("/Applications/Video.app")
    );
    assert!(install_macos::bundle_for_executable(Path::new("/tmp/video")).is_err());
    assert!(install_macos::bundle_for_executable(Path::new("/Video.app/Other/video")).is_err());
}

#[test]
fn modified_staged_artifact_is_refused_before_install() {
    let fixture = Fixture::new();
    let path = fixture.0.join("update.zip");
    fs::write(&path, b"abc").unwrap();
    let staged = StagedFile {
        path,
        size: 3,
        sha256: Sha256::digest(b"abc").into(),
    };
    verify_staged(&staged).unwrap();
    fs::write(&staged.path, b"bad").unwrap();
    assert!(verify_staged(&staged).is_err());
    fs::write(&staged.path, b"ab").unwrap();
    assert!(verify_staged(&staged).is_err());
}

#[cfg(unix)]
#[test]
fn swap_refuses_symlink_bundles_before_moving_anything() {
    let fixture = Fixture::new();
    let staged = fixture.bundle("new.app", "new");
    let target = fixture.bundle("target.app", "old");
    let alias = fixture.0.join("alias.app");
    std::os::unix::fs::symlink(&staged, &alias).unwrap();
    assert!(swap_bundle(
        &alias,
        &target,
        &fixture.0.join("backup.app"),
        &fixture.0,
        || Ok(())
    )
    .is_err());
    assert_eq!(fs::read_to_string(target.join("binary")).unwrap(), "old");
    assert!(alias.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn swap_writes_a_recovery_marker_before_moving_and_clears_it_on_success() {
    let fixture = Fixture::new();
    let staged = fixture.bundle("new.app", "new");
    let target = fixture.bundle("target.app", "old");
    let backup = fixture.0.join("backup.app");
    let data = fixture.0.join("data");
    swap_bundle(&staged, &target, &backup, &data, || {
        // Mid-swap the marker exists and names both locations.
        let marker = read_marker(&data).unwrap();
        assert_eq!(
            (marker.target, marker.backup),
            (target.clone(), backup.clone())
        );
        Ok(())
    })
    .unwrap();
    assert!(read_marker(&data).is_none());
}

#[test]
fn restored_swap_clears_the_marker_but_a_failed_restore_keeps_it() {
    let fixture = Fixture::new();
    let staged = fixture.bundle("new.app", "new");
    let target = fixture.bundle("target.app", "old");
    let backup = fixture.0.join("backup.app");
    let data = fixture.0.join("data");
    assert!(swap_bundle(&staged, &target, &backup, &data, || Err(
        UpdateError::Install("x".into())
    ))
    .is_err());
    assert!(read_marker(&data).is_none());
    // The new bundle vanishes mid-swap, so the move aside fails and the old app stays in the backup.
    let staged = fixture.bundle("new2.app", "new");
    let result = swap_bundle(&staged, &target, &backup, &data, || {
        fs::remove_dir_all(&target).unwrap();
        Err(UpdateError::Install("launch failed".into()))
    });
    assert!(result.is_err());
    assert!(read_marker(&data).is_some());
}

#[test]
fn failed_swap_writes_the_error_file_and_relaunches_the_restored_app() {
    let fixture = Fixture::new();
    let target = fixture.bundle("target.app", "old");
    let backup = fixture.0.join("root/previous.app");
    let data = fixture.0.join("data");
    let mut launched = 0;
    let result = settle(
        Err(UpdateError::Install("move failed".into())),
        &data,
        &target,
        &backup,
        || {
            launched += 1;
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(launched, 1);
    let text = fs::read_to_string(data.join(ERROR_FILE)).unwrap();
    assert!(text.contains("move failed") && text.contains("previous.app"));
}

#[test]
fn failed_swap_with_no_app_in_place_does_not_launch_and_names_the_old_bundle() {
    let fixture = Fixture::new();
    let backup = fixture.bundle("previous.app", "old");
    let data = fixture.0.join("data");
    let result = settle(
        Err(UpdateError::Install("restore failed".into())),
        &data,
        &fixture.0.join("missing.app"),
        &backup,
        || panic!("nothing to launch"),
    );
    assert!(result.is_err());
    assert!(fs::read_to_string(data.join(ERROR_FILE))
        .unwrap()
        .contains(&backup.display().to_string()));
}

#[test]
fn successful_swap_removes_error_file_backup_and_staging_folder() {
    let fixture = Fixture::new();
    let root = fixture.0.join(".vaio-update-1-0");
    fs::create_dir(&root).unwrap();
    let backup = root.join("previous.app");
    fs::create_dir(&backup).unwrap();
    let target = fixture.bundle("target.app", "new");
    let data = fixture.0.join("data");
    fs::create_dir(&data).unwrap();
    fs::write(data.join(ERROR_FILE), "old failure").unwrap();
    settle(Ok(()), &data, &target, &backup, || {
        panic!("already running")
    })
    .unwrap();
    assert!(!data.join(ERROR_FILE).exists());
    assert!(!root.exists());
    assert!(target.exists());
}

#[test]
fn success_cleanup_never_removes_a_folder_that_is_not_an_update_folder() {
    let fixture = Fixture::new();
    let root = fixture.0.join("precious");
    fs::create_dir(&root).unwrap();
    let backup = root.join("previous.app");
    fs::create_dir(&backup).unwrap();
    fs::write(root.join("keep"), "keep").unwrap();
    let data = fixture.0.join("data");
    settle(Ok(()), &data, &fixture.0, &backup, || Ok(())).unwrap();
    assert!(root.join("keep").exists());
}
