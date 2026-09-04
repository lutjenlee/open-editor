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
                            "payload": {
                                "type": "object",
                                "description": "Tagged EditorCommand. Supported types: addClip, removeClip, moveClip, trimClip, splitClip, duplicateClip, replaceClip, changeSpeed, cropClip, setOpacity, setVolume, fadeAudio, duckAudio, addCaption, editCaption, styleCaption, removeCaption, addTransition, removeTransition.",
                                "required": ["type"]
                            }
                        }
                    }
                }
            }
        }
    ])
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
        assert_eq!(tools.as_array().unwrap().len(), 2);
        assert_eq!(
            tools[1]["inputSchema"]["properties"]["envelope"]["properties"]["source"]["const"],
            "codex"
        );
    }

    #[test]
    fn initialize_identifies_open_editor() {
        let response = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }), "unused", "unused").unwrap();
        assert_eq!(response["result"]["serverInfo"]["name"], "open-editor");
    }
}
