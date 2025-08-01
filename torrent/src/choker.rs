use std::cmp::{Ordering, min};

use crate::peer_connection::PeerConnection;

struct Choker {
    /// A quota of peers that can be uploaded at same time.
    upload_slot: usize,
}

impl Choker {
    pub fn new(upload_slot: usize) -> Self {
        Self { upload_slot }
    }

    pub fn set_upload_slot(&mut self, upload_slot: usize) {
        self.upload_slot = upload_slot;
    }

    pub fn sort_by_unchoke(&self, peers: &mut Vec<PeerConnection>) -> usize {
        let upload_slot = min(self.upload_slot, peers.len());
        peers.select_nth_unstable_by(upload_slot - 1, |a, b| {
            Choker::unchoke_compare_round_robin(a, b)
        });

        upload_slot
    }

    /// Use to prioritizes peer to determine which peer should unchoke
    ///
    /// - unchoke the interested peer
    /// - if both peer is interested, unchoke the peer that have not been unchoke for a longer time
    fn unchoke_compare_round_robin(a: &PeerConnection, b: &PeerConnection) -> Ordering {
        match (a.is_peer_interesting, b.is_peer_interesting) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }

        match a.last_unchoked_at.cmp(&b.last_unchoked_at) {
            Ordering::Less => return Ordering::Less,
            Ordering::Greater => return Ordering::Greater,
            Ordering::Equal => return Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::peer_connection::PeerConnection;
    use std::time::Duration;
    use tokio::time::Instant;

    use super::*;
    fn make_peer(is_interested: bool, last_unchoke_at: Option<Instant>) -> PeerConnection {
        let mut peer = PeerConnection::new(30);
        peer.is_peer_interesting = is_interested;
        peer.last_unchoked_at = last_unchoke_at;
        peer
    }

    #[tokio::test]
    async fn test_sort_by_unchoke_basic() {
        let now = Instant::now();
        let mut peers = vec![
            make_peer(true, Some(now + Duration::from_secs(10))),
            make_peer(true, Some(now + Duration::from_secs(5))),
            make_peer(false, Some(now + Duration::from_secs(1))),
        ];
        let choker = Choker::new(2);
        let upload_slot = choker.sort_by_unchoke(&mut peers);

        assert_eq!(upload_slot, 2);
        // The first two should be interested peers, sorted by last_unchoke_at
        assert!(peers[0].is_peer_interesting);
        assert!(peers[1].is_peer_interesting);
        assert!(!peers[2].is_peer_interesting);
        assert!(peers[0].last_unchoked_at <= peers[1].last_unchoked_at);
    }

    #[tokio::test]
    async fn test_sort_by_unchoke_all_not_interested() {
        let now = Instant::now();
        let mut peers = vec![
            make_peer(false, Some(now + Duration::from_secs(1))),
            make_peer(false, Some(now + Duration::from_secs(2))),
        ];
        let choker = Choker::new(1);
        let upload_slot = choker.sort_by_unchoke(&mut peers);

        assert_eq!(upload_slot, 1);
        assert!(!peers[0].is_peer_interesting);
        assert!(!peers[1].is_peer_interesting);
    }

    #[tokio::test]
    async fn test_sort_by_unchoke_upload_slot_greater_than_peers() {
        let now = Instant::now();
        let mut peers = vec![
            make_peer(true, Some(now + Duration::from_secs(3))),
            make_peer(true, Some(now + Duration::from_secs(1))),
        ];
        let choker = Choker::new(5);
        let upload_slot = choker.sort_by_unchoke(&mut peers);

        assert_eq!(upload_slot, 2);
        assert!(peers[0].last_unchoked_at <= peers[1].last_unchoked_at);
    }

    #[tokio::test]
    async fn test_unchoke_compare_round_robin_ordering() {
        let now = Instant::now();
        let a = make_peer(true, Some(now + Duration::from_secs(1)));
        let b = make_peer(true, Some(now + Duration::from_secs(2)));
        assert_eq!(Choker::unchoke_compare_round_robin(&a, &b), Ordering::Less);
        assert_eq!(
            Choker::unchoke_compare_round_robin(&b, &a),
            Ordering::Greater
        );
        let c = make_peer(false, Some(now));
        assert_eq!(Choker::unchoke_compare_round_robin(&a, &c), Ordering::Less);
        assert_eq!(
            Choker::unchoke_compare_round_robin(&c, &a),
            Ordering::Greater
        );
    }

    #[tokio::test]
    async fn test_sort_by_unchoke_with_none_last_unchoke_at() {
        let now = Instant::now();
        let mut peers = vec![
            make_peer(true, None),
            make_peer(true, Some(now + Duration::from_secs(5))),
            make_peer(false, None),
            make_peer(false, Some(now + Duration::from_secs(2))),
        ];
        let choker = Choker::new(2);
        let upload_slot = choker.sort_by_unchoke(&mut peers);

        assert_eq!(upload_slot, 2);
        // The first two should be interested peers, and the one with None should be prioritized
        assert!(peers[0].is_peer_interesting);
        assert!(peers[1].is_peer_interesting);
        // None is considered less than Some, so peers[0] should have None last_unchoke_at
        assert!(peers[0].last_unchoked_at.is_none());
    }

    #[tokio::test]
    async fn test_unchoke_compare_round_robin_with_none_last_unchoke_at() {
        let now = Instant::now();
        let a = make_peer(true, None);
        let b = make_peer(true, Some(now));
        // None should be prioritized (treated as "older")
        assert_eq!(Choker::unchoke_compare_round_robin(&a, &b), Ordering::Less);
        assert_eq!(
            Choker::unchoke_compare_round_robin(&b, &a),
            Ordering::Greater
        );

        let c = make_peer(false, None);
        let d = make_peer(false, Some(now));
        assert_eq!(Choker::unchoke_compare_round_robin(&c, &d), Ordering::Less);
        assert_eq!(
            Choker::unchoke_compare_round_robin(&d, &c),
            Ordering::Greater
        );

        // Both None
        let e = make_peer(true, None);
        let f = make_peer(true, None);
        assert_eq!(Choker::unchoke_compare_round_robin(&e, &f), Ordering::Equal);
    }
}
