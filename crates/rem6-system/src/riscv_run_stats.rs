use std::collections::BTreeSet;

use rem6_cpu::{
    CpuId, RiscvCluster, RiscvClusterTurn, RiscvCoreDriveAction, RiscvDataAccessEventKind,
};
use rem6_kernel::Tick;
use rem6_stats::StatsRegistry;

use crate::{trap_event, ExecutionMode, RiscvO3RuntimeStats, RiscvSystemRunDriver, SystemError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvRetirementObservation {
    tick: Tick,
    cpu: CpuId,
    pc: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvRetirementSummary {
    count: u64,
    last_tick: Option<Tick>,
}

impl RiscvRetirementSummary {
    pub(crate) const fn count(self) -> u64 {
        self.count
    }

    pub(crate) const fn last_tick(self) -> Option<Tick> {
        self.last_tick
    }
}

impl RiscvSystemRunDriver {
    pub(crate) fn record_run_stats(
        &self,
        cluster: &RiscvCluster,
        tick: Tick,
        turn: &RiscvClusterTurn,
    ) -> Result<RiscvRetirementSummary, SystemError> {
        self.record_run_stats_inner(cluster, tick, turn, None)
    }

    pub(crate) fn record_run_stats_with_retirement_budget(
        &self,
        cluster: &RiscvCluster,
        tick: Tick,
        turn: &RiscvClusterTurn,
        retirement_budget: u64,
    ) -> Result<RiscvRetirementSummary, SystemError> {
        self.record_run_stats_inner(cluster, tick, turn, Some(retirement_budget))
    }

    fn record_run_stats_inner(
        &self,
        cluster: &RiscvCluster,
        tick: Tick,
        turn: &RiscvClusterTurn,
        retirement_budget: Option<u64>,
    ) -> Result<RiscvRetirementSummary, SystemError> {
        self.reset_runtime_stats_for_new_stats_resets(cluster)?;
        let retired =
            self.record_retirement_observations(cluster, turn, tick, retirement_budget)?;
        self.record_instruction_stats(&retired)?;
        self.record_data_access_stats(cluster)?;
        Ok(RiscvRetirementSummary {
            count: u64::try_from(retired.len()).unwrap_or(u64::MAX),
            last_tick: retired.iter().map(|instruction| instruction.tick).max(),
        })
    }

    #[cfg(test)]
    pub(crate) fn record_o3_runtime_stats(
        &self,
        cluster: &RiscvCluster,
        turn: &RiscvClusterTurn,
    ) -> Result<(), SystemError> {
        self.record_retirement_observations(cluster, turn, 0, None)
            .map(|_| ())
    }

    fn record_retirement_observations(
        &self,
        cluster: &RiscvCluster,
        turn: &RiscvClusterTurn,
        tick: Tick,
        retirement_budget: Option<u64>,
    ) -> Result<Vec<RiscvRetirementObservation>, SystemError> {
        let detailed_cpus = self.configured_detailed_cpus(cluster);
        let o3_authority_cpus = self.o3_authority_cpus(cluster, &detailed_cpus);
        let mut retired = Vec::new();
        let mut updated_cpus = BTreeSet::new();

        for event in turn.core_events() {
            match event.action() {
                RiscvCoreDriveAction::InstructionExecuted(instruction)
                    if instruction.counts_as_retired_instruction() =>
                {
                    let core = cluster
                        .core(event.cpu())
                        .map_err(SystemError::RiscvCluster)?;
                    let detailed = detailed_cpus.contains(&event.cpu());
                    let deferred_data_access = core.owns_pending_o3_live_data_access_retirement(
                        instruction.fetch().request_id(),
                    );
                    let owns_inherited_retirement =
                        core.owns_pending_o3_runtime_retirement(instruction.fetch().request_id());
                    if detailed && instruction.is_deferred_o3_data_access() {
                        assert!(
                            deferred_data_access,
                            "detailed deferred data execution must reserve CPU-owned retirement"
                        );
                    }
                    if !deferred_data_access {
                        retired.push(RiscvRetirementObservation {
                            tick,
                            cpu: event.cpu(),
                            pc: instruction.fetch_pc().get(),
                        });
                    }
                    if !deferred_data_access && (detailed || owns_inherited_retirement) {
                        core.record_o3_retired_instruction_with_trace(
                            instruction,
                            self.o3_runtime_trace_enabled,
                        );
                        updated_cpus.insert(event.cpu());
                    }
                }
                RiscvCoreDriveAction::DataAccessIssued { .. }
                    if o3_authority_cpus.contains(&event.cpu()) =>
                {
                    let core = cluster
                        .core(event.cpu())
                        .map_err(SystemError::RiscvCluster)?;
                    if !core.o3_live_data_access_lifecycle_is_quiescent() {
                        updated_cpus.insert(event.cpu());
                    }
                }
                RiscvCoreDriveAction::InstructionExecuted(_)
                | RiscvCoreDriveAction::FetchIssued { .. }
                | RiscvCoreDriveAction::PipelineCycleScheduled { .. }
                | RiscvCoreDriveAction::DataAccessIssued { .. } => {}
            }
        }

        let mut remaining_data_access_retirements = retirement_budget
            .unwrap_or(u64::MAX)
            .saturating_sub(u64::try_from(retired.len()).unwrap_or(u64::MAX));
        for cpu in o3_authority_cpus {
            let core = cluster.core(cpu).map_err(SystemError::RiscvCluster)?;
            loop {
                let kind = core.ready_o3_live_data_access_event_kind();
                if kind == Some(RiscvDataAccessEventKind::Completed)
                    && remaining_data_access_retirements == 0
                {
                    break;
                }
                let Some(instruction) = core.record_ready_o3_data_access_event_with_trace(
                    tick,
                    self.o3_runtime_trace_enabled,
                ) else {
                    break;
                };
                updated_cpus.insert(cpu);
                if instruction.data_access_event_kind() == Some(RiscvDataAccessEventKind::Completed)
                {
                    remaining_data_access_retirements =
                        remaining_data_access_retirements.saturating_sub(1);
                    retired.push(RiscvRetirementObservation {
                        tick,
                        cpu,
                        pc: instruction.fetch_pc().get(),
                    });
                }
            }
        }
        if self.o3_runtime_trace_enabled {
            updated_cpus.extend(cluster.core_ids());
        }
        if let Some(o3_runtime_stats) = &self.o3_runtime_stats {
            let controller = self.trap_port.controller();
            let mut controller = controller.lock().expect("system host controller lock");
            Self::sync_o3_runtime_stats(
                o3_runtime_stats,
                cluster,
                self.o3_runtime_trace_enabled,
                controller.executor_mut().stats_mut(),
                updated_cpus,
            )?;
        }
        Ok(retired)
    }

    fn configured_detailed_cpus(&self, cluster: &RiscvCluster) -> BTreeSet<CpuId> {
        {
            let controller = self.trap_port.controller();
            let controller = controller.lock().expect("system host controller lock");
            cluster
                .core_ids()
                .into_iter()
                .filter(|cpu| {
                    controller
                        .executor()
                        .execution_mode(&trap_event::execution_mode_target_for_cpu(*cpu))
                        .is_some_and(|mode| mode == ExecutionMode::Detailed)
                })
                .collect::<BTreeSet<_>>()
        }
    }

    fn o3_authority_cpus(
        &self,
        cluster: &RiscvCluster,
        configured: &BTreeSet<CpuId>,
    ) -> BTreeSet<CpuId> {
        cluster
            .core_ids()
            .into_iter()
            .filter(|cpu| {
                configured.contains(cpu)
                    || cluster
                        .core(*cpu)
                        .is_ok_and(|core| core.has_pending_o3_runtime_retirement())
            })
            .collect()
    }

    pub(crate) fn sync_o3_runtime_stats<I>(
        o3_runtime_stats: &RiscvO3RuntimeStats,
        cluster: &RiscvCluster,
        trace_enabled: bool,
        registry: &mut StatsRegistry,
        cpus: I,
    ) -> Result<(), SystemError>
    where
        I: IntoIterator<Item = CpuId>,
    {
        for cpu in cpus {
            let core = cluster.core(cpu).map_err(SystemError::RiscvCluster)?;
            let snapshot = core.o3_runtime_stats();
            let live_issue = core.o3_runtime_live_issue_telemetry();
            let runtime_snapshot = core.o3_runtime_snapshot();
            let trace_records = if trace_enabled {
                let trace_offset = o3_runtime_stats.trace_record_offset(cpu);
                let (next_trace_offset, trace_records) =
                    core.take_o3_runtime_trace_updates(trace_offset);
                o3_runtime_stats.set_trace_record_offset(cpu, next_trace_offset);
                trace_records
            } else {
                Vec::new()
            };
            let in_order_pipeline_cycles = core.in_order_pipeline_snapshot().cycle();
            o3_runtime_stats
                .record_cpu_snapshot(
                    registry,
                    cpu,
                    snapshot,
                    live_issue,
                    &runtime_snapshot,
                    &trace_records,
                    in_order_pipeline_cycles,
                )
                .map_err(SystemError::Stats)?;
        }
        Ok(())
    }

    fn record_instruction_stats(
        &self,
        retired: &[RiscvRetirementObservation],
    ) -> Result<(), SystemError> {
        let Some(instruction_stats) = &self.instruction_stats else {
            return Ok(());
        };

        let controller = self.trap_port.controller();
        let mut controller = controller.lock().expect("system host controller lock");
        let mut retired = retired.to_vec();
        retired.sort_by_key(|instruction| (instruction.tick, instruction.cpu));

        for instruction in retired {
            instruction_stats
                .record_retired_instruction_probe(instruction.cpu, instruction.tick, instruction.pc)
                .map_err(SystemError::Stats)?;
            if let Some(stat) = instruction_stats.committed_stat(instruction.cpu) {
                controller
                    .executor_mut()
                    .stats_mut()
                    .increment(stat, 1)
                    .map_err(SystemError::Stats)?;
            }
        }
        Ok(())
    }
}
