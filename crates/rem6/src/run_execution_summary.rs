use std::collections::BTreeMap;

use rem6_cpu::{
    BranchTargetKind, CpuId, InOrderPipelineStallCause, RiscvCluster, RiscvCoreDriveAction,
    TournamentPredictorSelection,
};
use rem6_isa_riscv::Register;
use rem6_memory::CacheLineLayout;
use rem6_system::{RiscvSyscallTraceRecord, RiscvSystemRun, RiscvSystemRunStopReason};
use rem6_transport::MemoryTrace;

use crate::data_access_counts::core_data_access_counts;
use crate::data_cache_runtime::CliDataCacheSummary;
use crate::parallel_stats::{
    parallel_frontier_summaries, parallel_partition_summaries, parallel_ready_partition_summaries,
    parallel_worker_lane_summaries, parallel_worker_slot_summaries,
};
use crate::pipeline_stats::{
    in_order_pipeline_data_wait_cycles, in_order_pipeline_execute_wait_cycles,
    in_order_pipeline_fetch_wait_cycles, in_order_pipeline_run_summary,
    in_order_pipeline_stage_branch_prediction_flushed,
    in_order_pipeline_stage_branch_prediction_flushed_cycles, in_order_pipeline_stage_flushed,
    in_order_pipeline_stage_flushed_cycles, in_order_pipeline_stage_in_flight,
    in_order_pipeline_stage_max_in_flight, in_order_pipeline_stage_occupied_cycles,
    in_order_pipeline_stage_ordering_blocked, in_order_pipeline_stage_ordering_blocked_cycles,
    in_order_pipeline_stage_resource_blocked, in_order_pipeline_stage_resource_blocked_cycles,
    in_order_pipeline_stage_resource_blocked_cycles_for_stall_cause,
    in_order_pipeline_stage_resource_blocked_for_stall_cause,
    in_order_pipeline_stage_trap_redirect_flushed,
    in_order_pipeline_stage_trap_redirect_flushed_cycles, in_order_pipeline_stage_widths,
};
use crate::runtime_memory::{read_memory_dumps, CliMemoryRuntime};
use crate::{
    execute_error, guest_trap_name, instruction_probe_summary, memory_transport_summary,
    Rem6BranchPredictorCounterSummary, Rem6CheckerSummary, Rem6CliError, Rem6CoreSummary,
    Rem6DataAccessProbeSummary, Rem6DebugSummary, Rem6ExecutionStop, Rem6ExecutionSummary,
    Rem6HostActionSummary, Rem6MemoryResourceInputs, Rem6MemoryResourceSummary,
    Rem6MultiperspectivePerceptronCounterSummary, Rem6RiscvGuestWriteSummary,
    Rem6RiscvSbiConsoleSummary, Rem6RiscvSbiHsmStatusSummary, Rem6RiscvSbiHsmSummary,
    Rem6RiscvSbiHsmWakeSummary, Rem6RiscvSbiIpiSummary, Rem6RiscvSbiResetSummary,
    Rem6RiscvSbiRfenceCompletionSummary, Rem6RiscvSbiRfenceSummary, Rem6RiscvSbiTimerSummary,
    Rem6RiscvUnknownSyscallSummary, Rem6RunConfig, Rem6RunFabricSummary, Rem6SbiTraceInputs,
    Rem6TageScLBranchPredictorCounterSummary, RISCV_DATA_PROBE_PAGE_BYTES,
};

