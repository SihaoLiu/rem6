use std::collections::BTreeSet;

use rem6_memory::Address;

use crate::branch_predictor::{
    BranchPrediction, BranchPredictorConfig, BranchPredictorError, BranchPredictorSnapshot,
    BranchSpeculation, BranchSpeculationId,
};

const MAX_CHECKPOINT_COUNTER: u8 = 3;
const CHECKPOINT_MAGIC: [u8; 4] = *b"RIBP";
const CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize =
    CHECKPOINT_MAGIC.len() + 1 + U32_BYTES + 1 + U64_BYTES * 4 + U32_BYTES * 2;
const CHECKPOINT_COUNTER_BYTES: usize = 1;
const CHECKPOINT_TARGET_BYTES: usize = 1 + U64_BYTES;
const CHECKPOINT_PENDING_SPECULATION_BYTES: usize =
    U64_BYTES + U64_BYTES + U32_BYTES + 1 + 1 + U64_BYTES + 1 + U64_BYTES + U64_BYTES + 1 + 1;
const CHECKPOINT_ACTIVE_SPECULATION_BYTES: usize = U64_BYTES + U64_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictorCheckpointPayload {
    snapshot: BranchPredictorSnapshot,
    active_speculations: Vec<(u64, BranchSpeculationId)>,
}

impl BranchPredictorCheckpointPayload {
    pub fn from_snapshot<I>(
        snapshot: BranchPredictorSnapshot,
        active_speculations: I,
    ) -> Result<Self, BranchPredictorError>
    where
        I: IntoIterator<Item = (u64, BranchSpeculationId)>,
    {
        let mut active_speculations = active_speculations.into_iter().collect::<Vec<_>>();
        active_speculations.sort_by_key(|(sequence, id)| (*sequence, id.get()));
        validate_checkpoint_snapshot(&snapshot, &active_speculations)?;
        Ok(Self {
            snapshot,
            active_speculations,
        })
    }

    pub const fn snapshot(&self) -> &BranchPredictorSnapshot {
        &self.snapshot
    }

    pub fn active_speculations(&self) -> &[(u64, BranchSpeculationId)] {
        &self.active_speculations
    }

    pub fn into_parts(self) -> (BranchPredictorSnapshot, Vec<(u64, BranchSpeculationId)>) {
        (self.snapshot, self.active_speculations)
    }

    pub fn encode(&self) -> Vec<u8> {
        let table_entries = self.snapshot.config.table_entries();
        let expected_len = checkpoint_payload_len(
            table_entries,
            self.snapshot.pending_speculations.len(),
            self.active_speculations.len(),
        )
        .expect("validated branch predictor checkpoint length is representable");
        let mut payload = Vec::with_capacity(expected_len);

        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(CHECKPOINT_VERSION);
        payload.extend_from_slice(&(table_entries as u32).to_le_bytes());
        payload.push(self.snapshot.config.history_bits());
        payload.extend_from_slice(&self.snapshot.update_count.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.committed_history.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.speculative_history.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.next_speculation.get().to_le_bytes());
        payload.extend_from_slice(&(self.snapshot.pending_speculations.len() as u32).to_le_bytes());
        payload.extend_from_slice(&(self.active_speculations.len() as u32).to_le_bytes());

        payload.extend(self.snapshot.counters.iter().copied());
        for target in &self.snapshot.targets {
            encode_address_option(&mut payload, *target);
        }
        for speculation in &self.snapshot.pending_speculations {
            encode_speculation(&mut payload, speculation);
        }
        for (sequence, id) in &self.active_speculations {
            payload.extend_from_slice(&sequence.to_le_bytes());
            payload.extend_from_slice(&id.get().to_le_bytes());
        }

        debug_assert_eq!(payload.len(), expected_len);
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, BranchPredictorError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(BranchPredictorError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != CHECKPOINT_VERSION {
            return Err(BranchPredictorError::UnsupportedCheckpointVersion { version });
        }
        let table_entries = read_u32(payload, &mut offset)? as usize;
        let history_bits = read_u8(payload, &mut offset)?;
        let update_count = read_u64(payload, &mut offset)?;
        let committed_history = read_u64(payload, &mut offset)?;
        let speculative_history = read_u64(payload, &mut offset)?;
        let next_speculation = BranchSpeculationId::new(read_u64(payload, &mut offset)?);
        let pending_count = read_u32(payload, &mut offset)? as usize;
        let active_count = read_u32(payload, &mut offset)? as usize;

        let expected_len = checkpoint_payload_len(table_entries, pending_count, active_count)?;
        if payload.len() != expected_len {
            return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: expected_len,
                actual: payload.len(),
            });
        }

