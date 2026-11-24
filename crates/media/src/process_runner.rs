//! Runs external media programs without a shell.
use std::{ffi::OsString, fmt, path::PathBuf, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

mod group;
mod output;

use output::{Output, Secrets};

/// An executable and its literal arguments; no shell is involved.
#[derive(Debug)]
pub struct ProcessSpec {
    /// Executable path, resolved by the caller.
    pub program: PathBuf,
    /// Each entry reaches the executable as one argument.
    pub args: Vec<OsString>,
    /// Working directory, or the current directory when absent.
    pub cwd: Option<PathBuf>,
}
/// Completion status and the last (at most) 8 KiB of redacted stderr.
#[derive(Debug)]
pub struct ExitInfo {
    /// Exit code, or `None` when terminated by a signal.
    pub code: Option<i32>,
    /// UTF-8 text with secret argument values replaced by `***`.
    pub stderr_tail: String,
}
/// Origin of a delivered line or long-line fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}
/// Failure to start, cancel, or communicate with a process.
#[derive(Debug)]
pub enum RunError {
    /// Process creation or process-group setup failed (redacted OS message).
    Spawn(String),
    /// Cancellation was requested; process-tree cleanup has run.
    Cancelled,
    /// Pipe, wait, or process-group operation failed (redacted OS message).
    Io(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(message) => write!(f, "could not spawn process: {message}"),
            Self::Cancelled => f.write_str("process cancelled"),
            Self::Io(message) => write!(f, "process I/O failed: {message}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Runs on a Tokio runtime, delivering lines in order within each stream.
///
/// Lines omit their newline; lines over 64 KiB are split on character boundaries.
/// Invalid UTF-8 is replaced lossily. Secret values following argument names
/// containing key, token, password, secret or authorization are redacted before
/// callbacks, the stderr tail, or errors see them. Callbacks run on this future.
/// Cancellation terminates the process group, escalating after at most two
/// seconds; dropping this future force-kills the group as well.
///
/// # Errors
/// Returns [`RunError::Spawn`] for startup failures, [`RunError::Cancelled`] for
/// cancellation, or [`RunError::Io`] for pipe, wait and cleanup failures.
pub async fn run(
    spec: ProcessSpec,
    mut on_line: impl FnMut(Stream, &str),
    cancel: CancellationToken,
) -> Result<ExitInfo, RunError> {
    if cancel.is_cancelled() {
        return Err(RunError::Cancelled);
    }
    let secrets = Secrets::from_args(&spec.args);
    let spawn_error = |e: std::io::Error| RunError::Spawn(secrets.redact(&e.to_string()));
    let io_error = |e: std::io::Error| RunError::Io(secrets.redact(&e.to_string()));
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    let mut group = group::Group::prepare(&mut command).map_err(spawn_error)?;
    let mut child = command.spawn().map_err(spawn_error)?;
    group.attach(&child).map_err(spawn_error)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| RunError::Io("stdout pipe was not created".into()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| RunError::Io("stderr pipe was not created".into()))?;
    let mut out = Output::new(Stream::Stdout);
    let mut err = Output::new(Stream::Stderr);
    let (mut out_done, mut err_done) = (false, false);
    let (mut out_bytes, mut err_bytes) = ([0; 8192], [0; 8192]);
    let mut status = None;
    while !out_done || !err_done || status.is_none() {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                let term = group.terminate();
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
                let kill = group.kill();
                let waited = child.wait().await;
                term.map_err(io_error)?;
                kill.map_err(io_error)?;
                waited.map_err(io_error)?;
                return Err(RunError::Cancelled);
            }
            read = stdout.read(&mut out_bytes), if !out_done => {
                let count = read.map_err(io_error)?;
                out_done = count == 0;
                out.feed(&out_bytes[..count], out_done, &secrets, &mut on_line);
            }
            read = stderr.read(&mut err_bytes), if !err_done => {
                let count = read.map_err(io_error)?;
                err_done = count == 0;
                err.feed(&err_bytes[..count], err_done, &secrets, &mut on_line);
            }
            waited = child.wait(), if status.is_none() => {
                status = Some(waited.map_err(io_error)?);
                // Descendants must not keep pipes open after the program exits.
                group.kill().map_err(io_error)?;
            }
        }
    }
    Ok(ExitInfo {
        code: status.and_then(|status| status.code()),
        stderr_tail: err.tail,
    })
}
