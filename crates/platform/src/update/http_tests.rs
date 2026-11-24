use super::http::HttpUpdateBackend;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
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
        let path = std::env::temp_dir().join(format!(
            "vaio-http-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for WorkDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn serve(body: Vec<u8>, redirect: Option<&str>) -> Url {
    let server = Server::http("127.0.0.1:0").expect("loopback bind blocked");
    let url = Url::parse(&format!("http://{}/manifest", server.server_addr())).unwrap();
    let redirect = redirect.map(str::to_owned);
    thread::spawn(move || {
        if let Some(request) = server.recv_timeout(Duration::from_secs(5)).unwrap() {
            let mut response = Response::from_data(body);
            if let Some(location) = redirect {
                response = response
                    .with_status_code(StatusCode(302))
                    .with_header(Header::from_bytes("Location", location).unwrap());
            }
            let _ = request.respond(response);
        }
    });
    url
}

fn asset(url: Url, size: u64) -> Asset {
    Asset {
        url: url.to_string(),
        size,
        sha256: "../../escape\\bad".into(),
        signature: String::new(),
    }
}

#[test]
fn http_backend_rejects_non_https_url() {
    let dir = WorkDir::new();
    for url in [
        "http://example.com/latest.json",
        "http://127.0.0.1/latest.json",
        "file:///tmp/manifest",
    ] {
        assert!(HttpUpdateBackend::new(Url::parse(url).unwrap(), dir.0.clone()).is_err());
    }
}

#[test]
fn download_writes_to_work_dir_only() {
    let dir = WorkDir::new();
    let mut url = serve(b"abc".to_vec(), None);
    url.set_path("/%2e%2e%2f..%5cescape.exe");
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    let mut progress = Vec::new();
    let staged = backend
        .download(&asset(url, 3), &mut |done, total| {
            progress.push((done, total))
        })
        .unwrap();
    assert_eq!(
        staged.path.parent(),
        Some(dir.0.canonicalize().unwrap().as_path())
    );
    assert_eq!(fs::read(&staged.path).unwrap(), b"abc");
    assert_eq!(staged.size, 3);
    assert_eq!(
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        staged
            .sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(progress.last(), Some(&(3, Some(3))));
}

#[test]
fn truncated_download_is_error() {
    let dir = WorkDir::new();
    let url = serve(b"short".to_vec(), None);
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    assert!(matches!(
        backend.download(&asset(url, 20), &mut |_, _| {}),
        Err(UpdateError::SizeMismatch { .. } | UpdateError::Network(_))
    ));
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn oversized_download_removes_partial_file() {
    let dir = WorkDir::new();
    let url = serve(b"too much".to_vec(), None);
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    assert!(matches!(
        backend.download(&asset(url, 2), &mut |_, _| {}),
        Err(UpdateError::SizeMismatch { .. } | UpdateError::Network(_))
    ));
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn manifest_body_is_capped_and_valid_utf8() {
    let dir = WorkDir::new();
    for body in [vec![b'a'; 1_048_577], vec![255]] {
        let backend = HttpUpdateBackend::for_tests(serve(body, None), dir.0.clone()).unwrap();
        assert!(backend.fetch_manifest().is_err());
    }
}

#[test]
fn fetch_manifest_returns_text() {
    let dir = WorkDir::new();
    let backend =
        HttpUpdateBackend::for_tests(serve(b"{\"schema\":1}".to_vec(), None), dir.0.clone())
            .unwrap();
    assert_eq!(backend.fetch_manifest().unwrap(), "{\"schema\":1}");
}

#[test]
fn redirects_cannot_escape_the_transport_policy() {
    let dir = WorkDir::new();
    let url = serve(Vec::new(), Some("http://example.com/insecure"));
    let backend = HttpUpdateBackend::for_tests(url, dir.0.clone()).unwrap();
    assert!(backend.fetch_manifest().is_err());
}

#[test]
fn stream_hashes_bytes_and_contains_paths() {
    let dir = WorkDir::new();
    let url = Url::parse("http://127.0.0.1/%2f..%5cfile").unwrap();
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    let mut progress = Vec::new();
    let staged = backend
        .stage_reader(&asset(url, 3), &b"abc"[..], &mut |done, total| {
            progress.push((done, total))
        })
        .unwrap();
    assert_eq!(
        staged.path.parent(),
        Some(dir.0.canonicalize().unwrap().as_path())
    );
    assert_eq!(fs::read(&staged.path).unwrap(), b"abc");
    assert_eq!(staged.size, 3);
    assert_eq!(
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        staged
            .sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    assert_eq!(progress.last(), Some(&(3, Some(3))));
}

#[test]
fn stream_size_errors_remove_partial_files() {
    let dir = WorkDir::new();
    let url = Url::parse("http://127.0.0.1/file").unwrap();
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    for size in [2, 20] {
        assert!(matches!(
            backend.stage_reader(&asset(url.clone(), size), &b"short"[..], &mut |_, _| {}),
            Err(UpdateError::SizeMismatch { .. })
        ));
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[test]
fn stream_read_failure_removes_partial_file() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("truncated transport"))
        }
    }
    let dir = WorkDir::new();
    let url = Url::parse("http://127.0.0.1/file").unwrap();
    let backend = HttpUpdateBackend::for_tests(url.clone(), dir.0.clone()).unwrap();
    assert!(matches!(
        backend.stage_reader(&asset(url, 20), Broken, &mut |_, _| {}),
        Err(UpdateError::Network(_))
    ));
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn manifest_reader_enforces_limit_and_utf8() {
    use super::http::read_manifest;
    assert_eq!(read_manifest(&b"{}"[..]).unwrap(), "{}");
    assert!(read_manifest(&vec![b'a'; 1_048_577][..]).is_err());
    assert!(read_manifest(&[255][..]).is_err());
}
