use std::io;

use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::types::{BitField, PeerId, Sha1Hash};

const PROTOCOL_STRING: &[u8] = b"BitTorrent protocol";

pub struct HandShake {
    pub info_hash: Sha1Hash,
    pub peer_id: PeerId,
}

// https://www.bittorrent.org/beps/bep_0003.html#peer-protocol
impl HandShake {
    pub fn new(info_hash: Sha1Hash, peer_id: PeerId) -> Self {
        Self { info_hash, peer_id }
    }
}

pub struct HandShakeCodec;

impl Encoder<HandShake> for HandShakeCodec {
    type Error = io::Error;

    fn encode(&mut self, item: HandShake, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(68);
        dst.put_u8(19u8);
        dst.extend_from_slice(PROTOCOL_STRING);
        dst.extend_from_slice(&[0u8; 8]); // reserved bytes
        dst.extend_from_slice(&item.info_hash);
        dst.extend_from_slice(&item.peer_id);
        Ok(())
    }
}

impl Decoder for HandShakeCodec {
    type Error = io::Error;
    type Item = HandShake;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 68 {
            return Ok(None); // Not enough data for a full handshake
        }

        let protocol_length = src.get_u8() as usize;
        if protocol_length != 19 || !src.starts_with(PROTOCOL_STRING) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid protocol",
            ));
        }

        src.advance(8); // Skip reserved bytes
        let mut info_hash: Sha1Hash = [0; 20];
        src.copy_to_slice(info_hash.as_mut());
        let mut peer_id: PeerId = [0; 20];
        src.copy_to_slice(peer_id.as_mut());

        Ok(Some(HandShake::new(info_hash, peer_id)))
    }
}

// All messages is length-prefixed messages
// According the document, All integers sent in the protocol are encoded as four bytes big-endian, which is u32.
// https://www.bittorrent.org/beps/bep_0003.html#peer-protocol
trait MessageEncodable {
    fn message_id(&self) -> Option<MessageId>;
    fn message_length(&self) -> usize;
    fn payload(&self) -> Option<Vec<u8>>;
}

#[repr(u8)]
#[derive(Debug)]
pub enum MessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl TryFrom<u8> for MessageId {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageId::Choke),
            1 => Ok(MessageId::Unchoke),
            2 => Ok(MessageId::Interested),
            3 => Ok(MessageId::NotInterested),
            4 => Ok(MessageId::Have),
            5 => Ok(MessageId::Bitfield),
            6 => Ok(MessageId::Request),
            7 => Ok(MessageId::Piece),
            8 => Ok(MessageId::Cancel),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unknown message ID",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have {
        piece_index: u32,
    },
    Bitfield {
        bitfield: BitField,
    },
    Request {
        piece_index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        piece_index: u32,
        begin: u32,
        piece: Vec<u8>,
    },
    Cancel {
        piece_index: u32,
        begin: u32,
        length: u32,
    },
}

impl Message {
    pub fn message_length(&self) -> usize {
        match self {
            Message::KeepAlive => 0,
            Message::Choke | Message::Unchoke | Message::Interested | Message::NotInterested => 1,
            // 1 byte for ID + 4 bytes for piece index
            Message::Have { .. } => 5,
            // 1 byte for ID + length of bitfield
            Message::Bitfield { bitfield } => 1 + bitfield.len(),
            // 1 byte for ID + 4 bytes for piece index + 4 bytes for begin + 4 bytes for length
            Message::Request { .. } => 13,
            // 1 byte for ID + 4 bytes for piece index + 4 bytes for begin + length of piece
            Message::Piece { piece, .. } => 9 + piece.len(),
            // 1 byte for ID + 4 bytes for piece index + 4 bytes for begin + 4 bytes for length
            Message::Cancel { .. } => 13,
        }
    }

    pub fn message_id(&self) -> Option<MessageId> {
        match self {
            Message::KeepAlive => None,
            Message::Choke => Some(MessageId::Choke),
            Message::Unchoke => Some(MessageId::Unchoke),
            Message::Interested => Some(MessageId::Interested),
            Message::NotInterested => Some(MessageId::NotInterested),
            Message::Have { .. } => Some(MessageId::Have),
            Message::Bitfield { .. } => Some(MessageId::Bitfield),
            Message::Request { .. } => Some(MessageId::Request),
            Message::Piece { .. } => Some(MessageId::Piece),
            Message::Cancel { .. } => Some(MessageId::Cancel),
        }
    }

