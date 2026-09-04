use serde_json::{json, Value};
use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

fn arg(name: &str) -> Result<String, String> {
    let mut args = env::args();
    while let Some(value) = args.next() {
        if value == name {
            return args
                .next()
                .ok_or_else(|| format!("{name} requires a value"));
        }
    }
    Err(format!("missing {name}"))
}

fn call_service(socket: &str, request: Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(socket).map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut stream, &request).map_err(|error| error.to_string())?;
    stream.write_all(b"\n").map_err(|error| error.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    let response: Value = serde_json::from_str(&line).map_err(|error| error.to_string())?;
    if response["ok"] == true {
        Ok(response["result"].clone())
    } else {
        Err(response["error"]
            .as_str()
            .unwrap_or("command service failed")
            .into())
    }
}

fn tool_definitions() -> Value {
    let payload_schema = command_payload_schema();
    json!([
        {
            "name": "editor_get_project",
            "description": "Read the current immutable Open Editor project snapshot, including revision, approved media, sequences, tracks, clips, captions, transitions, and analysis artifacts.",
            "inputSchema": {
                "type": "object", "additionalProperties": false, "required": ["projectId"],
                "properties": { "projectId": { "type": "string", "format": "uuid" } }
            }
        },
        {
            "name": "editor_apply_command",
            "description": "Apply one validated Open Editor command. Use the latest project revision. Commands cannot alter original media, and provider commands cannot add or remove approved media.",
            "inputSchema": {
                "type": "object", "additionalProperties": false, "required": ["envelope"],
                "properties": {
                    "envelope": {
                        "type": "object", "additionalProperties": false,
                        "required": ["commandId", "projectId", "source", "batchId", "expectedProjectRevision", "payload"],
                        "properties": {
                            "commandId": { "type": "string", "format": "uuid" },
                            "projectId": { "type": "string", "format": "uuid" },
                            "source": { "const": "codex" },
                            "conversationId": { "type": "string", "format": "uuid" },
                            "batchId": { "type": "string", "format": "uuid" },
                            "expectedProjectRevision": { "type": "integer", "minimum": 0 },
                            "payload": payload_schema.clone()
                        }
                    }
                }
            }
        },
        {
            "name": "editor_apply_batch",
            "description": "Atomically apply a complete edit batch. Every envelope must share the same project, source, conversation, and batch IDs and use sequential expected revisions. If any command fails, none are saved.",
            "inputSchema": {
                "type": "object", "additionalProperties": false, "required": ["envelopes"],
                "properties": {
                    "envelopes": {
                        "type": "array", "minItems": 1, "maxItems": 100,
                        "items": {
                            "type": "object", "additionalProperties": false,
                            "required": ["commandId", "projectId", "source", "batchId", "expectedProjectRevision", "payload"],
                            "properties": {
                                "commandId": { "type": "string", "format": "uuid" },
                                "projectId": { "type": "string", "format": "uuid" },
                                "source": { "const": "codex" },
                                "conversationId": { "type": "string", "format": "uuid" },
                                "batchId": { "type": "string", "format": "uuid" },
                                "expectedProjectRevision": { "type": "integer", "minimum": 0 },
                                "payload": payload_schema
                            }
                        }
                    }
                }
            }
        },
        {
            "name": "editor_start_media_job",
            "description": "Start cancellable local media analysis, proxy generation, or offline transcription for media already approved in this project. Returns a job record to poll.",
            "inputSchema": {
                "type": "object", "additionalProperties": false,
                "required": ["projectId", "assetId", "kind"],
                "properties": {
                    "projectId": { "type": "string", "format": "uuid" },
                    "assetId": { "type": "string", "format": "uuid" },
                    "kind": { "type": "string", "enum": ["analysis", "proxy", "transcription"] }
                }
            }
        },
        {
            "name": "editor_get_job",
            "description": "Read the current state and result of a local Open Editor media job.",
            "inputSchema": {
                "type": "object", "additionalProperties": false,
                "required": ["projectId", "jobId"],
                "properties": {
                    "projectId": { "type": "string", "format": "uuid" },
                    "jobId": { "type": "string", "format": "uuid" }
                }
            }
        },
        {
            "name": "editor_cancel_job",
            "description": "Cancel an active local Open Editor media job.",
            "inputSchema": {
                "type": "object", "additionalProperties": false,
                "required": ["projectId", "jobId"],
                "properties": {
                    "projectId": { "type": "string", "format": "uuid" },
                    "jobId": { "type": "string", "format": "uuid" }
                }
            }
        }
    ])
}

