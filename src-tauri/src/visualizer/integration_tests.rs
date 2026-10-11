//! Explicitly run with `cargo test visualizer::integration_tests -- --ignored`.
//! These tests use synthetic audio and mpv's silent null output.
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use super::{Shared, Tap, VisualizerSnapshot, ipc::Ipc};
use serde_json::json;

struct Fixture {
    directory: PathBuf,
    child: Child,
    tap: Option<Tap>,
    ipc: Ipc,
    shared: Arc<Shared>,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "gmusic-visualizer-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let input = directory.join("tones.wav");
        let socket = directory.join("mpv.sock");
        let _ = fs::remove_file(&socket);
        let rate = 48000u32;
        let data_length = rate * 12 * 4;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for index in 0..rate * 12 {
            let t = index as f64 / rate as f64;
            let frequency = [90.0, 800.0, 6000.0][(t as usize / 2) % 3];
            let sample = (12000.0 * (std::f64::consts::TAU * frequency * t).sin()) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        fs::write(&input, bytes).unwrap();
        let executable = crate::playback::dependency_executable(
            "GMUSIC_MPV_PATH",
            &["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"],
            "mpv",
        );
        let shared = Arc::new(Shared::default());
        let mut command = Command::new(&executable);
        command
            .args([
                "--no-config",
                "--no-terminal",
                "--idle=yes",
                "--pause=yes",
                "--video=no",
                "--ao=null",
            ])
            .arg(format!("--input-ipc-server={}", socket.display()))
            .arg(input)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut tap = Tap::prepare_with(&mut command, shared.clone()).unwrap();
        let child = command
            .spawn()
            .expect("installed mpv is required for this ignored test");
        let deadline = Instant::now() + Duration::from_secs(5);
        let ipc = loop {
            if let Ok(ipc) = Ipc::connect(&socket) {
                break ipc;
            }
            assert!(Instant::now() < deadline, "mpv startup timed out");
            thread::sleep(Duration::from_millis(20));
        };
        tap.start(&socket, &executable);
        Self {
            directory,
            child,
            tap: Some(tap),
            ipc,
            shared,
        }
    }

    fn wait(&self, predicate: impl Fn(&VisualizerSnapshot) -> bool) -> VisualizerSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = self.shared.inspect();
            assert_ne!(snapshot.status, "error", "{:?}", snapshot);
            if predicate(&snapshot) {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "snapshot timed out: {snapshot:?}"
            );
            thread::sleep(Duration::from_millis(15));
        }
    }
}

