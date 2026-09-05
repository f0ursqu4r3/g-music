use tracing_subscriber::EnvFilter;

use serde::Serialize;
use std::{process::Command, sync::atomic::AtomicBool, time::Duration};

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

    if let Err(error) = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init()
    {
        eprintln!("failed to initialize Rust logging: {error}");
    }
}

#[cfg(test)]
mod tests {
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
