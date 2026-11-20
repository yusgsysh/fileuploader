use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use reqwest::{
    header::{
        HeaderName, HeaderValue, ACCEPT_ENCODING, ACCEPT_RANGES, CONTENT_DISPOSITION,
        CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION, RANGE,
    },
    redirect::Policy,
    Client, Response, StatusCode,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use url::Url;

use crate::{
    models::AppSettings,
    progress::ProgressHandle,
    security::{
        filename_from_content_disposition, filename_from_url, sanitize_filename,
        validate_public_url, SafeDnsResolver,
    },
    services::s3::StreamPart,
};

const MAX_REDIRECTS_HARD_LIMIT: usize = 20;

#[derive(Debug, Clone)]
pub struct HttpSourceSpec {
    pub url: String,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub headers: HashMap<String, String>,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HttpSourceInfo {
    pub filename: String,
    pub content_type: Option<String>,
    pub total_bytes: Option<i64>,
    pub range_supported: Option<bool>,
    pub final_url: Url,
}

pub struct PreparedHttp {
    pub response: Response,
    pub info: HttpSourceInfo,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Download cancelled")]
    Cancelled,
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Server returned HTTP {0}")]
    Status(u16),
    #[error("Server does not support Range requests; this partial download cannot be resumed")]
    RangeUnsupported,
    #[error("Server ignored the Range request; this partial download cannot be resumed")]
    RangeIgnored,
    #[error(
        "Download interrupted after {downloaded} bytes; server does not support resume: {reason}"
    )]
    InterruptedNoResume { downloaded: i64, reason: String },
    #[error("Download incomplete: expected {expected} bytes, received {received}")]
    Incomplete { expected: i64, received: i64 },
    #[error("File exceeds the configured size limit of {0} bytes")]
    SizeLimit(i64),
    #[error("Download timed out after {0} seconds without data")]
    IdleTimeout(i64),
    #[error("Task exceeded its maximum runtime")]
    TaskTimeout,
    #[error("Invalid HTTP response: {0}")]
    InvalidResponse(String),
    #[error("Download channel closed")]
    ChannelClosed,
}

impl DownloadError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

#[derive(Clone)]
pub struct HttpDownloader {
    client: Client,
    idle_timeout: Duration,
    max_redirects: usize,
    max_file_bytes: i64,
    task_timeout: Duration,
    max_retries: usize,
    http_buffer: usize,
}