fn command_payload_schema() -> Value {
    let time = || {
        json!({
            "type": "object", "additionalProperties": false,
            "required": ["value", "timescale"],
            "properties": {
                "value": { "type": "integer" },
                "timescale": { "type": "integer", "minimum": 1 }
            }
        })
    };
    let id = || json!({ "type": "string", "format": "uuid" });
    let variant = |kind: &str, fields: Value, required: &[&str]| {
        let mut properties = fields.as_object().cloned().unwrap_or_default();
        properties.insert("type".into(), json!({ "const": kind }));
        let mut required = required
            .iter()
            .map(|value| json!(value))
            .collect::<Vec<_>>();
        required.insert(0, json!("type"));
        json!({
            "type": "object", "additionalProperties": false,
            "required": required, "properties": properties
        })
    };
    let transform = || {
        json!({
            "type": "object", "additionalProperties": false,
            "required": ["x", "y", "scale", "rotation", "opacity"],
            "properties": {
                "x": { "type": "number" }, "y": { "type": "number" },
                "scale": { "type": "number", "exclusiveMinimum": 0 },
                "rotation": { "type": "number" },
                "opacity": { "type": "number", "minimum": 0, "maximum": 1 }
            }
        })
    };
    let caption_style = || {
        json!({
            "type": "object", "additionalProperties": false,
            "required": ["fontSize", "color", "background", "position"],
            "properties": {
                "fontSize": { "type": "number", "exclusiveMinimum": 0 },
                "color": { "type": "string" }, "background": { "type": "string" },
                "position": { "type": "string", "enum": ["top", "center", "bottom"] }
            }
        })
    };
    json!({
        "description": "One validated, undoable Open Editor timeline command.",
        "oneOf": [
            variant("duplicateSequence", json!({ "sequenceId": id(), "name": { "type": "string", "minLength": 1 } }), &["sequenceId", "name"]),
            variant("setActiveSequence", json!({ "sequenceId": id() }), &["sequenceId"]),
            variant("renameSequence", json!({ "sequenceId": id(), "name": { "type": "string", "minLength": 1 } }), &["sequenceId", "name"]),
            variant("removeSequence", json!({ "sequenceId": id() }), &["sequenceId"]),
            variant("setTrackLocked", json!({ "trackId": id(), "locked": { "type": "boolean" } }), &["trackId", "locked"]),
            variant("setTrackMuted", json!({ "trackId": id(), "muted": { "type": "boolean" } }), &["trackId", "muted"]),
            variant("addClip", json!({ "trackId": id(), "assetId": id(), "timelineStart": time() }), &["trackId", "assetId", "timelineStart"]),
            variant("removeClip", json!({ "trackId": id(), "clipId": id() }), &["trackId", "clipId"]),
            variant("moveClip", json!({ "trackId": id(), "clipId": id(), "timelineStart": time() }), &["trackId", "clipId", "timelineStart"]),
            variant("trimClip", json!({ "trackId": id(), "clipId": id(), "sourceIn": time(), "sourceOut": time(), "timelineStart": { "anyOf": [time(), { "type": "null" }] } }), &["trackId", "clipId", "sourceIn", "sourceOut"]),
            variant("splitClip", json!({ "trackId": id(), "clipId": id(), "at": time() }), &["trackId", "clipId", "at"]),
            variant("duplicateClip", json!({ "trackId": id(), "clipId": id(), "timelineStart": time() }), &["trackId", "clipId", "timelineStart"]),
            variant("replaceClip", json!({ "trackId": id(), "clipId": id(), "assetId": id() }), &["trackId", "clipId", "assetId"]),
            variant("changeSpeed", json!({ "trackId": id(), "clipId": id(), "playbackRate": { "type": "number", "exclusiveMinimum": 0 } }), &["trackId", "clipId", "playbackRate"]),
            variant("cropClip", json!({ "trackId": id(), "clipId": id(), "transform": transform() }), &["trackId", "clipId", "transform"]),
            variant("setOpacity", json!({ "trackId": id(), "clipId": id(), "opacity": { "type": "number", "minimum": 0, "maximum": 1 } }), &["trackId", "clipId", "opacity"]),
            variant("setVolume", json!({ "trackId": id(), "clipId": id(), "volume": { "type": "number", "minimum": 0, "maximum": 4 } }), &["trackId", "clipId", "volume"]),
            variant("fadeAudio", json!({ "trackId": id(), "clipId": id(), "fadeIn": time(), "fadeOut": time() }), &["trackId", "clipId", "fadeIn", "fadeOut"]),
            variant("duckAudio", json!({ "trackId": id(), "clipId": id(), "enabled": { "type": "boolean" } }), &["trackId", "clipId", "enabled"]),
            variant("addCaption", json!({ "trackId": id(), "start": time(), "end": time(), "text": { "type": "string", "minLength": 1 } }), &["trackId", "start", "end", "text"]),
            variant("editCaption", json!({ "captionId": id(), "text": { "type": "string", "minLength": 1 } }), &["captionId", "text"]),
            variant("styleCaption", json!({ "captionId": id(), "style": caption_style() }), &["captionId", "style"]),
            variant("removeCaption", json!({ "captionId": id() }), &["captionId"]),
            variant("addTransition", json!({ "fromClipId": id(), "toClipId": id(), "kind": { "type": "string", "enum": ["cut", "fade", "crossDissolve"] }, "duration": time() }), &["fromClipId", "toClipId", "kind", "duration"]),
            variant("removeTransition", json!({ "transitionId": id() }), &["transitionId"])
        ]
    })
}

