use std::collections::BTreeSet;

use rem6_kernel::Tick;
use rem6_memory::{Address, AgentId, MemoryRequestId};

use super::o3_runtime_checkpoint_branch_mismatch::{
    read_o3_runtime_branch_mismatch_stats, write_o3_runtime_branch_mismatch_stats,
    O3RuntimeBranchMismatchCheckpointStats,
};
use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};
use crate::o3_pipeline::O3PendingStateCheckpointPayload;
use crate::o3_runtime_trace::{
    O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
};

use super::{
    encode_register_class, encode_u32, validate_runtime_snapshot, O3LoadStoreQueueEntry,
    O3LoadStoreQueueKind, O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeError,
    O3RuntimeSnapshot, O3RuntimeStats,
};

const O3_RUNTIME_CHECKPOINT_MAGIC: [u8; 4] = *b"O3RT";
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_STAGED_ROB: u8 = 21;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_RETIRE_GATE: u8 = 20;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_ROB_READY_TICKS: u8 = 19;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_EXTREMA_STATS: u8 = 18;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_MISMATCH_STATS: u8 = 17;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_BYTE_STATS: u8 = 16;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_PREDICTION_STATS: u8 = 15;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_REASON_STATS: u8 = 14;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_STATS: u8 = 13;
const O3_RUNTIME_CHECKPOINT_VERSION: u8 = O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_STAGED_ROB;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_STATS: u8 = 12;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_MATRIX_STATS: u8 = 11;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_IQ_BRANCH_ISSUED_STATS: u8 = 10;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_DEPENDENCY_STATS: u8 = 9;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_BRANCH_MISPREDICT_SPLIT_STATS: u8 = 8;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_DATA_LATENCY_STATS: u8 = 7;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_LATENCY_STATS: u8 = 6;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_REPAIR_STATS: u8 = 5;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_MATRIX_STATS: u8 = 4;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_STATS: u8 = 3;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_SCALAR_FU_STATS: u8 = 2;
const O3_RUNTIME_CHECKPOINT_VERSION_WITHOUT_STATS: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const O3_RUNTIME_CHECKPOINT_HEADER_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC.len() + 1 + U32_BYTES * 4;
const O3_RUNTIME_ROB_ENTRY_BYTES_LEGACY: usize = U64_BYTES + U64_BYTES + 1 + U32_BYTES + 1;
const O3_RUNTIME_ROB_ENTRY_BYTES_WITH_READY_TICK: usize =
    O3_RUNTIME_ROB_ENTRY_BYTES_LEGACY + U64_BYTES;
const O3_RUNTIME_ROB_ENTRY_BYTES: usize =
    O3_RUNTIME_ROB_ENTRY_BYTES_WITH_READY_TICK + 1 + 1 + 1 + U32_BYTES;
