use crate::hex::encode_hex_nibble;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteStopReply {
    Signal { signal: u8 },
}

impl GdbRemoteStopReply {
    pub const fn signal(signal: u8) -> Self {
        Self::Signal { signal }
    }

    pub(crate) fn encode_payload(&self) -> Vec<u8> {
        match self {
            Self::Signal { signal } => {
                vec![
                    b'S',
                    encode_hex_nibble(signal >> 4),
                    encode_hex_nibble(signal & 0x0f),
                ]
            }
        }
    }
}
