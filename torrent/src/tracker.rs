use std::net::{AddrParseError, SocketAddr};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};
use reqwest::Client;
use thiserror::Error;
use url::Url;

use crate::types::{PeerId, Sha1Hash};

pub(crate) type Result<T> = std::result::Result<T, TrackerError>;

const URL_ENCODE_RESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'~')
    .remove(b'.');

#[derive(Error, Debug)]
pub enum TrackerError {
    #[error("Http request failed")]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse tracker response")]
    Bencode(#[from] serde_bencode::Error),

    #[error("Query peers failed")]
    QueryPeers(String),

    #[error("Invalid IP address")]
    InvalidIpAddr(#[from] AddrParseError),
}

#[derive(Debug)]
pub struct Response {
    pub interval: u64,
    pub peers: Vec<SocketAddr>,
}

// Use to request peers from the tracker from the metainfo announce
// https://bittorrent.org/beps/bep_0003.html#trackers
pub struct Tracker {
    pub client: Client,

    pub url: Url,
}

#[derive(Debug)]
#[allow(dead_code)]
enum TrackerEvent {
    Started,
    Stopped,
    Completed,
    Empty,
}

#[derive(Debug)]
pub struct RequestParams {
    info_hash: Sha1Hash,
    peer_id: PeerId,
    ip: Option<String>,
    port: u16,
    uploaded: u64,
    downloaded: u64,
    left: u64,
    event: Option<TrackerEvent>,
    // If true, the peers are returned in compact format
    // https://www.bittorrent.org/beps/bep_0023.html
    compact: bool,
}

mod raw {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    use bytes::Buf;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum Response {
        Success(SuccessResponse),
        Error(ErrorResponse),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SuccessResponse {
        pub interval: u64,
        pub peers: Peer,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PeerItem {
        #[serde(rename = "id")]
        peer_id: Option<crate::types::PeerId>,
        #[serde(rename = "ip")]
        ip: String,
        #[serde(rename = "port")]
        port: u16,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum Peer {
        List(Vec<PeerItem>),
        #[serde(with = "serde_bytes")]
        Compact(Vec<u8>),
    }

    impl Peer {
        pub fn to_vec(&self) -> Result<Vec<SocketAddr>> {
            Ok(match self {
                Peer::List(peers) => peers
                    .iter()
                    .map(|peer| {
                        let ip: IpAddr = peer.ip.parse()?;
                        Ok(SocketAddr::new(ip, peer.port))
                    })
                    .collect::<Result<Vec<_>>>()?,
                // in compact format, each peer is represented by 6 bytes:
                // 4 bytes for the IPv4 address and 2 bytes for the port number
                // https://www.bittorrent.org/beps/bep_0023.html
                Peer::Compact(bytes) => {
                    let mut peers = Vec::new();

                    for mut chunk in bytes.chunks(6) {
                        if chunk.len() == 6 {
                            let ip = Ipv4Addr::from(chunk.get_u32());
                            let port = chunk.get_u16();
                            let addr = SocketAddr::new(IpAddr::V4(ip), port);
                            peers.push(addr);
                        }
                    }
                    peers
                }
            })
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ErrorResponse {
        #[serde(rename = "failure reason")]
        pub failure_reason: String,
    }
}

impl Tracker {
    pub fn new(url: Url) -> Self {
        let client = Client::new();
        Self { client, url }
    }

    pub async fn fetch_peers(&self, params: RequestParams) -> Result<Response> {
        let mut query = vec![
            ("port", params.port.to_string()),
            ("uploaded", params.uploaded.to_string()),
            ("downloaded", params.downloaded.to_string()),
            ("left", params.left.to_string()),
            ("compact", (params.compact as u8).to_string()),
        ];

        if let Some(ip) = params.ip {
            query.push(("ip", ip));
        }

        if let Some(event) = params.event {
            let event_str = match event {
                TrackerEvent::Started => "started",
                TrackerEvent::Stopped => "stopped",
                TrackerEvent::Completed => "completed",
                TrackerEvent::Empty => "",
            };
            query.push(("event", event_str.to_string()));
        }

        let info_hash_str = percent_encode(&params.info_hash, URL_ENCODE_RESERVED).to_string();
        let peer_id_str = percent_encode(&params.peer_id, URL_ENCODE_RESERVED).to_string();
        let url = format!(
            "{}?info_hash={}&peer_id={}",
            self.url.to_string(),
            info_hash_str,
            peer_id_str
        );

        let resp = self
            .client
            .get(&url)
            .query(&query)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        match serde_bencode::from_bytes::<raw::Response>(&resp) {
            Ok(resp) => match resp {
                raw::Response::Success(resp) => Ok(Response {
                    interval: resp.interval,
                    peers: resp.peers.to_vec()?,
                }),
                raw::Response::Error(e) => Err(TrackerError::QueryPeers(e.failure_reason)),
            },
            Err(e) => Err(TrackerError::Bencode(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_compact_peer_to_vec() {
        // 2 peers: 192.168.1.1:6881 and 10.0.0.2:51413
        let compact_bytes = vec![
            192, 168, 1, 1, 26, 225, // 192.168.1.1:6881 (6881 = 0x1AE1)
            10, 0, 0, 2, 200, 213, // 10.0.0.2:51413 (51413 = 0xC8D5)
        ];
        let compact = raw::Peer::Compact(compact_bytes);

        let peers = compact.to_vec();
        assert!(peers.is_ok());
        let peers = peers.unwrap();
        assert_eq!(peers.len(), 2);

        let expected_addrs = vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 6881),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 51413),
        ];

        for (peer, expected_addr) in peers.iter().zip(expected_addrs.iter()) {
            assert_eq!(peer, expected_addr);
        }
    }
}
