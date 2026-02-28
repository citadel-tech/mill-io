use std::net::SocketAddr;

/// Context available to RPC handlers during request processing.
///
/// Provides metadata about the current request and the connection it arrived on.
#[derive(Debug, Clone)]
pub struct RpcContext {
    /// Unique ID for this request.
    pub request_id: u64,
    /// Peer address of the client.
    pub peer_addr: Option<SocketAddr>,
    /// Service ID being called.
    pub service_id: u16,
    /// Method ID being called.
    pub method_id: u16,
}

impl RpcContext {
    pub fn new(request_id: u64, service_id: u16, method_id: u16) -> Self {
        Self {
            request_id,
            peer_addr: None,
            service_id,
            method_id,
        }
    }

    pub fn with_peer_addr(mut self, addr: SocketAddr) -> Self {
        self.peer_addr = Some(addr);
        self
    }
}
