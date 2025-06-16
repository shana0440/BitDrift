use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

use crate::types::Sha1Hash;

pub use serde_bencode::Error as BencodeError;

pub(crate) type Result<T> = std::result::Result<T, MetaInfoError>;

#[derive(Error, Debug)]
enum MetaInfoError {
    #[error("Failed to parse .torrent file")]
    Bencode(#[from] BencodeError),
}

pub struct MetaInfo {
    pub announce: String,
    pub info: raw::Info,
    pub comment: Option<String>,
    pub created_by: Option<String>,
    pub creation_date: Option<f64>,
    pub info_hash: Sha1Hash,
}

impl MetaInfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let metainfo: raw::MetaInfo = serde_bencode::from_bytes(bytes)?;
        let info_hash = metainfo.calculate_info_hash()?;
        Ok(Self {
            announce: metainfo.announce,
            info: metainfo.info,
            comment: metainfo.comment,
            created_by: metainfo.created_by,
            creation_date: metainfo.creation_date,
            info_hash,
        })
    }
}

mod raw {
    use super::*;
    use sha1::{Digest, Sha1};

    // implementation of https://bittorrent.org/beps/bep_0003.html#metainfo-files
    #[derive(Debug, Serialize, Deserialize)]
    pub struct MetaInfo {
        pub announce: String,
        pub info: Info,
        pub comment: Option<String>,
        #[serde(rename = "created by")]
        pub created_by: Option<String>,
        #[serde(rename = "creation date")]
        pub creation_date: Option<f64>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Info {
        pub name: String,
        #[serde(rename = "piece length")]
        pub piece_length: u32,
        #[serde(with = "serde_bytes")]
        pub pieces: Vec<u8>,
        pub length: Option<u64>,
        pub files: Option<Vec<File>>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct File {
        pub length: u64,
        pub path: Vec<String>,
    }

    impl MetaInfo {
        pub fn calculate_info_hash(&self) -> Result<Sha1Hash> {
            let info = serde_bencode::to_bytes(&self.info)?;
            let digest = Sha1::digest(&info);
            let mut info_hash = [0u8; 20];
            info_hash.copy_from_slice(&digest);
            Ok(info_hash)
        }
    }

    impl fmt::Debug for Info {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Info")
                .field("name", &self.name)
                .field("piece_length", &self.piece_length)
                .field("pieces", &"<pieces...>")
                .field("length", &self.length)
                .field("files", &self.files)
                .finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_torrent_file() {
        let data = fs::read("tests/test.torrent").expect("Failed to read test.torrent");
        let metainfo = MetaInfo::from_bytes(&data);
        assert!(
            metainfo.is_ok(),
            "Failed to parse .torrent file: {:?}",
            metainfo.err()
        );
    }
}
