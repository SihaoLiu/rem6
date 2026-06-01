use crate::{
    Address, CacheLineLayout, LineMemorySnapshot, LineMemoryStore, MemoryError, MemoryLineSnapshot,
};

const LINE_CHECKPOINT_MAGIC: [u8; 4] = *b"MLIN";
const LINE_CHECKPOINT_VERSION: u32 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const LINE_CHECKPOINT_HEADER_BYTES: usize =
    LINE_CHECKPOINT_MAGIC.len() + U32_BYTES + U64_BYTES + U32_BYTES + U32_BYTES;
const LINE_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineMemoryCheckpointPayload {
    snapshot: LineMemorySnapshot,
}

impl LineMemoryCheckpointPayload {
    pub fn from_store(store: &LineMemoryStore) -> Self {
        Self {
            snapshot: store.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: LineMemorySnapshot) -> Result<Self, MemoryError> {
        LineMemoryStore::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, MemoryError> {
        if payload.len() < LINE_CHECKPOINT_HEADER_BYTES {
            return Err(MemoryError::InvalidLineCheckpointPayloadSize {
                expected: LINE_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..LINE_CHECKPOINT_MAGIC.len()] != LINE_CHECKPOINT_MAGIC {
            return Err(MemoryError::InvalidLineCheckpointMagic);
        }

        let mut offset = LINE_CHECKPOINT_MAGIC.len();
        let version = read_u32(payload, &mut offset);
        if version != LINE_CHECKPOINT_VERSION {
            return Err(MemoryError::UnsupportedLineCheckpointVersion { version });
        }

        let layout = CacheLineLayout::new(read_u64(payload, &mut offset))?;
        let line_bytes = line_checkpoint_usize(layout.bytes())?;
        let line_count = read_u32(payload, &mut offset) as usize;
        let reserved = read_u32(payload, &mut offset);
        if reserved != 0 {
            return Err(MemoryError::InvalidLineCheckpointReserved { value: reserved });
        }
        let expected = line_checkpoint_payload_size(line_bytes, line_count)?;
        if payload.len() != expected {
            return Err(MemoryError::InvalidLineCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let mut lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            let line = Address::new(read_u64(payload, &mut offset));
            let data = payload[offset..offset + line_bytes].to_vec();
            offset += line_bytes;
            lines.push(MemoryLineSnapshot::new(line, data));
        }

        Self::from_snapshot(LineMemorySnapshot::new(layout, lines))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("line-memory checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, MemoryError> {
        let line_bytes = line_checkpoint_usize(self.snapshot.layout().bytes())?;
        let line_count = encode_line_checkpoint_u32("line count", self.snapshot.lines().len())?;
        let mut payload = Vec::with_capacity(line_checkpoint_payload_size(
            line_bytes,
            self.snapshot.lines().len(),
        )?);
        payload.extend_from_slice(&LINE_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&LINE_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.layout().bytes().to_le_bytes());
        payload.extend_from_slice(&line_count.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        for line in self.snapshot.lines() {
            payload.extend_from_slice(&line.line().get().to_le_bytes());
            payload.extend_from_slice(line.data());
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &LineMemorySnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> LineMemorySnapshot {
        self.snapshot
    }
}

fn line_checkpoint_usize(value: u64) -> Result<usize, MemoryError> {
    usize::try_from(value).map_err(|_| MemoryError::InvalidLineCheckpointLineSize { value })
}

fn line_checkpoint_payload_size(
    line_bytes: usize,
    line_count: usize,
) -> Result<usize, MemoryError> {
    let record_bytes =
        U64_BYTES
            .checked_add(line_bytes)
            .ok_or(MemoryError::InvalidLineCheckpointPayloadSize {
                expected: usize::MAX,
                actual: 0,
            })?;
    let line_payload_bytes = line_count.checked_mul(record_bytes).ok_or(
        MemoryError::InvalidLineCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        },
    )?;
    LINE_CHECKPOINT_HEADER_BYTES
        .checked_add(line_payload_bytes)
        .ok_or(MemoryError::InvalidLineCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        })
}

fn encode_line_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, MemoryError> {
    u32::try_from(value).map_err(|_| MemoryError::LineCheckpointValueTooLarge {
        field,
        value,
        maximum: LINE_CHECKPOINT_U32_MAX,
    })
}

fn read_u32(payload: &[u8], offset: &mut usize) -> u32 {
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    u32::from_le_bytes(bytes)
}

fn read_u64(payload: &[u8], offset: &mut usize) -> u64 {
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    u64::from_le_bytes(bytes)
}
