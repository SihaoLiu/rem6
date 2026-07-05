use rem6_memory::Address;

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
const O3_RUNTIME_CHECKPOINT_VERSION: u8 = 7;
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
const O3_RUNTIME_ROB_ENTRY_BYTES: usize = U64_BYTES + U64_BYTES + 1 + U32_BYTES + 1;
const O3_RUNTIME_LSQ_ENTRY_BYTES: usize = U64_BYTES + 1 + U64_BYTES + U32_BYTES + 1 + 1;
const O3_RUNTIME_RENAME_ENTRY_BYTES: usize = 1 + U32_BYTES + U32_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeCheckpointPayload {
    snapshot: O3RuntimeSnapshot,
    stats: O3RuntimeStats,
}

impl O3RuntimeCheckpointPayload {
    pub fn from_snapshot(snapshot: O3RuntimeSnapshot) -> Result<Self, O3RuntimeError> {
        Self::from_snapshot_with_stats(snapshot, O3RuntimeStats::default())
    }

    pub fn from_snapshot_with_stats(
        snapshot: O3RuntimeSnapshot,
        stats: O3RuntimeStats,
    ) -> Result<Self, O3RuntimeError> {
        validate_runtime_snapshot(&snapshot)?;
        Ok(Self { snapshot, stats })
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

        ensure_remaining(
            payload,
            offset,
            checked_bytes(rob_count, O3_RUNTIME_ROB_ENTRY_BYTES, payload.len())?,
        )?;
        let mut reorder_buffer = Vec::with_capacity(rob_count);
        for _ in 0..rob_count {
            reorder_buffer.push(read_rob_entry(payload, &mut offset)?);
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

        ensure_remaining(
            payload,
            offset,
            checked_bytes(rename_count, O3_RUNTIME_RENAME_ENTRY_BYTES, payload.len())?,
        )?;
        let mut rename_map = Vec::with_capacity(rename_count);
        for _ in 0..rename_count {
            rename_map.push(read_rename_entry(payload, &mut offset)?);
        }

        let stats = match version {
            O3_RUNTIME_CHECKPOINT_VERSION => {
                read_o3_runtime_stats(payload, &mut offset, true, true, true, true, true)?
            }
            O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_LATENCY_STATS => {
                read_o3_runtime_stats(payload, &mut offset, true, true, true, true, false)?
            }
            O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_REPAIR_STATS => {
                read_o3_runtime_stats(payload, &mut offset, true, true, true, false, false)?
            }
            O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_MATRIX_STATS => {
                read_o3_runtime_stats(payload, &mut offset, true, true, false, false, false)?
            }
            O3_RUNTIME_CHECKPOINT_VERSION_WITH_FU_CLASS_STATS => {
                read_o3_runtime_stats(payload, &mut offset, true, false, false, false, false)?
            }
            O3_RUNTIME_CHECKPOINT_VERSION_WITH_SCALAR_FU_STATS => {
                read_o3_runtime_stats(payload, &mut offset, false, false, false, false, false)?
            }
            O3_RUNTIME_CHECKPOINT_VERSION_WITHOUT_STATS => O3RuntimeStats::default(),
            _ => unreachable!("version was validated"),
        };

        if offset != payload.len() {
            return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshot_with_stats(
            O3RuntimeSnapshot::new(reorder_buffer, load_store_queue, rename_map, pending_state)?,
            stats,
        )
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
            write_rename_entry(&mut payload, *entry);
        }
        write_o3_runtime_stats(&mut payload, self.stats);
        payload
    }

    pub const fn snapshot(&self) -> &O3RuntimeSnapshot {
        &self.snapshot
    }

    pub const fn stats(&self) -> O3RuntimeStats {
        self.stats
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

fn write_rename_entry(payload: &mut Vec<u8>, entry: O3RenameMapEntry) {
    payload.push(encode_register_class(entry.register_class()));
    payload.extend_from_slice(&entry.architectural().to_le_bytes());
    payload.extend_from_slice(&entry.physical().get().to_le_bytes());
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
    for operation in O3RuntimeLsqOperation::TRACKED {
        payload.extend_from_slice(&stats.lsq_operation_count(operation).to_le_bytes());
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
        stats.max_rob_occupancy(),
        stats.max_lsq_occupancy(),
        stats.rename_map_entries(),
    ] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
}

fn read_rob_entry(
    payload: &[u8],
    offset: &mut usize,
) -> Result<O3ReorderBufferEntry, O3RuntimeError> {
    let sequence = read_u64(payload, offset)?;
    let pc = Address::new(read_u64(payload, offset)?);
    let destination_present = read_bool("ROB destination-present", payload, offset)?;
    let physical = O3PhysicalRegisterId::new(read_u32(payload, offset)?);
    let ready = read_bool("ROB ready", payload, offset)?;
    Ok(
        O3ReorderBufferEntry::new(sequence, pc, destination_present.then_some(physical))
            .with_ready(ready),
    )
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
) -> Result<O3RenameMapEntry, O3RuntimeError> {
    let register_class = decode_register_class(read_u8(payload, offset)?)?;
    let architectural = read_u32(payload, offset)?;
    let physical = O3PhysicalRegisterId::new(read_u32(payload, offset)?);
    Ok(O3RenameMapEntry::new(
        register_class,
        architectural,
        physical,
    ))
}

fn read_o3_runtime_stats(
    payload: &[u8],
    offset: &mut usize,
    has_class_stats: bool,
    has_lsq_matrix_stats: bool,
    has_branch_repair_stats: bool,
    has_lsq_latency_stats: bool,
    has_lsq_data_latency_stats: bool,
) -> Result<O3RuntimeStats, O3RuntimeError> {
    let mut fu_latency_class_instructions = [0; O3RuntimeFuLatencyClass::COUNT];
    let mut fu_latency_class_cycles = [0; O3RuntimeFuLatencyClass::COUNT];
    let mut lsq_operation_counts = [0; O3RuntimeLsqOperation::COUNT];
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
    let fu_latency_instructions = read_u64(payload, offset)?;
    let fu_latency_cycles = read_u64(payload, offset)?;
    if has_class_stats {
        for class in O3RuntimeFuLatencyClass::ALL {
            fu_latency_class_instructions[class.index()] = read_u64(payload, offset)?;
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            fu_latency_class_cycles[class.index()] = read_u64(payload, offset)?;
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
        lsq_operation_counts,
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
        fu_latency_instructions,
        fu_latency_cycles,
        fu_latency_class_instructions,
        fu_latency_class_cycles,
        max_rob_occupancy: read_u64(payload, offset)?,
        max_lsq_occupancy: read_u64(payload, offset)?,
        rename_map_entries: read_u64(payload, offset)?,
    })
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
mod tests {
    use super::*;

    const BASE_AND_FU_STATS_BYTES: usize = (12 + O3RuntimeFuLatencyClass::COUNT * 2) * U64_BYTES;
    const LSQ_OPERATION_STATS_BYTES: usize = O3RuntimeLsqOperation::TRACKED.len() * U64_BYTES;
    const LSQ_OPERATION_LATENCY_STATS_BYTES: usize =
        O3RuntimeLsqOperation::TRACKED.len() * 4 * U64_BYTES;
    const LSQ_DATA_LATENCY_STATS_BYTES: usize = 4 * U64_BYTES;
    const LSQ_ORDERING_STATS_BYTES: usize = (O3RuntimeLsqOrdering::TRACKED.len() + 1) * U64_BYTES;
    const BRANCH_REPAIR_STATS_BYTES: usize = (3 + crate::BranchTargetKind::COUNT * 3) * U64_BYTES;
    const CURRENT_STATS_BYTES: usize = (15 + O3RuntimeFuLatencyClass::COUNT * 2) * U64_BYTES
        + LSQ_OPERATION_STATS_BYTES
        + LSQ_OPERATION_LATENCY_STATS_BYTES
        + LSQ_DATA_LATENCY_STATS_BYTES
        + LSQ_ORDERING_STATS_BYTES
        + BRANCH_REPAIR_STATS_BYTES;

    #[test]
    fn checkpoint_v6_payloads_decode_without_aggregate_lsq_data_latency_stats() {
        let operation = O3RuntimeLsqOperation::StoreConditional;
        let mut operation_samples = [0; O3RuntimeLsqOperation::COUNT];
        let mut operation_ticks = [0; O3RuntimeLsqOperation::COUNT];
        let mut operation_max_ticks = [0; O3RuntimeLsqOperation::COUNT];
        let mut operation_min_ticks = [0; O3RuntimeLsqOperation::COUNT];
        operation_samples[operation.index()] = 2;
        operation_ticks[operation.index()] = 11;
        operation_max_ticks[operation.index()] = 6;
        operation_min_ticks[operation.index()] = 5;
        let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
            super::super::default_o3_runtime_snapshot(),
            O3RuntimeStats {
                lsq_data_latency_samples: 2,
                lsq_data_latency_ticks: 11,
                lsq_data_latency_max_ticks: 6,
                lsq_data_latency_min_ticks: 5,
                lsq_operation_latency_samples: operation_samples,
                lsq_operation_latency_ticks: operation_ticks,
                lsq_operation_latency_max_ticks: operation_max_ticks,
                lsq_operation_latency_min_ticks: operation_min_ticks,
                ..O3RuntimeStats::default()
            },
        )
        .unwrap();
        let encoded = payload.encode();
        let stats_offset = encoded.len().checked_sub(CURRENT_STATS_BYTES).unwrap();
        let data_latency_offset = stats_offset
            + BASE_AND_FU_STATS_BYTES
            + LSQ_OPERATION_STATS_BYTES
            + LSQ_OPERATION_LATENCY_STATS_BYTES;
        let mut v6_encoded = [
            &encoded[..data_latency_offset],
            &encoded[data_latency_offset + LSQ_DATA_LATENCY_STATS_BYTES..],
        ]
        .concat();
        v6_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
            O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_LATENCY_STATS;

        let decoded = O3RuntimeCheckpointPayload::decode(&v6_encoded).unwrap();
        let stats = decoded.stats();

        assert_eq!(stats.lsq_operation_latency_samples(operation), 2);
        assert_eq!(stats.lsq_operation_latency_ticks(operation), 11);
        assert_eq!(stats.lsq_operation_latency_max_ticks(operation), 6);
        assert_eq!(stats.lsq_operation_latency_min_ticks(operation), 5);
        assert_eq!(stats.lsq_operation_latency_avg_ticks(operation), 5);
        assert_eq!(stats.lsq_data_latency_samples(), 0);
        assert_eq!(stats.lsq_data_latency_ticks(), 0);
        assert_eq!(stats.lsq_data_latency_max_ticks(), 0);
        assert_eq!(stats.lsq_data_latency_min_ticks(), 0);
        assert_eq!(stats.lsq_data_latency_avg_ticks(), 0);
    }
}
