use tokio::{sync::broadcast, time::Instant};

use crate::types::BitField;

#[derive(Debug)]
pub struct PeerConnection {
    pub peer_bitfield: BitField,

    // I'm choke the peer
    pub is_choked: bool,
    // I'm interested the peer
    pub is_interesting: bool,
    // The peer is choke me
    pub is_peer_choked: bool,
    // The peer is interested me
    pub is_peer_interesting: bool,

    // Last time I'm unchoke the peer
    pub last_unchoked_at: Option<Instant>,
}

impl PeerConnection {
    pub fn new(bitfield_len: usize) -> Self {
        Self {
            peer_bitfield: BitField::with_capacity(bitfield_len),
            is_choked: true,
            is_interesting: false,
            is_peer_choked: true,
            is_peer_interesting: false,
            last_unchoked_at: None,
        }
    }
}
