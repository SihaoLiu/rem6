use crate::{
    BiModeBranchPredictorConfig, BiModeBranchPredictorError, BiModeBranchPredictorSnapshot,
    BiModeThreadSnapshot,
};

const CHECKPOINT_MAGIC: [u8; 4] = *b"RBIM";
const CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize =
    CHECKPOINT_MAGIC.len() + 1 + U32_BYTES * 3 + 3 + U64_BYTES * 4;
const CHECKPOINT_THREAD_BYTES: usize = U64_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeBranchPredictorCheckpointPayload {
    snapshot: BiModeBranchPredictorSnapshot,
}

impl BiModeBranchPredictorCheckpointPayload {
    pub fn from_snapshot(
        snapshot: BiModeBranchPredictorSnapshot,
    ) -> Result<Self, BiModeBranchPredictorError> {
        validate_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub const fn snapshot(&self) -> &BiModeBranchPredictorSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> BiModeBranchPredictorSnapshot {
        self.snapshot
    }

    pub fn encode(&self) -> Vec<u8> {
        let expected_len = checkpoint_payload_len(
            self.snapshot.config().choice_entries(),
            self.snapshot.config().global_entries(),
            self.snapshot.config().threads(),
        )
        .expect("validated bimode checkpoint length is representable");
        let mut payload = Vec::with_capacity(expected_len);

        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(CHECKPOINT_VERSION);
        payload.extend_from_slice(&(self.snapshot.config().threads() as u32).to_le_bytes());
        payload.extend_from_slice(&(self.snapshot.config().choice_entries() as u32).to_le_bytes());
        payload.extend_from_slice(&(self.snapshot.config().global_entries() as u32).to_le_bytes());
        payload.push(self.snapshot.config().choice_counter_bits());
        payload.push(self.snapshot.config().global_counter_bits());
        payload.push(self.snapshot.config().inst_shift());
        payload.extend_from_slice(&self.snapshot.lookup_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.history_update_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.update_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.squash_count().to_le_bytes());
        payload.extend_from_slice(self.snapshot.choice_counters());
        payload.extend_from_slice(self.snapshot.taken_counters());
        payload.extend_from_slice(self.snapshot.not_taken_counters());
        for thread in self.snapshot.threads() {
            payload.extend_from_slice(&thread.global_history().to_le_bytes());
        }

        debug_assert_eq!(payload.len(), expected_len);
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, BiModeBranchPredictorError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(BiModeBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(BiModeBranchPredictorError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != CHECKPOINT_VERSION {
            return Err(BiModeBranchPredictorError::UnsupportedCheckpointVersion { version });
        }
        let threads = read_u32(payload, &mut offset)? as usize;
        let choice_entries = read_u32(payload, &mut offset)? as usize;
        let global_entries = read_u32(payload, &mut offset)? as usize;
        let choice_counter_bits = read_u8(payload, &mut offset)?;
        let global_counter_bits = read_u8(payload, &mut offset)?;
        let inst_shift = read_u8(payload, &mut offset)?;
        let lookup_count = read_u64(payload, &mut offset)?;
        let history_update_count = read_u64(payload, &mut offset)?;
        let update_count = read_u64(payload, &mut offset)?;
        let squash_count = read_u64(payload, &mut offset)?;

        let expected_len = checkpoint_payload_len(choice_entries, global_entries, threads)?;
        if payload.len() != expected_len {
            return Err(BiModeBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: expected_len,
                actual: payload.len(),
            });
        }

        let config = BiModeBranchPredictorConfig::with_options(
            threads,
            choice_entries,
            global_entries,
            choice_counter_bits,
            global_counter_bits,
            inst_shift,
        )?;
        let choice_end = checked_offset(offset, choice_entries)?;
        let choice_counters = payload[offset..choice_end].to_vec();
        offset = choice_end;
        let taken_end = checked_offset(offset, global_entries)?;
        let taken_counters = payload[offset..taken_end].to_vec();
        offset = taken_end;
        let not_taken_end = checked_offset(offset, global_entries)?;
        let not_taken_counters = payload[offset..not_taken_end].to_vec();
        offset = not_taken_end;
        let mut thread_snapshots = Vec::with_capacity(threads);
        for _ in 0..threads {
            thread_snapshots.push(BiModeThreadSnapshot::from_global_history(read_u64(
                payload,
                &mut offset,
            )?));
        }
        debug_assert_eq!(offset, payload.len());

        Self::from_snapshot(BiModeBranchPredictorSnapshot::from_parts(
            config,
            choice_counters,
            taken_counters,
            not_taken_counters,
            thread_snapshots,
            lookup_count,
            history_update_count,
            update_count,
            squash_count,
        ))
    }
}

fn validate_snapshot(
    snapshot: &BiModeBranchPredictorSnapshot,
) -> Result<(), BiModeBranchPredictorError> {
    require_u32("threads", snapshot.config().threads())?;
    require_u32("choice-entries", snapshot.config().choice_entries())?;
    require_u32("global-entries", snapshot.config().global_entries())?;
    checkpoint_payload_len(
        snapshot.config().choice_entries(),
        snapshot.config().global_entries(),
        snapshot.config().threads(),
    )?;
    if snapshot.choice_counters().len() != snapshot.config().choice_entries()
        || snapshot.taken_counters().len() != snapshot.config().global_entries()
        || snapshot.not_taken_counters().len() != snapshot.config().global_entries()
        || snapshot.threads().len() != snapshot.config().threads()
    {
        return Err(BiModeBranchPredictorError::SnapshotShapeMismatch {
            expected_threads: snapshot.config().threads(),
            actual_threads: snapshot.threads().len(),
            expected_choice_entries: snapshot.config().choice_entries(),
            actual_choice_entries: snapshot.choice_counters().len(),
            expected_global_entries: snapshot.config().global_entries(),
            actual_global_entries: snapshot
                .taken_counters()
                .len()
                .max(snapshot.not_taken_counters().len()),
        });
    }

    let choice_max = counter_max(snapshot.config().choice_counter_bits());
    validate_counters("choice", snapshot.choice_counters(), choice_max)?;
    let global_max = counter_max(snapshot.config().global_counter_bits());
    validate_counters("taken", snapshot.taken_counters(), global_max)?;
    validate_counters("not-taken", snapshot.not_taken_counters(), global_max)
}

fn validate_counters(
    table: &'static str,
    counters: &[u8],
    max: u8,
) -> Result<(), BiModeBranchPredictorError> {
    for counter in counters {
        if *counter > max {
            return Err(BiModeBranchPredictorError::InvalidCheckpointCounter {
                table,
                value: *counter,
                max,
            });
        }
    }
    Ok(())
}

fn checkpoint_payload_len(
    choice_entries: usize,
    global_entries: usize,
    threads: usize,
) -> Result<usize, BiModeBranchPredictorError> {
    let global_bytes = checked_product("global-entries", global_entries, 2)?;
    let thread_bytes = checked_product("threads", threads, CHECKPOINT_THREAD_BYTES)?;
    let len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, choice_entries)?;
    let len = checked_sum("payload-size", len, global_bytes)?;
    checked_sum("payload-size", len, thread_bytes)
}

fn checked_product(
    name: &'static str,
    count: usize,
    bytes: usize,
) -> Result<usize, BiModeBranchPredictorError> {
    count
        .checked_mul(bytes)
        .ok_or(BiModeBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: count,
            max: usize::MAX / bytes,
        })
}

