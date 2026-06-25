use crate::{
    TournamentBranchPredictorConfig, TournamentBranchPredictorError,
    TournamentBranchPredictorSnapshot, TournamentThreadSnapshot,
};

const CHECKPOINT_MAGIC: [u8; 4] = *b"RTRN";
const CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize =
    CHECKPOINT_MAGIC.len() + 1 + U32_BYTES * 5 + 4 + U64_BYTES * 4;
const CHECKPOINT_LOCAL_HISTORY_BYTES: usize = U64_BYTES;
const CHECKPOINT_THREAD_BYTES: usize = U64_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentBranchPredictorCheckpointPayload {
    snapshot: TournamentBranchPredictorSnapshot,
}

impl TournamentBranchPredictorCheckpointPayload {
    pub fn from_snapshot(
        snapshot: TournamentBranchPredictorSnapshot,
    ) -> Result<Self, TournamentBranchPredictorError> {
        validate_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub const fn snapshot(&self) -> &TournamentBranchPredictorSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> TournamentBranchPredictorSnapshot {
        self.snapshot
    }

    pub fn encode(&self) -> Vec<u8> {
        let expected_len = checkpoint_payload_len(
            self.snapshot.config().local_entries(),
            self.snapshot.config().local_history_entries(),
            self.snapshot.config().global_entries(),
            self.snapshot.config().choice_entries(),
            self.snapshot.config().threads(),
        )
        .expect("validated tournament checkpoint length is representable");
        let mut payload = Vec::with_capacity(expected_len);

        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(CHECKPOINT_VERSION);
        payload.extend_from_slice(&(self.snapshot.config().threads() as u32).to_le_bytes());
        payload.extend_from_slice(&(self.snapshot.config().local_entries() as u32).to_le_bytes());
        payload.extend_from_slice(
            &(self.snapshot.config().local_history_entries() as u32).to_le_bytes(),
        );
        payload.extend_from_slice(&(self.snapshot.config().global_entries() as u32).to_le_bytes());
        payload.extend_from_slice(&(self.snapshot.config().choice_entries() as u32).to_le_bytes());
        payload.push(self.snapshot.config().local_counter_bits());
        payload.push(self.snapshot.config().global_counter_bits());
        payload.push(self.snapshot.config().choice_counter_bits());
        payload.push(self.snapshot.config().inst_shift());
        payload.extend_from_slice(&self.snapshot.lookup_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.history_update_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.update_count().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.squash_count().to_le_bytes());
        payload.extend_from_slice(self.snapshot.local_counters());
        for history in self.snapshot.local_history_table() {
            payload.extend_from_slice(&history.to_le_bytes());
        }
        payload.extend_from_slice(self.snapshot.global_counters());
        payload.extend_from_slice(self.snapshot.choice_counters());
        for thread in self.snapshot.threads() {
            payload.extend_from_slice(&thread.global_history().to_le_bytes());
        }

        debug_assert_eq!(payload.len(), expected_len);
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, TournamentBranchPredictorError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(
                TournamentBranchPredictorError::InvalidCheckpointPayloadSize {
                    expected: CHECKPOINT_HEADER_BYTES,
                    actual: payload.len(),
                },
            );
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(TournamentBranchPredictorError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != CHECKPOINT_VERSION {
            return Err(TournamentBranchPredictorError::UnsupportedCheckpointVersion { version });
        }
        let threads = read_u32(payload, &mut offset)? as usize;
        let local_entries = read_u32(payload, &mut offset)? as usize;
        let local_history_entries = read_u32(payload, &mut offset)? as usize;
        let global_entries = read_u32(payload, &mut offset)? as usize;
        let choice_entries = read_u32(payload, &mut offset)? as usize;
        let local_counter_bits = read_u8(payload, &mut offset)?;
        let global_counter_bits = read_u8(payload, &mut offset)?;
        let choice_counter_bits = read_u8(payload, &mut offset)?;
        let inst_shift = read_u8(payload, &mut offset)?;
        let lookup_count = read_u64(payload, &mut offset)?;
        let history_update_count = read_u64(payload, &mut offset)?;
        let update_count = read_u64(payload, &mut offset)?;
        let squash_count = read_u64(payload, &mut offset)?;

        let expected_len = checkpoint_payload_len(
            local_entries,
            local_history_entries,
            global_entries,
            choice_entries,
            threads,
        )?;
        if payload.len() != expected_len {
            return Err(
                TournamentBranchPredictorError::InvalidCheckpointPayloadSize {
                    expected: expected_len,
                    actual: payload.len(),
                },
            );
        }

        let config = TournamentBranchPredictorConfig::with_options(
            threads,
            local_entries,
            local_history_entries,
            global_entries,
            choice_entries,
            local_counter_bits,
            global_counter_bits,
            choice_counter_bits,
            inst_shift,
        )?;
        let local_counters_end = checked_offset(offset, local_entries)?;
        let local_counters = payload[offset..local_counters_end].to_vec();
        offset = local_counters_end;
        let mut local_history_table = Vec::with_capacity(local_history_entries);
        for _ in 0..local_history_entries {
            local_history_table.push(read_u64(payload, &mut offset)?);
        }
        let global_counters_end = checked_offset(offset, global_entries)?;
        let global_counters = payload[offset..global_counters_end].to_vec();
        offset = global_counters_end;
        let choice_counters_end = checked_offset(offset, choice_entries)?;
        let choice_counters = payload[offset..choice_counters_end].to_vec();
        offset = choice_counters_end;
        let mut thread_snapshots = Vec::with_capacity(threads);
        for _ in 0..threads {
            thread_snapshots.push(TournamentThreadSnapshot::from_global_history(read_u64(
                payload,
                &mut offset,
            )?));
        }
        debug_assert_eq!(offset, payload.len());

        Self::from_snapshot(TournamentBranchPredictorSnapshot::from_parts(
            config,
            local_counters,
            local_history_table,
            global_counters,
            choice_counters,
            thread_snapshots,
            lookup_count,
            history_update_count,
            update_count,
            squash_count,
        ))
    }
}

fn validate_snapshot(
    snapshot: &TournamentBranchPredictorSnapshot,
) -> Result<(), TournamentBranchPredictorError> {
    require_u32("threads", snapshot.config().threads())?;
    require_u32("local-entries", snapshot.config().local_entries())?;
    require_u32(
        "local-history-entries",
        snapshot.config().local_history_entries(),
    )?;
    require_u32("global-entries", snapshot.config().global_entries())?;
    require_u32("choice-entries", snapshot.config().choice_entries())?;
    checkpoint_payload_len(
        snapshot.config().local_entries(),
        snapshot.config().local_history_entries(),
        snapshot.config().global_entries(),
        snapshot.config().choice_entries(),
        snapshot.config().threads(),
    )?;
    if snapshot.local_counters().len() != snapshot.config().local_entries()
        || snapshot.local_history_table().len() != snapshot.config().local_history_entries()
        || snapshot.global_counters().len() != snapshot.config().global_entries()
        || snapshot.choice_counters().len() != snapshot.config().choice_entries()
        || snapshot.threads().len() != snapshot.config().threads()
    {
        return Err(TournamentBranchPredictorError::SnapshotShapeMismatch {
            expected_threads: snapshot.config().threads(),
            actual_threads: snapshot.threads().len(),
            expected_local_entries: snapshot.config().local_entries(),
            actual_local_entries: snapshot.local_counters().len(),
            expected_local_history_entries: snapshot.config().local_history_entries(),
            actual_local_history_entries: snapshot.local_history_table().len(),
            expected_global_entries: snapshot.config().global_entries(),
            actual_global_entries: snapshot.global_counters().len(),
            expected_choice_entries: snapshot.config().choice_entries(),
            actual_choice_entries: snapshot.choice_counters().len(),
        });
    }

    validate_counters(
        "local",
        snapshot.local_counters(),
        counter_max(snapshot.config().local_counter_bits()),
    )?;
    validate_counters(
        "global",
        snapshot.global_counters(),
        counter_max(snapshot.config().global_counter_bits()),
    )?;
    validate_counters(
        "choice",
        snapshot.choice_counters(),
        counter_max(snapshot.config().choice_counter_bits()),
    )?;
    let max_local_history = snapshot.config().local_entries() as u64 - 1;
    for history in snapshot.local_history_table() {
        if *history > max_local_history {
            return Err(
                TournamentBranchPredictorError::InvalidCheckpointLocalHistory {
                    value: *history,
                    max: max_local_history,
                },
            );
        }
    }
    Ok(())
}

fn validate_counters(
    table: &'static str,
    counters: &[u8],
    max: u8,
) -> Result<(), TournamentBranchPredictorError> {
    for counter in counters {
        if *counter > max {
            return Err(TournamentBranchPredictorError::InvalidCheckpointCounter {
                table,
                value: *counter,
                max,
            });
        }
    }
    Ok(())
}

fn checkpoint_payload_len(
    local_entries: usize,
    local_history_entries: usize,
    global_entries: usize,
    choice_entries: usize,
    threads: usize,
) -> Result<usize, TournamentBranchPredictorError> {
    let local_history_bytes = checked_product(
        "local-history-entries",
        local_history_entries,
        CHECKPOINT_LOCAL_HISTORY_BYTES,
    )?;
    let thread_bytes = checked_product("threads", threads, CHECKPOINT_THREAD_BYTES)?;
    let len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, local_entries)?;
    let len = checked_sum("payload-size", len, local_history_bytes)?;
    let len = checked_sum("payload-size", len, global_entries)?;
    let len = checked_sum("payload-size", len, choice_entries)?;
    checked_sum("payload-size", len, thread_bytes)
}

fn checked_product(
    name: &'static str,
    count: usize,
    bytes: usize,
) -> Result<usize, TournamentBranchPredictorError> {
    count
        .checked_mul(bytes)
        .ok_or(TournamentBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: count,
            max: usize::MAX / bytes,
        })
}