const O3_RUNTIME_LSQ_ENTRY_BYTES: usize = U64_BYTES + 1 + U64_BYTES + U32_BYTES + 1 + 1;
const O3_RUNTIME_RENAME_ENTRY_BYTES: usize = 1 + U32_BYTES + U32_BYTES;
const O3_RUNTIME_RENAME_ENTRY_BYTES_WITH_DEPENDENCY: usize = O3_RUNTIME_RENAME_ENTRY_BYTES + 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeCheckpointPayload {
    snapshot: O3RuntimeSnapshot,
    stats: O3RuntimeStats,
    dependency_producers_with_consumers: BTreeSet<O3PhysicalRegisterId>,
    live_retire_gate: Option<O3LiveRetireGateCheckpointPayload>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveRetireGateCheckpointPayload {
    request: MemoryRequestId,
    ready_tick: Tick,
}

impl O3LiveRetireGateCheckpointPayload {
    pub(crate) const fn new(request: MemoryRequestId, ready_tick: Tick) -> Self {
        Self {
            request,
            ready_tick,
        }
    }

    pub(crate) const fn request(self) -> MemoryRequestId {
        self.request
    }

    pub(crate) const fn ready_tick(self) -> Tick {
        self.ready_tick
    }
}

impl O3RuntimeCheckpointPayload {
    pub fn from_snapshot(snapshot: O3RuntimeSnapshot) -> Result<Self, O3RuntimeError> {
        Self::from_snapshot_with_stats(snapshot, O3RuntimeStats::default())
    }

    pub fn from_snapshot_with_stats(
        snapshot: O3RuntimeSnapshot,
        stats: O3RuntimeStats,
    ) -> Result<Self, O3RuntimeError> {
        Self::from_snapshot_with_stats_and_dependency_producers(snapshot, stats, BTreeSet::new())
    }

    pub(crate) fn from_snapshot_with_stats_and_dependency_producers(
        snapshot: O3RuntimeSnapshot,
        stats: O3RuntimeStats,
        dependency_producers_with_consumers: BTreeSet<O3PhysicalRegisterId>,
    ) -> Result<Self, O3RuntimeError> {
        validate_runtime_snapshot(&snapshot)?;
        Ok(Self {
            snapshot,
            stats,
            dependency_producers_with_consumers,
            live_retire_gate: None,
        })
    }

    pub(crate) fn with_live_retire_gate(
        mut self,
        live_retire_gate: Option<O3LiveRetireGateCheckpointPayload>,
    ) -> Self {
        self.live_retire_gate = live_retire_gate;
        self
    }

    pub fn decode(payload: &[u8]) -> Result<Self, O3RuntimeError> {
        if payload.len() < O3_RUNTIME_CHECKPOINT_HEADER_BYTES {
            return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: O3_RUNTIME_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[..O3_RUNTIME_CHECKPOINT_MAGIC.len()] != O3_RUNTIME_CHECKPOINT_MAGIC {
            return Err(O3RuntimeError::InvalidCheckpointMagic);
        }

        let mut offset = O3_RUNTIME_CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if !matches!(
            version,
            O3_RUNTIME_CHECKPOINT_VERSION_WITHOUT_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_SCALAR_FU_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_MATRIX_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_REPAIR_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_LATENCY_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_DATA_LATENCY_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_BRANCH_MISPREDICT_SPLIT_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_DEPENDENCY_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_IQ_BRANCH_ISSUED_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_MATRIX_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_REASON_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_PREDICTION_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_BYTE_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_MISMATCH_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_EXTREMA_STATS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_ROB_READY_TICKS
                | O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_RETIRE_GATE
                | O3_RUNTIME_CHECKPOINT_VERSION
        ) {
            return Err(O3RuntimeError::UnsupportedCheckpointVersion { version });
        }

        let pending_payload_len = read_u32(payload, &mut offset)? as usize;
        let rob_count = read_u32(payload, &mut offset)? as usize;
        let lsq_count = read_u32(payload, &mut offset)? as usize;
        let rename_count = read_u32(payload, &mut offset)? as usize;

        ensure_remaining(payload, offset, pending_payload_len)?;
        let pending_payload_end = offset + pending_payload_len;
        let pending_state =
            O3PendingStateCheckpointPayload::decode(&payload[offset..pending_payload_end])
                .map_err(|error| O3RuntimeError::InvalidPendingState { error })?
                .into_snapshot();
        offset = pending_payload_end;

        let has_rob_ready_ticks = version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_ROB_READY_TICKS;
        let has_live_staged_rob = version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_STAGED_ROB;
        let rob_entry_bytes = if has_live_staged_rob {
            O3_RUNTIME_ROB_ENTRY_BYTES
        } else if has_rob_ready_ticks {
            O3_RUNTIME_ROB_ENTRY_BYTES_WITH_READY_TICK
        } else {
            O3_RUNTIME_ROB_ENTRY_BYTES_LEGACY
        };
        ensure_remaining(
            payload,
            offset,
            checked_bytes(rob_count, rob_entry_bytes, payload.len())?,
        )?;
        let mut reorder_buffer = Vec::with_capacity(rob_count);
        for _ in 0..rob_count {
            reorder_buffer.push(read_rob_entry(
                payload,
                &mut offset,
                has_rob_ready_ticks,
                has_live_staged_rob,
            )?);
        }

        ensure_remaining(
            payload,
            offset,
            checked_bytes(lsq_count, O3_RUNTIME_LSQ_ENTRY_BYTES, payload.len())?,
        )?;
        let mut load_store_queue = Vec::with_capacity(lsq_count);
        for _ in 0..lsq_count {
            load_store_queue.push(read_lsq_entry(payload, &mut offset)?);
        }

        let has_dependency_producer_state =
            version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_DEPENDENCY_STATS;
        let rename_entry_bytes = if has_dependency_producer_state {
            O3_RUNTIME_RENAME_ENTRY_BYTES_WITH_DEPENDENCY
        } else {
            O3_RUNTIME_RENAME_ENTRY_BYTES
        };
        ensure_remaining(
            payload,
            offset,
            checked_bytes(rename_count, rename_entry_bytes, payload.len())?,
        )?;
        let mut rename_map = Vec::with_capacity(rename_count);
        let mut dependency_producers_with_consumers = BTreeSet::new();
        for _ in 0..rename_count {
            let (entry, producer_has_consumers) =
                read_rename_entry(payload, &mut offset, has_dependency_producer_state)?;
            if producer_has_consumers {
                dependency_producers_with_consumers.insert(entry.physical());
            }
            rename_map.push(entry);
        }

        let stats = if version == O3_RUNTIME_CHECKPOINT_VERSION_WITHOUT_STATS {
            O3RuntimeStats::default()
        } else {
            read_o3_runtime_stats(
                payload,
                &mut offset,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_MATRIX_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_BYTE_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_REPAIR_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_LATENCY_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_DATA_LATENCY_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_BRANCH_MISPREDICT_SPLIT_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_DEPENDENCY_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_IQ_BRANCH_ISSUED_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_MATRIX_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_PREDICTION_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_STATS,
                version
                    >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_REASON_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_MISMATCH_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_EXTREMA_STATS,
                version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_RETIRE_GATE,
            )?
        };
        let live_retire_gate = if version >= O3_RUNTIME_CHECKPOINT_VERSION_WITH_LIVE_RETIRE_GATE {
            read_live_retire_gate_checkpoint(payload, &mut offset)?
        } else {
            None
        };

        if offset != payload.len() {
            return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Ok(Self::from_snapshot_with_stats_and_dependency_producers(
            O3RuntimeSnapshot::new(reorder_buffer, load_store_queue, rename_map, pending_state)?,
            stats,
            dependency_producers_with_consumers,
        )?
        .with_live_retire_gate(live_retire_gate))
    }

    pub fn encode(&self) -> Vec<u8> {
        let pending_payload =
            O3PendingStateCheckpointPayload::from_snapshot(self.snapshot.pending_state.clone())
                .expect("O3 runtime checkpoint payload was validated before construction")
                .encode();
        let pending_payload_len = encode_u32("pending_payload_length", pending_payload.len())
            .expect("O3 runtime checkpoint payload was validated before construction");
        let rob_count = encode_u32("reorder_buffer_count", self.snapshot.reorder_buffer.len())
            .expect("O3 runtime checkpoint payload was validated before construction");
        let lsq_count = encode_u32(
            "load_store_queue_count",
            self.snapshot.load_store_queue.len(),
        )
        .expect("O3 runtime checkpoint payload was validated before construction");
        let rename_count = encode_u32("rename_map_count", self.snapshot.rename_map.len())
            .expect("O3 runtime checkpoint payload was validated before construction");

        let mut payload = Vec::new();
        payload.extend_from_slice(&O3_RUNTIME_CHECKPOINT_MAGIC);
        payload.push(O3_RUNTIME_CHECKPOINT_VERSION);
        payload.extend_from_slice(&pending_payload_len.to_le_bytes());
        payload.extend_from_slice(&rob_count.to_le_bytes());
        payload.extend_from_slice(&lsq_count.to_le_bytes());
        payload.extend_from_slice(&rename_count.to_le_bytes());
        payload.extend_from_slice(&pending_payload);
        for entry in &self.snapshot.reorder_buffer {
            write_rob_entry(&mut payload, *entry);
        }
        for entry in &self.snapshot.load_store_queue {
            write_lsq_entry(&mut payload, *entry);
        }
        for entry in &self.snapshot.rename_map {
            write_rename_entry(
                &mut payload,
                *entry,
                self.dependency_producers_with_consumers
                    .contains(&entry.physical()),
            );
        }
        write_o3_runtime_stats(&mut payload, self.stats);
        write_live_retire_gate_checkpoint(&mut payload, self.live_retire_gate);
        payload
    }

    pub const fn snapshot(&self) -> &O3RuntimeSnapshot {
        &self.snapshot
    }

    pub const fn stats(&self) -> O3RuntimeStats {
        self.stats
    }

    pub(crate) fn dependency_producers_with_consumers(&self) -> &BTreeSet<O3PhysicalRegisterId> {
        &self.dependency_producers_with_consumers
    }

    pub(crate) const fn live_retire_gate(&self) -> Option<O3LiveRetireGateCheckpointPayload> {
        self.live_retire_gate
    }

    pub fn into_snapshot(self) -> O3RuntimeSnapshot {
        self.snapshot
    }
}

fn checked_bytes(
    count: usize,
    bytes_per_item: usize,
    actual: usize,
) -> Result<usize, O3RuntimeError> {
    count
        .checked_mul(bytes_per_item)
        .ok_or(O3RuntimeError::InvalidCheckpointPayloadSize {
            expected: O3_RUNTIME_CHECKPOINT_HEADER_BYTES,
            actual,
        })
}

fn ensure_remaining(payload: &[u8], offset: usize, bytes: usize) -> Result<(), O3RuntimeError> {
    let expected =
        offset
            .checked_add(bytes)
            .ok_or(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            })?;
    if payload.len() < expected {
        return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}

fn write_rob_entry(payload: &mut Vec<u8>, entry: O3ReorderBufferEntry) {
    payload.extend_from_slice(&entry.sequence().to_le_bytes());
    payload.extend_from_slice(&entry.pc().get().to_le_bytes());
    if let Some(destination) = entry.destination() {
        payload.push(1);
        payload.extend_from_slice(&destination.get().to_le_bytes());
    } else {
        payload.push(0);
        payload.extend_from_slice(&O3PhysicalRegisterId::invalid().get().to_le_bytes());
    }
    payload.push(bool_flag(entry.is_ready()));
    payload.extend_from_slice(&entry.ready_tick().to_le_bytes());
    payload.push(bool_flag(entry.is_live_staged()));
    if let Some((register_class, architectural)) = entry.rename_destination() {
        payload.push(1);
        payload.push(encode_register_class(register_class));
        payload.extend_from_slice(&architectural.to_le_bytes());
    } else {
        payload.push(0);
        payload.push(encode_register_class(O3RegisterClass::Integer));
        payload.extend_from_slice(&0_u32.to_le_bytes());
    }
}

fn write_lsq_entry(payload: &mut Vec<u8>, entry: O3LoadStoreQueueEntry) {
    payload.extend_from_slice(&entry.sequence().to_le_bytes());
    if let Some(address) = entry.address() {
        payload.push(1);
        payload.extend_from_slice(&address.get().to_le_bytes());
    } else {
        payload.push(0);
        payload.extend_from_slice(&0_u64.to_le_bytes());
    }
    payload.extend_from_slice(&entry.bytes().to_le_bytes());
    payload.push(encode_lsq_kind(entry.kind()));
    payload.push(bool_flag(entry.is_completed()));
}

fn write_rename_entry(
    payload: &mut Vec<u8>,
    entry: O3RenameMapEntry,
    producer_has_consumers: bool,
) {
    payload.push(encode_register_class(entry.register_class()));
    payload.extend_from_slice(&entry.architectural().to_le_bytes());
    payload.extend_from_slice(&entry.physical().get().to_le_bytes());
    payload.push(bool_flag(producer_has_consumers));
}

fn write_o3_runtime_stats(payload: &mut Vec<u8>, stats: O3RuntimeStats) {
    for value in [
        stats.instructions(),
        stats.rob_allocations(),
        stats.rob_commits(),
        stats.rename_writes(),
        stats.lsq_loads(),
        stats.lsq_stores(),
        stats.lsq_load_bytes(),
        stats.lsq_store_bytes(),
        stats.lsq_store_to_load_forwarding_candidates(),
        stats.lsq_store_to_load_forwarding_matches(),
        stats.lsq_store_to_load_forwarding_suppressed(),
        stats.fu_latency_instructions(),
        stats.fu_latency_cycles(),
    ] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        payload.extend_from_slice(&stats.fu_latency_class_instructions(class).to_le_bytes());
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        payload.extend_from_slice(&stats.fu_latency_class_cycles(class).to_le_bytes());
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        payload.extend_from_slice(&stats.fu_latency_class_max_cycles(class).to_le_bytes());
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        payload.extend_from_slice(&stats.fu_latency_class_min_cycles(class).to_le_bytes());
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(&stats.lsq_operation_count(operation).to_le_bytes());
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(&stats.lsq_operation_load_bytes(operation).to_le_bytes());
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(&stats.lsq_operation_store_bytes(operation).to_le_bytes());
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_forwarding_candidates(operation)
                .to_le_bytes(),
        );
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_forwarding_matches(operation)
                .to_le_bytes(),
        );
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_forwarding_suppressed(operation)
                .to_le_bytes(),
        );
    }
    payload.extend_from_slice(
        &stats
            .lsq_store_to_load_forwarding_address_mismatches()
            .to_le_bytes(),
    );
    payload.extend_from_slice(
        &stats
            .lsq_store_to_load_forwarding_byte_mismatches()
            .to_le_bytes(),
    );
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_forwarding_address_mismatches(operation)
                .to_le_bytes(),
        );
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_forwarding_byte_mismatches(operation)
                .to_le_bytes(),
        );
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(&stats.lsq_operation_latency_samples(operation).to_le_bytes());
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(&stats.lsq_operation_latency_ticks(operation).to_le_bytes());
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_latency_max_ticks(operation)
                .to_le_bytes(),
        );
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(
            &stats
                .lsq_operation_latency_min_ticks(operation)
                .to_le_bytes(),
        );
    }
    for value in [
        stats.lsq_data_latency_samples(),
        stats.lsq_data_latency_ticks(),
        stats.lsq_data_latency_max_ticks(),
        stats.lsq_data_latency_min_ticks(),
    ] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        payload.extend_from_slice(&stats.lsq_ordering_count(ordering).to_le_bytes());
    }
    payload.extend_from_slice(&stats.lsq_store_conditional_failures().to_le_bytes());
    for value in [
        stats.branch_repair_targetless_mismatches(),
        stats.branch_repair_wrong_targets(),
        stats.branch_repair_direction_only_mismatches(),
    ] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(
            &stats
                .branch_repair_targetless_mismatch_kind(kind)
                .to_le_bytes(),
        );
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_repair_wrong_target_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_repair_direction_only_kind(kind).to_le_bytes());
    }
    for value in [
        stats.iew_predicted_taken_incorrect(),
        stats.iew_predicted_not_taken_incorrect(),
    ] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    for value in [stats.iew_producer_insts(), stats.iew_consumer_insts()] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    payload.extend_from_slice(&stats.iq_branch_insts_issued().to_le_bytes());
    for value in [
        stats.max_rob_occupancy(),
        stats.max_lsq_occupancy(),
        stats.rename_map_entries(),
        stats.live_retire_gate_scheduled_waits(),
        stats.live_retire_gate_wait_ticks(),
        stats.live_retire_gate_max_wait_ticks(),
    ] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_taken_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_resolved_target_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_link_write_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_squash_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(
            &stats
                .branch_event_squashed_target_without_link_write_kind(kind)
                .to_le_bytes(),
        );
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_predicted_taken_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(&stats.branch_event_predicted_target_kind(kind).to_le_bytes());
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(
            &stats
                .branch_event_predicted_target_match_kind(kind)
                .to_le_bytes(),
        );
    }
    for kind in crate::BranchTargetKind::ALL {
        payload.extend_from_slice(
            &stats
                .branch_event_predicted_target_mismatch_kind(kind)
                .to_le_bytes(),
        );
    }
    write_o3_runtime_branch_mismatch_stats(payload, stats);
}

