use crate::{
    GShareBranchPredictorConfig, GShareBranchPredictorError, GShareBranchPredictorSnapshot,
    GShareThreadSnapshot,
};

const CHECKPOINT_MAGIC: [u8; 4] = *b"RGSH";
const CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize =
    CHECKPOINT_MAGIC.len() + 1 + U32_BYTES * 2 + 2 + U64_BYTES * 4;
const CHECKPOINT_THREAD_BYTES: usize = U64_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GShareBranchPredictorCheckpointPayload {
    snapshot: GShareBranchPredictorSnapshot,
}

impl GShareBranchPredictorCheckpointPayload {
    pub fn from_snapshot(
        snapshot: GShareBranchPredictorSnapshot,
    ) -> Result<Self, GShareBranchPredictorError> {
        validate_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub const fn snapshot(&self) -> &GShareBranchPredictorSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> GShareBranchPredictorSnapshot {
        self.snapshot
    }

    pub fn encode(&self) -> Vec<u8> {
        let expected_len = checkpoint_payload_len(
            self.snapshot.config().table_entries(),
            self.snapshot.config().threads(),
        )
        .expect("validated gshare checkpoint length is representable");
        let mut payload = Vec::with_capacity(expected_len);

        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(CHECKPOINT_VERSION);
        payload.extend_from_slice(&(self.snapshot.config().threads() as u32).to_le_bytes());
        payload.extend_from_slice(&(self.snapshot.config().table_entries() as u32).to_le_bytes());
        payload.push(self.snapshot.config().counter_bits());
        payload.push(self.snapshot.config().inst_shift());
        payload.extend_from_slice(&self.snapshot.lookup_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.history_update_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.update_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.squash_count().to_le_bytes());
        payload.extend_from_slice(self.snapshot.counters());
        for thread in self.snapshot.threads() {
            payload.extend_from_slice(&thread.global_history().to_le_bytes());
        }

        debug_assert_eq!(payload.len(), expected_len);
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, GShareBranchPredictorError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(GShareBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(GShareBranchPredictorError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != CHECKPOINT_VERSION {
            return Err(GShareBranchPredictorError::UnsupportedCheckpointVersion { version });
        }
        let threads = read_u32(payload, &mut offset)? as usize;
        let table_entries = read_u32(payload, &mut offset)? as usize;
        let counter_bits = read_u8(payload, &mut offset)?;
        let inst_shift = read_u8(payload, &mut offset)?;
        let lookup_count = read_u64(payload, &mut offset)?;
        let history_update_count = read_u64(payload, &mut offset)?;
        let update_count = read_u64(payload, &mut offset)?;
        let squash_count = read_u64(payload, &mut offset)?;

        let expected_len = checkpoint_payload_len(table_entries, threads)?;
        if payload.len() != expected_len {
            return Err(GShareBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: expected_len,
                actual: payload.len(),
            });
        }

        let config = GShareBranchPredictorConfig::with_options(
            threads,
            table_entries,
            counter_bits,
            inst_shift,
        )?;
        let counters_end = checked_offset(offset, table_entries)?;
        let counters = payload[offset..counters_end].to_vec();
        offset = counters_end;
        let mut thread_snapshots = Vec::with_capacity(threads);
        for _ in 0..threads {
            thread_snapshots.push(GShareThreadSnapshot::from_global_history(read_u64(
                payload,
                &mut offset,
            )?));
        }
        debug_assert_eq!(offset, payload.len());

        Self::from_snapshot(GShareBranchPredictorSnapshot::from_parts(
            config,
            counters,
            thread_snapshots,
            lookup_count,
            history_update_count,
            update_count,
            squash_count,
        ))
    }
}

fn validate_snapshot(
    snapshot: &GShareBranchPredictorSnapshot,
) -> Result<(), GShareBranchPredictorError> {
    require_u32("threads", snapshot.config().threads())?;
    require_u32("table-entries", snapshot.config().table_entries())?;
    checkpoint_payload_len(
        snapshot.config().table_entries(),
        snapshot.config().threads(),
    )?;
    if snapshot.counters().len() != snapshot.config().table_entries()
        || snapshot.threads().len() != snapshot.config().threads()
    {
        return Err(GShareBranchPredictorError::SnapshotShapeMismatch {
            expected_threads: snapshot.config().threads(),
            actual_threads: snapshot.threads().len(),
            expected_entries: snapshot.config().table_entries(),
            actual_entries: snapshot.counters().len(),
        });
    }
    let counter_max = ((1u16 << snapshot.config().counter_bits()) - 1) as u8;
    for counter in snapshot.counters() {
        if *counter > counter_max {
            return Err(GShareBranchPredictorError::InvalidCheckpointCounter {
                value: *counter,
                max: counter_max,
            });
        }
    }
    Ok(())
}

fn checkpoint_payload_len(
    table_entries: usize,
    threads: usize,
) -> Result<usize, GShareBranchPredictorError> {
    let thread_bytes = checked_product("threads", threads, CHECKPOINT_THREAD_BYTES)?;
    let len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, table_entries)?;
    checked_sum("payload-size", len, thread_bytes)
}

fn checked_product(
    name: &'static str,
    count: usize,
    bytes: usize,
) -> Result<usize, GShareBranchPredictorError> {
    count
        .checked_mul(bytes)
        .ok_or(GShareBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: count,
            max: usize::MAX / bytes,
        })
}

fn checked_sum(
    name: &'static str,
    base: usize,
    increment: usize,
) -> Result<usize, GShareBranchPredictorError> {
    base.checked_add(increment)
        .ok_or(GShareBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: increment,
            max: usize::MAX - base,
        })
}

fn checked_offset(base: usize, increment: usize) -> Result<usize, GShareBranchPredictorError> {
    checked_sum("payload-offset", base, increment)
}

fn require_u32(name: &'static str, value: usize) -> Result<u32, GShareBranchPredictorError> {
    u32::try_from(value).map_err(|_| GShareBranchPredictorError::CheckpointValueTooLarge {
        name,
        value,
        max: CHECKPOINT_U32_MAX,
    })
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, GShareBranchPredictorError> {
    let value =
        *payload
            .get(*offset)
            .ok_or(GShareBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: *offset + 1,
                actual: payload.len(),
            })?;
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, GShareBranchPredictorError> {
    Ok(u32::from_le_bytes(read_array(payload, offset)?))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, GShareBranchPredictorError> {
    Ok(u64::from_le_bytes(read_array(payload, offset)?))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], GShareBranchPredictorError> {
    let end = checked_offset(*offset, N)?;
    let bytes = payload.get(*offset..end).ok_or(
        GShareBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.try_into().expect("slice length matches array length"))
}
