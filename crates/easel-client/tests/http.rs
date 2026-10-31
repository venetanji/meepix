use easel_client::{Client, Status};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn server(response: &'static str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            let n = stream.read(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    });
    (url, task)
}

#[test]
fn discovers_models_with_authentication() {
    let (url, task) = server("HTTP/1.1 200 OK\r\nContent-Length: 34\r\nConnection: close\r\n\r\n{\"data\":[{\"id\":\"qwen-image-2.1\"}]}");
    let client = Client::new(&url, Some("test-token".into())).unwrap();
    assert_eq!(client.models().unwrap(), vec!["qwen-image-2.1"]);
    let request = task.join().unwrap().to_lowercase();
    assert!(request.starts_with("get /v1/models "));
    assert!(request.contains("authorization: bearer test-token"));
}

#[test]
fn queued_submission_requests_async_url_once() {
    let body = r#"{"id":"job_1","status":"queued"}"#;
    let response = Box::leak(
        format!(
            "HTTP/1.1 202 Accepted\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .into_boxed_str(),
    );
    let (url, task) = server(response);
    let receipt = Client::new(&url, None)
        .unwrap()
        .submit("qwen-image-2.1", "cat", 64, 64, None)
        .unwrap();
    assert_eq!(receipt.status, Status::Queued);
    let request = task.join().unwrap().to_lowercase();
    assert!(request.starts_with("post /v1/images/generations "));
    assert!(request.contains("prefer: respond-async"));
}

fn reply(status: &str, body: &str) -> &'static str {
    Box::leak(
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_boxed_str(),
    )
}

#[test]
fn maps_http_errors_and_never_displays_response_body() {
    use easel_client::Error;
    for (status, expected) in [
        ("401 Unauthorized", Error::Unauthorized),
        ("429 Too Many Requests", Error::RateLimited),
        ("410 Gone", Error::Expired),
    ] {
        let (url, task) = server(reply(status, "secret-token"));
        let err = Client::new(&url, Some("secret-token".into()))
            .unwrap()
            .models()
            .unwrap_err();
        assert_eq!(err, expected);
        assert!(!err.to_string().contains("secret-token"));
        task.join().unwrap();
    }
}

#[test]
fn polling_all_states_and_rejects_bad_receipts() {
    for (name, status) in [
        ("queued", Status::Queued),
        ("in_progress", Status::InProgress),
        ("completed", Status::Completed),
        ("failed", Status::Failed),
        ("cancelled", Status::Cancelled),
        ("new_state", Status::Unknown),
    ] {
        let body = format!("{{\"id\":\"job_1\",\"status\":\"{name}\"}}");
        let (url, task) = server(reply("200 OK", &body));
        assert_eq!(
            Client::new(&url, None)
                .unwrap()
                .poll("job_1")
                .unwrap()
                .status,
            status
        );
        assert!(task
            .join()
            .unwrap()
            .starts_with("GET /v1/images/jobs/job_1 "));
    }
    for body in ["not json", r#"{"id":"other","status":"queued"}"#] {
        let (url, task) = server(reply("200 OK", body));
        assert!(Client::new(&url, None).unwrap().poll("job_1").is_err());
        task.join().unwrap();
    }
}

#[test]
fn refuses_redirects_and_untrusted_content_urls_without_leaking_auth() {
    use easel_client::{Content, Receipt};
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let redirect = Box::leak(format!("HTTP/1.1 302 Found\r\nLocation: http://{}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", target.local_addr().unwrap()).into_boxed_str());
    let (url, task) = server(redirect);
    let client = Client::new(&url, Some("secret".into())).unwrap();
    assert!(client.models().is_err());
    task.join().unwrap();
    assert!(target.accept().is_err());
    for path in [
        format!(
            "http://{}/v1/images/jobs/job_1/content/0",
            target.local_addr().unwrap()
        ),
        format!("{url}/v1/images/jobs/other/content/0"),
        format!("{url}/v1/images/jobs/job_1/content/0?secret=yes"),
    ] {
        let receipt = Receipt {
            id: "job_1".into(),
            status: Status::Completed,
            data: vec![Content { url: path }],
            progress: None,
        };
        assert!(client.fetch_content(&receipt).is_err());
    }
    assert!(target.accept().is_err());
}