fn write_live_retire_gate_checkpoint(
    payload: &mut Vec<u8>,
    checkpoint: Option<O3LiveRetireGateCheckpointPayload>,
) {
    if let Some(checkpoint) = checkpoint {
        payload.push(1);
        payload.extend_from_slice(&checkpoint.request().agent().get().to_le_bytes());
        payload.extend_from_slice(&checkpoint.request().sequence().to_le_bytes());
        payload.extend_from_slice(&checkpoint.ready_tick().to_le_bytes());
    } else {
        payload.push(0);
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&0_u64.to_le_bytes());
        payload.extend_from_slice(&0_u64.to_le_bytes());
    }
}

fn read_rob_entry(
    payload: &[u8],
    offset: &mut usize,
    has_ready_tick: bool,
    has_live_staged: bool,
) -> Result<O3ReorderBufferEntry, O3RuntimeError> {
    let sequence = read_u64(payload, offset)?;
    let pc = Address::new(read_u64(payload, offset)?);
    let destination_present = read_bool("ROB destination-present", payload, offset)?;
    let physical = O3PhysicalRegisterId::new(read_u32(payload, offset)?);
    let ready = read_bool("ROB ready", payload, offset)?;
    let ready_tick = if has_ready_tick {
        read_u64(payload, offset)?
    } else {
        pc.get()
    };
    let (live_staged, rename_destination) = if has_live_staged {
        let live_staged = read_bool("ROB live-staged", payload, offset)?;
        let rename_destination_present =
            read_bool("ROB rename-destination-present", payload, offset)?;
        let register_class = decode_register_class(read_u8(payload, offset)?)?;
        let architectural = read_u32(payload, offset)?;
        (
            live_staged,
            rename_destination_present.then_some((register_class, architectural)),
        )
    } else {
        (false, None)
    };
    let entry = O3ReorderBufferEntry::new(sequence, pc, destination_present.then_some(physical))
        .with_ready(ready)
        .with_ready_tick(ready_tick);
    Ok(if live_staged {
        entry.with_live_staged_rename_destination(rename_destination)
    } else {
        entry
    })
}

