use crate::{
    AccessSize, Address, AddressDecoder, AddressDecoderSnapshot, AddressInterleave,
    AddressMapRegion, AddressRange, MemoryError, MemoryTargetId,
};

const DECODER_CHECKPOINT_MAGIC: [u8; 4] = *b"MDEC";
const DECODER_CHECKPOINT_VERSION: u32 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const DECODER_CHECKPOINT_HEADER_BYTES: usize =
    DECODER_CHECKPOINT_MAGIC.len() + U32_BYTES + U32_BYTES * 3;
const DECODER_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressDecoderCheckpointPayload {
    snapshot: AddressDecoderSnapshot,
}

impl AddressDecoderCheckpointPayload {
    pub fn from_decoder(decoder: &AddressDecoder) -> Self {
        Self {
            snapshot: decoder.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: AddressDecoderSnapshot) -> Result<Self, MemoryError> {
        AddressDecoder::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, MemoryError> {
        if payload.len() < DECODER_CHECKPOINT_HEADER_BYTES {
            return Err(MemoryError::InvalidDecoderCheckpointPayloadSize {
                expected: DECODER_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..DECODER_CHECKPOINT_MAGIC.len()] != DECODER_CHECKPOINT_MAGIC {
            return Err(MemoryError::InvalidDecoderCheckpointMagic);
        }

        let mut offset = DECODER_CHECKPOINT_MAGIC.len();
        let version = read_u32(payload, &mut offset)?;
        if version != DECODER_CHECKPOINT_VERSION {
            return Err(MemoryError::UnsupportedDecoderCheckpointVersion { version });
        }
        let region_count = read_u32(payload, &mut offset)? as usize;
        reject_reserved(read_u32(payload, &mut offset)?)?;
        reject_reserved(read_u32(payload, &mut offset)?)?;

        let mut regions = Vec::new();
        for _ in 0..region_count {
            regions.push(read_region_record(payload, &mut offset)?);
        }

        if offset != payload.len() {
            return Err(MemoryError::InvalidDecoderCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshot(AddressDecoderSnapshot::new(regions))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("address-decoder checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, MemoryError> {
        let region_count =
            encode_decoder_checkpoint_u32("region count", self.snapshot.regions().len())?;
        let mut payload = Vec::new();
        payload.extend_from_slice(&DECODER_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&DECODER_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&region_count.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        for (target, region) in self.snapshot.regions() {
            write_region_record(&mut payload, *target, region)?;
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &AddressDecoderSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> AddressDecoderSnapshot {
        self.snapshot
    }
}

fn write_region_record(
    payload: &mut Vec<u8>,
    target: MemoryTargetId,
    region: &AddressMapRegion,
) -> Result<(), MemoryError> {
    let hole_count = encode_decoder_checkpoint_u32("hole count", region.holes().len())?;
    payload.extend_from_slice(&target.get().to_le_bytes());
    payload.extend_from_slice(&u32::from(region.interleave().is_some()).to_le_bytes());
    payload.extend_from_slice(&region.start().get().to_le_bytes());
    payload.extend_from_slice(&region.size().bytes().to_le_bytes());
    payload.extend_from_slice(&hole_count.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    if let Some(interleave) = region.interleave() {
        payload.extend_from_slice(&interleave.granularity().bytes().to_le_bytes());
        payload.extend_from_slice(&interleave.stripes().to_le_bytes());
        payload.extend_from_slice(&interleave.match_index().to_le_bytes());
    }
    for hole in region.holes() {
        payload.extend_from_slice(&hole.start().get().to_le_bytes());
        payload.extend_from_slice(&hole.size().bytes().to_le_bytes());
    }
    Ok(())
}

fn read_region_record(
    payload: &[u8],
    offset: &mut usize,
) -> Result<(MemoryTargetId, AddressMapRegion), MemoryError> {
    let target = MemoryTargetId::new(read_u32(payload, offset)?);
    let interleave_flag = read_u32(payload, offset)?;
    let base = AddressRange::new(
        Address::new(read_u64(payload, offset)?),
        AccessSize::new(read_u64(payload, offset)?)?,
    )?;
    let hole_count = read_u32(payload, offset)? as usize;
    reject_reserved(read_u32(payload, offset)?)?;

    let interleave = match interleave_flag {
        0 => None,
        1 => Some(AddressInterleave::modulo(
            AccessSize::new(read_u64(payload, offset)?)?,
            read_u32(payload, offset)?,
            read_u32(payload, offset)?,
        )?),
        value => return Err(MemoryError::InvalidDecoderCheckpointInterleaveFlag { value }),
    };

    let mut holes = Vec::new();
    for _ in 0..hole_count {
        holes.push(AddressRange::new(
            Address::new(read_u64(payload, offset)?),
            AccessSize::new(read_u64(payload, offset)?)?,
        )?);
    }

    let mut region = AddressMapRegion::new(base);
    if !holes.is_empty() {
        region = region.with_holes(holes)?;
    }
    if let Some(interleave) = interleave {
        region = region.with_interleave(interleave)?;
    }
    Ok((target, region))
}

fn reject_reserved(value: u32) -> Result<(), MemoryError> {
    if value != 0 {
        return Err(MemoryError::InvalidDecoderCheckpointReserved { value });
    }
    Ok(())
}

fn encode_decoder_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, MemoryError> {
    u32::try_from(value).map_err(|_| MemoryError::DecoderCheckpointValueTooLarge {
        field,
        value,
        maximum: DECODER_CHECKPOINT_U32_MAX,
    })
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, MemoryError> {
    let bytes = read_exact(payload, offset, U32_BYTES)?
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, MemoryError> {
    let bytes = read_exact(payload, offset, U64_BYTES)?
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    Ok(u64::from_le_bytes(bytes))
}

fn read_exact<'a>(
    payload: &'a [u8],
    offset: &mut usize,
    bytes: usize,
) -> Result<&'a [u8], MemoryError> {
    let expected =
        offset
            .checked_add(bytes)
            .ok_or(MemoryError::InvalidDecoderCheckpointPayloadSize {
                expected: usize::MAX,
                actual: payload.len(),
            })?;
    if expected > payload.len() {
        return Err(MemoryError::InvalidDecoderCheckpointPayloadSize {
            expected,
            actual: payload.len(),
        });
    }
    let result = &payload[*offset..expected];
    *offset = expected;
    Ok(result)
}
