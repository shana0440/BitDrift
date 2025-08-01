use std::{collections::HashMap, sync::Arc};

use bitvec::vec::BitVec;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
    metainfo::MetaInfo,
    piece::{Block, Piece, PieceError},
    piece_picker::PiecePicker,
};

pub(crate) type Result<T> = std::result::Result<T, TorrentError>;

#[derive(Debug, Error)]
pub enum TorrentError {
    #[error("invalid piece index")]
    InvalidPieceIndex,
    #[error("piece error")]
    Piece(#[from] PieceError),
}

pub struct Torrent {
    pieces: Vec<Piece>,
    piece_picker: Arc<Mutex<PiecePicker>>,
}

impl Torrent {
    pub fn from_metainfo(metainfo: MetaInfo) -> Self {
        let piece_length = metainfo.info.piece_length;
        let total_bytes = metainfo.total_bytes() as u32;
        let piece_size = total_bytes / piece_length;
        let piece_picker = PiecePicker::new(
            // TODO: if already have downloaded piece, read from disk
            BitVec::repeat(false, piece_size as usize),
            total_bytes,
            piece_length,
        );
        Self {
            pieces: Vec::new(),
            piece_picker: Arc::new(Mutex::new(piece_picker)),
        }
    }

    pub async fn add_block(&mut self, block: Block) -> Result<()> {
        let mut piece_picker = self.piece_picker.lock().await;
        piece_picker.mark_received(&block);

        if let Some(piece) = self.pieces.get_mut(block.piece_index as usize) {
            match piece.add_block(block) {
                Ok(_) => {
                    if piece.is_all_blocks_received() {
                        match piece.verify() {
                            Ok(_) => {
                                // TODO: write to disk and send have message
                                Ok(())
                            }
                            Err(e) => Err(TorrentError::Piece(e)),
                        }
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(TorrentError::Piece(e)),
            }
        } else {
            Err(TorrentError::InvalidPieceIndex)
        }
    }
}