fn read_lsq_entry(
    payload: &[u8],
    offset: &mut usize,
) -> Result<O3LoadStoreQueueEntry, O3RuntimeError> {
    let sequence = read_u64(payload, offset)?;
    let address_present = read_bool("LSQ address-present", payload, offset)?;
    let address = Address::new(read_u64(payload, offset)?);
    let bytes = read_u32(payload, offset)?;
    let kind = decode_lsq_kind(read_u8(payload, offset)?)?;
    let completed = read_bool("LSQ completed", payload, offset)?;
    let entry = match kind {
        O3LoadStoreQueueKind::Load => {
            O3LoadStoreQueueEntry::load(sequence, address_present.then_some(address), bytes)
        }
        O3LoadStoreQueueKind::Store => {
            O3LoadStoreQueueEntry::store(sequence, address_present.then_some(address), bytes)
        }
    };
    Ok(entry.with_completed(completed))
}

fn read_rename_entry(
    payload: &[u8],
    offset: &mut usize,
    has_dependency_producer_state: bool,
) -> Result<(O3RenameMapEntry, bool), O3RuntimeError> {
    let register_class = decode_register_class(read_u8(payload, offset)?)?;
    let architectural = read_u32(payload, offset)?;
    let physical = O3PhysicalRegisterId::new(read_u32(payload, offset)?);
    let producer_has_consumers = if has_dependency_producer_state {
        read_bool("rename-map producer-has-consumers", payload, offset)?
    } else {
        false
    };
    Ok((
        O3RenameMapEntry::new(register_class, architectural, physical),
        producer_has_consumers,
    ))
}

