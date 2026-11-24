//! The updater wired into the application, with a fake backend and the real `UpdateService`.
//! Nothing here uses the network.

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AnyWindowHandle, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions,
};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use vaio_core::{
    i18n::Language,
    ports::{ArtifactVerifier, UpdateBackend},
    settings::Settings,
    update::{
        Asset, StagedFile, UpdateClaim, UpdateError, UpdateService, UpdateState, VerifyError,
    },
};
use vaio_desktop::{app::AppView, update_wiring::UpdateSetup};
use vaio_platform::{channel::Channel, settings_store::SettingsStore};

const KEY: [u8; 32] = [7; 32];
const SIZE: u64 = 12_500_000;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn directory() -> Directory {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vaio-updater-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Directory(dir)
}

/// What the fake backend returns and how often it was called.
struct Fake {
    manifest: Mutex<Result<String, UpdateError>>,
    fetches: AtomicUsize,
    downloads: AtomicUsize,
    installs: AtomicUsize,
    restart: Arc<AtomicBool>,
}
impl Fake {
    fn new(version: &str) -> Arc<Self> {
        Arc::new(Self {
            manifest: Mutex::new(Ok(manifest(version))),
            fetches: AtomicUsize::new(0),
            downloads: AtomicUsize::new(0),
            installs: AtomicUsize::new(0),
            restart: Arc::new(AtomicBool::new(false)),
        })
    }
    fn calls(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
            + self.downloads.load(Ordering::SeqCst)
            + self.installs.load(Ordering::SeqCst)
    }
}

fn manifest(version: &str) -> String {
    format!(
        r#"{{"schema": 1, "version": "{version}", "notes_en": "Fixed playback", "notes_vi": "Sửa phát video", "assets": {{"test-platform": {{"url": "https://example.invalid/app.zip", "size": {SIZE}, "sha256": "{}", "signature": "{}=="}}}}}}"#,
        "a".repeat(64),
        "A".repeat(86)
    )
}

