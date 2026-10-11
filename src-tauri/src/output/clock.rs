//! Timing math for AirPlay output. All times are seconds unless the name says `_ms`.

pub(crate) fn queued_ms(enqueued_end_s: f64, synchronizer_s: f64) -> f64 {
    ((enqueued_end_s - synchronizer_s) * 1000.0).max(0.0)
}

pub(crate) fn corrected_position_ms(raw_ms: u64, queued_ms: f64) -> u64 {
    raw_ms.saturating_sub(queued_ms.round() as u64)
}

/// The tap follows the local device clock; the renderer follows the AirPlay clock.
/// A queue that grows or shrinks past the limit means the clocks drifted apart.
pub(crate) struct DriftMonitor {
    started_s: f64,
    baseline_ms: Option<f64>,
}

impl DriftMonitor {
    pub(crate) const SETTLE_S: f64 = 3.0;
    pub(crate) const LIMIT_MS: f64 = 250.0;

    pub(crate) fn new(start_s: f64) -> Self {
        Self {
            started_s: start_s,
            baseline_ms: None,
        }
    }

    pub(crate) fn reset(&mut self, now_s: f64) {
        self.started_s = now_s;
        self.baseline_ms = None;
    }

    pub(crate) fn observe(&mut self, now_s: f64, queued_ms: f64) -> bool {
        match self.baseline_ms {
            None if now_s - self.started_s >= Self::SETTLE_S => {
                self.baseline_ms = Some(queued_ms);
                false
            }
            None => false,
            Some(baseline) => (queued_ms - baseline).abs() > Self::LIMIT_MS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_is_enqueued_minus_played_and_never_negative() {
        assert_eq!(queued_ms(12.5, 10.0), 2500.0);
        assert_eq!(queued_ms(10.0, 10.2), 0.0);
    }

    #[test]
    fn corrected_position_subtracts_queue_and_clamps() {
        assert_eq!(corrected_position_ms(60_000, 2_000.4), 58_000);
        assert_eq!(corrected_position_ms(1_000, 2_000.0), 0);
    }

    #[test]
    fn drift_monitor_waits_for_settle_then_flags_large_changes() {
        let mut monitor = DriftMonitor::new(100.0);
        assert!(!monitor.observe(101.0, 2_000.0)); // still settling
        assert!(!monitor.observe(103.0, 2_000.0)); // baseline taken at 3 s
        assert!(!monitor.observe(200.0, 2_249.0));
        assert!(monitor.observe(300.0, 2_251.0));
        assert!(monitor.observe(301.0, 1_749.0));
    }

    #[test]
    fn reset_restarts_the_settle_period() {
        let mut monitor = DriftMonitor::new(0.0);
        monitor.observe(3.0, 2_000.0);
        monitor.reset(10.0);
        assert!(!monitor.observe(11.0, 5_000.0));
        assert!(!monitor.observe(13.0, 1_000.0));
        assert!(!monitor.observe(14.0, 1_100.0));
    }
}
