use std::collections::BTreeSet;

use rem6_memory::Address;

use crate::{
    branch_predictor::{
        BranchPrediction, BranchPredictorConfig, BranchPredictorError, BranchPredictorSnapshot,
        BranchSpeculation, BranchSpeculationId, BranchTargetBuffer, BranchTargetBufferConfig,
        BranchTargetBufferError, BranchTargetBufferSnapshot, BranchTargetEntry, BranchTargetKind,
        BranchTargetPrediction,
    },
    return_address_stack::{
        ReturnAddressStack, ReturnAddressStackConfig, ReturnAddressStackOperation,
        ReturnAddressStackOperationId, ReturnAddressStackOperationKind, ReturnAddressStackSnapshot,
    },
    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY, DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
    DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES,
};

const MAX_CHECKPOINT_COUNTER: u8 = 3;
const CHECKPOINT_MAGIC: [u8; 4] = *b"RIBP";
const LEGACY_CHECKPOINT_VERSION: u8 = 1;
const V2_CHECKPOINT_VERSION: u8 = 2;
const V3_CHECKPOINT_VERSION: u8 = 3;
const CHECKPOINT_VERSION: u8 = 4;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize =
    CHECKPOINT_MAGIC.len() + 1 + U32_BYTES + 1 + U64_BYTES * 4 + U32_BYTES * 2;
const CHECKPOINT_COUNTER_BYTES: usize = 1;
const CHECKPOINT_TARGET_BYTES: usize = 1 + U64_BYTES;
const CHECKPOINT_BTB_HEADER_BYTES: usize = U32_BYTES * 2 + U64_BYTES * 6;
const CHECKPOINT_BTB_ENTRY_BYTES: usize = 1 + U64_BYTES * 3 + 1;
const CHECKPOINT_RAS_HEADER_BYTES: usize = U32_BYTES * 3 + U64_BYTES;
const CHECKPOINT_RAS_OPERATION_FIXED_BYTES: usize =
    U64_BYTES + 1 + CHECKPOINT_TARGET_BYTES * 2 + U32_BYTES * 2;
const CHECKPOINT_PENDING_SPECULATION_BYTES: usize =
    U64_BYTES + U64_BYTES + U32_BYTES + 1 + 1 + U64_BYTES + 1 + U64_BYTES + U64_BYTES + 1 + 1;
const CHECKPOINT_ACTIVE_SPECULATION_BYTES: usize = U64_BYTES + U64_BYTES;
const CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES: usize =
    CHECKPOINT_ACTIVE_SPECULATION_BYTES + 1 + 1 + CHECKPOINT_TARGET_BYTES;
const CHECKPOINT_ACTIVE_SPECULATION_V4_BYTES: usize =
    CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES + 1 + U64_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictorCheckpointPayload {
    snapshot: BranchPredictorSnapshot,
    branch_target_buffer: BranchTargetBufferSnapshot,
    return_address_stack: ReturnAddressStackSnapshot,
    active_speculations: Vec<(u64, BranchSpeculationId)>,
    active_branch_target_predictions: Vec<(u64, BranchTargetPrediction)>,
    active_return_address_stack_operations: Vec<(u64, ReturnAddressStackOperationId)>,
}

impl BranchPredictorCheckpointPayload {
    pub fn from_snapshot<I>(
        snapshot: BranchPredictorSnapshot,
        active_speculations: I,
    ) -> Result<Self, BranchPredictorError>
    where
        I: IntoIterator<Item = (u64, BranchSpeculationId)>,
    {
        Self::from_snapshots(
            snapshot,
            default_branch_target_buffer_snapshot(),
            active_speculations,
        )
    }

    pub fn from_snapshots<I>(
        snapshot: BranchPredictorSnapshot,
        branch_target_buffer: BranchTargetBufferSnapshot,
        active_speculations: I,
    ) -> Result<Self, BranchPredictorError>
    where
        I: IntoIterator<Item = (u64, BranchSpeculationId)>,
    {
        Self::from_snapshots_with_branch_target_predictions(
            snapshot,
            branch_target_buffer,
            active_speculations,
            std::iter::empty::<(u64, BranchTargetPrediction)>(),
        )
    }

    pub fn from_snapshots_with_branch_target_predictions<I, P>(
        snapshot: BranchPredictorSnapshot,
        branch_target_buffer: BranchTargetBufferSnapshot,
        active_speculations: I,
        active_branch_target_predictions: P,
    ) -> Result<Self, BranchPredictorError>
    where
        I: IntoIterator<Item = (u64, BranchSpeculationId)>,
        P: IntoIterator<Item = (u64, BranchTargetPrediction)>,
    {
        Self::from_snapshots_with_branch_target_predictions_and_return_address_stack(
            snapshot,
            branch_target_buffer,
            active_speculations,
            active_branch_target_predictions,
            default_return_address_stack_snapshot(),
            std::iter::empty::<(u64, ReturnAddressStackOperationId)>(),
        )
    }

