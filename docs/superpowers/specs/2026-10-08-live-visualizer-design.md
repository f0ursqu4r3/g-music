# Live audio visualization window

Status: Implemented. Automated tests and browser rendering checks are complete. Physical audio-to-display timing and native hardware performance remain unmeasured.

## Outcome

Add a separate visualization window that responds to the audio G Music plays. Synchronize the visual response with the audio playback clock. Preserve reliable music playback when the window is slow, hidden, closed, or unavailable.

macOS is the primary target. Keep the analysis design compatible with Linux, but verify its mpv build and WebView separately.

## First release

- A native Visualizer window, available from the Window menu and command palette.
- WebGL2 rendering with a spectrum preset and a flowing color preset.
- Sixteen frequency bands, with derived bass, midrange, treble, and overall intensity. This is a filter-bank spectrum, not a raw FFT or waveform.
- Fullscreen, preset selection, intensity, and a manual synchronization offset.
- Track colors from artwork, with a theme fallback.
- A 60 fps target, a 30 fps option, and capped rendering resolution.
- No raw waveform preset in this release. The proposed analysis stream contains energy measurements, not audio samples.

## Audio analysis

Split the decoded audio inside the existing mpv audio filter graph. Keep the playback branch separate. Analyze the other branch with FFmpeg filters provided by mpv. Do not open a second YouTube stream.

Produce short frames with media timestamps and band energy. The probe used 960 samples at 48 kHz, or 20 ms. Production must normalize the analysis branch sample rate or derive the sample count from the actual rate.

Send the measurements through inherited, nonblocking pipes. Dedicated Rust readers continuously drain these pipes into bounded memory queues. Do not route raw audio through Tauri events or save analysis streams to disk. This requires no standalone FFmpeg executable at runtime; it uses mpv's filter support.

Check required filters before enabling this feature. If analysis is unsupported or stops, show a visualization error while keeping playback usable. Validate recovery after pipe overflow; uninterrupted playback alone does not prove that analysis recovers.

## Synchronization

Read audio-pts through a dedicated persistent mpv IPC connection. This clock accounts for the delay reported by the audio driver. Do not drive visuals from measurement arrival time or the existing low-frequency player UI updates.

Hold measurements by media timestamp. Select the measurement covering the current audio clock position, then interpolate or smooth only within a bounded interval. Anchor local monotonic time to fresh audio clock observations. Stop extrapolating when observations become stale or playback stops progressing.

Assign a generation to each track load and seek. Reset pending measurements, clock anchors, and smoothing when the generation changes. Synchronize this reset with mpv seek and playback-restart events; timestamp regression alone is insufficient for forward seeks or repeated seeks to the same position. Never display data from the previous generation.

On pause, freeze the audio-driven response. During buffering or missing measurements, fade the response to neutral. Re-anchor on resume, device change, or recovery. A manual offset compensates for output latency that the driver does not report, including some wireless-device paths.

## Window and resource behavior

Use the existing native window registry and lazy Vue window loading. Put rendering and synchronization logic in separate modules. Keep frame-rate updates outside Vue's normal reactive component tree.

Stop rendering when minimized or closed. Stop analysis when no visualization consumer needs it, after validating that changing the filter graph does not interrupt playback. Handle WebGL context loss and missing WebGL2 with a clear window state. Support reduced motion through lower movement and a spectrum preset.

## Evidence from the temporary probe

Tested locally with mpv 0.41.0 and FFmpeg libraries 9.0.1. The input was synthetic stereo audio. Output used mpv's silent null device.

- Bass, midrange, and treble measurements correctly distinguished 90 Hz, 900 Hz, and 6 kHz tones.
- Analysis ran about 350 ms ahead of the audio clock. Immediate rendering would therefore be early.
- Buffered timestamp selection stayed within 20 ms of the audio clock during playback and after forward and backward seeks.
- Pause stopped the clock and further analysis once buffered work settled.
- Playback advanced 6.995 seconds during a 6.995-second observation while analysis readers were stopped for seven seconds.

These results prove an analysis and clock-selection path. They do not measure visible rendering delay, actual speaker delay, Bluetooth behavior, network buffering, CPU cost, sixteen-band performance, or supported Linux configurations.

## Implementation verification

- All 538 frontend tests and 223 native tests pass. Nine native tests require explicit opt-in; all three visualizer integration tests were also run and passed.
- Installed-mpv tests cover the sixteen-band stream, a long pause, resume, forward/backward and same-position seeks, and visualizer deactivation/reactivation.
- Seven seconds of unread analysis output did not block playback. Reinstalling the filter restored complete timestamped measurements.
- Decoded PCM samples were identical with and without the analysis branch.
- Both presets rendered in Chromium at 960 × 640 and 480 × 320. WebGL context loss and restoration recovered without graphics errors. These checks used software rendering.
- Production build, frontend lint, Rust formatting, and Clippy pass. Prettier passes except for an existing style warning in the unchanged `.serena/project.yml` file.
- Independent code review covered synchronization, lifecycle, error recovery, and renderer cleanup. Regression tests cover the corrected retry-generation race.

Physical speaker-to-screen latency, wireless output, output-device switching, real network stalls, native GPU/CPU use, and Linux hardware remain manual validation tasks. The 50 ms end-to-end target below is not yet verified.

## Acceptance checks

1. Known tone bursts activate the expected frequency bands and silence produces a neutral response.
2. Rendered response follows the measured audio output within a 50 ms target on the reference Mac with built-in or wired audio. Measure this end to end; do not infer it from the null-output probe.
3. Repeated seek, pause, resume, track change, and network stall tests never display stale measurements.
4. A slow or closed visualization consumer cannot block playback. Verify pipe overflow and reader recovery.
5. Compare decoded playback samples with and without the analysis branch to detect unintended signal changes.
6. Verify output-device changes and document any required calibration for wireless output.
7. Verify bounded memory, no accumulating analysis files, and stopped rendering when minimized.
8. Measure GPU and CPU use at both frame-rate settings, including sixteen-band analysis.
9. Run focused Rust and Vue tests, then the existing lint, type/build, formatting, and relevant playback regression checks.

## References

- mpv audio-pts and audio filter support: https://mpv.io/manual/stable/
- FFmpeg acrossover, astats, and ametadata: https://ffmpeg.org/ffmpeg-filters.html
