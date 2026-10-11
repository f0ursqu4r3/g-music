use std::collections::VecDeque;

pub(super) const BAND_COUNT: usize = 16;
pub(super) const FRAME_MS: f64 = 20.0;
const MAX_FRAMES: usize = 150;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisFrame {
    pub position_ms: f64,
    pub bands: Vec<f32>,
}

#[derive(Default)]
pub(super) struct MeasurementParser {
    position_ms: Option<f64>,
}

impl MeasurementParser {
    pub fn line(&mut self, line: &str) -> Option<(f64, f32)> {
        if line.starts_with("frame:") {
            self.position_ms = line
                .split_whitespace()
                .find_map(|part| part.strip_prefix("pts_time:"))
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && value.abs() < 1e9)
                .map(|value| value * 1000.0);
            return None;
        }
        let value = line.strip_prefix("lavfi.astats.Overall.RMS_level=")?;
        let position = self.position_ms.take()?;
        let decibels = value.trim().parse::<f32>().ok()?;
        if decibels == f32::NEG_INFINITY {
            return Some((position, 0.0));
        }
        decibels
            .is_finite()
            .then_some((position, ((decibels + 72.0) / 72.0).clamp(0.0, 1.0)))
    }
}

pub(super) struct AnalysisBuffer {
    bands: [VecDeque<(f64, f32)>; BAND_COUNT],
}

impl Default for AnalysisBuffer {
    fn default() -> Self {
        Self {
            bands: std::array::from_fn(|_| VecDeque::new()),
        }
    }
}

impl AnalysisBuffer {
    pub fn push(&mut self, band: usize, position_ms: f64, energy: f32) {
        if band >= BAND_COUNT || !position_ms.is_finite() || !energy.is_finite() {
            return;
        }
        let rows = &mut self.bands[band];
        if rows
            .back()
            .is_some_and(|(previous, _)| *previous >= position_ms)
        {
            // A backwards timestamp starts a new stream. The worker also resets all
            // bands on lifecycle events, including seeks without a timestamp regression.
            rows.clear();
        }
        rows.push_back((position_ms, energy.clamp(0.0, 1.0)));
        while rows.len() > MAX_FRAMES {
            rows.pop_front();
        }
    }

    pub fn frames(&self, position_ms: f64) -> Vec<AnalysisFrame> {
        self.bands[0]
            .iter()
            .filter(|(time, _)| *time >= position_ms - 600.0 && *time <= position_ms + 1200.0)
            .filter_map(|(time, _)| {
                let bands: Option<Vec<_>> = self
                    .bands
                    .iter()
                    .map(|band| {
                        band.iter()
                            .find(|(other, _)| (*other - *time).abs() < 0.1)
                            .map(|(_, value)| *value)
                    })
                    .collect();
                bands.map(|bands| AnalysisFrame {
                    position_ms: *time,
                    bands,
                })
            })
            .take(100)
            .collect()
    }

    pub fn clear(&mut self) {
        for band in &mut self.bands {
            band.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_timestamps_and_normalizes_silence() {
        let mut parser = MeasurementParser::default();
        assert_eq!(parser.line("frame:4 pts:3840 pts_time:0.08"), None);
        assert_eq!(
            parser.line("lavfi.astats.Overall.RMS_level=-36.000000"),
            Some((80.0, 0.5))
        );
        parser.line("frame:5 pts:4800 pts_time:0.10");
        assert_eq!(
            parser.line("lavfi.astats.Overall.RMS_level=-inf"),
            Some((100.0, 0.0))
        );
    }

    #[test]
    fn malformed_frame_cannot_reuse_previous_timestamp() {
        let mut parser = MeasurementParser::default();
        parser.line("frame:0 pts:0 pts_time:0");
        parser.line("frame:1 pts:960 pts_time:NaN");
        assert_eq!(parser.line("lavfi.astats.Overall.RMS_level=-12"), None);
        parser.line("frame:2 pts:1920 pts_time:0.04");
        assert_eq!(parser.line("lavfi.astats.Overall.RMS_level=NaN"), None);
        assert_eq!(parser.line("lavfi.astats.Overall.RMS_level=-12"), None);
    }

    #[test]
    fn only_complete_frames_are_published_and_buffer_is_bounded() {
        let mut buffer = AnalysisBuffer::default();
        for band in 0..BAND_COUNT - 1 {
            buffer.push(band, 100.0, 0.5);
        }
        assert!(buffer.frames(100.0).is_empty());
        buffer.push(15, 100.0, 0.25);
        let frames = buffer.frames(100.0);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].bands[15], 0.25);
        for frame in 0..500 {
            for band in 0..BAND_COUNT {
                buffer.push(band, frame as f64 * FRAME_MS, 0.5);
            }
        }
        assert!(buffer.bands.iter().all(|band| band.len() == MAX_FRAMES));
        assert!(buffer.frames(9900.0).len() <= 100);
    }

    #[test]
    fn clear_removes_all_old_track_measurements() {
        let mut buffer = AnalysisBuffer::default();
        for band in 0..BAND_COUNT {
            buffer.push(band, 1000.0, 0.8);
        }
        assert_eq!(buffer.frames(1000.0).len(), 1);
        buffer.clear();
        assert!(buffer.frames(1000.0).is_empty());
    }
}