fn result_content(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into()) }],
        "structuredContent": value,
        "isError": false
    })
}

fn tool_error(message: String) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn handle(request: &Value, socket: &str, token: &str) -> Option<Value> {
    let id = request.get("id")?.clone();
    let method = request["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => json!({
            "protocolVersion": request["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "open-editor", "version": env!("CARGO_PKG_VERSION") }
        }),
        "tools/list" => json!({ "tools": tool_definitions() }),
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or_default();
            let arguments = request["params"]["arguments"].clone();
            let service_request = match name {
                "editor_get_project" => json!({
                    "token": token, "action": "snapshot", "projectId": arguments["projectId"]
                }),
                "editor_apply_command" => json!({
                    "token": token, "action": "command", "envelope": arguments["envelope"]
                }),
                "editor_apply_batch" => json!({
                    "token": token, "action": "batch", "envelopes": arguments["envelopes"]
                }),
                "editor_start_media_job" => json!({
                    "token": token, "action": "startJob", "projectId": arguments["projectId"],
                    "assetId": arguments["assetId"], "kind": arguments["kind"]
                }),
                "editor_get_job" => json!({
                    "token": token, "action": "jobStatus", "projectId": arguments["projectId"],
                    "jobId": arguments["jobId"]
                }),
                "editor_cancel_job" => json!({
                    "token": token, "action": "cancelJob", "projectId": arguments["projectId"],
                    "jobId": arguments["jobId"]
                }),
                _ => {
                    return Some(
                        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "unknown tool" } }),
                    )
                }
            };
            match call_service(socket, service_request) {
                Ok(value) => result_content(value),
                Err(error) => tool_error(error),
            }
        }
        "ping" => json!({}),
        _ => {
            return Some(
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "method not found" } }),
            )
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn main() {
    let socket = match arg("--socket") {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let token = match arg("--token") {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(response) = handle(&request, &socket, &token) {
            if serde_json::to_writer(&mut stdout, &response).is_ok() {
                let _ = stdout.write_all(b"\n");
                let _ = stdout.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_only_snapshot_and_validated_command_tools() {
        let tools = tool_definitions();
        assert_eq!(tools.as_array().unwrap().len(), 6);
        assert_eq!(
            tools[1]["inputSchema"]["properties"]["envelope"]["properties"]["source"]["const"],
            "codex"
        );
        assert_eq!(
            tools[1]["inputSchema"]["properties"]["envelope"]["properties"]["payload"]["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            25
        );
    }

    #[test]
    fn initialize_identifies_open_editor() {
        let response = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }), "unused", "unused").unwrap();
        assert_eq!(response["result"]["serverInfo"]["name"], "open-editor");
    }
}
