//! Proxy server implementations

pub mod http_proxy;
pub mod socks_proxy;

pub use http_proxy::HttpProxy;
pub use socks_proxy::SocksProxy;
