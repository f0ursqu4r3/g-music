# Smart playlists and command palette implementation plan

> **For Hermes:** Use subagent-driven-development with spec and quality review.

**Goal:** Add durable rule-based playlists and fast keyboard access to music and
common actions without changing the existing window layout.

**Architecture:** Rust owns rule validation, evaluation, persistence, and resolved
playlist membership. Vue owns draft forms and command search. Reuse existing
playlist mutations, playback commands, SQLite transactions, and Reka dialogs.

**Tech Stack:** Rust, SQLite, Tauri 2, Vue 3, TypeScript, Reka UI, Vitest, Bun.

## Boundaries

- Do not commit, push, read credentials, or modify the live application library.
- Use disposable databases and mocked IPC for tests. Do not play real audio.
- Preserve manual playlists, Favorites, Most Played, queue semantics, and metadata.
- Do not add recommendations, account sync, audio downloads, or dependencies.
- Keep derived playlists out of manual membership editing and drop targets.
- Keep dialogs accessible, theme-aware, and usable at minimum window sizes.

## Shared contracts

These are new contracts extending the existing `Playlist` type in `src/api.ts`
and `src-tauri/src/playback/mod.rs`.

- Add optional nullable `smart: SmartPlaylistDefinition` to `Playlist`. Omit it
  when serializing ordinary playlists. Existing payloads remain valid.
- A definition has `match: 'all' | 'any'`, `rules: SmartPlaylistRule[]`,
  `sort: { field, direction: 'asc' | 'desc' }`, and `limit: number | null`.
- Sort fields are `libraryOrder`, `title`, `artist`, `album`, `durationMs`,
  `playCount`, and `lastPlayedAtMs`. Use library order as a stable tie-breaker.
  Put never-played values first in ascending last-played order and last in
  descending order.
- A rule has `field`, `operator`, and `value: string | number | boolean`.
- Text fields: `title`, `artist`, `album`, `label`, `genre`. Operators:
  `contains`, `equals`, `notContains`. Trim rule values; compare case-insensitively.
  Genre matching checks individual genres; `notContains` requires no match.
- Number fields: `durationMs`, `playCount`. Operators: `equals`, `lessThan`,
  `greaterThan`. Values are non-negative safe integers; duration uses milliseconds.
- Boolean field: `favorite`. Operator: `equals`, boolean value.
- Relative-time field: `lastPlayedDays`. Operators: `within`, `notWithin`.
  Value is an integer from 1 through 36500. Within includes the cutoff timestamp.
  Never-played tracks match `notWithin` and do not match `within`.
- Validate 1–20 rules, text values 1–200 characters after trim, and an optional
  integer limit from 1 through 10000. Reject invalid field/operator/value pairs,
  non-finite values, and unsupported fields before mutation.
- Evaluate only available tracks. Favorites use stable membership, not names.
  Use an explicit evaluation clock internally for deterministic boundary tests.
- `upsert_playlist({ playlist })` saves definitions and ignores supplied derived
  `trackIds` for smart playlists. Returned snapshots contain resolved track IDs.
  An ordinary upsert must reject an attempt to strip an existing smart definition;
  conversion uses the explicit command below. This protects legacy MCP clients.
- `preview_smart_playlist({ definition })` returns `totalMatches: number` and
  `matches: { trackId: string, matchedRuleIndexes: number[] }[]` in one object.
  `totalMatches` is before the limit; `matches` is sorted and limited. Preview
  never changes the library or queue.
- `freeze_smart_playlist({ id })` replaces that playlist's definition with its
  current resolved ordered membership and returns `LibrarySnapshot`. It preserves
  its ID and name. Reject ordinary/default/missing targets. Broadcast the existing
  library update event after successful writes.
- Persist definitions in a separate `smart_playlists` SQLite table keyed by the
  existing playlist ID with cascading deletion. Save in the existing transaction.
  Existing databases need no destructive schema changes. Backups include rules.
- `show_app_window({ surface })` accepts only `library`, `artwork`, `queue`,
  `mini`, `settings`, or `import`. Implement it in `src-tauri/src/windows.rs`
  through the existing `show_surface` lifecycle. Register it in `lib.rs` during
  integration. This closes the verified missing frontend window-opening seam
  without broad WebView creation permissions or duplicate native window layouts.

## Task 1: Backend rule engine and persistence

**Files:** Create `src-tauri/src/playback/smart.rs`; modify
`src-tauri/src/playback/mod.rs`, `src-tauri/src/playback/youtube.rs`,
`src-tauri/src/persistence.rs`, `src-tauri/src/commands.rs`,
`src-tauri/src/lib.rs`, and affected Rust tests and agent adapters.

1. Add failing rule tests for all/any, text, numbers, Favorites, relative time,
   never-played values, sort ties, limits, and invalid definitions.
2. Run the focused Rust tests and record the missing-feature failures.
3. Implement the pure rule engine with the shared contracts.
4. Add failing persistence/provider tests using temporary SQLite databases.
5. Integrate smart playlist snapshots, transactional saving, preview, freezing,
   deletion, ordinary-playlist compatibility, and legacy-agent write protection.
