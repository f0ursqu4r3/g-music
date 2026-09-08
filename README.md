# G Music

G Music is a desktop YouTube audio player built with Tauri 2, Rust, Vue 3, Vite,
Tailwind CSS v4, Reka UI, and Lucide icons. It builds on macOS and Linux; macOS
remains the primary distribution target. See [release readiness](docs/release-readiness.md)
for the verification and distribution requirements.

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
- Keyboard controls: Space toggles play, J selects the previous track, K selects
  the next track, and Q toggles the queue.
- Typed Tauri IPC client and serializable Rust payloads.
- Durable editable track metadata: title, artist, album, label, and genres.
- Durable playlists that preserve ordered stable track IDs.
- Smart playlists with saved rules, match previews, and automatic membership.
- A command palette for music search, playback, queue actions, and app windows.
- Per-track play counts, last-played times, and a bounded play-time history.
- A private local agent socket and an MCP bridge for Hermes-driven organization.

## Artwork window

Artwork fills the window behind one bottom scrim and native window buttons.
Playback controls slide up when Artwork becomes active and stay visible until
it loses focus, regardless of cursor position. On focus loss, controls slide
below the frame and track information moves into the freed bottom space.
Title and artist also appear on inactive hover. Native cursor tracking includes
the titlebar and does not activate the window. Hover and open menus cannot reveal
inactive playback controls. Overlay entrances take 300 ms; exits take 1200 ms. Reduced-motion
mode applies state changes immediately. Thin bottom progress remains when controls
hide. Native window buttons are unchanged.

Drag the seek slider to preview a timestamp, then release to seek. Escape, a
canceled drag, a track change, or window blur cancels the preview. Seeking is
disabled when the track duration is unknown. Use the volume slider and mute button
to adjust sound. Open queue and Open library show their native windows. Drag the
artwork to move the window, or right-click it for favorite and window actions.
Tab reaches playback controls only in an active window. Playback shortcuts stay the same.

## Artwork cache

Artwork persists in the application data directory under `artwork/`. The
`artwork_cache` table in `library.sqlite3` tracks file size and last access.
The cache keeps up to **512 MiB** of indexed images and evicts the least recently
used files. Each image has an **8 MiB** limit. This cache stores artwork only.

One backend resolver serves all native windows. It checks cached max-resolution
and HQ images before any network request, including after an app restart.
Valid cached images remain available offline until eviction. Missing, invalid,
or oversized files are removed from the index and can be fetched again.

The resolver combines concurrent requests for the same video ID. It permits
**4 blocking jobs** across windows. Excess requests wait asynchronously for a
job slot instead of creating more workers or returning a missing image.
Network requests run concurrently and reuse an HTTP client. Each quality has a
**10-second timeout**. A failed max-resolution fetch still tries HQ. Failed
lookups have a **5-second cooldown**, held in a bounded list of **256 IDs**.
Local files are checked before this cooldown.

Downloads read at most 8 MiB plus one byte to detect overflow. Cached file checks
use file size and at most 12 signature bytes. Supported signatures are JPEG,
PNG, and WebP. Writes use unique temporary files and atomic rename; errors remove
the temporary file. Artwork directories have owner-only `0700` permissions and
new files have `0600` permissions. The asset protocol stays scoped to
`$APPDATA/artwork/**`.

Each WebView also keeps up to **256 results** in least-recently-used order.
Successful paths expire after **30 seconds**, since another window can evict the
file. Missing results and command errors expire after **2 seconds**. Each WebView
allows **8 active requests** and queues the rest, sharing promises for duplicate
video IDs. Both layers accept only exact 11-character YouTube video IDs.
An image load error clears its WebView cache entry
and shows the placeholder. Expiry and invalidation do not start automatic retry
loops. Responses for an earlier video cannot replace the current image.

## Development

Install Bun and Rust, then install the local playback tools and Tauri system
dependencies for your platform.

On macOS:

```sh
brew install mpv yt-dlp
```

On Ubuntu 24.04:

```sh
sudo apt install mpv yt-dlp libwebkit2gtk-4.1-dev \
  build-essential curl wget file libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf
```

