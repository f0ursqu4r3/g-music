use std::{
    collections::HashSet,
    fs,
    fs::OpenOptions,
    os::unix::fs::OpenOptionsExt,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::playback::{EditableTrackMetadata, MediaItem, Playlist, QueueEntry, RepeatMode};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedPlaybackState {
    #[serde(default)]
    pub(crate) current_item_id: Option<String>,
    #[serde(default)]
    pub(crate) position_ms: u64,
    #[serde(default = "default_volume_percent")]
    pub(crate) volume_percent: u8,
    #[serde(default)]
    pub(crate) shuffle_enabled: bool,
    #[serde(default)]
    pub(crate) repeat_mode: RepeatMode,
    #[serde(default)]
    pub(crate) queue_ids: Vec<String>,
    #[serde(default)]
    pub(crate) shuffle_order: Vec<String>,
}

fn default_volume_percent() -> u8 {
    72
}

static BACKUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn export_library_backup(database_path: &Path) -> Result<PathBuf, String> {
    if !database_path.is_file() {
        return Err(format!(
            "the library database does not exist: {}",
            database_path.display()
        ));
    }
    let parent = database_path
        .parent()
        .ok_or_else(|| "the library database has no parent directory".to_string())?;
    let backup_directory = parent.join("backups");
    fs::create_dir_all(&backup_directory).map_err(error)?;
    fs::set_permissions(&backup_directory, fs::Permissions::from_mode(0o700)).map_err(error)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(error)?
        .as_millis();
    let sequence = BACKUP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let backup_path = backup_directory.join(format!(
        "library-backup-{timestamp}-{}-{sequence}.sqlite3",
        std::process::id()
    ));

    let result = (|| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&backup_path)
            .map_err(error)?;
        let source = Connection::open(database_path).map_err(error)?;
        source
            .execute("VACUUM INTO ?1", [backup_path.to_string_lossy().as_ref()])
            .map_err(error)?;

        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600)).map_err(error)?;
        let mut backup = Connection::open(&backup_path).map_err(error)?;
        backup
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(error)?;
        let transaction = backup.transaction().map_err(error)?;
        transaction
            .execute("DELETE FROM artwork_cache", [])
            .map_err(error)?;
        transaction
            .execute("DELETE FROM playback_state", [])
            .map_err(error)?;
        transaction.commit().map_err(error)?;
        backup.execute_batch("VACUUM;").map_err(error)?;
        Ok(())
    })();
    if let Err(export_error) = result {
        let _ = fs::remove_file(&backup_path);
        return Err(export_error);
    }
    Ok(backup_path)
}

pub(crate) fn load_library(path: &Path) -> Result<(Vec<QueueEntry>, Vec<Playlist>), String> {
    let connection = open(path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, provider, source_url, title, artist, album, album_artist, track_number,
                    disc_number, release_date, upload_date, description, channel, channel_id,
                    uploader, uploader_id, thumbnail_url, label, genres_json, categories_json,
                    tags_json, language, availability, is_live, view_count, like_count, duration_ms,
                    metadata_dirty, play_count, last_played_at_ms,
                    track_metadata.provider_json, track_metadata.overrides_json
             FROM tracks
             LEFT JOIN track_metadata ON track_metadata.track_id = tracks.id
             ORDER BY library_position",
        )
        .map_err(error)?;
    let mut entries = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let history =
                read_history(&connection, &id).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let provider_json: Option<String> = row.get(30)?;
            let provider_metadata = provider_json
                .map(|value| serde_json::from_str::<EditableTrackMetadata>(&value))
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        30,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let overrides_json: Option<String> = row.get(31)?;
            let metadata_overrides = overrides_json
                .map(|value| serde_json::from_str::<HashSet<String>>(&value))
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        31,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?
                .unwrap_or_default();
            Ok(QueueEntry {
                source_url: row.get(2)?,
                provider_metadata,
                metadata_overrides,
                item: MediaItem {
                    id,
                    provider: row.get(1)?,
                    source_url: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    album: row.get(5)?,
                    album_artist: row.get(6)?,
                    track_number: row.get(7)?,
                    disc_number: row.get(8)?,
                    release_date: row.get(9)?,
                    upload_date: row.get(10)?,
                    description: row.get(11)?,
                    channel: row.get(12)?,
                    channel_id: row.get(13)?,
                    uploader: row.get(14)?,
                    uploader_id: row.get(15)?,
                    thumbnail_url: row.get(16)?,
                    label: row.get(17)?,
                    genres: decode_values(row.get(18)?),
                    categories: decode_values(row.get(19)?),
                    tags: decode_values(row.get(20)?),
                    language: row.get(21)?,
                    availability: row.get(22)?,
                    is_live: row.get::<_, i64>(23)? != 0,
                    view_count: row.get(24)?,
                    like_count: row.get(25)?,
                    duration_ms: row.get(26)?,
                    metadata_dirty: row.get::<_, i64>(27)? != 0,
                    play_count: row.get(28)?,
                    last_played_at_ms: row.get(29)?,
                    play_history_ms: history,
                },
            })
        })
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;

    let mut playlist_statement = connection
        .prepare(
            "SELECT playlists.id, name, definition_json FROM playlists
                  LEFT JOIN smart_playlists ON smart_playlists.playlist_id = playlists.id
                  ORDER BY library_position",
        )
        .map_err(error)?;
    let playlists = playlist_statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let mut track_statement = connection
                .prepare(
                    "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
                )
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let track_ids = track_statement
                .query_map([&id], |track_row| track_row.get(0))
                .map_err(|_| rusqlite::Error::InvalidQuery)?
                .collect::<Result<Vec<String>, _>>()?;
            let definition_json: Option<String> = row.get(2)?;
            let smart = definition_json
                .map(|value| {
                    serde_json::from_str::<crate::playback::SmartPlaylistDefinition>(&value)
                        .map_err(|error| error.to_string())?
                        .normalized()
                })
                .transpose()
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::other(error)),
                    )
                })?;
            Ok(Playlist {
                track_ids: if smart.is_some() {
                    Vec::new()
                } else {
                    track_ids
                },
                smart,
                id,
                name: row.get(1)?,
            })
        })
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;

    entries.shrink_to_fit();
    Ok((entries, playlists))
}

