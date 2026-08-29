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
- Durable editable track metadata: title, artist, album, label, and genres.
- Durable playlists that preserve ordered stable track IDs.
- Per-track play counts, last-played times, and a bounded play-time history.
- A private local agent socket and an MCP bridge for Hermes-driven organization.

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

## Hermes library control

When G Music starts, it opens a local Unix socket at:

```text
~/Library/Application Support/com.kyle.gmusic/agent.sock
```

The socket has owner-only permissions. It keeps all writes inside G Music so the
live library, playback state, and durable `library.json` stay consistent. Do not
edit `library.json` directly.

The MCP bridge is `scripts/gmusic-mcp.ts`. It exposes these tools after Hermes
starts it:

- `inspect_library`
- `update_track_metadata`
- `upsert_playlist`
- `delete_playlist`
- `move_library_track`

All write tools require `confirmed=true`. An agent must first show the proposed
metadata, playlist, or order change and get the user's current explicit
approval.

Add this to `~/.hermes/config.yaml`, then restart Hermes:

```yaml
mcp_servers:
  gmusic:
    command: bun
    args:
      - run
      - /Users/la.kyle.dougan/git/personal/gmusic/scripts/gmusic-mcp.ts
    timeout: 30
    connect_timeout: 30
```

Start G Music before asking Hermes to inspect or organize the library. You can
also run the bridge directly for protocol debugging:

```sh
bun run mcp
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

The YouTube provider is a local-only experiment. It uses `yt-dlp` to resolve
track metadata and temporary media URLs, then uses `mpv` for audio playback.
Imported videos and playlist entries populate the library and queue. Track
metadata persists in the application data directory between launches.
It does not download or keep media files, collect credentials, or bypass DRM.
Optional session cookies stay in the application data directory. This
integration depends on YouTube's current site behavior and can break without
notice.
