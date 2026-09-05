//! Bounded child execution. Every exit path kills and reaps the owned child.
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub(crate) struct OwnedChild(pub(crate) Child);

static STOPPING: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_stopping() -> bool {
    STOPPING.load(Ordering::Acquire)
}
pub(crate) fn shutdown() {
    STOPPING.store(true, Ordering::Release);
}

pub(crate) fn spawn(command: &mut Command) -> std::io::Result<OwnedChild> {
    if is_stopping() {
        return Err(std::io::Error::other("service stopped"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map(OwnedChild)
}

pub(crate) fn kill(child: &mut Child) {
    #[cfg(unix)]
    // Each child created above owns its process group. Stop extractor helper processes too.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        kill(&mut self.0);
        let _ = self.0.wait();
    }
}

pub(crate) fn capture(
    command: &mut Command,
    cancelled: &AtomicBool,
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, &'static str> {
    if cancelled.load(Ordering::Acquire) || is_stopping() {
        return Err("Operation cancelled.");
    }
    let mut child = spawn(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()),
    )
    .map_err(|_| "Dependency could not start.")?;
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or("Dependency output is unavailable.")?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let started = Instant::now();
    let result = loop {
        if cancelled.load(Ordering::Acquire) || is_stopping() {
            break Err("Operation cancelled.");
        }
        if started.elapsed() >= timeout {
            break Err("Dependency timed out.");
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err("Dependency failed. Check its version and your connection.")
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => break Err("Dependency status is unavailable."),
        }
    };
    drop(child);
    let bytes = reader
        .join()
        .map_err(|_| "Dependency output is unavailable.")?
        .map_err(|_| "Dependency output is unavailable.")?;
    result?;
    if bytes.len() > limit {
        return Err("Dependency output exceeded the size limit.");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_reaps_a_running_child() {
        let directory = std::env::temp_dir().join(format!("gmusic-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let pid_path = directory.join("pid");
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let worker_path = pid_path.clone();
        let worker = thread::spawn(move || {
            let mut command = Command::new("/bin/sh");
            command
                .args(["-c", "echo $$ > \"$1\"; exec sleep 30", "fixture"])
                .arg(worker_path);
            capture(&mut command, &worker_cancelled, Duration::from_secs(3), 64)
        });
        let start = Instant::now();
        while !pid_path.exists() && start.elapsed() < Duration::from_secs(1) {
            thread::sleep(Duration::from_millis(10));
        }
        cancelled.store(true, Ordering::Release);
        assert_eq!(worker.join().unwrap(), Err("Operation cancelled."));
        let pid = std::fs::read_to_string(&pid_path).unwrap();
        let status = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert!(
            !status.success(),
            "cancelled child must be reaped before returning"
        );
    }

    #[test]
    fn timeout_kills_and_reaps_child() {
        let mut command = Command::new("sleep");
        command.arg("30");
        let start = Instant::now();
        assert_eq!(
            capture(
                &mut command,
                &AtomicBool::new(false),
                Duration::from_millis(40),
                64
            ),
            Err("Dependency timed out.")
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn cancellation_before_spawn_does_not_start_child() {
        assert_eq!(
            capture(
                &mut Command::new("nonexistent-test-command"),
                &AtomicBool::new(true),
                Duration::from_secs(1),
                64
            ),
            Err("Operation cancelled.")
        );
    }
}
