# Release readiness

## Supported product boundary

G Music is a local-first, third-party YouTube audio player. The initial release
target is macOS. The application owns its library, playlists, queue, metadata,
and listening history. These are not a synchronized YouTube Music account
library.

The provider uses locally installed `yt-dlp` and `mpv`. Public content does not
require sign-in. Optional sessions stay local. The application does not retain
media downloads, collect account passwords, or bypass DRM.

Recommendations, lyrics, crossfade, an equalizer, and downloads are not 1.0
acceptance criteria. Do not advertise Windows or Linux support until their
native integration and packaging pass the same checks as macOS.

## Distribution policy

Use explicit local dependencies for this release. Do not imply that the app
contains managed playback sidecars.

```sh
brew install mpv yt-dlp
```

Users control updates to these tools. Diagnose the installed versions before
suggesting an update. Provider site changes can break extraction independently
of an application release. Do not silently install or update executables.

A developer build is not a public release. Public distribution requires:

- A release build, not `--debug`.
- A valid Developer ID signature and appropriate entitlements.
- Apple notarization and a stapled notarization ticket.
- Gatekeeper acceptance after quarantine on a clean Mac.
- First-run verification without the developer's PATH or caches.
- A tested installation and application-update procedure.
- A documented minimum supported macOS version.
- A service-terms, account-risk, distribution, and support review.

Signing credentials and legal approval are external release gates. Do not put
credentials in source, command transcripts, diagnostic exports, or this guide.
No signing or notarization claim follows from an ad-hoc debug signature.

## Local quality gate

Run these checks from the repository root:

```sh
bun install --frozen-lockfile
bun run test
bun run lint
bun run build
bun run format
bun audit
```

Run the Rust checks from `src-tauri`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets --future-incompat-report
cargo rustc --locked -p block -- -D warnings
cargo test --locked --manifest-path vendor/block/Cargo.toml
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

The dependency security floor uses Vite 6.4.3 or later and Vitest 3.2.6 or
later. The `qs` override prevents installation below 6.16.0. Re-run `bun audit`
when updating the lockfile. A clean audit is a point-in-time package check, not
proof that the application has no security defects.

Relevant advisories:

- <https://github.com/advisories/GHSA-fx2h-pf6j-xcff>
- <https://github.com/advisories/GHSA-5xrq-8626-4rwp>
- <https://github.com/advisories/GHSA-x5fp-wj9c-mxmx>
- <https://github.com/advisories/GHSA-4mjr-xmp4-gh2g>

## Isolated provider checks

Default tests must not contact YouTube, open the real library, or play audio.
Use temporary directories and deterministic extractor/IPC fixtures for
regressions. Live checks must be explicit and bounded.

Run the existing public metadata probes from `src-tauri`:

```sh
cargo test imports_the_reported_youtu_be_url -- --ignored --nocapture
cargo test imports_a_live_youtube_playlist_with_track_metadata -- --ignored --nocapture
```

These imports must populate the test library and preserve its listening queue.
They must not auto-start playback. Record provider/network failures separately
from assertion failures. Never alter a test to manufacture a passing provider
response.

Native audio verification must use a disposable library, public content, and
explicit player cleanup. Verify that the player process exits even if a test
fails. Do not use the running user's library as a test fixture for mutations.

## Native smoke matrix

Record the application revision, build profile, tool versions, macOS version,
and actual result for each row. An unchecked row remains unverified.

- [ ] Launch with an empty library and no saved session.
- [ ] Launch with missing mpv and with missing yt-dlp.
- [ ] Search YouTube and import a selected result without starting playback.
- [ ] Import a video and a small playlist while another track is playing.
- [ ] Cancel import during discovery and enrichment without losing saved tracks.
- [ ] Retry failed metadata enrichment after network recovery.
- [ ] Edit mixed metadata and rename a reordered playlist without data loss.
- [ ] Refresh provider metadata after edits and after application restart.
- [ ] Start playback and verify real audio and advancing position.
- [ ] Seek, drag volume, mute, pause, resume, Previous, and Next.
- [ ] Skip the final item before EOF and confirm that audio stops.
- [ ] Let the final item reach EOF and confirm queue completion.
- [ ] Verify shuffle and every repeat mode through native playback.
- [ ] Keep Library, Queue, Mini, and Artwork open during transport changes.
- [ ] Toggle Favorites in each playback window and verify persistence.
- [ ] Clear the queue without removing library tracks.
- [ ] Save the queue as a playlist and verify the saved order.
- [ ] Use media keys and inspect the macOS Now Playing surface.
- [ ] Sleep and wake without unexpected audio or stale playing state.
- [ ] Change the default audio device and disconnect a Bluetooth output.
- [ ] Fail playback and verify visible error and usable recovery in each window.
- [ ] Fail initialization and an editor save; retry without losing input.
- [ ] Test Tab, Shift+Tab, Escape, arrows, Space, and focus return in dialogs.
- [ ] Test Library at 780 × 480 and each other window at its configured minimum.
- [ ] Test long metadata, validation errors, increased text size, and reduced
      motion.
- [ ] Verify VoiceOver labels, modal focus, and menu state.
- [ ] Change theme in Settings and verify every already-open window.
- [ ] Export a backup and restore a disposable copy of the library.
- [ ] Copy support information and verify it excludes credentials and library
      data.
- [ ] Disconnect an optional session during playback and verify local cleanup.
- [ ] Quit and verify no owned mpv or extractor process remains.

Browser tests with a stubbed Tauri bridge verify Vue rendering and command
routing only. They do not prove WKWebView behavior, macOS vibrancy, VoiceOver,
media-key handling, authenticated playback, or audio-device recovery.

## Library recovery

The active macOS database is:

```text
~/Library/Application Support/com.kyle.gmusic/library.sqlite3
```

The app stores normalized library data in SQLite. A migrated `library.json`
becomes `library.json.migrated`; it is not the active store. Keep that migration
backup until a verified current backup exists.

Use a consistent application backup when available. Do not copy an active SQLite
database without its transaction state. A raw file copy during a write can omit
committed data or create an unusable backup.

For manual recovery:

1. Quit G Music completely.
2. Preserve the entire application-data directory in a separate recovery copy.
3. Validate a candidate backup with SQLite's `PRAGMA integrity_check`.
4. Test the candidate in a disposable user environment.
5. Replace the active library only after explicit user approval.
6. Reopen G Music and verify tracks, playlist order, metadata, and paused state.

Do not delete the only copy of a corrupt database. Do not package the entire
application-data directory into a support report: it can contain a private
YouTube session. A library backup itself contains listening history and should
also be treated as private.

## Bundle and signing verification

Build an unsigned/ad-hoc development artifact with:

```sh
bun run tauri build --debug --bundles app
```

Build the release artifact with:

```sh
bun run tauri build --bundles app
```

Verify the release artifact after signing and notarization:

```sh
app='src-tauri/target/release/bundle/macos/G Music.app'
codesign --verify --deep --strict --verbose=2 "$app"
spctl --assess --type execute --verbose=2 "$app"
xcrun stapler validate "$app"
```

Record failures honestly. A successful bundle build does not satisfy the
signature, notarization, clean-install, or native smoke gates.
