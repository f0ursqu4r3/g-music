use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::blocking::Client;
use tauri::{AppHandle, Manager};

const ARTWORK_CACHE_LIMIT: u64 = 512 * 1024 * 1024;
const ARTWORK_MAX_BYTES: u64 = 8 * 1024 * 1024;
const QUALITIES: [&str; 2] = ["maxresdefault", "hqdefault"];

pub(crate) fn resolve_youtube_artwork(
    app: &AppHandle,
    video_id: &str,
) -> Result<Option<String>, String> {
    if !is_youtube_video_id(video_id) {
        return Ok(None);
    }
    let application_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    resolve_youtube_artwork_in(&application_directory, video_id)
}

fn resolve_youtube_artwork_in(
    application_directory: &Path,
    video_id: &str,
) -> Result<Option<String>, String> {
    let database_path = application_directory.join("library.sqlite3");
    let artwork_directory = application_directory.join("artwork");
    fs::create_dir_all(&artwork_directory).map_err(|error| error.to_string())?;
    for quality in QUALITIES {
        let cache_key = format!("youtube:{video_id}:{quality}");
        let relative_path = format!("artwork/{video_id}-{quality}.jpg");
        let cached_path = application_directory.join(&relative_path);
        if crate::persistence::read_artwork_cache(&database_path, &cache_key, now_epoch_ms())?
            .is_some()
        {
            if is_valid_cached_image(&cached_path) {
                return Ok(Some(cached_path.to_string_lossy().into()));
            }
            let _ = fs::remove_file(&cached_path);
            crate::persistence::remove_artwork_cache(&database_path, &cache_key)?;
        }

        let Some(bytes) = fetch_artwork(video_id, quality)? else {
            continue;
        };
        write_image_atomically(&cached_path, &bytes)?;
        let evicted = crate::persistence::write_artwork_cache(
            &database_path,
            &cache_key,
            &relative_path,
            bytes.len() as u64,
            now_epoch_ms(),
            ARTWORK_CACHE_LIMIT,
        )?;
        for relative_path in evicted {
            let _ = fs::remove_file(application_directory.join(relative_path));
        }
        return Ok(Some(cached_path.to_string_lossy().into()));
    }
    Ok(None)
}

fn fetch_artwork(video_id: &str, quality: &str) -> Result<Option<Vec<u8>>, String> {
    let url = format!("https://i.ytimg.com/vi/{video_id}/{quality}.jpg");
    let response = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("image/") {
        return Ok(None);
    }
    let bytes = response.bytes().map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > ARTWORK_MAX_BYTES as usize || !is_image_signature(&bytes) {
        return Ok(None);
    }
    Ok(Some(bytes.to_vec()))
}

fn write_image_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary_path = path.with_extension(format!("tmp-{}", now_epoch_ms()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())?;
    fs::rename(temporary_path, path).map_err(|error| error.to_string())
}

fn is_valid_cached_image(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    !bytes.is_empty() && bytes.len() <= ARTWORK_MAX_BYTES as usize && is_image_signature(&bytes)
}

fn is_image_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn is_youtube_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{is_image_signature, is_youtube_video_id};

    #[test]
    fn accepts_only_stable_youtube_video_ids() {
        assert!(is_youtube_video_id("M7lc1UVf-VE"));
        assert!(!is_youtube_video_id("../bad"));
    }

    #[test]
    fn validates_supported_image_signatures() {
        assert!(is_image_signature(&[0xff, 0xd8, 0xff, 0xe0]));
        assert!(is_image_signature(b"\x89PNG\r\n\x1a\nimage"));
        assert!(!is_image_signature(b"not an image"));
    }
}
