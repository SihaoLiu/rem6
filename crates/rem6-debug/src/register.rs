use crate::hex::encode_hex_bytes;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GdbRemoteRegisterBytes {
    bytes: Vec<u8>,
}

impl GdbRemoteRegisterBytes {
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn encode_payload(&self) -> Vec<u8> {
        encode_hex_bytes(&self.bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteRegisterValue {
    Bytes(GdbRemoteRegisterBytes),
    Unavailable { byte_len: usize },
}

impl GdbRemoteRegisterValue {
    pub const fn bytes(bytes: GdbRemoteRegisterBytes) -> Self {
        Self::Bytes(bytes)
    }

    pub const fn unavailable(byte_len: usize) -> Self {
        Self::Unavailable { byte_len }
    }

    pub(crate) fn encode_payload(&self) -> Vec<u8> {
        match self {
            Self::Bytes(bytes) => bytes.encode_payload(),
            Self::Unavailable { byte_len } => vec![b'x'; byte_len * 2],
        }
    }
}

impl From<GdbRemoteRegisterBytes> for GdbRemoteRegisterValue {
    fn from(bytes: GdbRemoteRegisterBytes) -> Self {
        Self::Bytes(bytes)
    }
}