        let config = BranchPredictorConfig::with_history_bits(table_entries, history_bits)?;
        let counters = read_counters(payload, &mut offset, table_entries)?;
        let targets = read_targets(payload, &mut offset, table_entries)?;
        let pending_speculations =
            read_speculations(payload, &mut offset, table_entries, pending_count)?;
        let active_speculations = read_active_speculations(payload, &mut offset, active_count)?;
        debug_assert_eq!(offset, payload.len());

        Self::from_snapshot(
            BranchPredictorSnapshot {
                config,
                counters,
                targets,
                update_count,
                committed_history,
                speculative_history,
                next_speculation,
                pending_speculations,
            },
            active_speculations,
        )
    }
}

fn validate_checkpoint_snapshot(
    snapshot: &BranchPredictorSnapshot,
    active_speculations: &[(u64, BranchSpeculationId)],
) -> Result<(), BranchPredictorError> {
    let table_entries = snapshot.config.table_entries();
    require_u32("table-entries", table_entries)?;
    require_u32("pending-speculations", snapshot.pending_speculations.len())?;
    require_u32("active-speculations", active_speculations.len())?;
    if snapshot.counters.len() != table_entries {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: table_entries,
            actual: snapshot.counters.len(),
        });
    }
    if snapshot.targets.len() != table_entries {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: table_entries,
            actual: snapshot.targets.len(),
        });
    }
    for counter in &snapshot.counters {
        validate_checkpoint_counter(*counter)?;
    }

    let mut pending_ids = BTreeSet::new();
    let mut ordered_pending_ids = Vec::with_capacity(snapshot.pending_speculations.len());
    for speculation in &snapshot.pending_speculations {
        validate_checkpoint_speculation(speculation, table_entries)?;
        if !pending_ids.insert(speculation.id()) {
            return Err(BranchPredictorError::DuplicateCheckpointSpeculationId {
                id: speculation.id(),
            });
        }
        ordered_pending_ids.push(speculation.id());
        if snapshot.next_speculation <= speculation.id() {
            return Err(BranchPredictorError::InvalidCheckpointNextSpeculation {
                next: snapshot.next_speculation,
                pending: speculation.id(),
            });
        }
    }
    if snapshot.next_speculation.get() == u64::MAX {
        return Err(
            BranchPredictorError::InvalidCheckpointNextSpeculationOverflow {
                next: snapshot.next_speculation,
            },
        );
    }

    let mut active_sequences = BTreeSet::new();
    let mut active_ids = BTreeSet::new();
    for (order, (sequence, id)) in active_speculations.iter().enumerate() {
        if !active_sequences.insert(*sequence) {
            return Err(
                BranchPredictorError::DuplicateCheckpointSpeculationSequence {
                    sequence: *sequence,
                },
            );
        }
        if !active_ids.insert(*id) {
            return Err(BranchPredictorError::DuplicateCheckpointSpeculationId { id: *id });
        }
        if !pending_ids.contains(id) {
            return Err(BranchPredictorError::MissingCheckpointSpeculation { id: *id });
        }
        let Some(expected) = ordered_pending_ids.get(order).copied() else {
            return Err(BranchPredictorError::MissingCheckpointSpeculation { id: *id });
        };
        if *id != expected {
            return Err(BranchPredictorError::InvalidCheckpointSpeculationOrder {
                sequence: *sequence,
                id: *id,
                expected,
            });
        }
    }
    for id in pending_ids {
        if !active_ids.contains(&id) {
            return Err(BranchPredictorError::UnmappedCheckpointSpeculation { id });
        }
    }

    checkpoint_payload_len(
        table_entries,
        snapshot.pending_speculations.len(),
        active_speculations.len(),
    )?;
    Ok(())
}

