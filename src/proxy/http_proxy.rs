//! HTTP/HTTPS proxy server with domain filtering

use crate::error::{Result, SandboxError};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, body::Incoming, Method, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};
use regex::Regex;

/// HTTP Proxy server
pub struct HttpProxy {
    allowed_domains: Arc<Vec<Regex>>,
    denied_domains: Arc<Vec<Regex>>,
    port: u16,
}

impl HttpProxy {
    /// Create a new HTTP proxy
    pub fn new(allowed_domains: Vec<String>, denied_domains: Vec<String>) -> Result<Self> {
        let allowed_domains = allowed_domains
            .iter()
            .map(|d| domain_to_regex(d))
            .collect::<Result<Vec<_>>>()?;

        let denied_domains = denied_domains
            .iter()
            .map(|d| domain_to_regex(d))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            allowed_domains: Arc::new(allowed_domains),
            denied_domains: Arc::new(denied_domains),
            port: 0,
        })
    }

    /// Start the proxy server on a random port
    pub async fn start(&mut self) -> Result<u16> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        self.port = local_addr.port();

        info!("HTTP proxy listening on {}", local_addr);

        let allowed = Arc::clone(&self.allowed_domains);
        let denied = Arc::clone(&self.denied_domains);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let allowed = Arc::clone(&allowed);
                        let denied = Arc::clone(&denied);

                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, allowed, denied).await {
                                warn!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Accept error: {}", e);
                    }
                }
            }
        });

        Ok(self.port)
    }

    /// Get the proxy port
    pub fn port(&self) -> u16 {
        self.port
    }
}

async fn handle_connection(
    stream: TcpStream,
    allowed_domains: Arc<Vec<Regex>>,
    denied_domains: Arc<Vec<Regex>>,
) -> Result<()> {
    let io = TokioIo::new(stream);

    let service = service_fn(move |req: Request<Incoming>| {
        let allowed = Arc::clone(&allowed_domains);
        let denied = Arc::clone(&denied_domains);
        async move { handle_request(req, allowed, denied).await }
    });

    http1::Builder::new()
        .serve_connection(io, service)
        .await
        .map_err(|e| SandboxError::Proxy(e.to_string()))?;

    Ok(())
}

async fn handle_request(
    req: Request<Incoming>,
    allowed_domains: Arc<Vec<Regex>>,
    denied_domains: Arc<Vec<Regex>>,
) -> std::result::Result<Response<String>, hyper::Error> {
    let host = req
        .uri()
        .host()
        .or_else(|| {
            req.headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.split(':').next())
        })
        .unwrap_or("");

    debug!("HTTP request to: {}", host);

    if !is_domain_allowed(host, &allowed_domains, &denied_domains) {
        warn!("Blocked request to: {}", host);
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(format!("Access to {} is blocked by sandbox policy", host))
            .unwrap());
    }

    // For CONNECT method (HTTPS tunneling)
    if req.method() == Method::CONNECT {
        debug!("CONNECT request to: {}", host);
        // In a full implementation, we would establish a tunnel here
        // For now, just allow it if domain is permitted
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .body(String::new())
            .unwrap());
    }

    // For regular HTTP requests, we would proxy them here
    // For now, just return OK
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body("Proxied request".to_string())
        .unwrap())
}

/// Check if a domain is allowed
fn is_domain_allowed(domain: &str, allowed: &[Regex], denied: &[Regex]) -> bool {
    // Check denied list first (takes precedence)
    if denied.iter().any(|re| re.is_match(domain)) {
        return false;
    }

    // If no allowed list, allow by default
    if allowed.is_empty() {
        return true;
    }

    // Check allowed list
    allowed.iter().any(|re| re.is_match(domain))
}

/// Convert domain pattern to regex
fn domain_to_regex(pattern: &str) -> Result<Regex> {
    let pattern = pattern
        .replace(".", r"\.")
        .replace("*", ".*");

    Regex::new(&format!("^{}$", pattern))
        .map_err(|e| SandboxError::Config(format!("Invalid domain pattern: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_matching() {
        let allowed = vec![
            domain_to_regex("*.example.com").unwrap(),
            domain_to_regex("google.com").unwrap(),
        ];
        let denied = vec![domain_to_regex("evil.example.com").unwrap()];

        assert!(is_domain_allowed("api.example.com", &allowed, &denied));
        assert!(is_domain_allowed("google.com", &allowed, &denied));
        assert!(!is_domain_allowed("evil.example.com", &allowed, &denied));
        assert!(!is_domain_allowed("other.com", &allowed, &denied));
    }

    #[tokio::test]
    async fn test_proxy_creation() {
        let mut proxy = HttpProxy::new(
            vec!["*.example.com".to_string()],
            vec!["evil.com".to_string()],
        )
        .unwrap();

        let port = proxy.start().await.unwrap();
        assert!(port > 0);
    }
}
