use crate::error::RpcError;
use serde::{de::DeserializeOwned, Serialize};

/// Supported codec types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecType {
    Bincode,
}

/// Codec for serializing/deserializing RPC payloads.
#[derive(Debug, Clone)]
pub struct Codec {
    codec_type: CodecType,
}

impl Codec {
    pub fn bincode() -> Self {
        Self {
            codec_type: CodecType::Bincode,
        }
    }

    pub fn codec_type(&self) -> CodecType {
        self.codec_type
    }

    /// Serialize a value to bytes.
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, RpcError> {
        match self.codec_type {
            CodecType::Bincode => bincode::serialize(value)
                .map_err(|e| RpcError::codec_error(format!("serialize: {}", e))),
        }
    }

    /// Deserialize bytes to a value.
    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T, RpcError> {
        match self.codec_type {
            CodecType::Bincode => bincode::deserialize(data)
                .map_err(|e| RpcError::codec_error(format!("deserialize: {}", e))),
        }
    }
}

impl Default for Codec {
    fn default() -> Self {
        Self::bincode()
    }
}
