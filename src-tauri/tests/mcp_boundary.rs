use open_editor_lib::{command_service::CommandService, project};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

#[test]
fn mcp_sidecar_reads_an_authorized_project_through_the_socket() {
    let root = tempfile::tempdir().unwrap();
    let project = project::ProjectDocument::new("MCP boundary".into());
    project::create(root.path(), &project).unwrap();
    let service = CommandService::default();
    let info = service
        .authorize(project.id, root.path().to_path_buf())
        .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_open-editor-mcp"))
        .args([
            "--socket",
            &info.socket_path,
            "--token",
            &info.capability_token,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut send = |request: Value| {
        serde_json::to_writer(&mut input, &request).unwrap();
        input.write_all(b"\n").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };

    let initialized = send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18" }
    }));
    assert_eq!(initialized["result"]["serverInfo"]["name"], "open-editor");
    let snapshot = send(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "editor_get_project", "arguments": { "projectId": project.id } }
    }));
    assert_eq!(snapshot["result"]["isError"], false);
    assert_eq!(
        snapshot["result"]["structuredContent"]["id"],
        project.id.to_string()
    );
    drop(input);
    child.wait().unwrap();
}