fn validate_checkpoint_speculation(
    speculation: &BranchSpeculation,
    table_entries: usize,
) -> Result<(), BranchPredictorError> {
    let index = speculation.prediction.index;
    if index >= table_entries {
        return Err(BranchPredictorError::InvalidCheckpointSpeculationIndex {
            index,
            table_entries,
        });
    }
    let expected = branch_checkpoint_index(speculation.prediction.pc, table_entries);
    if index != expected {
        return Err(BranchPredictorError::InvalidCheckpointSpeculationPcIndex {
            pc: speculation.prediction.pc,
            index,
            expected,
        });
    }
    validate_checkpoint_counter(speculation.prediction.counter)
}

fn encode_speculation(payload: &mut Vec<u8>, speculation: &BranchSpeculation) {
    payload.extend_from_slice(&speculation.id.get().to_le_bytes());
    payload.extend_from_slice(&speculation.prediction.pc.get().to_le_bytes());
    payload.extend_from_slice(&(speculation.prediction.index as u32).to_le_bytes());
    payload.push(bool_flag(speculation.prediction.predicted_taken));
    encode_address_option(payload, speculation.prediction.target);
    payload.push(speculation.prediction.counter);
    payload.extend_from_slice(&speculation.history_before.to_le_bytes());
    payload.extend_from_slice(&speculation.history_after.to_le_bytes());
    payload.push(bool_flag(speculation.history_taken));
    payload.push(bool_flag(speculation.repaired));
}

fn read_speculations(
    payload: &[u8],
    offset: &mut usize,
    table_entries: usize,
    count: usize,
) -> Result<Vec<BranchSpeculation>, BranchPredictorError> {
    let mut speculations = Vec::with_capacity(count);
    for _ in 0..count {
        let id = BranchSpeculationId::new(read_u64(payload, offset)?);
        let pc = Address::new(read_u64(payload, offset)?);
        let index = read_u32(payload, offset)? as usize;
        let predicted_taken = read_bool(payload, offset, "predicted-taken")?;
        let target = read_address_option(payload, offset, "target")?;
        let counter = read_u8(payload, offset)?;
        validate_checkpoint_counter(counter)?;
        let history_before = read_u64(payload, offset)?;
        let history_after = read_u64(payload, offset)?;
        let history_taken = read_bool(payload, offset, "history-taken")?;
        let repaired = read_bool(payload, offset, "repaired")?;

        if index >= table_entries {
            return Err(BranchPredictorError::InvalidCheckpointSpeculationIndex {
                index,
                table_entries,
            });
        }
        let expected = branch_checkpoint_index(pc, table_entries);
        if index != expected {
            return Err(BranchPredictorError::InvalidCheckpointSpeculationPcIndex {
                pc,
                index,
                expected,
            });
        }

        speculations.push(BranchSpeculation {
            id,
            prediction: BranchPrediction {
                pc,
                index,
                predicted_taken,
                target,
                counter,
            },
            history_before,
            history_after,
            history_taken,
            repaired,
        });
    }
    Ok(speculations)
}

fn read_counters(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<u8>, BranchPredictorError> {
    let end = checked_offset(*offset, count)?;
    let counters = payload
        .get(*offset..end)
        .ok_or(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        })?
        .to_vec();
    for counter in &counters {
        validate_checkpoint_counter(*counter)?;
    }
    *offset = end;
    Ok(counters)
}

