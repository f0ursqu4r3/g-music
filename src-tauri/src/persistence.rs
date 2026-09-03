use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::playback::{MediaItem, Playlist, QueueEntry, RepeatMode};

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

pub(crate) fn load_library(path: &Path) -> Result<(Vec<QueueEntry>, Vec<Playlist>), String> {
    let connection = open(path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, provider, source_url, title, artist, album, album_artist, track_number,
                    disc_number, release_date, upload_date, description, channel, channel_id,
                    uploader, uploader_id, thumbnail_url, label, genres_json, categories_json,
                    tags_json, language, availability, is_live, view_count, like_count, duration_ms,
                    metadata_dirty, play_count, last_played_at_ms
             FROM tracks ORDER BY library_position",
        )
        .map_err(error)?;
    let mut entries = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let history =
                read_history(&connection, &id).map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(QueueEntry {
                source_url: row.get(2)?,
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
        .prepare("SELECT id, name FROM playlists ORDER BY library_position")
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
            Ok(Playlist {
                id,
                name: row.get(1)?,
                track_ids,
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
    transaction
        .execute("DELETE FROM playlist_tracks", [])
        .map_err(error)?;
    transaction
        .execute("DELETE FROM playlists", [])
        .map_err(error)?;
    transaction
        .execute("DELETE FROM play_history", [])
        .map_err(error)?;
    transaction
        .execute("DELETE FROM tracks", [])
        .map_err(error)?;

    for (position, entry) in entries.iter().enumerate() {
        let item = &entry.item;
        transaction
            .execute(
                "INSERT INTO tracks (
                    id, provider, source_url, title, artist, album, album_artist, track_number,
                    disc_number, release_date, upload_date, description, channel, channel_id,
                    uploader, uploader_id, thumbnail_url, label, genres_json, categories_json,
                    tags_json, language, availability, is_live, view_count, like_count, duration_ms,
                    metadata_dirty, play_count, last_played_at_ms, library_position
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
                 )",
                params![
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
                ],
            )
            .map_err(error)?;
        for (history_position, played_at_ms) in item.play_history_ms.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO play_history (track_id, position, played_at_ms) VALUES (?1, ?2, ?3)",
                    params![item.id, history_position as i64, played_at_ms],
                )
                .map_err(error)?;
        }
    }

    for (position, playlist) in playlists.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO playlists (id, name, library_position) VALUES (?1, ?2, ?3)",
                params![playlist.id, playlist.name, position as i64],
            )
            .map_err(error)?;
        for (track_position, track_id) in playlist.track_ids.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                    params![playlist.id, track_id, track_position as i64],
                )
                .map_err(error)?;
        }
    }

    transaction.commit().map_err(error)
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
