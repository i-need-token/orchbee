use super::*;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, atomic::Ordering as Order},
    thread,
};
pub(crate) struct Root(pub(crate) PathBuf);
impl Root {
    pub(crate) fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = Self(std::env::temp_dir().join(format!(
            "orchbee-agent-updates-{:032x}",
            u128::from_le_bytes(nonce)
        )));
        fs::create_dir(&root.0).unwrap();
        root
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-agent-updates-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
pub(crate) fn rules(provider: &str, version: &str) -> String {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/resources/agent-detection/codex.toml"
    ))
    .replacen("id = \"codex\"", &format!("id = \"{provider}\""), 1)
    .replace(
        "version = \"2026.09.14.1\"",
        &format!("version = \"{version}\""),
    )
}
/// Scripted fetches and pre-fetch cancellation never contact this reserved example host.
const CATALOG: &str = "https://example.com/agent-detection/index.toml";
pub(crate) const INDEX: &str = "schema_version=1\n[[agents]]\nid='codex'\npath='codex.toml'\n";
fn update(root: &Root, source: &CatalogUrl, content: &str) -> UpdateReport {
    let stop = AtomicBool::new(false);
    run_with_fetch(
        &root.0,
        source,
        &stop,
        Instant::now() + Duration::from_secs(10),
        |url, _, _| {
            Ok(if url.0.ends_with("index.toml") {
                INDEX.into()
            } else {
                content.into()
            })
        },
    )
}
fn scripted(root: &Root, content: &str) -> UpdateReport {
    update(root, &CatalogUrl::parse(CATALOG).unwrap(), content)
}
/// A leaf in the directory of the catalog that `scripted` updates.
fn cached(root: &Root, name: &str) -> PathBuf {
    remote_root(&root.0, &CatalogUrl::parse(CATALOG).unwrap()).join(name)
}
/// The cached Codex version that a Session configured with `source` selects, if any.
fn selected(root: &Root, source: &CatalogUrl) -> Option<String> {
    let cache = remote_cache(Some(&root.0), &Ok(Some(source.clone())));
    let codex = ScreenDetector::load(Some(&root.0), cache)
        .unwrap()
        .summaries()
        .into_iter()
        .find(|m| m.agent == "codex")
        .unwrap();
    (codex.manifest.source == orchbee_workbench_protocol::AgentManifestSource::RemoteCache)
        .then_some(codex.version)
        .flatten()
}
#[test]
fn cache_updates_preserve_versions_bytes_partial_outcomes_and_corrupt_inputs() {
    let root = Root::new();
    let original = rules("codex", "1");
    let first = scripted(&root, &original);
    assert!(first.status_saved);
    assert_eq!(first.agents[0].result, Disposition::Updated);
    let current = scripted(&root, &original);
    assert_eq!(current.agents[0].result, Disposition::Current);
    let changed = scripted(&root, &(original.clone() + "\n# changed bytes\n"));
    assert_eq!(changed.agents[0].error, Some(Failure::ChangedSameVersion));
    let zero_tail = scripted(&root, &rules("codex", "1.0"));
    assert_eq!(zero_tail.agents[0].error, Some(Failure::ChangedSameVersion));
    let older = scripted(&root, &rules("codex", "0.9"));
    assert_eq!(older.agents[0].error, Some(Failure::OlderVersion));
    assert_eq!(
        fs::read_to_string(cached(&root, "codex.toml")).unwrap(),
        original
    );
    let stop = AtomicBool::new(false);
    let partial = run_with_fetch(
        &root.0,
        &CatalogUrl::parse(CATALOG).unwrap(),
        &stop,
        Instant::now() + Duration::from_secs(10),
        |url, _, _| {
            Ok(if url.0.ends_with("index.toml") {
                format!("{INDEX}[[agents]]\nid='kimi'\npath='kimi.toml'\n")
            } else if url.0.ends_with("codex.toml") {
                rules("codex", "2")
            } else {
                "bad".into()
            })
        },
    );
    assert_eq!(partial.agents[0].result, Disposition::Updated);
    assert_eq!(partial.agents[1].error, Some(Failure::InvalidManifest));
    assert!(partial.status_saved);
    let path = cached(&root, "codex.toml");
    fs::write(&path, b"corrupt").unwrap();
    let rejected = scripted(&root, &rules("codex", "3"));
    assert_eq!(rejected.agents[0].error, Some(Failure::Cache));
    assert_eq!(fs::read(&path).unwrap(), b"corrupt");
    let status = cached(&root, "status.json");
    fs::write(&status, b"corrupt status").unwrap();
    let rejected = scripted(&root, &rules("codex", "3"));
    assert_eq!(rejected.failure, Some(Failure::Cache));
    assert!(!rejected.status_saved);
    assert_eq!(fs::read(status).unwrap(), b"corrupt status");
}
#[test]
fn each_catalog_selects_only_rules_its_own_updates_cached() {
    let root = Root::new();
    let first = CatalogUrl::parse(CATALOG).unwrap();
    let other = CatalogUrl::parse("https://example.com/other/index.toml").unwrap();
    assert_eq!(
        scripted(&root, &rules("codex", "2099.1")).agents[0].result,
        Disposition::Updated
    );
    assert_eq!(selected(&root, &first).as_deref(), Some("2099.1"));
    assert_eq!(selected(&root, &other), None);
    let foreign = maintenance(Some(&root.0), &Ok(Some(other.clone())));
    assert_eq!((foreign.last_update, foreign.status_error), (None, None));
    // Read and write sharing without delete sharing refuses replacing a held status.
    let hold = |source: &CatalogUrl| {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(remote_root(&root.0, source).join("status.json"))
            .unwrap()
    };
    // Switching catalogs while the first catalog's status cannot be replaced, and then failing to
    // publish the new catalog's status after its rules were cached, never pairs one catalog's
    // status with the other's rules.
    let held = hold(&first);
    assert!(update(&root, &other, &rules("codex", "2099.2")).status_saved);
    drop(held);
    assert_eq!(selected(&root, &first).as_deref(), Some("2099.1"));
    assert_eq!(selected(&root, &other).as_deref(), Some("2099.2"));
    let held = hold(&other);
    let unpublished = update(&root, &other, &rules("codex", "2099.3"));
    drop(held);
    assert_eq!(
        (unpublished.failure, unpublished.status_saved),
        (Some(Failure::Cache), false)
    );
    assert_eq!(selected(&root, &first).as_deref(), Some("2099.1"));
    assert_eq!(selected(&root, &other).as_deref(), Some("2099.3"));
    // A status in a catalog's directory that names another catalog admits nothing. Even a failed
    // catalog fetch removes it and then the rules before recording its own.
    fs::copy(
        remote_root(&root.0, &other).join("status.json"),
        cached(&root, "status.json"),
    )
    .unwrap();
    assert_eq!(selected(&root, &first), None);
    let report = run_with_fetch(
        &root.0,
        &first,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(10),
        |_, _, _| Err(Failure::InvalidCatalog),
    );
    assert_eq!(report.failure, Some(Failure::InvalidCatalog));
    assert!(report.status_saved);
    assert!(!cached(&root, "codex.toml").exists());
    // Rules without a recorded catalog are never admitted.
    fs::remove_file(remote_root(&root.0, &other).join("status.json")).unwrap();
    assert_eq!(selected(&root, &other), None);
}
#[test]
fn catalog_and_cancellation_fail_before_any_manifest_commit() {
    let source = CatalogUrl::parse(CATALOG).unwrap();
    for path in [
        "/absolute",
        "../other",
        "a/../b",
        "a\\b",
        "https://evil/file",
        "%2e%2e/file",
        "a//b",
        "a#fragment",
    ] {
        assert!(
            catalog(&INDEX.replace("codex.toml", path), &source).is_err(),
            "{path}"
        );
    }
    assert!(
        catalog(
            &(INDEX.to_owned() + "[[agents]]\nid='codex'\npath='other.toml'\n"),
            &source
        )
        .is_err()
    );
    assert!(
        catalog(
            &INDEX.replace("schema_version=1", "schema_version=2"),
            &source
        )
        .is_err()
    );
    assert!(
        catalog(
            &INDEX.replace("id='codex'", "id='unrecognized-future-provider'"),
            &source
        )
        .unwrap()
        .is_empty()
    );
    for url in [
        "file:///file",
        "https://user:secret@host/index.toml",
        "https://host/index.toml#fragment",
        "https://host/index.toml?token=secret",
        "https://host/ bad",
    ] {
        assert!(CatalogUrl::parse(url).is_err());
    }
    let root = Root::new();
    let report = run(&root.0, &source, &AtomicBool::new(true));
    assert_eq!(report.failure, Some(Failure::Cancelled));
    assert!(!root.0.join("agent-detection").exists());
}
pub(crate) struct Http {
    pub(crate) url: CatalogUrl,
    stop: Arc<AtomicBool>,
    pub(crate) ready: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Http {
    pub(crate) fn start(responses: Vec<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = CatalogUrl::parse(&format!(
            "http://{}/index.toml",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let ready = Arc::new(AtomicBool::new(false));
        let accepted = Arc::clone(&ready);
        let handle = thread::spawn(move || {
            for (index, body) in responses.into_iter().enumerate() {
                let until = Instant::now() + Duration::from_secs(10);
                let expected = if index % 2 == 0 {
                    "GET /index.toml HTTP/1.1"
                } else {
                    "GET /codex.toml HTTP/1.1"
                };
                let mut stream = loop {
                    if stopping.load(Order::Acquire) || Instant::now() >= until {
                        return;
                    }
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(e) => panic!("{e}"),
                    };
                    let budget = until
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_secs(2));
                    if budget.is_zero() {
                        return;
                    }
                    // Windows accepts inherit the listener's nonblocking mode.
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(budget)).unwrap();
                    stream.set_write_timeout(Some(budget)).unwrap();
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") && request.len() < 4096 {
                        let mut b = [0];
                        if stream.read_exact(&mut b).is_err() {
                            break;
                        }
                        request.push(b[0]);
                    }
                    // A closed probe or a retry for another path cannot consume this response.
                    if !request.ends_with(b"\r\n\r\n") {
                        continue;
                    }
                    if String::from_utf8_lossy(&request).lines().next() != Some(expected) {
                        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        continue;
                    }
                    break stream;
                };
                accepted.store(true, Order::Release);
                if body.is_empty() {
                    while !stopping.load(Order::Acquire) && Instant::now() < until {
                        thread::sleep(Duration::from_millis(5));
                    }
                    return;
                }
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream
                    .write_all(header.as_bytes())
                    .and_then(|_| stream.write_all(&body));
            }
        });
        Self {
            url,
            stop,
            ready,
            handle: Some(handle),
        }
    }
}
impl Drop for Http {
    fn drop(&mut self) {
        self.stop.store(true, Order::Release);
        self.handle.take().unwrap().join().unwrap();
    }
}
#[test]
fn http_update_peer_preserves_catalog_after_an_incomplete_connection() {
    let server = Http::start(vec![
        INDEX.as_bytes().to_vec(),
        rules("codex", "2099.1").into_bytes(),
    ]);
    let address = server
        .url
        .0
        .strip_prefix("http://")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    drop(std::net::TcpStream::connect(address).unwrap());
    let text = fetch::text(
        &server.url,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(text, INDEX);
}

#[test]
fn http_update_peer_waits_for_delayed_and_fragmented_request_headers() {
    let server = Http::start(vec![INDEX.as_bytes().to_vec()]);
    let address = server
        .url
        .0
        .strip_prefix("http://")
        .unwrap()
        .split('/')
        .next()
        .unwrap();
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    for partial in [
        b"".as_slice(),
        b"GET /index.toml HTTP/1.1\r\nHost: localhost\r\n",
    ] {
        stream.write_all(partial).unwrap();
        let waiting = stream.peek(&mut [0]);
        assert!(
            matches!(&waiting, Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)),
            "the peer must keep an incomplete request open: {waiting:?}"
        );
    }
    stream.write_all(b"\r\n").unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.ends_with(INDEX), "{response}");
}

