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

Do not treat YouTube stream extraction as a dependency. Do not use undocumented
endpoints, cookie scraping, `yt-dlp`, or DRM bypasses.

## Initial implementation assumption

The first vertical slice uses a deterministic fake provider. It has no account
flow, network traffic, or audio output. This validates the shell, queue, and
typed command boundary before a permitted real provider is selected.

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

Before implementing a real provider, select one permitted playback mode:

- Official embedded or web playback.
- Native playback backed by a permitted audio source.
- A different music service with a documented playback API.

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