    pub fn from_snapshots_with_branch_target_predictions_and_return_address_stack<I, P, R>(
        snapshot: BranchPredictorSnapshot,
        branch_target_buffer: BranchTargetBufferSnapshot,
        active_speculations: I,
        active_branch_target_predictions: P,
        return_address_stack: ReturnAddressStackSnapshot,
        active_return_address_stack_operations: R,
    ) -> Result<Self, BranchPredictorError>
    where
        I: IntoIterator<Item = (u64, BranchSpeculationId)>,
        P: IntoIterator<Item = (u64, BranchTargetPrediction)>,
        R: IntoIterator<Item = (u64, ReturnAddressStackOperationId)>,
    {
        let mut active_speculations = active_speculations.into_iter().collect::<Vec<_>>();
        active_speculations.sort_by_key(|(sequence, id)| (*sequence, id.get()));
        let mut active_branch_target_predictions = active_branch_target_predictions
            .into_iter()
            .collect::<Vec<_>>();
        active_branch_target_predictions.sort_by_key(|(sequence, _)| *sequence);
        let mut active_return_address_stack_operations = active_return_address_stack_operations
            .into_iter()
            .collect::<Vec<_>>();
        active_return_address_stack_operations.sort_by_key(|(sequence, id)| (*sequence, id.get()));
        validate_checkpoint_snapshot(
            &snapshot,
            &branch_target_buffer,
            &return_address_stack,
            &active_speculations,
            &active_branch_target_predictions,
            &active_return_address_stack_operations,
        )?;
        Ok(Self {
            snapshot,
            branch_target_buffer,
            return_address_stack,
            active_speculations,
            active_branch_target_predictions,
            active_return_address_stack_operations,
        })
    }

    pub const fn snapshot(&self) -> &BranchPredictorSnapshot {
        &self.snapshot
    }

    pub const fn branch_target_buffer_snapshot(&self) -> &BranchTargetBufferSnapshot {
        &self.branch_target_buffer
    }

    pub const fn return_address_stack_snapshot(&self) -> &ReturnAddressStackSnapshot {
        &self.return_address_stack
    }

    pub fn active_speculations(&self) -> &[(u64, BranchSpeculationId)] {
        &self.active_speculations
    }

    pub fn active_branch_target_predictions(&self) -> &[(u64, BranchTargetPrediction)] {
        &self.active_branch_target_predictions
    }

    pub fn active_return_address_stack_operations(
        &self,
    ) -> &[(u64, ReturnAddressStackOperationId)] {
        &self.active_return_address_stack_operations
    }

    pub fn into_parts(
        self,
    ) -> (
        BranchPredictorSnapshot,
        BranchTargetBufferSnapshot,
        Vec<(u64, BranchSpeculationId)>,
    ) {
        (
            self.snapshot,
            self.branch_target_buffer,
            self.active_speculations,
        )
    }

