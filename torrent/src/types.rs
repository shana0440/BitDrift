use bitvec::{order::Msb0, vec::BitVec};

pub type Sha1Hash = [u8; 20];

pub type PeerId = [u8; 20];

// Represents which pieces exists for a peer.
// Each bit represents a piece, where 1 means the piece exists and 0 means it does not.
// The length of the BitField is determined by the number of pieces in the torrent.
// It can calculate by metainfo.info.piece_length and the total file size.
// Using Msb0 order for BitVec to match the BitTorrent protocol specification.
// https://www.bittorrent.org/beps/bep_0003.html#peer-messages
pub type BitField = BitVec<u8, Msb0>;
