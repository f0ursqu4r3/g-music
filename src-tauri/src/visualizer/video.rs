//! Serves a track's YouTube video to the visualizer through the `gmvideo` scheme.
//!
//! The webview cannot read googlevideo pixels into WebGL across origins, and the stream
//! URL is bound to the yt-dlp request. This scheme resolves the stream in Rust and relays
//! bounded byte ranges with CORS headers. It accepts only video IDs, never URLs.

use std::{
    collections::HashMap,
    process::Command,
    sync::{Mutex, OnceLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

use tauri::{
    AppHandle,
    http::{Request, Response, StatusCode, header},
};

pub(crate) const SCHEME: &str = "gmvideo";
/// WebKit asks for open-ended ranges. Bounded replies keep each response in memory small.
const CHUNK_BYTES: u64 = 2 * 1024 * 1024;
/// googlevideo URLs expire after about six hours.
const URL_LIFETIME: Duration = Duration::from_secs(3 * 60 * 60);
const CACHE_LIMIT: usize = 8;
/// H.264 in MP4 plays in every desktop webview. Video-only streams avoid duplicate audio.
const FORMAT: &str = "bestvideo[vcodec^=avc1][height<=1080][ext=mp4]/bestvideo[vcodec^=avc1][ext=mp4]/best[vcodec^=avc1][ext=mp4]";

#[derive(Clone, Debug, PartialEq)]
struct Stream {
    url: String,
    headers: Vec<(String, String)>,
    length: Option<u64>,
    resolved_at: Instant,
}

fn cache() -> &'static Mutex<HashMap<String, Stream>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Stream>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// One resolution at a time. Parallel range requests for one video share its result.
fn resolving() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Default::default)
}

fn client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default()
    })
}

pub(crate) fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Returns the first byte and the optional last byte of a `bytes=` range.
fn parse_range(value: Option<&str>) -> Option<(u64, Option<u64>)> {
    let Some(value) = value else {
        return Some((0, None));
    };
    let (start, end) = value.trim().strip_prefix("bytes=")?.split_once('-')?;
    // Suffix ranges and multiple ranges are not used by media elements.
    let start = start.trim().parse::<u64>().ok()?;
    let end = match end.trim() {
        "" => None,
        end => Some(end.parse::<u64>().ok().filter(|end| *end >= start)?),
    };
    Some((start, end))
}

/// Limits a requested range to one chunk and to the known stream length.
fn bounded_range(start: u64, end: Option<u64>, length: Option<u64>) -> Option<(u64, u64)> {
    if length.is_some_and(|length| start >= length) {
        return None;
    }
    let mut last = start + CHUNK_BYTES - 1;
    if let Some(end) = end {
        last = last.min(end);
    }
    if let Some(length) = length {
        last = last.min(length - 1);
    }
    Some((start, last))
}

/// Reads the total length from `bytes start-end/total`.
fn content_range_total(value: &str) -> Option<u64> {
    value.rsplit_once('/')?.1.trim().parse().ok()
}

fn trusted_stream_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host.ends_with(".googlevideo.com"))
    })
}

