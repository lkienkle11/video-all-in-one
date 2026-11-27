#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use gpui_kit::{AppContext, TitlebarOptions, WindowOptions};
use std::{path::PathBuf, sync::Arc};
use tokio::{runtime::Runtime, sync::mpsc};
use vaio_core::i18n::{tr, Language, Text};
use vaio_core::{
    domain::{job::JobState, tool::Tool},
    ports::{
        events::{EventSender, JobEvent},
        JobStore,
    },
    services::job_scheduler::{JobScheduler, Limits},
};
use vaio_desktop::app::AppView;
use vaio_desktop::state::{AppState, ToolSearch};
use vaio_desktop::update_wiring::UpdateSetup;
use vaio_media::{
    export_runner::{FfmpegExportRunner, ToolPathsCell},
    ffmpeg::FfmpegPaths,
};
use vaio_platform::channel::Channel;
use vaio_platform::{binary_resolver, paths::data_dir, settings_store::SettingsStore};
use vaio_storage::{Database, SqliteJobStore};

fn main() {
    if let Some(code) = vaio_platform::update::handle_apply_update(std::env::args_os().skip(1)) {
        std::process::exit(code);
    }
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Failed to start application runtime: {error}");
            return;
        }
    };
    let channel = Channel::current();
    let state = match initialize(&runtime, channel) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("Failed to open application data: {error}");
            return;
        }
    };
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let store = AppView::settings_store(channel);
            let language = store.read().language.unwrap_or(Language::English);

            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(tr(language, Text::AppTitle).into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                let state = cx.new(|_| state);
                let updates = match vaio_desktop::app::data_base() {
                    Some(base) => UpdateSetup::for_app(channel, data_dir(channel, &base)),
                    None => UpdateSetup::disabled(),
                };
                cx.new(|cx| {
                    AppView::with_updates(
                        store,
                        channel,
                        state,
                        updates,
                        |cx| cx.quit(),
                        window,
                        cx,
                    )
                })
            }) {
                eprintln!("Failed to open application window: {error:#}");
                cx.quit();
            }
        });
    // Keep the runtime alive until GPUI has released the workspace and its workers.
    drop(runtime);
}

fn initialize(runtime: &Runtime, channel: Channel) -> Result<AppState, String> {
    let base = vaio_desktop::app::data_base()
        .ok_or_else(|| "no absolute application data directory".to_owned())?;
    runtime.block_on(async {
        let (store, tools) = tokio::task::spawn_blocking(move || {
            let directory = data_dir(channel, &base);
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let store: Arc<dyn JobStore> = Arc::new(SqliteJobStore::new(
                Database::open(&directory.join("jobs.db")).map_err(|error| error.to_string())?,
            ));
            // An empty executable is deliberately unusable. Missing tools must not block the shell.
            // The folder the user chose last time is searched before the PATH.
            let tool_dir = SettingsStore::load(&directory).tool_dir;
            let ffmpeg = binary_resolver::resolve(Tool::Ffmpeg, tool_dir.as_deref())
                .unwrap_or_else(|_| PathBuf::new());
            let ffprobe = binary_resolver::resolve(Tool::Ffprobe, tool_dir.as_deref())
                .unwrap_or_else(|_| PathBuf::new());
            Ok::<_, String>((store, ToolPathsCell::new(FfmpegPaths { ffmpeg, ffprobe })))
        })
        .await
        .map_err(|error| error.to_string())??;
        let (sender, receiver) = mpsc::unbounded_channel();
        let runner = Arc::new(FfmpegExportRunner::with_shared_tools(
            store.clone(),
            tools.clone(),
        ));
        let scheduler = Arc::new(JobScheduler::new(
            store.clone(),
            runner,
            EventSender(sender),
            Limits::default(),
        ));
        if let Err(error) = scheduler.recover().await {
            eprintln!("Could not recover all jobs: {error}");
        }
        let jobs = tokio::task::spawn_blocking(move || store.list())
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let mut state = AppState::new(runtime.handle().clone(), None, receiver)
            .with_scheduler(scheduler)
            .with_tool_search(tools, ToolSearch::from_env());
        for stored in jobs {
            let job = stored.job;
            state.apply_event(JobEvent::StateChanged(job.id(), job.state()));
            if job.state() == JobState::Failed {
                if let Some(error) = job.error() {
                    state.apply_event(JobEvent::Failed(job.id(), error.to_owned()));
                }
            }
        }
        Ok(state)
    })
}