pub(super) struct ExecutionSummaryInputs<'a> {
    pub(super) core_count: u32,
    pub(super) memory: &'a CliMemoryRuntime,
    pub(super) line_layout: CacheLineLayout,
    pub(super) config: &'a Rem6RunConfig,
    pub(super) instruction_cache: CliDataCacheSummary,
    pub(super) instruction_cache_l2: CliDataCacheSummary,
    pub(super) instruction_cache_l3: CliDataCacheSummary,
    pub(super) data_cache: CliDataCacheSummary,
    pub(super) data_cache_l2: CliDataCacheSummary,
    pub(super) data_cache_l3: CliDataCacheSummary,
    pub(super) fetch_trace: &'a MemoryTrace,
    pub(super) data_trace: &'a MemoryTrace,
    pub(super) fabric: Rem6RunFabricSummary,
    pub(super) riscv_guest_writes: Vec<Rem6RiscvGuestWriteSummary>,
    pub(super) riscv_unknown_syscalls: Vec<Rem6RiscvUnknownSyscallSummary>,
    pub(super) riscv_sbi_console: Rem6RiscvSbiConsoleSummary,
    pub(super) riscv_sbi_timers: Vec<Rem6RiscvSbiTimerSummary>,
    pub(super) riscv_sbi_hsm_events: Vec<Rem6RiscvSbiHsmSummary>,
    pub(super) riscv_sbi_hsm_wakes: Vec<Rem6RiscvSbiHsmWakeSummary>,
    pub(super) riscv_sbi_hsm_statuses: Vec<Rem6RiscvSbiHsmStatusSummary>,
    pub(super) riscv_sbi_ipis: Vec<Rem6RiscvSbiIpiSummary>,
    pub(super) riscv_sbi_rfences: Vec<Rem6RiscvSbiRfenceSummary>,
    pub(super) riscv_sbi_rfence_completions: Vec<Rem6RiscvSbiRfenceCompletionSummary>,
    pub(super) riscv_sbi_resets: Vec<Rem6RiscvSbiResetSummary>,
    pub(super) riscv_syscall_trace: Vec<RiscvSyscallTraceRecord>,
    pub(super) host_actions: Rem6HostActionSummary,
    pub(super) prior_committed_by_cpu: BTreeMap<CpuId, u64>,
}