struct FakeBackend(Arc<Fake>);
impl UpdateBackend for FakeBackend {
    fn fetch_manifest(&self) -> Result<String, UpdateError> {
        self.0.fetches.fetch_add(1, Ordering::SeqCst);
        self.0.manifest.lock().unwrap().clone()
    }
    fn download(
        &self,
        _: &Asset,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<StagedFile, UpdateError> {
        self.0.downloads.fetch_add(1, Ordering::SeqCst);
        progress(SIZE / 2, Some(SIZE));
        Ok(StagedFile {
            path: PathBuf::from("staged.bin"),
            size: SIZE,
            sha256: [0xaa; 32],
        })
    }
    fn install(&self, _: StagedFile) -> Result<(), UpdateError> {
        self.0.installs.fetch_add(1, Ordering::SeqCst);
        self.0.restart.store(true, Ordering::SeqCst);
        Ok(())
    }
}
struct OkVerifier;
impl ArtifactVerifier for OkVerifier {
    fn verify(&self, _: &UpdateClaim, _: &str, _: &str, _: &[u8; 32]) -> Result<(), VerifyError> {
        Ok(())
    }
}

fn service(fake: &Arc<Fake>, key: [u8; 32]) -> UpdateService<FakeBackend, OkVerifier> {
    UpdateService::new(
        FakeBackend(fake.clone()),
        OkVerifier,
        "1.0.0".parse().unwrap(),
        "test-platform".to_owned(),
        key,
        true,
    )
}

struct App {
    handle: AnyWindowHandle,
    restarts: Rc<Cell<usize>>,
    states: Rc<RefCell<Vec<UpdateState>>>,
}

fn open(
    cx: &mut TestAppContext,
    dir: &Path,
    channel: Channel,
    language: Language,
    setup: impl FnOnce(Rc<RefCell<Vec<UpdateState>>>) -> UpdateSetup,
) -> App {
    cx.update(gpui_kit::init);
    SettingsStore::new(dir)
        .save(&Settings {
            language: Some(language),
            ..Default::default()
        })
        .unwrap();
    let restarts = Rc::new(Cell::new(0));
    let states = Rc::new(RefCell::new(Vec::new()));
    let setup = setup(states.clone());
    let counter = restarts.clone();
    let (handle, _) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                Default::default(),
                size(px(1000.), px(800.)),
            ))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| {
            let (_, receiver) = tokio::sync::mpsc::unbounded_channel();
            let state = cx.new(|_| {
                vaio_desktop::state::AppState::new(runtime().handle().clone(), None, receiver)
            });
            cx.new(|cx| {
                AppView::with_updates(
                    SettingsStore::new(dir),
                    channel,
                    state,
                    setup,
                    move |_| counter.set(counter.get() + 1),
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    App {
        handle,
        restarts,
        states,
    }
}

fn recording(
    fake: &Arc<Fake>,
    dir: &Path,
    key: [u8; 32],
) -> impl FnOnce(Rc<RefCell<Vec<UpdateState>>>) -> UpdateSetup {
    let (fake, dir) = (fake.clone(), dir.to_owned());
    move |states| {
        let restart = fake.restart.clone();
        UpdateSetup::new(service(&fake, key), restart, dir, &key)
            .observe(move |state| states.borrow_mut().push(state.clone()))
    }
}

async fn wait_until(
    cx: &mut TestAppContext,
    what: &str,
    mut done: impl FnMut(&mut TestAppContext) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        runtime()
            .spawn(async { tokio::time::sleep(Duration::from_millis(10)).await })
            .await
            .unwrap();
    }
}

fn label(cx: &mut TestAppContext, app: &App, id: &'static str) -> Option<String> {
    cx.update_window(app.handle, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find(id)
            .map(|element| element.label().unwrap_or_default().to_owned())
    })
    .unwrap()
}

fn click(cx: &mut TestAppContext, app: &App, id: &'static str) {
    cx.update_window(app.handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn collapsed(states: &Rc<RefCell<Vec<UpdateState>>>) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = states
        .borrow()
        .iter()
        .map(|state| match state {
            UpdateState::Idle => "Idle",
            UpdateState::Checking => "Checking",
            UpdateState::UpToDate => "UpToDate",
            UpdateState::Available(_) => "Available",
            UpdateState::Downloading { .. } => "Downloading",
            UpdateState::Ready(_) => "Ready",
            UpdateState::InstallFailed { .. } => "InstallFailed",
            UpdateState::Installed => "Installed",
            UpdateState::Failed(_) => "Failed",
        })
        .collect();
    names.dedup();
    names
}

fn has(states: &Rc<RefCell<Vec<UpdateState>>>, name: &'static str) -> bool {
    collapsed(states).contains(&name)
}

#[gpui_kit::test]
async fn development_channel_never_calls_the_backend(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, fake) = (directory(), Fake::new("1.1.0"));
    let app = open(
        cx,
        &dir.0,
        Channel::Development,
        Language::English,
        recording(&fake, &dir.0, KEY),
    );
    cx.run_until_parked();
    runtime()
        .spawn(async { tokio::time::sleep(Duration::from_millis(200)).await })
        .await
        .unwrap();
    cx.run_until_parked();
    click(cx, &app, "workspace-settings");
    assert_eq!(label(cx, &app, "update-action"), None, "no update button");
    assert_eq!(label(cx, &app, "updates"), None);
    assert_eq!(fake.calls(), 0);
    assert!(app.states.borrow().is_empty());
}

#[gpui_kit::test]
async fn production_start_checks_once_and_shows_the_update_dialog_when_newer(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    for (language, notes, title) in [
        (Language::English, "Fixed playback", "Update now?"),
        (
            Language::Vietnamese,
            "Sửa phát video",
            "Cập nhật ngay bây giờ?",
        ),
    ] {
        let (dir, fake) = (directory(), Fake::new("1.1.0"));
        let app = open(
            cx,
            &dir.0,
            Channel::Production,
            language,
            recording(&fake, &dir.0, KEY),
        );
        wait_until(cx, "the update dialog", |cx| {
            label(cx, &app, "update-notes").is_some()
        })
        .await;
        assert_eq!(label(cx, &app, "modal-title").as_deref(), Some(title));
        assert_eq!(label(cx, &app, "update-notes").as_deref(), Some(notes));
        assert!(label(cx, &app, "update-size").unwrap().contains("12.5 MB"));
        assert_eq!(fake.fetches.load(Ordering::SeqCst), 1);
        assert_eq!(fake.downloads.load(Ordering::SeqCst), 0);
        click(cx, &app, "update-later");
        assert_eq!(label(cx, &app, "update-notes"), None);
        assert_eq!(fake.calls(), 1, "Later starts nothing");
    }
}

#[gpui_kit::test]
async fn check_download_install_walks_the_states_in_order(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, fake) = (directory(), Fake::new("1.1.0"));
    let app = open(
        cx,
        &dir.0,
        Channel::Production,
        Language::English,
        recording(&fake, &dir.0, KEY),
    );
    wait_until(cx, "the update dialog", |cx| {
        label(cx, &app, "update-now").is_some()
    })
    .await;
    assert_eq!(app.restarts.get(), 0);
    click(cx, &app, "update-now");
    wait_until(cx, "the install", |_| has(&app.states, "Installed")).await;
    assert_eq!(
        collapsed(&app.states),
        ["Checking", "Available", "Downloading", "Ready", "Installed"]
    );
    assert_eq!(fake.installs.load(Ordering::SeqCst), 1);
    assert_eq!(app.restarts.get(), 1, "the restart hook runs once");
    cx.run_until_parked();
    assert_eq!(app.restarts.get(), 1);
}

