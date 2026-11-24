//! Tests for check and install retries, and for the download and verify error paths of the
//! updater state machine. Driven by the same fake backend and verifier as `service_tests`.

use crate::update::{UpdateError, UpdateState, VerifyError};

use super::service_fakes::{manifest, staged, Fake, ASSET_SIZE};
use super::service_tests::{available_service, service, PLATFORM};

fn install_failed_state() -> UpdateState {
    UpdateState::InstallFailed {
        staged: staged(ASSET_SIZE),
        error: UpdateError::Install("disk full".to_string()),
    }
}

#[test]
fn check_does_nothing_from_ready() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    service.download();
    let ready = UpdateState::Ready(staged(ASSET_SIZE));
    assert_eq!(service.state(), &ready);
    let fetches = fake.fetches.get();
    service.check();
    assert_eq!(service.state(), &ready);
    assert_eq!(fake.fetches.get(), fetches);
}

#[test]
fn check_does_nothing_from_installed() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    service.download();
    service.install();
    assert_eq!(service.state(), &UpdateState::Installed);
    let fetches = fake.fetches.get();
    service.check();
    assert_eq!(service.state(), &UpdateState::Installed);
    assert_eq!(fake.fetches.get(), fetches);
}

#[test]
fn check_does_nothing_from_install_failed() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    *fake.install.borrow_mut() = Err(UpdateError::Install("disk full".to_string()));
    let mut service = available_service(&fake);
    service.download();
    service.install();
    assert_eq!(service.state(), &install_failed_state());
    let fetches = fake.fetches.get();
    service.check();
    assert_eq!(service.state(), &install_failed_state());
    assert_eq!(fake.fetches.get(), fetches);
}

#[test]
fn install_failure_keeps_the_staged_file_for_a_retry() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    *fake.install.borrow_mut() = Err(UpdateError::Install("disk full".to_string()));
    let mut service = available_service(&fake);
    service.download();
    service.install();
    assert_eq!(service.state(), &install_failed_state());
    *fake.install.borrow_mut() = Ok(());
    service.install();
    assert_eq!(service.state(), &UpdateState::Installed);
    assert_eq!(fake.installs.get(), 2);
    assert_eq!(fake.downloads.get(), 1);
}

#[test]
fn backend_download_error_is_reported() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    *fake.staged.borrow_mut() = Err(UpdateError::Network("x".to_string()));
    let mut service = available_service(&fake);
    service.download();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Network("x".to_string()))
    );
    assert_eq!(fake.verifies.get(), 0);
}

#[test]
fn download_from_ready_does_nothing() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    service.download();
    let ready = UpdateState::Ready(staged(ASSET_SIZE));
    service.download();
    assert_eq!(service.state(), &ready);
    assert_eq!(fake.downloads.get(), 1);
}

#[test]
fn download_from_failed_does_nothing() {
    let fake = Fake::new(String::new());
    *fake.manifest.borrow_mut() = Err(UpdateError::Network("offline".to_string()));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    let failed = UpdateState::Failed(UpdateError::Network("offline".to_string()));
    service.download();
    assert_eq!(service.state(), &failed);
    assert_eq!(fake.downloads.get(), 0);
}

#[test]
fn install_from_downloading_does_nothing() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    let mut service = available_service(&fake);
    let downloading = UpdateState::Downloading {
        done: 1,
        total: Some(ASSET_SIZE),
    };
    service.state = downloading.clone();
    service.install();
    assert_eq!(service.state(), &downloading);
    assert_eq!(fake.installs.get(), 0);
}

#[test]
fn install_from_failed_does_nothing() {
    let fake = Fake::new(String::new());
    *fake.manifest.borrow_mut() = Err(UpdateError::Network("offline".to_string()));
    let mut service = service(&fake, "0.1.0", true);
    service.check();
    let failed = UpdateState::Failed(UpdateError::Network("offline".to_string()));
    service.install();
    assert_eq!(service.state(), &failed);
    assert_eq!(fake.installs.get(), 0);
}

#[test]
fn verifier_malformed_error_is_reported() {
    let fake = Fake::new(manifest(1, "0.1.1", Some(PLATFORM)));
    fake.verify.set(Err(VerifyError::Malformed));
    let mut service = available_service(&fake);
    service.download();
    assert_eq!(
        service.state(),
        &UpdateState::Failed(UpdateError::Verify(VerifyError::Malformed))
    );
}