6. Verify reload, metadata/Favorites/history changes, unavailable tracks, rollback,
   backup retention, and freezing order without changing playback.
7. Register both Tauri commands and test the IPC contract.
8. Run Rust tests, formatting, check, and clippy.

## Task 2: Smart playlist UI

**Files:** Modify `src/api.ts`, `src/composables/usePlayback.ts`,
`src/components/LibraryWindow.vue`, `src/components/library/LibrarySidebar.vue`,
`src/App.vue`; create `src/components/library/SmartPlaylistEditor.vue` and nearby
tests. Reuse `LibraryDialog.vue` and existing playlist mutation error handling.

1. Add failing API and rendered component tests before implementation.
2. Add a compact New smart playlist action beside playlist creation.
3. Build a labeled rule editor with all/any, add/remove rules, sort, limit,
   validation, and explicit Preview action. Show total matches and bounded visible
   preview results with human-readable matched rules. Retain drafts on failure.
4. Offer Never played, Forgotten favorites, and Short tracks presets that fill
   the form but do not save without an explicit action.
5. Show smart playlists in existing navigation with a distinct restrained icon.
6. Route editing to the rule editor; block track drops and membership removal.
7. Add confirmed Convert to regular playlist and existing confirmed deletion.
8. Preserve displayed order for playback, selection, list/grid views, and normal
   playlist behavior. Reevaluate snapshots after semantic library changes and
   refresh time-relative membership periodically while the Library is focused.
9. Test creation, editing, failures, retry, conversion, deletion, and both blocked
   manual-membership paths through rendered controls and the real async owner.

## Task 3: Command palette

**Files:** Create `src/components/CommandPalette.vue`,
`src/lib/command-palette.ts`, and nearby tests; integrate in
`src/components/LibraryWindow.vue` and `src/App.vue` as appropriate.

1. Add failing result-ranking and rendered keyboard interaction tests.
2. Provide a visible Library header trigger and Command-K / Control-K.
3. Search tracks, albums, artists, and ordinary/smart playlists locally. Rank
   exact and prefix matches above substring matches and bound rendered results.
4. Let users play a result, play next, or add to queue with clear action labels.
   Preserve ordered collection contexts and await whole multi-track batches.
5. Expose New playlist, New smart playlist, Import, Queue, and Settings actions.
   Support adding current selected tracks to a chosen ordinary playlist.
6. Use the installed Reka dialog primitives for focus containment and return.
   Support arrows, Enter, Escape, empty results, busy states, and visible errors.
7. Ignore IME composition, repeated/handled shortcut events, and other open
   dialogs/menus. Do not interfere with existing Command-F or playback hotkeys.
8. Open from all app windows through a safe Library handoff if the compact window
   cannot host the palette. Use verified existing native window APIs, not invented
   command names. Keep the selected-track action Library-local.
9. Test actual IPC calls, keyboard routes, navigation, bounded results, empty
   library, pending mutations, errors, focus return, and listener cleanup.

## Task 4: Integration and verification

1. Run spec review against every acceptance item above.
2. Fix spec gaps before independent code-quality review.
3. Run `bun run test`, `bun run lint`, `bun run build`, and `bun run format`.
4. Run `cargo fmt --all -- --check`, `cargo check --locked --all-targets`,
   `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`
   from `src-tauri`.
5. Exercise the real Vue UI in a browser with an isolated synthetic IPC fixture.
   Verify palette keyboard behavior and smart editor layout at normal and
   minimum Library sizes. Do not describe this as native WKWebView verification.
6. Update README and release smoke scenarios to describe actual behavior.
7. Run `git diff --check` and report evidence and remaining native checks.

## Delivery status

- Backend and frontend implementation is present. Both passed spec review.
- Backend code-quality and final frontend integration reviews passed with no
  blocking findings.
- The full frontend suite passed: 414 tests across 43 files.
- The full Rust suite passed: 159 tests, with 5 opt-in tests ignored.
- Frontend lint, type checking, production build, and repository formatting passed.
- Rust formatting, all-target checks, and strict all-feature Clippy passed.
- Isolated Chromium checks passed for keyboard search, result bounds, exact IPC,
  draft retention and retry, match explanations, conversion confirmation,
  collection queue order, window actions, and minimum-size editor layout.
- The macOS debug app bundle built. Its configured signing and notarization
  completed. Signature verification, stapled-ticket validation, and Gatekeeper
  assessment passed. This does not establish a public release or native UI QA.
- No live-library mutation or real audio was used for these checks. The native
  smoke matrix remains unchecked in `docs/release-readiness.md`.

## Follow-on priorities

These remain planned work, not completed features:

1. Library health with reviewable, reversible metadata and duplicate cleanup.
2. Saved listening sessions with track position, queue, shuffle, and repeat state.
3. In-app backup restore and verified installation/update procedures.

Local-file audio is a separate provider expansion. Do not add it implicitly to
this release or describe the current YouTube provider as provider-neutral.
