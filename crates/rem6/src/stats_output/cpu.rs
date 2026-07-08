use rem6_cpu::{BranchTargetKind, BranchTargetProvider};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use super::o3_runtime::emit_o3_runtime_stats;
use super::pipeline::{
    emit_in_order_cause_stage_stats, emit_in_order_cause_stat, emit_in_order_stage_stats,
    emit_in_order_stall_cause_stage_stats, emit_in_order_stall_cause_stat,
};
use crate::{Rem6CliError, Rem6CoreSummary};

pub(super) fn emit_cpu_run_stats(
    stats: &mut StatsRegistry,
    cores: &[Rem6CoreSummary],
) -> Result<(), Rem6CliError> {
    let single_cpu_run = cores.len() == 1;
    for core in cores {
        let gem5_cpu_alias_prefix = if single_cpu_run {
            "system.cpu".to_string()
        } else {
            format!("system.cpu{}", core.cpu)
        };
        increment_stat(
            stats,
            &format!("sim.cpu{}.instructions.committed", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.committed_instructions,
        )?;
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.numInsts"),
            "Count",
            StatResetPolicy::Monotonic,
            core.committed_instructions,
        )?;
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.numOps"),
            "Count",
            StatResetPolicy::Monotonic,
            core.committed_instructions,
        )?;
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.commitStats0.numInsts"),
            "Count",
            StatResetPolicy::Monotonic,
            core.committed_instructions,
        )?;
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.commitStats0.numOps"),
            "Count",
            StatResetPolicy::Monotonic,
            core.committed_instructions,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_cycles,
        )?;
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.numCycles"),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.retired", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_retired,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.advanced", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_advanced,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.flushed", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_flushed,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.flush_cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_flush_cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.resource_blocked", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_resource_blocked,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.ordering_blocked", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_ordering_blocked,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.in_flight", core.cpu),
            "Count",
            StatResetPolicy::Constant,
            core.in_order_pipeline_in_flight,
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "width",
            "Count",
            StatResetPolicy::Constant,
            core.in_order_pipeline_stage_widths.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "in_flight",
            "Count",
            StatResetPolicy::Constant,
            core.in_order_pipeline_stage_in_flight.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "max_in_flight",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_max_in_flight.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "occupied_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_occupied_cycles.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "advanced",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_advanced.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "advanced_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_advanced_cycles.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "retired",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_retired.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "retired_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_retired_cycles.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "resource_blocked",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_resource_blocked.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "resource_blocked_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_resource_blocked_cycles
                .values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "ordering_blocked",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_ordering_blocked.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "ordering_blocked_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_ordering_blocked_cycles
                .values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "flushed",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_flushed.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "flushed_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_flushed_cycles.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "branch_prediction_flushed",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_branch_prediction_flushed
                .values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "branch_prediction_flushed_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_branch_prediction_flushed_cycles
                .values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "trap_redirect_flushed",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_trap_redirect_flushed.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "trap_redirect_flushed_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_trap_redirect_flushed_cycles
                .values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "interrupt_redirect_flushed",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_interrupt_redirect_flushed
                .values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "interrupt_redirect_flushed_cycles",
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_interrupt_redirect_flushed_cycles
                .values(),
        )?;
        for (cause, flush_records, redirect_records, stage_records, flushed, flushed_cycles) in [
            (
                "branch_prediction",
                core.in_order_pipeline_branch_prediction_flush_records,
                core.in_order_pipeline_branch_prediction_redirect_records,
                core.in_order_pipeline_stage_branch_prediction_records,
                core.in_order_pipeline_stage_branch_prediction_flushed,
                core.in_order_pipeline_stage_branch_prediction_flushed_cycles,
            ),
            (
                "trap_redirect",
                core.in_order_pipeline_trap_redirect_flush_records,
                core.in_order_pipeline_trap_redirect_records,
                core.in_order_pipeline_stage_trap_redirect_records,
                core.in_order_pipeline_stage_trap_redirect_flushed,
                core.in_order_pipeline_stage_trap_redirect_flushed_cycles,
            ),
            (
                "interrupt_redirect",
                core.in_order_pipeline_interrupt_redirect_flush_records,
                core.in_order_pipeline_interrupt_redirect_records,
                core.in_order_pipeline_stage_interrupt_redirect_records,
                core.in_order_pipeline_stage_interrupt_redirect_flushed,
                core.in_order_pipeline_stage_interrupt_redirect_flushed_cycles,
            ),
        ] {
            let flushed_total = in_order_pipeline_stage_total(flushed);
            let flushed_cycle_total = in_order_pipeline_stage_total(flushed_cycles);
            emit_in_order_cause_stat(
                stats,
                core,
                "flush_cause",
                cause,
                "records",
                "Count",
                StatResetPolicy::Monotonic,
                flush_records,
            )?;
            emit_in_order_cause_stat(
                stats,
                core,
                "flush_cause",
                cause,
                "flushed",
                "Count",
                StatResetPolicy::Monotonic,
                flushed_total,
            )?;
            emit_in_order_cause_stat(
                stats,
                core,
                "flush_cause",
                cause,
                "flushed_cycles",
                "Cycle",
                StatResetPolicy::Monotonic,
                flushed_cycle_total,
            )?;
            emit_in_order_cause_stage_stats(
                stats,
                core,
                "flush_cause",
                cause,
                "records",
                "Count",
                StatResetPolicy::Monotonic,
                stage_records.values(),
            )?;
            emit_in_order_cause_stage_stats(
                stats,
                core,
                "flush_cause",
                cause,
                "flushed",
                "Count",
                StatResetPolicy::Monotonic,
                flushed.values(),
            )?;
            emit_in_order_cause_stage_stats(
                stats,
                core,
                "flush_cause",
                cause,
                "flushed_cycles",
                "Cycle",
                StatResetPolicy::Monotonic,
                flushed_cycles.values(),
            )?;
            emit_in_order_cause_stat(
                stats,
                core,
                "redirect_cause",
                cause,
                "records",
                "Count",
                StatResetPolicy::Monotonic,
                redirect_records,
            )?;
            emit_in_order_cause_stat(
                stats,
                core,
                "redirect_cause",
                cause,
                "flushed",
                "Count",
                StatResetPolicy::Monotonic,
                flushed_total,
            )?;
            emit_in_order_cause_stat(
                stats,
                core,
                "redirect_cause",
                cause,
                "flushed_cycles",
                "Cycle",
                StatResetPolicy::Monotonic,
                flushed_cycle_total,
            )?;
            emit_in_order_cause_stage_stats(
                stats,
                core,
                "redirect_cause",
                cause,
                "records",
                "Count",
                StatResetPolicy::Monotonic,
                stage_records.values(),
            )?;
            emit_in_order_cause_stage_stats(
                stats,
                core,
                "redirect_cause",
                cause,
                "flushed",
                "Count",
                StatResetPolicy::Monotonic,
                flushed.values(),
            )?;
            emit_in_order_cause_stage_stats(
                stats,
                core,
                "redirect_cause",
                cause,
                "flushed_cycles",
                "Cycle",
                StatResetPolicy::Monotonic,
                flushed_cycles.values(),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.stall_cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stall_cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.fetch_wait_cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_fetch_wait_cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.data_wait_cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_data_wait_cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.execute_wait_cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_execute_wait_cycles,
        )?;
        for (
            cause,
            record_count,
            records,
            resource_blocked,
            resource_blocked_cycles,
            ordering_blocked,
            ordering_blocked_cycles,
        ) in [
            (
                "fetch_wait",
                core.in_order_pipeline_fetch_wait_records,
                core.in_order_pipeline_fetch_wait_stage_records,
                core.in_order_pipeline_fetch_wait_stage_resource_blocked,
                core.in_order_pipeline_fetch_wait_stage_resource_blocked_cycles,
                core.in_order_pipeline_fetch_wait_stage_ordering_blocked,
                core.in_order_pipeline_fetch_wait_stage_ordering_blocked_cycles,
            ),
            (
                "data_wait",
                core.in_order_pipeline_data_wait_records,
                core.in_order_pipeline_data_wait_stage_records,
                core.in_order_pipeline_data_wait_stage_resource_blocked,
                core.in_order_pipeline_data_wait_stage_resource_blocked_cycles,
                core.in_order_pipeline_data_wait_stage_ordering_blocked,
                core.in_order_pipeline_data_wait_stage_ordering_blocked_cycles,
            ),
            (
                "execute_wait",
                core.in_order_pipeline_execute_wait_records,
                core.in_order_pipeline_execute_wait_stage_records,
                core.in_order_pipeline_execute_wait_stage_resource_blocked,
                core.in_order_pipeline_execute_wait_stage_resource_blocked_cycles,
                core.in_order_pipeline_execute_wait_stage_ordering_blocked,
                core.in_order_pipeline_execute_wait_stage_ordering_blocked_cycles,
            ),
        ] {
            emit_in_order_stall_cause_stat(
                stats,
                core,
                cause,
                "records",
                "Count",
                StatResetPolicy::Monotonic,
                record_count,
            )?;
            emit_in_order_stall_cause_stage_stats(
                stats,
                core,
                cause,
                "records",
                "Count",
                StatResetPolicy::Monotonic,
                records.values(),
            )?;
            emit_in_order_stall_cause_stage_stats(
                stats,
                core,
                cause,
                "resource_blocked",
                "Count",
                StatResetPolicy::Monotonic,
                resource_blocked.values(),
            )?;
            emit_in_order_stall_cause_stage_stats(
                stats,
                core,
                cause,
                "resource_blocked_cycles",
                "Cycle",
                StatResetPolicy::Monotonic,
                resource_blocked_cycles.values(),
            )?;
            emit_in_order_stall_cause_stage_stats(
                stats,
                core,
                cause,
                "ordering_blocked",
                "Count",
                StatResetPolicy::Monotonic,
                ordering_blocked.values(),
            )?;
            emit_in_order_stall_cause_stage_stats(
                stats,
                core,
                cause,
                "ordering_blocked_cycles",
                "Cycle",
                StatResetPolicy::Monotonic,
                ordering_blocked_cycles.values(),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.branch_predictions", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_branch_predictions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.branch_mispredictions",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_branch_mispredictions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.conditional_branch_predictions",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_conditional_branch_predictions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.conditional_branch_predicted_taken",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_conditional_branch_predicted_taken,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.conditional_branch_mispredictions",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_conditional_branch_mispredictions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.branch_prediction_flushes",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_branch_prediction_flushes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.branch_prediction_flush_cycles",
                core.cpu
            ),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_branch_prediction_flush_cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.redirects", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_redirects,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.branch_prediction_redirects",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_branch_prediction_redirects,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.trap_redirects", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_trap_redirects,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.interrupt_redirects", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_interrupt_redirects,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.interrupt_redirect_flushes",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_interrupt_redirect_flushes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.interrupt_redirect_flush_cycles",
                core.cpu
            ),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_interrupt_redirect_flush_cycles,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.trap_redirect_flushes",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_trap_redirect_flushes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.trap_redirect_flush_cycles",
                core.cpu
            ),
            "Cycle",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_trap_redirect_flush_cycles,
        )?;
        for (name, value) in [
            (
                "branch_speculation_predictions",
                core.in_order_pipeline_branch_speculation_predictions,
            ),
            (
                "branch_speculation_repairs",
                core.in_order_pipeline_branch_speculation_repairs,
            ),
            (
                "branch_speculation_removed_youngers",
                core.in_order_pipeline_branch_speculation_removed_youngers,
            ),
            (
                "branch_speculation_max_pending",
                core.in_order_pipeline_branch_speculation_max_pending,
            ),
        ] {
            increment_stat(
                stats,
                &format!("sim.cpu{}.pipeline.in_order.{name}", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        emit_o3_runtime_stats(stats, core, &gem5_cpu_alias_prefix)?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.lookups", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_lookups,
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.btb.lookups.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_target_buffer_lookup_kinds.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.hits", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_hits,
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.btb.hits.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_target_buffer_hit_kinds.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.misses", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_misses,
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.btb.misses.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_target_buffer_miss_kinds.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.updates", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_updates,
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.btb.updates.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_target_buffer_update_kinds.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.evictions", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_evictions,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.mispredictions", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_mispredictions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.branch_predictor.btb.predicted_taken_misses",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_predicted_taken_misses,
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.btb.mispredict_due_to_btb_miss.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_target_buffer_mispredict_due_to_btb_miss
                    .value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.branch_predictor.btb.mispredict_due_to_btb_miss.total",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_mispredict_due_to_btb_miss.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.lookups.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_lookups.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.lookups.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_lookups.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.squashes.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_squashes.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.squashes.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_squashes_total,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.indirect_hits", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_indirect_hits,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.indirect_mispredicted", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_indirect_mispredicted,
        )?;
        for (name, value) in [
            ("pushes", core.branch_predictor_ras.pushes()),
            ("pops", core.branch_predictor_ras.pops()),
            ("squashes", core.branch_predictor_ras.squashes()),
            ("used", core.branch_predictor_ras.used()),
            ("correct", core.branch_predictor_ras.correct()),
            ("incorrect", core.branch_predictor_ras.incorrect()),
        ] {
            increment_stat(
                stats,
                &format!("sim.cpu{}.branch_predictor.ras.{name}", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        for provider in BranchTargetProvider::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.target_provider.{}",
                    core.cpu,
                    provider.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_target_provider.value(provider),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.target_provider.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_target_provider.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.committed.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_committed.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.committed.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_committed.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.mispredicted.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_mispredicted.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.mispredicted.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_mispredicted.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.corrected.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_corrected.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.corrected.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_corrected.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.target_wrong.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_target_wrong.value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.target_wrong.total", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_target_wrong.total(),
        )?;
        for kind in BranchTargetKind::ALL {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.branch_predictor.mispredict_due_to_predictor.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.branch_predictor_mispredict_due_to_predictor
                    .value(kind),
            )?;
        }
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.branch_predictor.mispredict_due_to_predictor.total",
                core.cpu
            ),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_predictor_mispredict_due_to_predictor.total(),
        )?;
        emit_branch_predictor_counter_stats(
            stats,
            core,
            "gshare",
            [
                ("lookups", core.branch_predictor_gshare.lookups),
                (
                    "history_updates",
                    core.branch_predictor_gshare.history_updates,
                ),
                ("updates", core.branch_predictor_gshare.updates),
                ("squashes", core.branch_predictor_gshare.squashes),
            ],
        )?;
        emit_branch_predictor_counter_stats(
            stats,
            core,
            "bimode",
            [
                ("lookups", core.branch_predictor_bimode.lookups),
                (
                    "history_updates",
                    core.branch_predictor_bimode.history_updates,
                ),
                ("updates", core.branch_predictor_bimode.updates),
                ("squashes", core.branch_predictor_bimode.squashes),
            ],
        )?;
        emit_branch_predictor_counter_stats(
            stats,
            core,
            "tournament",
            [
                ("lookups", core.branch_predictor_tournament.lookups),
                (
                    "history_updates",
                    core.branch_predictor_tournament.history_updates,
                ),
                ("updates", core.branch_predictor_tournament.updates),
                ("squashes", core.branch_predictor_tournament.squashes),
            ],
        )?;
        for (name, value) in [
            ("local_predictions", core.tournament_local_predictions),
            ("global_predictions", core.tournament_global_predictions),
        ] {
            increment_stat(
                stats,
                &format!("sim.cpu{}.branch_predictor.tournament.{name}", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        emit_branch_predictor_counter_stats(
            stats,
            core,
            "tage_sc_l",
            [
                ("lookups", core.branch_predictor_tage_sc_l.lookups),
                (
                    "history_updates",
                    core.branch_predictor_tage_sc_l.history_updates,
                ),
                ("updates", core.branch_predictor_tage_sc_l.updates),
                ("repairs", core.branch_predictor_tage_sc_l.repairs),
                (
                    "selected_rollbacks",
                    core.branch_predictor_tage_sc_l.selected_rollbacks,
                ),
            ],
        )?;
        emit_branch_predictor_counter_stats(
            stats,
            core,
            "multiperspective_perceptron",
            [
                (
                    "lookups",
                    core.branch_predictor_multiperspective_perceptron.lookups,
                ),
                (
                    "updates",
                    core.branch_predictor_multiperspective_perceptron.updates,
                ),
                (
                    "selected_rollbacks",
                    core.branch_predictor_multiperspective_perceptron
                        .selected_rollbacks,
                ),
            ],
        )?;
        if let Some(checker) = &core.checker {
            increment_stat(
                stats,
                &format!("sim.cpu{}.checker.checked_instructions", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                checker.checked_instructions,
            )?;
            increment_stat(
                stats,
                &format!("sim.cpu{}.checker.mismatches", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                checker.mismatches,
            )?;
        }
        increment_stat(
            stats,
            &format!("sim.cpu{}.data.loads", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.data_loads,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.data.stores", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.data_stores,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.data.atomics", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.data_atomics,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.data.load_bytes", core.cpu),
            "Byte",
            StatResetPolicy::Monotonic,
            core.data_load_bytes,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.data.store_bytes", core.cpu),
            "Byte",
            StatResetPolicy::Monotonic,
            core.data_store_bytes,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.data.atomic_bytes", core.cpu),
            "Byte",
            StatResetPolicy::Monotonic,
            core.data_atomic_bytes,
        )?;
    }
    Ok(())
}

fn in_order_pipeline_stage_total(
    summary: crate::pipeline_stats::Rem6InOrderPipelineStageSummary,
) -> u64 {
    summary.values().into_iter().sum()
}

fn emit_branch_predictor_counter_stats<const N: usize>(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    family: &str,
    counters: [(&str, u64); N],
) -> Result<(), Rem6CliError> {
    for (name, value) in counters {
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.{family}.{name}", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}