fn checked_sum(
    name: &'static str,
    base: usize,
    increment: usize,
) -> Result<usize, TournamentBranchPredictorError> {
    base.checked_add(increment)
        .ok_or(TournamentBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: increment,
            max: usize::MAX - base,
        })
}

fn checked_offset(base: usize, increment: usize) -> Result<usize, TournamentBranchPredictorError> {
    checked_sum("payload-offset", base, increment)
}

fn require_u32(name: &'static str, value: usize) -> Result<u32, TournamentBranchPredictorError> {
    u32::try_from(value).map_err(
        |_| TournamentBranchPredictorError::CheckpointValueTooLarge {
            name,
            value,
            max: CHECKPOINT_U32_MAX,
        },
    )
}

fn counter_max(bits: u8) -> u8 {
    ((1u16 << bits) - 1) as u8
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, TournamentBranchPredictorError> {
    let value = *payload.get(*offset).ok_or(
        TournamentBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: *offset + 1,
            actual: payload.len(),
        },
    )?;
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, TournamentBranchPredictorError> {
    Ok(u32::from_le_bytes(read_array(payload, offset)?))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, TournamentBranchPredictorError> {
    Ok(u64::from_le_bytes(read_array(payload, offset)?))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], TournamentBranchPredictorError> {
    let end = checked_offset(*offset, N)?;
    let bytes = payload.get(*offset..end).ok_or(
        TournamentBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.try_into().expect("slice length matches array length"))
}