#[test]
#[ignore = "requires installed mpv; writes synthetic PCM to a temporary test directory"]
fn analysis_branch_preserves_pcm_samples() {
    let fixture = Fixture::new();
    let executable = crate::playback::dependency_executable(
        "GMUSIC_MPV_PATH",
        &["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"],
        "mpv",
    );
    for analyzed in [false, true] {
        let output = fixture.directory.join(if analyzed {
            "analyzed.pcm"
        } else {
            "original.pcm"
        });
        let mut command = Command::new(&executable);
        command
            .args([
                "--no-config",
                "--no-terminal",
                "--video=no",
                "--audio-display=no",
                "--ao=pcm",
                "--ao-pcm-waveheader=no",
                "--audio-format=s16",
                "--audio-samplerate=48000",
                "--audio-channels=stereo",
            ])
            .arg(format!("--ao-pcm-file={}", output.display()));
        if analyzed {
            command.arg(format!("--af={}", super::pipes::filter_graph(None)));
        }
        let status = command
            .arg(fixture.directory.join("tones.wav"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }
    let original = fs::read(fixture.directory.join("original.pcm")).unwrap();
    let analyzed = fs::read(fixture.directory.join("analyzed.pcm")).unwrap();
    assert!(!original.is_empty());
    assert_eq!(original.len(), analyzed.len());
    let differences = original
        .as_chunks::<2>()
        .0
        .iter()
        .zip(analyzed.as_chunks::<2>().0.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differences, 0, "analysis changed playback PCM samples");
}

#[test]
#[ignore = "requires installed mpv; deliberately stalls analysis readers with silent output"]
fn pipe_overflow_keeps_audio_running_and_filter_reinstall_restores_measurements() {
    let fixture = Fixture::new();
    let executable = crate::playback::dependency_executable(
        "GMUSIC_MPV_PATH",
        &["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"],
        "mpv",
    );
    let socket = fixture.directory.join("overflow.sock");
    let mut pipes = super::pipes::Pipes::new().unwrap();
    let mut command = Command::new(executable);
    command
        .args([
            "--no-config",
            "--no-terminal",
            "--idle=yes",
            "--video=no",
            "--ao=null",
            "--pause=yes",
        ])
        .arg(format!("--input-ipc-server={}", socket.display()))
        .arg(format!("--af={}", pipes.graph))
        .arg(fixture.directory.join("tones.wav"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    pipes.configure(&mut command);
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _child = ChildGuard(command.spawn().unwrap());
    pipes.spawned();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ipc = loop {
        if let Ok(ipc) = Ipc::connect(&socket) {
            break ipc;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    };
    // No reader drains the sixteen pipes while the null audio driver advances.
    ipc.command(json!(["set_property", "pause", false]))
        .unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(7) {
        ipc.command(json!(["get_property", "audio-pts"])).unwrap();
        ipc.events().unwrap();
        thread::sleep(Duration::from_millis(40));
    }
    let position = ipc
        .command(json!(["get_property", "audio-pts"]))
        .unwrap()
        .as_f64()
        .unwrap();
    assert!(position > 6.5, "analysis backpressure must not stop audio");
    for reader in &mut pipes.readers {
        reader.reset();
    }
    ipc.command(json!([
        "af",
        "remove",
        format!("@{}", super::pipes::FILTER_LABEL)
    ]))
    .unwrap();
    ipc.command(json!(["af", "add", pipes.graph])).unwrap();
    let mut buffer = super::analysis::AnalysisBuffer::default();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        for (band, reader) in pipes.readers.iter_mut().enumerate() {
            reader.drain(band, &mut buffer).unwrap();
        }
        let position = ipc
            .command(json!(["get_property", "audio-pts"]))
            .unwrap()
            .as_f64()
            .unwrap()
            * 1000.0;
        ipc.events().unwrap();
        if buffer
            .frames(position)
            .iter()
            .any(|frame| frame.position_ms <= position && position < frame.position_ms + 20.1)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "filter reinstall did not restore all sixteen measurements"
        );
        thread::sleep(Duration::from_millis(15));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.tap.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
#[ignore = "requires installed mpv; uses only silent synthetic audio"]
fn real_mpv_analyzes_audio_and_recovers_from_pause_seek_and_window_toggle() {
    let mut fixture = Fixture::new();
    fixture.shared.activate(true);
    fixture.wait(|value| value.status == "paused");
    fixture
        .ipc
        .command(json!(["set_property", "pause", false]))
        .unwrap();
    let first = fixture.wait(|value| {
        value.status == "playing"
            && value
                .audio_position_ms
                .is_some_and(|position| position > 600.0)
    });
    assert!(first.frames.iter().all(|frame| frame.bands.len() == 16));
    let frame = first
        .frames
        .iter()
        .find(|frame| {
            frame.position_ms <= first.audio_position_ms.unwrap()
                && first.audio_position_ms.unwrap() < frame.position_ms + 20.1
        })
        .unwrap();
    let loudest = frame
        .bands
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .unwrap()
        .0;
    assert_eq!(loudest, 2, "90 Hz should dominate the 80–120 Hz band");

    fixture
        .ipc
        .command(json!(["set_property", "pause", true]))
        .unwrap();
    let paused = fixture.wait(|value| value.status == "paused");
    let pause_deadline = Instant::now() + Duration::from_millis(3300);
    while Instant::now() < pause_deadline {
        fixture.shared.inspect(); // Renew the window lease during the long pause.
        thread::sleep(Duration::from_millis(50));
    }
    let still = fixture.shared.inspect();
    assert!((still.audio_position_ms.unwrap() - paused.audio_position_ms.unwrap()).abs() < 2.0);
    fixture
        .ipc
        .command(json!(["set_property", "pause", false]))
        .unwrap();
    fixture.wait(|value| value.status == "playing");

    for target in [4.0, 2.0, 2.0] {
        let generation = fixture.shared.invalidate("warming");
        fixture
            .ipc
            .command(json!(["seek", target, "absolute+exact"]))
            .unwrap();
        fixture
            .ipc
            .command(json!(["set_property", "pause", false]))
            .unwrap();
        let sample = fixture.wait(|value| {
            value.generation >= generation
                && value.status == "playing"
                && value.audio_position_ms.is_some_and(|position| {
                    position >= target * 1000.0 && position < target * 1000.0 + 1500.0
                })
        });
        assert!(
            sample
                .frames
                .iter()
                .all(|frame| frame.position_ms >= target * 1000.0 - 1.0)
        );
    }
    fixture.shared.activate(false);
    fixture.wait(|value| value.status == "idle");
    let before = fixture
        .ipc
        .command(json!(["get_property", "audio-pts"]))
        .unwrap()
        .as_f64()
        .unwrap();
    thread::sleep(Duration::from_millis(100));
    let after = fixture
        .ipc
        .command(json!(["get_property", "audio-pts"]))
        .unwrap()
        .as_f64()
        .unwrap();
    assert!(
        after > before,
        "closing visualization must preserve playback"
    );
    fixture.shared.activate(true);
    fixture.wait(|value| value.status == "playing");
    fixture
        .ipc
        .command(json!(["set_property", "pause", true]))
        .unwrap();
    fixture.wait(|value| value.status == "paused");
    fixture.tap.take();
    let ended = fixture.shared.inspect();
    assert_eq!(ended.status, "idle");
    assert!(ended.frames.is_empty());
}