fn checked_sum(
    name: &'static str,
    base: usize,
    increment: usize,
) -> Result<usize, BiModeBranchPredictorError> {
    base.checked_add(increment)
        .ok_or(BiModeBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: increment,
            max: usize::MAX - base,
        })
}

fn checked_offset(base: usize, increment: usize) -> Result<usize, BiModeBranchPredictorError> {
    checked_sum("payload-offset", base, increment)
}

fn require_u32(name: &'static str, value: usize) -> Result<u32, BiModeBranchPredictorError> {
    u32::try_from(value).map_err(|_| BiModeBranchPredictorError::CheckpointValueTooLarge {
        name,
        value,
        max: CHECKPOINT_U32_MAX,
    })
}

fn counter_max(bits: u8) -> u8 {
    ((1u16 << bits) - 1) as u8
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, BiModeBranchPredictorError> {
    let value =
        *payload
            .get(*offset)
            .ok_or(BiModeBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: *offset + 1,
                actual: payload.len(),
            })?;
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, BiModeBranchPredictorError> {
    Ok(u32::from_le_bytes(read_array(payload, offset)?))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, BiModeBranchPredictorError> {
    Ok(u64::from_le_bytes(read_array(payload, offset)?))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], BiModeBranchPredictorError> {
    let end = checked_offset(*offset, N)?;
    let bytes = payload.get(*offset..end).ok_or(
        BiModeBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.try_into().expect("slice length matches array length"))
}
