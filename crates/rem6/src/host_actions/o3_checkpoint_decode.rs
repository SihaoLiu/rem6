use rem6_cpu::{O3RuntimeCheckpointPayload, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation};
use rem6_system::RISCV_O3_RUNTIME_STATE_CHUNK;

use super::Rem6HostO3RuntimeCheckpointChunkSummary;

const O3_RUNTIME_CHECKPOINT_VERSION_OFFSET: usize = 4;

pub(super) fn decode_o3_runtime_checkpoint_chunk(
    name: &str,
    payload: &[u8],
) -> Option<Rem6HostO3RuntimeCheckpointChunkSummary> {
    if name != RISCV_O3_RUNTIME_STATE_CHUNK {
        return None;
    }
    let Ok(decoded) = O3RuntimeCheckpointPayload::decode(payload) else {
        return Some(Rem6HostO3RuntimeCheckpointChunkSummary::decode_error());
    };
    let snapshot = decoded.snapshot();
    let stats = decoded.stats();
    let pending_live_retire_gate = decoded.pending_live_retire_gate();
    let live_retire_gate_request = pending_live_retire_gate.map(|(request, _)| request);
    let integer_mul = O3RuntimeFuLatencyClass::ScalarIntegerMul;
    let integer_div = O3RuntimeFuLatencyClass::ScalarIntegerDiv;
    let float_misc = O3RuntimeFuLatencyClass::ScalarFloatMisc;
    Some(Rem6HostO3RuntimeCheckpointChunkSummary {
        decode_error: false,
        checkpoint_version: payload
            .get(O3_RUNTIME_CHECKPOINT_VERSION_OFFSET)
            .copied()
            .map(u64::from),
        live_retire_gate_request_agent: live_retire_gate_request
            .map(|request| u64::from(request.agent().get())),
        live_retire_gate_request_sequence: live_retire_gate_request
            .map(|request| request.sequence()),
        live_retire_gate_ready_tick: pending_live_retire_gate.map(|(_, ready_tick)| ready_tick),
        snapshot_rob_entries: Some(snapshot.reorder_buffer().len() as u64),
        snapshot_lsq_entries: Some(snapshot.load_store_queue().len() as u64),
        snapshot_rename_map_entries: Some(snapshot.rename_map().len() as u64),
        stats_max_rob_occupancy: Some(stats.max_rob_occupancy()),
        stats_max_lsq_occupancy: Some(stats.max_lsq_occupancy()),
        stats_rename_map_entries: Some(stats.rename_map_entries()),
        stats_issue_cycles: Some(stats.issue_cycles()),
        stats_issued_rows: Some(stats.issued_rows()),
        stats_resource_blocked_row_cycles: Some(stats.resource_blocked_row_cycles()),
        stats_dependency_blocked_row_cycles: Some(stats.dependency_blocked_row_cycles()),
        stats_max_rows_per_cycle: Some(stats.max_rows_per_cycle()),
        stats_writeback_port_cycles: Some(stats.writeback_port_cycles()),
        stats_writeback_port_admitted_rows: Some(stats.writeback_port_admitted_rows()),
        stats_writeback_port_deferred_rows: Some(stats.writeback_port_deferred_rows()),
        stats_writeback_port_deferred_row_cycles: Some(stats.writeback_port_deferred_row_cycles()),
        stats_writeback_port_max_ready_rows_per_cycle: Some(
            stats.writeback_port_max_ready_rows_per_cycle(),
        ),
        stats_writeback_port_max_deferred_rows: Some(stats.writeback_port_max_deferred_rows()),
        stats_lsq_operation_load: Some(stats.lsq_operation_count(O3RuntimeLsqOperation::Load)),
        stats_lsq_operation_store: Some(stats.lsq_operation_count(O3RuntimeLsqOperation::Store)),
        stats_lsq_data_latency_samples: Some(stats.lsq_data_latency_samples()),
        stats_lsq_data_latency_ticks: Some(stats.lsq_data_latency_ticks()),
        stats_lsq_data_latency_max_ticks: Some(stats.lsq_data_latency_max_ticks()),
        stats_lsq_data_latency_min_ticks: Some(stats.lsq_data_latency_min_ticks()),
        stats_lsq_data_latency_avg_ticks: Some(stats.lsq_data_latency_avg_ticks()),
        stats_lsq_operation_load_latency_samples: Some(
            stats.lsq_operation_latency_samples(O3RuntimeLsqOperation::Load),
        ),
        stats_lsq_operation_load_latency_ticks: Some(
            stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::Load),
        ),
        stats_lsq_operation_store_latency_samples: Some(
            stats.lsq_operation_latency_samples(O3RuntimeLsqOperation::Store),
        ),
        stats_lsq_operation_store_latency_ticks: Some(
            stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::Store),
        ),
        stats_fu_latency_instructions: Some(stats.fu_latency_instructions()),
        stats_fu_latency_cycles: Some(stats.fu_latency_cycles()),
        stats_fu_latency_class_integer_mul_instructions: Some(
            stats.fu_latency_class_instructions(integer_mul),
        ),
        stats_fu_latency_class_integer_mul_cycles: Some(stats.fu_latency_class_cycles(integer_mul)),
        stats_fu_latency_class_integer_mul_max_cycles: Some(
            stats.fu_latency_class_max_cycles(integer_mul),
        ),
        stats_fu_latency_class_integer_mul_min_cycles: Some(
            stats.fu_latency_class_min_cycles(integer_mul),
        ),
        stats_fu_latency_class_integer_mul_avg_cycles: Some(
            stats.fu_latency_class_avg_cycles(integer_mul),
        ),
        stats_fu_latency_class_integer_div_instructions: Some(
            stats.fu_latency_class_instructions(integer_div),
        ),
        stats_fu_latency_class_integer_div_cycles: Some(stats.fu_latency_class_cycles(integer_div)),
        stats_fu_latency_class_integer_div_max_cycles: Some(
            stats.fu_latency_class_max_cycles(integer_div),
        ),
        stats_fu_latency_class_integer_div_min_cycles: Some(
            stats.fu_latency_class_min_cycles(integer_div),
        ),
        stats_fu_latency_class_integer_div_avg_cycles: Some(
            stats.fu_latency_class_avg_cycles(integer_div),
        ),
        stats_fu_latency_class_float_misc_instructions: Some(
            stats.fu_latency_class_instructions(float_misc),
        ),
        stats_fu_latency_class_float_misc_cycles: Some(stats.fu_latency_class_cycles(float_misc)),
        stats_fu_latency_class_float_misc_max_cycles: Some(
            stats.fu_latency_class_max_cycles(float_misc),
        ),
        stats_fu_latency_class_float_misc_min_cycles: Some(
            stats.fu_latency_class_min_cycles(float_misc),
        ),
        stats_fu_latency_class_float_misc_avg_cycles: Some(
            stats.fu_latency_class_avg_cycles(float_misc),
        ),
    })
}
