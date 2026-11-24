#![cfg(unix)]
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
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
    ffprobe::{ffprobe_args, parse_ffprobe_json},
};
use vaio_storage::{Database, SqliteJobStore};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = kill_leftover_ffmpeg(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn tools() -> FfmpegPaths {
    FfmpegPaths {
        ffmpeg: "/opt/homebrew/bin/ffmpeg".into(),
        ffprobe: "/opt/homebrew/bin/ffprobe".into(),
    }
}

// Limit process cleanup to FFmpeg commands containing this test's unique directory.
fn kill_leftover_ffmpeg(root: &Path) -> bool {
    let result = Command::new("ps")
        .args(["-ax", "-o", "pid=,command="])
        .output();
    let output = match result {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            eprintln!(
                "ps -ax -o pid=,command= denied: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return false;
        }
        Err(error) => {
            eprintln!("ps -ax -o pid=,command= denied: {error}");
            return false;
        }
    };
    let marker = root.to_str().unwrap();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        let Some((pid, command)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if command.contains("ffmpeg") && command.contains(marker) && pid.parse::<u32>().is_ok() {
            let status = Command::new("kill").args(["-KILL", pid]).status().unwrap();
            assert!(status.success(), "could not kill leftover FFmpeg pid {pid}");
        }
    }
    true
}

#[test]
fn killed_app_leaves_failed_job_and_clean_temp_files() {
    let root = std::env::temp_dir().join(format!(
        "vaio-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let _fixture = Fixture(root.clone());
    let source = root.join("long source.mp4");
    let out_dir = root.join("output");
    fs::create_dir(&out_dir).unwrap();
    let generated = Command::new(tools().ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-n",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=25",
            "-t",
            "120",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "25",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "fixture: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let database = root.join("jobs.sqlite");
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_vaio-recovery-child"))
            .arg(&database)
            .arg(&source)
            .arg(&out_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send((result, line));
    });
    let (read, line) = rx
        .recv_timeout(Duration::from_secs(20))
        .expect("child did not report Running within 20 s");
    assert!(read.unwrap() > 0, "child exited before Running");
    assert_eq!(line.trim(), "running");
    reader.join().unwrap();
    // Running is persisted before FFmpeg starts. Wait for a real partial export,
    // so this test cannot pass by killing the child before it creates any files.
    let piece = out_dir.join("crash.piece-01.partial.mp4");
    let deadline = Instant::now() + Duration::from_secs(20);
    while fs::metadata(&piece).map(|m| m.len()).unwrap_or(0) <= 48 {
        assert!(
            Instant::now() < deadline,
            "FFmpeg did not write a partial export"
        );
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited during export"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(out_dir.join("crash.concat.txt").exists());
    child.0.kill().unwrap();
    let status = child.0.wait().unwrap();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(status.signal(), Some(9));
    let listed = kill_leftover_ffmpeg(&out_dir);
    if !listed {
        eprintln!("UNPROVEN: orphan process termination; ps is unavailable in this sandbox");
    }

    let store = Arc::new(SqliteJobStore::new(Database::open(&database).unwrap()));
    let jobs = store.list().unwrap();
    assert_eq!(jobs.len(), 1);
    let old = jobs[0].job.id();
    assert_eq!(jobs[0].job.state(), JobState::Running);
    assert!(piece.exists());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let runner = Arc::new(FfmpegExportRunner::new(store.clone(), tools()));
        let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let scheduler = JobScheduler::new(
            store.clone(),
            runner,
            EventSender(events),
            Limits::default(),
        );
        scheduler.recover().await.unwrap();
        let recovered = store.get(old).unwrap().job;
        assert_eq!(recovered.state(), JobState::Failed);
        assert!(recovered.error().unwrap().contains("interrupted"));
        assert!(!out_dir.join("crash.mp4").exists());
        assert_eq!(
            fs::read_dir(&out_dir).unwrap().count(),
            0,
            "recovery left temporary outputs"
        );
        assert!(source.exists());
        let payload = ExportJobPayload {
            source: source.clone(),
            segments: vec![(0, 1_000)],
            cut: CutMode::Precise,
            export: ExportMode::Separate,
            out_dir: out_dir.clone(),
            stem: "fresh".into(),
        }
        .to_json()
        .unwrap();
        let new = scheduler
            .submit(NewJob {
                kind: JobKind::Cut,
                payload,
            })
            .unwrap();
        assert_ne!(new, old);
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let job = store.get(new).unwrap().job;
                match job.state() {
                    JobState::Succeeded => break,
                    JobState::Failed | JobState::Cancelled => panic!("new export failed: {job:?}"),
                    _ => {
                        let _ =
                            tokio::time::timeout(Duration::from_millis(20), receiver.recv()).await;
                    }
                }
            }
        })
        .await
        .expect("new export did not finish");
        assert_eq!(store.get(old).unwrap().job.state(), JobState::Failed);
    });
    let final_output = out_dir.join("fresh-01.mp4");
    let probe = Command::new(tools().ffprobe)
        .args(ffprobe_args(&final_output).unwrap())
        .output()
        .unwrap();
    assert!(probe.status.success());
    let info = parse_ffprobe_json(std::str::from_utf8(&probe.stdout).unwrap()).unwrap();
    assert!(info.duration.0.abs_diff(1_000) <= 140);
    assert_eq!(fs::read_dir(&out_dir).unwrap().count(), 1);
}
