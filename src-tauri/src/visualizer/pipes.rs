use std::{
    fs::File,
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::process::CommandExt,
    },
    process::Command,
};

use super::analysis::{AnalysisBuffer, BAND_COUNT, MeasurementParser};

pub(super) const FILTER_LABEL: &str = "gmusic_visualizer";

pub(super) fn filter_graph(descriptors: Option<&[RawFd]>) -> String {
    let mut graph = "[in]asplit=2[out][analysis];[analysis]aresample=48000,asetnsamples=n=960:p=0,acrossover=split='50 80 120 180 270 400 600 900 1300 2000 3000 4500 6500 9500 14000':order=4th".to_string();
    for band in 0..BAND_COUNT {
        graph.push_str(&format!("[b{band}]"));
    }
    for band in 0..BAND_COUNT {
        let destination = descriptors.map_or_else(
            || "/dev/null".to_string(),
            |fds| format!("'pipe\\:{}'", fds[band]),
        );
        graph.push_str(&format!(";[b{band}]astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,ametadata=mode=print:key=lavfi.astats.Overall.RMS_level:file={destination}:direct=1,anullsink"));
    }
    format!("@{FILTER_LABEL}:lavfi=[{graph}]")
}

pub(super) struct Pipes {
    pub readers: Vec<PipeReader>,
    writers: Vec<File>,
    pub graph: String,
}

impl Pipes {
    pub fn new() -> io::Result<Self> {
        let mut readers = Vec::with_capacity(BAND_COUNT);
        let mut writers = Vec::with_capacity(BAND_COUNT);
        for _ in 0..BAND_COUNT {
            let mut fds = [-1; 2];
            // SAFETY: pipe writes exactly two valid descriptors into this array.
            if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: Each descriptor is newly created and has one owner.
            let reader = unsafe { File::from_raw_fd(fds[0]) };
            let writer = unsafe { File::from_raw_fd(fds[1]) };
            for file in [&reader, &writer] {
                // Both ends are nonblocking. A stalled reader cannot stop mpv's audio thread.
                // SAFETY: fcntl receives a live descriptor and integer flags.
                if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0
                    || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            readers.push(PipeReader {
                file: reader,
                bytes: Vec::new(),
                parser: MeasurementParser::default(),
                discarding: false,
            });
            writers.push(writer);
        }
        let descriptors: Vec<_> = writers.iter().map(AsRawFd::as_raw_fd).collect();
        Ok(Self {
            readers,
            writers,
            graph: filter_graph(Some(&descriptors)),
        })
    }

    pub fn configure(&self, command: &mut Command) {
        let descriptors: Vec<_> = self.writers.iter().map(AsRawFd::as_raw_fd).collect();
        // SAFETY: Only async-signal-safe fcntl calls run between fork and exec.
        // Clear CLOEXEC for the write ends only, which the spawned mpv must inherit.
        unsafe {
            command.pre_exec(move || {
                for fd in &descriptors {
                    if libc::fcntl(*fd, libc::F_SETFD, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
    }

    pub fn spawned(&mut self) {
        self.writers.clear();
    }
}

pub(super) struct PipeReader {
    file: File,
    bytes: Vec<u8>,
    parser: MeasurementParser,
    discarding: bool,
}

impl PipeReader {
    pub fn drain(&mut self, band: usize, target: &mut AnalysisBuffer) -> io::Result<usize> {
        let mut chunk = [0u8; 4096];
        let mut count = 0;
        for _ in 0..16 {
            let n = match self.file.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => n,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            for &byte in &chunk[..n] {
                if byte == b'\n' {
                    if !self.discarding
                        && let Ok(line) = std::str::from_utf8(&self.bytes)
                        && let Some((position, energy)) = self.parser.line(line)
                    {
                        target.push(band, position, energy);
                        count += 1;
                    }
                    self.bytes.clear();
                    self.discarding = false;
                } else if !self.discarding {
                    if self.bytes.len() >= 512 {
                        self.bytes.clear();
                        self.parser = MeasurementParser::default();
                        self.discarding = true;
                    } else {
                        self.bytes.push(byte);
                    }
                }
            }
        }
        Ok(count)
    }

    pub fn reset(&mut self) {
        let mut discard = AnalysisBuffer::default();
        let _ = self.drain(0, &mut discard);
        self.bytes.clear();
        self.parser = MeasurementParser::default();
        self.discarding = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn full_pipe_returns_instead_of_blocking_the_audio_writer() {
        let mut pipes = Pipes::new().unwrap();
        let writer = &mut pipes.writers[0];
        let bytes = [b'x'; 4096];
        let mut total = 0;
        loop {
            match writer.write(&bytes) {
                Ok(n) => total += n,
                Err(error) => {
                    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
                    break;
                }
            }
            assert!(total < 16 * 1024 * 1024);
        }
        let mut buffer = AnalysisBuffer::default();
        pipes.readers[0].drain(0, &mut buffer).unwrap();
        assert!(buffer.frames(0.0).is_empty());
        assert!(pipes.readers[0].bytes.len() <= 512);
    }
}
