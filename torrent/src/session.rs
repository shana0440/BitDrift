use std::{collections::HashSet, sync::Arc};

use bitvec::vec::BitVec;
use tokio::sync::Mutex;

use crate::{
    message::Message, peer_connection::PeerConnection, piece::Block, piece_picker::BlockInfo,
    torrent::Torrent,
};

pub struct Session {
    torrent: Arc<Mutex<Torrent>>,
    peer_connection: PeerConnection,
    request_queue: Vec<BlockInfo>,
}

impl Session {
    pub async fn receive_msg(&mut self, msg: Message) {
        match msg {
            Message::KeepAlive => {}
            Message::Interested => {
                self.peer_connection.is_peer_interesting = true;
            }
            Message::NotInterested => {
                self.peer_connection.is_peer_interesting = false;
            }
            Message::Choke => {
                self.peer_connection.is_peer_choked = true;
            }
            Message::Unchoke => {
                self.peer_connection.is_peer_choked = false;
            }
            Message::Have { piece_index } => {
                self.peer_connection
                    .peer_bitfield
                    .set(piece_index as usize, true);
            }
            Message::Bitfield { bitfield } => {
                self.peer_connection.peer_bitfield = bitfield;
            }
            Message::Request {
                piece_index,
                begin,
                length,
            } => {
                if !self.peer_connection.is_choked {
                    self.request_queue
                        .push(BlockInfo::new(piece_index, begin, length))
                }
            }
            Message::Piece {
                piece_index,
                begin,
                piece,
            } => {
                let mut torrent = self.torrent.lock().await;
                match torrent.add_block(Block {
                    piece_index,
                    begin,
                    data: piece,
                }) {
                    Ok(_) => {}
                    Err(_) => {
                        // TODO: show error or mark block is unreceived.
                    }
                }
            }
            Message::Cancel {
                piece_index,
                begin,
                length,
            } => {
                self.request_queue.retain(|block| {
                    let cancel_block = BlockInfo::new(piece_index, begin, length);
                    !block.is_same_block(&cancel_block)
                });
            }
        }
    }
}