Ubuntu installations that already use Ayatana AppIndicator should keep
`libayatana-appindicator3-dev`; installing `libappindicator3-dev` instead can
cause a package conflict.

Install both tools explicitly. They are local prerequisites, not bundled
sidecars. Then run the app:

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

### Playback session errors

A password change can invalidate the saved YouTube session. G Music waits for
`mpv` to finish loading each track. A failed load stops playback instead of
skipping through the queue. Known session failures show a persistent notification:

1. Select **Reconnect YouTube** to open the sign-in window.
2. Complete sign-in in that window.
3. Select **Save session and retry** in the notification.

Other load failures keep the normal Retry action. Session detection uses known
extractor error markers; it does not treat every network or format error as an
expired session. Raw extractor messages and session values stay out of the UI
and application logs.

### Subscriber-only tracks during metadata refresh

YouTube tracks that require a channel membership or supporter subscription may be
inaccessible to the current YouTube session. When the metadata refresh job for
such a track is skipped by the backend, G Music handles it as follows:

- The track stays hidden from the library track list and play queue, including
  after restart. The database record, play history, and playlist membership are
  preserved. Only a confirmed provider access denial triggers this filter;
  titles and the provider's `subscriber_only` metadata value do not.
- The Metadata refresh drawer shows a dedicated **Skipped** section for
  skipped tracks, separate from refreshed and failed tracks.
- Skipped tracks do not count toward the refreshed or failed tallies.
- The **Retry failed metadata** button does not appear when only skipped tracks
  are present; it only appears when real network or parse failures exist.
- The status line says "Finished with skipped tracks" when skips are the only
  non-refreshed outcome. The library header keeps the skipped count and details
  available after completion.
- Re-importing a track with a YouTube session that has the required membership
  restores full metadata and makes the track visible again.

## Hermes library control

When G Music starts, it opens a local Unix socket at:

```text
~/Library/Application Support/com.kyle.gmusic/agent.sock
```

The socket has owner-only permissions. It keeps all writes inside G Music so the
live library, playback state, and durable SQLite store stay consistent. The
active store is `library.sqlite3`. The app migrates a legacy `library.json` to
SQLite once and retains `library.json.migrated` as a backup. Do not edit the
active database or migration backup while the app is running.

The MCP bridge is `scripts/gmusic-mcp.ts`. It exposes these tools after Hermes
starts it:

- `inspect_library`
- `update_track_metadata`
- `update_library_track_metadata_batch`
- `remove_library_tracks`
- `upsert_playlist`
- `delete_playlist`
- `move_library_track`

All write tools require `confirmed=true`. An agent must first show the proposed
metadata, playlist, or order change and get the user's current explicit
approval.

The MCP `upsert_playlist` tool edits regular playlists. Use the app's smart
playlist editor to create, edit, or convert smart rules. A regular upsert cannot
erase an existing smart definition.

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

## Library controls

Library search starts collapsed. Click the search icon or press Command-F on
macOS (Control-F on other platforms) to open or close it. Opening search focuses
the input. Escape closes search and clears its filter, as does the search icon.

Click a track to select it. Command-click or Control-click toggles individual
tracks. Shift-click selects a range. The context menu applies playback, queue,
Favorites, and removal actions to the selected tracks.

To remove tracks from a regular custom playlist, open it, select the tracks,
and choose **Remove from playlist** from the context menu. This keeps the tracks
in the library, other playlists, and the play queue. **Remove from library** is
a separate action that requires confirmation.

Drag selected tracks from the list or grid onto a regular custom playlist.
Favorites, Most Played, and smart playlists are not drop targets.
Drag the sidebar's right edge to resize it. When the resize control has keyboard
focus, use the arrow keys, Home, or End to change its width.

The Library window uses HTML drag and drop. Keep Tauri's native file-drop
handler disabled: `dragDropEnabled: false` in the startup window configuration
and `disable_drag_drop_handler()` when Rust recreates the Library window. The
native handler intercepts DOM drag events on macOS. Restart the native app after
changing this setting; frontend hot reload does not recreate its WebView.

### Smart playlists

Select **New smart playlist** beside the Playlists heading, or use that action
in the command palette. Choose **All rules** or **Any rule**, then add rules for
title, artist, album, label, genre, duration, play count, Favorites, or time since
last playback. Text comparisons ignore case and surrounding rule-value spaces.
Duration values use milliseconds. Relative-time values use days.

