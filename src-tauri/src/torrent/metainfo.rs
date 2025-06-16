use serde::{Deserialize, Serialize};
use std::fmt;

// https://bittorrent.org/beps/bep_0003.html#metainfo-files
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
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_bencode::Error> {
        serde_bencode::from_bytes(bytes)
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
