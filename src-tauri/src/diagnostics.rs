use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use serde::Serialize;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, atomic::AtomicBool},
    time::Duration,
};

const APP_IDENTIFIER: &str = "com.kyle.gmusic";
const LOG_FILE: &str = "gmusic.log";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSnapshot {
    app_version: &'static str,
    platform: &'static str,
    dependencies: Vec<DependencyDiagnostic>,
    audio_output_policy: &'static str,
}

#[derive(Serialize)]
pub struct DependencyDiagnostic {
    name: &'static str,
    available: bool,
    version: Option<String>,
    message: String,
}

pub fn unavailable() -> DiagnosticsSnapshot {
    DiagnosticsSnapshot {
        app_version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        dependencies: vec![],
        audio_output_policy: "Use the system default audio output. Select an output in system settings. Retry Play after reconnecting a device. Sleep and wake keep playback paused.",
    }
}

pub fn inspect() -> DiagnosticsSnapshot {
    let mut result = unavailable();
    for (name, variable, candidates) in [
        (
            "mpv",
            "GMUSIC_MPV_PATH",
            ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"],
        ),
        (
            "yt-dlp",
            "GMUSIC_YT_DLP_PATH",
            ["/opt/homebrew/bin/yt-dlp", "/usr/local/bin/yt-dlp"],
        ),
    ] {
        let executable = crate::playback::dependency_executable(variable, &candidates, name);
        let output = crate::process::capture(
            Command::new(executable).arg("--version"),
            &AtomicBool::new(false),
            Duration::from_secs(3),
            16 * 1024,
        );
        result
            .dependencies
            .push(dependency_diagnostic(name, output));
    }
    result
}

fn dependency_diagnostic(
    name: &'static str,
    output: Result<Vec<u8>, &'static str>,
) -> DependencyDiagnostic {
    // A successful probe establishes availability; version parsing is display-only.
    let available = output.is_ok();
    let version = output.ok().and_then(|output| safe_version(&output));
    let message = if available {
        "Available.".into()
    } else if cfg!(target_os = "macos") {
        format!("Install {name} with Homebrew: brew install {name}. Then restart G Music.")
    } else {
        format!(
            "Install {name} from its official distribution and add it to PATH. Then restart G Music."
        )
    };
    DependencyDiagnostic {
        name,
        available,
        version,
        message,
    }
}

fn safe_version(output: &[u8]) -> Option<String> {
    // Only return a version token, never arbitrary process output or an installation path.
    String::from_utf8_lossy(output)
        .split_whitespace()
        .take(3)
        .map(|token| token.strip_prefix('v').unwrap_or(token))
        .find(|token| {
            token.len() <= 40
                && token.starts_with(|c: char| c.is_ascii_digit())
                && token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-+_".contains(c))
        })
        .map(str::to_owned)
}

pub fn init() {
    let default_filter = if cfg!(debug_assertions) {
        "gmusic_lib=debug,gmusic=debug"
    } else {
        "gmusic_lib=info,gmusic=info"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let log_file = log_directory(
        std::env::consts::OS,
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .as_deref(),
    )
    .and_then(|directory| {
        open_log_file(&directory)
            .inspect_err(|error| eprintln!("failed to open the log file: {error}"))
            .ok()
    });
    let file_layer = log_file.map(|file| {
        fmt::layer()
            .with_ansi(false)
            .with_target(false)
            .compact()
            .with_writer(Mutex::new(file))
    });

    if let Err(error) = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false).compact())
        .with(file_layer)
        .try_init()
    {
        eprintln!("failed to initialize Rust logging: {error}");
    }
}

/// Mirrors Tauri's app log directory, which is not available before the app starts.
fn log_directory(os: &str, home: Option<&Path>, xdg_data_home: Option<&Path>) -> Option<PathBuf> {
    if os == "macos" {
        return Some(home?.join("Library/Logs").join(APP_IDENTIFIER));
    }
    let data_home = match xdg_data_home {
        Some(path) => path.to_path_buf(),
        None => home?.join(".local/share"),
    };
    Some(data_home.join(APP_IDENTIFIER).join("logs"))
}

/// Starts a new log and keeps the previous run as `gmusic.log.1`.
fn open_log_file(directory: &Path) -> io::Result<File> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join(LOG_FILE);
    match std::fs::rename(&path, directory.join(format!("{LOG_FILE}.1"))) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    File::create(path)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    #[test]
    fn log_directories_follow_platform_conventions() {
        let home = Some(Path::new("/home/user"));
        assert_eq!(
            super::log_directory("macos", home, None),
            Some(PathBuf::from("/home/user/Library/Logs/com.kyle.gmusic"))
        );
        assert_eq!(
            super::log_directory("linux", home, None),
            Some(PathBuf::from(
                "/home/user/.local/share/com.kyle.gmusic/logs"
            ))
        );
        assert_eq!(
            super::log_directory("linux", home, Some(Path::new("/data"))),
            Some(PathBuf::from("/data/com.kyle.gmusic/logs"))
        );
        assert_eq!(super::log_directory("linux", None, None), None);
    }

    #[test]
    fn opening_the_log_keeps_the_previous_run() {
        let directory = std::env::temp_dir().join(format!("gmusic-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("gmusic.log"), "previous run").unwrap();

        let file = super::open_log_file(&directory).unwrap();
        drop(file);

        assert_eq!(
            std::fs::read_to_string(directory.join("gmusic.log.1")).unwrap(),
            "previous run"
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("gmusic.log")).unwrap(),
            ""
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
    #[test]
    fn successful_probe_is_available_even_without_a_known_version_format() {
        for output in [b"mpv custom-build".as_slice(), b""] {
            let diagnostic = super::dependency_diagnostic("mpv", Ok(output.to_vec()));
            assert!(diagnostic.available);
            assert_eq!(diagnostic.version, None);
            assert_eq!(diagnostic.message, "Available.");
        }
    }

    #[test]
    fn failed_probe_is_unavailable() {
        let diagnostic = super::dependency_diagnostic("mpv", Err("Dependency could not start."));
        assert!(!diagnostic.available);
        assert_eq!(diagnostic.version, None);
    }

    #[test]
    fn recognizes_mpv_v_prefixed_version() {
        assert_eq!(
            super::safe_version(
                b"mpv v0.41.0 Copyright 2000-2025 mpv/MPlayer/mplayer2 projects\nlibplacebo version: v7.360.1"
            ),
            Some("0.41.0".into())
        );
    }

    #[test]
    #[ignore = "requires an installed mpv executable"]
    fn installed_mpv_is_reported_available() {
        let snapshot = super::inspect();
        let mpv = snapshot
            .dependencies
            .iter()
            .find(|item| item.name == "mpv")
            .unwrap();
        assert!(mpv.available, "{}", mpv.message);
        assert!(
            mpv.version.is_some(),
            "installed mpv version should be recognized"
        );
        println!("{}", serde_json::to_string(&snapshot).unwrap());
    }

    #[test]
    fn versions_exclude_paths_and_provider_messages() {
        assert_eq!(
            super::safe_version(b"mpv 0.40.0 Copyright /private/path"),
            Some("0.40.0".into())
        );
        assert_eq!(
            super::safe_version(b"ERROR https://private/url /Users/private"),
            None
        );
        for output in [
            b"mpv v0.41.0/private/path".as_slice(),
            b"mpv v0.41.0?token=private",
            b"mpv version-unknown",
        ] {
            assert_eq!(super::safe_version(output), None);
        }
        assert_eq!(
            super::safe_version(b"2026.08.19\n"),
            Some("2026.08.19".into())
        );
    }
}