The presets populate the editor without saving:

- **Never played:** play count equals zero.
- **Forgotten favorites:** Favorites not played within 30 days, including tracks
  that have never played.
- **Short tracks:** duration less than three minutes, shortest first.

Choose a sort field and direction, with an optional track limit. Select
**Preview** to see the total before the limit, the resulting playlist count,
and why each displayed track matches. The preview shows at most 20 tracks.
Select **Save playlist** to store the rules. Failed requests keep the draft.

Smart playlists include available tracks only. Membership updates after library
changes. The focused Library also refreshes relative-time rules once a minute.
Smart playlists use their saved rule order for display and playback; edit the
rules to change that order. Manual track removal and drop-to-add are disabled.

Open a smart playlist's editor to change its rules, delete it, or select
**Convert to regular playlist**. Conversion requires confirmation. It keeps
the saved name, playlist ID, and current track order, then removes the rules.
Conversion discards unsaved edits. Deleting a playlist keeps its library tracks.
SQLite backups include smart playlist definitions.

### Command palette

Select **Commands** in the Library header, or press Command-K on macOS
(Control-K on other platforms). The shortcut opens the Library from other app
windows. It does not open over another dialog or menu.

Search tracks, albums, artists, playlists, and actions. Exact and prefix title
matches rank above substring matches. The palette shows up to 40 results.
Use the arrow keys to choose a result, Enter to run it, or Escape to close.
Choose **Play**, **Play next**, or **Add to queue** in **Music action** to set
the operation for a music result.

Actions include creating regular or smart playlists, opening Import, Queue,
or Settings, and adding the Library's selected tracks to a regular playlist.
Multi-track queue actions preserve collection order. An empty collection or a
failed action shows an error without closing the palette. If a queue batch
fails partway through, earlier successful changes remain in the queue.

## App icon

`public/app-icon.svg` is the editable icon source and browser favicon. The violet
tile uses an ivory G with a play-shaped cutout. Keep its transparent outer margin
so the icon has the correct visual size in the macOS Dock.

Regenerate the desktop PNG, macOS ICNS, and Windows ICO assets after editing it:

```sh
bun run icons
```

The generator uses the installed Tauri CLI and does not retain mobile assets.
The bundle icon paths are configured in `src-tauri/tauri.conf.json`. Rebuild the
native app to apply icon changes; frontend hot reload does not update Dock icons.

## Verification

```sh
bun run test
bun run lint
bun run build
bun run format
bun audit

cd src-tauri
cargo fmt --all -- --check
cargo check --all-targets --future-incompat-report
cargo rustc --locked -p block -- -D warnings
cargo test --locked --manifest-path vendor/block/Cargo.toml
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Build safeguards

Window components load on demand. The test suite builds the production bundle
and checks that each window has a dynamic entry and every JavaScript chunk stays
within the 500 kB budget. Do not raise the warning limit to bypass this check.

Souvlaki requires the legacy `block` crate on macOS. A local compatibility patch
fixes its foreign-static declaration without changing the public API. See
[`GMUSIC-PATCH.md`](src-tauri/vendor/block/GMUSIC-PATCH.md) for provenance,
verification, and the condition for removing the patch.

## Product boundary

Windows read the latest import progress on startup and receive shared progress
events while the app runs. This progress stays in memory, including the last
completed, failed, or cancelled result. A fresh app launch starts with no import
progress. App-level command errors use the shadcn-vue Sonner component, with
Retry and dismissal controls. Repeated errors update one toast. Startup errors
stay visible until recovery; inline form errors remain beside their fields.

The YouTube provider is a local-only experiment. It uses `yt-dlp` to resolve
track metadata and temporary media URLs, then uses `mpv` for audio playback.
Imports add videos and playlist entries to the durable library. Imports do not
start playback or replace the listening queue. An explicit Play action sets the
playback context. The app restores saved playback state as paused; it never
starts audio automatically after launch. It does not download or keep media
files, collect credentials, or bypass DRM. Optional session cookies stay in the
application data directory. This integration depends on YouTube's current site
behavior and can break without notice.
