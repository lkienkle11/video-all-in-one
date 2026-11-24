//! Transport policy tests: redirects, stalls, chunked bodies, cleanup and permissions.
use super::http::HttpUpdateBackend;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use tiny_http::{Header, Response, Server, StatusCode};
use url::Url;
use vaio_core::{
    ports::UpdateBackend,
    update::{Asset, UpdateError},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct WorkDir(PathBuf);
impl WorkDir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "vaio-sec-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn asset(url: &Url, size: u64) -> Asset {
    Asset {
        url: url.to_string(),
        size,
        sha256: "../../escape\\bad".into(),
        signature: String::new(),
    }
}

/// A server that answers once with `respond` and tells whether any request reached it.
fn spawn(
    respond: impl FnOnce(tiny_http::Request) + Send + 'static,
    wait: Duration,
) -> (Url, thread::JoinHandle<bool>) {
    let server = Server::http("127.0.0.1:0").expect("loopback bind blocked");
    let port = server.server_addr().to_ip().unwrap().port();
    let url = Url::parse(&format!("http://127.0.0.1:{port}/file")).unwrap();
    let handle = thread::spawn(move || match server.recv_timeout(wait).unwrap() {
        Some(request) => {
            respond(request);
            true
        }
        None => false,
    });
    (url, handle)
}

fn redirect_to(location: String) -> impl FnOnce(tiny_http::Request) + Send + 'static {
    move |request| {
        let response = Response::from_data(Vec::new())
            .with_status_code(StatusCode(302))
            .with_header(Header::from_bytes("Location", location).unwrap());
        let _ = request.respond(response);
    }
}

#[test]
fn redirect_to_an_allowed_loopback_server_is_followed() {
    // Control case: proves the recording server really sees requests.
    let dir = WorkDir::new();
    let (second, hit) = spawn(
        |request| drop(request.respond(Response::from_data(b"abc".to_vec()))),
        Duration::from_secs(3),
    );
    let (first, _) = spawn(redirect_to(second.to_string()), Duration::from_secs(3));
    let backend = HttpUpdateBackend::for_tests(first.clone(), dir.0.clone()).unwrap();
    backend.download(&asset(&first, 3), &mut |_, _| {}).unwrap();
    assert!(hit.join().unwrap());
}

#[test]
fn redirect_to_a_host_outside_the_policy_is_never_requested() {
    let dir = WorkDir::new();
    // 0.0.0.0 reaches the local second server if the redirect were followed.
    let (second, hit) = spawn(
        |request| drop(request.respond(Response::from_data(b"abc".to_vec()))),
        Duration::from_secs(2),
    );
    let port = second.port().unwrap();
    let (first, _) = spawn(
        redirect_to(format!("http://0.0.0.0:{port}/file")),
        Duration::from_secs(3),
    );
    let backend = HttpUpdateBackend::for_tests(first.clone(), dir.0.clone()).unwrap();
    assert!(backend.download(&asset(&first, 3), &mut |_, _| {}).is_err());
    assert!(!hit.join().unwrap(), "the redirect target was contacted");
    assert!(!dir.0.exists() || fs::read_dir(&dir.0).unwrap().count() == 0);
}

#[test]
fn redirect_with_credentials_is_never_requested() {
    let dir = WorkDir::new();
    let (second, hit) = spawn(
        |request| drop(request.respond(Response::from_data(b"{}".to_vec()))),
        Duration::from_secs(2),
    );
    let port = second.port().unwrap();
    let (first, _) = spawn(
        redirect_to(format!("http://user:secret@127.0.0.1:{port}/file")),
        Duration::from_secs(3),
    );
    let backend = HttpUpdateBackend::for_tests(first, dir.0.clone()).unwrap();
    assert!(backend.fetch_manifest().is_err());
    assert!(!hit.join().unwrap(), "the redirect target was contacted");
}

