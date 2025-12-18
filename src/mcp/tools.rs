use crate::lsp_client::LspClient;

use serde_json::{json, Value};

use crate::guardrails::Guard;

pub async fn handle_tool_call(
    name: &str,
    args: Value,
    client: &LspClient,
    guard: &Guard,
) -> anyhow::Result<Value> {
    // 1. Rate Check
    guard.check_rate_limit(name)?;

    // 2. Dispatch
    match name {
        "domainforge/hover" => hover_tool(args, client, guard).await,
        "domainforge/definition" => definition_tool(args, client, guard).await,
        "domainforge/references" => references_tool(args, client, guard).await,
        "domainforge/diagnostics" => diagnostics_tool(args, client, guard).await,
        "domainforge/rename-preview" => rename_preview_tool(args, client, guard).await,
        "domainforge/code-actions" => code_action_tool(args, client, guard).await,
        _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
    }
}

async fn hover_tool(args: Value, client: &LspClient, guard: &Guard) -> anyhow::Result<Value> {
    let uri = extract_uri(&args, guard)?;
    let (line, char) = extract_pos(&args)?;
    client.hover(&uri, line, char).await
}

async fn definition_tool(args: Value, client: &LspClient, guard: &Guard) -> anyhow::Result<Value> {
    let uri = extract_uri(&args, guard)?;
    let (line, char) = extract_pos(&args)?;
    client.definition(&uri, line, char).await
}

async fn references_tool(args: Value, client: &LspClient, guard: &Guard) -> anyhow::Result<Value> {
    let uri = extract_uri(&args, guard)?;
    let (line, char) = extract_pos(&args)?;
    let include_decl = args
        .get("includeDeclaration")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    client.references(&uri, line, char, include_decl).await
}

async fn diagnostics_tool(args: Value, client: &LspClient, guard: &Guard) -> anyhow::Result<Value> {
    let uri = extract_uri(&args, guard)?;
    let cache = client.diagnostics_cache.read().await;
    let diags = cache.get(&uri).cloned().unwrap_or_else(|| vec![]);
    Ok(json!(diags))
}

async fn rename_preview_tool(
    args: Value,
    client: &LspClient,
    guard: &Guard,
) -> anyhow::Result<Value> {
    let uri = extract_uri(&args, guard)?;
    let (line, char) = extract_pos(&args)?;
    let new_name = args
        .get("newName")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing newName"))?;

    // Call rename but wrap it to indicate it's a preview?
    // The LSP rename returns a WorkspaceEdit. We just return that.
    let edit = client.rename(&uri, line, char, new_name).await?;

    // Wrap the edit to indicate it requires human approval
    Ok(json!({
        "requiresHumanApproval": true,
        "edit": edit
    }))
}

async fn code_action_tool(args: Value, client: &LspClient, guard: &Guard) -> anyhow::Result<Value> {
    let uri = extract_uri(&args, guard)?;
    let range = args
        .get("range")
        .ok_or(anyhow::anyhow!("Missing range"))?
        .clone();
    client.code_action(&uri, range).await
}

// Helpers
fn extract_uri(args: &Value, guard: &Guard) -> anyhow::Result<String> {
    let uri = args
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing uri"))?;
    let path_str = uri.strip_prefix("file://").unwrap_or(uri);
    guard.check_path(path_str)?;
    Ok(uri.to_string())
}

fn extract_pos(args: &Value) -> anyhow::Result<(u64, u64)> {
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or(anyhow::anyhow!("Missing line"))?;
    let char = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or(anyhow::anyhow!("Missing character"))?;
    Ok((line, char))
}

pub fn list_tools() -> Value {
    json!([
        {
            "name": "domainforge/hover",
            "description": "Get hover information for a symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uri": { "type": "string" },
                    "line": { "type": "integer" },
                    "character": { "type": "integer" }
                },
                "required": ["uri", "line", "character"]
            }
        },
        {
            "name": "domainforge/definition",
            "description": "Get definition location for a symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uri": { "type": "string" },
                    "line": { "type": "integer" },
                    "character": { "type": "integer" }
                },
                "required": ["uri", "line", "character"]
            }
        },
        {
            "name": "domainforge/references",
            "description": "Get references for a symbol",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uri": { "type": "string" },
                    "line": { "type": "integer" },
                    "character": { "type": "integer" },
                    "includeDeclaration": { "type": "boolean" }
                },
                "required": ["uri", "line", "character"]
            }
        },
        {
            "name": "domainforge/diagnostics",
            "description": "Get cached diagnostics for a file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uri": { "type": "string" }
                },
                "required": ["uri"]
            }
        },
        {
            "name": "domainforge/rename-preview",
            "description": "Preview a rename operation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uri": { "type": "string" },
                    "line": { "type": "integer" },
                    "character": { "type": "integer" },
                    "newName": { "type": "string" }
                },
                "required": ["uri", "line", "character", "newName"]
            }
        },
        {
            "name": "domainforge/code-actions",
            "description": "Get available code actions",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "uri": { "type": "string" },
                    "range": { "type": "object" }
                },
                "required": ["uri", "range"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_tools_returns_schema() {
        let tools = list_tools();
        let arr = tools.as_array().expect("Tools should be an array");
        assert!(arr.len() >= 6);

        let tool_names: Vec<&str> = arr
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();

        assert!(tool_names.contains(&"domainforge/hover"));
        assert!(tool_names.contains(&"domainforge/definition"));
        assert!(tool_names.contains(&"domainforge/references"));
        assert!(tool_names.contains(&"domainforge/diagnostics"));
        assert!(tool_names.contains(&"domainforge/rename-preview"));
        assert!(tool_names.contains(&"domainforge/code-actions"));
    }
}