#[test]
fn real_inbox_curl_commits_validated_rules_and_rejects_oversized_and_invalid_utf8() {
    let root = Root::new();
    let remote = rules("codex", "2099.1");
    let server = Http::start(vec![INDEX.as_bytes().to_vec(), remote.as_bytes().to_vec()]);
    let report = run(&root.0, &server.url, &AtomicBool::new(false));
    assert_eq!(report.failure, None, "{report:?}");
    assert!(report.status_saved);
    assert_eq!(report.agents[0].result, Disposition::Updated);
    let active = ScreenDetector::load(
        Some(&root.0),
        remote_cache(Some(&root.0), &Ok(Some(server.url.clone()))),
    )
    .unwrap();
    let codex = active
        .summaries()
        .into_iter()
        .find(|m| m.agent == "codex")
        .unwrap();
    assert_eq!(codex.version.as_deref(), Some("2099.1"));
    assert_eq!(
        codex.manifest.source,
        orchbee_workbench_protocol::AgentManifestSource::RemoteCache
    );
    drop(server);
    let oversized = Http::start(vec![vec![b'x'; LIMIT + 1]]);
    assert_eq!(
        fetch::text(
            &oversized.url,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5)
        ),
        Err(Failure::TooLarge)
    );
    drop(oversized);
    let invalid = Http::start(vec![vec![0xff]]);
    assert_eq!(
        fetch::text(
            &invalid.url,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5)
        ),
        Err(Failure::InvalidUtf8)
    );
}

#[test]
fn cancelling_a_live_curl_fetch_settles_its_job_and_pipe_reader() {
    let server = Http::start(vec![vec![]]);
    let stop = Arc::new(AtomicBool::new(false));
    let cancelling = Arc::clone(&stop);
    let source = server.url.clone();
    let work = thread::spawn(move || {
        fetch::text(
            &source,
            &cancelling,
            Instant::now() + Duration::from_secs(10),
        )
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !server.ready.load(Order::Acquire) && !work.is_finished() && Instant::now() < until {
        thread::sleep(Duration::from_millis(5));
    }
    let admitted = server.ready.load(Order::Acquire);
    let started = Instant::now();
    stop.store(true, Order::Release);
    let result = work.join().unwrap();
    assert!(admitted, "fetch never reached the HTTP peer: {result:?}");
    assert_eq!(result, Err(Failure::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(3));
}