pub(super) fn execution_summary(
    cluster: &RiscvCluster,
    run: &RiscvSystemRun,
    inputs: ExecutionSummaryInputs<'_>,
) -> Result<Rem6ExecutionSummary, Rem6CliError> {
    let mut committed_by_cpu = inputs.prior_committed_by_cpu.clone();
    for (cpu, committed) in committed_instructions_by_cpu(run) {
        *committed_by_cpu.entry(cpu).or_insert(0) += committed;
    }
    let committed_instructions = committed_by_cpu.values().sum();
    let final_tick = run.final_tick().ok_or_else(|| Rem6CliError::Execute {
        error: "RISC-V execution stopped without a final tick".to_string(),
    })?;
    let tournament_selection_counts = tournament_selection_counts_by_cpu(run);
    let stop = match run.stop_reason() {
        RiscvSystemRunStopReason::HostStop(stop) => {
            if let Some(scheduled_trap) = run.scheduled_traps().first() {
                Rem6ExecutionStop::HostTrap {
                    stop_code: stop.code(),
                    trap: guest_trap_name(scheduled_trap.trap().kind()),
                    trap_pc: scheduled_trap.trap().pc(),
                }
            } else {
                Rem6ExecutionStop::HostStop {
                    stop_code: stop.code(),
                }
            }
        }
        RiscvSystemRunStopReason::DebugStop { .. } => {
            return Err(Rem6CliError::Execute {
                error: "RISC-V execution stopped at a debugger watchpoint".to_string(),
            });
        }
        RiscvSystemRunStopReason::InstructionLimit { limit, .. } => {
            Rem6ExecutionStop::InstructionLimit {
                instruction_limit: inputs.config.max_instructions().unwrap_or(limit),
            }
        }
        RiscvSystemRunStopReason::TickLimit { limit, .. } => {
            Rem6ExecutionStop::TickLimit { tick_limit: limit }
        }
        RiscvSystemRunStopReason::Idle { .. } => {
            if inputs
                .riscv_sbi_hsm_events
                .iter()
                .any(Rem6RiscvSbiHsmSummary::is_hart_stop)
            {
                Rem6ExecutionStop::Idle
            } else {
                return Err(Rem6CliError::Execute {
                    error: "RISC-V execution stopped without a host trap".to_string(),
                });
            }
        }
    };
    let mut cores = Vec::new();
    let mut data_loads = 0;
    let mut data_stores = 0;
    let mut data_atomics = 0;
    let mut data_load_bytes = 0;
    let mut data_store_bytes = 0;
    let mut data_atomic_bytes = 0;
    for cpu_index in 0..inputs.core_count {
        let cpu = CpuId::new(cpu_index);
        let core = cluster.core(cpu).map_err(execute_error)?;
        let data = core_data_access_counts(&core);
        data_loads += data.loads;
        data_stores += data.stores;
        data_atomics += data.atomics;
        data_load_bytes += data.load_bytes;
        data_store_bytes += data.store_bytes;
        data_atomic_bytes += data.atomic_bytes;
        let mut registers = Vec::new();
        for register_index in 1..32 {
            let register = Register::new(register_index).map_err(execute_error)?;
            let value = core.read_register(register);
            if value != 0 {
                registers.push((register_index, value));
            }
        }
        let pipeline_summary = in_order_pipeline_run_summary(&core);
        let pipeline_snapshot = core.in_order_pipeline_snapshot();
        let pipeline_stage_in_flight = in_order_pipeline_stage_in_flight(&pipeline_snapshot);
        let pipeline_stage_widths = in_order_pipeline_stage_widths(&pipeline_snapshot);
        let pipeline_stage_max_in_flight =
            in_order_pipeline_stage_max_in_flight(&core, &pipeline_snapshot);
        let pipeline_stage_occupied_cycles = in_order_pipeline_stage_occupied_cycles(&core);
        let pipeline_stage_resource_blocked = in_order_pipeline_stage_resource_blocked(&core);
        let pipeline_stage_resource_blocked_cycles =
            in_order_pipeline_stage_resource_blocked_cycles(&core);
        let pipeline_fetch_wait_stage_resource_blocked =
            in_order_pipeline_stage_resource_blocked_for_stall_cause(
                &core,
                InOrderPipelineStallCause::FetchWait,
            );
        let pipeline_fetch_wait_stage_resource_blocked_cycles =
            in_order_pipeline_stage_resource_blocked_cycles_for_stall_cause(
                &core,
                InOrderPipelineStallCause::FetchWait,
            );
        let pipeline_data_wait_stage_resource_blocked =
            in_order_pipeline_stage_resource_blocked_for_stall_cause(
                &core,
                InOrderPipelineStallCause::DataWait,
            );
        let pipeline_data_wait_stage_resource_blocked_cycles =
            in_order_pipeline_stage_resource_blocked_cycles_for_stall_cause(
                &core,
                InOrderPipelineStallCause::DataWait,
            );
        let pipeline_execute_wait_stage_resource_blocked =
            in_order_pipeline_stage_resource_blocked_for_stall_cause(
                &core,
                InOrderPipelineStallCause::ExecuteWait,
            );
        let pipeline_execute_wait_stage_resource_blocked_cycles =
            in_order_pipeline_stage_resource_blocked_cycles_for_stall_cause(
                &core,
                InOrderPipelineStallCause::ExecuteWait,
            );
        let pipeline_stage_ordering_blocked = in_order_pipeline_stage_ordering_blocked(&core);
        let pipeline_stage_ordering_blocked_cycles =
            in_order_pipeline_stage_ordering_blocked_cycles(&core);
        let pipeline_stage_flushed = in_order_pipeline_stage_flushed(&core);
        let pipeline_stage_flushed_cycles = in_order_pipeline_stage_flushed_cycles(&core);
        let pipeline_stage_branch_prediction_flushed =
            in_order_pipeline_stage_branch_prediction_flushed(&core);
        let pipeline_stage_branch_prediction_flushed_cycles =
            in_order_pipeline_stage_branch_prediction_flushed_cycles(&core);
        let pipeline_stage_trap_redirect_flushed =
            in_order_pipeline_stage_trap_redirect_flushed(&core);
        let pipeline_stage_trap_redirect_flushed_cycles =
            in_order_pipeline_stage_trap_redirect_flushed_cycles(&core);
        let branch_speculation_summary = core.branch_speculation_summary();
        let branch_target_buffer = core.branch_target_buffer_snapshot();
        let gshare_branch_predictor = core.gshare_branch_predictor_snapshot();
        let bimode_branch_predictor = core.bimode_branch_predictor_snapshot();
        let tournament_branch_predictor = core.tournament_branch_predictor_snapshot();
        let tage_sc_l_branch_predictor = core.tage_sc_l_branch_predictor_snapshot();
        let multiperspective_perceptron = core.multiperspective_perceptron_snapshot();
        let mispredicted_branch_kinds = branch_speculation_summary.mispredicted_branch_kinds();
        let branch_predictor_indirect_mispredicted = BranchTargetKind::ALL
            .into_iter()
            .filter(|kind| kind.is_indirect_non_return())
            .fold(0_u64, |total, kind| {
                total.saturating_add(mispredicted_branch_kinds.value(kind))
            });
        let tournament_selection_counts = tournament_selection_counts
            .get(&cpu)
            .copied()
            .unwrap_or_default();
        let checker = core
            .checker_cpu_snapshot()
            .map(|snapshot| Rem6CheckerSummary {
                checked_instructions: snapshot.checked_instructions(),
                mismatches: snapshot.mismatches().len() as u64,
            });
        cores.push(Rem6CoreSummary {
            cpu: cpu_index,
            pc: core.pc().get(),
            committed_instructions: committed_by_cpu.get(&cpu).copied().unwrap_or(0),
            in_order_pipeline_cycles: pipeline_snapshot.cycle(),
            in_order_pipeline_in_flight: pipeline_snapshot.in_flight().len() as u64,
            in_order_pipeline_stage_widths: pipeline_stage_widths,
            in_order_pipeline_stage_in_flight: pipeline_stage_in_flight,
            in_order_pipeline_stage_max_in_flight: pipeline_stage_max_in_flight,
            in_order_pipeline_stage_occupied_cycles: pipeline_stage_occupied_cycles,
            in_order_pipeline_stage_resource_blocked: pipeline_stage_resource_blocked,
            in_order_pipeline_stage_resource_blocked_cycles: pipeline_stage_resource_blocked_cycles,
            in_order_pipeline_fetch_wait_stage_resource_blocked:
                pipeline_fetch_wait_stage_resource_blocked,
            in_order_pipeline_fetch_wait_stage_resource_blocked_cycles:
                pipeline_fetch_wait_stage_resource_blocked_cycles,
            in_order_pipeline_data_wait_stage_resource_blocked:
                pipeline_data_wait_stage_resource_blocked,
            in_order_pipeline_data_wait_stage_resource_blocked_cycles:
                pipeline_data_wait_stage_resource_blocked_cycles,
            in_order_pipeline_execute_wait_stage_resource_blocked:
                pipeline_execute_wait_stage_resource_blocked,
            in_order_pipeline_execute_wait_stage_resource_blocked_cycles:
                pipeline_execute_wait_stage_resource_blocked_cycles,
            in_order_pipeline_stage_ordering_blocked: pipeline_stage_ordering_blocked,
            in_order_pipeline_stage_ordering_blocked_cycles: pipeline_stage_ordering_blocked_cycles,
            in_order_pipeline_stage_flushed: pipeline_stage_flushed,
            in_order_pipeline_stage_flushed_cycles: pipeline_stage_flushed_cycles,
            in_order_pipeline_stage_branch_prediction_flushed:
                pipeline_stage_branch_prediction_flushed,
            in_order_pipeline_stage_branch_prediction_flushed_cycles:
                pipeline_stage_branch_prediction_flushed_cycles,
            in_order_pipeline_stage_trap_redirect_flushed: pipeline_stage_trap_redirect_flushed,
            in_order_pipeline_stage_trap_redirect_flushed_cycles:
                pipeline_stage_trap_redirect_flushed_cycles,
            in_order_pipeline_retired: pipeline_summary.retired_count() as u64,
            in_order_pipeline_advanced: pipeline_summary.advanced_count() as u64,
            in_order_pipeline_flushed: pipeline_summary.flushed_count() as u64,
            in_order_pipeline_flush_cycles: pipeline_summary.flush_cycle_count() as u64,
            in_order_pipeline_resource_blocked: pipeline_summary.resource_blocked_count() as u64,
            in_order_pipeline_ordering_blocked: pipeline_summary.ordering_blocked_count() as u64,
            in_order_pipeline_stall_cycles: pipeline_summary.stall_cycle_count(),
            in_order_pipeline_fetch_wait_cycles: in_order_pipeline_fetch_wait_cycles(&core),
            in_order_pipeline_data_wait_cycles: in_order_pipeline_data_wait_cycles(&core),
            in_order_pipeline_execute_wait_cycles: in_order_pipeline_execute_wait_cycles(&core),
            in_order_pipeline_branch_predictions: pipeline_summary.branch_prediction_count() as u64,
            in_order_pipeline_branch_mispredictions: pipeline_summary.branch_misprediction_count()
                as u64,
            in_order_pipeline_conditional_branch_predictions: pipeline_summary
                .conditional_branch_prediction_count()
                as u64,
            in_order_pipeline_conditional_branch_predicted_taken: pipeline_summary
                .conditional_branch_predicted_taken_count()
                as u64,
            in_order_pipeline_conditional_branch_mispredictions: pipeline_summary
                .conditional_branch_misprediction_count()
                as u64,
            in_order_pipeline_branch_prediction_flushes: pipeline_summary
                .branch_prediction_flushed_count()
                as u64,
            in_order_pipeline_branch_prediction_flush_cycles: pipeline_summary
                .branch_prediction_flush_cycle_count()
                as u64,
            in_order_pipeline_redirects: pipeline_summary.redirect_count() as u64,
            in_order_pipeline_branch_prediction_redirects: pipeline_summary
                .branch_prediction_redirect_count()
                as u64,
            in_order_pipeline_trap_redirects: pipeline_summary.trap_redirect_count() as u64,
            in_order_pipeline_trap_redirect_flushes: pipeline_summary.trap_redirect_flushed_count()
                as u64,
            in_order_pipeline_trap_redirect_flush_cycles: pipeline_summary
                .trap_redirect_flush_cycle_count()
                as u64,
            in_order_pipeline_branch_speculation_predictions: branch_speculation_summary
                .predictions(),
            in_order_pipeline_branch_speculation_repairs: branch_speculation_summary.repairs(),
            in_order_pipeline_branch_speculation_removed_youngers: branch_speculation_summary
                .removed_youngers(),
            in_order_pipeline_branch_speculation_max_pending: branch_speculation_summary
                .max_pending(),
            o3_runtime: core.o3_runtime_stats(),
            branch_target_buffer_lookups: branch_target_buffer.lookup_count(),
            branch_target_buffer_hits: branch_target_buffer.hit_count(),
            branch_target_buffer_misses: branch_target_buffer.miss_count(),
            branch_target_buffer_updates: branch_target_buffer.update_count(),
            branch_target_buffer_evictions: branch_target_buffer.eviction_count(),
            branch_target_buffer_lookup_kinds: branch_target_buffer.lookup_kind_counts(),
            branch_target_buffer_hit_kinds: branch_target_buffer.hit_kind_counts(),
            branch_target_buffer_miss_kinds: branch_target_buffer.miss_kind_counts(),
            branch_target_buffer_update_kinds: branch_target_buffer.update_kind_counts(),
            branch_target_buffer_mispredictions: branch_speculation_summary.btb_mispredictions(),
            branch_target_buffer_predicted_taken_misses: branch_speculation_summary
                .predicted_taken_btb_misses(),
            branch_target_buffer_mispredict_due_to_btb_miss: branch_speculation_summary
                .btb_mispredict_due_to_btb_miss(),
            branch_predictor_lookups: branch_speculation_summary.lookup_branch_kinds(),
            branch_predictor_squashes: branch_speculation_summary.squashed_branch_kinds(),
            branch_predictor_squashes_total: branch_speculation_summary.removed_youngers(),
            branch_predictor_target_provider: branch_speculation_summary.target_provider(),
            branch_predictor_indirect_hits: branch_speculation_summary.indirect_hits(),
            branch_predictor_indirect_mispredicted,
            branch_predictor_ras: branch_speculation_summary.return_address_stack(),
            branch_predictor_committed: branch_speculation_summary.committed_branch_kinds(),
            branch_predictor_mispredicted: mispredicted_branch_kinds,
            branch_predictor_corrected: branch_speculation_summary.corrected_branch_kinds(),
            branch_predictor_target_wrong: branch_speculation_summary.target_wrong_branch_kinds(),
            branch_predictor_mispredict_due_to_predictor: branch_speculation_summary
                .mispredict_due_to_predictor(),
            branch_predictor_gshare: Rem6BranchPredictorCounterSummary {
                lookups: gshare_branch_predictor.lookup_count(),
                history_updates: gshare_branch_predictor.history_update_count(),
                updates: gshare_branch_predictor.update_count(),
                squashes: gshare_branch_predictor.squash_count(),
            },
            branch_predictor_bimode: Rem6BranchPredictorCounterSummary {
                lookups: bimode_branch_predictor.lookup_count(),
                history_updates: bimode_branch_predictor.history_update_count(),
                updates: bimode_branch_predictor.update_count(),
                squashes: bimode_branch_predictor.squash_count(),
            },
            branch_predictor_tournament: Rem6BranchPredictorCounterSummary {
                lookups: tournament_branch_predictor.lookup_count(),
                history_updates: tournament_branch_predictor.history_update_count(),
                updates: tournament_branch_predictor.update_count(),
                squashes: tournament_branch_predictor.squash_count(),
            },
            branch_predictor_tage_sc_l: Rem6TageScLBranchPredictorCounterSummary {
                lookups: tage_sc_l_branch_predictor.lookup_count(),
                history_updates: tage_sc_l_branch_predictor.history_update_count(),
                updates: tage_sc_l_branch_predictor.update_count(),
                repairs: tage_sc_l_branch_predictor.repair_count(),
                selected_rollbacks: core.selected_tage_sc_l_branch_predictor_rollback_count(),
            },
            branch_predictor_multiperspective_perceptron:
                Rem6MultiperspectivePerceptronCounterSummary {
                    lookups: multiperspective_perceptron.lookup_count(),
                    updates: multiperspective_perceptron.update_count(),
                    selected_rollbacks: core.selected_multiperspective_perceptron_rollback_count(),
                },
            tournament_local_predictions: tournament_selection_counts.local_predictions,
            tournament_global_predictions: tournament_selection_counts.global_predictions,
            data_loads: data.loads,
            data_stores: data.stores,
            data_atomics: data.atomics,
            data_load_bytes: data.load_bytes,
            data_store_bytes: data.store_bytes,
            data_atomic_bytes: data.atomic_bytes,
            checker,
            registers,
        });
    }

    let fetch_transport = memory_transport_summary(inputs.fetch_trace);
    let data_transport = memory_transport_summary(inputs.data_trace);
    let dram = inputs.memory.dram_summary_until(final_tick);
    let memory_resources =
        Rem6MemoryResourceSummary::from_run_resources(Rem6MemoryResourceInputs {
            instruction_caches: [
                &inputs.instruction_cache,
                &inputs.instruction_cache_l2,
                &inputs.instruction_cache_l3,
            ],
            data_caches: [
                &inputs.data_cache,
                &inputs.data_cache_l2,
                &inputs.data_cache_l3,
            ],
            fetch_transport: &fetch_transport,
            data_transport: &data_transport,
            fabric: &inputs.fabric,
            dram: &dram,
        });
    let power_records = crate::power_output::run_power_analysis_records_from_parts(
        final_tick,
        &cores,
        &inputs.instruction_cache,
        &inputs.data_cache,
        &memory_resources,
        &dram,
    );
    let debug = Rem6DebugSummary::from_run(
        inputs.config,
        cluster,
        run,
        inputs.fetch_trace,
        inputs.data_trace,
        &inputs.fabric,
        &dram,
        &memory_resources,
        &power_records,
        &inputs.riscv_syscall_trace,
        &inputs.host_actions,
        Rem6SbiTraceInputs {
            console: &inputs.riscv_sbi_console,
            timers: &inputs.riscv_sbi_timers,
            hsm_events: &inputs.riscv_sbi_hsm_events,
            hsm_wakes: &inputs.riscv_sbi_hsm_wakes,
            hsm_statuses: &inputs.riscv_sbi_hsm_statuses,
            ipis: &inputs.riscv_sbi_ipis,
            rfences: &inputs.riscv_sbi_rfences,
            rfence_completions: &inputs.riscv_sbi_rfence_completions,
            resets: &inputs.riscv_sbi_resets,
        },
    );

    Ok(Rem6ExecutionSummary {
        final_tick,
        stop,
        committed_instructions,
        data_loads,
        data_stores,
        data_atomics,
        data_load_bytes,
        data_store_bytes,
        data_atomic_bytes,
        instruction_cache: inputs.instruction_cache,
        instruction_cache_l2: inputs.instruction_cache_l2,
        instruction_cache_l3: inputs.instruction_cache_l3,
        data_cache: inputs.data_cache,
        data_cache_l2: inputs.data_cache_l2,
        data_cache_l3: inputs.data_cache_l3,
        instruction_probes: instruction_probe_summary(run),
        data_access_probes: data_access_probe_summary(
            run,
            inputs.line_layout,
            RISCV_DATA_PROBE_PAGE_BYTES,
        ),
        parallel_scheduler_epochs: run.parallel_scheduler_epochs().len() as u64,
        parallel_scheduler_dispatches: run.parallel_scheduler_dispatches().len() as u64,
        parallel_scheduler_batches: run.parallel_scheduler_batches().len() as u64,
        parallel_scheduler_max_workers: run.max_parallel_scheduler_workers() as u64,
        parallel_scheduler_total_workers: run.parallel_scheduler_workers().len() as u64,
        parallel_scheduler_active_partitions: run.active_parallel_scheduler_partition_count()
            as u64,
        parallel_scheduler_remote_sends: run.parallel_scheduler_total_remote_send_count() as u64,
        parallel_scheduler_batch_worker_ticks: run.parallel_scheduler_batch_worker_ticks(),
        parallel_scheduler_batch_worker_capacity_ticks: run
            .parallel_scheduler_batch_worker_capacity_ticks(),
        parallel_scheduler_batch_idle_worker_ticks: run
            .parallel_scheduler_batch_idle_worker_ticks(),
        parallel_scheduler_worker_slots: parallel_worker_slot_summaries(run),
        parallel_scheduler_worker_lanes: parallel_worker_lane_summaries(run),
        parallel_scheduler_partitions: parallel_partition_summaries(run),
        parallel_scheduler_frontiers: parallel_frontier_summaries(
            run.parallel_scheduler_frontiers(),
        ),
        parallel_scheduler_final_frontiers: parallel_frontier_summaries(
            run.parallel_scheduler_final_frontiers(),
        ),
        parallel_scheduler_ready_partitions: parallel_ready_partition_summaries(
            run.parallel_scheduler_ready_partitions(),
        ),
        fetch_transport,
        data_transport,
        fabric: inputs.fabric,
        dram,
        memory_resources,
        debug,
        cores,
        memory_dumps: read_memory_dumps(
            inputs.memory,
            inputs.line_layout,
            inputs.config.memory_dumps(),
        )?,
        riscv_guest_writes: inputs.riscv_guest_writes,
        riscv_unknown_syscalls: inputs.riscv_unknown_syscalls,
        riscv_sbi_console: inputs.riscv_sbi_console,
        riscv_sbi_timers: inputs.riscv_sbi_timers,
        riscv_sbi_hsm_events: inputs.riscv_sbi_hsm_events,
        riscv_sbi_hsm_wakes: inputs.riscv_sbi_hsm_wakes,
        riscv_sbi_hsm_statuses: inputs.riscv_sbi_hsm_statuses,
        riscv_sbi_ipis: inputs.riscv_sbi_ipis,
        riscv_sbi_rfences: inputs.riscv_sbi_rfences,
        riscv_sbi_rfence_completions: inputs.riscv_sbi_rfence_completions,
        riscv_sbi_resets: inputs.riscv_sbi_resets,
        host_actions: inputs.host_actions,
    })
}

