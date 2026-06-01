use crate::{
    AccessSize, Address, ByteMask, CacheLineLayout, MemoryAccessOrdering, MemoryAtomicOp,
    MemoryBarrierSet, MemoryError, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryRequestSnapshot,
};

const REQUEST_CHECKPOINT_MAGIC: [u8; 4] = *b"MREQ";
const REQUEST_CHECKPOINT_VERSION: u32 = 1;
const REQUEST_CHECKPOINT_HEADER_SIZE: usize = 80;

const FLAG_DATA_PRESENT: u32 = 1 << 0;
const FLAG_MASK_PRESENT: u32 = 1 << 1;
const FLAG_ATOMIC_PRESENT: u32 = 1 << 2;
const FLAG_UNCACHEABLE: u32 = 1 << 3;
const FLAG_STRICT_ORDER: u32 = 1 << 4;
const FLAG_BEFORE_READ: u32 = 1 << 5;
const FLAG_BEFORE_WRITE: u32 = 1 << 6;
const FLAG_AFTER_READ: u32 = 1 << 7;
const FLAG_AFTER_WRITE: u32 = 1 << 8;
const FLAG_BEFORE_PRESENT: u32 = 1 << 9;
const FLAG_AFTER_PRESENT: u32 = 1 << 10;
const KNOWN_FLAGS: u32 = FLAG_DATA_PRESENT
    | FLAG_MASK_PRESENT
    | FLAG_ATOMIC_PRESENT
    | FLAG_UNCACHEABLE
    | FLAG_STRICT_ORDER
    | FLAG_BEFORE_READ
    | FLAG_BEFORE_WRITE
    | FLAG_AFTER_READ
    | FLAG_AFTER_WRITE
    | FLAG_BEFORE_PRESENT
    | FLAG_AFTER_PRESENT;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRequestCheckpointPayload {
    snapshot: MemoryRequestSnapshot,
}

