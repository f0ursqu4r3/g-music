use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::json;

use super::{
    Shared, VisualizerSnapshot,
    analysis::{AnalysisBuffer, FRAME_MS},
    ipc::Ipc,
    pipes::{FILTER_LABEL, Pipes, filter_graph},
};

fn check_filters(executable: &Path, stopped: &AtomicBool) -> bool {
    // Test the installed mpv's actual filter support before modifying real playback.
    let Ok(mut child) = Command::new(executable)
        .args([
            "--no-config",
            "--no-terminal",
            "--ao=null",
            "--video=no",
            "--audio-display=no",
            "--demuxer=rawaudio",
            "--demuxer-rawaudio-format=s16le",
            "--demuxer-rawaudio-rate=48000",
            "--demuxer-rawaudio-channels=stereo",
        ])
        .arg(format!("--af={}", filter_graph(None)))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut input| input.write_all(&[0; 3840]).is_ok());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return wrote && status.success(),
            Err(_) => break,
            _ => {}
        }
        if stopped.load(Ordering::Acquire) || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    false
}

fn reset(pipes: &mut Pipes, frames: &mut AnalysisBuffer) {
    for reader in &mut pipes.readers {
        reader.reset();
    }
    frames.clear();
}

fn connect(socket: &Path) -> Result<Ipc, String> {
    let mut ipc = Ipc::connect(socket)?;
    for (id, property) in ["pause", "core-idle", "idle-active"]
        .into_iter()
        .enumerate()
    {
        ipc.command(json!(["observe_property", id, property]))?;
    }
    Ok(ipc)
}