#[test]
fn accepts_only_secure_or_literal_loopback_origin() {
    for endpoint in [
        "http://example.com",
        "http://localhost:1234",
        "https://user:pass@example.com",
        "https://example.com/base",
        "https://example.com/?x=1",
    ] {
        assert!(Client::new(endpoint, None).is_err(), "{endpoint}");
    }
    assert!(Client::new("https://example.com", None).is_ok());
}

#[test]
fn timeout_does_not_retry_submission() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let (mut socket, _) = loop {
            if let Ok(pair) = listener.accept() {
                break pair;
            }
            thread::yield_now();
        };
        let mut data = [0; 4096];
        socket.read(&mut data).unwrap();
        thread::sleep(std::time::Duration::from_millis(200));
        assert!(listener.accept().is_err());
    });
    let client = Client::with_timeout(&url, None, std::time::Duration::from_millis(50)).unwrap();
    assert_eq!(
        client.submit("model", "prompt", 64, 64, None).unwrap_err(),
        easel_client::Error::Timeout
    );
    task.join().unwrap();
}

#[test]
fn edit_submits_png_reference_as_multipart() {
    let mut png = Vec::new();
    image::DynamicImage::new_rgba8(1, 1)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let (url, task) = server(reply("202 Accepted", r#"{"id":"job_1","status":"queued"}"#));
    let client = Client::new(&url, None).unwrap();
    client.submit("model", "edit", 64, 64, Some(png)).unwrap();
    let req = task.join().unwrap().to_lowercase();
    assert!(req.starts_with("post /v1/images/edits "));
    assert!(req.contains("multipart/form-data"));
    assert!(req.contains("name=\"image\""));
}

#[test]
fn content_requires_valid_png() {
    use easel_client::{Content, Receipt};
    let (url, task) = server(reply("200 OK", "not png"));
    let receipt = Receipt {
        id: "job_1".into(),
        status: Status::Completed,
        data: vec![Content {
            url: format!("{url}/v1/images/jobs/job_1/content/0"),
        }],
        progress: None,
    };
    assert!(Client::new(&url, Some("secret".into()))
        .unwrap()
        .fetch_content(&receipt)
        .is_err());
    assert!(task
        .join()
        .unwrap()
        .to_lowercase()
        .contains("authorization: bearer secret"));
}

#[test]
fn submission_requires_async_acceptance_status() {
    let (url, task) = server(reply("200 OK", r#"{"id":"job_1","status":"queued"}"#));
    assert!(Client::new(&url, None)
        .unwrap()
        .submit("model", "prompt", 64, 64, None)
        .is_err());
    task.join().unwrap();
}

#[test]
fn fetches_original_png_from_exact_job_route() {
    use easel_client::{Content, Receipt};
    let mut png = Vec::new();
    image::DynamicImage::new_rgba8(2, 2)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let expected = png.clone();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let n = stream.read(&mut request).unwrap();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    png.len()
                )
                .as_bytes(),
            )
            .unwrap();
        stream.write_all(&png).unwrap();
        String::from_utf8_lossy(&request[..n]).into_owned()
    });
    let receipt = Receipt {
        id: "job_1".into(),
        status: Status::Completed,
        data: vec![Content {
            url: format!("{url}/v1/images/jobs/job_1/content/0"),
        }],
        progress: None,
    };
    assert_eq!(
        Client::new(&url, Some("secret".into()))
            .unwrap()
            .fetch_content(&receipt)
            .unwrap(),
        vec![expected]
    );
    assert!(task
        .join()
        .unwrap()
        .starts_with("GET /v1/images/jobs/job_1/content/0 "));
}
