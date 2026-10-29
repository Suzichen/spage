//! Exercise sync with local files and a loopback-only S3 stub.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use spage_engine::media_sync::{sync_media_with_context, S3Credentials, SyncConfig, SyncContext};

const INDEX: &str = "generated/albums-index.json";

#[derive(Default)]
struct Store {
    objects: HashMap<String, Vec<u8>>,
    writes: Vec<String>,
    fail: Option<(String, String)>,
}

struct Server {
    endpoint: String,
    store: Arc<Mutex<Store>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let store = Arc::new(Mutex::new(Store::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (state, done) = (store.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                socket.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let request: Vec<_> = line.split_whitespace().collect();
                let method = request[0].to_string();
                let key = request[1].strip_prefix("/b/").unwrap().to_string();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" { break; }
                    if let Some((name, value)) = header.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            length = value.trim().parse::<usize>().unwrap();
                        }
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let mut store = state.lock().unwrap();
                let (status, response) = if store.fail.as_ref() == Some(&(method.clone(), key.clone())) {
                    (403, Vec::new())
                } else if method == "PUT" {
                    store.writes.push(key.clone());
                    store.objects.insert(key, body);
                    (200, Vec::new())
                } else {
                    assert_eq!(method, "GET");
                    match store.objects.get(&key) {
                        Some(body) => (200, body.clone()),
                        None => (404, Vec::new()),
                    }
                };
                drop(store);
                write!(socket, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
                socket.write_all(&response).unwrap();
            }
        });
        Self { endpoint, store, stop, worker: Some(worker) }
    }

    fn seed(&self) {
        let mut store = self.store.lock().unwrap();
        store.objects.insert(INDEX.into(), json!([
            { "dirname": "old", "name": "Old", "cover": null, "photoCount": 1, "custom": true }
        ]).to_string().into_bytes());
        for key in ["albums/old/photo.jpg", "albums/old/thumbs/photo.webp", "generated/album-old.json"] {
            store.objects.insert(key.into(), b"original remote bytes".to_vec());
        }
    }

    fn index(&self) -> Vec<Value> {
        serde_json::from_slice(&self.store.lock().unwrap().objects[INDEX]).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn configure(tmp: &tempfile::TempDir, server: &Server, enabled: bool, albums: Value) {
    std::fs::write(tmp.path().join("album.config.json"), json!({
        "enabled": enabled, "albums": albums,
        "provider": { "type": "s3", "endpoint": server.endpoint,
            "region": "auto", "bucket": "b", "publicUrl": "https://example.invalid" }
    }).to_string()).unwrap();
}

fn project(server: &Server) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let album = tmp.path().join("albums/test");
    std::fs::create_dir_all(&album).unwrap();
    image::RgbImage::new(4, 4).save(album.join("photo.png")).unwrap();
    configure(&tmp, server, true, json!([{ "dir": "old" }, { "dir": "test" }]));
    tmp
}

fn sync(tmp: &tempfile::TempDir) -> Result<spage_engine::media_sync::SyncResult, spage_engine::error::EngineError> {
    sync_media_with_context(
        SyncConfig { work_dir: tmp.path().into(), ..Default::default() },
        Some(SyncContext {
            on_progress: None,
            credentials: Some(S3Credentials { access_key: "test".into(), secret_key: "test".into() }),
            cancelled: Some(Arc::new(AtomicBool::new(false))),
        }),
    )
}

#[test]
fn config_controls_adding_updating_and_removing_albums() {
    let server = Server::new();
    server.seed();
    let old = server.index()[0].clone();
    let tmp = project(&server);
    // Configured remote-only album survives while a local album is uploaded.
    assert_eq!(sync(&tmp).unwrap().uploaded, 1);
    let index = server.index();
    assert_eq!(index.len(), 2);
    assert_eq!(index[0], old);
    assert_eq!(index[1]["dirname"], "test");
    assert_eq!(server.store.lock().unwrap().writes.last().unwrap(), INDEX);
    for key in ["albums/old/photo.jpg", "albums/old/thumbs/photo.webp", "generated/album-old.json"] {
        assert_eq!(server.store.lock().unwrap().objects[key].as_slice(), b"original remote bytes");
    }

    // Local data and config order remain authoritative for local albums.
    configure(&tmp, &server, true, json!([{ "dir": "test", "name": "Renamed" }, { "dir": "old" }]));
    image::RgbImage::new(4, 4).save(tmp.path().join("albums/test/second.png")).unwrap();
    sync(&tmp).unwrap();
    let index = server.index();
    assert_eq!(index.len(), 2);
    assert_eq!(index[0]["name"], "Renamed");
    assert_eq!(index[0]["photoCount"], 2);
    assert_eq!(index[1], old);

    // Remove the test album from config, leaving its local and remote files.
    let objects = server.store.lock().unwrap().objects.clone();
    configure(&tmp, &server, true, json!([{ "dir": "old" }]));
    sync(&tmp).unwrap();
    assert_eq!(server.index(), vec![old.clone()]);
    for (key, bytes) in objects {
        if key != INDEX { assert_eq!(server.store.lock().unwrap().objects[&key], bytes); }
    }

    // Configuration changes work even without an albums/ directory.
    std::fs::remove_dir_all(tmp.path().join("albums")).unwrap();
    sync(&tmp).unwrap();
    assert_eq!(server.index(), vec![old]);
    configure(&tmp, &server, true, json!([]));
    sync(&tmp).unwrap();
    assert!(server.index().is_empty());
    assert!(server.store.lock().unwrap().objects.contains_key("albums/test/photo.png"));
}

#[test]
fn disabling_albums_still_publishes_an_empty_index() {
    let server = Server::new();
    server.seed();
    let tmp = tempfile::tempdir().unwrap();
    configure(&tmp, &server, false, json!([{ "dir": "old" }]));
    sync(&tmp).unwrap();
    assert!(server.index().is_empty());
}

#[test]
fn first_sync_initializes_index() {
    let server = Server::new();
    sync(&project(&server)).unwrap();
    assert_eq!(server.index().len(), 1);
    assert_eq!(server.index()[0]["dirname"], "test");
}

#[test]
fn unreadable_remote_index_stops_before_uploading() {
    for body in [b"invalid JSON".as_slice(), b"[{}]"] {
        let server = Server::new();
        server.store.lock().unwrap().objects.insert(INDEX.into(), body.to_vec());
        assert!(sync(&project(&server)).is_err());
        assert!(server.store.lock().unwrap().writes.is_empty());
    }
    let server = Server::new();
    server.store.lock().unwrap().fail = Some(("GET".into(), INDEX.into()));
    assert!(sync(&project(&server)).is_err());
    assert!(server.store.lock().unwrap().writes.is_empty());
}

#[test]
fn upload_failure_keeps_index_and_can_retry() {
    for key in ["albums/test/photo.png", "albums/test/thumbs/photo.webp", "generated/album-test.json"] {
        let server = Server::new();
        server.seed();
        let old = server.index();
        let tmp = project(&server);
        server.store.lock().unwrap().fail = Some(("PUT".into(), key.into()));
        assert!(sync(&tmp).is_err());
        assert_eq!(server.index(), old);
        server.store.lock().unwrap().fail = None;
        sync(&tmp).unwrap();
        assert_eq!(server.index().len(), 2);
    }
}
