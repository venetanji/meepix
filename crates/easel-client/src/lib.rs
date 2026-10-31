//! Blocking, bounded Easel image-job client. Call from a worker thread.
use reqwest::{
    blocking::{multipart, Client as Http, RequestBuilder, Response},
    redirect::Policy,
    Url,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::{Cursor, Read},
    net::IpAddr,
    time::Duration,
};

const JSON_LIMIT: u64 = 1024 * 1024;
const IMAGE_LIMIT: u64 = 16 * 1024 * 1024;
const MAX_IMAGES: usize = 16;
const MAX_DIMENSION: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidEndpoint,
    InvalidInput,
    Timeout,
    Network,
    Unauthorized,
    RateLimited,
    Expired,
    Http(u16),
    InvalidResponse,
    TooLarge,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint => write!(f, "Invalid Easel endpoint"),
            Self::InvalidInput => write!(f, "Invalid Easel request"),
            Self::Timeout => write!(
                f,
                "Easel request timed out; submission may have been accepted"
            ),
            Self::Network => write!(
                f,
                "Easel connection failed; submission may have been accepted"
            ),
            Self::Unauthorized => write!(f, "Easel authentication failed (401)"),
            Self::RateLimited => write!(f, "Easel is busy (429); try again later"),
            Self::Expired => write!(f, "Easel job expired (410)"),
            Self::Http(code) => write!(f, "Easel returned HTTP {code}"),
            Self::InvalidResponse => write!(f, "Invalid Easel response"),
            Self::TooLarge => write!(f, "Easel response exceeds limits"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: String,
    pub status: Status,
    #[serde(default)]
    pub data: Vec<Content>,
    #[serde(default)]
    pub progress: Option<f64>,
}