impl MemoryRequestCheckpointPayload {
    pub fn from_request(request: &MemoryRequest) -> Self {
        Self {
            snapshot: request.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: MemoryRequestSnapshot) -> Result<Self, MemoryError> {
        MemoryRequest::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, MemoryError> {
        if payload.len() < REQUEST_CHECKPOINT_HEADER_SIZE {
            return Err(MemoryError::InvalidRequestCheckpointPayloadSize {
                expected: REQUEST_CHECKPOINT_HEADER_SIZE,
                actual: payload.len(),
            });
        }
        if payload[..4] != REQUEST_CHECKPOINT_MAGIC {
            return Err(MemoryError::InvalidRequestCheckpointMagic);
        }

        let mut offset = 4;
        let version = read_u32(payload, &mut offset)?;
        if version != REQUEST_CHECKPOINT_VERSION {
            return Err(MemoryError::UnsupportedRequestCheckpointVersion { version });
        }

        let operation = decode_operation(read_u32(payload, &mut offset)?)?;
        let flags = read_u32(payload, &mut offset)?;
        if flags & !KNOWN_FLAGS != 0 {
            return Err(MemoryError::InvalidRequestCheckpointFlags { flags });
        }
        let agent = read_u32(payload, &mut offset)?;
        let reserved = read_u32(payload, &mut offset)?;
        if reserved != 0 {
            return Err(MemoryError::InvalidRequestCheckpointReserved { value: reserved });
        }

        let sequence = read_u64(payload, &mut offset)?;
        let address = Address::new(read_u64(payload, &mut offset)?);
        let size = AccessSize::new(read_u64(payload, &mut offset)?)?;
        let line_layout = CacheLineLayout::new(read_u64(payload, &mut offset)?)?;
        let data_len = read_u64(payload, &mut offset)?;
        let mask_len = read_u64(payload, &mut offset)?;
        let atomic_code = read_u32(payload, &mut offset)?;
        let reserved = read_u32(payload, &mut offset)?;
        if reserved != 0 {
            return Err(MemoryError::InvalidRequestCheckpointReserved { value: reserved });
        }

        let data_len_usize = request_checkpoint_usize(data_len)?;
        let mask_len_usize = request_checkpoint_usize(mask_len)?;
        let expected = request_checkpoint_payload_size(data_len_usize, mask_len_usize)?;
        if payload.len() != expected {
            return Err(MemoryError::InvalidRequestCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let data = read_optional_data(payload, &mut offset, flags, data_len_usize, data_len)?;
        let byte_mask = read_optional_mask(payload, &mut offset, flags, mask_len_usize, mask_len)?;
        let atomic_op = decode_optional_atomic_op(flags, atomic_code)?;
        let ordering = decode_ordering(flags)?;
        let snapshot = MemoryRequestSnapshot::new(
            MemoryRequestId::new(crate::AgentId::new(agent), sequence),
            operation,
            address,
            size,
            line_layout,
            ordering,
            flags & FLAG_UNCACHEABLE != 0,
            flags & FLAG_STRICT_ORDER != 0,
            data,
            byte_mask,
            atomic_op,
        )?;

        Ok(Self { snapshot })
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("memory-request checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, MemoryError> {
        let data_len = self.snapshot.data().map(<[u8]>::len).unwrap_or(0);
        let mask_len = self
            .snapshot
            .byte_mask()
            .map(|mask| mask.bits().len())
            .unwrap_or(0);
        let mut payload = Vec::with_capacity(request_checkpoint_payload_size(data_len, mask_len)?);
        payload.extend_from_slice(&REQUEST_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&REQUEST_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&encode_operation(self.snapshot.operation()).to_le_bytes());
        payload.extend_from_slice(&encode_flags(&self.snapshot).to_le_bytes());
        payload.extend_from_slice(&self.snapshot.id().agent().get().to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.id().sequence().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.range().start().get().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.range().size().bytes().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.line_layout().bytes().to_le_bytes());
        payload.extend_from_slice(
            &encode_request_checkpoint_u64("data length", data_len)?.to_le_bytes(),
        );
        payload.extend_from_slice(
            &encode_request_checkpoint_u64("mask length", mask_len)?.to_le_bytes(),
        );
        payload
            .extend_from_slice(&encode_optional_atomic_op(self.snapshot.atomic_op()).to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        if let Some(data) = self.snapshot.data() {
            payload.extend_from_slice(data);
        }
        if let Some(mask) = self.snapshot.byte_mask() {
            for bit in mask.bits() {
                payload.push(u8::from(*bit));
            }
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &MemoryRequestSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> MemoryRequestSnapshot {
        self.snapshot
    }
}

fn encode_flags(snapshot: &MemoryRequestSnapshot) -> u32 {
    let mut flags = 0;
    if snapshot.data().is_some() {
        flags |= FLAG_DATA_PRESENT;
    }
    if snapshot.byte_mask().is_some() {
        flags |= FLAG_MASK_PRESENT;
    }
    if snapshot.atomic_op().is_some() {
        flags |= FLAG_ATOMIC_PRESENT;
    }
    if snapshot.is_uncacheable() {
        flags |= FLAG_UNCACHEABLE;
    }
    if snapshot.is_strict_ordered() {
        flags |= FLAG_STRICT_ORDER;
    }
    if let Some(before) = snapshot.ordering().before() {
        flags |= FLAG_BEFORE_PRESENT;
        if before.read() {
            flags |= FLAG_BEFORE_READ;
        }
        if before.write() {
            flags |= FLAG_BEFORE_WRITE;
        }
    }
    if let Some(after) = snapshot.ordering().after() {
        flags |= FLAG_AFTER_PRESENT;
        if after.read() {
            flags |= FLAG_AFTER_READ;
        }
        if after.write() {
            flags |= FLAG_AFTER_WRITE;
        }
    }
    flags
}

fn decode_ordering(flags: u32) -> Result<MemoryAccessOrdering, MemoryError> {
    let before = decode_barrier(
        flags,
        FLAG_BEFORE_PRESENT,
        FLAG_BEFORE_READ,
        FLAG_BEFORE_WRITE,
    )?;
    let after = decode_barrier(flags, FLAG_AFTER_PRESENT, FLAG_AFTER_READ, FLAG_AFTER_WRITE)?;
    Ok(MemoryAccessOrdering::new(before, after))
}

fn decode_barrier(
    flags: u32,
    present_flag: u32,
    read_flag: u32,
    write_flag: u32,
) -> Result<Option<MemoryBarrierSet>, MemoryError> {
    let present = flags & present_flag != 0;
    let read = flags & read_flag != 0;
    let write = flags & write_flag != 0;
    if !present && (read || write) {
        return Err(MemoryError::InvalidRequestCheckpointFlags { flags });
    }
    Ok(present.then_some(MemoryBarrierSet::new(read, write)))
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
            return Err(MemoryError::InvalidRequestCheckpointDataLength { length: data_len });
        }
        return Ok(None);
    }
    Ok(Some(read_exact(payload, offset, data_len_usize)?.to_vec()))
}

fn read_optional_mask(
    payload: &[u8],
    offset: &mut usize,
    flags: u32,
    mask_len_usize: usize,
    mask_len: u64,
) -> Result<Option<ByteMask>, MemoryError> {
    if flags & FLAG_MASK_PRESENT == 0 {
        if mask_len != 0 {
            return Err(MemoryError::InvalidRequestCheckpointMaskLength { length: mask_len });
        }
        return Ok(None);
    }

    let bytes = read_exact(payload, offset, mask_len_usize)?;
    let mut bits = Vec::new();
    for byte in bytes {
        match *byte {
            0 => bits.push(false),
            1 => bits.push(true),
            value => return Err(MemoryError::InvalidRequestCheckpointMaskBit { value }),
        }
    }
    ByteMask::from_bits(bits).map(Some)
}

fn encode_operation(operation: MemoryOperation) -> u32 {
    match operation {
        MemoryOperation::InstructionFetch => 0,
        MemoryOperation::ReadShared => 1,
        MemoryOperation::ReadUnique => 2,
        MemoryOperation::Write => 3,
        MemoryOperation::Upgrade => 4,
        MemoryOperation::WritebackClean => 5,
        MemoryOperation::WritebackDirty => 6,
        MemoryOperation::Atomic => 7,
        MemoryOperation::PrefetchRead => 8,
        MemoryOperation::PrefetchWrite => 9,
        MemoryOperation::CleanEvict => 10,
        MemoryOperation::Invalidate => 11,
    }
}

fn decode_operation(code: u32) -> Result<MemoryOperation, MemoryError> {
    match code {
        0 => Ok(MemoryOperation::InstructionFetch),
        1 => Ok(MemoryOperation::ReadShared),
        2 => Ok(MemoryOperation::ReadUnique),
        3 => Ok(MemoryOperation::Write),
        4 => Ok(MemoryOperation::Upgrade),
        5 => Ok(MemoryOperation::WritebackClean),
        6 => Ok(MemoryOperation::WritebackDirty),
        7 => Ok(MemoryOperation::Atomic),
        8 => Ok(MemoryOperation::PrefetchRead),
        9 => Ok(MemoryOperation::PrefetchWrite),
        10 => Ok(MemoryOperation::CleanEvict),
        11 => Ok(MemoryOperation::Invalidate),
        code => Err(MemoryError::InvalidRequestCheckpointOperation { code }),
    }
}

fn encode_optional_atomic_op(op: Option<MemoryAtomicOp>) -> u32 {
    match op {
        None => 0,
        Some(MemoryAtomicOp::Swap) => 1,
        Some(MemoryAtomicOp::Add) => 2,
        Some(MemoryAtomicOp::Xor) => 3,
        Some(MemoryAtomicOp::Or) => 4,
        Some(MemoryAtomicOp::And) => 5,
        Some(MemoryAtomicOp::MinSigned) => 6,
        Some(MemoryAtomicOp::MaxSigned) => 7,
        Some(MemoryAtomicOp::MinUnsigned) => 8,
        Some(MemoryAtomicOp::MaxUnsigned) => 9,
    }
}

fn decode_optional_atomic_op(flags: u32, code: u32) -> Result<Option<MemoryAtomicOp>, MemoryError> {
    if flags & FLAG_ATOMIC_PRESENT == 0 {
        if code == 0 {
            return Ok(None);
        }
        return Err(MemoryError::InvalidRequestCheckpointAtomicOp { code });
    }

    match code {
        1 => Ok(Some(MemoryAtomicOp::Swap)),
        2 => Ok(Some(MemoryAtomicOp::Add)),
        3 => Ok(Some(MemoryAtomicOp::Xor)),
        4 => Ok(Some(MemoryAtomicOp::Or)),
        5 => Ok(Some(MemoryAtomicOp::And)),
        6 => Ok(Some(MemoryAtomicOp::MinSigned)),
        7 => Ok(Some(MemoryAtomicOp::MaxSigned)),
        8 => Ok(Some(MemoryAtomicOp::MinUnsigned)),
        9 => Ok(Some(MemoryAtomicOp::MaxUnsigned)),
        code => Err(MemoryError::InvalidRequestCheckpointAtomicOp { code }),
    }
}

fn request_checkpoint_usize(value: u64) -> Result<usize, MemoryError> {
    value
        .try_into()
        .map_err(|_| MemoryError::InvalidRequestCheckpointUsize { value })
}

fn request_checkpoint_payload_size(data_len: usize, mask_len: usize) -> Result<usize, MemoryError> {
    REQUEST_CHECKPOINT_HEADER_SIZE
        .checked_add(data_len)
        .and_then(|size| size.checked_add(mask_len))
        .ok_or(MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE,
            actual: usize::MAX,
        })
}

fn encode_request_checkpoint_u64(field: &'static str, value: usize) -> Result<u64, MemoryError> {
    value
        .try_into()
        .map_err(|_| MemoryError::RequestCheckpointValueTooLarge {
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
        .ok_or(MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE,
            actual: payload.len(),
        })?;
    let bytes =
        payload
            .get(*offset..end)
            .ok_or(MemoryError::InvalidRequestCheckpointPayloadSize {
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
