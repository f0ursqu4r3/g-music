# YouTube Music Player Idea

## Status

Initial implementation is underway. `AGENTS.md` defines the repository rules
and implementation constraints.

## One-line concept

A small, keyboard-first desktop music player with a focused now-playing surface,
fast queue control, and native desktop behavior without a bundled browser
runtime.

## Product direction

Build a thin Tauri shell around a replaceable playback provider:

```text
UI
  |
  v
Typed Tauri commands and events
  |
  v
Application services
  |
  v
PlaybackProvider
  |---- Official web or embedded player, if permitted
  |---- Native provider for a permitted audio source
  |---- Fake provider for development and tests
```

The current local-only provider uses `yt-dlp` to resolve YouTube track metadata
and temporary media URLs, then uses headless `mpv` for audio playback. Optional
session cookies stay local to the application. It does not save media files,
collect credentials, or bypass DRM.

## Current implementation

The app keeps the deterministic fake provider for domain tests. Runtime playback
accepts YouTube video or playlist URLs, imports title, artist, album, duration,
video ID, and source URL data, and streams audio through `mpv` over its local
JSON IPC socket. The imported tracks populate the application queue and library
views. Imported metadata persists in the application data directory and is
restored at launch.

## MVP

- One adaptive main window.
- Now-playing details and play, pause, previous, next, seek, and volume
  controls.
- Queue inspection and reorder.
- Clear loading and error states.
- Keyboard-first controls.
- A typed playback-provider boundary with a fake provider for tests.

Do not start with account synchronization, recommendations, lyrics, downloads,
or multiple windows.

## Open decisions

- Decide whether to bundle playback sidecars or keep Homebrew prerequisites.
- Decide whether to add a different service with a documented playback API.

Authentication must remain in the provider's official web flow or use an
approved OAuth flow. The application must not collect or inspect passwords.

## Non-goals

- Downloading or exporting audio.
- Circumventing DRM or service restrictions.
- Reimplementing the full YouTube Music website.
- Lyrics scraping.
- A custom browser engine.
- Multiple frontend frameworks.

## Sources

- <https://developers.google.com/youtube/documentation>
- <https://developers.google.com/youtube/iframe_api_reference>
- <https://developers.google.com/youtube/terms/developer-policies>