pub struct Client {
    endpoint: Url,
    token: Option<String>,
    http: Http,
}
impl Client {
    pub fn new(endpoint: &str, token: Option<String>) -> Result<Self, Error> {
        Self::with_timeout(endpoint, token, Duration::from_secs(30))
    }
    pub fn with_timeout(
        endpoint: &str,
        token: Option<String>,
        timeout: Duration,
    ) -> Result<Self, Error> {
        let mut url = Url::parse(endpoint).map_err(|_| Error::InvalidEndpoint)?;
        let loopback = url
            .host_str()
            .and_then(|h| h.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
        {
            return Err(Error::InvalidEndpoint);
        }
        url.set_path("/");
        if token
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 8192 || s.chars().any(|c| c.is_control()))
        {
            return Err(Error::InvalidInput);
        }
        let builder = Http::builder()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .timeout(timeout);
        let builder = if loopback {
            builder.no_proxy()
        } else {
            builder
        };
        let http = builder.build().map_err(|_| Error::Network)?;
        Ok(Self {
            endpoint: url,
            token,
            http,
        })
    }
    fn request(&self, req: RequestBuilder) -> RequestBuilder {
        if let Some(token) = &self.token {
            req.bearer_auth(token)
        } else {
            req
        }
    }
    fn url(&self, path: &str) -> Url {
        self.endpoint.join(path).expect("static valid path")
    }
    fn send(&self, req: RequestBuilder, limit: u64) -> Result<Vec<u8>, Error> {
        self.send_with_status(req, limit, false)
    }
    fn send_with_status(
        &self,
        req: RequestBuilder,
        limit: u64,
        accepted: bool,
    ) -> Result<Vec<u8>, Error> {
        let response = self.request(req).send().map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Network
            }
        })?;
        if accepted && response.status().is_success() && response.status().as_u16() != 202 {
            return Err(Error::InvalidResponse);
        }
        read_response(response, limit)
    }
    pub fn models(&self) -> Result<Vec<String>, Error> {
        #[derive(Deserialize)]
        struct List {
            data: Vec<Model>,
        }
        #[derive(Deserialize)]
        struct Model {
            id: String,
        }
        let data = self.send(self.http.get(self.url("/v1/models")), JSON_LIMIT)?;
        let list: List = serde_json::from_slice(&data).map_err(|_| Error::InvalidResponse)?;
        if list.data.len() > 1024
            || list
                .data
                .iter()
                .any(|m| m.id.is_empty() || m.id.len() > 256)
        {
            return Err(Error::InvalidResponse);
        }
        Ok(list.data.into_iter().map(|m| m.id).collect())
    }
    pub fn submit(
        &self,
        model: &str,
        prompt: &str,
        width: u32,
        height: u32,
        reference: Option<Vec<u8>>,
    ) -> Result<Receipt, Error> {
        if model.trim().is_empty()
            || model.len() > 256
            || prompt.trim().is_empty()
            || prompt.len() > 32768
            || width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
        {
            return Err(Error::InvalidInput);
        }
        let size = format!("{width}x{height}");
        let req = if let Some(bytes) = reference {
            validate_png(&bytes).map_err(|_| Error::InvalidInput)?;
            let part = multipart::Part::bytes(bytes)
                .file_name("reference.png")
                .mime_str("image/png")
                .map_err(|_| Error::InvalidInput)?;
            let form = multipart::Form::new()
                .text("model", model.to_owned())
                .text("prompt", prompt.to_owned())
                .text("size", size)
                .text("response_format", "url")
                .part("image", part);
            self.http.post(self.url("/v1/images/edits")).multipart(form)
        } else {
            self.http.post(self.url("/v1/images/generations")).json(&serde_json::json!({"model":model,"prompt":prompt,"size":size,"response_format":"url"}))
        };
        let bytes =
            self.send_with_status(req.header("Prefer", "respond-async"), JSON_LIMIT, true)?;
        parse_receipt(&bytes, None)
    }
    pub fn poll(&self, id: &str) -> Result<Receipt, Error> {
        if !valid_id(id) {
            return Err(Error::InvalidInput);
        }
        let bytes = self.send(
            self.http.get(self.url(&format!("/v1/images/jobs/{id}"))),
            JSON_LIMIT,
        )?;
        parse_receipt(&bytes, Some(id))
    }
    pub fn fetch_content(&self, receipt: &Receipt) -> Result<Vec<Vec<u8>>, Error> {
        if !valid_id(&receipt.id)
            || receipt.status != Status::Completed
            || receipt.data.is_empty()
            || receipt.data.len() > MAX_IMAGES
        {
            return Err(Error::InvalidResponse);
        }
        let mut images = Vec::new();
        for (i, content) in receipt.data.iter().enumerate() {
            let url = self
                .endpoint
                .join(&content.url)
                .map_err(|_| Error::InvalidResponse)?;
            let path = format!("/v1/images/jobs/{}/content/{i}", receipt.id);
            if url.origin() != self.endpoint.origin()
                || url.path() != path
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(Error::InvalidResponse);
            }
            let bytes = self.send(self.http.get(url), IMAGE_LIMIT)?;
            validate_png(&bytes)?;
            images.push(bytes);
        }
        Ok(images)
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn parse_receipt(bytes: &[u8], expected: Option<&str>) -> Result<Receipt, Error> {
    let receipt: Receipt = serde_json::from_slice(bytes).map_err(|_| Error::InvalidResponse)?;
    if !valid_id(&receipt.id)
        || expected.is_some_and(|id| id != receipt.id)
        || receipt.data.len() > MAX_IMAGES
    {
        return Err(Error::InvalidResponse);
    }
    Ok(receipt)
}
fn read_response(response: Response, limit: u64) -> Result<Vec<u8>, Error> {
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => Error::Unauthorized,
            429 => Error::RateLimited,
            410 => Error::Expired,
            code => Error::Http(code),
        });
    }
    if response.content_length().is_some_and(|n| n > limit) {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                Error::Timeout
            } else {
                Error::Network
            }
        })?;
    if bytes.len() as u64 > limit {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}
fn validate_png(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() as u64 > IMAGE_LIMIT || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::InvalidResponse);
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|_| Error::InvalidResponse)?;
    Ok(())
}
