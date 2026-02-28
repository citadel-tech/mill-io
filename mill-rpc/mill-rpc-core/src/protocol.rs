//! Wire protocol: frame format for Mill-RPC.
//!
//! ```text
//! +--------+--------+-------+--------+-----------+---------+
//! | Magic  | Version| Flags | MsgType| PayloadLen| Payload |
//! | 2B     | 1B     | 1B    | 1B     | 4B (LE)   | N bytes |
//! +--------+--------+-------+--------+-----------+---------+
//! ```
//!
//! Request payload:
//! ```text
//! +------------+-----------+-----------+---------+
//! | RequestID  | ServiceID | MethodID  | Args    |
//! | 8B (LE)    | 2B (LE)   | 2B (LE)   | N bytes |
//! +------------+-----------+-----------+---------+
//! ```

use crate::error::RpcError;
use serde::{Deserialize, Serialize};

/// Magic bytes identifying Mill-RPC frames.
pub const MAGIC: [u8; 2] = [0x4D, 0x52]; // "MR"

/// Current protocol version.
pub const VERSION: u8 = 1;

/// Header size in bytes (magic:2 + version:1 + flags:1 + msg_type:1 + payload_len:4 = 9).
pub const HEADER_SIZE: usize = 9;

/// Maximum payload size (16 MB).
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

/// Message types in the wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    Request = 0x01,
    Response = 0x02,
    Error = 0x03,
    Ping = 0x04,
    Pong = 0x05,
    Cancel = 0x06,
}

impl MessageType {
    pub fn from_u8(v: u8) -> Result<Self, RpcError> {
        match v {
            0x01 => Ok(MessageType::Request),
            0x02 => Ok(MessageType::Response),
            0x03 => Ok(MessageType::Error),
            0x04 => Ok(MessageType::Ping),
            0x05 => Ok(MessageType::Pong),
            0x06 => Ok(MessageType::Cancel),
            _ => Err(RpcError::invalid_argument(format!(
                "Unknown message type: 0x{:02X}",
                v
            ))),
        }
    }
}

/// Bit flags for frame options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(pub u8);

impl Flags {
    pub const NONE: Flags = Flags(0);
    pub const COMPRESSED: Flags = Flags(1 << 0);
    pub const ONE_WAY: Flags = Flags(1 << 1);

    pub fn is_one_way(self) -> bool {
        self.0 & Self::ONE_WAY.0 != 0
    }

    pub fn is_compressed(self) -> bool {
        self.0 & Self::COMPRESSED.0 != 0
    }
}

/// Parsed frame header.
#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub version: u8,
    pub flags: Flags,
    pub message_type: MessageType,
    pub payload_len: u32,
}

impl FrameHeader {
    /// Encode the header into a 9-byte array.
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = MAGIC[0];
        buf[1] = MAGIC[1];
        buf[2] = self.version;
        buf[3] = self.flags.0;
        buf[4] = self.message_type as u8;
        buf[5..9].copy_from_slice(&self.payload_len.to_le_bytes());
        buf
    }

    /// Decode a header from a 9-byte slice.
    pub fn decode(buf: &[u8; HEADER_SIZE]) -> Result<Self, RpcError> {
        if buf[0] != MAGIC[0] || buf[1] != MAGIC[1] {
            return Err(RpcError::invalid_argument(format!(
                "Invalid magic: [{:#04X}, {:#04X}]",
                buf[0], buf[1]
            )));
        }

        let version = buf[2];
        if version != VERSION {
            return Err(RpcError::invalid_argument(format!(
                "Unsupported version: {}",
                version
            )));
        }

        let flags = Flags(buf[3]);
        let message_type = MessageType::from_u8(buf[4])?;
        let payload_len = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);

        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(RpcError::invalid_argument(format!(
                "Payload too large: {} > {}",
                payload_len, MAX_PAYLOAD_SIZE
            )));
        }

        Ok(Self {
            version,
            flags,
            message_type,
            payload_len,
        })
    }
}

