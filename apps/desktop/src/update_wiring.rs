//! Connects the update service to the application: what to build, and how its states reach the UI.

use crate::ui::{
    settings::{SettingsView, UpdateCommand},
    update_dialog::UpdateDialog,
};
use gpui_kit::{App, WeakEntity, Window};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::runtime::Handle;
use vaio_core::{
    i18n::Language,
    ports::{ArtifactVerifier, UpdateBackend},
    update::{AvailableUpdate, UpdateService, UpdateState},
};

/// What the controller needs from an update service, without its generic parameters, so tests can
/// inject fakes. It runs on a blocking worker, never on the UI thread.
pub trait UpdateDriver: Send {
    fn check(&mut self);
    fn download(&mut self, progress: &mut dyn FnMut(u64, Option<u64>));
    fn install(&mut self);
    fn state(&self) -> UpdateState;
}

impl<B: UpdateBackend + Send, V: ArtifactVerifier + Send> UpdateDriver for UpdateService<B, V> {
    fn check(&mut self) {
        UpdateService::check(self);
    }
    fn download(&mut self, progress: &mut dyn FnMut(u64, Option<u64>)) {
        self.download_with_progress(progress);
    }
    fn install(&mut self) {
        UpdateService::install(self);
    }
    fn state(&self) -> UpdateState {
        self.state().clone()
    }
}

type StateObserver = Rc<dyn Fn(&UpdateState)>;

/// Everything `AppView` needs to run updates, built outside the view.
pub struct UpdateSetup {
    pub(crate) data_dir: Option<PathBuf>,
    pub(crate) driver: Option<Box<dyn UpdateDriver>>,
    pub(crate) restart: Arc<AtomicBool>,
    pub(crate) start_check: bool,
    pub(crate) observer: Option<StateObserver>,
}

impl UpdateSetup {
    /// No update work at all: used where the app is embedded without a data folder.
    pub fn disabled() -> Self {
        Self {
            data_dir: None,
            driver: None,
            restart: Arc::new(AtomicBool::new(false)),
            start_check: false,
            observer: None,
        }
    }
}

impl UpdateSetup {
    /// A service to drive. With the placeholder public key nothing is driven: the UI says that
    /// updates are not set up and no download can start.
    pub fn new(
        driver: impl UpdateDriver + 'static,
        restart: Arc<AtomicBool>,
        data_dir: PathBuf,
        public_key: &[u8; 32],
    ) -> Self {
        let driver: Option<Box<dyn UpdateDriver>> =
            vaio_platform::update::public_key::is_configured(public_key)
                .then(|| Box::new(driver) as Box<dyn UpdateDriver>);
        Self {
            data_dir: Some(data_dir),
            driver,
            restart,
            start_check: true,
            observer: None,
        }
    }

    /// Updates cannot run in this build, and the UI says so.
    pub fn not_set_up(data_dir: PathBuf) -> Self {
        Self {
            data_dir: Some(data_dir),
            start_check: true,
            ..Self::disabled()
        }
    }

    /// Calls `observer` with every state that reaches the UI.
    pub fn observe(mut self, observer: impl Fn(&UpdateState) + 'static) -> Self {
        self.observer = Some(Rc::new(observer));
        self
    }

    /// The setup of the real application: the HTTPS backend and the Ed25519 verifier, when the
    /// build has a manifest URL, a known platform and a configured public key.
    pub fn for_app(channel: vaio_platform::channel::Channel, data_dir: PathBuf) -> Self {
        use vaio_platform::update::{
            http::HttpUpdateBackend,
            manifest_url, platform_key,
            public_key::{is_configured, UPDATE_PUBLIC_KEY},
            verify::Ed25519Verifier,
        };
        if !channel.updates_enabled() || !is_configured(&UPDATE_PUBLIC_KEY) {
            return Self::not_set_up(data_dir);
        }
        let (Some(url), Some(platform), Ok(current)) = (
            manifest_url(),
            platform_key(),
            env!("CARGO_PKG_VERSION").parse(),
        ) else {
            return Self::not_set_up(data_dir);
        };
        let restart = Arc::new(AtomicBool::new(false));
        let flag = restart.clone();
        let Ok(backend) = HttpUpdateBackend::new(url, data_dir.join("updates")) else {
            return Self::not_set_up(data_dir);
        };
        let backend = backend
            .with_data_dir(data_dir.clone())
            .with_restart_hook(Box::new(move || {
                flag.store(true, std::sync::atomic::Ordering::SeqCst)
            }));
        let service = UpdateService::new(
            backend,
            Ed25519Verifier,
            current,
            platform.to_owned(),
            UPDATE_PUBLIC_KEY,
            true,
        );
        Self::new(service, restart, data_dir, &UPDATE_PUBLIC_KEY)
    }
}

