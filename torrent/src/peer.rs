use std::{net::SocketAddr, time::Duration};

use futures::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::{net::TcpStream, time::interval};
use tokio_util::codec::Framed;

use crate::{
    message::{HandShake, HandShakeCodec, Message, MessageCodec},
    peer_stats::PeerStats,
    types::{BitField, PeerId, Sha1Hash},
};

pub(crate) type Result<T> = std::result::Result<T, PeerError>;

#[derive(Debug, Error)]
enum PeerError {
    #[error("Failed to connect to peer")]
    Io(#[from] std::io::Error),
}

enum Session {
    Idle(IdleSession),
    Connected(ConnectedSession),
    Active(ActiveSession),
    Disconnected(DisconnectedSession),
}

struct IdleSession {
    addr: SocketAddr,
}

struct ConnectedSession {
    socket: Framed<TcpStream, HandShakeCodec>,
}

struct SessionContext {
    is_choked: bool,
    is_interested: bool,
    is_peer_choked: bool,
    is_peer_interested: bool,
}

struct ActiveSession {
    socket: Framed<TcpStream, MessageCodec>,
    is_bitfield_exchanged: bool,
    ctx: SessionContext,
    bitfield: Option<BitField>,
    stats: PeerStats,
}

struct DisconnectedSession;

impl IdleSession {
    fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }

    async fn connect(self) -> Result<Session> {
        let socket = TcpStream::connect(self.addr).await?;
        let socket = Framed::new(socket, HandShakeCodec);
        Ok(Session::Connected(ConnectedSession::new(socket)))
    }
}

impl ConnectedSession {
    fn new(socket: Framed<TcpStream, HandShakeCodec>) -> Self {
        Self { socket }
    }

    async fn handshake(self, info_hash: Sha1Hash, peer_id: PeerId) -> Result<Session> {
        let mut socket = self.socket;
        log::info!("Waiting for handshake with peer");
        let handshake = HandShake::new(info_hash, peer_id);
        socket.send(handshake).await?;
        if let Some(handshake) = socket.next().await {
            match handshake {
                Ok(handshake) => {
                    log::info!("Received handshake response from peer");
                    if handshake.info_hash != info_hash {
                        log::error!(
                            "Info hash mismatch: expected {:?}, got {:?}",
                            info_hash,
                            handshake.info_hash
                        );
                        socket.close().await?;
                        Ok(Session::Disconnected(DisconnectedSession {}))
                    } else {
                        let socket = Framed::new(socket.into_inner(), MessageCodec);
                        Ok(Session::Active(ActiveSession::new(socket)))
                    }
                }
                Err(e) => {
                    log::error!("Failed to decode handshake response: {:?}", e);
                    socket.close().await?;
                    return Err(PeerError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Failed to decode handshake response",
                    )));
                }
            }
        } else {
            log::error!("Did not receive handshake response from peer");
            socket.close().await?;
            Ok(Session::Disconnected(DisconnectedSession {}))
        }
    }
}

impl ActiveSession {
    fn new(socket: Framed<TcpStream, MessageCodec>) -> Self {
        Self {
            socket,
            ctx: SessionContext {
                is_choked: true,
                is_interested: false,
                is_peer_choked: true,
                is_peer_interested: false,
            },
            is_bitfield_exchanged: false,
            bitfield: None,
            stats: PeerStats::new(20),
        }
    }

    async fn on_tick(&mut self) -> Result<()> {
        // Check if we need to send keep-alive message or any other message should be sent.
        Ok(())
    }

    async fn on_message(&mut self, message: Message) -> Result<()> {
        let message_id = message.message_id();
        log::info!("Received message: {:?}", message_id);
        match message {
            Message::KeepAlive => Ok(()),
            Message::Choke => {
                self.ctx.is_peer_choked = true;
                Ok(())
            }
            Message::Unchoke => {
                self.ctx.is_peer_choked = false;
                Ok(())
            }
            Message::Interested => {
                self.ctx.is_peer_interested = true;
                // TODO: send unchoke message base on strategy
                Ok(())
            }
            Message::NotInterested => {
                self.ctx.is_peer_interested = false;
                // TODO: send choke message base on strategy
                Ok(())
            }
            Message::Have { piece_index } => {
                // TODO: update bitfield and check if we need to send interested message or request
                Ok(())
            }
            Message::Bitfield { bitfield } => {
                if !self.is_bitfield_exchanged {
                    self.is_bitfield_exchanged = true;
                    self.bitfield = Some(bitfield);
                    log::info!("Received bitfield message from peer");
                } else {
                    log::warn!("Received bitfield message again, ignoring");
                }
                Ok(())
            }
            Message::Request {
                piece_index,
                begin,
                length,
            } => {
                // TODO: if I have the piece and unchoked, try to send the piece base on strategy
                Ok(())
            }
            Message::Piece {
                piece_index,
                begin,
                piece,
            } => {
                // TODO: verify piece
                // if verified, write to disk and send have message to other peers
                // also update the own bitfield
                self.stats.record_download(piece.len());
                // TODO: save piece information and wait the piece is fully completed, verify it
                Ok(())
            }
            Message::Cancel {
                piece_index,
                begin,
                length,
            } => {
                // TODO: if piece not send yet, cancel the request
                Ok(())
            }
        }
    }

    async fn run(mut self) -> Result<Session> {
        log::info!("Handling messages with peer");

        let mut ticker = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                _now = ticker.tick() => {
                    self.on_tick().await?;
                }
                Some(message) = self.socket.next() => {
                    match message {
                        Ok(message) => {
                            self.on_message(message).await?;
                        }
                        Err(e) => {
                            log::error!("Failed to decode message: {:?}", e);
                            return Err(PeerError::Io(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Failed to decode message",
                            )));
                        }
                    }
                }
            }
        }

        self.socket.close().await?;
        Ok(Session::Disconnected(DisconnectedSession {}))
    }
}
