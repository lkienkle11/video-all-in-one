#[cfg(unix)]
use std::time::Duration;
use std::{ffi::OsString, path::PathBuf};
use tokio_util::sync::CancellationToken;
use vaio_media::process_runner::{run, ProcessSpec, RunError, Stream};

fn spec(args: &[&str]) -> ProcessSpec {
    ProcessSpec {
        program: env!("CARGO_BIN_EXE_vaio-test-helper").into(),
        args: args.iter().map(OsString::from).collect(),
        cwd: None,
    }
}

#[tokio::test]
async fn runner_streams_stdout_lines_in_order() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run(
        spec(&["lines"]),
        |stream, line| match stream {
            Stream::Stdout => stdout.push(line.to_owned()),
            Stream::Stderr => stderr.push(line.to_owned()),
        },
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        stdout,
        (0..100).map(|i| format!("out {i}")).collect::<Vec<_>>()
    );
    assert_eq!(
        stderr,
        (0..100).map(|i| format!("err {i}")).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn runner_returns_exit_code_and_stderr_tail() {
    let info = run(spec(&["exit"]), |_, _| {}, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(info.code, Some(23));
    assert_eq!(info.stderr_tail, "chosen failure\n");
}

#[tokio::test]
async fn args_with_spaces_unicode_and_leading_dash_arrive_unchanged() {
    let mut lines = Vec::new();
    run(
        spec(&["echo", "with spaces", "tiếng Việt 界", "-ss.mp4"]),
        |_, s| lines.push(s.to_owned()),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(lines, ["with spaces", "tiếng Việt 界", "-ss.mp4"]);
}

#[tokio::test]
async fn runner_does_not_use_a_shell() {
    let mut lines = Vec::new();
    run(
        spec(&["echo", "; echo hacked"]),
        |_, s| lines.push(s.to_owned()),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(lines, ["; echo hacked"]);
}

#[tokio::test]
async fn long_lines_are_split_lossy_and_tail_is_bounded() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let info = run(
        spec(&["bytes"]),
        |stream, s| match stream {
            Stream::Stdout => stdout.push(s.to_owned()),
            Stream::Stderr => stderr.push(s.to_owned()),
        },
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(stdout, ["a�"]);
    assert!(stderr.len() >= 2);
    assert!(stderr.iter().all(|s| s.len() <= 65_536));
    assert_eq!(stderr.concat(), "界".repeat(30_000));
    assert!(info.stderr_tail.len() <= 8192);
    assert!(info.stderr_tail.ends_with(&"界".repeat(2700)));
}

#[tokio::test]
async fn secret_values_are_redacted_from_lines_and_tail() {
    let mut lines = Vec::new();
    let info = run(
        spec(&[
            "stderr",
            "--api-key",
            "one-secret",
            "--TOKEN",
            "two-secret",
            "--password",
            "three-secret",
            "--secret",
            "four-secret",
            "--authorization",
            "five-secret",
        ]),
        |_, s| lines.push(s.to_owned()),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    for value in [
        "one-secret",
        "two-secret",
        "three-secret",
        "four-secret",
        "five-secret",
    ] {
        assert!(!info.stderr_tail.contains(value));
        assert!(lines.iter().all(|s| !s.contains(value)));
    }
    assert!(info.stderr_tail.contains("***"));
}

#[tokio::test]
async fn secrets_crossing_long_line_boundaries_are_redacted() {
    let mut lines = Vec::new();
    let info = run(
        spec(&["boundary", "--token", "sensitive-value"]),
        |_, s| lines.push(s.to_owned()),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(lines.concat(), format!("{}***", "x".repeat(65_533)));
    assert!(info.stderr_tail.ends_with("***\n"));
}

#[tokio::test]
async fn spawn_failure_is_typed_and_secret_free() {
    let mut s = spec(&["--key", "secret-value"]);
    s.program = PathBuf::from("/nonexistent/secret-value/program");
    let error = run(s, |_, _| {}, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, RunError::Spawn(_)));
    assert!(!format!("{error:?}").contains("secret-value"));
}

#[tokio::test]
async fn already_cancelled_does_not_spawn() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut s = spec(&[]);
    s.program = "/nonexistent/program".into();
    assert!(matches!(
        run(s, |_, _| {}, cancel).await,
        Err(RunError::Cancelled)
    ));
}

#[tokio::test]
async fn cwd_is_forwarded() {
    let cwd = std::env::current_dir().unwrap();
    let mut s = spec(&["cwd"]);
    s.cwd = Some(cwd.clone());
    let mut lines = Vec::new();
    run(
        s,
        |_, line| lines.push(line.to_owned()),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(lines, [cwd.display().to_string()]);
}

// All native calls, including the bounded ESRCH probe, live in this one module.
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../src/process_runner/group.rs"]
mod group;

#[cfg(unix)]
mod lifecycle {
    use super::*;

    fn pid_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vaio-runner-{}-{name}.pid", std::process::id()))
    }

    async fn assert_dead(file: &std::path::Path) {
        let pid = std::fs::read_to_string(file).unwrap().parse().unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !group::is_dead(pid) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("grandchild must be gone (ESRCH)");
    }

    #[tokio::test]
    async fn cancel_kills_the_process_and_its_child() {
        let file = pid_file("cancel");
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        let result = tokio::time::timeout(
            Duration::from_secs(6),
            run(
                spec(&["tree", file.to_str().unwrap()]),
                move |_, line| {
                    if line == "ready" {
                        trigger.cancel();
                    }
                },
                cancel,
            ),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(RunError::Cancelled)), "{result:?}");
        assert_dead(&file).await;
    }

    #[tokio::test]
    async fn dropping_run_future_kills_the_group() {
        let file = pid_file("drop");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut tx = Some(tx);
        let args = spec(&["tree", file.to_str().unwrap()]);
        let task = tokio::spawn(run(
            args,
            move |_, line| {
                if line == "ready" {
                    if let Some(tx) = tx.take() {
                        let _ = tx.send(());
                    }
                }
            },
            CancellationToken::new(),
        ));
        tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .unwrap()
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_dead(&file).await;
    }
}