fn committed_instructions_by_cpu(run: &RiscvSystemRun) -> BTreeMap<CpuId, u64> {
    let mut committed = BTreeMap::new();
    for event in run.turns().iter().flat_map(|turn| turn.core_events()) {
        let RiscvCoreDriveAction::InstructionExecuted(instruction) = event.action() else {
            continue;
        };
        if instruction.counts_as_retired_instruction() {
            *committed.entry(event.cpu()).or_insert(0) += 1;
        }
    }
    committed
}

fn data_access_probe_summary(
    run: &RiscvSystemRun,
    line_layout: CacheLineLayout,
    page_bytes: u64,
) -> Rem6DataAccessProbeSummary {
    run.data_access_probes()
        .map(|probes| {
            let stack_distance = probes.stack_distance();
            let histograms = stack_distance.histograms();
            let infinite_samples = stack_distance.infinite_samples();
            let finite_samples = stack_distance.finite_samples();
            let footprint = probes.memory_footprint();
            Rem6DataAccessProbeSummary {
                sample_count: infinite_samples.saturating_add(finite_samples),
                stack_distance_infinite_samples: infinite_samples,
                stack_distance_finite_samples: finite_samples,
                stack_distance_stack_depth: stack_distance.stack().len() as u64,
                stack_distance_read_linear: histograms.read_linear().to_vec(),
                stack_distance_write_linear: histograms.write_linear().to_vec(),
                stack_distance_read_log: histograms.read_log().to_vec(),
                stack_distance_write_log: histograms.write_log().to_vec(),
                memory_footprint_cache_line_bytes: footprint
                    .map(|snapshot| {
                        (snapshot.cache_lines().len() as u64).saturating_mul(line_layout.bytes())
                    })
                    .unwrap_or(0),
                memory_footprint_cache_line_total_bytes: footprint
                    .map(|snapshot| {
                        (snapshot.cache_lines_total().len() as u64)
                            .saturating_mul(line_layout.bytes())
                    })
                    .unwrap_or(0),
                memory_footprint_page_bytes: footprint
                    .map(|snapshot| (snapshot.pages().len() as u64).saturating_mul(page_bytes))
                    .unwrap_or(0),
                memory_footprint_page_total_bytes: footprint
                    .map(|snapshot| {
                        (snapshot.pages_total().len() as u64).saturating_mul(page_bytes)
                    })
                    .unwrap_or(0),
            }
        })
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TournamentSelectionCounts {
    local_predictions: u64,
    global_predictions: u64,
}

fn tournament_selection_counts_by_cpu(
    run: &RiscvSystemRun,
) -> BTreeMap<CpuId, TournamentSelectionCounts> {
    let mut counts: BTreeMap<CpuId, TournamentSelectionCounts> = BTreeMap::new();
    for event in run.turns().iter().flat_map(|turn| turn.core_events()) {
        let RiscvCoreDriveAction::InstructionExecuted(instruction) = event.action() else {
            continue;
        };
        let Some(update) = instruction.tournament_branch_update() else {
            continue;
        };
        let entry = counts.entry(event.cpu()).or_insert_with(Default::default);
        match update.prediction().selection() {
            TournamentPredictorSelection::Local => entry.local_predictions += 1,
            TournamentPredictorSelection::Global => entry.global_predictions += 1,
        }
    }
    counts
}