    pub fn payload(&self) -> Option<Vec<u8>> {
        match self {
            Message::KeepAlive => None,
            Message::Choke | Message::Unchoke | Message::Interested | Message::NotInterested => {
                None
            }
            Message::Have { piece_index } => Some(piece_index.to_be_bytes().to_vec()),
            Message::Bitfield { bitfield } => {
                // bitfield.len() is the number of bits, we need to convert it to bytes
                let mut buffer = Vec::with_capacity(bitfield.len() / 8);
                buffer.extend_from_slice(bitfield.as_raw_slice());
                Some(buffer)
            }
            Message::Request {
                piece_index,
                begin,
                length,
            } => {
                let mut buffer = Vec::with_capacity(13);
                buffer.extend_from_slice(&piece_index.to_be_bytes());
                buffer.extend_from_slice(&begin.to_be_bytes());
                buffer.extend_from_slice(&length.to_be_bytes());
                Some(buffer)
            }
            Message::Piece {
                piece_index,
                begin,
                piece,
            } => {
                let mut buffer = Vec::with_capacity(9 + piece.len());
                buffer.extend_from_slice(&piece_index.to_be_bytes());
                buffer.extend_from_slice(&begin.to_be_bytes());
                buffer.extend_from_slice(piece);
                Some(buffer)
            }
            Message::Cancel {
                piece_index,
                begin,
                length,
            } => {
                let mut buffer = Vec::with_capacity(13);
                buffer.extend_from_slice(&piece_index.to_be_bytes());
                buffer.extend_from_slice(&begin.to_be_bytes());
                buffer.extend_from_slice(&length.to_be_bytes());
                Some(buffer)
            }
        }
    }
}

pub struct MessageCodec;

impl Encoder<Message> for MessageCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let message_length = item.message_length();
        dst.reserve(4 + message_length); // 4 bytes for length prefix + message length
        dst.put_u32(message_length as u32); // Write the length prefix
        if let Some(id) = item.message_id() {
            dst.put_u8(id as u8); // Write the message ID
        }
        if let Some(payload) = item.payload() {
            dst.extend_from_slice(&payload); // Write the payload
        }
        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Error = io::Error;
    type Item = Message;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None); // Not enough data for a length prefix
        }

        // length include the message ID and payload
        let length = (&src[..4]).get_u32() as usize;
        if src.len() < 4 + length {
            return Ok(None); // Not enough data for the full message
        }
        src.advance(4); // Advance past the length prefix

        if length == 0 {
            return Ok(Some(Message::KeepAlive));
        }
        let message_id = MessageId::try_from(src.get_u8())?;
        match message_id {
            MessageId::Choke => Ok(Some(Message::Choke)),
            MessageId::Unchoke => Ok(Some(Message::Unchoke)),
            MessageId::Interested => Ok(Some(Message::Interested)),
            MessageId::NotInterested => Ok(Some(Message::NotInterested)),
            MessageId::Have => {
                let piece_index = src.get_u32();
                Ok(Some(Message::Have { piece_index }))
            }
            MessageId::Bitfield => {
                // bitfield length = length - 1 (1 byte for the message ID)
                let bitfield = src.split_to(length - 1).to_vec();
                Ok(Some(Message::Bitfield {
                    bitfield: BitField::from_vec(bitfield),
                }))
            }
            MessageId::Request => {
                let piece_index = src.get_u32();
                let begin = src.get_u32();
                let length = src.get_u32();
                Ok(Some(Message::Request {
                    piece_index,
                    begin,
                    length,
                }))
            }
            MessageId::Piece => {
                let piece_index = src.get_u32();
                let begin = src.get_u32();
                let piece = src.split_to(length - 9).to_vec(); // 9 bytes for message_id, piece_index and begin
                Ok(Some(Message::Piece {
                    piece_index,
                    begin,
                    piece,
                }))
            }
            MessageId::Cancel => {
                let piece_index = src.get_u32();
                let begin = src.get_u32();
                let length = src.get_u32();
                Ok(Some(Message::Cancel {
                    piece_index,
                    begin,
                    length,
                }))
            }
        }
    }
}
