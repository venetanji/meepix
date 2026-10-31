use easel_client::Client;
use meepix_app::{App, ToolRequest};
use sprite_core::{EditCommand, Point};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    png: Vec<u8>,
}
impl Fake {
    fn new() -> Self {
        let mut png = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png, 16, 16);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().unwrap();
            let mut pixels = vec![0; 16 * 16 * 4];
            for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
                p.copy_from_slice(&[i as u8, 20, 30, 128]);
            }
            w.write_image_data(&pixels).unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let origin = url.clone();
        let image = png.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let task = thread::spawn(move || {
            let mut job = 0;
            let mut polls = std::collections::HashMap::<String, usize>::new();
            while !stopped.load(Ordering::Relaxed) {
                let (mut s, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 8192];
                loop {
                    let n = s.read(&mut buf).unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                        let len = headers
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&request);
                assert!(req
                    .to_lowercase()
                    .contains("authorization: bearer fake-token"));
                let first = req.lines().next().unwrap();
                let (status, body) = if first.starts_with("GET /v1/models ") {
                    ("200 OK", br#"{"data":[{"id":"qwen-image-2.1"},{"id":"video-model"},{"id":"audio-model"}]}"#.to_vec())
                } else if first.starts_with("POST /v1/images/generations ") {
                    assert!(req.to_lowercase().contains("prefer: respond-async"));
                    assert!(req.contains("512"));
                    job += 1;
                    (
                        "202 Accepted",
                        format!("{{\"id\":\"job_{job}\",\"status\":\"queued\"}}").into_bytes(),
                    )
                } else if first.contains("/content/0 ") {
                    ("200 OK", image.clone())
                } else {
                    let path = first.split_whitespace().nth(1).unwrap();
                    let id = path.rsplit('/').next().unwrap();
                    let count = polls.entry(id.into()).or_default();
                    *count += 1;
                    let body = if *count == 1 {
                        format!("{{\"id\":\"{id}\",\"status\":\"in_progress\"}}")
                    } else {
                        format!("{{\"id\":\"{id}\",\"status\":\"completed\",\"data\":[{{\"url\":\"{origin}/v1/images/jobs/{id}/content/0\"}}]}}")
                    };
                    ("200 OK", body.into_bytes())
                };
                write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                s.write_all(&body).unwrap();
            }
        });
        Self {
            url,
            stop,
            thread: Some(task),
            png,
        }
    }
    fn app(&self, dir: &std::path::Path) -> App {
        App::new(
            dir.into(),
            Some(Client::new(&self.url, Some("fake-token".into())).unwrap()),
        )
        .unwrap()
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn completed(app: &App, revision: u64) -> meepix_app::Generation {
    let g = app
        .submit(revision, "model", "pixel bird", "generate")
        .unwrap();
    assert_eq!(g.status, "queued");
    assert_eq!(app.poll(&g.id).unwrap().status, "in_progress");
    let g = app.poll(&g.id).unwrap();
    assert_eq!(g.status, "completed");
    g
}
#[test]
fn fake_flow_persists_preview_rejects_and_accepts_undoably() {
    let fake = Fake::new();
    let dir = tempfile::tempdir().unwrap();
    let app = fake.app(dir.path());
    assert_eq!(app.models().unwrap().models, vec!["qwen-image-2.1"]);
    let s = app.snapshot().unwrap();
    let s = app
        .command(
            s.revision,
            EditCommand::Paint {
                layer_id: s.active_layer_id.clone(),
                frame_id: s.active_frame_id.clone(),
                points: vec![Point { x: 0, y: 0 }],
                color: [3, 4, 5, 128],
            },
        )
        .unwrap();
    let before = s.pixels.clone();
    let g = completed(&app, s.revision);
    let candidate = g.pixels.clone().unwrap();
    assert_eq!(candidate.len(), 64 * 64 * 4);
    assert_eq!(&candidate[..4], &[0, 20, 30, 128]);
    assert_eq!(&candidate[4 * 4..4 * 5], &[1, 20, 30, 128]);
    let folder = dir.path().join("candidates").join(&g.id);
    assert_eq!(
        std::fs::read(folder.join("original-0.png")).unwrap(),
        fake.png
    );
    for file in ["receipt.json", "provenance.json", "preview.png"] {
        assert!(folder.join(file).is_file());
    }
    assert_eq!(app.poll(&g.id).unwrap().pixels.unwrap(), candidate);
    assert_eq!(app.snapshot().unwrap().pixels, before);
    app.reject(&g.id).unwrap();
    assert_eq!(app.snapshot().unwrap().pixels, before);
    assert!(app.accept(&g.id, s.revision).is_err());
    let g = completed(&app, s.revision);
    let accepted = app.accept(&g.id, s.revision).unwrap();
    assert_eq!(accepted.revision, s.revision + 1);
    assert_eq!(accepted.pixels, g.pixels.unwrap());
    let undone = app
        .command(accepted.revision, EditCommand::Undo {})
        .unwrap();
    assert_eq!(undone.pixels, before);
}
#[test]
fn stale_revision_new_and_load_epoch_refuse_accept() {
    let fake = Fake::new();
    let dir = tempfile::tempdir().unwrap();
    let app = fake.app(dir.path());
    let s = app.snapshot().unwrap();
    app.save("same").unwrap();
    let g = completed(&app, s.revision);
    let changed = app.command(s.revision, EditCommand::AddFrame {}).unwrap();
    assert!(app.accept(&g.id, changed.revision).is_err());
    let g = completed(&app, changed.revision);
    let fresh = app.create(64, 64).unwrap();
    assert!(app.accept(&g.id, fresh.revision).is_err());
    let s = app.load("same").unwrap();
    let g = completed(&app, s.revision);
    let loaded = app.load("same").unwrap();
    assert_eq!(loaded.document_id, s.document_id);
    assert_eq!(loaded.revision, s.revision);
    assert!(app.accept(&g.id, loaded.revision).is_err());
}
#[test]
fn selection_change_refuses_accept() {
    let fake = Fake::new();
    let dir = tempfile::tempdir().unwrap();
    let app = fake.app(dir.path());
    let s = app.snapshot().unwrap();
    let first = s.active_frame_id.clone();
    let s = app.command(s.revision, EditCommand::AddFrame {}).unwrap();
    let g = completed(&app, s.revision);
    let selected = app
        .command(s.revision, EditCommand::SelectFrame { frame_id: first })
        .unwrap();
    assert_eq!(selected.revision, s.revision);
    assert!(
        app.accept(&g.id, selected.revision).is_err(),
        "accept must reject changed frame selection"
    );
}
#[test]
fn tool_request_cannot_supply_approval() {
    let request =
        r#"{"version":1,"expected_revision":0,"approved":true,"action":{"type":"add_frame"}}"#;
    assert!(serde_json::from_str::<ToolRequest>(request).is_err());
}

#[test]
fn layer_selection_change_refuses_accept() {
    let fake = Fake::new();
    let dir = tempfile::tempdir().unwrap();
    let app = fake.app(dir.path());
    let s = app.snapshot().unwrap();
    let first = s.active_layer_id.clone();
    let s = app
        .command(
            s.revision,
            EditCommand::AddLayer {
                name: "Second".into(),
            },
        )
        .unwrap();
    let g = completed(&app, s.revision);
    let selected = app
        .command(s.revision, EditCommand::SelectLayer { layer_id: first })
        .unwrap();
    assert_eq!(selected.revision, s.revision);
    assert!(
        app.accept(&g.id, selected.revision).is_err(),
        "accept must reject changed layer selection"
    );
}

#[test]
fn accepted_job_remains_recoverable_when_candidate_directory_creation_fails() {
    let fake = Fake::new();
    let dir = tempfile::tempdir().unwrap();
    let app = fake.app(dir.path());
    let candidate = dir.path().join("candidates/job_1");
    std::fs::write(&candidate, b"block directory").unwrap();
    let accepted = app
        .submit(0, "qwen-image-2.1", "pixel bird", "generate")
        .unwrap();
    assert_eq!(accepted.id, "job_1");
    assert_eq!(accepted.status, "queued");
    let err = accepted.error.expect("storage warning");
    assert!(err.contains("job_1"), "missing accepted job ID: {err}");
    assert!(
        err.to_lowercase().contains("do not resubmit"),
        "missing retry warning: {err}"
    );
    std::fs::remove_file(&candidate).unwrap();
    std::fs::create_dir(&candidate).unwrap();
    assert_eq!(app.poll("job_1").unwrap().status, "in_progress");
}

#[test]
fn accepted_job_remains_recoverable_when_provenance_write_fails() {
    let fake = Fake::new();
    let dir = tempfile::tempdir().unwrap();
    let app = fake.app(dir.path());
    let candidate = dir.path().join("candidates/job_1");
    std::fs::create_dir_all(candidate.join("provenance.json")).unwrap();
    let accepted = app
        .submit(0, "qwen-image-2.1", "pixel bird", "generate")
        .unwrap();
    assert_eq!(accepted.id, "job_1");
    assert_eq!(accepted.status, "queued");
    let err = accepted.error.expect("storage warning");
    assert!(err.contains("job_1"), "missing accepted job ID: {err}");
    assert!(
        err.to_lowercase().contains("do not resubmit"),
        "missing retry warning: {err}"
    );
    assert!(candidate.join("receipt.json").is_file());
    std::fs::remove_dir(candidate.join("provenance.json")).unwrap();
    assert_eq!(app.poll("job_1").unwrap().status, "in_progress");
}

#[test]
fn save_rejects_another_documents_existing_project_but_allows_own_target() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(dir.path().into(), None).unwrap();
    app.save("art").unwrap();
    let path = dir.path().join("projects/art.spriteproj");
    let original = std::fs::read(&path).unwrap();
    app.save("art").unwrap();
    app.create(16, 16).unwrap();
    assert!(
        app.save("art").is_err(),
        "new document must not overwrite another project"
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    app.load("art").unwrap();
    assert!(
        app.save("art").is_ok(),
        "loaded project may save to its own target"
    );
}
