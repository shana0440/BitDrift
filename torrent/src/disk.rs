use std::io::{Seek, Write};

use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    metainfo::MetaInfo,
    piece::{self, Piece},
    types::BitField,
};

pub enum DiskCommand {
    WritePiece(MetaInfo, Piece, Vec<u8>),
    BitField(MetaInfo, oneshot::Sender<BitField>),
    Shutdown,
}

pub struct Disk {
    sender: mpsc::UnboundedSender<DiskCommand>,
    handle: JoinHandle<()>,
}

impl Disk {
    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<DiskCommand>();

        let handle = tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                match command {
                    DiskCommand::Shutdown => break,
                    _ => Disk::handle_command(command),
                }
            }
        });

        Self { sender, handle }
    }

    pub fn write_piece(&self, meta_info: MetaInfo, piece: Piece, data: Vec<u8>) {
        let command = DiskCommand::WritePiece(meta_info, piece, data);
        self.sender.send(command).unwrap();
    }

    pub async fn shutdown(self) {
        self.sender.send(DiskCommand::Shutdown).unwrap();
        self.handle.await.unwrap();
    }

    pub async fn bitfield(self, metainfo: MetaInfo) -> BitField {
        let (tx, rx) = oneshot::channel();

        let command = DiskCommand::BitField(metainfo, tx);
        self.sender.send(command).unwrap();

        rx.await.unwrap()
    }

    fn handle_command(command: DiskCommand) {
        match command {
            DiskCommand::Shutdown => {}
            DiskCommand::WritePiece(meta_info, piece, data) => {
                let filepath = Disk::filepath(&meta_info, piece.index);
                let offset = Disk::offset_of_file(&meta_info, piece.index);
                let full_path = filepath.join("/");

                // Ensure the directory exists
                std::fs::create_dir_all(std::path::Path::new(&full_path).parent().unwrap())
                    .unwrap();

                // Open the file and write the data
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(full_path)
                    .unwrap();

                file.seek(std::io::SeekFrom::Start(offset as u64)).unwrap();
                file.write_all(&data).unwrap();
                file.flush().unwrap();
            }
            DiskCommand::BitField(meta_info, response_tx) => {
                // TODO: read data from disk and get which pieces are available
                let piece_length = meta_info.info.piece_length as usize;
                let total_bytes = meta_info.total_bytes() as usize;
                let piece_size = total_bytes / piece_length;
                let mut bitfield = BitField::repeat(false, piece_size);

                response_tx.send(bitfield).unwrap();
            }
        }
    }

    fn filepath(metainfo: &MetaInfo, piece_index: usize) -> Vec<String> {
        if let Some(_) = metainfo.info.length {
            return vec![metainfo.info.name.clone()];
        }
        if let Some(files) = &metainfo.info.files {
            let mut offset = piece_index as u64 * metainfo.info.piece_length as u64;
            for file in files {
                if offset < file.length {
                    return file.path.clone();
                }
                offset -= file.length;
            }
        }
        panic!("Invalid metainfo, must have length or files");
    }

    fn offset_of_file(metainfo: &MetaInfo, piece_index: usize) -> u32 {
        if let Some(_) = metainfo.info.length {
            return piece_index as u32 * metainfo.info.piece_length;
        }
        if let Some(files) = &metainfo.info.files {
            let mut offset = piece_index as u64 * metainfo.info.piece_length as u64;
            for file in files {
                if offset < file.length {
                    return offset as u32;
                }
                offset -= file.length;
            }
        }
        panic!("Invalid metainfo, must have length or files");
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[tokio::test]
    async fn test_write_piece_command() {
        // Mock MetaInfo and Piece
        let meta_info = MetaInfo {
            announce: "http://example.com/announce".parse().unwrap(),
            info: crate::metainfo::raw::Info {
                name: "test_file".to_string(),
                piece_length: 1024,
                length: Some(2048),
                files: None,
                pieces: vec![0; 20],
                extra: std::collections::BTreeMap::new(),
            },
            comment: None,
            created_by: None,
            creation_date: None,
            info_hash: [0u8; 20],
        };

        let piece = Piece::new_unverified(1, [0u8; 20], 1024); // Changed piece_index to 1

        let data = vec![1, 2, 3, 4, 5];

        Disk::handle_command(DiskCommand::WritePiece(
            meta_info.clone(),
            piece.clone(),
            data.clone(),
        ));

        // Verify the file was created and data was written
        let filepath = Disk::filepath(&meta_info, piece.index);
        let full_path = filepath.join("/");
        let mut file = std::fs::File::open(&full_path).unwrap();
        let offset = Disk::offset_of_file(&meta_info, piece.index);
        file.seek(std::io::SeekFrom::Start(offset as u64)).unwrap();
        let mut buffer = vec![0; data.len()];
        file.read_exact(&mut buffer).unwrap();

        assert_eq!(buffer, data);

        // Clean up the test file
        let _ = std::fs::remove_file(full_path);
    }

    #[tokio::test]
    async fn test_write_piece_command_multiple_files() {
        // Mock MetaInfo with multiple files
        let meta_info = MetaInfo {
            announce: "http://example.com/announce".parse().unwrap(),
            info: crate::metainfo::raw::Info {
                name: "test_torrent".to_string(),
                piece_length: 1024,
                length: None,
                files: Some(vec![
                    crate::metainfo::raw::File {
                        length: 1024,
                        path: vec!["test/file1.txt".to_string()],
                    },
                    crate::metainfo::raw::File {
                        length: 2048,
                        path: vec!["test/file2.txt".to_string()],
                    },
                ]),
                pieces: vec![0; 40],
                extra: std::collections::BTreeMap::new(),
            },
            comment: None,
            created_by: None,
            creation_date: None,
            info_hash: [0u8; 20],
        };

        let piece = Piece::new_unverified(2, [0u8; 20], 1024); // Piece index 2

        let data = vec![6, 7, 8, 9, 10];

        Disk::handle_command(DiskCommand::WritePiece(
            meta_info.clone(),
            piece.clone(),
            data.clone(),
        ));

        // Verify the file was created and data was written
        let filepath = Disk::filepath(&meta_info, piece.index);
        let full_path = filepath.join("/");
        let mut file = std::fs::File::open(&full_path).unwrap();
        let offset = Disk::offset_of_file(&meta_info, piece.index);
        file.seek(std::io::SeekFrom::Start(offset as u64)).unwrap();
        let mut buffer = vec![0; data.len()];
        file.read_exact(&mut buffer).unwrap();

        assert_eq!(buffer, data);

        // Clean up the test files
        let _ = std::fs::remove_dir_all("test");
    }
}