/// A complete frame (header + payload).
#[derive(Debug, Clone)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Create a new request frame.
    pub fn request(
        request_id: u64,
        service_id: u16,
        method_id: u16,
        args: Vec<u8>,
        one_way: bool,
    ) -> Self {
        let mut payload = Vec::with_capacity(12 + args.len());
        payload.extend_from_slice(&request_id.to_le_bytes());
        payload.extend_from_slice(&service_id.to_le_bytes());
        payload.extend_from_slice(&method_id.to_le_bytes());
        payload.extend_from_slice(&args);

        let flags = if one_way { Flags::ONE_WAY } else { Flags::NONE };

        Frame {
            header: FrameHeader {
                version: VERSION,
                flags,
                message_type: MessageType::Request,
                payload_len: payload.len() as u32,
            },
            payload,
        }
    }

    /// Create a response frame.
    pub fn response(request_id: u64, data: Vec<u8>) -> Self {
        let mut payload = Vec::with_capacity(8 + data.len());
        payload.extend_from_slice(&request_id.to_le_bytes());
        payload.extend_from_slice(&data);

        Frame {
            header: FrameHeader {
                version: VERSION,
                flags: Flags::NONE,
                message_type: MessageType::Response,
                payload_len: payload.len() as u32,
            },
            payload,
        }
    }

    /// Create an error frame.
    pub fn error(request_id: u64, error_data: Vec<u8>) -> Self {
        let mut payload = Vec::with_capacity(8 + error_data.len());
        payload.extend_from_slice(&request_id.to_le_bytes());
        payload.extend_from_slice(&error_data);

        Frame {
            header: FrameHeader {
                version: VERSION,
                flags: Flags::NONE,
                message_type: MessageType::Error,
                payload_len: payload.len() as u32,
            },
            payload,
        }
    }

    /// Create a ping frame.
    pub fn ping() -> Self {
        Frame {
            header: FrameHeader {
                version: VERSION,
                flags: Flags::NONE,
                message_type: MessageType::Ping,
                payload_len: 0,
            },
            payload: Vec::new(),
        }
    }

    /// Create a pong frame.
    pub fn pong() -> Self {
        Frame {
            header: FrameHeader {
                version: VERSION,
                flags: Flags::NONE,
                message_type: MessageType::Pong,
                payload_len: 0,
            },
            payload: Vec::new(),
        }
    }

    /// Encode the full frame to bytes.
    pub fn encode(&self) -> Vec<u8> {
        let header_bytes = self.header.encode();
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        buf.extend_from_slice(&header_bytes);
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parse request payload fields (request_id, service_id, method_id, args).
    pub fn parse_request_payload(&self) -> Result<(u64, u16, u16, &[u8]), RpcError> {
        if self.payload.len() < 12 {
            return Err(RpcError::invalid_argument("Request payload too short"));
        }
        let request_id = u64::from_le_bytes(self.payload[0..8].try_into().unwrap());
        let service_id = u16::from_le_bytes(self.payload[8..10].try_into().unwrap());
        let method_id = u16::from_le_bytes(self.payload[10..12].try_into().unwrap());
        let args = &self.payload[12..];
        Ok((request_id, service_id, method_id, args))
    }

    /// Parse response payload fields (request_id, data).
    pub fn parse_response_payload(&self) -> Result<(u64, &[u8]), RpcError> {
        if self.payload.len() < 8 {
            return Err(RpcError::invalid_argument("Response payload too short"));
        }
        let request_id = u64::from_le_bytes(self.payload[0..8].try_into().unwrap());
        let data = &self.payload[8..];
        Ok((request_id, data))
    }
}

