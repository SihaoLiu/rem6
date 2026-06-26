use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
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
            "resource_blocked",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_resource_blocked.values(),
        )?;
        emit_in_order_stage_stats(
            stats,
            core,
            "ordering_blocked",
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_stage_ordering_blocked.values(),
        )?;
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
            &format!("sim.cpu{}.pipeline.in_order.redirects", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.in_order_pipeline_redirects,
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
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.lookups", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_lookups,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.hits", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_hits,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.misses", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_misses,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.branch_predictor.btb.updates", core.cpu),
            "Count",
            StatResetPolicy::Monotonic,
            core.branch_target_buffer_updates,
        )?;
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

fn emit_in_order_stage_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    name: &str,
    unit: &'static str,
    reset_policy: StatResetPolicy,
    values: [u64; 5],
) -> Result<(), Rem6CliError> {
    for (stage, value) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .into_iter()
        .zip(values)
    {
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.stage.{stage}.{name}", core.cpu),
            unit,
            reset_policy,
            value,
        )?;
    }
    Ok(())
}
