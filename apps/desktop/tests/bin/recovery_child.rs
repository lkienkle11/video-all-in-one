//! Process killed by the recovery integration test after its export starts.
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use vaio_core::{
    domain::{
        export::{CutMode, ExportMode},
        job::{JobKind, JobState},
    },
    ports::{events::EventSender, JobStore},
    services::job_scheduler::{JobScheduler, Limits, NewJob},
};
use vaio_media::{
    export_runner::{ExportJobPayload, FfmpegExportRunner},
    ffmpeg::FfmpegPaths,
};
use vaio_storage::{Database, SqliteJobStore};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let database = PathBuf::from(args.next().ok_or("missing database argument")?);
    let source = PathBuf::from(args.next().ok_or("missing source argument")?);
    let out_dir = PathBuf::from(args.next().ok_or("missing output folder argument")?);
    let store = Arc::new(SqliteJobStore::new(Database::open(&database)?));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let runner = Arc::new(FfmpegExportRunner::new(
            store.clone(),
            FfmpegPaths {
                ffmpeg: "/opt/homebrew/bin/ffmpeg".into(),
                ffprobe: "/opt/homebrew/bin/ffprobe".into(),
            },
        ));
        let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let scheduler = JobScheduler::new(
            store.clone(),
            runner,
            EventSender(events),
            Limits::default(),
        );
        let payload = ExportJobPayload {
            source,
            segments: vec![(0, 55_000), (60_000, 120_000)],
            cut: CutMode::Precise,
            export: ExportMode::Merged,
            out_dir,
            stem: "crash".into(),
        }
        .to_json()?;
        let id = scheduler.submit(NewJob {
            kind: JobKind::Merge,
            payload,
        })?;
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                match store.get(id)?.job.state() {
                    JobState::Running => break,
                    JobState::Failed => return Err("export failed before Running".into()),
                    _ => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await??;
        println!("running");
        io::stdout().flush()?;
        while receiver.recv().await.is_some() {}
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}
