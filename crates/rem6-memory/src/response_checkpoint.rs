use crate::{
    AgentId, MemoryError, MemoryRequestId, MemoryResponse, MemoryResponseSnapshot, ResponseStatus,
};

const RESPONSE_CHECKPOINT_MAGIC: [u8; 4] = *b"MRES";
const RESPONSE_CHECKPOINT_VERSION: u32 = 1;
const RESPONSE_CHECKPOINT_HEADER_SIZE: usize = 40;

const FLAG_DATA_PRESENT: u32 = 1 << 0;
const KNOWN_FLAGS: u32 = FLAG_DATA_PRESENT;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryResponseCheckpointPayload {
    snapshot: MemoryResponseSnapshot,
}

impl MemoryResponseCheckpointPayload {
    pub fn from_response(response: &MemoryResponse) -> Self {
        Self {
            snapshot: response.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: MemoryResponseSnapshot) -> Result<Self, MemoryError> {
        MemoryResponse::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, MemoryError> {
        if payload.len() < RESPONSE_CHECKPOINT_HEADER_SIZE {
            return Err(MemoryError::InvalidResponseCheckpointPayloadSize {
                expected: RESPONSE_CHECKPOINT_HEADER_SIZE,
                actual: payload.len(),
            });
        }
        if payload[..4] != RESPONSE_CHECKPOINT_MAGIC {
            return Err(MemoryError::InvalidResponseCheckpointMagic);
        }

        let mut offset = 4;
        let version = read_u32(payload, &mut offset)?;
        if version != RESPONSE_CHECKPOINT_VERSION {
            return Err(MemoryError::UnsupportedResponseCheckpointVersion { version });
        }

        let status = decode_status(read_u32(payload, &mut offset)?)?;
        let flags = read_u32(payload, &mut offset)?;
        if flags & !KNOWN_FLAGS != 0 {
            return Err(MemoryError::InvalidResponseCheckpointFlags { flags });
        }
        let agent = read_u32(payload, &mut offset)?;
        let reserved = read_u32(payload, &mut offset)?;
        if reserved != 0 {
            return Err(MemoryError::InvalidResponseCheckpointReserved { value: reserved });
        }
        let sequence = read_u64(payload, &mut offset)?;
        let data_len = read_u64(payload, &mut offset)?;
        let data_len_usize = response_checkpoint_usize(data_len)?;
        let expected = response_checkpoint_payload_size(data_len_usize)?;
        if payload.len() != expected {
            return Err(MemoryError::InvalidResponseCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let data = read_optional_data(payload, &mut offset, flags, data_len_usize, data_len)?;
        let snapshot = MemoryResponseSnapshot::new(
            MemoryRequestId::new(AgentId::new(agent), sequence),
            status,
            data,
        )?;
        Ok(Self { snapshot })
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("memory-response checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, MemoryError> {
        let data_len = self.snapshot.data().map(<[u8]>::len).unwrap_or(0);
        let mut payload = Vec::with_capacity(response_checkpoint_payload_size(data_len)?);
        payload.extend_from_slice(&RESPONSE_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&RESPONSE_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&encode_status(self.snapshot.status()).to_le_bytes());
        payload.extend_from_slice(&encode_flags(&self.snapshot).to_le_bytes());
        payload.extend_from_slice(&self.snapshot.request_id().agent().get().to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.request_id().sequence().to_le_bytes());
        payload.extend_from_slice(
            &encode_response_checkpoint_u64("data length", data_len)?.to_le_bytes(),
        );
        if let Some(data) = self.snapshot.data() {
            payload.extend_from_slice(data);
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &MemoryResponseSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> MemoryResponseSnapshot {
        self.snapshot
    }
}

fn encode_flags(snapshot: &MemoryResponseSnapshot) -> u32 {
    if snapshot.data().is_some() {
        FLAG_DATA_PRESENT
    } else {
        0
    }
}

fn read_optional_data(
    payload: &[u8],
    offset: &mut usize,
    flags: u32,
    data_len_usize: usize,
    data_len: u64,
) -> Result<Option<Vec<u8>>, MemoryError> {
    if flags & FLAG_DATA_PRESENT == 0 {
        if data_len != 0 {
            return Err(MemoryError::InvalidResponseCheckpointDataLength { length: data_len });
        }
        return Ok(None);
    }

    Ok(Some(read_exact(payload, offset, data_len_usize)?.to_vec()))
}

fn encode_status(status: ResponseStatus) -> u32 {
    match status {
        ResponseStatus::Completed => 0,
        ResponseStatus::Retry => 1,
    }
}

fn decode_status(code: u32) -> Result<ResponseStatus, MemoryError> {
    match code {
        0 => Ok(ResponseStatus::Completed),
        1 => Ok(ResponseStatus::Retry),
        code => Err(MemoryError::InvalidResponseCheckpointStatus { code }),
    }
}

fn response_checkpoint_usize(value: u64) -> Result<usize, MemoryError> {
    value
        .try_into()
        .map_err(|_| MemoryError::InvalidResponseCheckpointUsize { value })
}

fn response_checkpoint_payload_size(data_len: usize) -> Result<usize, MemoryError> {
    RESPONSE_CHECKPOINT_HEADER_SIZE.checked_add(data_len).ok_or(
        MemoryError::InvalidResponseCheckpointPayloadSize {
            expected: RESPONSE_CHECKPOINT_HEADER_SIZE,
            actual: usize::MAX,
        },
    )
}

fn encode_response_checkpoint_u64(field: &'static str, value: usize) -> Result<u64, MemoryError> {
    value
        .try_into()
        .map_err(|_| MemoryError::ResponseCheckpointValueTooLarge {
            field,
            value,
            maximum: u64::MAX as usize,
        })
}

fn read_exact<'a>(
    payload: &'a [u8],
    offset: &mut usize,
    len: usize,
) -> Result<&'a [u8], MemoryError> {
    let end = offset
        .checked_add(len)
        .ok_or(MemoryError::InvalidResponseCheckpointPayloadSize {
            expected: RESPONSE_CHECKPOINT_HEADER_SIZE,
            actual: payload.len(),
        })?;
    let bytes =
        payload
            .get(*offset..end)
            .ok_or(MemoryError::InvalidResponseCheckpointPayloadSize {
                expected: end,
                actual: payload.len(),
            })?;
    *offset = end;
    Ok(bytes)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, MemoryError> {
    let bytes = read_exact(payload, offset, 4)?;
    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .expect("checkpoint u32 slice width is fixed"),
    ))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, MemoryError> {
    let bytes = read_exact(payload, offset, 8)?;
    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .expect("checkpoint u64 slice width is fixed"),
    ))
}
