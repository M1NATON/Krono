use serde_json::{json, Value};

use crate::tools::VaultSandbox;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

fn jsonrpc_ok(id: Value, result: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

fn jsonrpc_err(id: Value, code: i64, message: impl Into<String>) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
    .to_string()
}

/// Processes one JSON-RPC 2.0 line and returns an optional serialized response.
/// Notifications (messages without an `id`) return `None`.
pub fn handle_json_line(line: &str, sandbox: &VaultSandbox) -> Option<String> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(jsonrpc_err(Value::Null, -32700, format!("Parse error: {e}"))),
    };

    let obj = match msg.as_object() {
        Some(o) => o,
        None => return Some(jsonrpc_err(Value::Null, -32600, "Invalid Request")),
    };

    let id = obj.get("id").cloned();
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return id.map(|req_id| jsonrpc_err(req_id, -32600, "Invalid jsonrpc version"));
    }

    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return id.map(|req_id| jsonrpc_err(req_id, -32600, "Missing method"));
    };

    // Notifications do not carry an `id` and must not emit a response.
    let Some(req_id) = id else {
        return None;
    };

    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "initialize" => Some(jsonrpc_ok(
            req_id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "krono-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )),
        "ping" => Some(jsonrpc_ok(req_id, json!({}))),
        "tools/list" => Some(jsonrpc_ok(req_id, json!({ "tools": build_tool_list(sandbox) }))),
        "tools/call" => Some(handle_tools_call(req_id, &params, sandbox)),
        _ => Some(jsonrpc_err(
            req_id,
            -32601,
            format!("Method not found: {method}"),
        )),
    }
}

fn build_tool_list(sandbox: &VaultSandbox) -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "list_files",
            "description": "List allowed Markdown (.md) notes inside configured --allow-dir folders. Encrypted (.md.enc) and hidden (.krono/.git) files are never returned.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Optional relative subfolder inside an allowed directory."
                    }
                },
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "search",
            "description": "Search allowed Markdown notes using Krono query syntax (tag:, status:, path:, and text terms).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (supports tag:, status:, path:, and keywords)."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 50,
                        "description": "Maximum number of hits (default 15, max 50)."
                    },
                    "folder": {
                        "type": "string",
                        "description": "Optional relative subfolder filter inside allowed directories."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "openWorldHint": false
            }
        }),
        json!({
            "name": "read",
            "description": "Read a Markdown (.md) note from an allowed directory with line/byte caps.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Vault-relative path to the .md file."
                    },
                    "max_lines": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 2000,
                        "description": "Maximum number of lines to return (default 400)."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "openWorldHint": false
            }
        }),
    ];

    if sandbox.policy().allow_write() {
        tools.push(json!({
            "name": "append",
            "description": "Safely append Markdown content to the end of an allowed .md note or under a specific heading (## ...).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Vault-relative path to the .md file inside an allowed directory."
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown text to append."
                    },
                    "heading": {
                        "type": "string",
                        "description": "Optional Markdown heading (e.g. '## Log' or 'Log') under which to append."
                    }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            }
        }));
    }

    tools
}

fn handle_tools_call(req_id: Value, params: &Value, sandbox: &VaultSandbox) -> String {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return jsonrpc_err(req_id, -32602, "Missing params.name");
    };
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let result = match name {
        "list_files" => {
            let folder = args.get("folder").and_then(Value::as_str);
            sandbox.list_files(folder)
        }
        "search" => {
            let Some(query) = args.get("query").and_then(Value::as_str) else {
                return jsonrpc_err(req_id, -32602, "Missing required string argument: query");
            };
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|n| n as usize);
            let folder = args.get("folder").and_then(Value::as_str);
            sandbox.search(query, limit, folder)
        }
        "read" => {
            let Some(path) = args.get("path").and_then(Value::as_str) else {
                return jsonrpc_err(req_id, -32602, "Missing required string argument: path");
            };
            let max_lines = args
                .get("max_lines")
                .and_then(Value::as_u64)
                .map(|n| n as usize);
            sandbox.read(path, max_lines)
        }
        "append" => {
            if !sandbox.policy().allow_write() {
                return jsonrpc_err(
                    req_id,
                    -32601,
                    "Tool 'append' is disabled (server started without --allow-write)",
                );
            }
            let Some(path) = args.get("path").and_then(Value::as_str) else {
                return jsonrpc_err(req_id, -32602, "Missing required string argument: path");
            };
            let Some(content) = args.get("content").and_then(Value::as_str) else {
                return jsonrpc_err(req_id, -32602, "Missing required string argument: content");
            };
            let heading = args.get("heading").and_then(Value::as_str);
            sandbox.append(path, content, heading)
        }
        _ => {
            return jsonrpc_err(req_id, -32601, format!("Unknown tool: {name}"));
        }
    };

    match result {
        Ok(val) => {
            let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
            jsonrpc_ok(
                req_id,
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }),
            )
        }
        Err(err) => jsonrpc_ok(
            req_id,
            json!({
                "content": [{ "type": "text", "text": err }],
                "isError": true
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krono_core::mcp_policy::McpPolicy;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static PROTO_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_vault() -> PathBuf {
        let id = PROTO_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "krono_mcp_proto_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("notes")).unwrap();
        fs::write(dir.join("notes/hello.md"), "# Hello\nWorld\n").unwrap();
        dir
    }

    #[test]
    fn lifecycle_and_readonly_vs_write_tools_list() {
        let vault = temp_vault();
        let ro_sb = VaultSandbox::new(
            vault.clone(),
            McpPolicy::new(&["notes".to_string()], false, false).unwrap(),
        )
        .unwrap();

        let init_resp: Value = serde_json::from_str(
            &handle_json_line(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                &ro_sb,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "krono-mcp");

        assert!(handle_json_line(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &ro_sb
        )
        .is_none());

        let list_ro: Value = serde_json::from_str(
            &handle_json_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, &ro_sb).unwrap(),
        )
        .unwrap();
        let ro_names: Vec<&str> = list_ro["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert_eq!(ro_names, vec!["list_files", "search", "read"]);

        let append_denied: Value = serde_json::from_str(
            &handle_json_line(
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"append","arguments":{"path":"notes/hello.md","content":"x"}}}"#,
                &ro_sb,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(append_denied["error"]["code"], -32601);

        let rw_sb = VaultSandbox::new(
            vault.clone(),
            McpPolicy::new(&["notes".to_string()], false, true).unwrap(),
        )
        .unwrap();
        let list_rw: Value = serde_json::from_str(
            &handle_json_line(r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#, &rw_sb).unwrap(),
        )
        .unwrap();
        let tools_rw = list_rw["result"]["tools"].as_array().unwrap();
        assert_eq!(tools_rw.len(), 4);
        assert_eq!(tools_rw[3]["name"], "append");
        assert_eq!(tools_rw[3]["annotations"]["readOnlyHint"], false);

        let bad_json: Value =
            serde_json::from_str(&handle_json_line("{not-json", &ro_sb).unwrap()).unwrap();
        assert_eq!(bad_json["error"]["code"], -32700);

        let _ = fs::remove_dir_all(&vault);
    }
}
