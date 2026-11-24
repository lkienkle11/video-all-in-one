//! Blocking HTTPS transport; callers run it away from the UI thread.
use reqwest::{blocking::Client, redirect::Policy};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{sync_channel, Receiver, RecvTimeoutError},
    },
    thread,
    time::Duration,
};
use url::Url;
use vaio_core::{
    ports::UpdateBackend,
    update::{Asset, StagedFile, UpdateError},
};

/// Called after an install that needs the app to quit. The application supplies it;
/// the backend never exits the process itself.
pub type RestartHook = Box<dyn Fn() + Send + Sync>;

pub struct HttpUpdateBackend {
    manifest_url: Url,
    work_dir: PathBuf,
    data_dir: PathBuf,
    manifest_client: Client,
    download_client: Client,
    allow_loopback: bool,
    stall_timeout: Duration,
    restart: RestartHook,
}

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
const MANIFEST_LIMIT: u64 = 1_048_576;
const MANIFEST_DEADLINE: Duration = Duration::from_secs(120);
const STALL_TIMEOUT: Duration = Duration::from_secs(30);

impl HttpUpdateBackend {
    /// Refuses insecure URLs before creating a transport or touching disk.
    pub fn new(manifest_url: Url, work_dir: PathBuf) -> Result<Self, UpdateError> {
        Self::build(manifest_url, work_dir, false, STALL_TIMEOUT)
    }
    #[cfg(test)]
    pub(super) fn for_tests(manifest_url: Url, work_dir: PathBuf) -> Result<Self, UpdateError> {
        Self::build(manifest_url, work_dir, true, Duration::from_secs(1))
    }

    /// The folder holding the settings file; the update helper writes `update-error.txt`
    /// there. Defaults to the work folder.
    pub fn with_data_dir(mut self, data_dir: PathBuf) -> Self {
        self.data_dir = data_dir;
        self
    }

    /// What to run after an install that needs the app to quit (default: nothing).
    pub fn with_restart_hook(mut self, restart: RestartHook) -> Self {
        self.restart = restart;
        self
    }

    fn build(
        manifest_url: Url,
        work_dir: PathBuf,
        allow_loopback: bool,
        stall_timeout: Duration,
    ) -> Result<Self, UpdateError> {
        validate_url(&manifest_url, allow_loopback)?;
        Ok(Self {
            manifest_url,
            data_dir: work_dir.clone(),
            work_dir,
            // Small, bounded text: a total deadline is right here.
            manifest_client: client(allow_loopback, Some(MANIFEST_DEADLINE))?,
            // Large files: no total limit, the stall timeout below ends dead transfers.
            download_client: client(allow_loopback, None)?,
            allow_loopback,
            stall_timeout,
            restart: Box::new(|| {}),
        })
    }

