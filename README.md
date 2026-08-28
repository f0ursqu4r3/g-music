# G Music

G Music is a focused desktop music-player shell built with Tauri 2, Rust, Vue
3, Vite, Tailwind CSS v4, shadcn-vue, and Lucide icons.

## Current mode

G Music imports YouTube video and playlist URLs, resolves track metadata with
local `yt-dlp`, and streams audio through `mpv`. Optional sign-in stores a
restricted local YouTube session for URLs that require account access. It does
not save media files or collect Google credentials.

## Features

- Compact desktop mini player with album art, seek, volume, and transport
  controls.
- Expandable queue with deterministic navigation and reorder controls.
- Midnight, Plum, and Ember themes, saved in the local WebView store.
- Keyboard controls: Space toggles play, J selects the previous track, K
  selects the next track, and Q toggles the queue.
- Typed Tauri IPC client and serializable Rust payloads.

## Development

Install the local playback tools:

```sh
brew install mpv
```

The Homebrew `mpv` formula installs `yt-dlp` as a dependency. Then run the app:

```sh
bun install
bun run tauri dev
```

Rust logs are written to the terminal running Tauri. Development builds use
`DEBUG` by default. Set `RUST_LOG` to change the level:

```sh
RUST_LOG=gmusic_lib=info bun run tauri dev
RUST_LOG=gmusic_lib=trace bun run tauri dev
```

Logs include Tauri commands, YouTube metadata resolution, queue persistence,
`mpv` process startup, and IPC command names. They do not include cookie values,
cookie paths, raw stream URLs, or IPC payload bodies.

## Verification

```sh
bun run test
bun run build

cd src-tauri
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Product boundary

The YouTube provider is a local-only experiment. It uses `yt-dlp` to resolve
track metadata and temporary media URLs, then uses `mpv` for audio playback.
Imported videos and playlist entries populate the library and queue. Track
metadata persists in the application data directory between launches.
It does not download or keep media files, collect credentials, or bypass DRM.
Optional session cookies stay in the application data directory. This
integration depends on YouTube's current site behavior and can break without
notice.
