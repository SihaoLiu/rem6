use crate::line_checkpoint::LineMemoryCheckpointPayload;
use crate::{
    AccessSize, Address, AddressInterleave, AddressMapRegion, AddressRange, LineMemorySnapshot,
    MemoryError, MemoryPartitionSnapshot, MemoryTargetId, PartitionedMemorySnapshot,
    PartitionedMemoryStore,
};

const PARTITION_CHECKPOINT_MAGIC: [u8; 4] = *b"MPAR";
const PARTITION_CHECKPOINT_VERSION: u32 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const PARTITION_CHECKPOINT_HEADER_BYTES: usize =
    PARTITION_CHECKPOINT_MAGIC.len() + U32_BYTES + U32_BYTES * 4;
const PARTITION_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const PARTITION_CHECKPOINT_U64_MAX: usize = u64::MAX as usize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMemoryCheckpointPayload {
    snapshot: PartitionedMemorySnapshot,
}

impl PartitionedMemoryCheckpointPayload {
    pub fn from_store(store: &PartitionedMemoryStore) -> Self {
        Self {
            snapshot: store.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: PartitionedMemorySnapshot) -> Result<Self, MemoryError> {
        PartitionedMemoryStore::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, MemoryError> {
        if payload.len() < PARTITION_CHECKPOINT_HEADER_BYTES {
            return Err(MemoryError::InvalidPartitionCheckpointPayloadSize {
                expected: PARTITION_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..PARTITION_CHECKPOINT_MAGIC.len()] != PARTITION_CHECKPOINT_MAGIC {
            return Err(MemoryError::InvalidPartitionCheckpointMagic);
        }

        let mut offset = PARTITION_CHECKPOINT_MAGIC.len();
        let version = read_u32(payload, &mut offset)?;
        if version != PARTITION_CHECKPOINT_VERSION {
            return Err(MemoryError::UnsupportedPartitionCheckpointVersion { version });
        }
        let partition_count = read_u32(payload, &mut offset)? as usize;
        let region_count = read_u32(payload, &mut offset)? as usize;
        let reserved = read_u32(payload, &mut offset)?;
        let reserved2 = read_u32(payload, &mut offset)?;
        reject_reserved(reserved)?;
        reject_reserved(reserved2)?;

        let mut partitions = Vec::new();
        for _ in 0..partition_count {
            partitions.push(read_partition_record(payload, &mut offset)?);
        }

        let mut regions = Vec::new();
        for _ in 0..region_count {
            regions.push(read_region_record(payload, &mut offset)?);
        }

        if offset != payload.len() {
            return Err(MemoryError::InvalidPartitionCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshot(PartitionedMemorySnapshot::new(partitions, regions))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("partitioned-memory checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, MemoryError> {
        let partition_count =
            encode_partition_checkpoint_u32("partition count", self.snapshot.partitions().len())?;
        let region_count =
            encode_partition_checkpoint_u32("region count", self.snapshot.regions().len())?;
        let mut payload = Vec::new();
        payload.extend_from_slice(&PARTITION_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&PARTITION_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&partition_count.to_le_bytes());
        payload.extend_from_slice(&region_count.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        for partition in self.snapshot.partitions() {
            write_partition_record(&mut payload, partition)?;
        }
        for (target, region) in self.snapshot.regions() {
            write_region_record(&mut payload, *target, region)?;
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &PartitionedMemorySnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> PartitionedMemorySnapshot {
        self.snapshot
    }
}

fn write_partition_record(
    payload: &mut Vec<u8>,
    partition: &MemoryPartitionSnapshot,
) -> Result<(), MemoryError> {
    let store_snapshot = LineMemorySnapshot::new(partition.layout(), partition.lines().to_vec());
    let line_payload = LineMemoryCheckpointPayload::from_snapshot(store_snapshot)?.encode();
    let line_payload_size =
        encode_partition_checkpoint_u64("line payload size", line_payload.len())?;
    payload.extend_from_slice(&partition.target().get().to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&line_payload_size.to_le_bytes());
    payload.extend_from_slice(&line_payload);
    Ok(())
}

fn read_partition_record(
    payload: &[u8],
    offset: &mut usize,
) -> Result<MemoryPartitionSnapshot, MemoryError> {
    let target = MemoryTargetId::new(read_u32(payload, offset)?);
    let reserved = read_u32(payload, offset)?;
    reject_reserved(reserved)?;
    let line_payload_size = partition_checkpoint_usize(read_u64(payload, offset)?)?;
    let line_payload = read_exact(payload, offset, line_payload_size)?;
    let decoded = LineMemoryCheckpointPayload::decode(line_payload)?;
    Ok(MemoryPartitionSnapshot::new(
        target,
        decoded.into_snapshot(),
    ))
}

fn write_region_record(
    payload: &mut Vec<u8>,
    target: MemoryTargetId,
    region: &AddressMapRegion,
) -> Result<(), MemoryError> {
    let hole_count = encode_partition_checkpoint_u32("hole count", region.holes().len())?;
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
    let reserved = read_u32(payload, offset)?;
    reject_reserved(reserved)?;

    let interleave = match interleave_flag {
        0 => None,
        1 => Some(AddressInterleave::modulo(
            AccessSize::new(read_u64(payload, offset)?)?,
            read_u32(payload, offset)?,
            read_u32(payload, offset)?,
        )?),
        value => return Err(MemoryError::InvalidPartitionCheckpointInterleaveFlag { value }),
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
        return Err(MemoryError::InvalidPartitionCheckpointReserved { value });
    }
    Ok(())
}

fn partition_checkpoint_usize(value: u64) -> Result<usize, MemoryError> {
    usize::try_from(value).map_err(|_| MemoryError::InvalidPartitionCheckpointUsize { value })
}

fn encode_partition_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, MemoryError> {
    u32::try_from(value).map_err(|_| MemoryError::PartitionCheckpointValueTooLarge {
        field,
        value,
        maximum: PARTITION_CHECKPOINT_U32_MAX,
    })
}

fn encode_partition_checkpoint_u64(field: &'static str, value: usize) -> Result<u64, MemoryError> {
    u64::try_from(value).map_err(|_| MemoryError::PartitionCheckpointValueTooLarge {
        field,
        value,
        maximum: PARTITION_CHECKPOINT_U64_MAX,
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
            .ok_or(MemoryError::InvalidPartitionCheckpointPayloadSize {
                expected: usize::MAX,
                actual: payload.len(),
            })?;
    if expected > payload.len() {
        return Err(MemoryError::InvalidPartitionCheckpointPayloadSize {
            expected,
            actual: payload.len(),
        });
    }
    let result = &payload[*offset..expected];
    *offset = expected;
    Ok(result)
}