fn read_o3_runtime_stats(
    payload: &[u8],
    offset: &mut usize,
    has_class_stats: bool,
    has_lsq_matrix_stats: bool,
    has_lsq_operation_byte_stats: bool,
    has_branch_repair_stats: bool,
    has_lsq_latency_stats: bool,
    has_lsq_data_latency_stats: bool,
    has_iew_branch_mispredict_split_stats: bool,
    has_iew_dependency_stats: bool,
    has_iq_branch_issued_stats: bool,
    has_lsq_forwarding_matrix_stats: bool,
    has_branch_event_stats: bool,
    has_branch_event_prediction_stats: bool,
    has_lsq_forwarding_suppression_stats: bool,
    has_lsq_forwarding_suppression_reason_stats: bool,
    has_branch_mismatch_stats: bool,
    has_fu_latency_class_extrema_stats: bool,
    has_live_retire_gate_stats: bool,
) -> Result<O3RuntimeStats, O3RuntimeError> {
    let mut fu_latency_class_instructions = [0; O3RuntimeFuLatencyClass::COUNT];
    let mut fu_latency_class_cycles = [0; O3RuntimeFuLatencyClass::COUNT];
    let mut fu_latency_class_max_cycles = [0; O3RuntimeFuLatencyClass::COUNT];
    let mut fu_latency_class_min_cycles = [0; O3RuntimeFuLatencyClass::COUNT];
    let mut lsq_operation_counts = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_load_bytes = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_store_bytes = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_forwarding_candidates = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_forwarding_matches = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_forwarding_suppressed = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_forwarding_address_mismatches = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_forwarding_byte_mismatches = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_data_latency_samples = 0;
    let mut lsq_data_latency_ticks = 0;
    let mut lsq_data_latency_max_ticks = 0;
    let mut lsq_data_latency_min_ticks = 0;
    let mut lsq_operation_latency_samples = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_latency_ticks = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_latency_max_ticks = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_operation_latency_min_ticks = [0; O3RuntimeLsqOperation::COUNT];
    let mut lsq_ordering_counts = [0; O3RuntimeLsqOrdering::COUNT];
    let mut branch_repair_targetless_mismatch_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_repair_wrong_target_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_repair_direction_only_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_taken_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_taken_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_match_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_mismatch_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_resolved_target_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_link_write_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_squash_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_squashed_target_without_link_write_kinds =
        [0; crate::BranchTargetKind::COUNT];
    let instructions = read_u64(payload, offset)?;
    let rob_allocations = read_u64(payload, offset)?;
    let rob_commits = read_u64(payload, offset)?;
    let rename_writes = read_u64(payload, offset)?;
    let lsq_loads = read_u64(payload, offset)?;
    let lsq_stores = read_u64(payload, offset)?;
    let lsq_load_bytes = read_u64(payload, offset)?;
    let lsq_store_bytes = read_u64(payload, offset)?;
    let lsq_store_to_load_forwarding_candidates = read_u64(payload, offset)?;
    let lsq_store_to_load_forwarding_matches = read_u64(payload, offset)?;
    let lsq_store_to_load_forwarding_suppressed = if has_lsq_forwarding_suppression_stats {
        read_u64(payload, offset)?
    } else {
        0
    };
    let mut lsq_store_to_load_forwarding_address_mismatches = 0;
    let mut lsq_store_to_load_forwarding_byte_mismatches = 0;
    let fu_latency_instructions = read_u64(payload, offset)?;
    let fu_latency_cycles = read_u64(payload, offset)?;
    if has_class_stats {
        for class in O3RuntimeFuLatencyClass::ALL {
            fu_latency_class_instructions[class.index()] = read_u64(payload, offset)?;
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            fu_latency_class_cycles[class.index()] = read_u64(payload, offset)?;
        }
        if has_fu_latency_class_extrema_stats {
            for class in O3RuntimeFuLatencyClass::ALL {
                fu_latency_class_max_cycles[class.index()] = read_u64(payload, offset)?;
            }
            for class in O3RuntimeFuLatencyClass::ALL {
                fu_latency_class_min_cycles[class.index()] = read_u64(payload, offset)?;
            }
        }
    } else {
        fu_latency_class_instructions[O3RuntimeFuLatencyClass::ScalarIntegerMul.index()] =
            read_u64(payload, offset)?;
        fu_latency_class_cycles[O3RuntimeFuLatencyClass::ScalarIntegerMul.index()] =
            read_u64(payload, offset)?;
        fu_latency_class_instructions[O3RuntimeFuLatencyClass::ScalarIntegerDiv.index()] =
            read_u64(payload, offset)?;
        fu_latency_class_cycles[O3RuntimeFuLatencyClass::ScalarIntegerDiv.index()] =
            read_u64(payload, offset)?;
    }
    let lsq_store_conditional_failures = if has_lsq_matrix_stats {
        for operation in O3RuntimeLsqOperation::TRACKED {
            lsq_operation_counts[operation.index()] = read_u64(payload, offset)?;
        }
        if has_lsq_operation_byte_stats {
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_load_bytes[operation.index()] = read_u64(payload, offset)?;
            }
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_store_bytes[operation.index()] = read_u64(payload, offset)?;
            }
        }
        if has_lsq_forwarding_matrix_stats {
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_forwarding_candidates[operation.index()] = read_u64(payload, offset)?;
            }
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_forwarding_matches[operation.index()] = read_u64(payload, offset)?;
            }
            if has_lsq_forwarding_suppression_stats {
                for operation in O3RuntimeLsqOperation::TRACKED {
                    lsq_operation_forwarding_suppressed[operation.index()] =
                        read_u64(payload, offset)?;
                }
            }
            if has_lsq_forwarding_suppression_reason_stats {
                lsq_store_to_load_forwarding_address_mismatches = read_u64(payload, offset)?;
                lsq_store_to_load_forwarding_byte_mismatches = read_u64(payload, offset)?;
                for operation in O3RuntimeLsqOperation::TRACKED {
                    lsq_operation_forwarding_address_mismatches[operation.index()] =
                        read_u64(payload, offset)?;
                }
                for operation in O3RuntimeLsqOperation::TRACKED {
                    lsq_operation_forwarding_byte_mismatches[operation.index()] =
                        read_u64(payload, offset)?;
                }
            }
        }
        if has_lsq_latency_stats {
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_latency_samples[operation.index()] = read_u64(payload, offset)?;
            }
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_latency_ticks[operation.index()] = read_u64(payload, offset)?;
            }
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_latency_max_ticks[operation.index()] = read_u64(payload, offset)?;
            }
            for operation in O3RuntimeLsqOperation::TRACKED {
                lsq_operation_latency_min_ticks[operation.index()] = read_u64(payload, offset)?;
            }
            if has_lsq_data_latency_stats {
                lsq_data_latency_samples = read_u64(payload, offset)?;
                lsq_data_latency_ticks = read_u64(payload, offset)?;
                lsq_data_latency_max_ticks = read_u64(payload, offset)?;
                lsq_data_latency_min_ticks = read_u64(payload, offset)?;
            }
        }
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            lsq_ordering_counts[ordering.index()] = read_u64(payload, offset)?;
        }
        read_u64(payload, offset)?
    } else {
        0
    };
    let (
        branch_repair_targetless_mismatches,
        branch_repair_wrong_targets,
        branch_repair_direction_only_mismatches,
    ) = if has_branch_repair_stats {
        let targetless = read_u64(payload, offset)?;
        let wrong_target = read_u64(payload, offset)?;
        let direction_only = read_u64(payload, offset)?;
        for kind in crate::BranchTargetKind::ALL {
            branch_repair_targetless_mismatch_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_repair_wrong_target_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_repair_direction_only_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        (targetless, wrong_target, direction_only)
    } else {
        (0, 0, 0)
    };
    let (iew_predicted_taken_incorrect, iew_predicted_not_taken_incorrect) =
        if has_iew_branch_mispredict_split_stats {
            (read_u64(payload, offset)?, read_u64(payload, offset)?)
        } else {
            (0, 0)
        };
    let (iew_producer_insts, iew_consumer_insts) = if has_iew_dependency_stats {
        (read_u64(payload, offset)?, read_u64(payload, offset)?)
    } else {
        (0, 0)
    };
    let iq_branch_insts_issued = if has_iq_branch_issued_stats {
        read_u64(payload, offset)?
    } else {
        0
    };
    let max_rob_occupancy = read_u64(payload, offset)?;
    let max_lsq_occupancy = read_u64(payload, offset)?;
    let rename_map_entries = read_u64(payload, offset)?;
    let (
        live_retire_gate_scheduled_waits,
        live_retire_gate_wait_ticks,
        live_retire_gate_max_wait_ticks,
    ) = if has_live_retire_gate_stats {
        (
            read_u64(payload, offset)?,
            read_u64(payload, offset)?,
            read_u64(payload, offset)?,
        )
    } else {
        (0, 0, 0)
    };
    if has_branch_event_stats {
        for kind in crate::BranchTargetKind::ALL {
            branch_event_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_taken_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_resolved_target_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_link_write_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_squash_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_squashed_target_without_link_write_kinds[kind.index()] =
                read_u64(payload, offset)?;
        }
    }
    if has_branch_event_prediction_stats {
        for kind in crate::BranchTargetKind::ALL {
            branch_event_predicted_taken_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_predicted_target_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_predicted_target_match_kinds[kind.index()] = read_u64(payload, offset)?;
        }
        for kind in crate::BranchTargetKind::ALL {
            branch_event_predicted_target_mismatch_kinds[kind.index()] = read_u64(payload, offset)?;
        }
    }
    let branch_mismatch_stats = if has_branch_mismatch_stats {
        read_o3_runtime_branch_mismatch_stats(payload, offset)?
    } else {
        O3RuntimeBranchMismatchCheckpointStats::default()
    };
    Ok(O3RuntimeStats {
        instructions,
        rob_allocations,
        rob_commits,
        rename_writes,
        lsq_loads,
        lsq_stores,
        lsq_load_bytes,
        lsq_store_bytes,
        lsq_store_to_load_forwarding_candidates,
        lsq_store_to_load_forwarding_matches,
        lsq_store_to_load_forwarding_suppressed,
        lsq_store_to_load_forwarding_address_mismatches,
        lsq_store_to_load_forwarding_byte_mismatches,
        lsq_operation_counts,
        lsq_operation_load_bytes,
        lsq_operation_store_bytes,
        lsq_operation_forwarding_candidates,
        lsq_operation_forwarding_matches,
        lsq_operation_forwarding_suppressed,
        lsq_operation_forwarding_address_mismatches,
        lsq_operation_forwarding_byte_mismatches,
        lsq_data_latency_samples,
        lsq_data_latency_ticks,
        lsq_data_latency_max_ticks,
        lsq_data_latency_min_ticks,
        lsq_operation_latency_samples,
        lsq_operation_latency_ticks,
        lsq_operation_latency_max_ticks,
        lsq_operation_latency_min_ticks,
        lsq_ordering_counts,
        lsq_store_conditional_failures,
        branch_repair_targetless_mismatches,
        branch_repair_wrong_targets,
        branch_repair_direction_only_mismatches,
        branch_repair_targetless_mismatch_kinds,
        branch_repair_wrong_target_kinds,
        branch_repair_direction_only_kinds,
        branch_direction_mismatch_kinds: branch_mismatch_stats.branch_direction_mismatch_kinds,
        branch_direction_mismatch_link_write_kinds: branch_mismatch_stats
            .branch_direction_mismatch_link_write_kinds,
        branch_direction_mismatch_without_link_write_kinds: branch_mismatch_stats
            .branch_direction_mismatch_without_link_write_kinds,
        branch_direction_mismatch_squashed_target_kinds: branch_mismatch_stats
            .branch_direction_mismatch_squashed_target_kinds,
        branch_direction_mismatch_squashed_target_link_write_kinds: branch_mismatch_stats
            .branch_direction_mismatch_squashed_target_link_write_kinds,
        branch_direction_mismatch_squashed_target_without_link_write_kinds: branch_mismatch_stats
            .branch_direction_mismatch_squashed_target_without_link_write_kinds,
        branch_target_mismatch_targetless_kinds: branch_mismatch_stats
            .branch_target_mismatch_targetless_kinds,
        branch_target_mismatch_targetless_without_link_write_kinds: branch_mismatch_stats
            .branch_target_mismatch_targetless_without_link_write_kinds,
        branch_target_mismatch_targetless_squashed_target_kinds: branch_mismatch_stats
            .branch_target_mismatch_targetless_squashed_target_kinds,
        branch_target_mismatch_targetless_squashed_target_without_link_write_kinds:
            branch_mismatch_stats
                .branch_target_mismatch_targetless_squashed_target_without_link_write_kinds,
        branch_target_mismatch_wrong_target_kinds: branch_mismatch_stats
            .branch_target_mismatch_wrong_target_kinds,
        branch_target_mismatch_wrong_target_link_write_kinds: branch_mismatch_stats
            .branch_target_mismatch_wrong_target_link_write_kinds,
        branch_target_mismatch_wrong_target_without_link_write_kinds: branch_mismatch_stats
            .branch_target_mismatch_wrong_target_without_link_write_kinds,
        branch_target_mismatch_wrong_target_squashed_target_kinds: branch_mismatch_stats
            .branch_target_mismatch_wrong_target_squashed_target_kinds,
        branch_target_mismatch_wrong_target_squashed_target_link_write_kinds: branch_mismatch_stats
            .branch_target_mismatch_wrong_target_squashed_target_link_write_kinds,
        branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds:
            branch_mismatch_stats
                .branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds,
        branch_event_kinds,
        branch_event_taken_kinds,
        branch_event_predicted_taken_kinds,
        branch_event_predicted_target_kinds,
        branch_event_predicted_target_match_kinds,
        branch_event_predicted_target_mismatch_kinds,
        branch_event_resolved_target_kinds,
        branch_event_link_write_kinds,
        branch_event_squash_kinds,
        branch_event_squashed_target_without_link_write_kinds,
        iew_predicted_taken_incorrect,
        iew_predicted_not_taken_incorrect,
        iew_producer_insts,
        iew_consumer_insts,
        fu_latency_instructions,
        fu_latency_cycles,
        fu_latency_class_instructions,
        fu_latency_class_cycles,
        fu_latency_class_max_cycles,
        fu_latency_class_min_cycles,
        iq_branch_insts_issued,
        live_retire_gate_scheduled_waits,
        live_retire_gate_wait_ticks,
        live_retire_gate_max_wait_ticks,
        max_rob_occupancy,
        max_lsq_occupancy,
        rename_map_entries,
    })
}