fn read_targets(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<Option<Address>>, BranchPredictorError> {
    let mut targets = Vec::with_capacity(count);
    for _ in 0..count {
        targets.push(read_address_option(payload, offset, "target")?);
    }
    Ok(targets)
}

fn read_active_speculations(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<(u64, BranchSpeculationId)>, BranchPredictorError> {
    let mut active_speculations = Vec::with_capacity(count);
    for _ in 0..count {
        let sequence = read_u64(payload, offset)?;
        let id = BranchSpeculationId::new(read_u64(payload, offset)?);
        active_speculations.push((sequence, id));
    }
    Ok(active_speculations)
}

fn encode_address_option(payload: &mut Vec<u8>, address: Option<Address>) {
    match address {
        Some(address) => {
            payload.push(1);
            payload.extend_from_slice(&address.get().to_le_bytes());
        }
        None => {
            payload.push(0);
            payload.extend_from_slice(&0_u64.to_le_bytes());
        }
    }
}

fn read_address_option(
    payload: &[u8],
    offset: &mut usize,
    name: &'static str,
) -> Result<Option<Address>, BranchPredictorError> {
    let flag = read_u8(payload, offset)?;
    let value = read_u64(payload, offset)?;
    match flag {
        0 => Ok(None),
        1 => Ok(Some(Address::new(value))),
        value => Err(BranchPredictorError::InvalidCheckpointFlag { name, value }),
    }
}

fn checkpoint_payload_len(
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    let counter_bytes = checked_product("counter-table", table_entries, CHECKPOINT_COUNTER_BYTES)?;
    let target_bytes = checked_product("target-table", table_entries, CHECKPOINT_TARGET_BYTES)?;
    let pending_bytes = checked_product(
        "pending-speculations",
        pending_count,
        CHECKPOINT_PENDING_SPECULATION_BYTES,
    )?;
    let active_bytes = checked_product(
        "active-speculations",
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_BYTES,
    )?;
    let len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let len = checked_sum("payload-size", len, target_bytes)?;
    let len = checked_sum("payload-size", len, pending_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

fn checked_product(
    name: &'static str,
    count: usize,
    bytes: usize,
) -> Result<usize, BranchPredictorError> {
    count
        .checked_mul(bytes)
        .ok_or(BranchPredictorError::CheckpointValueTooLarge {
            name,
            value: count,
            max: usize::MAX / bytes,
        })
}

fn checked_sum(
    name: &'static str,
    base: usize,
    increment: usize,
) -> Result<usize, BranchPredictorError> {
    base.checked_add(increment)
        .ok_or(BranchPredictorError::CheckpointValueTooLarge {
            name,
            value: increment,
            max: usize::MAX - base,
        })
}

fn require_u32(name: &'static str, value: usize) -> Result<u32, BranchPredictorError> {
    u32::try_from(value).map_err(|_| BranchPredictorError::CheckpointValueTooLarge {
        name,
        value,
        max: CHECKPOINT_U32_MAX,
    })
}

fn validate_checkpoint_counter(value: u8) -> Result<(), BranchPredictorError> {
    if value > MAX_CHECKPOINT_COUNTER {
        return Err(BranchPredictorError::InvalidCheckpointCounter { value });
    }
    Ok(())
}

fn branch_checkpoint_index(pc: Address, table_entries: usize) -> usize {
    ((pc.get() >> 2) % table_entries as u64) as usize
}

fn bool_flag(value: bool) -> u8 {
    u8::from(value)
}

fn read_bool(
    payload: &[u8],
    offset: &mut usize,
    name: &'static str,
) -> Result<bool, BranchPredictorError> {
    match read_u8(payload, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(BranchPredictorError::InvalidCheckpointFlag { name, value }),
    }
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, BranchPredictorError> {
    let value =
        *payload
            .get(*offset)
            .ok_or(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: *offset + 1,
                actual: payload.len(),
            })?;
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, BranchPredictorError> {
    Ok(u32::from_le_bytes(read_array(payload, offset)?))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, BranchPredictorError> {
    Ok(u64::from_le_bytes(read_array(payload, offset)?))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], BranchPredictorError> {
    let end = checked_offset(*offset, N)?;
    let bytes =
        payload
            .get(*offset..end)
            .ok_or(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: end,
                actual: payload.len(),
            })?;
    *offset = end;
    Ok(bytes.try_into().expect("slice length matches array length"))
}

fn checked_offset(base: usize, increment: usize) -> Result<usize, BranchPredictorError> {
    base.checked_add(increment)
        .ok_or(BranchPredictorError::CheckpointValueTooLarge {
            name: "payload-offset",
            value: increment,
            max: usize::MAX - base,
        })
}