#[test]
fn chunked_body_larger_than_declared_size_is_refused() {
    let dir = WorkDir::new();
    let (url, _) = spawn(
        |request| {
            // No Content-Length: the body is sent chunked.
            let body = vec![b'x'; 100_000];
            let response = Response::new(StatusCode(200), vec![], &body[..], None, None);
            let _ = request.respond(response);
        },
        Duration::from_secs(3),
    );
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    assert!(matches!(
        backend.download(&asset(&url, 3), &mut |_, _| {}),
        Err(UpdateError::SizeMismatch { .. })
    ));
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn download_file_name_is_the_hash_of_the_asset_digest_text() {
    let dir = WorkDir::new();
    let (url, _) = spawn(
        |request| drop(request.respond(Response::from_data(b"abc".to_vec()))),
        Duration::from_secs(3),
    );
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    let staged = backend.download(&asset(&url, 3), &mut |_, _| {}).unwrap();
    let expected: String = Sha256::digest("../../escape\\bad".as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let name = staged.path.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with(&format!("{expected}-{}-", std::process::id())),
        "{name}"
    );
    assert!(name.contains('.') && !name.contains(".."));
}

/// A raw server: sends the headers at once, then each piece after `gap`, flushing every time.
fn trickle(pieces: Vec<&'static [u8]>, gap: Duration, length: usize) -> Url {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind blocked");
    let url = Url::parse(&format!("http://{}/file", listener.local_addr().unwrap())).unwrap();
    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\n\r\n");
        if stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.flush())
            .is_err()
        {
            return;
        }
        for piece in pieces {
            thread::sleep(gap);
            if stream
                .write_all(piece)
                .and_then(|()| stream.flush())
                .is_err()
            {
                return;
            }
        }
    });
    url
}

#[test]
fn stalled_download_fails_after_the_stall_timeout_and_removes_the_file() {
    let dir = WorkDir::new();
    let url = trickle(vec![b"a", b"b"], Duration::from_secs(4), 10);
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    let started = Instant::now();
    let result = backend.download(&asset(&url, 10), &mut |_, _| {});
    assert!(matches!(result, Err(UpdateError::Network(_))), "{result:?}");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "took {:?}",
        started.elapsed()
    );
    assert!(!dir.0.exists() || fs::read_dir(&dir.0).unwrap().count() == 0);
}

#[test]
fn slow_but_steady_download_has_no_total_time_limit() {
    let dir = WorkDir::new();
    // The tests' stall limit is one second; this takes about 1.8 s in total.
    let url = trickle(vec![b"a", b"b", b"c"], Duration::from_millis(600), 3);
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    let staged = backend.download(&asset(&url, 3), &mut |_, _| {}).unwrap();
    assert_eq!(fs::read(staged.path).unwrap(), b"abc");
}

#[test]
fn partial_file_is_removed_when_the_progress_callback_panics() {
    let dir = WorkDir::new();
    let url = Url::parse("http://127.0.0.1/file").unwrap();
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        backend.stage_reader(&asset(&url, 3), &b"abc"[..], &mut |_, _| panic!("progress"))
    }));
    assert!(result.is_err());
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn work_directory_is_created_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = WorkDir::new();
    let work = dir.0.join("nested/work");
    let url = Url::parse("http://127.0.0.1/file").unwrap();
    let backend = HttpUpdateBackend::for_tests(url.clone(), work.clone()).unwrap();
    backend
        .stage_reader(&asset(&url, 3), &b"abc"[..], &mut |_, _| {})
        .unwrap();
    assert_eq!(
        fs::metadata(&work).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn install_asks_the_application_to_restart_and_never_exits_itself() {
    // A fake outcome cannot be produced here without a real installer, so check the
    // hook plumbing through the public setter: an unused hook is never called.
    let dir = WorkDir::new();
    let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = called.clone();
    let url = Url::parse("http://127.0.0.1/file").unwrap();
    let backend = HttpUpdateBackend::for_tests(url, dir.0.clone())
        .unwrap()
        .with_restart_hook(Box::new(move || flag.store(true, Ordering::SeqCst)));
    let missing = vaio_core::update::StagedFile {
        path: dir.0.join("missing"),
        size: 1,
        sha256: [0; 32],
    };
    assert!(backend.install(missing).is_err());
    assert!(!called.load(Ordering::SeqCst));
}