fn read_live_retire_gate_checkpoint(
    payload: &[u8],
    offset: &mut usize,
) -> Result<Option<O3LiveRetireGateCheckpointPayload>, O3RuntimeError> {
    let present = read_bool("live-retire-gate present", payload, offset)?;
    let agent = read_u32(payload, offset)?;
    let sequence = read_u64(payload, offset)?;
    let ready_tick = read_u64(payload, offset)?;
    Ok(present.then_some(O3LiveRetireGateCheckpointPayload::new(
        MemoryRequestId::new(AgentId::new(agent), sequence),
        ready_tick,
    )))
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, O3RuntimeError> {
    ensure_remaining(payload, *offset, 1)?;
    let value = payload[*offset];
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, O3RuntimeError> {
    ensure_remaining(payload, *offset, U32_BYTES)?;
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("O3 runtime checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, O3RuntimeError> {
    ensure_remaining(payload, *offset, U64_BYTES)?;
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("O3 runtime checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    Ok(u64::from_le_bytes(bytes))
}

fn read_bool(
    field: &'static str,
    payload: &[u8],
    offset: &mut usize,
) -> Result<bool, O3RuntimeError> {
    match read_u8(payload, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(O3RuntimeError::InvalidCheckpointBool { field, value }),
    }
}

fn decode_register_class(code: u8) -> Result<O3RegisterClass, O3RuntimeError> {
    match code {
        0 => Ok(O3RegisterClass::Integer),
        1 => Ok(O3RegisterClass::FloatingPoint),
        2 => Ok(O3RegisterClass::Vector),
        3 => Ok(O3RegisterClass::ConditionCode),
        4 => Ok(O3RegisterClass::Misc),
        _ => Err(O3RuntimeError::InvalidRegisterClassCode { code }),
    }
}

fn encode_lsq_kind(kind: O3LoadStoreQueueKind) -> u8 {
    match kind {
        O3LoadStoreQueueKind::Load => 0,
        O3LoadStoreQueueKind::Store => 1,
    }
}

fn decode_lsq_kind(code: u8) -> Result<O3LoadStoreQueueKind, O3RuntimeError> {
    match code {
        0 => Ok(O3LoadStoreQueueKind::Load),
        1 => Ok(O3LoadStoreQueueKind::Store),
        _ => Err(O3RuntimeError::InvalidLoadStoreKindCode { code }),
    }
}

fn bool_flag(value: bool) -> u8 {
    u8::from(value)
}

#[cfg(test)]
#[path = "o3_runtime_checkpoint_tests.rs"]
mod tests;
