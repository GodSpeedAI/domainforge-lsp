use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

/// Security and Stability Guardrails for the MCP Server
pub struct Guard {
    /// Allowed workspace roots. Access to files outside these roots is denied.
    allowed_roots: Vec<PathBuf>,
    /// Rate limiters per tool type
    rate_limiters: Arc<Mutex<HashMap<String, RateLimiter>>>,
}

impl Guard {
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self {
        // Canonicalize roots at startup to handle symlinks correctly
        let allowed_roots = allowed_roots
            .into_iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();

        Self {
            allowed_roots,
            rate_limiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Verifies that a path is within the allowed workspace roots.
    /// Returns the canonicalized path if allowed, or an error if denied.
    pub fn check_path<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let path = path.as_ref();
        let canonical_path = path
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("Invalid path {:?}: {}", path, e))?;

        if self
            .allowed_roots
            .iter()
            .any(|root| canonical_path.starts_with(root))
        {
            Ok(canonical_path)
        } else {
            Err(anyhow::anyhow!(
                "Access denied: Path {:?} is outside workspace roots",
                path
            ))
        }
    }

    /// Checks if a request for a specific tool should be allowed based on rate limits.
    /// Returns Ok if allowed, Err if rate limit exceeded.
    pub fn check_rate_limit(&self, tool_name: &str) -> Result<()> {
        let mut limiters = self.rate_limiters.lock().unwrap();
        let limiter = limiters.entry(tool_name.to_string()).or_insert_with(|| {
            // Default limits based on tool type
            match tool_name {
                "domainforge/hover" => RateLimiter::new(20, Duration::from_secs(1)),
                "domainforge/diagnostics" => RateLimiter::new(10, Duration::from_secs(1)),
                "domainforge/definition" => RateLimiter::new(10, Duration::from_secs(1)),
                "domainforge/references" => RateLimiter::new(5, Duration::from_secs(1)),
                "domainforge/code-actions" => RateLimiter::new(5, Duration::from_secs(1)),
                "domainforge/rename-preview" => RateLimiter::new(2, Duration::from_secs(1)),
                _ => RateLimiter::new(10, Duration::from_secs(1)), // Default for unknown tools
            }
        });

        if limiter.check() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Rate limit exceeded for tool: {}",
                tool_name
            ))
        }
    }
}

/// Token bucket rate limiter
struct RateLimiter {
    max_tokens: u32,
    tokens: f64,
    fill_rate: f64, // tokens per second
    last_update: Instant,
}

impl RateLimiter {
    fn new(max_tokens: u32, period: Duration) -> Self {
        let fill_rate = max_tokens as f64 / period.as_secs_f64();
        Self {
            max_tokens,
            tokens: max_tokens as f64,
            fill_rate,
            last_update: Instant::now(),
        }
    }

    fn check(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();

        // Refill tokens
        self.tokens = (self.tokens + elapsed * self.fill_rate).min(self.max_tokens as f64);
        self.last_update = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_path_verification() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let file_path = root.join("test.txt");
        File::create(&file_path).unwrap();

        let guard = Guard::new(vec![root.clone()]);

        // Allowed path
        assert!(guard.check_path(&file_path).is_ok());

        // Denied path (outside root)
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("outside.txt");
        File::create(&outside_file).unwrap();
        assert!(guard.check_path(&outside_file).is_err());
    }

    #[test]
    fn test_rate_limiting() {
        // Create a guard with dummy root
        let guard = Guard::new(vec![]);

        // "test_tool" will fallback to default 10 requests/sec
        // Let's create a custom limiter test by mocking or just trusting the math.
        // But since we can't easily mock time with std::time::Instant in simple tests without external crates,
        // we'll simulate rapid calls.

        let tool = "domainforge/rename-preview"; // limit 2/sec

        assert!(guard.check_rate_limit(tool).is_ok()); // 1st
        assert!(guard.check_rate_limit(tool).is_ok()); // 2nd

        // 3rd should fail immediately
        assert!(guard.check_rate_limit(tool).is_err());
    }
}
