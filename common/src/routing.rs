use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct UdpSession<T> {
    pub external_addr: SocketAddr,
    pub local_data: T,
    pub last_activity: Instant,
}

pub struct UdpRoutingTable<T> {
    // Maps external addresses to session info
    external_to_session: HashMap<SocketAddr, UdpSession<T>>,
    // Maps local addresses to external addresses (for client-side routing)
    local_to_external: HashMap<SocketAddr, SocketAddr>,
    // Session timeout
    session_timeout: Duration,
}

impl<T: Clone> UdpRoutingTable<T> {
    pub fn new(session_timeout_secs: u64) -> Self {
        Self {
            external_to_session: HashMap::new(),
            local_to_external: HashMap::new(),
            session_timeout: Duration::from_secs(session_timeout_secs),
        }
    }

    pub fn register_session(&mut self, external_addr: SocketAddr, local_data: T) {
        let session = UdpSession {
            external_addr,
            local_data,
            last_activity: Instant::now(),
        };

        self.external_to_session.insert(external_addr, session);
        debug!("Registered UDP session for {}", external_addr);
    }

    pub fn register_client_session(
        &mut self,
        external_addr: SocketAddr,
        local_addr: SocketAddr,
        local_data: T,
    ) {
        let session = UdpSession {
            external_addr,
            local_data,
            last_activity: Instant::now(),
        };

        self.external_to_session.insert(external_addr, session);
        self.local_to_external.insert(local_addr, external_addr);

        debug!(
            "Registered UDP client session: {} <-> {}",
            external_addr, local_addr
        );
    }

    pub fn update_activity(&mut self, addr: &SocketAddr, is_external: bool) {
        if is_external {
            if let Some(session) = self.external_to_session.get_mut(addr) {
                session.last_activity = Instant::now();
            }
        } else {
            // Update via local address
            if let Some(external_addr) = self.local_to_external.get(addr) {
                if let Some(session) = self.external_to_session.get_mut(external_addr) {
                    session.last_activity = Instant::now();
                }
            }
        }
    }

    pub fn get_session(&self, external_addr: &SocketAddr) -> Option<&UdpSession<T>> {
        self.external_to_session.get(external_addr)
    }

    pub fn get_local_data(&self, external_addr: &SocketAddr) -> Option<&T> {
        self.external_to_session
            .get(external_addr)
            .map(|s| &s.local_data)
    }

    pub fn get_external_addr(&self, local_addr: &SocketAddr) -> Option<SocketAddr> {
        self.local_to_external.get(local_addr).copied()
    }

    pub fn cleanup_expired_sessions(&mut self) -> usize {
        let now = Instant::now();

        let mut expired_external: Vec<SocketAddr> = Vec::new();
        let mut expired_local: Vec<SocketAddr> = Vec::new();

        for (external_addr, session) in &self.external_to_session {
            if now.duration_since(session.last_activity) > self.session_timeout {
                expired_external.push(*external_addr);
                // Find corresponding local addresses to clean up
                for (local_addr, ext_addr) in &self.local_to_external {
                    if *ext_addr == *external_addr {
                        expired_local.push(*local_addr);
                    }
                }
            }
        }

        for addr in &expired_external {
            self.external_to_session.remove(addr);
            debug!("Cleaned up expired UDP session for external {}", addr);
        }

        for addr in &expired_local {
            self.local_to_external.remove(addr);
        }

        let cleaned = expired_external.len();
        if cleaned > 0 {
            info!("Cleaned up {} expired UDP sessions", cleaned);
        }
        cleaned
    }

    pub fn session_count(&self) -> usize {
        self.external_to_session.len()
    }
}