    pub(crate) fn into_parts_with_branch_target_predictions(
        self,
    ) -> (
        BranchPredictorSnapshot,
        BranchTargetBufferSnapshot,
        ReturnAddressStackSnapshot,
        Vec<(u64, BranchSpeculationId)>,
        Vec<(u64, BranchTargetPrediction)>,
        Vec<(u64, ReturnAddressStackOperationId)>,
    ) {
        (
            self.snapshot,
            self.branch_target_buffer,
            self.return_address_stack,
            self.active_speculations,
            self.active_branch_target_predictions,
            self.active_return_address_stack_operations,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        let table_entries = self.snapshot.config.table_entries();
        let expected_len = checkpoint_payload_len(
            table_entries,
            self.branch_target_buffer.config().entries(),
            &self.return_address_stack,
            self.snapshot.pending_speculations.len(),
            self.active_speculations.len(),
            CHECKPOINT_VERSION,
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
        encode_branch_target_buffer(&mut payload, &self.branch_target_buffer);
        encode_return_address_stack(&mut payload, &self.return_address_stack);
        for (sequence, id) in &self.active_speculations {
            payload.extend_from_slice(&sequence.to_le_bytes());
            payload.extend_from_slice(&id.get().to_le_bytes());
            if let Some((_, prediction)) = self
                .active_branch_target_predictions
                .iter()
                .find(|(prediction_sequence, _)| prediction_sequence == sequence)
            {
                payload.push(1);
                payload.push(bool_flag(prediction.hit()));
                encode_address_option(&mut payload, prediction.target());
            } else {
                payload.push(0);
                payload.push(0);
                encode_address_option(&mut payload, None);
            }
            if let Some((_, operation_id)) = self
                .active_return_address_stack_operations
                .iter()
                .find(|(operation_sequence, _)| operation_sequence == sequence)
            {
                payload.push(1);
                payload.extend_from_slice(&operation_id.get().to_le_bytes());
            } else {
                payload.push(0);
                payload.extend_from_slice(&0_u64.to_le_bytes());
            }
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
        if !matches!(
            version,
            LEGACY_CHECKPOINT_VERSION
                | V2_CHECKPOINT_VERSION
                | V3_CHECKPOINT_VERSION
                | CHECKPOINT_VERSION
        ) {
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

        let expected_len = if version == LEGACY_CHECKPOINT_VERSION {
            legacy_checkpoint_payload_len(table_entries, pending_count, active_count)?
        } else if version == V2_CHECKPOINT_VERSION {
            v2_checkpoint_payload_len(payload, table_entries, pending_count, active_count)?
        } else if version == V3_CHECKPOINT_VERSION {
            v3_checkpoint_payload_len(payload, table_entries, pending_count, active_count)?
        } else {
            v4_checkpoint_payload_len(payload, table_entries, pending_count, active_count)?
        };
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
        let branch_target_buffer = if version == LEGACY_CHECKPOINT_VERSION {
            default_branch_target_buffer_snapshot()
        } else {
            read_branch_target_buffer(payload, &mut offset)?
        };
        let return_address_stack = if version == CHECKPOINT_VERSION {
            read_return_address_stack(payload, &mut offset)?
        } else {
            default_return_address_stack_snapshot()
        };
        let (
            active_speculations,
            active_branch_target_predictions,
            active_return_address_stack_operations,
        ) = if version == CHECKPOINT_VERSION {
            read_active_speculations_v4(payload, &mut offset, active_count)?
        } else if version == V3_CHECKPOINT_VERSION {
            let (active_speculations, active_branch_target_predictions) =
                read_active_speculations_v3(payload, &mut offset, active_count)?;
            (
                active_speculations,
                active_branch_target_predictions,
                Vec::new(),
            )
        } else {
            (
                read_active_speculations(payload, &mut offset, active_count)?,
                Vec::new(),
                Vec::new(),
            )
        };
        if offset != payload.len() {
            return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshots_with_branch_target_predictions_and_return_address_stack(
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
            branch_target_buffer,
            active_speculations,
            active_branch_target_predictions,
            return_address_stack,
            active_return_address_stack_operations,
        )
    }
}

fn default_branch_target_buffer_snapshot() -> BranchTargetBufferSnapshot {
    BranchTargetBuffer::new(
        BranchTargetBufferConfig::new(
            DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
            DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY,
        )
        .expect("default RISC-V branch target buffer config is valid"),
    )
    .snapshot()
}

fn default_return_address_stack_snapshot() -> ReturnAddressStackSnapshot {
    ReturnAddressStack::new(
        ReturnAddressStackConfig::new(DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES)
            .expect("default RISC-V return-address stack config is valid"),
    )
    .snapshot()
}

fn validate_checkpoint_snapshot(
    snapshot: &BranchPredictorSnapshot,
    branch_target_buffer: &BranchTargetBufferSnapshot,
    return_address_stack: &ReturnAddressStackSnapshot,
    active_speculations: &[(u64, BranchSpeculationId)],
    active_branch_target_predictions: &[(u64, BranchTargetPrediction)],
    active_return_address_stack_operations: &[(u64, ReturnAddressStackOperationId)],
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
    let mut branch_target_prediction_sequences = BTreeSet::new();
    for (sequence, _prediction) in active_branch_target_predictions {
        if !branch_target_prediction_sequences.insert(*sequence) {
            return Err(
                BranchPredictorError::DuplicateCheckpointSpeculationSequence {
                    sequence: *sequence,
                },
            );
        }
        if !active_sequences.contains(sequence) {
            return Err(
                BranchPredictorError::DuplicateCheckpointSpeculationSequence {
                    sequence: *sequence,
                },
            );
        }
    }
    validate_return_address_stack_snapshot(return_address_stack)?;
    let ras_pending_ids = return_address_stack
        .pending_operations()
        .iter()
        .map(|operation| operation.id())
        .collect::<BTreeSet<_>>();
    let mut ras_active_sequences = BTreeSet::new();
    let mut ras_active_ids = BTreeSet::new();
    for (sequence, id) in active_return_address_stack_operations {
        if !ras_active_sequences.insert(*sequence) {
            return Err(
                BranchPredictorError::DuplicateCheckpointSpeculationSequence {
                    sequence: *sequence,
                },
            );
        }
        if !active_sequences.contains(sequence) {
            return Err(
                BranchPredictorError::DuplicateCheckpointSpeculationSequence {
                    sequence: *sequence,
                },
            );
        }
        if !ras_active_ids.insert(*id) {
            return Err(
                BranchPredictorError::DuplicateCheckpointReturnAddressStackOperation { id: *id },
            );
        }
        if !ras_pending_ids.contains(id) {
            return Err(
                BranchPredictorError::MissingCheckpointReturnAddressStackOperation { id: *id },
            );
        }
    }
    for id in ras_pending_ids {
        if !ras_active_ids.contains(&id) {
            return Err(BranchPredictorError::UnmappedCheckpointReturnAddressStackOperation { id });
        }
    }
    let active_ras_operation_order = active_speculations
        .iter()
        .filter_map(|(sequence, _)| {
            active_return_address_stack_operations
                .iter()
                .find(|(operation_sequence, _)| operation_sequence == sequence)
                .map(|(_, id)| *id)
        })
        .collect::<Vec<_>>();
    let pending_ras_operation_order = return_address_stack
        .pending_operations()
        .iter()
        .map(|operation| operation.id())
        .collect::<Vec<_>>();
    for (id, expected) in active_ras_operation_order
        .iter()
        .zip(pending_ras_operation_order.iter())
    {
        if id != expected {
            return Err(
                BranchPredictorError::InvalidCheckpointReturnAddressStackOperationOrder {
                    id: *id,
                    expected: *expected,
                },
            );
        }
    }

    checkpoint_payload_len(
        table_entries,
        branch_target_buffer.config().entries(),
        return_address_stack,
        snapshot.pending_speculations.len(),
        active_speculations.len(),
        CHECKPOINT_VERSION,
    )?;
    validate_branch_target_buffer_snapshot(branch_target_buffer)?;
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

fn read_active_speculations_v3(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<
    (
        Vec<(u64, BranchSpeculationId)>,
        Vec<(u64, BranchTargetPrediction)>,
    ),
    BranchPredictorError,
> {
    let mut active_speculations = Vec::with_capacity(count);
    let mut active_branch_target_predictions = Vec::new();
    for _ in 0..count {
        let sequence = read_u64(payload, offset)?;
        let id = BranchSpeculationId::new(read_u64(payload, offset)?);
        let has_prediction = read_bool(payload, offset, "branch-target-prediction")?;
        let hit = read_bool(payload, offset, "branch-target-hit")?;
        let target = read_address_option(payload, offset, "branch-target")?;
        active_speculations.push((sequence, id));
        if has_prediction {
            active_branch_target_predictions
                .push((sequence, BranchTargetPrediction::new(hit, target)));
        }
    }
    Ok((active_speculations, active_branch_target_predictions))
}

fn read_active_speculations_v4(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<
    (
        Vec<(u64, BranchSpeculationId)>,
        Vec<(u64, BranchTargetPrediction)>,
        Vec<(u64, ReturnAddressStackOperationId)>,
    ),
    BranchPredictorError,
> {
    let mut active_speculations = Vec::with_capacity(count);
    let mut active_branch_target_predictions = Vec::new();
    let mut active_return_address_stack_operations = Vec::new();
    for _ in 0..count {
        let sequence = read_u64(payload, offset)?;
        let id = BranchSpeculationId::new(read_u64(payload, offset)?);
        let has_prediction = read_bool(payload, offset, "branch-target-prediction")?;
        let hit = read_bool(payload, offset, "branch-target-hit")?;
        let target = read_address_option(payload, offset, "branch-target")?;
        let has_ras_operation = read_bool(payload, offset, "return-address-stack-operation")?;
        let ras_operation = ReturnAddressStackOperationId::new(read_u64(payload, offset)?);
        active_speculations.push((sequence, id));
        if has_prediction {
            active_branch_target_predictions
                .push((sequence, BranchTargetPrediction::new(hit, target)));
        }
        if has_ras_operation {
            active_return_address_stack_operations.push((sequence, ras_operation));
        }
    }
    Ok((
        active_speculations,
        active_branch_target_predictions,
        active_return_address_stack_operations,
    ))
}

fn encode_return_address_stack(payload: &mut Vec<u8>, snapshot: &ReturnAddressStackSnapshot) {
    payload.extend_from_slice(&(snapshot.config().entries() as u32).to_le_bytes());
    payload.extend_from_slice(&(snapshot.stack_entries().len() as u32).to_le_bytes());
    payload.extend_from_slice(&(snapshot.pending_operations().len() as u32).to_le_bytes());
    payload.extend_from_slice(&snapshot.next_operation().get().to_le_bytes());
    for address in snapshot.stack_entries() {
        payload.extend_from_slice(&address.get().to_le_bytes());
    }
    for operation in snapshot.pending_operations() {
        encode_return_address_stack_operation(payload, operation);
    }
}

fn encode_return_address_stack_operation(
    payload: &mut Vec<u8>,
    operation: &ReturnAddressStackOperation,
) {
    payload.extend_from_slice(&operation.id().get().to_le_bytes());
    payload.push(encode_return_address_stack_operation_kind(operation.kind()));
    encode_address_option(payload, operation.pushed_address());
    encode_address_option(payload, operation.predicted_return());
    payload.extend_from_slice(&(operation.stack_before().len() as u32).to_le_bytes());
    payload.extend_from_slice(&(operation.stack_after().len() as u32).to_le_bytes());
    for address in operation.stack_before() {
        payload.extend_from_slice(&address.get().to_le_bytes());
    }
    for address in operation.stack_after() {
        payload.extend_from_slice(&address.get().to_le_bytes());
    }
}

fn read_return_address_stack(
    payload: &[u8],
    offset: &mut usize,
) -> Result<ReturnAddressStackSnapshot, BranchPredictorError> {
    let entries = read_u32(payload, offset)? as usize;
    let stack_count = read_u32(payload, offset)? as usize;
    let pending_count = read_u32(payload, offset)? as usize;
    let next_operation = ReturnAddressStackOperationId::new(read_u64(payload, offset)?);
    let config = ReturnAddressStackConfig::new(entries)
        .map_err(|error| BranchPredictorError::InvalidReturnAddressStackCheckpoint { error })?;
    let stack = read_address_vec(payload, offset, stack_count)?;
    let mut pending_operations = Vec::with_capacity(pending_count);
    for _ in 0..pending_count {
        pending_operations.push(read_return_address_stack_operation(payload, offset)?);
    }
    let snapshot = ReturnAddressStackSnapshot::from_checkpoint_parts(
        config,
        stack,
        next_operation,
        pending_operations,
    );
    validate_return_address_stack_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn read_return_address_stack_operation(
    payload: &[u8],
    offset: &mut usize,
) -> Result<ReturnAddressStackOperation, BranchPredictorError> {
    let id = ReturnAddressStackOperationId::new(read_u64(payload, offset)?);
    let kind = read_return_address_stack_operation_kind(payload, offset)?;
    let pushed_address = read_address_option(payload, offset, "return-address-stack-pushed")?;
    let predicted_return =
        read_address_option(payload, offset, "return-address-stack-predicted-return")?;
    let stack_before_count = read_u32(payload, offset)? as usize;
    let stack_after_count = read_u32(payload, offset)? as usize;
    let stack_before = read_address_vec(payload, offset, stack_before_count)?;
    let stack_after = read_address_vec(payload, offset, stack_after_count)?;
    Ok(ReturnAddressStackOperation::from_checkpoint_parts(
        id,
        kind,
        pushed_address,
        predicted_return,
        stack_before,
        stack_after,
    ))
}

fn read_address_vec(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<Address>, BranchPredictorError> {
    let mut addresses = Vec::with_capacity(count);
    for _ in 0..count {
        addresses.push(Address::new(read_u64(payload, offset)?));
    }
    Ok(addresses)
}

fn encode_branch_target_buffer(payload: &mut Vec<u8>, snapshot: &BranchTargetBufferSnapshot) {
    payload.extend_from_slice(&(snapshot.config().entries() as u32).to_le_bytes());
    payload.extend_from_slice(&(snapshot.config().associativity() as u32).to_le_bytes());
    payload.extend_from_slice(&snapshot.access_sequence().to_le_bytes());
    payload.extend_from_slice(&snapshot.lookup_count().to_le_bytes());
    payload.extend_from_slice(&snapshot.hit_count().to_le_bytes());
    payload.extend_from_slice(&snapshot.miss_count().to_le_bytes());
    payload.extend_from_slice(&snapshot.update_count().to_le_bytes());
    payload.extend_from_slice(&snapshot.eviction_count().to_le_bytes());
    for entry in snapshot.entries() {
        match entry {
            Some(entry) => {
                payload.push(1);
                payload.extend_from_slice(&entry.pc().get().to_le_bytes());
                payload.extend_from_slice(&entry.target().get().to_le_bytes());
                payload.push(encode_branch_target_kind(entry.kind()));
                payload.extend_from_slice(&entry.last_used().to_le_bytes());
            }
            None => {
                payload.push(0);
                payload.extend_from_slice(&0_u64.to_le_bytes());
                payload.extend_from_slice(&0_u64.to_le_bytes());
                payload.push(encode_branch_target_kind(BranchTargetKind::NoBranch));
                payload.extend_from_slice(&0_u64.to_le_bytes());
            }
        }
    }
}

fn read_branch_target_buffer(
    payload: &[u8],
    offset: &mut usize,
) -> Result<BranchTargetBufferSnapshot, BranchPredictorError> {
    let entries = read_u32(payload, offset)? as usize;
    let associativity = read_u32(payload, offset)? as usize;
    let access_sequence = read_u64(payload, offset)?;
    let lookup_count = read_u64(payload, offset)?;
    let hit_count = read_u64(payload, offset)?;
    let miss_count = read_u64(payload, offset)?;
    let update_count = read_u64(payload, offset)?;
    let eviction_count = read_u64(payload, offset)?;
    let config = BranchTargetBufferConfig::new(entries, associativity)
        .map_err(|error| BranchPredictorError::InvalidBranchTargetBufferCheckpoint { error })?;

    let mut snapshot_entries = Vec::with_capacity(entries);
    for index in 0..entries {
        let valid = read_bool(payload, offset, "branch-target-valid")?;
        let pc = Address::new(read_u64(payload, offset)?);
        let target = Address::new(read_u64(payload, offset)?);
        let kind = read_branch_target_kind(payload, offset)?;
        let last_used = read_u64(payload, offset)?;
        if valid {
            let set = index / associativity;
            let way = index % associativity;
            snapshot_entries.push(Some(BranchTargetEntry {
                pc,
                target,
                kind,
                set,
                way,
                last_used,
            }));
        } else {
            snapshot_entries.push(None);
        }
    }

    Ok(BranchTargetBufferSnapshot {
        config,
        entries: snapshot_entries,
        access_sequence,
        lookup_count,
        hit_count,
        miss_count,
        update_count,
        eviction_count,
    })
}

fn validate_branch_target_buffer_snapshot(
    snapshot: &BranchTargetBufferSnapshot,
) -> Result<(), BranchPredictorError> {
    BranchTargetBufferConfig::new(
        snapshot.config().entries(),
        snapshot.config().associativity(),
    )
    .map_err(|error| BranchPredictorError::InvalidBranchTargetBufferCheckpoint { error })?;
    require_u32("branch-target-buffer-entries", snapshot.config().entries())?;
    require_u32(
        "branch-target-buffer-associativity",
        snapshot.config().associativity(),
    )?;
    branch_target_buffer_checkpoint_len(snapshot.config().entries())?;
    if snapshot.entries().len() != snapshot.config().entries() {
        return Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint {
            error: BranchTargetBufferError::SnapshotShapeMismatch {
                expected_entries: snapshot.config().entries(),
                expected_associativity: snapshot.config().associativity(),
                actual_entries: snapshot.entries().len(),
                actual_associativity: snapshot.config().associativity(),
            },
        });
    }
    let mut seen_pcs = BTreeSet::new();
    for (index, entry) in snapshot.entries().iter().enumerate() {
        let Some(entry) = entry else {
            continue;
        };
        if !seen_pcs.insert(entry.pc()) {
            return Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint {
                error: BranchTargetBufferError::DuplicateSnapshotEntry(entry.pc()),
            });
        }
        let actual_set = index / snapshot.config().associativity();
        let expected_set = branch_target_buffer_set_index(entry.pc(), snapshot.config());
        if actual_set != expected_set {
            return Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint {
                error: BranchTargetBufferError::SnapshotEntrySetMismatch(
                    entry.pc(),
                    expected_set,
                    actual_set,
                ),
            });
        }
    }
    Ok(())
}

fn validate_return_address_stack_snapshot(
    snapshot: &ReturnAddressStackSnapshot,
) -> Result<(), BranchPredictorError> {
    require_u32("return-address-stack-entries", snapshot.config().entries())?;
    require_u32("return-address-stack-depth", snapshot.stack_entries().len())?;
    require_u32(
        "return-address-stack-pending-operations",
        snapshot.pending_operations().len(),
    )?;
    if snapshot.stack_entries().len() > snapshot.config().entries() {
        return Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackDepth {
                depth: snapshot.stack_entries().len(),
                entries: snapshot.config().entries(),
            },
        );
    }
    let mut seen = BTreeSet::new();
    let mut previous: Option<ReturnAddressStackOperationId> = None;
    let mut previous_stack_after: Option<Vec<Address>> = None;
    for operation in snapshot.pending_operations() {
        validate_return_address_stack_operation(snapshot.config().entries(), operation)?;
        if let Some(previous_stack_after) = previous_stack_after.as_ref() {
            if operation.stack_before() != previous_stack_after {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
        }
        if !seen.insert(operation.id()) {
            return Err(
                BranchPredictorError::DuplicateCheckpointReturnAddressStackOperation {
                    id: operation.id(),
                },
            );
        }
        if let Some(previous) = previous {
            if operation.id() <= previous {
                let Some(expected) = previous.get().checked_add(1) else {
                    return Err(
                        BranchPredictorError::InvalidCheckpointReturnAddressStackNextOperationOverflow {
                            next: previous,
                        },
                    );
                };
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperationOrder {
                        id: operation.id(),
                        expected: ReturnAddressStackOperationId::new(expected),
                    },
                );
            }
        }
        if snapshot.next_operation() <= operation.id() {
            return Err(
                BranchPredictorError::InvalidCheckpointReturnAddressStackNextOperation {
                    next: snapshot.next_operation(),
                    pending: operation.id(),
                },
            );
        }
        previous = Some(operation.id());
        previous_stack_after = Some(operation.stack_after().to_vec());
    }
    if let Some(final_stack) = previous_stack_after {
        if final_stack != snapshot.stack_entries() {
            return Err(
                BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                    id: previous.expect("final stack exists only when an operation was seen"),
                },
            );
        }
    }
    if snapshot.next_operation().get() == u64::MAX {
        return Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackNextOperationOverflow {
                next: snapshot.next_operation(),
            },
        );
    }
    return_address_stack_checkpoint_len(snapshot)?;
    Ok(())
}

fn validate_return_address_stack_operation(
    entries: usize,
    operation: &ReturnAddressStackOperation,
) -> Result<(), BranchPredictorError> {
    require_u32(
        "return-address-stack-operation-before",
        operation.stack_before().len(),
    )?;
    require_u32(
        "return-address-stack-operation-after",
        operation.stack_after().len(),
    )?;
    if operation.stack_before().len() > entries {
        return Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackDepth {
                depth: operation.stack_before().len(),
                entries,
            },
        );
    }
    if operation.stack_after().len() > entries {
        return Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackDepth {
                depth: operation.stack_after().len(),
                entries,
            },
        );
    }
    match operation.kind() {
        ReturnAddressStackOperationKind::Push => {
            let Some(pushed) = operation.pushed_address() else {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            };
            if operation.predicted_return().is_some() {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
            let mut expected_after = operation.stack_before().to_vec();
            if expected_after.len() == entries {
                expected_after.remove(0);
            }
            expected_after.push(pushed);
            if operation.stack_after() != expected_after {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
        }
        ReturnAddressStackOperationKind::Pop => {
            if operation.pushed_address().is_some() {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
            let mut expected_after = operation.stack_before().to_vec();
            let predicted_return = expected_after.pop();
            if operation.predicted_return() != predicted_return {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
            if operation.stack_after() != expected_after {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
        }
        ReturnAddressStackOperationKind::PopThenPush => {
            let Some(pushed) = operation.pushed_address() else {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            };
            let mut expected_after = operation.stack_before().to_vec();
            let predicted_return = expected_after.pop();
            if operation.predicted_return() != predicted_return {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
            if expected_after.len() == entries {
                expected_after.remove(0);
            }
            expected_after.push(pushed);
            if operation.stack_after() != expected_after {
                return Err(
                    BranchPredictorError::InvalidCheckpointReturnAddressStackOperation {
                        id: operation.id(),
                    },
                );
            }
        }
    }
    Ok(())
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

const fn encode_branch_target_kind(kind: BranchTargetKind) -> u8 {
    match kind {
        BranchTargetKind::NoBranch => 0,
        BranchTargetKind::DirectConditional => 1,
        BranchTargetKind::DirectUnconditional => 2,
        BranchTargetKind::IndirectConditional => 3,
        BranchTargetKind::IndirectUnconditional => 4,
        BranchTargetKind::CallDirect => 5,
        BranchTargetKind::CallIndirect => 6,
        BranchTargetKind::Return => 7,
    }
}

const fn encode_return_address_stack_operation_kind(kind: ReturnAddressStackOperationKind) -> u8 {
    match kind {
        ReturnAddressStackOperationKind::Push => 0,
        ReturnAddressStackOperationKind::Pop => 1,
        ReturnAddressStackOperationKind::PopThenPush => 2,
    }
}

fn read_branch_target_kind(
    payload: &[u8],
    offset: &mut usize,
) -> Result<BranchTargetKind, BranchPredictorError> {
    match read_u8(payload, offset)? {
        0 => Ok(BranchTargetKind::NoBranch),
        1 => Ok(BranchTargetKind::DirectConditional),
        2 => Ok(BranchTargetKind::DirectUnconditional),
        3 => Ok(BranchTargetKind::IndirectConditional),
        4 => Ok(BranchTargetKind::IndirectUnconditional),
        5 => Ok(BranchTargetKind::CallDirect),
        6 => Ok(BranchTargetKind::CallIndirect),
        7 => Ok(BranchTargetKind::Return),
        value => Err(BranchPredictorError::InvalidCheckpointFlag {
            name: "branch-target-kind",
            value,
        }),
    }
}

fn read_return_address_stack_operation_kind(
    payload: &[u8],
    offset: &mut usize,
) -> Result<ReturnAddressStackOperationKind, BranchPredictorError> {
    match read_u8(payload, offset)? {
        0 => Ok(ReturnAddressStackOperationKind::Push),
        1 => Ok(ReturnAddressStackOperationKind::Pop),
        2 => Ok(ReturnAddressStackOperationKind::PopThenPush),
        value => Err(BranchPredictorError::InvalidCheckpointFlag {
            name: "return-address-stack-operation-kind",
            value,
        }),
    }
}

fn checkpoint_payload_len(
    table_entries: usize,
    branch_target_buffer_entries: usize,
    return_address_stack: &ReturnAddressStackSnapshot,
    pending_count: usize,
    active_count: usize,
    version: u8,
) -> Result<usize, BranchPredictorError> {
    let len = legacy_checkpoint_payload_len(table_entries, pending_count, active_count)?;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries)?;
    let len = checked_sum("payload-size", len, branch_target_buffer_bytes)?;
    if version == V3_CHECKPOINT_VERSION {
        let active_delta = checked_product(
            "active-speculations",
            active_count,
            CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES - CHECKPOINT_ACTIVE_SPECULATION_BYTES,
        )?;
        checked_sum("payload-size", len, active_delta)
    } else if version == CHECKPOINT_VERSION {
        let return_address_stack_bytes = return_address_stack_checkpoint_len(return_address_stack)?;
        let len = checked_sum("payload-size", len, return_address_stack_bytes)?;
        let active_delta = checked_product(
            "active-speculations",
            active_count,
            CHECKPOINT_ACTIVE_SPECULATION_V4_BYTES - CHECKPOINT_ACTIVE_SPECULATION_BYTES,
        )?;
        checked_sum("payload-size", len, active_delta)
    } else {
        Ok(len)
    }
}

fn legacy_checkpoint_payload_len(
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

fn v2_checkpoint_payload_len(
    payload: &[u8],
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
    let btb_offset = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, target_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, pending_bytes)?;
    let btb_header_end = checked_offset(btb_offset, CHECKPOINT_BTB_HEADER_BYTES)?;
    if payload.len() < btb_header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: btb_header_end,
            actual: payload.len(),
        });
    }
    let mut btb_header = btb_offset;
    let branch_target_buffer_entries = read_u32(payload, &mut btb_header)? as usize;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries)?;
    let len = checked_sum("payload-size", btb_offset, branch_target_buffer_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

fn v3_checkpoint_payload_len(
    payload: &[u8],
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
        CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES,
    )?;
    let btb_offset = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, target_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, pending_bytes)?;
    let btb_header_end = checked_offset(btb_offset, CHECKPOINT_BTB_HEADER_BYTES)?;
    if payload.len() < btb_header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: btb_header_end,
            actual: payload.len(),
        });
    }
    let mut btb_header = btb_offset;
    let branch_target_buffer_entries = read_u32(payload, &mut btb_header)? as usize;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries)?;
    let len = checked_sum("payload-size", btb_offset, branch_target_buffer_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

fn v4_checkpoint_payload_len(
    payload: &[u8],
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
        CHECKPOINT_ACTIVE_SPECULATION_V4_BYTES,
    )?;
    let btb_offset = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, target_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, pending_bytes)?;
    let btb_header_end = checked_offset(btb_offset, CHECKPOINT_BTB_HEADER_BYTES)?;
    if payload.len() < btb_header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: btb_header_end,
            actual: payload.len(),
        });
    }
    let mut btb_header = btb_offset;
    let branch_target_buffer_entries = read_u32(payload, &mut btb_header)? as usize;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries)?;
    let ras_offset = checked_sum("payload-size", btb_offset, branch_target_buffer_bytes)?;
    let return_address_stack_bytes =
        return_address_stack_checkpoint_len_from_payload(payload, ras_offset)?;
    let len = checked_sum("payload-size", ras_offset, return_address_stack_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

fn return_address_stack_checkpoint_len(
    snapshot: &ReturnAddressStackSnapshot,
) -> Result<usize, BranchPredictorError> {
    let stack_bytes = checked_product(
        "return-address-stack-entries",
        snapshot.stack_entries().len(),
        U64_BYTES,
    )?;
    let mut len = checked_sum(
        "return-address-stack-size",
        CHECKPOINT_RAS_HEADER_BYTES,
        stack_bytes,
    )?;
    for operation in snapshot.pending_operations() {
        let before_bytes = checked_product(
            "return-address-stack-operation-before",
            operation.stack_before().len(),
            U64_BYTES,
        )?;
        let after_bytes = checked_product(
            "return-address-stack-operation-after",
            operation.stack_after().len(),
            U64_BYTES,
        )?;
        let operation_bytes = checked_sum(
            "return-address-stack-operation-size",
            CHECKPOINT_RAS_OPERATION_FIXED_BYTES,
            before_bytes,
        )?;
        let operation_bytes = checked_sum(
            "return-address-stack-operation-size",
            operation_bytes,
            after_bytes,
        )?;
        len = checked_sum("return-address-stack-size", len, operation_bytes)?;
    }
    Ok(len)
}

fn return_address_stack_checkpoint_len_from_payload(
    payload: &[u8],
    offset: usize,
) -> Result<usize, BranchPredictorError> {
    let header_end = checked_offset(offset, CHECKPOINT_RAS_HEADER_BYTES)?;
    if payload.len() < header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: header_end,
            actual: payload.len(),
        });
    }
    let mut header = offset;
    let _entries = read_u32(payload, &mut header)? as usize;
    let stack_count = read_u32(payload, &mut header)? as usize;
    let pending_count = read_u32(payload, &mut header)? as usize;
    let _next_operation = read_u64(payload, &mut header)?;
    let stack_bytes = checked_product("return-address-stack-entries", stack_count, U64_BYTES)?;
    let mut ras_len = checked_sum(
        "return-address-stack-size",
        CHECKPOINT_RAS_HEADER_BYTES,
        stack_bytes,
    )?;
    let mut operation_offset = checked_sum("payload-size", offset, ras_len)?;
    for _ in 0..pending_count {
        let operation_header_end =
            checked_offset(operation_offset, CHECKPOINT_RAS_OPERATION_FIXED_BYTES)?;
        if payload.len() < operation_header_end {
            return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: operation_header_end,
                actual: payload.len(),
            });
        }
        let mut operation_header = checked_offset(
            operation_offset,
            U64_BYTES + 1 + CHECKPOINT_TARGET_BYTES * 2,
        )?;
        let stack_before_count = read_u32(payload, &mut operation_header)? as usize;
        let stack_after_count = read_u32(payload, &mut operation_header)? as usize;
        let before_bytes = checked_product(
            "return-address-stack-operation-before",
            stack_before_count,
            U64_BYTES,
        )?;
        let after_bytes = checked_product(
            "return-address-stack-operation-after",
            stack_after_count,
            U64_BYTES,
        )?;
        let operation_len = checked_sum(
            "return-address-stack-operation-size",
            CHECKPOINT_RAS_OPERATION_FIXED_BYTES,
            before_bytes,
        )?;
        let operation_len = checked_sum(
            "return-address-stack-operation-size",
            operation_len,
            after_bytes,
        )?;
        ras_len = checked_sum("return-address-stack-size", ras_len, operation_len)?;
        operation_offset = checked_sum("payload-size", operation_offset, operation_len)?;
    }
    Ok(ras_len)
}

fn branch_target_buffer_checkpoint_len(entries: usize) -> Result<usize, BranchPredictorError> {
    let entry_bytes = checked_product(
        "branch-target-buffer-entries",
        entries,
        CHECKPOINT_BTB_ENTRY_BYTES,
    )?;
    checked_sum(
        "branch-target-buffer-size",
        CHECKPOINT_BTB_HEADER_BYTES,
        entry_bytes,
    )
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

fn branch_target_buffer_set_index(pc: Address, config: &BranchTargetBufferConfig) -> usize {
    ((pc.get() >> 2) % config.sets() as u64) as usize
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
