# G Music

G Music is a focused desktop music-player shell built with Tauri 2, Rust, Vue
3, Vite, Tailwind CSS v4, shadcn-vue, and Lucide icons.

## Current mode

The first slice uses a deterministic local fake playback provider. It has no
account flow, network traffic, audio output, cookies, or extracted media URLs.
This keeps the UI and command contracts testable while a permitted real provider
is selected.

## Features

- Compact desktop mini player with album art, seek, volume, and transport
  controls.
- Expandable queue with deterministic navigation and reorder controls.
- Midnight, Plum, and Ember themes, saved in the local WebView store.
- Keyboard controls: Space toggles play, J selects the previous track, K
  selects the next track, and Q toggles the queue.
- Typed Tauri IPC client and serializable Rust payloads.

## Development

```sh
bun install
bun run tauri dev
```

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

G Music does not extract YouTube stream URLs or use undocumented endpoints,
cookie scraping, browser-token theft, DRM bypasses, or `yt-dlp`. Any real
provider must use a permitted, documented playback and authentication flow.
