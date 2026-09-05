use std::{
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    thread,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    commands::{AppState, CommandError},
    playback::{EditableTrackMetadata, Playlist},
};

const SOCKET_FILE: &str = "agent.sock";

#[derive(Debug, Deserialize)]
struct AgentRequest {
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct AgentResponse {
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<CommandError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackUpdateRequest {
    id: String,
    metadata: EditableTrackMetadata,
}

#[derive(Deserialize)]
struct TrackMetadataBatchRequest {
    updates: Vec<TrackUpdateRequest>,
}

#[derive(Deserialize)]
struct TrackRemovalRequest {
    ids: Vec<String>,
}

#[derive(Deserialize)]
struct PlaylistUpsertRequest {
    playlist: Playlist,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistReorderRequest {
    playlist_ids: Vec<String>,
}

#[derive(Deserialize)]
struct PlaylistDeleteRequest {
    id: String,
}

#[derive(Deserialize)]
struct QueueMoveRequest {
    from: usize,
    to: usize,
}

pub fn start(app: AppHandle) -> tauri::Result<()> {
    let socket_path = socket_path(&app)?;
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
    tracing::info!(path = %socket_path.display(), "local agent control socket started");

    thread::Builder::new()
        .name("gmusic-agent-control".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => handle_connection(&app, stream),
                    Err(error) => tracing::error!(%error, "agent control socket failed"),
                }
            }
        })?;
    Ok(())
}

pub fn shutdown(app: &AppHandle) {
    match socket_path(app) {
        Ok(path) if path.exists() => {
            if let Err(error) = fs::remove_file(path) {
                tracing::debug!(%error, "could not remove agent control socket");
            }
        }
        Ok(_) => {}
        Err(error) => tracing::debug!(%error, "could not find agent control socket"),
    }
}

pub fn socket_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(SOCKET_FILE))
}

fn handle_connection(app: &AppHandle, stream: UnixStream) {
    let Ok(writer_stream) = stream.try_clone() else {
        tracing::debug!("could not clone agent control socket stream");
        return;
    };
    let reader = BufReader::new(stream);
    let mut writer = BufWriter::new(writer_stream);
    for line in reader.lines() {
        let response = match line {
            Ok(line) => handle_request_line(app, &line),
            Err(error) => {
                tracing::debug!(%error, "could not read agent control request");
                return;
            }
        };
        let Ok(response) = serde_json::to_string(&response) else {
            tracing::error!("could not serialize agent control response");
            return;
        };
        if writer.write_all(response.as_bytes()).is_err()
            || writer.write_all(b"\n").is_err()
            || writer.flush().is_err()
        {
            return;
        }
    }
}

fn handle_request_line(app: &AppHandle, line: &str) -> AgentResponse {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(error) => {
            return AgentResponse {
                id: Value::Null,
                result: None,
                error: Some(error),
            };
        }
    };
    match dispatch(app, &request) {
        Ok(result) => AgentResponse {
            id: request.id,
            result: Some(result),
            error: None,
        },
        Err(error) => AgentResponse {
            id: request.id,
            result: None,
            error: Some(error),
        },
    }
}

fn parse_request(line: &str) -> Result<AgentRequest, CommandError> {
    serde_json::from_str(line).map_err(|error| protocol_error(format!("invalid request: {error}")))
}

fn dispatch(app: &AppHandle, request: &AgentRequest) -> Result<Value, CommandError> {
    let state = app.state::<AppState>();
    let method = request.method.as_str();
    let result = (|| match method {
        "library.inspect" => serialize(state.library_snapshot()?),
        "track.update" => {
            let request: TrackUpdateRequest = decode_params(&request.params)?;
            serialize(state.update_track_metadata(&request.id, request.metadata)?)
        }
        "tracks.update" => {
            let request: TrackMetadataBatchRequest = decode_params(&request.params)?;
            let updates = request
                .updates
                .into_iter()
                .map(|update| (update.id, update.metadata))
                .collect();
            serialize(state.update_tracks_metadata(updates)?)
        }
        "track.remove" => {
            let request: TrackRemovalRequest = decode_params(&request.params)?;
            serialize(state.remove_tracks(&request.ids)?)
        }
        "playlist.upsert" => {
            let request: PlaylistUpsertRequest = decode_params(&request.params)?;
            serialize(state.upsert_playlist(request.playlist)?)
        }
        "playlist.reorder" => {
            let request: PlaylistReorderRequest = decode_params(&request.params)?;
            serialize(state.reorder_playlists(&request.playlist_ids)?)
        }
        "playlist.delete" => {
            let request: PlaylistDeleteRequest = decode_params(&request.params)?;
            serialize(state.delete_playlist(&request.id)?)
        }
        "queue.move" => {
            let request: QueueMoveRequest = decode_params(&request.params)?;
            serialize(state.move_queue_item(request.from, request.to)?)
        }
        "library.move" => {
            let request: QueueMoveRequest = decode_params(&request.params)?;
            serialize(state.move_library_item(request.from, request.to)?)
        }
        _ => Err(protocol_error(format!("unknown method {}", request.method))),
    })();
    if library_mutation_method(method) {
        state.emit_cached_playback(app);
    }
    let result = result?;
    if library_mutation_method(method)
        && let Err(error) = app.emit("library-updated", ())
    {
        tracing::debug!(%error, method, "could not deliver library update event");
    }
    Ok(result)
}

fn library_mutation_method(method: &str) -> bool {
    matches!(
        method,
        "track.update"
            | "tracks.update"
            | "track.remove"
            | "playlist.upsert"
            | "playlist.reorder"
            | "playlist.delete"
            | "queue.move"
            | "library.move"
    )
}

fn decode_params<T: DeserializeOwned>(params: &Value) -> Result<T, CommandError> {
    serde_json::from_value(params.clone())
        .map_err(|error| protocol_error(format!("invalid request parameters: {error}")))
}

fn serialize<T: Serialize>(value: T) -> Result<Value, CommandError> {
    serde_json::to_value(value)
        .map_err(|error| protocol_error(format!("could not serialize result: {error}")))
}

fn protocol_error(message: String) -> CommandError {
    CommandError {
        code: "agent_protocol_failed",
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::{library_mutation_method, parse_request};

    #[test]
    fn malformed_requests_return_a_structured_protocol_error() {
        let error = parse_request("not json").expect_err("invalid JSON is rejected");

        assert_eq!(error.code, "agent_protocol_failed");
    }

    #[test]
    fn library_mutations_emit_refresh_events() {
        for method in [
            "track.update",
            "tracks.update",
            "track.remove",
            "playlist.upsert",
            "playlist.reorder",
            "playlist.delete",
            "queue.move",
            "library.move",
        ] {
            assert!(
                library_mutation_method(method),
                "{method} must refresh windows"
            );
        }
        assert!(!library_mutation_method("library.inspect"));
    }
}
