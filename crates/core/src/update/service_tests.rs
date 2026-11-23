//! Tests for the updater state machine, driven by a fake backend and a fake verifier.

use std::rc::Rc;

use semver::Version;

use crate::update::{
    Asset, AvailableUpdate, ManifestError, UpdateError, UpdateService, UpdateState, VerifyError,
};

use super::service_fakes::{manifest, signature, staged, Backend, Fake, Verifier, ASSET_SIZE};

const KEY: [u8; 32] = [3; 32];
const PLATFORM: &str = "macos-aarch64";

fn service(fake: &Rc<Fake>, current: &str, enabled: bool) -> UpdateService<Backend, Verifier> {
    UpdateService::new(
        Backend(fake.clone()),
        Verifier(fake.clone()),
        Version::parse(current).unwrap(),
        PLATFORM.to_string(),
        KEY,
        enabled,
    )
}

fn available(version: &str) -> UpdateState {
    UpdateState::Available(AvailableUpdate {
        version: Version::parse(version).unwrap(),
        size: ASSET_SIZE,
        notes_en: "Fixes".to_string(),
        notes_vi: "Sửa lỗi".to_string(),
        asset: Asset {
            url: "https://example.invalid/app.zip".to_string(),
            size: ASSET_SIZE,
            sha256: "a".repeat(64),
            signature: signature(),
        },
    })
}

/// A service already in `Available`, ready to download.
fn available_service(fake: &Rc<Fake>) -> UpdateService<Backend, Verifier> {
    let mut service = service(fake, "0.1.0", true);
    service.check();
    assert_eq!(service.state(), &available("0.1.1"));
    service
}

#[test]
fn disabled_service_never_calls_backend() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = service(&fake, "0.1.0", false);
    service.check();
    service.download();
    service.install();
    assert_eq!(service.state(), &UpdateState::Idle);
    assert_eq!(
        (
            fake.fetches.get(),
            fake.downloads.get(),
            fake.installs.get(),
            fake.verifies.get()
        ),
        (0, 0, 0, 0)
    );
}

#[test]
fn check_failure_leaves_state_failed_and_app_usable() {
    let fake = Fake::new(String::new());
    *fake.manifest.borrow_mut() = Err(UpdateError::Network("offline".to_string()));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Network("offline".to_string()))
    );
    service.download();
    assert_eq!(fake.downloads.get(), 0);
}

#[test]
fn download_with_bad_signature_never_reaches_ready() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    fake.verify.set(Err(VerifyError::BadSignature));
    service.download();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Verify(VerifyError::BadSignature))
    );
}

#[test]
fn download_with_wrong_digest_never_reaches_ready() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    fake.verify.set(Err(VerifyError::DigestMismatch));
    service.download();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Verify(VerifyError::DigestMismatch))
    );
}

#[test]
fn install_only_allowed_from_ready() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut idle = service(&fake, "0.1.0", true);
    idle.install();
    let mut offered = available_service(&fake);
    offered.install();
    assert_eq!(idle.state(), &UpdateState::Idle);
    assert_eq!(offered.state(), &available("0.1.1"));
    assert_eq!(fake.installs.get(), 0);
}

#[test]
fn progress_is_reported_with_unknown_total() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    *fake.progress.borrow_mut() = vec![(0, None), (512, None)];
    let mut service = available_service(&fake);
    let mut seen = Vec::new();
    service.download_with_progress(&mut |done, total| seen.push((done, total)));
    assert_eq!(seen, vec![(0, None), (512, None)]);
}

#[test]
fn check_with_a_newer_manifest_makes_the_update_available() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    assert_eq!(service.state(), &available("0.1.1"));
}

#[test]
fn check_with_the_same_version_is_up_to_date() {
    let fake = Fake::new(manifest(1, "0.1.0", Some(PLATFORM)));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    assert_eq!(service.state(), &UpdateState::UpToDate);
}

#[test]
fn check_with_a_prerelease_manifest_is_up_to_date() {
    let fake = Fake::new(manifest(1, "0.2.0-beta.1", Some(PLATFORM)));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    assert_eq!(service.state(), &UpdateState::UpToDate);
}

#[test]
fn check_with_unknown_schema_fails_with_a_manifest_error() {
    let fake = Fake::new(manifest(2, "0.1.1", Some(PLATFORM)));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Manifest(ManifestError::UnknownSchema(2)))
    );
}

#[test]
fn check_without_asset_for_platform_fails() {
    let fake = Fake::new(manifest(1, "0.1.1", None));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::NoAssetForPlatform)
    );
}

#[test]
fn download_from_idle_does_nothing() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = service(&fake, "0.1.0", true);
    service.download();
    assert_eq!(service.state(), &UpdateState::Idle);
    assert_eq!(fake.downloads.get(), 0);
}

#[test]
fn download_with_size_mismatch_fails() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    *fake.staged.borrow_mut() = Ok(staged(ASSET_SIZE - 1));
    let mut service = available_service(&fake);
    service.download();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::SizeMismatch {
            expected: ASSET_SIZE,
            actual: ASSET_SIZE - 1,
        })
    );
}

#[test]
fn successful_download_reaches_ready_then_install_reaches_installed() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    service.download();
    assert_eq!(service.state(), &UpdateState::Ready(staged(ASSET_SIZE)));
    service.install();
    assert_eq!(service.state(), &UpdateState::Installed);
    assert_eq!(fake.installs.get(), 1);
}

#[test]
fn install_failure_is_reported() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    *fake.install.borrow_mut() = Err(UpdateError::Install("disk full".to_string()));
    let mut service = available_service(&fake);
    service.download();
    service.install();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Install("disk full".to_string()))
    );
}

#[test]
fn retry_after_failure_can_check_again() {
    let fake = Fake::new(String::new());
    *fake.manifest.borrow_mut() = Err(UpdateError::Network("offline".to_string()));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    *fake.manifest.borrow_mut() = Ok(manifest(1, "0.1.1", Some(PLATFORM)));
    service.check();
    assert_eq!(service.state(), &available("0.1.1"));
}

#[test]
fn check_does_nothing_while_downloading() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    service.state = UpdateState::Downloading {
        done: 1,
        total: Some(ASSET_SIZE),
    };
    service.check();
    assert_eq!(
        service.state(),
        &UpdateState::Downloading {
            done: 1,
            total: Some(ASSET_SIZE)
        }
    );
    assert_eq!(fake.fetches.get(), 1);
}