impl HttpDownloader {
    pub fn new(settings: &AppSettings) -> Result<Self, DownloadError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(SafeDnsResolver))
            .connect_timeout(Duration::from_secs(settings.http_timeout_secs as u64))
            .pool_idle_timeout(Duration::from_secs(30))
            .user_agent("FileUploader/0.1 (self-hosted)")
            .build()
            .map_err(|err| DownloadError::Http(format!("failed to create HTTP client: {err}")))?;
        Ok(Self {
            client,
            idle_timeout: Duration::from_secs(settings.http_timeout_secs as u64),
            max_redirects: (settings.max_redirects as usize).min(MAX_REDIRECTS_HARD_LIMIT),
            max_file_bytes: settings.max_file_bytes,
            task_timeout: Duration::from_secs(settings.task_timeout_secs as u64),
            max_retries: settings.max_retries.max(0) as usize,
            http_buffer: settings
                .http_buffer_bytes
                .clamp(64 * 1024, 16 * 1024 * 1024) as usize,
        })
    }

    pub async fn prepare(
        &self,
        spec: &HttpSourceSpec,
        requested_filename: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<PreparedHttp, DownloadError> {
        let mut last_error: Option<DownloadError> = None;
        let mut opened = None;
        for attempt in 0..=self.max_retries {
            match self.open(spec, None, cancel, "initial request").await {
                Ok(response) => {
                    opened = Some(response);
                    break;
                }
                Err(DownloadError::Cancelled) => return Err(DownloadError::Cancelled),
                Err(err @ DownloadError::Http(_)) | Err(err @ DownloadError::IdleTimeout(_)) => {
                    last_error = Some(err);
                    if attempt < self.max_retries {
                        self.retry_delay(attempt + 1, cancel).await?;
                    }
                }
                Err(err) => return Err(err),
            }
        }
        let opened = opened.ok_or_else(|| {
            last_error.unwrap_or_else(|| DownloadError::Http("initial request failed".into()))
        })?;
        let response = opened.response;
        if !response.status().is_success() {
            return Err(DownloadError::Status(response.status().as_u16()));
        }
        let headers = response.headers();
        if let Some(encoding) = headers.get(CONTENT_ENCODING) {
            let encoding = encoding.to_str().unwrap_or_default();
            if !encoding.is_empty() && !encoding.eq_ignore_ascii_case("identity") {
                return Err(DownloadError::InvalidResponse(
                    "compressed HTTP responses are not supported".into(),
                ));
            }
        }
        let total_bytes = headers
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0);
        if let Some(total) = total_bytes {
            if self.max_file_bytes > 0 && total > self.max_file_bytes {
                return Err(DownloadError::SizeLimit(self.max_file_bytes));
            }
        }
        let content_type = headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let range_supported = headers
            .get(ACCEPT_RANGES)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_ascii_lowercase().contains("bytes"));
        let filename = requested_filename
            .map(sanitize_filename)
            .or_else(|| {
                headers
                    .get(CONTENT_DISPOSITION)
                    .and_then(|value| value.to_str().ok())
                    .and_then(filename_from_content_disposition)
            })
            .or_else(|| filename_from_url(&opened.final_url))
            .unwrap_or_else(|| "download".into());
        debug!(
            status = %response.status(),
            filename = %filename,
            total_bytes = ?total_bytes,
            range_supported = ?range_supported,
            "HTTP source prepared"
        );
        Ok(PreparedHttp {
            response,
            info: HttpSourceInfo {
                filename,
                content_type,
                total_bytes,
                range_supported,
                final_url: opened.final_url,
            },
        })
    }

    /// Stream the response body as bounded multipart parts. If the connection
    /// breaks, a Range request resumes at the exact number of bytes already
    /// placed in the assembler (including the current partial part).
    #[allow(clippy::too_many_arguments)]
    pub async fn stream(
        &self,
        prepared: PreparedHttp,
        spec: HttpSourceSpec,
        part_size: usize,
        tx: mpsc::Sender<StreamPart>,
        progress: ProgressHandle,
        cancel: CancellationToken,
    ) -> Result<(), DownloadError> {
        let started = Instant::now();
        let expected_total = prepared.info.total_bytes;
        let mut assembler = PartAssembler::new(part_size, tx.clone(), progress.clone());
        let mut response = Some(prepared.response);
        let mut expect_offset: u64 = 0;
        let mut consecutive_failures = 0usize;
        let mut total_failures = 0usize;
        let mut last_error = String::new();

        loop {
            if cancel.is_cancelled() {
                self.send_failed(&tx, "Task cancelled").await;
                return Err(DownloadError::Cancelled);
            }
            if started.elapsed() > self.task_timeout {
                self.send_failed(&tx, "Task exceeded its maximum runtime")
                    .await;
                return Err(DownloadError::TaskTimeout);
            }

            let active_response = match response.take() {
                Some(response) => response,
                None => {
                    let range_start = if expect_offset > 0 {
                        Some(expect_offset)
                    } else {
                        None
                    };
                    match self
                        .open(&spec, range_start, &cancel, "retry request")
                        .await
                    {
                        Ok(opened) => opened.response,
                        Err(DownloadError::RangeIgnored) => {
                            let reason = "server returned a 200 response to a Range request";
                            let _ = self
                                .send_failed(
                                    &tx,
                                    &format!("Server does not support Range resume: {reason}"),
                                )
                                .await;
                            return Err(DownloadError::InterruptedNoResume {
                                downloaded: expect_offset as i64,
                                reason: reason.into(),
                            });
                        }
                        Err(DownloadError::Cancelled) => {
                            return Err(DownloadError::Cancelled);
                        }
                        Err(err) => {
                            last_error = err.to_string();
                            consecutive_failures += 1;
                            total_failures += 1;
                            if consecutive_failures > self.allowed_retries()
                                || total_failures > self.allowed_retries() * 4
                            {
                                break;
                            }
                            self.retry_delay(consecutive_failures, &cancel).await?;
                            continue;
                        }
                    }
                }
            };

            let mut body = active_response.bytes_stream();
            let mut clean_eof = false;
            loop {
                if started.elapsed() > self.task_timeout {
                    let _ = self
                        .send_failed(&tx, "Task exceeded its maximum runtime")
                        .await;
                    return Err(DownloadError::TaskTimeout);
                }
                tokio::select! {
                    _ = cancel.cancelled() => {
                        self.send_failed(&tx, "Task cancelled").await;
                        return Err(DownloadError::Cancelled);
                    }
                    chunk = tokio::time::timeout(self.idle_timeout, body.next()) => {
                        match chunk {
                            Err(_) => {
                                last_error = format!(
                                    "no data for {} seconds",
                                    self.idle_timeout.as_secs()
                                );
                                break;
                            }
                            Ok(None) => {
                                clean_eof = true;
                                break;
                            }
                            Ok(Some(Err(err))) => {
                                last_error = classify_reqwest_error(&err);
                                break;
                            }
                            Ok(Some(Ok(bytes))) => {
                                if bytes.is_empty() {
                                    continue;
                                }
                                if self.max_file_bytes > 0 {
                                    let next = assembler.received() + bytes.len() as u64;
                                    if next > self.max_file_bytes as u64 {
                                        let _ = self
                                            .send_failed(&tx, "File exceeds the configured size limit")
                                            .await;
                                        return Err(DownloadError::SizeLimit(self.max_file_bytes));
                                    }
                                }
                                let len = bytes.len();
                                let mut offset = 0usize;
                                while offset < bytes.len() {
                                    let end = (offset + self.http_buffer).min(bytes.len());
                                    let slice = bytes.slice(offset..end);
                                    assembler.push(slice, &cancel).await?;
                                    offset = end;
                                }
                                progress.add_downloaded(len as u64);
                                consecutive_failures = 0;
                            }
                        }
                    }
                }
            }

            let received = assembler.received();
            if let Some(expected) = expected_total {
                if received > expected as u64 {
                    return Err(DownloadError::InvalidResponse(format!(
                        "received {received} bytes but Content-Length was {expected}"
                    )));
                }
                if received == expected as u64 {
                    return assembler.finish().await;
                }
                if clean_eof {
                    last_error = "connection ended before Content-Length was reached".into();
                }
            } else if clean_eof {
                return assembler.finish().await;
            }

            consecutive_failures += 1;
            total_failures += 1;
            if consecutive_failures > self.allowed_retries()
                || total_failures > self.allowed_retries() * 4
            {
                break;
            }
            expect_offset = received;
            debug!(
                offset = expect_offset,
                "retrying HTTP download after stream failure"
            );
            self.retry_delay(consecutive_failures, &cancel).await?;
        }

        let received = assembler.received();
        if let Some(expected) = expected_total {
            let reason = if last_error.is_empty() {
                "connection ended before Content-Length was reached".to_string()
            } else {
                last_error
            };
            let _ = self
                .send_failed(
                    &tx,
                    &format!(
                        "Download interrupted after {received} of {expected} bytes.                          The source did not provide a usable Range resume: {reason}"
                    ),
                )
                .await;
            Err(DownloadError::Incomplete {
                expected,
                received: received as i64,
            })
        } else {
            let message = format!(
                "Download interrupted after {received} bytes: {}",
                if last_error.is_empty() {
                    "stream ended unexpectedly"
                } else {
                    &last_error
                }
            );
            let _ = self.send_failed(&tx, &message).await;
            Err(DownloadError::Http(message))
        }
    }

    async fn retry_delay(
        &self,
        attempt: usize,
        cancel: &CancellationToken,
    ) -> Result<(), DownloadError> {
        let delay =
            Duration::from_millis(500 * (1u64 << (attempt.saturating_sub(1) as u32).min(5)));
        tokio::select! {
            _ = cancel.cancelled() => Err(DownloadError::Cancelled),
            _ = tokio::time::sleep(delay) => Ok(()),
        }
    }

    fn allowed_retries(&self) -> usize {
        self.max_retries
    }

    async fn send_failed(&self, tx: &mpsc::Sender<StreamPart>, message: &str) {
        let _ = tx
            .send(StreamPart::Failed {
                message: message.to_string(),
            })
            .await;
    }

    async fn open(
        &self,
        spec: &HttpSourceSpec,
        range_start: Option<u64>,
        cancel: &CancellationToken,
        phase: &str,
    ) -> Result<OpenedResponse, DownloadError> {
        let mut url =
            Url::parse(&spec.url).map_err(|_| DownloadError::Http("invalid URL".into()))?;
        let original_origin = origin(&url);
        let mut redirects = 0usize;
        loop {
            validate_public_url(url.as_str())
                .await
                .map_err(DownloadError::Http)?;
            let same_origin = origin(&url) == original_origin;
            let mut request = self.client.get(url.clone());
            request = request.header(ACCEPT_ENCODING, "identity");
            if let Some(user_agent) = spec.user_agent.as_deref() {
                request = request.header(reqwest::header::USER_AGENT, user_agent);
            }
            if let Some(referer) = spec.referer.as_deref() {
                request = request.header(reqwest::header::REFERER, referer);
            }
            if same_origin {
                if let Some(cookie) = spec.cookie.as_deref() {
                    request = request.header(reqwest::header::COOKIE, cookie);
                }
            }
            for (name, value) in &spec.headers {
                if is_forbidden_request_header(name) {
                    continue;
                }
                if !same_origin
                    && matches!(
                        name.to_ascii_lowercase().as_str(),
                        "authorization" | "cookie" | "proxy-authorization"
                    )
                {
                    continue;
                }
                let name = HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| DownloadError::Http(format!("invalid header name: {name}")))?;
                let value = HeaderValue::from_str(value)
                    .map_err(|_| DownloadError::Http("invalid header value".into()))?;
                request = request.header(name, value);
            }
            if same_origin {
                if let Some(username) = spec.auth_username.as_deref() {
                    request = request.basic_auth(username, spec.auth_password.as_deref());
                }
            }
            if let Some(start) = range_start {
                if start > 0 {
                    request = request.header(RANGE, format!("bytes={start}-"));
                }
            }
            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
                result = tokio::time::timeout(self.idle_timeout, request.send()) => {
                    match result {
                        Err(_) => return Err(DownloadError::IdleTimeout(self.idle_timeout.as_secs() as i64)),
                        Ok(Err(err)) => return Err(DownloadError::Http(classify_reqwest_error(&err))),
                        Ok(Ok(response)) => response,
                    }
                }
            };
            let status = response.status();
            if matches!(
                status,
                StatusCode::MOVED_PERMANENTLY
                    | StatusCode::FOUND
                    | StatusCode::SEE_OTHER
                    | StatusCode::TEMPORARY_REDIRECT
                    | StatusCode::PERMANENT_REDIRECT
            ) {
                redirects += 1;
                if redirects > self.max_redirects {
                    return Err(DownloadError::Http("too many redirects".into()));
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| DownloadError::Http("redirect without Location".into()))?;
                url = url
                    .join(location)
                    .map_err(|_| DownloadError::Http("invalid redirect URL".into()))?;
                continue;
            }
            if range_start.unwrap_or(0) > 0 {
                if status == StatusCode::OK {
                    return Err(DownloadError::RangeIgnored);
                }
                if status == StatusCode::RANGE_NOT_SATISFIABLE {
                    return Err(DownloadError::InvalidResponse(
                        "server rejected the resume range".into(),
                    ));
                }
                if status != StatusCode::PARTIAL_CONTENT {
                    return Err(DownloadError::Status(status.as_u16()));
                }
                validate_content_range(&response, range_start.unwrap_or(0))?;
            }
            debug!(phase, status = %status, host = url.host_str().unwrap_or("?"), "HTTP response received");
            return Ok(OpenedResponse {
                response,
                final_url: url,
            });
        }
    }
}