/// What the user asked the updater to do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Check,
    Download,
    Install,
}

/// Runs update steps on a blocking worker and shows every state in Settings.
pub(crate) struct UpdateController {
    driver: Option<Arc<Mutex<Box<dyn UpdateDriver>>>>,
    restart: Arc<AtomicBool>,
    runtime: Handle,
    settings: RefCell<Option<WeakEntity<SettingsView>>>,
    on_restart: Rc<dyn Fn(&mut App)>,
    observer: Option<StateObserver>,
    busy: Cell<bool>,
}

impl UpdateController {
    pub(crate) fn new(
        setup: &mut UpdateSetup,
        runtime: Handle,
        on_restart: Rc<dyn Fn(&mut App)>,
    ) -> Rc<Self> {
        Rc::new(Self {
            driver: setup
                .driver
                .take()
                .map(|driver| Arc::new(Mutex::new(driver))),
            restart: setup.restart.clone(),
            runtime,
            settings: RefCell::new(None),
            on_restart,
            observer: setup.observer.take(),
            busy: Cell::new(false),
        })
    }

    pub(crate) fn attach(&self, settings: WeakEntity<SettingsView>) {
        *self.settings.borrow_mut() = Some(settings);
    }

    /// Runs what a Settings button asked for.
    pub(crate) fn command(
        self: &Rc<Self>,
        command: UpdateCommand,
        window: &mut Window,
        cx: &mut App,
    ) {
        let step = match command {
            UpdateCommand::Check => Step::Check,
            UpdateCommand::Download => Step::Download,
            UpdateCommand::Install => Step::Install,
        };
        self.run(step, false, window, cx);
    }

    /// The silent check at start.
    pub(crate) fn check_at_start(self: &Rc<Self>, window: &mut Window, cx: &mut App) {
        self.run(Step::Check, false, window, cx);
    }

    fn show(&self, state: UpdateState, cx: &mut App) {
        if let Some(observer) = &self.observer {
            observer(&state);
        }
        if let Some(settings) = self.settings.borrow().as_ref().and_then(|s| s.upgrade()) {
            settings.update(cx, |settings, cx| settings.set_update_state(state, cx));
        }
    }

    fn run(self: &Rc<Self>, step: Step, then_install: bool, window: &mut Window, cx: &mut App) {
        let Some(driver) = self.driver.clone() else {
            if let Some(settings) = self.settings.borrow().as_ref().and_then(|s| s.upgrade()) {
                settings.update(cx, |settings, cx| settings.set_updates_unavailable(cx));
            }
            return;
        };
        if self.busy.replace(true) {
            return;
        }
        match step {
            Step::Check => self.show(UpdateState::Checking, cx),
            Step::Download => self.show(
                UpdateState::Downloading {
                    done: 0,
                    total: None,
                },
                cx,
            ),
            Step::Install => {}
        }
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        self.runtime.spawn_blocking(move || {
            let mut driver = driver
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match step {
                Step::Check => driver.check(),
                Step::Download => {
                    let progress = sender.clone();
                    driver.download(&mut |done, total| {
                        let _ = progress.send(UpdateState::Downloading { done, total });
                    });
                }
                Step::Install => driver.install(),
            }
            let _ = sender.send(driver.state());
        });
        let this = self.clone();
        window
            .spawn(cx, async move |cx| {
                let mut last = None;
                while let Some(state) = receiver.recv().await {
                    last = Some(state.clone());
                    if cx.update(|_, cx| this.show(state, cx)).is_err() {
                        return;
                    }
                }
                cx.update(|window, cx| this.finished(step, then_install, last, window, cx))
                    .ok();
            })
            .detach();
    }

    fn finished(
        self: &Rc<Self>,
        step: Step,
        then_install: bool,
        state: Option<UpdateState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.busy.set(false);
        match (step, state) {
            (Step::Check, Some(UpdateState::Available(update))) => self.offer(update, window, cx),
            (Step::Download, Some(UpdateState::Ready(_))) if then_install => {
                self.run(Step::Install, false, window, cx)
            }
            (Step::Install, Some(UpdateState::Installed))
                if self.restart.swap(false, Ordering::SeqCst) =>
            {
                (self.on_restart)(cx)
            }
            _ => {}
        }
    }

    fn offer(self: &Rc<Self>, update: AvailableUpdate, window: &mut Window, cx: &mut App) {
        let language = self
            .settings
            .borrow()
            .as_ref()
            .and_then(|settings| settings.upgrade())
            .map_or(Language::English, |settings| settings.read(cx).language());
        let this = self.clone();
        UpdateDialog::open(
            language,
            update,
            move |window, cx| this.run(Step::Download, true, window, cx),
            |_, _| {},
            window,
            cx,
        );
    }
}
