use crate::types::BitField;

// Used to track the state of each block
pub struct PiecePicker {
    own_bitfield: BitField,
    // the total file length of metainfo
    total_length: u32,
    piece_length: u32,
    missing_blocks: Vec<BlockInfo>,
}

// Block size 16KB is recommend by document
// https://www.bittorrent.org/beps/bep_0003.html#peer-messages
const BLOCK_SIZE: u32 = 16 * 1024;

struct BlockInfo {
    pub piece_index: u32,
    pub begin: u32,
    pub length: u32,

    state: BlockState,
}

impl BlockInfo {
    fn new(piece_index: u32, begin: u32, length: u32) -> Self {
        Self {
            piece_index,
            begin,
            length,
            state: BlockState::NotRequested,
        }
    }

    pub fn index(&self) -> u32 {
        self.begin / BLOCK_SIZE
    }

    fn is_same_block(&self, block: &BlockInfo) -> bool {
        self.piece_index == block.piece_index
            && self.begin == block.begin
            && self.length == block.length
    }
}

#[derive(Clone, PartialEq)]
pub enum BlockState {
    NotRequested,
    Requested,
    Received,
}

impl PiecePicker {
    pub fn new(own_bitfield: BitField, total_length: u32, piece_length: u32) -> Self {
        let num_of_missing_blocks = own_bitfield.iter().fold(0, |acc, it| {
            if it == false {
                acc + piece_length / BLOCK_SIZE
            } else {
                acc
            }
        });

        let mut missing_blocks = Vec::with_capacity(num_of_missing_blocks as usize);

        for piece_index in 0..own_bitfield.len() {
            if own_bitfield[piece_index] == false {
                let num_of_blocks = piece_length / BLOCK_SIZE;
                for i in 0..num_of_blocks {
                    let info = BlockInfo::new(
                        piece_index as u32,
                        i * BLOCK_SIZE,
                        PiecePicker::block_size(
                            &own_bitfield,
                            piece_length,
                            total_length,
                            piece_index as u32,
                            i,
                        ),
                    );
                    missing_blocks.push(info);
                }
            }
        }

        Self {
            own_bitfield,
            missing_blocks,
            total_length,
            piece_length,
        }
    }

    pub fn pick_block(&mut self, peer_bitfield: &BitField) -> Option<&BlockInfo> {
        self.missing_blocks.iter().find(|it| {
            peer_bitfield[it.piece_index as usize] == true && it.state == BlockState::NotRequested
        })
    }

    fn block_size(
        own_bitfield: &BitField,
        piece_length: u32,
        total_length: u32,
        piece_index: u32,
        block_index: u32,
    ) -> u32 {
        let is_last_piece = own_bitfield.len() as u32 == piece_index + 1;
        let is_last_block = block_index * BLOCK_SIZE + BLOCK_SIZE >= piece_length;
        if is_last_piece && is_last_block {
            let last_block_size = total_length % BLOCK_SIZE;
            last_block_size
        } else {
            BLOCK_SIZE
        }
    }

    pub fn mark_received(&mut self, piece_index: u32, block: &BlockInfo) {
        let mut_block = self
            .missing_blocks
            .iter_mut()
            .find(|it| it.is_same_block(block));
        if let Some(mut_block) = mut_block {
            mut_block.state = BlockState::Received;
            let is_all_blocks_received = self
                .missing_blocks
                .iter()
                .filter(|it| it.piece_index == block.piece_index)
                .all(|it| it.state == BlockState::Received);
            if is_all_blocks_received {
                self.own_bitfield.set(piece_index as usize, true);
            }
        }
    }
}