pub(crate) fn save_library(
    path: &Path,
    entries: &[QueueEntry],
    playlists: &[Playlist],
) -> Result<(), String> {
    let mut connection = open(path)?;
    let transaction = connection.transaction().map_err(error)?;
    save_library_rows(&transaction, entries, playlists)?;
    transaction.commit().map_err(error)
}

pub(crate) fn save_library_and_playback(
    path: &Path,
    entries: &[QueueEntry],
    playlists: &[Playlist],
    state: &SavedPlaybackState,
) -> Result<(), String> {
    let state_json = serde_json::to_string(state).map_err(error)?;
    let mut connection = open(path)?;
    let transaction = connection.transaction().map_err(error)?;
    save_library_rows(&transaction, entries, playlists)?;
    write_playback_state(&transaction, &state_json)?;
    transaction.commit().map_err(error)
}

fn save_library_rows(
    transaction: &rusqlite::Transaction<'_>,
    entries: &[QueueEntry],
    playlists: &[Playlist],
) -> Result<(), String> {
    for playlist in playlists {
        if let Some(definition) = &playlist.smart {
            if matches!(playlist.id.as_str(), "favorites" | "most-played") {
                return Err("default playlists cannot have smart definitions".into());
            }
            definition.validate()?;
        }
    }
    transaction
        .execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS desired_track_ids (id TEXT PRIMARY KEY);
             CREATE TEMP TABLE IF NOT EXISTS desired_playlist_ids (id TEXT PRIMARY KEY);
             DELETE FROM desired_track_ids;
             DELETE FROM desired_playlist_ids;",
        )
        .map_err(error)?;

    {
        let mut insert_desired = transaction
            .prepare("INSERT INTO desired_track_ids (id) VALUES (?1)")
            .map_err(error)?;
        for entry in entries {
            insert_desired.execute([&entry.item.id]).map_err(error)?;
        }
    }
    transaction
        .execute(
            "DELETE FROM tracks
             WHERE NOT EXISTS (SELECT 1 FROM desired_track_ids WHERE id = tracks.id)",
            [],
        )
        .map_err(error)?;

    {
        let mut read_position = transaction
            .prepare("SELECT library_position FROM tracks WHERE id = ?1")
            .map_err(error)?;
        let mut stage_position = transaction
            .prepare("UPDATE tracks SET library_position = -library_position - 1 WHERE id = ?1")
            .map_err(error)?;
        for (position, entry) in entries.iter().enumerate() {
            let stored_position = read_position
                .query_row([&entry.item.id], |row| row.get::<_, i64>(0))
                .optional()
                .map_err(error)?;
            if stored_position.is_some_and(|stored| stored != position as i64) {
                stage_position.execute([&entry.item.id]).map_err(error)?;
            }
        }
    }

    {
        let mut upsert_track = transaction
            .prepare(
                "INSERT INTO tracks (
                    id, provider, source_url, title, artist, album, album_artist, track_number,
                    disc_number, release_date, upload_date, description, channel, channel_id,
                    uploader, uploader_id, thumbnail_url, label, genres_json, categories_json,
                    tags_json, language, availability, is_live, view_count, like_count, duration_ms,
                    metadata_dirty, play_count, last_played_at_ms, library_position
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
                 )
                 ON CONFLICT(id) DO UPDATE SET
                    provider = excluded.provider,
                    source_url = excluded.source_url,
                    title = excluded.title,
                    artist = excluded.artist,
                    album = excluded.album,
                    album_artist = excluded.album_artist,
                    track_number = excluded.track_number,
                    disc_number = excluded.disc_number,
                    release_date = excluded.release_date,
                    upload_date = excluded.upload_date,
                    description = excluded.description,
                    channel = excluded.channel,
                    channel_id = excluded.channel_id,
                    uploader = excluded.uploader,
                    uploader_id = excluded.uploader_id,
                    thumbnail_url = excluded.thumbnail_url,
                    label = excluded.label,
                    genres_json = excluded.genres_json,
                    categories_json = excluded.categories_json,
                    tags_json = excluded.tags_json,
                    language = excluded.language,
                    availability = excluded.availability,
                    is_live = excluded.is_live,
                    view_count = excluded.view_count,
                    like_count = excluded.like_count,
                    duration_ms = excluded.duration_ms,
                    metadata_dirty = excluded.metadata_dirty,
                    play_count = excluded.play_count,
                    last_played_at_ms = excluded.last_played_at_ms,
                    library_position = excluded.library_position
                 WHERE tracks.provider IS NOT excluded.provider
                    OR tracks.source_url IS NOT excluded.source_url
                    OR tracks.title IS NOT excluded.title
                    OR tracks.artist IS NOT excluded.artist
                    OR tracks.album IS NOT excluded.album
                    OR tracks.album_artist IS NOT excluded.album_artist
                    OR tracks.track_number IS NOT excluded.track_number
                    OR tracks.disc_number IS NOT excluded.disc_number
                    OR tracks.release_date IS NOT excluded.release_date
                    OR tracks.upload_date IS NOT excluded.upload_date
                    OR tracks.description IS NOT excluded.description
                    OR tracks.channel IS NOT excluded.channel
                    OR tracks.channel_id IS NOT excluded.channel_id
                    OR tracks.uploader IS NOT excluded.uploader
                    OR tracks.uploader_id IS NOT excluded.uploader_id
                    OR tracks.thumbnail_url IS NOT excluded.thumbnail_url
                    OR tracks.label IS NOT excluded.label
                    OR tracks.genres_json IS NOT excluded.genres_json
                    OR tracks.categories_json IS NOT excluded.categories_json
                    OR tracks.tags_json IS NOT excluded.tags_json
                    OR tracks.language IS NOT excluded.language
                    OR tracks.availability IS NOT excluded.availability
                    OR tracks.is_live IS NOT excluded.is_live
                    OR tracks.view_count IS NOT excluded.view_count
                    OR tracks.like_count IS NOT excluded.like_count
                    OR tracks.duration_ms IS NOT excluded.duration_ms
                    OR tracks.metadata_dirty IS NOT excluded.metadata_dirty
                    OR tracks.play_count IS NOT excluded.play_count
                    OR tracks.last_played_at_ms IS NOT excluded.last_played_at_ms
                    OR tracks.library_position IS NOT excluded.library_position",
            )
            .map_err(error)?;
        for (position, entry) in entries.iter().enumerate() {
            let item = &entry.item;
            upsert_track
                .execute(params![
                    item.id,
                    item.provider,
                    entry.source_url,
                    item.title,
                    item.artist,
                    item.album,
                    item.album_artist,
                    item.track_number,
                    item.disc_number,
                    item.release_date,
                    item.upload_date,
                    item.description,
                    item.channel,
                    item.channel_id,
                    item.uploader,
                    item.uploader_id,
                    item.thumbnail_url,
                    item.label,
                    encode_values(&item.genres),
                    encode_values(&item.categories),
                    encode_values(&item.tags),
                    item.language,
                    item.availability,
                    i64::from(item.is_live),
                    item.view_count,
                    item.like_count,
                    item.duration_ms,
                    i64::from(item.metadata_dirty),
                    item.play_count,
                    item.last_played_at_ms,
                    position as i64,
                ])
                .map_err(error)?;
        }
    }

    {
        let mut upsert_history = transaction
            .prepare(
                "INSERT INTO play_history (track_id, position, played_at_ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT(track_id, position) DO UPDATE SET played_at_ms = excluded.played_at_ms
                 WHERE play_history.played_at_ms IS NOT excluded.played_at_ms",
            )
            .map_err(error)?;
        let mut delete_stale_history = transaction
            .prepare("DELETE FROM play_history WHERE track_id = ?1 AND position >= ?2")
            .map_err(error)?;
        for entry in entries {
            for (position, played_at_ms) in entry.item.play_history_ms.iter().enumerate() {
                upsert_history
                    .execute(params![entry.item.id, position as i64, played_at_ms])
                    .map_err(error)?;
            }
            delete_stale_history
                .execute(params![
                    entry.item.id,
                    entry.item.play_history_ms.len() as i64
                ])
                .map_err(error)?;
        }
    }

    {
        let mut upsert_metadata = transaction
            .prepare(
                "INSERT INTO track_metadata (track_id, provider_json, overrides_json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(track_id) DO UPDATE SET
                    provider_json = excluded.provider_json,
                    overrides_json = excluded.overrides_json
                 WHERE track_metadata.provider_json IS NOT excluded.provider_json
                    OR track_metadata.overrides_json IS NOT excluded.overrides_json",
            )
            .map_err(error)?;
        let mut delete_metadata = transaction
            .prepare("DELETE FROM track_metadata WHERE track_id = ?1")
            .map_err(error)?;
        for entry in entries {
            if entry.provider_metadata.is_none() && entry.metadata_overrides.is_empty() {
                delete_metadata.execute([&entry.item.id]).map_err(error)?;
                continue;
            }
            let provider_json = entry
                .provider_metadata
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(error)?;
            let overrides_json = encode_overrides(&entry.metadata_overrides).map_err(error)?;
            upsert_metadata
                .execute(params![entry.item.id, provider_json, overrides_json])
                .map_err(error)?;
        }
    }

    {
        let mut insert_desired = transaction
            .prepare("INSERT INTO desired_playlist_ids (id) VALUES (?1)")
            .map_err(error)?;
        for playlist in playlists {
            insert_desired.execute([&playlist.id]).map_err(error)?;
        }
    }
    transaction
        .execute(
            "DELETE FROM playlists
             WHERE NOT EXISTS (SELECT 1 FROM desired_playlist_ids WHERE id = playlists.id)",
            [],
        )
        .map_err(error)?;

    {
        let mut read_position = transaction
            .prepare("SELECT library_position FROM playlists WHERE id = ?1")
            .map_err(error)?;
        let mut stage_position = transaction
            .prepare("UPDATE playlists SET library_position = -library_position - 1 WHERE id = ?1")
            .map_err(error)?;
        for (position, playlist) in playlists.iter().enumerate() {
            let stored_position = read_position
                .query_row([&playlist.id], |row| row.get::<_, i64>(0))
                .optional()
                .map_err(error)?;
            if stored_position.is_some_and(|stored| stored != position as i64) {
                stage_position.execute([&playlist.id]).map_err(error)?;
            }
        }
    }

    {
        let mut upsert_playlist = transaction
            .prepare(
                "INSERT INTO playlists (id, name, library_position) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    library_position = excluded.library_position
                 WHERE playlists.name IS NOT excluded.name
                    OR playlists.library_position IS NOT excluded.library_position",
            )
            .map_err(error)?;
        let mut upsert_member = transaction
            .prepare(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)
                 ON CONFLICT(playlist_id, position) DO UPDATE SET track_id = excluded.track_id
                 WHERE playlist_tracks.track_id IS NOT excluded.track_id",
            )
            .map_err(error)?;
        let mut delete_stale_members = transaction
            .prepare("DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position >= ?2")
            .map_err(error)?;
        let mut upsert_smart = transaction
            .prepare(
                "INSERT INTO smart_playlists (playlist_id, definition_json) VALUES (?1, ?2)
             ON CONFLICT(playlist_id) DO UPDATE SET definition_json = excluded.definition_json
             WHERE smart_playlists.definition_json IS NOT excluded.definition_json",
            )
            .map_err(error)?;
        let mut delete_smart = transaction
            .prepare("DELETE FROM smart_playlists WHERE playlist_id = ?1")
            .map_err(error)?;
        for (position, playlist) in playlists.iter().enumerate() {
            upsert_playlist
                .execute(params![playlist.id, playlist.name, position as i64])
                .map_err(error)?;
            let track_ids = if let Some(definition) = &playlist.smart {
                upsert_smart
                    .execute(params![
                        playlist.id,
                        serde_json::to_string(definition).map_err(error)?
                    ])
                    .map_err(error)?;
                &[][..]
            } else {
                delete_smart.execute([&playlist.id]).map_err(error)?;
                playlist.track_ids.as_slice()
            };
            for (track_position, track_id) in track_ids.iter().enumerate() {
                upsert_member
                    .execute(params![playlist.id, track_id, track_position as i64])
                    .map_err(error)?;
            }
            delete_stale_members
                .execute(params![playlist.id, track_ids.len() as i64])
                .map_err(error)?;
        }
    }

    Ok(())
}

