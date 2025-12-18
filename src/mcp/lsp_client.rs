use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

pub struct LspClient {
    child: Child,
    request_id: AtomicI64,
    sender: mpsc::Sender<Value>,
    pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<anyhow::Result<Value>>>>>,
    pub diagnostics_cache: Arc<RwLock<HashMap<String, Vec<Value>>>>, // URI -> Diagnostics list
}

impl LspClient {
    pub async fn new(lsp_path: &str) -> anyhow::Result<Self> {
        let mut child = Command::new(lsp_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or(anyhow::anyhow!("Failed to open stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(anyhow::anyhow!("Failed to open stdout"))?;

        let (tx, mut rx) = mpsc::channel::<Value>(32);
        let pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<anyhow::Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let diagnostics_cache = Arc::new(RwLock::new(HashMap::new()));

        // Writer task
        let mut stdin = stdin;
        let pending_requests_writer = pending_requests.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let body = serde_json::to_string(&msg).expect("Failed to serialize LSP message");
                let content_length = body.len();
                let header = format!("Content-Length: {}\r\n\r\n{}", content_length, body);

                if let Err(e) = stdin.write_all(header.as_bytes()).await {
                    log::error!("Failed to write to LSP stdin: {}", e);
                    abort_pending_requests(&pending_requests_writer).await;
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    log::error!("Failed to flush LSP stdin: {}", e);
                    abort_pending_requests(&pending_requests_writer).await;
                    break;
                }
            }
        });

        // Reader task
        let pending_requests_clone = pending_requests.clone();
        let diagnostics_cache_clone = diagnostics_cache.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                // Read headers
                let mut size = 0;
                let mut line = String::new();

                loop {
                    line.clear();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        return; // EOF
                    }
                    if line == "\r\n" {
                        break; // End of headers
                    }
                    if line.starts_with("Content-Length: ") {
                        if let Ok(len) = line
                            .trim_start_matches("Content-Length: ")
                            .trim()
                            .parse::<usize>()
                        {
                            size = len;
                        }
                    }
                }

                if size > 0 {
                    let mut buf = vec![0; size];
                    if reader.read_exact(&mut buf).await.is_err() {
                        break;
                    }
                    if let Ok(msg_str) = String::from_utf8(buf) {
                        if let Ok(msg) = serde_json::from_str::<Value>(&msg_str) {
                            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                                // It's a notification or request from server
                                if method == "textDocument/publishDiagnostics" {
                                    if let Some(params) = msg.get("params") {
                                        if let Some(uri) =
                                            params.get("uri").and_then(|u| u.as_str())
                                        {
                                            if let Some(diags) =
                                                params.get("diagnostics").and_then(|d| d.as_array())
                                            {
                                                let mut cache =
                                                    diagnostics_cache_clone.write().await;
                                                cache.insert(uri.to_string(), diags.clone());
                                            }
                                        }
                                    }
                                } else if method == "textDocument/didClose" {
                                    if let Some(params) = msg.get("params") {
                                        if let Some(uri) = params
                                            .get("textDocument")
                                            .and_then(|td| td.get("uri"))
                                            .and_then(|u| u.as_str())
                                        {
                                            let mut cache = diagnostics_cache_clone.write().await;
                                            cache.remove(uri);
                                        }
                                    }
                                }
                            } else if let Some(id) = msg.get("id").and_then(|id| id.as_i64()) {
                                // It's a response
                                if msg.get("result").is_some() || msg.get("error").is_some() {
                                    let mut pending = pending_requests_clone.lock().await;
                                    if let Some(sender) = pending.remove(&id) {
                                        let result = if let Some(err) = msg.get("error") {
                                            Err(anyhow::anyhow!("LSP Error: {:?}", err))
                                        } else if let Some(res) = msg.get("result") {
                                            Ok(res.clone())
                                        } else {
                                            Ok(Value::Null)
                                        };
                                        let _ = sender.send(result);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            child,
            request_id: AtomicI64::new(1),
            sender: tx,
            pending_requests,
            diagnostics_cache,
        })
    }

    pub async fn initialize(&self, root_path: Option<String>) -> anyhow::Result<()> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let root_uri = root_path.map(|p| format!("file://{}", p));

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {}
            }
        });

        self.send_request(id, req).await?;

        // Send initialized notification
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        self.sender
            .send(notif)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send initialized"))?;

        Ok(())
    }

    pub async fn hover(&self, uri: &str, line: u64, character: u64) -> anyhow::Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        });

        self.send_request(id, req).await
    }

    pub async fn definition(&self, uri: &str, line: u64, character: u64) -> anyhow::Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        });
        self.send_request(id, req).await
    }

    pub async fn references(
        &self,
        uri: &str,
        line: u64,
        character: u64,
        include_decl: bool,
    ) -> anyhow::Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "includeDeclaration": include_decl }
            }
        });
        self.send_request(id, req).await
    }

    pub async fn rename(
        &self,
        uri: &str,
        line: u64,
        character: u64,
        new_name: &str,
    ) -> anyhow::Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/rename",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "newName": new_name
            }
        });
        self.send_request(id, req).await
    }

    pub async fn code_action(&self, uri: &str, range: Value) -> anyhow::Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/codeAction",
            "params": {
                "textDocument": { "uri": uri },
                "range": range,
                "context": { "diagnostics": [] }
            }
        });
        self.send_request(id, req).await
    }

    async fn send_request(&self, id: i64, req: Value) -> anyhow::Result<Value> {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        self.sender
            .send(req)
            .await
            .map_err(|_| anyhow::anyhow!("Client sender closed"))?;

        rx.await
            .map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    #[allow(dead_code)]
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

async fn abort_pending_requests(
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<anyhow::Result<Value>>>>>,
) {
    let mut map = pending.lock().await;
    for (_, sender) in map.drain() {
        let _ = sender.send(Err(anyhow::anyhow!("LSP Client connection lost")));
    }
}