    pub(super) fn stage_reader(
        &self,
        asset: &Asset,
        mut reader: impl Read,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<StagedFile, UpdateError> {
        create_work_dir(&self.work_dir).map_err(network)?;
        let root = self.work_dir.canonicalize().map_err(network)?;
        // Hash the asset metadata instead of trusting any URL path or digest spelling.
        let name: String = Sha256::digest(asset.sha256.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let extension = if cfg!(target_os = "windows") {
            "exe"
        } else if cfg!(target_os = "linux") {
            "deb"
        } else {
            "zip"
        };
        let path = root.join(format!(
            "{name}-{}-{}.{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed),
            extension
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // create_new refuses an existing file or symlink, without truncating it.
        let file = options.open(&path).map_err(network)?;
        // Declared after `file` is created, dropped first on any exit, even a panic.
        let mut partial = PartialFile {
            path: path.clone(),
            armed: true,
        };
        let mut file = file;
        let mut digest = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = reader.read(&mut buffer).map_err(network)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > asset.size {
                return Err(UpdateError::SizeMismatch {
                    expected: asset.size,
                    actual: size,
                });
            }
            file.write_all(&buffer[..count]).map_err(network)?;
            digest.update(&buffer[..count]);
            progress(size, Some(asset.size));
        }
        if size != asset.size {
            return Err(UpdateError::SizeMismatch {
                expected: asset.size,
                actual: size,
            });
        }
        file.sync_all().map_err(network)?;
        drop(file);
        partial.armed = false;
        Ok(StagedFile {
            path,
            size,
            sha256: digest.finalize().into(),
        })
    }
}

/// Removes a half-written download when it goes out of scope still armed.
struct PartialFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for PartialFile {
    fn drop(&mut self) {
        if self.armed {
            // Best effort: never replaces the error that caused the cleanup.
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn create_work_dir(path: &std::path::Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn client(allow_loopback: bool, total: Option<Duration>) -> Result<Client, UpdateError> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // `None` really disables reqwest's 30 second default total timeout.
        .timeout(total)
        .tcp_keepalive(Duration::from_secs(30))
        .redirect(Policy::custom(move |attempt| {
            if attempt.previous().len() >= 10 {
                attempt.error("too many redirects")
            } else if validate_url(attempt.url(), allow_loopback).is_err()
                || (attempt.url().scheme() == "http"
                    && attempt.previous().iter().any(|url| url.scheme() == "https"))
            {
                attempt.error("insecure redirect refused")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(network)
}

enum Message {
    Ready,
    Data(Vec<u8>),
    End,
    Failed(String),
}

/// Reads a response on a helper thread so a silent connection can be abandoned:
/// each read waits at most `stall` for the next bytes, and there is no total limit.
struct StallReader {
    messages: Receiver<Message>,
    stall: Duration,
    pending: Vec<u8>,
    offset: usize,
}

impl StallReader {
    fn start(client: Client, url: Url, stall: Duration) -> Result<Self, UpdateError> {
        let (sender, messages) = sync_channel(2);
        thread::spawn(move || {
            let response = client.get(url).send().and_then(|r| r.error_for_status());
            let mut response = match response {
                Ok(response) => response,
                Err(error) => {
                    let _ = sender.send(Message::Failed(error.to_string()));
                    return;
                }
            };
            if sender.send(Message::Ready).is_err() {
                return;
            }
            let mut buffer = vec![0_u8; 64 * 1024];
            loop {
                let message = match response.read(&mut buffer) {
                    Ok(0) => Message::End,
                    Ok(count) => Message::Data(buffer[..count].to_vec()),
                    Err(error) => Message::Failed(error.to_string()),
                };
                let last = !matches!(message, Message::Data(_));
                // A closed channel means the caller gave up; stop reading.
                if sender.send(message).is_err() || last {
                    return;
                }
            }
        });
        let reader = Self {
            messages,
            stall,
            pending: Vec::new(),
            offset: 0,
        };
        match reader.messages.recv_timeout(stall) {
            Ok(Message::Ready) => Ok(reader),
            Ok(Message::Failed(error)) => Err(network(error)),
            Ok(_) | Err(RecvTimeoutError::Disconnected) => Err(network("transfer failed")),
            Err(RecvTimeoutError::Timeout) => Err(network("no response from the update server")),
        }
    }
}

impl Read for StallReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Error, ErrorKind};
        if self.offset >= self.pending.len() {
            match self.messages.recv_timeout(self.stall) {
                Ok(Message::Data(data)) => {
                    self.pending = data;
                    self.offset = 0;
                }
                Ok(Message::End | Message::Ready) => return Ok(0),
                Ok(Message::Failed(error)) => return Err(Error::other(error)),
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Error::new(ErrorKind::TimedOut, "download stalled"))
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "transfer ended"))
                }
            }
        }
        let count = buffer.len().min(self.pending.len() - self.offset);
        buffer[..count].copy_from_slice(&self.pending[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

pub(super) fn read_manifest(reader: impl Read) -> Result<String, UpdateError> {
    let mut bytes = Vec::new();
    reader
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(network)?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err(network("manifest exceeds 1 MiB"));
    }
    String::from_utf8(bytes).map_err(network)
}

fn validate_url(url: &Url, allow_loopback: bool) -> Result<(), UpdateError> {
    let local = allow_loopback
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    if url.host_str().is_none()
        || !(url.scheme() == "https" || local)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(network("update URL must use HTTPS without credentials"));
    }
    Ok(())
}

fn network(error: impl std::fmt::Display) -> UpdateError {
    UpdateError::Network(error.to_string())
}

impl UpdateBackend for HttpUpdateBackend {
    fn fetch_manifest(&self) -> Result<String, UpdateError> {
        let response = self
            .manifest_client
            .get(self.manifest_url.clone())
            .send()
            .map_err(network)?
            .error_for_status()
            .map_err(network)?;
        read_manifest(response)
    }
    fn download(
        &self,
        asset: &Asset,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<StagedFile, UpdateError> {
        let url = Url::parse(&asset.url).map_err(network)?;
        validate_url(&url, self.allow_loopback)?;
        let reader = StallReader::start(self.download_client.clone(), url, self.stall_timeout)?;
        self.stage_reader(asset, reader, progress)
    }
    fn install(&self, staged: StagedFile) -> Result<(), UpdateError> {
        match super::install_for_current_os(&staged, &self.data_dir)? {
            // The helper or installer waits for this process to end; the application
            // decides how to quit, so the backend never exits the process itself.
            super::InstallOutcome::RestartScheduled => {
                (self.restart)();
                Ok(())
            }
            super::InstallOutcome::OpenedInstaller(_) => Ok(()),
        }
    }
}
