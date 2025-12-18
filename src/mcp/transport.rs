use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::tools;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "method")]
enum JsonRpcRequest {
    #[serde(rename = "initialize")]
    Initialize { id: Value, params: Value },
    #[serde(rename = "notifications/initialized")]
    Initialized,
    #[serde(rename = "tools/list")]
    ToolsList { id: Value },
    #[serde(rename = "tools/call")]
    ToolsCall { id: Value, params: ToolCallParams },
}

#[derive(Serialize, Deserialize, Debug)]
struct ToolCallParams {
    name: String,
    arguments: Value,
}

use crate::guardrails::Guard;
use std::sync::Arc;

pub async fn run_stdio_loop(
    client: &crate::lsp_client::LspClient,
    guard: Arc<Guard>,
) -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        log::debug!("Received MCP message: {}", line);

        let req: Result<JsonRpcRequest, serde_json::Error> = serde_json::from_str(&line);

        match req {
            Ok(JsonRpcRequest::Initialize { id, .. }) => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "serverInfo": {
                            "name": "domainforge-mcp",
                            "version": env!("CARGO_PKG_VERSION")
                        },
                        "capabilities": {
                            "tools": {}
                        }
                    }
                });
                let mut out = serde_json::to_vec(&resp)?;
                out.push(b'\n');
                stdout.write_all(&out).await?;
                stdout.flush().await?;
            }
            Ok(JsonRpcRequest::Initialized) => {
                log::info!("MCP Client initialized");
            }
            Ok(JsonRpcRequest::ToolsList { id }) => {
                let tools = tools::list_tools();
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tools
                    }
                });
                let mut out = serde_json::to_vec(&resp)?;
                out.push(b'\n');
                stdout.write_all(&out).await?;
                stdout.flush().await?;
            }
            Ok(JsonRpcRequest::ToolsCall { id, params }) => {
                log::info!("Calling tool: {}", params.name);
                match tools::handle_tool_call(&params.name, params.arguments, client, &guard).await
                {
                    Ok(result) => {
                        let resp = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        });
                        let mut out = serde_json::to_vec(&resp)?;
                        out.push(b'\n');
                        stdout.write_all(&out).await?;
                        stdout.flush().await?;
                    }
                    Err(e) => {
                        let resp = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32603,
                                "message": e.to_string()
                            }
                        });
                        let mut out = serde_json::to_vec(&resp)?;
                        out.push(b'\n');
                        stdout.write_all(&out).await?;
                        stdout.flush().await?;
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to parse message: {}", e);
            }
        }
    }
    Ok(())
}