struct OpenedResponse {
    response: Response,
    final_url: Url,
}

struct PartAssembler {
    buffer: BytesMut,
    part_size: usize,
    next_part: i32,
    received: u64,
    tx: mpsc::Sender<StreamPart>,
    progress: ProgressHandle,
}

impl PartAssembler {
    fn new(part_size: usize, tx: mpsc::Sender<StreamPart>, progress: ProgressHandle) -> Self {
        Self {
            buffer: BytesMut::with_capacity(part_size.min(16 * 1024 * 1024)),
            part_size,
            next_part: 1,
            received: 0,
            tx,
            progress,
        }
    }

    fn received(&self) -> u64 {
        self.received
    }

    async fn push(
        &mut self,
        mut chunk: Bytes,
        cancel: &CancellationToken,
    ) -> Result<(), DownloadError> {
        self.received += chunk.len() as u64;
        while !chunk.is_empty() {
            let space = self.part_size.saturating_sub(self.buffer.len());
            let take = space.min(chunk.len());
            self.buffer.extend_from_slice(&chunk[..take]);
            chunk = chunk.slice(take..);
            if self.buffer.len() == self.part_size {
                let data = self.buffer.split().freeze();
                self.send_part(data, cancel).await?;
            }
        }
        Ok(())
    }

