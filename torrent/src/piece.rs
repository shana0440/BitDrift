use bitvec::vec::BitVec;
use bytes::BytesMut;
use thiserror::Error;

use crate::{hash::calculate_sha1_hash, types::Sha1Hash};

pub(crate) type Result<T> = std::result::Result<T, PieceError>;

#[derive(Debug, Error)]
pub enum PieceError {
    #[error("Invalid piece hash")]
    InvalidHash,
    #[error("Incomplete blocks received")]
    IncompleteBlocks,
    #[error("Invalid block")]
    InvalidBlock,
}

#[derive(Clone)]
enum PieceStatus {
    Verified(Vec<u8>),
    UnVerified(Vec<Block>),
}

pub struct Piece {
    pub index: usize,
    pub hash: Sha1Hash,
    pub status: PieceStatus,
    pub length: u32,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct Block {
    pub piece_index: u32,
    pub begin: u32,
    pub data: Vec<u8>,
}

impl Piece {
    pub fn new_unverified(index: usize, hash: Sha1Hash, length: u32) -> Self {
        Self {
            index,
            hash,
            length,
            status: PieceStatus::UnVerified(Vec::new()),
        }
    }

    pub fn new_verified(index: usize, hash: Sha1Hash, length: u32, data: Vec<u8>) -> Self {
        Self {
            index,
            hash,
            length,
            status: PieceStatus::Verified(data),
        }
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        match &self.status {
            PieceStatus::Verified(items) => Err(PieceError::InvalidBlock),
            PieceStatus::UnVerified(blocks) => {
                let mut new_blocks = blocks.clone();
                new_blocks.push(block);
                self.status = PieceStatus::UnVerified(new_blocks);
                Ok(())
            }
        }
    }

    pub fn verify(&mut self) -> Result<Vec<u8>> {
        match &self.status {
            PieceStatus::Verified(data) => Ok(()),
            PieceStatus::UnVerified(blocks) => {
                if !self.is_all_blocks_received() {
                    return Err(PieceError::IncompleteBlocks);
                }
                let received_pieces_length = blocks.iter().map(|it| it.data.len()).sum();
                let mut data = BytesMut::with_capacity(received_pieces_length);
                for block in blocks {
                    let begin = block.begin as usize;
                    data[begin..begin + block.data.len()].copy_from_slice(&block.data);
                }
                let hash = calculate_sha1_hash(data.to_vec());
                if self.hash == hash {
                    self.status = PieceStatus::Verified(data.to_vec());
                    Ok(data.to_vec())
                } else {
                    Err(PieceError::InvalidHash)
                }
            }
        }
    }

    pub fn is_all_blocks_received(&self) -> bool {
        match &self.status {
            PieceStatus::Verified(data) => true,
            PieceStatus::UnVerified(blocks) => {
                // Last one piece may be truncated due to file length,
                // so we check the diff between received length and expected length is less than block size
                // to determine received all blocks or not.
                let received_pieces_length: usize = blocks.iter().map(|it| it.data.len()).sum();
                let diff = self.length as usize - received_pieces_length;
                if let Some(block) = blocks.first() {
                    diff < block.data.len()
                } else {
                    false
                }
            }
        }
    }

    pub fn request(&self, begin: usize, length: usize) -> Vec<u8> {
        match &self.status {
            PieceStatus::Verified(data) => data[begin..begin + length].to_vec(),
            PieceStatus::UnVerified(blocks) => panic!("Request data from unverified piece"),
        }
    }
}
