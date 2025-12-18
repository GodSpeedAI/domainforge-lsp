# DomainForge MCP Server

The **DomainForge MCP Server** (`domainforge-mcp`) enables AI agents (like Claude or VS Code Copilot) to intelligently interact with the DomainForge language server using the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/).

It acts as a **bridge** between the MCP protocol and the DomainForge LSP server.

## Architecture

```mermaid
graph LR
    Agent[AI Agent client] <-->|MCP (JSON-RPC/Stdio)| MCP[domainforge-mcp]
    MCP <-->|LSP (JSON-RPC/Stdio)| LSP[domainforge-lsp]
```

The MCP server spawns the LSP server as a child process and proxies high-level tool requests (like "get hover info") into low-level LSP protocol requests (like `textDocument/hover`).

## Capabilities

Currently supported tools:

### `domainforge/hover`

Retrieves hover information for a symbol at a specific location.

- **Arguments**:
  - `uri` (string): The file URI (e.g., `file:///path/to/project/main.sea`)
  - `line` (integer): 0-based line number
  - `character` (integer): 0-based character offset
- **Returns**: Markdown content describing the symbol (type, definition, relations).

## Usage

### Building

The MCP server is part of the `domainforge-lsp` crate. Build it with:

```bash
cargo build --bin domainforge-mcp
```

### Running

You normally don't run this manually; it is configured as a server in your AI agent's configuration (e.g., in `claude_desktop_config.json` or similar).

**Command Line Arguments**:

- `--lsp-path <PATH>`: Explicit path to the `domainforge-lsp` binary. If omitted, defaults to looking for `domainforge-lsp` in your `$PATH`.
- `--workspace-root <PATH>`: (Optional) The root directory of the workspace to initialize the LSP with.

**Example Configuration (Claude Desktop)**:

```json
{
  "mcpServers": {
    "domainforge": {
      "command": "/path/to/domainforge-mcp",
      "args": [
        "--lsp-path",
        "/path/to/domainforge-lsp",
        "--workspace-root",
        "/path/to/your/project"
      ]
    }
  }
}
```

## Development

The code is located in `domainforge-lsp/src/mcp/`.

- `main.rs`: Entry point and CLI parsing.
- `transport.rs`: MCP protocol loop (stdio).
- `tools.rs`: Tool implementations.
- `lsp_client.rs`: Async client for managing the child LSP process.