fn parse_resolution(output: &[u8]) -> Result<Stream, String> {
    let value: serde_json::Value =
        serde_json::from_slice(output).map_err(|_| "yt-dlp returned unreadable video data.")?;
    // A merged selection lists its parts; a single selection is at the top level.
    let format = value
        .get("requested_formats")
        .and_then(|formats| formats.as_array())
        .and_then(|formats| formats.first())
        .unwrap_or(&value);
    let url = format
        .get("url")
        .and_then(|url| url.as_str())
        .ok_or("This track has no playable video stream.")?;
    if !trusted_stream_url(url) {
        return Err("YouTube returned an unexpected video host.".into());
    }
    let headers = format
        .get("http_headers")
        .and_then(|headers| headers.as_object())
        .map(|headers| {
            headers
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let length = format
        .get("filesize")
        .and_then(|size| size.as_u64())
        .filter(|size| *size > 0);
    Ok(Stream {
        url: url.to_string(),
        headers,
        length,
        resolved_at: Instant::now(),
    })
}

fn resolve(app: &AppHandle, id: &str) -> Result<Stream, String> {
    let cookies = crate::auth::session_cookie_path(app)
        .ok()
        .filter(|path| crate::auth::session_exists(path));
    resolve_with(cookies.as_deref(), id)
}

fn resolve_with(cookies: Option<&std::path::Path>, id: &str) -> Result<Stream, String> {
    let mut arguments = vec![
        "--ignore-config".to_string(),
        "--no-warnings".into(),
        "--no-playlist".into(),
        "--skip-download".into(),
        "--dump-single-json".into(),
        "--socket-timeout".into(),
        "10".into(),
        "-f".into(),
        FORMAT.into(),
    ];
    if let Some(path) = cookies {
        arguments.push("--cookies".into());
        arguments.push(path.to_string_lossy().into_owned());
    }
    arguments.push("--".into());
    arguments.push(format!("https://www.youtube.com/watch?v={id}"));
    let executable = crate::playback::dependency_executable(
        "GMUSIC_YT_DLP_PATH",
        &["/opt/homebrew/bin/yt-dlp", "/usr/local/bin/yt-dlp"],
        "yt-dlp",
    );
    let output = crate::process::capture(
        Command::new(executable)
            .args(arguments)
            // yt-dlp finds its JavaScript runtime on PATH. Apps started from the Dock get a minimal one.
            .env("PATH", crate::playback::playback_path()),
        &AtomicBool::new(false),
        Duration::from_secs(45),
        4 * 1024 * 1024,
    )
    .map_err(|_| "yt-dlp could not find a video for this track.".to_string())?;
    parse_resolution(&output)
}

fn stream(app: &AppHandle, id: &str, refresh: bool) -> Result<Stream, String> {
    let fresh = |stream: &Stream| stream.resolved_at.elapsed() < URL_LIFETIME;
    let cached = |refresh: bool| {
        let cache = cache().lock().ok()?;
        cache
            .get(id)
            .filter(|stream| !refresh && fresh(stream))
            .cloned()
    };
    if let Some(stream) = cached(refresh) {
        return Ok(stream);
    }
    let _guard = resolving().lock().map_err(|_| "Video resolution failed.")?;
    // Another request may have resolved this video while this one waited.
    if !refresh && let Some(stream) = cached(false) {
        return Ok(stream);
    }
    let stream = resolve(app, id)?;
    if let Ok(mut cache) = cache().lock() {
        if cache.len() >= CACHE_LIMIT && !cache.contains_key(id) {
            cache.retain(|_, stream| fresh(stream));
            if cache.len() >= CACHE_LIMIT {
                let oldest = cache
                    .iter()
                    .min_by_key(|(_, stream)| stream.resolved_at)
                    .map(|(id, _)| id.clone());
                if let Some(oldest) = oldest {
                    cache.remove(&oldest);
                }
            }
        }
        cache.insert(id.to_string(), stream.clone());
    }
    Ok(stream)
}

fn remember_length(id: &str, length: u64) {
    if let Ok(mut cache) = cache().lock()
        && let Some(stream) = cache.get_mut(id)
    {
        stream.length = Some(length);
    }
}

struct Chunk {
    bytes: Vec<u8>,
    start: u64,
    end: u64,
    length: u64,
    content_type: String,
}

fn fetch(stream: &Stream, start: u64, end: u64) -> Result<Chunk, (u16, String)> {
    let mut request = client().get(&stream.url);
    for (name, value) in &stream.headers {
        request = request.header(name, value);
    }
    let response = request
        .header("Range", format!("bytes={start}-{end}"))
        .send()
        .map_err(|_| (502, "The video stream did not respond.".to_string()))?;
    let status = response.status().as_u16();
    if status != 206 {
        return Err((
            status,
            format!("The video stream returned status {status}."),
        ));
    }
    let length = response
        .headers()
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(content_range_total)
        .or(stream.length)
        .ok_or((502, "The video stream has no length.".to_string()))?;
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("video/"))
        .unwrap_or("video/mp4")
        .to_string();
    let bytes = response
        .bytes()
        .map_err(|_| (502, "The video stream stopped.".to_string()))?
        .to_vec();
    if bytes.is_empty() {
        return Err((502, "The video stream was empty.".to_string()));
    }
    let end = start + bytes.len() as u64 - 1;
    Ok(Chunk {
        bytes,
        start,
        end,
        length,
        content_type,
    })
}

fn reply(status: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(status)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store")
}

fn failure(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    reply(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(message.as_bytes().to_vec())
        .unwrap_or_default()
}

pub(crate) fn handle(app: &AppHandle, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let id = request.uri().path().trim_matches('/');
    if !valid_video_id(id) {
        return failure(StatusCode::NOT_FOUND, "Unknown video.");
    }
    let range = request
        .headers()
        .get(header::RANGE)
        .map(|value| value.to_str().unwrap_or(""));
    let Some((start, end)) = parse_range(range) else {
        return failure(StatusCode::RANGE_NOT_SATISFIABLE, "Unsupported range.");
    };
    let mut refresh = false;
    loop {
        let stream = match stream(app, id, refresh) {
            Ok(stream) => stream,
            Err(message) => return failure(StatusCode::BAD_GATEWAY, &message),
        };
        let Some((start, last)) = bounded_range(start, end, stream.length) else {
            return failure(StatusCode::RANGE_NOT_SATISFIABLE, "Range is past the end.");
        };
        match fetch(&stream, start, last) {
            Ok(chunk) => {
                remember_length(id, chunk.length);
                return reply(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, chunk.content_type)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", chunk.start, chunk.end, chunk.length),
                    )
                    .header(header::CONTENT_LENGTH, chunk.bytes.len())
                    .body(chunk.bytes)
                    .unwrap_or_default();
            }
            // An expired or rejected URL gets one fresh resolution.
            Err((403 | 410, _)) if !refresh => refresh = true,
            Err((_, message)) => return failure(StatusCode::BAD_GATEWAY, &message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs yt-dlp and the network: `cargo test -- --ignored relays_a_real_video_range`.
    #[test]
    #[ignore]
    fn relays_a_real_video_range() {
        let stream = resolve_with(None, "dQw4w9WgXcQ").unwrap();
        let (start, last) = bounded_range(0, None, stream.length).unwrap();
        let chunk = fetch(&stream, start, last).unwrap();
        assert_eq!(chunk.bytes.len() as u64, CHUNK_BYTES);
        assert!(chunk.content_type.starts_with("video/mp4"));
        // MP4 files start with an ftyp box.
        assert_eq!(&chunk.bytes[4..8], b"ftyp");
        let tail = fetch(&stream, chunk.length - 10, chunk.length - 1).unwrap();
        assert_eq!(tail.bytes.len(), 10);
    }

    #[test]
    fn accepts_only_youtube_video_ids() {
        assert!(valid_video_id("dQw4w9WgXcQ"));
        assert!(valid_video_id("a-b_c123456"));
        for id in [
            "",
            "short",
            "dQw4w9WgXcQx",
            "dQw4w9WgXc/",
            "../etc/pas",
            "dQw4w9WgX c",
        ] {
            assert!(!valid_video_id(id), "{id}");
        }
    }

    #[test]
    fn parses_media_ranges() {
        assert_eq!(parse_range(None), Some((0, None)));
        assert_eq!(parse_range(Some("bytes=0-")), Some((0, None)));
        assert_eq!(parse_range(Some("bytes=100-199")), Some((100, Some(199))));
        for value in [
            "bytes=-500",
            "bytes=5-2",
            "items=0-1",
            "bytes=0-1,4-5",
            "bytes=x-",
        ] {
            assert_eq!(parse_range(Some(value)), None, "{value}");
        }
    }

    #[test]
    fn bounds_each_reply_to_one_chunk_and_the_stream_end() {
        assert_eq!(bounded_range(0, None, None), Some((0, CHUNK_BYTES - 1)));
        assert_eq!(bounded_range(10, Some(19), None), Some((10, 19)));
        assert_eq!(bounded_range(90, None, Some(100)), Some((90, 99)));
        assert_eq!(bounded_range(100, None, Some(100)), None);
    }

    #[test]
    fn reads_total_length_from_content_range() {
        assert_eq!(content_range_total("bytes 0-99/12345"), Some(12345));
        assert_eq!(content_range_total("bytes 0-99/*"), None);
    }

    #[test]
    fn accepts_streams_only_from_googlevideo() {
        let stream = parse_resolution(
            br#"{"url":"https://rr1---sn-abc.googlevideo.com/videoplayback?x=1","http_headers":{"User-Agent":"UA"},"filesize":42}"#,
        )
        .unwrap();
        assert_eq!(stream.headers, vec![("User-Agent".into(), "UA".into())]);
        assert_eq!(stream.length, Some(42));
        let merged = parse_resolution(
            br#"{"requested_formats":[{"url":"https://r.googlevideo.com/v"}],"url":"ignored"}"#,
        )
        .unwrap();
        assert_eq!(merged.url, "https://r.googlevideo.com/v");
        for output in [
            br#"{"url":"https://evil.example/videoplayback"}"#.as_slice(),
            br#"{"url":"http://r.googlevideo.com/v"}"#,
            br#"{"url":"https://googlevideo.com.evil.example/v"}"#,
            br#"{}"#,
            b"not json",
        ] {
            assert!(parse_resolution(output).is_err());
        }
    }
}
