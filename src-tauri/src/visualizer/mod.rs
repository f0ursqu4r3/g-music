mod analysis;
#[cfg(test)]
mod integration_tests;
mod ipc;
mod pipes;
pub(crate) mod video;
mod worker;

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use analysis::AnalysisFrame;
use pipes::Pipes;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerSnapshot {
    pub generation: u64,
    pub status: &'static str,
    pub error: Option<String>,
    pub audio_position_ms: Option<f64>,
    pub frames: Vec<AnalysisFrame>,
}

impl VisualizerSnapshot {
    fn empty(generation: u64, status: &'static str) -> Self {
        Self {
            generation,
            status,
            error: None,
            audio_position_ms: None,
            frames: Vec::new(),
        }
    }
}

struct Shared {
    demand: Mutex<Option<Instant>>,
    snapshot: Mutex<(VisualizerSnapshot, Instant)>,
    generation: AtomicU64,
    retry: AtomicU64,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            demand: Mutex::new(None),
            snapshot: Mutex::new((VisualizerSnapshot::empty(1, "idle"), Instant::now())),
            generation: AtomicU64::new(1),
            retry: AtomicU64::new(0),
        }
    }
}

impl Shared {
    fn active(&self) -> bool {
        self.demand
            .lock()
            .is_ok_and(|lease| lease.is_some_and(|time| time.elapsed() < Duration::from_secs(2)))
    }

    fn activate(&self, active: bool) {
        if let Ok(mut demand) = self.demand.lock() {
            *demand = active.then(Instant::now);
        }
        self.retry.fetch_add(1, Ordering::AcqRel);
    }

    fn invalidate(&self, status: &'static str) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.publish(VisualizerSnapshot::empty(generation, status));
        generation
    }

    fn publish(&self, snapshot: VisualizerSnapshot) {
        if let Ok(mut target) = self.snapshot.lock()
            && snapshot.generation == self.generation.load(Ordering::Acquire)
        {
            *target = (snapshot, Instant::now());
        }
    }

    fn error(&self, generation: u64, message: &str) {
        let mut snapshot = VisualizerSnapshot::empty(generation, "error");
        snapshot.error = Some(message.to_string());
        self.publish(snapshot);
    }

    fn inspect(&self) -> VisualizerSnapshot {
        if let Ok(mut demand) = self.demand.lock()
            && demand.is_some()
        {
            *demand = Some(Instant::now());
        }
        let Ok((mut snapshot, observed)) = self.snapshot.lock().map(|value| value.clone()) else {
            return VisualizerSnapshot::empty(self.generation.load(Ordering::Acquire), "error");
        };
        if snapshot.status == "playing" {
            let age = observed.elapsed();
            if age > Duration::from_millis(120) {
                snapshot.status = "buffering";
                snapshot.frames.clear();
            } else if let Some(position) = &mut snapshot.audio_position_ms {
                *position += age.as_secs_f64() * 1000.0;
            }
        }
        snapshot
    }
}

fn shared() -> Arc<Shared> {
    static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
    SHARED.get_or_init(|| Arc::new(Shared::default())).clone()
}

/// Invalidate before a transport command, so an in-flight inspection cannot expose
/// the previous track or seek even before mpv's event reaches the analysis worker.
pub(crate) fn invalidate() {
    shared().invalidate("warming");
}

#[tauri::command]
pub fn set_visualizer_active(window: tauri::WebviewWindow, active: bool) -> Result<(), String> {
    if window.label() != "visualizer" {
        return Err("Only the Visualizer window can start audio analysis.".into());
    }
    shared().activate(active);
    Ok(())
}

#[tauri::command]
pub fn inspect_visualizer(window: tauri::WebviewWindow) -> Result<VisualizerSnapshot, String> {
    if window.label() != "visualizer" {
        return Err("Only the Visualizer window can inspect audio analysis.".into());
    }
    Ok(shared().inspect())
}

/// Owned by the mpv process, not by a window or a temporary transport snapshot.
pub(crate) struct Tap {
    pipes: Option<Pipes>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    state: Arc<Shared>,
}

impl Tap {
    pub fn prepare(command: &mut Command) -> Option<Self> {
        Self::prepare_with(command, shared())
    }

    fn prepare_with(command: &mut Command, state: Arc<Shared>) -> Option<Self> {
        match Pipes::new() {
            Ok(pipes) => {
                pipes.configure(command);
                Some(Self {
                    pipes: Some(pipes),
                    stopped: Arc::new(AtomicBool::new(false)),
                    worker: None,
                    state,
                })
            }
            Err(_) => {
                state.error(
                    state.generation.load(Ordering::Acquire),
                    "Audio analysis could not start. Music playback is still available.",
                );
                None
            }
        }
    }

    pub fn start(&mut self, socket: &Path, executable: &Path) {
        let Some(mut pipes) = self.pipes.take() else {
            return;
        };
        pipes.spawned();
        let stopped = self.stopped.clone();
        let state = self.state.clone();
        let socket = PathBuf::from(socket);
        let executable = PathBuf::from(executable);
        self.worker = Some(std::thread::spawn(move || {
            worker::run(pipes, socket, executable, state, stopped)
        }));
    }
}

impl Drop for Tap {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
            self.state.invalidate("idle");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_generation_cannot_publish_after_seek() {
        let shared = Shared::default();
        let old = shared.generation.load(Ordering::Acquire);
        let new = shared.invalidate("warming");
        shared.publish(VisualizerSnapshot::empty(old, "playing"));
        let snapshot = shared.inspect();
        assert_eq!(snapshot.generation, new);
        assert_eq!(snapshot.status, "warming");
    }

    #[test]
    fn stale_clock_stops_extrapolation_but_pause_keeps_position() {
        let shared = Shared::default();
        let mut value = VisualizerSnapshot::empty(1, "playing");
        value.audio_position_ms = Some(1000.0);
        *shared.snapshot.lock().unwrap() = (value.clone(), Instant::now() - Duration::from_secs(1));
        assert_eq!(shared.inspect().status, "buffering");
        value.status = "paused";
        *shared.snapshot.lock().unwrap() = (value, Instant::now() - Duration::from_secs(1));
        assert_eq!(shared.inspect().audio_position_ms, Some(1000.0));
    }
}
