use sha1::{Digest, Sha1};

use crate::types::Sha1Hash;

pub fn calculate_sha1_hash(data: Vec<u8>) -> Sha1Hash {
    let digest = Sha1::digest(&data);
    let mut hash = [0u8; 20];
    hash.copy_from_slice(&digest);
    hash
}
