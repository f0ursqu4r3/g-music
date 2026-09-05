# Professional player hardening implementation plan

> **For Hermes:** Use subagent-driven-development for implementation and
> independent review.

**Goal:** Resolve the current-state review findings and complete the defined
desktop player workflows without changing the user's live library.

**Architecture:** Keep Rust authoritative for transport, library, and jobs.
Preserve the existing Vue window layouts. Add regression tests at the provider,
command, store, and rendered-component boundaries.

**Tech stack:** Tauri 2, Rust, SQLite, mpv, yt-dlp, Vue 3, TypeScript, Reka UI,
Bun, Vitest.

## Boundaries

- Do not commit, push, use account credentials, or modify the live library.
- Use temporary directories and public media fixtures for verification.
- Preserve URL imports as library-only operations and restore playback paused.
- Keep downloads, recommendations, lyrics, equalizers, and account-library
  synchronization out of this scope.
- Target macOS first. Keep supported fallback builds compile-safe.
- Signing, notarization, service-policy approval, and user-account verification
  remain explicit external release gates.

## Work streams

Backend owns `src-tauri/`. Frontend owns `src/`. Integration owns documentation,
final review, and end-to-end verification. Do not edit another stream's files
before handoff.

For each task, first add and run a regression test that fails on the current
defect. Then implement the smallest complete fix and run the focused tests
again.

1. F01: Stop mpv before clearing the final queue item. Cover manual skip, EOF,
   and explicit Clear queue.
2. F02: Persist field-level user metadata ownership and provider baselines.
   Preserve overrides through imports, enrichment, and restart. Provide an
   explicit reset action. Protect legacy edited fields conservatively.
3. F03: Show mixed metadata values and update only changed fields. Preserve each
   untouched track value.
4. F04: Preserve playlist membership order. Remove deselected IDs in place and
   append new selections deterministically.
5. F05: Reject stale command/poll responses and coalesce pending invalidations.
   Register listeners before startup reads.
6. F06: Broadcast every semantic transport change from UI, native menus, queue
   completion, and media controls.
7. F07: Use authoritative favorite, shuffle, repeat, and starting state on every
   playback surface.
8. F08: Show initialization failure with Retry and operation errors in each
   window. Preserve failed editor drafts.
9. F09: Respect handled events and focused controls. Use accessible
   focus-managed dialogs and menus.
10. F10: Bound modal content and actions to the viewport. Match startup and
    recreated window minima.
11. F11: Replace full database rewrites with changed-row transactions. Move
    blocking transport work off the native event loop and expose
    pending/cancellation state.
12. F12: Synchronize themes across windows. Add Settings theme selection and
    semantic shared surfaces.
13. F13: Correct live import assertions and add behavioral regression coverage
    for these fixes.
14. Add fast library text search with ordered result playback and visible
    clear/empty states.
15. Add bounded YouTube search with preview metadata and explicit library
    import. Search must not modify the queue or library.
16. Add import cancellation and failed enrichment retry. Cancel and reap the
    extractor; keep already committed discoveries.
17. Add queue clearing and saving as an ordered playlist. Keep consumptive queue
    semantics explicit.
18. Add dependency diagnostics and privacy-safe support information in Settings.
    Use explicit system-default audio routing with recovery guidance.
19. Add native media-key and Now Playing integration where the platform supports
    it. Route actions through the authoritative backend command boundary.
20. Add safe user-triggered library backup/export and documented recovery. Do
    not export sessions or credentials.
21. Enable production CSP while retaining scoped cached artwork and Tauri IPC.
    Review cookie-write atomicity, disconnect, and error redaction.
22. Update README, DESIGN, and release/smoke documentation to match the actual
    implementation.

## Shared new command contracts

Existing commands retain their payloads. New commands use camelCase argument and
response fields.

- `search_youtube({ query: string }) -> MediaItem[]`: at most 20 normalized
  results, no mutation, bounded execution.
- `cancel_youtube_import({ runId: number }) -> void`: reject stale run IDs; emit
  terminal `cancelled` import phase.
- `retry_metadata_refreshes() -> MetadataRefreshSnapshot`: retry failed jobs and
  retain observable status.
- `clear_queue() -> PlaybackSnapshot`: stop playback, clear only the queue, emit
  authoritative state.
- `reset_track_metadata({ ids: string[] }) -> LibrarySnapshot`: reset user
  overrides to stored provider metadata, validate all IDs before mutation.
- `inspect_diagnostics()`: return string fields `appVersion`, `platform`, and
  `audioOutputPolicy`, plus a `dependencies` array. Each dependency has `name`
  and `message` strings, an `available` boolean, and a nullable `version` string.
- `export_library_backup() -> { path: string }`: create a consistent owner-only
  backup in the application backup directory; exclude authentication and caches.

Settings can show/copy privacy-safe diagnostics and the backup path. Search
imports use the existing import command. Queue saving uses the existing
playlist-upsert command. Add any extra shared contract to this document before
integrating it.

## Verification gates

1. Run focused regressions and record the actual red/green results.
2. Independently review implementation against F01–F13 and the workflow list.
3. Run `bun run test`, `bun run lint`, `bun run build`, and `bun run format`.
4. Run `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test`,
   and `cargo clippy --all-targets --all-features -- -D warnings` in
   `src-tauri`.
5. Test public metadata search/import and isolated native playback with bounded
   cleanup.
6. Exercise actual Vue components at supported minimum sizes with a stubbed
   Tauri bridge. Distinguish this evidence from native verification.
7. Build the macOS app bundle and inspect its signature. Do not claim
   notarization from an ad-hoc build.
8. Verify `git diff --check` and report unresolved external gates explicitly.