/// Keep the process-owned worker alive after IPC failure. Explicit Retry gets a
/// fresh connection and replaces the old filter, including any failed AVIO state.
fn wait_for_retry(
    socket: &Path,
    shared: &Shared,
    stopped: &AtomicBool,
    mut failed_retry: u64,
) -> Option<Ipc> {
    let mut generation = None;
    while !stopped.load(Ordering::Acquire) {
        let current_generation = shared.generation.load(Ordering::Acquire);
        if Some(current_generation) != generation {
            generation = Some(current_generation);
            shared.error(
                current_generation,
                "The audio connection is unavailable. Restart playback, then select Retry.",
            );
        }
        let retry = shared.retry.load(Ordering::Acquire);
        if shared.active() && retry != failed_retry {
            match connect(socket) {
                Ok(mut ipc) => {
                    let _ = ipc.command(json!(["af", "remove", format!("@{FILTER_LABEL}")]));
                    return Some(ipc);
                }
                Err(_) => {
                    shared.error(
                        shared.generation.load(Ordering::Acquire),
                        "The audio connection is unavailable. Restart playback, then select Retry.",
                    );
                    failed_retry = retry;
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    None
}

struct ClockProgress {
    previous: Option<f64>,
    advanced: Instant,
}

impl ClockProgress {
    fn new() -> Self {
        Self {
            previous: None,
            advanced: Instant::now(),
        }
    }
    fn observe(&mut self, position: Option<f64>, now: Instant) -> bool {
        if position.is_some() && position != self.previous {
            self.advanced = now;
        }
        self.previous = position;
        position.is_some() && now.duration_since(self.advanced) < Duration::from_millis(120)
    }
}

pub(super) fn run(
    mut pipes: Pipes,
    socket: PathBuf,
    executable: PathBuf,
    shared: Arc<Shared>,
    stopped: Arc<AtomicBool>,
) {
    let mut generation = shared.invalidate("idle");
    let mut ipc = match connect(&socket) {
        Ok(ipc) => ipc,
        Err(_) => {
            shared.error(
                generation,
                "The audio analysis connection could not start. Select Retry to reconnect.",
            );
            let Some(ipc) = wait_for_retry(
                &socket,
                &shared,
                &stopped,
                shared.retry.load(Ordering::Acquire),
            ) else {
                return;
            };
            ipc
        }
    };
    let mut frames = AnalysisBuffer::default();
    let mut installed = false;
    let mut checked = false;
    let mut failed = false;
    let mut retry = shared.retry.load(Ordering::Acquire);
    let mut paused = true;
    let mut core_idle = true;
    let mut idle = true;
    let mut waiting_restart = false;
    let mut last_complete = Instant::now();
    let mut complete_position = None;
    let mut progress = ClockProgress::new();
    while !stopped.load(Ordering::Acquire) {
        let next_retry = shared.retry.load(Ordering::Acquire);
        if retry != next_retry {
            failed = false;
            retry = next_retry;
        }
        let active = shared.active();
        if installed && !active {
            if ipc
                .command(json!(["af", "remove", format!("@{FILTER_LABEL}")]))
                .is_err()
            {
                shared.error(
                    generation,
                    "Audio analysis could not stop. Select Retry to reconnect.",
                );
                let Some(connection) = wait_for_retry(&socket, &shared, &stopped, retry) else {
                    break;
                };
                ipc = connection;
            }
            installed = false;
            reset(&mut pipes, &mut frames);
            generation = shared.invalidate("idle");
        }
        if active && !installed && !failed {
            if !checked {
                checked = check_filters(&executable, &stopped);
                if !checked {
                    shared.error(generation, "This mpv installation cannot run the visualizer filters. Update mpv, then select Retry.");
                    failed = true;
                }
            }
            if checked && shared.active() && !stopped.load(Ordering::Acquire) {
                if ipc.command(json!(["af", "add", pipes.graph])).is_ok() {
                    installed = true;
                    reset(&mut pipes, &mut frames);
                    generation = shared.invalidate("warming");
                    waiting_restart = false;
                    last_complete = Instant::now();
                    complete_position = None;
                    progress = ClockProgress::new();
                } else {
                    shared.error(
                        generation,
                        "Audio analysis could not start. Music playback is still available.",
                    );
                    failed = true;
                }
            }
        }
        let external_generation = shared.generation.load(Ordering::Acquire);
        if generation != external_generation {
            generation = external_generation;
            reset(&mut pipes, &mut frames);
            waiting_restart = true;
            last_complete = Instant::now();
            complete_position = None;
            progress = ClockProgress::new();
            if failed {
                shared.error(
                    generation,
                    "Audio analysis is unavailable. Select Retry to reconnect.",
                );
            }
        }
        let observed = (|| -> Result<_, String> {
            let clock = if installed {
                ipc.command(json!(["get_property", "audio-pts"]))?
                    .as_f64()
                    .filter(|time| time.is_finite())
                    .map(|time| time * 1000.0)
            } else {
                None
            };
            Ok((clock, ipc.events()?))
        })();
        let (clock, events) = match observed {
            Ok(observed) => observed,
            Err(_) => {
                shared.error(
                    generation,
                    "The audio playback clock disconnected. Select Retry to reconnect.",
                );
                let Some(connection) = wait_for_retry(&socket, &shared, &stopped, retry) else {
                    break;
                };
                ipc = connection;
                installed = false;
                failed = false;
                reset(&mut pipes, &mut frames);
                generation = shared.invalidate("warming");
                paused = true;
                core_idle = true;
                idle = true;
                waiting_restart = false;
                continue;
            }
        };
        let was_suspended = paused || core_idle || idle;
        let mut discontinuity = false;
        for event in events {
            match event["event"].as_str() {
                Some("property-change") => match event["name"].as_str() {
                    Some("pause") => paused = event["data"].as_bool().unwrap_or(true),
                    Some("core-idle") => core_idle = event["data"].as_bool().unwrap_or(true),
                    Some("idle-active") => idle = event["data"].as_bool().unwrap_or(true),
                    _ => {}
                },
                Some("start-file" | "seek" | "end-file") => {
                    discontinuity = true;
                    waiting_restart = true;
                }
                Some("playback-restart") => {
                    discontinuity = true;
                    waiting_restart = false;
                }
                // mpv can emit this AFTER playback-restart when a filter is added.
                // Reset measurements, but do not wait for a second restart event.
                Some("audio-reconfig") => discontinuity = true,
                _ => {}
            }
        }
        if discontinuity {
            reset(&mut pipes, &mut frames);
            generation = shared.invalidate(if failed {
                "error"
            } else if installed {
                "warming"
            } else {
                "idle"
            });
            last_complete = Instant::now();
            complete_position = None;
            progress = ClockProgress::new();
        }
        if was_suspended && !paused && !core_idle && !idle {
            // A pause or a buffering interval must not consume the analysis timeout.
            // mpv can resume buffered audio before producing the next analysis frame.
            last_complete = Instant::now();
            progress = ClockProgress::new();
        }
        for (band, reader) in pipes.readers.iter_mut().enumerate() {
            match reader.drain(band, &mut frames) {
                Ok(_) => {}
                Err(_) => {
                    failed = true;
                }
            }
        }
        if installed {
            let mut available = clock
                .map(|position| frames.frames(position))
                .unwrap_or_default();
            if let Some(latest) = available.last().map(|frame| frame.position_ms)
                && complete_position != Some(latest)
            {
                complete_position = Some(latest);
                last_complete = Instant::now();
            }
            if waiting_restart || discontinuity {
                available.clear();
            }
            let covered = clock.is_some_and(|position| {
                available.iter().any(|frame| {
                    frame.position_ms <= position && position < frame.position_ms + FRAME_MS + 0.1
                })
            });
            let advancing = progress.observe(clock, Instant::now());
            let status = if idle {
                "idle"
            } else if paused {
                "paused"
            } else if core_idle || !advancing {
                "buffering"
            } else if covered {
                "playing"
            } else {
                "warming"
            };
            if failed
                || (!paused
                    && !idle
                    && !core_idle
                    && last_complete.elapsed() > Duration::from_secs(3))
            {
                let _ = ipc.command(json!(["af", "remove", format!("@{FILTER_LABEL}")]));
                installed = false;
                failed = true;
                shared.error(
                    generation,
                    "Audio analysis stopped. Select Retry to reconnect the visualizer.",
                );
            } else {
                shared.publish(VisualizerSnapshot {
                    generation,
                    status,
                    error: None,
                    audio_position_ms: if discontinuity { None } else { clock },
                    frames: available,
                });
            }
        }
        thread::sleep(Duration::from_millis(if installed { 15 } else { 80 }));
    }
    // Also attempt removal on an IPC failure. The native player still owns its audio.
    if installed {
        let _ = ipc.command(json!(["af", "remove", format!("@{FILTER_LABEL}")]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_error_survives_seek_before_and_during_retry_wait() {
        let shared = Shared::default();
        let stopped = AtomicBool::new(false);
        shared.invalidate("warming");
        let states = thread::scope(|scope| {
            let worker =
                scope.spawn(|| wait_for_retry(Path::new("unused.sock"), &shared, &stopped, 0));
            let wait_for_error = || {
                let deadline = Instant::now() + Duration::from_millis(250);
                while shared.inspect().status != "error" && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(5));
                }
                shared.inspect()
            };
            let initial = wait_for_error();
            let generation = shared.invalidate("warming");
            let after_seek = wait_for_error();
            stopped.store(true, Ordering::Release);
            worker.join().unwrap();
            (initial, after_seek, generation)
        });
        assert_eq!(states.0.status, "error");
        assert!(states.0.error.is_some());
        assert_eq!(states.1.status, "error");
        assert_eq!(states.1.generation, states.2);
    }

    #[test]
    fn a_frozen_clock_is_not_kept_alive_by_successful_queries() {
        let mut progress = ClockProgress::new();
        let now = Instant::now();
        assert!(progress.observe(Some(10.0), now));
        assert!(!progress.observe(Some(10.0), now + Duration::from_millis(121)));
        assert!(progress.observe(Some(20.0), now + Duration::from_millis(140)));
        assert!(!progress.observe(None, now + Duration::from_millis(150)));
    }
}