pub(crate) fn load_playback_state(path: &Path) -> Result<Option<SavedPlaybackState>, String> {
    let connection = open(path)?;
    let state_json = connection
        .query_row(
            "SELECT state_json FROM playback_state WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(error)?;
    state_json
        .map(|state_json| serde_json::from_str(&state_json).map_err(error))
        .transpose()
}

pub(crate) fn save_playback_state(path: &Path, state: &SavedPlaybackState) -> Result<(), String> {
    let connection = open(path)?;
    let state_json = serde_json::to_string(state).map_err(error)?;
    write_playback_state(&connection, &state_json)
}

fn write_playback_state(connection: &Connection, state_json: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO playback_state (id, state_json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET state_json = excluded.state_json",
            [state_json],
        )
        .map_err(error)?;
    Ok(())
}

pub(crate) fn database_exists(path: &Path) -> bool {
    path.is_file()
}

fn open(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error)?;
    }
    let connection = Connection::open(path).map_err(error)?;
    if path.is_file() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(error)?;
    }
    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                source_url TEXT NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT,
                album_artist TEXT,
                track_number INTEGER,
                disc_number INTEGER,
                release_date TEXT,
                upload_date TEXT,
                description TEXT,
                channel TEXT,
                channel_id TEXT,
                uploader TEXT,
                uploader_id TEXT,
                thumbnail_url TEXT,
                label TEXT,
                genres_json TEXT NOT NULL,
                categories_json TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                language TEXT,
                availability TEXT,
                is_live INTEGER NOT NULL,
                view_count INTEGER,
                like_count INTEGER,
                duration_ms INTEGER NOT NULL,
                metadata_dirty INTEGER NOT NULL,
                play_count INTEGER NOT NULL,
                last_played_at_ms INTEGER,
                library_position INTEGER NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS playlists (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                library_position INTEGER NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS smart_playlists (
                playlist_id TEXT PRIMARY KEY REFERENCES playlists(id) ON DELETE CASCADE,
                definition_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS playlist_tracks (
                playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                PRIMARY KEY (playlist_id, position)
            );
            CREATE TABLE IF NOT EXISTS play_history (
                track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                played_at_ms INTEGER NOT NULL,
                PRIMARY KEY (track_id, position)
            );
            CREATE TABLE IF NOT EXISTS track_metadata (
                track_id TEXT PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                provider_json TEXT,
                overrides_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS artwork_cache (
                cache_key TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL,
                byte_size INTEGER NOT NULL,
                last_accessed_at_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS playback_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                state_json TEXT NOT NULL
            );
            ",
        )
        .map_err(error)?;
    Ok(connection)
}

fn read_history(connection: &Connection, track_id: &str) -> Result<Vec<u64>, String> {
    let mut statement = connection
        .prepare("SELECT played_at_ms FROM play_history WHERE track_id = ?1 ORDER BY position")
        .map_err(error)?;
    statement
        .query_map([track_id], |row| row.get(0))
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)
}

