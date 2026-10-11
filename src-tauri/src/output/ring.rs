//! Sample buffer between the Core Audio IOProc thread and the renderer feeder.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct SampleRing {
    samples: Mutex<VecDeque<f32>>,
    capacity: usize,
    open: AtomicBool,
}

impl SampleRing {
    pub(crate) fn new(capacity_samples: usize) -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(capacity_samples)),
            capacity: capacity_samples,
            open: AtomicBool::new(false),
        }
    }

    /// A closed ring drops input. Close it while paused, so silence cannot
    /// push queued music out.
    pub(crate) fn set_open(&self, open: bool) {
        self.open.store(open, Ordering::Release);
    }

    /// Called from the real-time thread: never blocks. Returns samples kept.
    pub(crate) fn push(&self, input: &[f32]) -> usize {
        if !self.open.load(Ordering::Acquire) {
            return 0;
        }
        let Ok(mut samples) = self.samples.try_lock() else {
            return 0;
        };
        let overflow = (samples.len() + input.len()).saturating_sub(self.capacity);
        let dropped = overflow.min(samples.len());
        samples.drain(..dropped);
        let kept = input.len().min(self.capacity);
        samples.extend(&input[input.len() - kept..]);
        kept
    }

    pub(crate) fn pop_into(&self, output: &mut Vec<f32>, max: usize) -> usize {
        let mut samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        let count = max.min(samples.len());
        output.extend(samples.drain(..count));
        count
    }

    pub(crate) fn clear(&self) {
        self.samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.samples.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_ring_drops_input() {
        let ring = SampleRing::new(8);
        assert_eq!(ring.push(&[1.0, 2.0]), 0);
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn open_ring_keeps_newest_samples_when_full() {
        let ring = SampleRing::new(4);
        ring.set_open(true);
        ring.push(&[1.0, 2.0, 3.0]);
        ring.push(&[4.0, 5.0]);
        let mut out = Vec::new();
        assert_eq!(ring.pop_into(&mut out, 10), 4);
        assert_eq!(out, [2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn oversized_input_keeps_its_tail() {
        let ring = SampleRing::new(2);
        ring.set_open(true);
        assert_eq!(ring.push(&[1.0, 2.0, 3.0]), 2);
        let mut out = Vec::new();
        ring.pop_into(&mut out, 2);
        assert_eq!(out, [2.0, 3.0]);
    }

    #[test]
    fn pause_gate_protects_queued_audio() {
        let ring = SampleRing::new(4);
        ring.set_open(true);
        ring.push(&[0.5, 0.5]);
        ring.set_open(false);
        ring.push(&[0.0, 0.0, 0.0, 0.0]);
        let mut out = Vec::new();
        ring.pop_into(&mut out, 4);
        assert_eq!(out, [0.5, 0.5]);
    }
}
