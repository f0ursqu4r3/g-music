// Browser-only synthetic IPC. No native process, network media, or persisted library.
;(() => {
  const clone = (value) => (value === undefined ? value : structuredClone(value))
  const tracks = Array.from({ length: 85 }, (_, index) => ({
    id: `synthetic-${index}`,
    title: `Synthetic track ${String(index).padStart(2, '0')}`,
    artist: `Artist ${index % 4}`,
    album: `Album ${index % 6}`,
    durationMs: 120000 + index * 1000,
    playCount: 0,
  }))
  const smart = {
    match: 'all',
    rules: [{ field: 'playCount', operator: 'equals', value: 0 }],
    sort: { field: 'libraryOrder', direction: 'asc' },
    limit: null,
  }
  const library = {
    tracks,
    playlists: [
      {
        id: 'smart',
        name: 'Never played',
        smart,
        trackIds: tracks
          .slice(0, 5)
          .reverse()
          .map((t) => t.id),
      },
      { id: 'manual', name: 'Manual', trackIds: [tracks[2].id] },
    ],
  }
  let state = { currentItem: null, queue: [], status: 'paused', positionMs: 0, volumePercent: 50 }
  const callbacks = new Map()
  const listeners = new Map()
  let sequence = 0
  window.fixtureCalls = []
  window.fixtureFailures = new Set()
  window.fixtureErrors = []
  addEventListener('error', (event) => window.fixtureErrors.push(event.message))
  addEventListener('unhandledrejection', (event) => window.fixtureErrors.push(String(event.reason)))
  const emit = (event, payload) => {
    for (const [id, listener] of listeners)
      if (listener.event === event)
        callbacks.get(listener.handler)?.({ id, event, payload: clone(payload) })
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_, id) => listeners.delete(id) }
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    transformCallback(callback) {
      const id = ++sequence
      callbacks.set(id, callback)
      return id
    },
    unregisterCallback(id) {
      callbacks.delete(id)
    },
    convertFileSrc(path) {
      return path
    },
    async invoke(command, args = {}) {
      window.fixtureCalls.push({ command, args: clone(args) })
      if (window.fixtureFailures.has(command)) throw { message: 'Synthetic failure. Try again.' }
      switch (command) {
        case 'plugin:event|listen': {
          const id = ++sequence
          listeners.set(id, args)
          return id
        }
        case 'plugin:event|unlisten':
          listeners.delete(args.eventId)
          return
        case 'plugin:event|emit_to':
        case 'plugin:event|emit':
          emit(args.event, args.payload)
          return
        case 'plugin:window|is_focused':
          return true
        case 'resolve_youtube_artwork':
        case 'inspect_import_progress':
          return null
        case 'inspect_library':
          return clone(library)
        case 'inspect_playback':
        case 'inspect_playback_transport':
          return clone(state)
        case 'inspect_metadata_refreshes':
          return { jobs: [], totalTracks: 0, completedTracks: 0 }
        case 'show_app_window':
        case 'show_import_window':
          return
        // This fixture checks rendering and IPC, not Rust rule evaluation.
        case 'preview_smart_playlist':
          return {
            totalMatches: tracks.length,
            matches: tracks.slice(0, args.definition.limit ?? tracks.length).map((track) => ({
              trackId: track.id,
              matchedRuleIndexes: args.definition.rules.map((_, i) => i),
            })),
          }
        case 'upsert_playlist': {
          const value = clone(args.playlist)
          if (value.smart) value.trackIds = tracks.slice(0, value.smart.limit ?? 5).map((t) => t.id)
          const index = library.playlists.findIndex((p) => p.id === value.id)
          if (index < 0) library.playlists.push(value)
          else library.playlists[index] = value
          emit('library-updated', null)
          return clone(library)
        }
        case 'freeze_smart_playlist': {
          delete library.playlists.find((p) => p.id === args.id).smart
          emit('library-updated', null)
          return clone(library)
        }
        case 'delete_playlist':
          library.playlists = library.playlists.filter((p) => p.id !== args.id)
          emit('library-updated', null)
          return clone(library)
        case 'play_track':
          state = {
            ...state,
            currentItem: tracks.find((t) => t.id === args.id),
            queue: (args.queueIds ?? [args.id]).map((id) => tracks.find((t) => t.id === id)),
            status: 'playing',
          }
          break
        case 'add_to_queue':
          state.queue = [
            ...state.queue.filter((t) => t.id !== args.id),
            tracks.find((t) => t.id === args.id),
          ]
          break
        case 'queue_track_next': {
          state.queue = state.queue.filter((t) => t.id !== args.id)
          state.queue.splice(
            Math.max(0, state.queue.findIndex((t) => t.id === state.currentItem?.id) + 1),
            0,
            tracks.find((t) => t.id === args.id),
          )
          break
        }
        default:
          throw new Error(`Unsupported synthetic command: ${command}`)
      }
      emit('playback-updated', state)
      return clone(state)
    },
  }
})()
