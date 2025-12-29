//! SOCKS5 proxy server with domain filtering

use crate::error::{Result, SandboxError};
use fast_socks5::server::{Config, Socks5Server, Socks5Socket};
use fast_socks5::{Result as SocksResult, SocksError};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, info, warn};
use regex::Regex;

/// SOCKS5 Proxy server
pub struct SocksProxy {
    allowed_domains: Arc<Vec<Regex>>,
    denied_domains: Arc<Vec<Regex>>,
    port: u16,
}

impl SocksProxy {
    /// Create a new SOCKS5 proxy
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

        info!("SOCKS5 proxy listening on {}", local_addr);

        let allowed = Arc::clone(&self.allowed_domains);
        let denied = Arc::clone(&self.denied_domains);

        tokio::spawn(async move {
            let config = Config::default();
            let server = Socks5Server::new(listener, Arc::new(config));

            loop {
                match server.accept().await {
                    Ok(socket) => {
                        let allowed = Arc::clone(&allowed);
                        let denied = Arc::clone(&denied);

                        tokio::spawn(async move {
                            if let Err(e) = handle_socks_connection(socket, allowed, denied).await {
                                warn!("SOCKS connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("SOCKS accept error: {}", e);
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

async fn handle_socks_connection(
    socket: Socks5Socket<fast_socks5::server::IncomingConnection>,
    allowed_domains: Arc<Vec<Regex>>,
    denied_domains: Arc<Vec<Regex>>,
) -> SocksResult<()> {
    let request = socket.upgrade_to_socks5().await?;

    let target_host = match &request.target_addr {
        fast_socks5::util::target_addr::TargetAddr::Ip(ip) => ip.ip().to_string(),
        fast_socks5::util::target_addr::TargetAddr::Domain(domain, _) => domain.clone(),
    };

    debug!("SOCKS5 request to: {}", target_host);

    if !is_domain_allowed(&target_host, &allowed_domains, &denied_domains) {
        warn!("Blocked SOCKS request to: {}", target_host);
        return Err(SocksError::Other(anyhow::anyhow!(
            "Access to {} is blocked by sandbox policy",
            target_host
        )));
    }

    // Connect to the target
    request.connect().await?;

    Ok(())
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
            domain_to_regex("*.github.com").unwrap(),
            domain_to_regex("gitlab.com").unwrap(),
        ];
        let denied = vec![domain_to_regex("malicious.github.com").unwrap()];

        assert!(is_domain_allowed("api.github.com", &allowed, &denied));
        assert!(is_domain_allowed("gitlab.com", &allowed, &denied));
        assert!(!is_domain_allowed("malicious.github.com", &allowed, &denied));
        assert!(!is_domain_allowed("other.com", &allowed, &denied));
    }

    #[tokio::test]
    async fn test_proxy_creation() {
        let mut proxy = SocksProxy::new(
            vec!["*.github.com".to_string()],
            vec!["evil.com".to_string()],
        )
        .unwrap();

        let port = proxy.start().await.unwrap();
        assert!(port > 0);
    }
}
