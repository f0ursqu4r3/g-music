# Live Visualizer Implementation Plan

> **For agentic workers:** Use test-driven implementation and task review. The user approved the design and explicitly requested the build. Execute in this workspace and preserve existing changes.

**Goal:** Add an audio-responsive shader window with timestamp-based synchronization.

**Architecture:** Analyze the existing mpv decoded stream through nonblocking pipes. A native worker owns analysis and playback clock state. The WebGL2 renderer consumes bounded timestamped frames through a dedicated inspection API.

**Tech Stack:** Rust, mpv/FFmpeg filters, Tauri 2, Vue 3, WebGL2.

**Spec:** `docs/superpowers/specs/2026-10-08-live-visualizer-design.md`

## Constraints and rulings

- Preserve existing uncommitted work; do not commit unrelated changes or create a checkout that omits them.
- Use the user's build authorization to execute without another approval round.
- The frontend worker owns new visualizer frontend files only. The main agent owns native analysis and shared integration files.
- Analysis uses 16 bands, 48 kHz, and 20 ms windows. All queues and messages have fixed limits.
- Reader failure must not block playback. A missing sample must never become synthetic audio response.
- Target 60 fps with a 30 fps option. Stop rendering while hidden or minimized.
- Report hardware timing as unverified unless measured end to end.

## Shared interface

`set_visualizer_active({ active: boolean }): Promise<void>` enables or disables analysis. Inspection renews the active window lease. `inspect_visualizer(): Promise<VisualizerSnapshot>` returns:

```ts
interface VisualizerSnapshot {
  generation: number
  status: 'idle' | 'warming' | 'playing' | 'paused' | 'buffering' | 'error'
  error: string | null
  audioPositionMs: number | null
  frames: { positionMs: number; bands: number[] }[]
}
```

Frames contain 16 normalized values in `[0, 1]`. They span 20 ms. The renderer must reject old generations, freeze paused audio motion, neutralize stale input, and use a local monotonic clock bounded by recent observations. Positive user offset delays the visual response.

## Review focus

- Same-position seek and track replacement: reject the previous generation.
- Hidden window and process shutdown: release analysis work and GPU resources.
- Pipe saturation or malformed measurements: preserve playback and bound memory.
- Missing filters, WebGL2, or clock: show a recoverable unavailable state.
- Device and stream changes: reset clock anchors and avoid extrapolating through stalls.

## Task 1: Native analysis

Files: new `src-tauri/src/visualizer/` modules; integration in `playback/youtube.rs` and `lib.rs`.

- [x] Write failing tests for parsing, timestamp selection, queue bounds, and seek generation reset.
- [x] Implement a 16-band filter graph and nonblocking inherited pipes.
- [x] Implement a persistent IPC worker with filter capability checks, active-window lease, clock capture, and lifecycle reset.
- [x] Connect worker ownership to the existing mpv child without changing library ownership.
- [x] Run native unit and silent mpv integration tests, including reader saturation and signal preservation.

## Task 2: Renderer and window

Files: new `src/lib/visualizer/*`, `src/components/VisualizerWindow.vue`, focused tests.

- [x] Write failing tests for clock selection, generation resets, pause, stale data, and offset behavior.
- [x] Implement the typed API, bounded timeline, WebGL2 renderer, and two presets.
- [x] Implement window controls, saved preferences, artwork palette, context-loss recovery, and visibility cleanup.
- [x] Run frontend tests and review the new modules.

## Task 3: App integration and verification

Files: native window registry, capabilities, `src/App.vue`, command palette, window routing, README, related tests.

- [x] Add the native Visualizer window and command palette action.
- [x] Run the actual renderer in a browser and inspect the layout and shader output.
- [x] Run Rust and Vue suites, lint, build, formatting, and Clippy.
- [x] Obtain an independent code review and fix actionable findings.
- [x] Record completed checks and practical limits in the documentation and final response.

## Completion evidence

- Frontend: 538 tests pass across 53 files; production build and lint pass.
- Native: 223 tests pass; nine opt-in tests remain excluded from the default suite. All three visualizer opt-in tests pass with installed mpv and silent output.
- Integration checks prove seek/pause recovery, nonblocking reader saturation, analysis recovery, and identical playback PCM samples.
- Both presets render at normal and minimum window sizes in Chromium. Forced context loss recovers. Browser checks used software rendering.
- Rust formatting and Clippy pass. Prettier passes when the unchanged `.serena/project.yml` file is excluded; that file has an existing formatting warning.
- Independent review findings were fixed, including the retry-generation race, with a regression test.
- Physical audio-to-display timing, output-device switching, wireless output, real network stalls, native CPU/GPU performance, and Linux hardware remain unmeasured. Do not claim the 50 ms target is verified.