/// Reads frames from a byte buffer. Returns parsed frames and the number of bytes consumed.
///
/// This is a streaming parser: it handles partial frames by returning only
/// complete frames and leaving remaining bytes unconsumed.
pub fn parse_frames(buf: &[u8]) -> Result<(Vec<Frame>, usize), RpcError> {
    let mut frames = Vec::new();
    let mut offset = 0;

    while offset + HEADER_SIZE <= buf.len() {
        let header_bytes: &[u8; HEADER_SIZE] = buf[offset..offset + HEADER_SIZE]
            .try_into()
            .map_err(|_| RpcError::internal("Header slice conversion failed"))?;

        let header = FrameHeader::decode(header_bytes)?;
        let total_frame_size = HEADER_SIZE + header.payload_len as usize;

        if offset + total_frame_size > buf.len() {
            // Incomplete frame, wait for more data.
            break;
        }

        let payload = buf[offset + HEADER_SIZE..offset + total_frame_size].to_vec();
        frames.push(Frame { header, payload });
        offset += total_frame_size;
    }

    Ok((frames, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = FrameHeader {
            version: VERSION,
            flags: Flags::NONE,
            message_type: MessageType::Request,
            payload_len: 42,
        };
        let encoded = header.encode();
        let decoded = FrameHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.version, VERSION);
        assert_eq!(decoded.flags, Flags::NONE);
        assert_eq!(decoded.message_type, MessageType::Request);
        assert_eq!(decoded.payload_len, 42);
    }

    #[test]
    fn test_request_frame_roundtrip() {
        let frame = Frame::request(123, 1, 2, vec![10, 20, 30], false);
        let bytes = frame.encode();
        let (frames, consumed) = parse_frames(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(frames.len(), 1);

        let (req_id, svc_id, method_id, args) = frames[0].parse_request_payload().unwrap();
        assert_eq!(req_id, 123);
        assert_eq!(svc_id, 1);
        assert_eq!(method_id, 2);
        assert_eq!(args, &[10, 20, 30]);
    }

    #[test]
    fn test_response_frame_roundtrip() {
        let frame = Frame::response(456, vec![1, 2, 3]);
        let bytes = frame.encode();
        let (frames, _) = parse_frames(&bytes).unwrap();
        let (req_id, data) = frames[0].parse_response_payload().unwrap();
        assert_eq!(req_id, 456);
        assert_eq!(data, &[1, 2, 3]);
    }

    #[test]
    fn test_multiple_frames() {
        let f1 = Frame::request(1, 0, 0, vec![0xAA], false);
        let f2 = Frame::response(1, vec![0xBB]);
        let mut bytes = f1.encode();
        bytes.extend_from_slice(&f2.encode());

        let (frames, consumed) = parse_frames(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].header.message_type, MessageType::Request);
        assert_eq!(frames[1].header.message_type, MessageType::Response);
    }

    #[test]
    fn test_partial_frame() {
        let frame = Frame::request(1, 0, 0, vec![0xAA; 100], false);
        let bytes = frame.encode();
        // Only give half the bytes
        let partial = &bytes[..bytes.len() / 2];
        let (frames, consumed) = parse_frames(partial).unwrap();
        assert_eq!(frames.len(), 0);
        assert_eq!(consumed, 0);
    }

    #[test]
    fn test_ping_pong() {
        let ping = Frame::ping();
        let pong = Frame::pong();
        assert_eq!(ping.header.message_type, MessageType::Ping);
        assert_eq!(pong.header.message_type, MessageType::Pong);
        assert_eq!(ping.payload.len(), 0);
        assert_eq!(pong.payload.len(), 0);
    }

    #[test]
    fn test_invalid_magic() {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = 0xFF;
        buf[1] = 0xFF;
        assert!(FrameHeader::decode(&buf).is_err());
    }

    #[test]
    fn test_one_way_flag() {
        let frame = Frame::request(1, 0, 0, vec![], true);
        assert!(frame.header.flags.is_one_way());

        let frame = Frame::request(1, 0, 0, vec![], false);
        assert!(!frame.header.flags.is_one_way());
    }
}
