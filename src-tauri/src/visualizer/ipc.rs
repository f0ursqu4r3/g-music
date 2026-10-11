use std::{
    collections::VecDeque,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

const MAX_MESSAGE: usize = 32 * 1024;

/// A separate connection keeps visualization sampling off the transport mutex.
pub(super) struct Ipc {
    stream: UnixStream,
    pending: Vec<u8>,
    messages: VecDeque<Value>,
    events: Vec<Value>,
    request_id: u64,
}

impl Ipc {
    pub fn connect(path: &Path) -> Result<Self, String> {
        let stream = UnixStream::connect(path).map_err(|_| "Audio connection is unavailable.")?;
        stream
            .set_nonblocking(true)
            .map_err(|_| "Audio connection could not be configured.")?;
        Ok(Self {
            stream,
            pending: Vec::new(),
            messages: VecDeque::new(),
            events: Vec::new(),
            request_id: 0,
        })
    }

    pub fn command(&mut self, command: Value) -> Result<Value, String> {
        self.request_id += 1;
        let mut data =
            serde_json::to_vec(&json!({"command": command, "request_id": self.request_id}))
                .unwrap();
        data.push(b'\n');
        let deadline = Instant::now() + Duration::from_millis(300);
        let mut written = 0;
        while written < data.len() {
            match self.stream.write(&data[written..]) {
                Ok(0) => return Err("Audio connection closed.".into()),
                Ok(n) => written += n,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err("Audio connection timed out.".into());
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => return Err("Audio connection closed.".into()),
            }
        }
        loop {
            self.read_available()?;
            while let Some(message) = self.messages.pop_front() {
                if message.get("event").is_some() {
                    if self.events.len() >= 128 {
                        return Err("Audio event queue overflowed.".into());
                    }
                    self.events.push(message);
                } else if message["request_id"].as_u64() == Some(self.request_id) {
                    return match message["error"].as_str() {
                        Some("success") => Ok(message["data"].clone()),
                        Some("property unavailable") => Ok(Value::Null),
                        _ => Err("Audio analysis command failed.".into()),
                    };
                }
            }
            if Instant::now() >= deadline {
                return Err("Audio connection timed out.".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    pub fn events(&mut self) -> Result<Vec<Value>, String> {
        self.read_available()?;
        while let Some(message) = self.messages.pop_front() {
            if message.get("event").is_some() {
                if self.events.len() >= 128 {
                    return Err("Audio event queue overflowed.".into());
                }
                self.events.push(message);
            }
        }
        Ok(std::mem::take(&mut self.events))
    }

    fn read_available(&mut self) -> Result<(), String> {
        let mut bytes = [0u8; 4096];
        // Bound work even when the producer is continuously writing.
        for _ in 0..16 {
            match self.stream.read(&mut bytes) {
                Ok(0) => return Err("Audio connection closed.".into()),
                Ok(n) => {
                    self.pending.extend_from_slice(&bytes[..n]);
                    while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
                        if end > MAX_MESSAGE || self.messages.len() >= 128 {
                            return Err("Audio message exceeded its limit.".into());
                        }
                        let message = serde_json::from_slice(&self.pending[..end])
                            .map_err(|_| "Audio message was invalid.")?;
                        self.messages.push_back(message);
                        self.pending.drain(..=end);
                    }
                    if self.pending.len() > MAX_MESSAGE {
                        return Err("Audio message exceeded its limit.".into());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err("Audio connection closed.".into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Ipc, UnixStream) {
        let (stream, other) = UnixStream::pair().unwrap();
        stream.set_nonblocking(true).unwrap();
        (
            Ipc {
                stream,
                pending: Vec::new(),
                messages: VecDeque::new(),
                events: Vec::new(),
                request_id: 0,
            },
            other,
        )
    }

    #[test]
    fn fragmented_events_are_kept_until_complete() {
        let (mut ipc, mut writer) = pair();
        writer.write_all(b"{\"event\":\"se").unwrap();
        assert!(ipc.events().unwrap().is_empty());
        writer.write_all(b"ek\"}\n").unwrap();
        assert_eq!(ipc.events().unwrap()[0]["event"], "seek");
    }

    #[test]
    fn oversized_unterminated_messages_are_rejected() {
        let (mut ipc, mut writer) = pair();
        for _ in 0..8 {
            writer.write_all(&[b'x'; 4096]).unwrap();
            assert!(ipc.events().is_ok());
        }
        writer.write_all(b"x").unwrap();
        assert!(ipc.events().is_err());
        assert!(ipc.pending.len() <= MAX_MESSAGE + 4096);
    }
}
