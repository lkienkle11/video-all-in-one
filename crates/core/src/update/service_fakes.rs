//! Test doubles for the updater: a fake backend and a fake verifier with call counters.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use base64::Engine;

use crate::ports::{ArtifactVerifier, UpdateBackend};
use crate::update::{Asset, StagedFile, UpdateClaim, UpdateError, VerifyError};

pub(super) const ASSET_SIZE: u64 = 1024;

/// Everything the fakes return or record. Tests keep a handle to set and read it.
pub(super) struct Fake {
    pub(super) manifest: RefCell<Result<String, UpdateError>>,
    pub(super) staged: RefCell<Result<StagedFile, UpdateError>>,
    pub(super) progress: RefCell<Vec<(u64, Option<u64>)>>,
    pub(super) install: RefCell<Result<(), UpdateError>>,
    pub(super) verify: Cell<Result<(), VerifyError>>,
    pub(super) fetches: Cell<u32>,
    pub(super) downloads: Cell<u32>,
    pub(super) installs: Cell<u32>,
    pub(super) verifies: Cell<u32>,
    pub(super) claims: RefCell<Vec<UpdateClaim>>,
}

impl Fake {
    pub(super) fn new(manifest: String) -> Rc<Self> {
        Rc::new(Self {
            manifest: RefCell::new(Ok(manifest)),
            staged: RefCell::new(Ok(staged(ASSET_SIZE))),
            progress: RefCell::new(Vec::new()),
            install: RefCell::new(Ok(())),
            verify: Cell::new(Ok(())),
            fetches: Cell::new(0),
            downloads: Cell::new(0),
            installs: Cell::new(0),
            verifies: Cell::new(0),
            claims: RefCell::new(Vec::new()),
        })
    }
}

pub(super) struct Backend(pub(super) Rc<Fake>);
pub(super) struct Verifier(pub(super) Rc<Fake>);

impl UpdateBackend for Backend {
    fn fetch_manifest(&self) -> Result<String, UpdateError> {
        self.0.fetches.set(self.0.fetches.get() + 1);
        self.0.manifest.borrow().clone()
    }

    fn download(
        &self,
        _asset: &Asset,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<StagedFile, UpdateError> {
        self.0.downloads.set(self.0.downloads.get() + 1);
        for (done, total) in self.0.progress.borrow().iter() {
            progress(*done, *total);
        }
        self.0.staged.borrow().clone()
    }

    fn install(&self, _staged: StagedFile) -> Result<(), UpdateError> {
        self.0.installs.set(self.0.installs.get() + 1);
        self.0.install.borrow().clone()
    }
}

impl ArtifactVerifier for Verifier {
    fn verify(
        &self,
        claim: &UpdateClaim,
        _expected_sha256_hex: &str,
        _signature_b64: &str,
        _public_key: &[u8; 32],
    ) -> Result<(), VerifyError> {
        self.0.verifies.set(self.0.verifies.get() + 1);
        self.0.claims.borrow_mut().push(claim.clone());
        self.0.verify.get()
    }
}

pub(super) fn staged(size: u64) -> StagedFile {
    StagedFile {
        path: PathBuf::from("staged-update.bin"),
        size,
        sha256: [0xaa; 32],
    }
}

/// A base64 string that decodes to 64 bytes, as the manifest validation requires.
pub(super) fn signature() -> String {
    base64::engine::general_purpose::STANDARD.encode([0u8; 64])
}

/// A schema `schema` manifest for `version`, with an artifact for `platform` when given.
pub(super) fn manifest(schema: u32, version: &str, platform: Option<&str>) -> String {
    let assets = match platform {
        Some(key) => format!(
            r#""{key}": {{"url": "https://example.invalid/app.zip", "size": {ASSET_SIZE}, "sha256": "{}", "signature": "{}"}}"#,
            "a".repeat(64),
            signature()
        ),
        None => String::new(),
    };
    format!(
        r#"{{"schema": {schema}, "version": "{version}", "notes_en": "Fixes", "notes_vi": "Sửa lỗi", "assets": {{{assets}}}}}"#
    )
}
