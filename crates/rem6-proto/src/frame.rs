use crate::{DependencyTrace, ProtoError, ProtoTrace};

const FRAME_MAGIC: &[u8; 4] = b"RM6P";
const FRAME_VERSION: u16 = 1;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const FIXED_HEADER_BYTES: usize = 4 + 2 + 1 + 2 + 8;
const CHECKSUM_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TraceFrameKind {
    InstructionPacketTrace,
    DependencyTrace,
}

impl TraceFrameKind {
    const fn code(self) -> u8 {
        match self {
            Self::InstructionPacketTrace => 1,
            Self::DependencyTrace => 2,
        }
    }

    const fn from_code(code: u8) -> Result<Self, ProtoError> {
        match code {
            1 => Ok(Self::InstructionPacketTrace),
            2 => Ok(Self::DependencyTrace),
            kind => Err(ProtoError::UnknownFrameKind { kind }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrame {
    kind: TraceFrameKind,
    version: u16,
    identity: String,
    payload: Vec<u8>,
}

impl TraceFrame {
    pub fn new(
        kind: TraceFrameKind,
        identity: impl Into<String>,
        payload: Vec<u8>,
    ) -> Result<Self, ProtoError> {
        let identity = identity.into();
        if identity.is_empty() {
            return Err(ProtoError::EmptyFrameIdentity);
        }
        if identity.len() > usize::from(u16::MAX) {
            return Err(ProtoError::FrameIdentityTooLong {
                bytes: identity.len(),
            });
        }
        if payload.is_empty() {
            return Err(ProtoError::EmptyFramePayload);
        }
        Ok(Self {
            kind,
            version: FRAME_VERSION,
            identity,
            payload,
        })
    }

    pub fn from_proto_trace(trace: &ProtoTrace, payload: Vec<u8>) -> Result<Self, ProtoError> {
        Self::new(
            TraceFrameKind::InstructionPacketTrace,
            trace.identity().as_str(),
            payload,
        )
    }

    pub fn from_dependency_trace(
        trace: &DependencyTrace,
        payload: Vec<u8>,
    ) -> Result<Self, ProtoError> {
        Self::new(
            TraceFrameKind::DependencyTrace,
            trace.identity().as_str(),
            payload,
        )
    }

    pub const fn kind(&self) -> TraceFrameKind {
        self.kind
    }

    pub const fn version(&self) -> u16 {
        self.version
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn encode(&self) -> Vec<u8> {
        let identity = self.identity.as_bytes();
        let mut bytes = Vec::with_capacity(
            FIXED_HEADER_BYTES + identity.len() + self.payload.len() + CHECKSUM_BYTES,
        );
        bytes.extend_from_slice(FRAME_MAGIC);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(self.kind.code());
        bytes.extend_from_slice(&(identity.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(self.payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(identity);
        bytes.extend_from_slice(&self.payload);
        let checksum = checksum(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtoError> {
        if bytes.len() < FIXED_HEADER_BYTES + CHECKSUM_BYTES {
            return Err(ProtoError::TruncatedFrame);
        }
        if &bytes[..4] != FRAME_MAGIC {
            return Err(ProtoError::InvalidFrameMagic);
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != FRAME_VERSION {
            return Err(ProtoError::UnsupportedFrameVersion { version });
        }
        let kind = TraceFrameKind::from_code(bytes[6])?;
        let identity_len = u16::from_le_bytes([bytes[7], bytes[8]]) as usize;
        let payload_len = u64::from_le_bytes(
            bytes[9..17]
                .try_into()
                .map_err(|_| ProtoError::TruncatedFrame)?,
        ) as usize;

        let content_len = FIXED_HEADER_BYTES
            .checked_add(identity_len)
            .and_then(|len| len.checked_add(payload_len))
            .ok_or(ProtoError::TruncatedFrame)?;
        let total_len = content_len
            .checked_add(CHECKSUM_BYTES)
            .ok_or(ProtoError::TruncatedFrame)?;
        if bytes.len() != total_len {
            return Err(ProtoError::TruncatedFrame);
        }

        let expected_checksum = u64::from_le_bytes(
            bytes[content_len..total_len]
                .try_into()
                .map_err(|_| ProtoError::TruncatedFrame)?,
        );
        if checksum(&bytes[..content_len]) != expected_checksum {
            return Err(ProtoError::FrameChecksumMismatch);
        }

        let identity_start = FIXED_HEADER_BYTES;
        let identity_end = identity_start + identity_len;
        let payload_end = identity_end + payload_len;
        let identity = std::str::from_utf8(&bytes[identity_start..identity_end])
            .map_err(|_| ProtoError::InvalidFrameIdentity)?
            .to_string();
        let payload = bytes[identity_end..payload_end].to_vec();
        Self::new(kind, identity, payload)
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