#[gpui_kit::test]
async fn update_is_refused_when_the_public_key_is_not_configured(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, fake) = (directory(), Fake::new("1.1.0"));
    let app = open(
        cx,
        &dir.0,
        Channel::Production,
        Language::English,
        recording(&fake, &dir.0, [0; 32]),
    );
    cx.run_until_parked();
    click(cx, &app, "workspace-settings");
    let message = "Updates are not set up in this build.";
    assert_eq!(label(cx, &app, "update-state").as_deref(), Some(message));
    click(cx, &app, "update-action");
    assert_eq!(label(cx, &app, "update-state").as_deref(), Some(message));
    assert_eq!(
        fake.calls(),
        0,
        "nothing is fetched, downloaded or installed"
    );
    assert_eq!(label(cx, &app, "update-notes"), None);
    assert!(app.states.borrow().is_empty());
}

#[gpui_kit::test]
async fn up_to_date_manifest_shows_up_to_date(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, fake) = (directory(), Fake::new("1.0.0"));
    let app = open(
        cx,
        &dir.0,
        Channel::Production,
        Language::English,
        recording(&fake, &dir.0, KEY),
    );
    wait_until(cx, "the check", |_| has(&app.states, "UpToDate")).await;
    click(cx, &app, "workspace-settings");
    assert_eq!(
        label(cx, &app, "update-state").as_deref(),
        Some("You have the latest version")
    );
    assert_eq!(label(cx, &app, "update-notes"), None);
}

#[gpui_kit::test]
async fn network_error_leaves_the_app_usable_and_shows_the_error(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, fake) = (directory(), Fake::new("1.1.0"));
    *fake.manifest.lock().unwrap() = Err(UpdateError::Network("offline".into()));
    let app = open(
        cx,
        &dir.0,
        Channel::Production,
        Language::English,
        recording(&fake, &dir.0, KEY),
    );
    wait_until(cx, "the failed check", |_| has(&app.states, "Failed")).await;
    assert!(label(cx, &app, "workspace-toolbar").is_some(), "app usable");
    click(cx, &app, "workspace-settings");
    assert_eq!(
        label(cx, &app, "update-state").as_deref(),
        Some("Couldn’t update. Try again.")
    );
    assert_eq!(
        label(cx, &app, "update-action").as_deref(),
        Some("Try again")
    );
    click(cx, &app, "update-action");
    wait_until(cx, "the second check", |_| {
        fake.fetches.load(Ordering::SeqCst) == 2
    })
    .await;
}

#[gpui_kit::test]
async fn update_error_file_is_shown_once_and_removed(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (dir, fake) = (directory(), Fake::new("1.1.0"));
    let file = dir.0.join("update-error.txt");
    std::fs::write(
        &file,
        "The update could not be completed.\nWhat failed: test",
    )
    .unwrap();
    let app = open(
        cx,
        &dir.0,
        Channel::Production,
        Language::English,
        recording(&fake, &dir.0, [0; 32]),
    );
    cx.run_until_parked();
    assert_eq!(
        label(cx, &app, "modal-title").as_deref(),
        Some("The last update did not finish")
    );
    assert_eq!(
        label(cx, &app, "update-error-text").as_deref(),
        Some("The update could not be completed.\nWhat failed: test")
    );
    assert!(!file.exists(), "the file is removed once shown");
    click(cx, &app, "update-error-close");
    assert_eq!(label(cx, &app, "update-error-text"), None);
    let again = open(
        cx,
        &dir.0,
        Channel::Production,
        Language::English,
        recording(&fake, &dir.0, [0; 32]),
    );
    cx.run_until_parked();
    assert_eq!(label(cx, &again, "update-error-text"), None, "shown once");
}
