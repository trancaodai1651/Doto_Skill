//! Phục vụ file video: HTTP Range (206 Partial Content / 416), Content-Disposition cho tải về,
//! stream theo khối với giới hạn băng thông tuỳ chọn. Không bao giờ lộ đường dẫn đĩa.

use std::path::Path;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::error::{ApiError, ApiResult};

#[derive(Debug, PartialEq, Eq)]
pub enum RangeSpec {
    Full,
    Partial(u64, u64), // inclusive
    Unsatisfiable,
}

/// Parse `Range: bytes=...` (chỉ một đoạn; nhiều đoạn → trả toàn bộ file theo RFC 9110 cho phép).
pub fn parse_range(header: Option<&str>, size: u64) -> RangeSpec {
    let Some(h) = header else { return RangeSpec::Full };
    let Some(spec) = h.trim().strip_prefix("bytes=") else { return RangeSpec::Full };
    if spec.contains(',') {
        return RangeSpec::Full;
    }
    let Some((a, b)) = spec.split_once('-') else { return RangeSpec::Unsatisfiable };
    let (a, b) = (a.trim(), b.trim());
    if size == 0 {
        return RangeSpec::Unsatisfiable;
    }
    match (a.is_empty(), b.is_empty()) {
        (true, true) => RangeSpec::Unsatisfiable,
        (true, false) => match b.parse::<u64>() {
            Ok(0) | Err(_) => RangeSpec::Unsatisfiable,
            Ok(n) => RangeSpec::Partial(size.saturating_sub(n), size - 1),
        },
        (false, _) => {
            let Ok(start) = a.parse::<u64>() else { return RangeSpec::Unsatisfiable };
            if start >= size {
                return RangeSpec::Unsatisfiable;
            }
            let end = if b.is_empty() {
                size - 1
            } else {
                match b.parse::<u64>() {
                    Ok(e) => e.min(size - 1),
                    Err(_) => return RangeSpec::Unsatisfiable,
                }
            };
            if end < start {
                return RangeSpec::Unsatisfiable;
            }
            RangeSpec::Partial(start, end)
        }
    }
}

/// RFC 6266/5987: filename ASCII dự phòng + filename* UTF-8.
pub fn content_disposition(file_name: &str) -> String {
    let ascii: String = file_name.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_ ".contains(c) { c } else { '_' }).collect();
    let enc: String =
        file_name.bytes().map(|b| if b.is_ascii_alphanumeric() || b".-_".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{enc}")
}

pub struct ServeOpts<'a> {
    pub download_name: Option<&'a str>,
    pub bytes_per_sec: Option<u64>,
    pub permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

pub async fn serve_file(path: &Path, req_headers: &HeaderMap, opts: ServeOpts<'_>) -> ApiResult<Response> {
    let meta = tokio::fs::metadata(path).await.map_err(|_| ApiError::not_found("file video không còn trên đĩa"))?;
    let size = meta.len();
    let range = parse_range(req_headers.get(header::RANGE).and_then(|v| v.to_str().ok()), size);
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut b = Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, mime.essence_str())
        .header(header::CACHE_CONTROL, "private, max-age=0, must-revalidate")
        .header("X-Content-Type-Options", "nosniff");
    if let Some(n) = opts.download_name {
        b = b.header(header::CONTENT_DISPOSITION, HeaderValue::from_str(&content_disposition(n)).map_err(ApiError::internal)?);
    }
    let (status, start, len) = match range {
        RangeSpec::Full => (StatusCode::OK, 0, size),
        RangeSpec::Partial(s, e) => {
            b = b.header(header::CONTENT_RANGE, format!("bytes {s}-{e}/{size}"));
            (StatusCode::PARTIAL_CONTENT, s, e - s + 1)
        }
        RangeSpec::Unsatisfiable => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                .body(Body::empty())
                .map_err(ApiError::internal);
        }
    };
    let mut file = tokio::fs::File::open(path).await.map_err(ApiError::internal)?;
    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start)).await.map_err(ApiError::internal)?;
    }
    let reader = file.take(len);
    let rate = opts.bytes_per_sec.filter(|r| *r > 0);
    let permit = opts.permit;
    const CHUNK: usize = 64 * 1024;
    let stream = tokio_util::io::ReaderStream::with_capacity(reader, CHUNK).then(move |chunk| {
        let _keep = &permit; // giữ suất phát luồng tới khi body bị drop
        async move {
            if let (Some(r), Ok(c)) = (rate, &chunk) {
                tokio::time::sleep(Duration::from_secs_f64(c.len() as f64 / r as f64)).await;
            }
            chunk
        }
    });
    b.status(status).header(header::CONTENT_LENGTH, len).body(Body::from_stream(stream)).map_err(ApiError::internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_parsing() {
        assert_eq!(parse_range(None, 100), RangeSpec::Full);
        assert_eq!(parse_range(Some("bytes=0-9"), 100), RangeSpec::Partial(0, 9));
        assert_eq!(parse_range(Some("bytes=90-"), 100), RangeSpec::Partial(90, 99));
        assert_eq!(parse_range(Some("bytes=-10"), 100), RangeSpec::Partial(90, 99));
        assert_eq!(parse_range(Some("bytes=50-1000"), 100), RangeSpec::Partial(50, 99));
        assert_eq!(parse_range(Some("bytes=100-"), 100), RangeSpec::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=20-10"), 100), RangeSpec::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=abc"), 100), RangeSpec::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=0-1,5-6"), 100), RangeSpec::Full);
        assert_eq!(parse_range(Some("items=0-1"), 100), RangeSpec::Full);
    }

    #[test]
    fn disposition_encodes_unicode() {
        let d = content_disposition("Bản tin \"số 1\".mp4");
        assert!(d.starts_with("attachment; filename=\"B_n tin _s_ 1_.mp4\""));
        assert!(d.contains("filename*=UTF-8''B%E1%BA%A3n%20tin%20%22s%E1%BB%91%201%22.mp4"));
    }
}
