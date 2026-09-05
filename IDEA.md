# YouTube audio player direction

## Product

G Music is a local-first, keyboard-friendly desktop YouTube audio player. The
initial supported platform is macOS. It uses a dense Library for organization
and separate Mini, Queue, Artwork, Import, and Settings windows for focused
tasks.

The original single-window MVP has been superseded. This document describes the
current product boundary. The
[hardening plan](docs/plans/professional-player-hardening.md) and
[release guide](docs/release-readiness.md) define the completion work and
verification gates.

## Architecture

The Vue UI calls typed Tauri commands. Rust owns the library, playback queue,
background imports, and transport state. SQLite stores library metadata, ordered
playlists, listening history, and paused restoration state.

The local provider uses yt-dlp for YouTube metadata and mpv for native audio. A
deterministic fake provider supports domain tests. Imported items enter the
library without replacing the queue or starting playback. Explicit playback uses
the selected context. Application restart restores playback paused.

Provider metadata and user edits have different ownership. Refreshes must not
silently overwrite user organization. Local playlists and Favorites are not
synchronized to a YouTube account.

## Professional 1.0 requirements

- Truthful, authoritative transport controls across all playback windows.
- Safe metadata edits, playlist order, and durable library operations.
- Fast library text search and explicit YouTube discovery/import.
- Observable, cancellable imports and retryable metadata enrichment.
- Queue inspection, reorder, removal, clearing, and playlist saving.
- Visible failures, retained editor input, and usable recovery actions.
- Accessible keyboard interaction and usable minimum-window layouts.
- Shared themes and consistent controls without a wholesale visual redesign.
- Native media integration and a tested system-default audio-output policy.
- Dependency diagnostics, private backups, and privacy-safe support information.
- Verified release packaging, signing, notarization, and clean-Mac setup.

## Distribution and service boundary

Use explicitly installed mpv and yt-dlp for the initial distribution. Do not
claim bundled sidecars or managed dependency updates. Revisit that policy only
with an installation, licensing, and update plan.

Authentication remains optional. Users sign in through the provider's own web
flow. Local session export must not expose passwords or cookies to the UI, logs,
or support reports. Public release requires a separate service-terms and
account-risk review. Using the official YouTube API does not by itself permit an
audio-only or background-playback product.

## Non-goals

- Downloading or exporting audio.
- Circumventing DRM or service restrictions.
- Reimplementing the full YouTube Music website.
- Recommendations or account-library synchronization.
- Lyrics scraping, crossfade, or an equalizer in 1.0.
- A custom browser engine or multiple frontend frameworks.

## Sources

- <https://developers.google.com/youtube/documentation>
- <https://developers.google.com/youtube/iframe_api_reference>
- <https://developers.google.com/youtube/terms/developer-policies>
