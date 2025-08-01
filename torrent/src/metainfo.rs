use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use url::Url;

use crate::types::Sha1Hash;

pub(crate) type Result<T> = std::result::Result<T, MetaInfoError>;

#[derive(Error, Debug)]
pub enum MetaInfoError {
    #[error("Failed to parse .torrent file")]
    Bencode(#[from] serde_bencode::Error),

    #[error("Failed to parse URL")]
    InvalidAnnounce(#[from] url::ParseError),
}

#[derive(Debug, Clone)]
pub struct MetaInfo {
    pub announce: Url,
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
            announce: Url::parse(&metainfo.announce)?,
            info: metainfo.info,
            comment: metainfo.comment,
            created_by: metainfo.created_by,
            creation_date: metainfo.creation_date,
            info_hash,
        })
    }

    pub fn total_bytes(self) -> usize {
        if let Some(length) = self.info.length {
            return length as usize;
        }
        if let Some(files) = self.info.files {
            return files.iter().fold(0, |acc, it| acc + it.length as usize);
        }
        panic!("Invalid metainfo, must have length or files");
    }
}

pub mod raw {
    use crate::hash::calculate_sha1_hash;

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

    #[derive(Serialize, Deserialize, Clone)]
    pub struct Info {
        pub name: String,
        // How many bytes each piece is.
        #[serde(rename = "piece length")]
        pub piece_length: u32,
        // The SHA1 hash of each piece, concatenated together.
        // Used to verify the integrity of the pieces.
        #[serde(with = "serde_bytes")]
        pub pieces: Vec<u8>,
        // If this is a single file torrent, this is the length of the file, in bytes.
        pub length: Option<u64>,
        // If this is a multi-file torrent, this is a list of files.
        pub files: Option<Vec<File>>,
        // We not going to use the extra fields,
        // but we need this to capture any additional fields to get the correct info_hash.
        #[serde(flatten)]
        pub extra: std::collections::BTreeMap<String, serde_bencode::value::Value>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct File {
        // The length of the file, in bytes.
        pub length: u64,
        pub path: Vec<String>,
    }

    impl MetaInfo {
        pub fn calculate_info_hash(&self) -> Result<Sha1Hash> {
            let info = serde_bencode::to_bytes(&self.info)?;
            let info_hash = calculate_sha1_hash(info);
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
                .field("extra", &self.extra)
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