    async fn send_part(
        &mut self,
        data: Bytes,
        cancel: &CancellationToken,
    ) -> Result<(), DownloadError> {
        let number = self.next_part;
        self.next_part += 1;
        tokio::select! {
            _ = cancel.cancelled() => Err(DownloadError::Cancelled),
            result = self.tx.send(StreamPart::Part { number, data }) => {
                result.map_err(|_| DownloadError::ChannelClosed)
            }
        }
    }

    async fn finish(mut self) -> Result<(), DownloadError> {
        if !self.buffer.is_empty() {
            let data = self.buffer.split().freeze();
            let number = self.next_part;
            self.next_part += 1;
            self.tx
                .send(StreamPart::Part { number, data })
                .await
                .map_err(|_| DownloadError::ChannelClosed)?;
        }
        let total = self.received as i64;
        self.progress.set_stage(crate::models::TaskStage::Uploading);
        self.tx
            .send(StreamPart::Finish { total_bytes: total })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        Ok(())
    }
}

fn validate_content_range(response: &Response, expected_start: u64) -> Result<(), DownloadError> {
    let value = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            DownloadError::InvalidResponse("206 response without Content-Range".into())
        })?;
    let rest = value
        .strip_prefix("bytes ")
        .ok_or_else(|| DownloadError::InvalidResponse("invalid Content-Range unit".into()))?;
    let (range, _total) = rest
        .split_once('/')
        .ok_or_else(|| DownloadError::InvalidResponse("invalid Content-Range".into()))?;
    let (start, _end) = range
        .split_once('-')
        .ok_or_else(|| DownloadError::InvalidResponse("invalid Content-Range".into()))?;
    let start: u64 = start
        .parse()
        .map_err(|_| DownloadError::InvalidResponse("invalid Content-Range start".into()))?;
    if start != expected_start {
        return Err(DownloadError::InvalidResponse(format!(
            "server resumed at byte {start}, expected {expected_start}"
        )));
    }
    Ok(())
}

fn is_forbidden_request_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "range"
            | "accept-encoding"
            | "proxy-authorization"
            | "upgrade"
            | "te"
            | "trailer"
    )
}

fn origin(url: &Url) -> (String, String, Option<u16>) {
    (
        url.scheme().to_string(),
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default(),
    )
}

fn classify_reqwest_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "request timed out".into()
    } else if err.is_connect() {
        "could not connect to source".into()
    } else if err.is_body() {
        "response body stream failed".into()
    } else if err.is_decode() {
        "could not decode source response".into()
    } else {
        "network error".into()
    }
}
