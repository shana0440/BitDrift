use std::collections::HashMap;

use thiserror::Error;

use crate::{
    metainfo::MetaInfo,
    piece::{Block, Piece, PieceError},
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
}

impl Torrent {
    pub fn from_metainfo(metainfo: MetaInfo) -> Self {
        Self { pieces: Vec::new() }
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        if let Some(piece) = self.pieces.get_mut(block.piece_index as usize) {
            match piece.add_block(block) {
                Ok(_) => {
                    if piece.is_all_blocks_received() {
                        match piece.verify() {
                            Ok(_) => {
                                // TODO: write to disk
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
