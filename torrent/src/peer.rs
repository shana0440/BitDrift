use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::types::PeerId;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Peer {
    #[serde(rename = "id")]
    pub peer_id: Option<PeerId>,
    #[serde(rename = "ip")]
    pub ip: String,
    #[serde(rename = "port")]
    pub port: u16,
}

impl Peer {
    pub fn from_socket_addr(addr: SocketAddr) -> Self {
        Self {
            peer_id: None,
            ip: addr.ip().to_string(),
            port: addr.port(),
        }
    }
}