fn encode_values(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".into())
}

fn decode_values(value: String) -> Vec<String> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn encode_overrides(overrides: &HashSet<String>) -> Result<String, serde_json::Error> {
    let mut values = overrides.iter().collect::<Vec<_>>();
    values.sort_unstable();
    serde_json::to_string(&values)
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(crate) fn read_artwork_cache(
    database_path: &Path,
    cache_key: &str,
    accessed_at_ms: u64,
) -> Result<Option<String>, String> {
    let connection = open(database_path)?;
    let relative_path = connection
        .query_row(
            "SELECT relative_path FROM artwork_cache WHERE cache_key = ?1",
            [cache_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(error)?;
    if relative_path.is_some() {
        connection
            .execute(
                "UPDATE artwork_cache SET last_accessed_at_ms = ?2 WHERE cache_key = ?1",
                params![cache_key, accessed_at_ms],
            )
            .map_err(error)?;
    }
    Ok(relative_path)
}

pub(crate) fn write_artwork_cache(
    database_path: &Path,
    cache_key: &str,
    relative_path: &str,
    byte_size: u64,
    accessed_at_ms: u64,
    maximum_bytes: u64,
) -> Result<Vec<String>, String> {
    let mut connection = open(database_path)?;
    let transaction = connection.transaction().map_err(error)?;
    transaction
        .execute(
            "INSERT INTO artwork_cache (cache_key, relative_path, byte_size, last_accessed_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(cache_key) DO UPDATE SET relative_path = excluded.relative_path,
                 byte_size = excluded.byte_size, last_accessed_at_ms = excluded.last_accessed_at_ms",
            params![cache_key, relative_path, byte_size, accessed_at_ms],
        )
        .map_err(error)?;
    let total: u64 = transaction
        .query_row(
            "SELECT COALESCE(SUM(byte_size), 0) FROM artwork_cache",
            [],
            |row| row.get(0),
        )
        .map_err(error)?;
    let mut remaining = total;
    let mut evicted = Vec::new();
    if remaining > maximum_bytes {
        let mut statement = transaction
            .prepare(
                "SELECT cache_key, relative_path, byte_size FROM artwork_cache
                 ORDER BY last_accessed_at_ms, cache_key",
            )
            .map_err(error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        drop(statement);
        for (key, path, size) in rows {
            if remaining <= maximum_bytes {
                break;
            }
            transaction
                .execute("DELETE FROM artwork_cache WHERE cache_key = ?1", [key])
                .map_err(error)?;
            remaining = remaining.saturating_sub(size);
            evicted.push(path);
        }
    }
    transaction.commit().map_err(error)?;
    Ok(evicted)
}

pub(crate) fn remove_artwork_cache(database_path: &Path, cache_key: &str) -> Result<(), String> {
    let connection = open(database_path)?;
    connection
        .execute(
            "DELETE FROM artwork_cache WHERE cache_key = ?1",
            [cache_key],
        )
        .map_err(error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use rusqlite::Connection;

    use super::{
        SavedPlaybackState, export_library_backup, load_library, load_playback_state, save_library,
        save_library_and_playback, save_playback_state, write_artwork_cache,
    };
    use crate::playback::{EditableTrackMetadata, MediaItem, Playlist, QueueEntry, RepeatMode};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct DatabaseFixture {
        directory: PathBuf,
        path: PathBuf,
    }

    impl DatabaseFixture {
        fn new(name: &str) -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "gmusic-persistence-{name}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&directory).expect("the temporary fixture directory is created");
            let path = directory.join("library.sqlite3");
            Self { directory, path }
        }
    }

    impl Drop for DatabaseFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn entry(index: usize) -> QueueEntry {
        let id = format!("track-{index:04}");
        let source_url = format!("https://example.test/{id}");
        QueueEntry {
            source_url: source_url.clone(),
            provider_metadata: None,
            metadata_overrides: Default::default(),
            item: MediaItem {
                id,
                provider: "test".into(),
                source_url: Some(source_url),
                title: format!("Track {index}"),
                artist: "Artist".into(),
                album: None,
                album_artist: None,
                track_number: None,
                disc_number: None,
                release_date: None,
                upload_date: None,
                description: None,
                channel: None,
                channel_id: None,
                uploader: None,
                uploader_id: None,
                thumbnail_url: None,
                label: None,
                genres: Vec::new(),
                categories: Vec::new(),
                tags: Vec::new(),
                language: None,
                availability: None,
                is_live: false,
                view_count: None,
                like_count: None,
                duration_ms: 180_000,
                metadata_dirty: false,
                play_count: 0,
                last_played_at_ms: None,
                play_history_ms: Vec::new(),
            },
        }
    }

    fn install_write_audit(path: &Path) {
        let connection = Connection::open(path).expect("the fixture database opens");
        connection
            .execute_batch(
                "
                CREATE TABLE write_audit (table_name TEXT NOT NULL, operation TEXT NOT NULL);
                CREATE TRIGGER audit_tracks_insert AFTER INSERT ON tracks BEGIN
                    INSERT INTO write_audit VALUES ('tracks', 'insert');
                END;
                CREATE TRIGGER audit_tracks_update AFTER UPDATE ON tracks BEGIN
                    INSERT INTO write_audit VALUES ('tracks', 'update');
                END;
                CREATE TRIGGER audit_tracks_delete AFTER DELETE ON tracks BEGIN
                    INSERT INTO write_audit VALUES ('tracks', 'delete');
                END;
                CREATE TRIGGER audit_history_insert AFTER INSERT ON play_history BEGIN
                    INSERT INTO write_audit VALUES ('play_history', 'insert');
                END;
                CREATE TRIGGER audit_history_delete AFTER DELETE ON play_history BEGIN
                    INSERT INTO write_audit VALUES ('play_history', 'delete');
                END;
                CREATE TRIGGER audit_metadata_insert AFTER INSERT ON track_metadata BEGIN
                    INSERT INTO write_audit VALUES ('track_metadata', 'insert');
                END;
                CREATE TRIGGER audit_metadata_update AFTER UPDATE ON track_metadata BEGIN
                    INSERT INTO write_audit VALUES ('track_metadata', 'update');
                END;
                CREATE TRIGGER audit_metadata_delete AFTER DELETE ON track_metadata BEGIN
                    INSERT INTO write_audit VALUES ('track_metadata', 'delete');
                END;
                CREATE TRIGGER audit_playlists_insert AFTER INSERT ON playlists BEGIN
                    INSERT INTO write_audit VALUES ('playlists', 'insert');
                END;
                CREATE TRIGGER audit_playlists_update AFTER UPDATE ON playlists BEGIN
                    INSERT INTO write_audit VALUES ('playlists', 'update');
                END;
                CREATE TRIGGER audit_playlists_delete AFTER DELETE ON playlists BEGIN
                    INSERT INTO write_audit VALUES ('playlists', 'delete');
                END;
                CREATE TRIGGER audit_members_insert AFTER INSERT ON playlist_tracks BEGIN
                    INSERT INTO write_audit VALUES ('playlist_tracks', 'insert');
                END;
                CREATE TRIGGER audit_members_update AFTER UPDATE ON playlist_tracks BEGIN
                    INSERT INTO write_audit VALUES ('playlist_tracks', 'update');
                END;
                CREATE TRIGGER audit_members_delete AFTER DELETE ON playlist_tracks BEGIN
                    INSERT INTO write_audit VALUES ('playlist_tracks', 'delete');
                END;
                ",
            )
            .expect("write-audit triggers are installed");
    }

    fn audit_count(path: &Path, table_name: &str) -> u64 {
        let connection = Connection::open(path).expect("the fixture database opens");
        connection
            .query_row(
                "SELECT COUNT(*) FROM write_audit WHERE table_name = ?1",
                [table_name],
                |row| row.get(0),
            )
            .expect("the audit count is readable")
    }

    fn clear_audit(path: &Path) {
        Connection::open(path)
            .expect("the fixture database opens")
            .execute("DELETE FROM write_audit", [])
            .expect("the write audit is cleared");
    }

    #[test]
    fn saves_only_rows_changed_in_a_large_library() {
        let fixture = DatabaseFixture::new("changed-rows");
        let mut entries = (0..2_000).map(entry).collect::<Vec<_>>();
        let mut playlists = vec![Playlist {
            smart: None,
            id: "favorites".into(),
            name: "Favorites".into(),
            track_ids: Vec::new(),
        }];
        save_library(&fixture.path, &entries, &playlists).expect("the initial library is saved");
        install_write_audit(&fixture.path);

        entries[1_127].item.title = "Corrected title".into();
        entries[1_127].item.metadata_dirty = true;
        save_library(&fixture.path, &entries, &playlists).expect("metadata is saved");
        assert_eq!(audit_count(&fixture.path, "tracks"), 1);
        assert_eq!(audit_count(&fixture.path, "play_history"), 0);
        assert_eq!(audit_count(&fixture.path, "track_metadata"), 0);
        assert_eq!(audit_count(&fixture.path, "playlists"), 0);
        assert_eq!(audit_count(&fixture.path, "playlist_tracks"), 0);

        clear_audit(&fixture.path);
        entries[1_127].item.play_count = 1;
        entries[1_127].item.last_played_at_ms = Some(42);
        entries[1_127].item.play_history_ms.push(42);
        save_library(&fixture.path, &entries, &playlists).expect("the play is saved");
        assert_eq!(audit_count(&fixture.path, "tracks"), 1);
        assert_eq!(audit_count(&fixture.path, "play_history"), 1);
        assert_eq!(audit_count(&fixture.path, "track_metadata"), 0);
        assert_eq!(audit_count(&fixture.path, "playlists"), 0);
        assert_eq!(audit_count(&fixture.path, "playlist_tracks"), 0);

        clear_audit(&fixture.path);
        playlists[0].track_ids.push(entries[1_127].item.id.clone());
        save_library(&fixture.path, &entries, &playlists).expect("the favorite is saved");
        assert_eq!(audit_count(&fixture.path, "tracks"), 0);
        assert_eq!(audit_count(&fixture.path, "play_history"), 0);
        assert_eq!(audit_count(&fixture.path, "track_metadata"), 0);
        assert_eq!(audit_count(&fixture.path, "playlists"), 0);
        assert_eq!(audit_count(&fixture.path, "playlist_tracks"), 1);

        let (loaded_entries, loaded_playlists) =
            load_library(&fixture.path).expect("the changed library loads");
        assert_eq!(loaded_entries[1_127].item.title, "Corrected title");
        assert_eq!(loaded_entries[1_127].item.play_history_ms, vec![42]);
        assert_eq!(loaded_playlists, playlists);

        clear_audit(&fixture.path);
        save_playback_state(
            &fixture.path,
            &SavedPlaybackState {
                current_item_id: Some(entries[1_127].item.id.clone()),
                position_ms: 84,
                volume_percent: 72,
                shuffle_enabled: false,
                repeat_mode: RepeatMode::Off,
                queue_ids: vec![entries[1_127].item.id.clone()],
                shuffle_order: Vec::new(),
            },
        )
        .expect("the playback position is saved");
        for table_name in [
            "tracks",
            "play_history",
            "track_metadata",
            "playlists",
            "playlist_tracks",
        ] {
            assert_eq!(audit_count(&fixture.path, table_name), 0);
        }
    }

    #[test]
    fn round_trips_provider_metadata_and_override_ownership() {
        let fixture = DatabaseFixture::new("metadata-ownership");
        let mut entries = vec![entry(0)];
        entries[0].provider_metadata = Some(EditableTrackMetadata {
            title: "Provider title".into(),
            artist: "Provider artist".into(),
            album: Some("Provider album".into()),
            label: None,
            genres: vec!["Provider genre".into()],
        });
        entries[0].metadata_overrides = HashSet::from(["title".into(), "genres".into()]);

        save_library(&fixture.path, &entries, &[]).expect("the library is saved");
        let (loaded, _) = load_library(&fixture.path).expect("the library loads");

        assert_eq!(loaded[0].provider_metadata, entries[0].provider_metadata);
        assert_eq!(loaded[0].metadata_overrides, entries[0].metadata_overrides);

        install_write_audit(&fixture.path);
        save_library(&fixture.path, &entries, &[]).expect("the unchanged library is saved");
        assert_eq!(audit_count(&fixture.path, "track_metadata"), 0);

        entries[0].metadata_overrides.insert("artist".into());
        save_library(&fixture.path, &entries, &[]).expect("the new ownership is saved");
        assert_eq!(audit_count(&fixture.path, "track_metadata"), 1);
        assert_eq!(
            load_library(&fixture.path)
                .expect("the updated ownership loads")
                .0[0]
                .metadata_overrides,
            entries[0].metadata_overrides
        );
    }

    #[test]
    fn rolls_back_all_changed_rows_when_a_statement_fails() {
        let fixture = DatabaseFixture::new("rollback");
        let entries = (0..12).map(entry).collect::<Vec<_>>();
        let playlists = vec![Playlist {
            smart: None,
            id: "favorites".into(),
            name: "Favorites".into(),
            track_ids: vec![entries[0].item.id.clone()],
        }];
        save_library(&fixture.path, &entries, &playlists).expect("the initial library is saved");
        Connection::open(&fixture.path)
            .expect("the fixture database opens")
            .execute_batch(
                "CREATE TRIGGER reject_track_update BEFORE UPDATE OF title ON tracks
                 WHEN NEW.id = 'track-0007'
                 BEGIN SELECT RAISE(ABORT, 'injected transaction failure'); END;",
            )
            .expect("the failure trigger is installed");

        let mut changed_entries = entries.clone();
        changed_entries[2].item.title = "This update must roll back".into();
        changed_entries[7].item.title = "This update must fail".into();
        let mut changed_playlists = playlists.clone();
        changed_playlists[0]
            .track_ids
            .push(entries[1].item.id.clone());
        let result = save_library(&fixture.path, &changed_entries, &changed_playlists);

        assert!(
            result
                .expect_err("the injected failure rejects the save")
                .contains("injected transaction failure")
        );
        assert_eq!(
            load_library(&fixture.path).expect("the original library still loads"),
            (entries, playlists)
        );
    }

    #[test]
    fn exports_consistent_owner_only_library_without_session_or_cache_rows() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = DatabaseFixture::new("backup");
        let entries = vec![entry(0), entry(1)];
        let playlists = vec![Playlist {
            smart: None,
            id: "favorites".into(),
            name: "Favorites".into(),
            track_ids: vec![entries[1].item.id.clone()],
        }];
        save_library(&fixture.path, &entries, &playlists).expect("the library is saved");
        save_playback_state(
            &fixture.path,
            &SavedPlaybackState {
                current_item_id: Some(entries[1].item.id.clone()),
                position_ms: 23_000,
                volume_percent: 60,
                shuffle_enabled: false,
                repeat_mode: RepeatMode::Off,
                queue_ids: vec![entries[1].item.id.clone()],
                shuffle_order: vec!["SECRET-SESSION-MARKER-39f3e41a".into()],
            },
        )
        .expect("playback state is saved");
        write_artwork_cache(
            &fixture.path,
            "cover",
            "artwork/SECRET-CACHE-MARKER-39f3e41a.jpg",
            12,
            100,
            1_024,
        )
        .expect("artwork cache metadata is saved");

        let backup_path = export_library_backup(&fixture.path).expect("the backup is exported");

        assert_eq!(
            backup_path.parent(),
            Some(fixture.directory.join("backups").as_path())
        );
        assert_eq!(
            fs::metadata(&backup_path)
                .expect("backup metadata is readable")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(backup_path.parent().expect("the backup has a parent"))
                .expect("backup directory metadata is readable")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            load_library(&backup_path).expect("the backup library loads"),
            (entries, playlists)
        );
        let backup = Connection::open(&backup_path).expect("the backup database opens");
        assert_eq!(
            backup
                .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
                .expect("the backup integrity is checked"),
            "ok"
        );
        assert_eq!(
            backup
                .query_row("SELECT COUNT(*) FROM artwork_cache", [], |row| row
                    .get::<_, u64>(0))
                .expect("the backup cache count is read"),
            0
        );
        assert_eq!(
            backup
                .query_row("SELECT COUNT(*) FROM playback_state", [], |row| row
                    .get::<_, u64>(0))
                .expect("the backup playback-state count is read"),
            0
        );
        let backup_bytes = fs::read(&backup_path).expect("the backup bytes are readable");
        assert!(
            !backup_bytes
                .windows(b"SECRET-CACHE-MARKER-39f3e41a".len())
                .any(|window| window == b"SECRET-CACHE-MARKER-39f3e41a")
        );
        assert!(
            !backup_bytes
                .windows(b"SECRET-SESSION-MARKER-39f3e41a".len())
                .any(|window| window == b"SECRET-SESSION-MARKER-39f3e41a")
        );
        let source = Connection::open(&fixture.path).expect("the source database opens");
        assert_eq!(
            source
                .query_row("SELECT COUNT(*) FROM artwork_cache", [], |row| row
                    .get::<_, u64>(0))
                .expect("the source cache count is read"),
            1
        );
        assert_eq!(
            source
                .query_row("SELECT COUNT(*) FROM playback_state", [], |row| row
                    .get::<_, u64>(0))
                .expect("the source playback-state count is read"),
            1
        );
    }

    #[test]
    fn playback_state_failure_rolls_back_the_track_play_and_history() {
        let fixture = DatabaseFixture::new("atomic-playback");
        let entries = vec![entry(0)];
        let initial_state = SavedPlaybackState {
            current_item_id: None,
            position_ms: 0,
            volume_percent: 72,
            shuffle_enabled: false,
            repeat_mode: RepeatMode::Off,
            queue_ids: vec![entries[0].item.id.clone()],
            shuffle_order: Vec::new(),
        };
        save_library(&fixture.path, &entries, &[]).expect("the initial library is saved");
        save_playback_state(&fixture.path, &initial_state)
            .expect("the initial playback state is saved");
        Connection::open(&fixture.path)
            .expect("the fixture database opens")
            .execute_batch(
                "CREATE TRIGGER reject_playback_state BEFORE UPDATE ON playback_state
                 BEGIN SELECT RAISE(ABORT, 'injected playback-state failure'); END;",
            )
            .expect("the playback-state failure trigger is installed");

        let mut changed_entries = entries.clone();
        changed_entries[0].item.play_count = 1;
        changed_entries[0].item.last_played_at_ms = Some(42);
        changed_entries[0].item.play_history_ms.push(42);
        let changed_state = SavedPlaybackState {
            current_item_id: Some(entries[0].item.id.clone()),
            position_ms: 42,
            volume_percent: 72,
            shuffle_enabled: false,
            repeat_mode: RepeatMode::Off,
            queue_ids: vec![entries[0].item.id.clone()],
            shuffle_order: Vec::new(),
        };

        let result =
            save_library_and_playback(&fixture.path, &changed_entries, &[], &changed_state);

        assert!(
            result
                .expect_err("the injected failure rejects the atomic save")
                .contains("injected playback-state failure")
        );
        assert_eq!(
            load_library(&fixture.path).expect("the original library still loads"),
            (entries, Vec::new())
        );
        let loaded_state = load_playback_state(&fixture.path)
            .expect("the original playback state loads")
            .expect("the original playback state exists");
        assert_eq!(loaded_state.current_item_id, initial_state.current_item_id);
        assert_eq!(loaded_state.position_ms, initial_state.position_ms);
        assert_eq!(loaded_state.queue_ids, initial_state.queue_ids);
    }
}
