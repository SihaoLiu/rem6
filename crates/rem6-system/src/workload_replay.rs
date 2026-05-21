use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootError;
use rem6_cpu::{CpuCore, CpuError, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, AgentId, CacheLineLayout, MemoryError, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId, TransportError,
};
use rem6_workload::{
    HostEventIntent, WorkloadError, WorkloadHostEvent, WorkloadReplayPlan, WorkloadResult,
    WorkloadRouteId,
};

use crate::{
    GuestEvent, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId, HostEventPolicy,
    RiscvSystemRun, RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort,
    SystemActionOutcome, SystemHostController, SystemHostEventPort,
};

const DEFAULT_MAX_TURNS: usize = 64;
const WORKLOAD_STOP_REASON_HOST: &str = "host-stop";
const WORKLOAD_STOP_REASON_IDLE: &str = "idle";
const PLANNED_HOST_EVENT_ID_BASE: u64 = 10_000;

#[derive(Clone, Debug)]
pub struct RiscvWorkloadReplay {
    plan: WorkloadReplayPlan,
    max_turns: usize,
}

impl RiscvWorkloadReplay {
    pub const fn new(plan: WorkloadReplayPlan) -> Self {
        Self {
            plan,
            max_turns: DEFAULT_MAX_TURNS,
        }
    }

    pub const fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    pub fn run_parallel(&self) -> Result<RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let route_map = self.build_route_map()?;
        let store = Arc::new(Mutex::new(self.load_memory()?));
        let cluster = self.build_cluster(&route_map)?;
        let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(
            topology.partition_count(),
            topology.min_remote_delay(),
            topology.parallel_worker_limit(),
        )
        .map_err(RiscvWorkloadReplayError::Scheduler)?;
        let transport = self.build_transport()?;
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        self.schedule_planned_host_events(
            &mut scheduler,
            &controller,
            PartitionId::new(topology.host().partition()),
            GuestSourceId::new(topology.host().source()),
        )?;
        let trap_port = RiscvTrapEventPort::new(
            SystemHostEventPort::with_controller(
                PartitionId::new(topology.host().partition()),
                topology.host().latency(),
                Arc::clone(&controller),
            )
            .map_err(RiscvWorkloadReplayError::System)?,
            GuestSourceId::new(topology.host().source()),
        );
        let driver = RiscvSystemRunDriver::new(trap_port);
        let run = driver
            .drive_until_host_stop_parallel(
                &cluster,
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    let store = Arc::clone(&store);
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = Arc::clone(&store);
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                self.max_turns,
                |cpu| GuestEventId::new(1_000 + u64::from(cpu.get())),
            )
            .map_err(RiscvWorkloadReplayError::System)?;
        let (result, host_action_outcomes) = self.result_from_run(&run, &controller)?;
        self.plan
            .verify_result(&result)
            .map_err(RiscvWorkloadReplayError::Workload)?;

        Ok(RiscvWorkloadReplayOutcome::new(
            run,
            result,
            host_action_outcomes,
        ))
    }

    fn schedule_planned_host_events(
        &self,
        scheduler: &mut PartitionedScheduler,
        controller: &Arc<Mutex<SystemHostController>>,
        host_partition: PartitionId,
        host_source: GuestSourceId,
    ) -> Result<(), RiscvWorkloadReplayError> {
        for (index, event) in self.plan.host_events().iter().enumerate() {
            let Some(guest_event) = planned_host_guest_event(event, index, host_source) else {
                continue;
            };
            let controller = Arc::clone(controller);
            scheduler
                .schedule_parallel_at(host_partition, event.tick(), move |context| {
                    let delivery = GuestEventDelivery::new(
                        context.now(),
                        host_partition,
                        host_partition,
                        guest_event,
                    );
                    controller
                        .lock()
                        .expect("system host controller lock")
                        .handle_delivery(delivery);
                })
                .map_err(RiscvWorkloadReplayError::Scheduler)?;
        }

        Ok(())
    }

    fn build_transport(&self) -> Result<MemoryTransport, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let mut transport = MemoryTransport::new();
        for route in topology.memory_routes() {
            let route = MemoryRoute::new(
                TransportEndpointId::new(route.source_endpoint())
                    .map_err(RiscvWorkloadReplayError::Transport)?,
                PartitionId::new(route.source_partition()),
                TransportEndpointId::new(route.target_endpoint())
                    .map_err(RiscvWorkloadReplayError::Transport)?,
                PartitionId::new(route.target_partition()),
                route.request_latency(),
                route.response_latency(),
            )
            .map_err(RiscvWorkloadReplayError::Transport)?;
            transport
                .add_route(route)
                .map_err(RiscvWorkloadReplayError::Transport)?;
        }
        Ok(transport)
    }

    fn build_route_map(
        &self,
    ) -> Result<BTreeMap<WorkloadRouteId, MemoryRouteId>, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let mut route_map = BTreeMap::new();
        for (next_route, route) in topology.memory_routes().iter().enumerate() {
            route_map.insert(route.id().clone(), MemoryRouteId::new(next_route as u64));
        }
        Ok(route_map)
    }

    fn build_cluster(
        &self,
        route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
    ) -> Result<RiscvCluster, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let layout = self.instruction_layout()?;
        let fetch_size = AccessSize::new(4).map_err(RiscvWorkloadReplayError::Memory)?;
        let cores = topology
            .riscv_cores()
            .iter()
            .map(|core| {
                let route = route_map.get(core.fetch_route()).copied().ok_or_else(|| {
                    RiscvWorkloadReplayError::MissingRoute {
                        route: core.fetch_route().clone(),
                    }
                })?;
                let cpu_core = CpuCore::new(
                    CpuResetState::new(
                        CpuId::new(core.cpu()),
                        PartitionId::new(core.partition()),
                        AgentId::new(core.agent()),
                        core.entry(),
                    ),
                    CpuFetchConfig::new(
                        TransportEndpointId::new(core.fetch_endpoint())
                            .map_err(RiscvWorkloadReplayError::Transport)?,
                        route,
                        layout,
                        fetch_size,
                    ),
                )
                .map_err(RiscvWorkloadReplayError::Cpu)?;
                Ok(RiscvCore::new(cpu_core))
            })
            .collect::<Result<Vec<_>, RiscvWorkloadReplayError>>()?;

        RiscvCluster::new(cores).map_err(RiscvWorkloadReplayError::RiscvCluster)
    }

    fn instruction_layout(&self) -> Result<CacheLineLayout, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let target = topology
            .memory_targets()
            .first()
            .ok_or(RiscvWorkloadReplayError::MissingMemoryTarget)?;
        CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)
    }

    fn load_memory(&self) -> Result<PartitionedMemoryStore, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let mut store = PartitionedMemoryStore::new();
        for target in topology.memory_targets() {
            let target_id = MemoryTargetId::new(target.target());
            let layout = CacheLineLayout::new(target.line_bytes())
                .map_err(RiscvWorkloadReplayError::Memory)?;
            store
                .add_partition(target_id, layout)
                .map_err(RiscvWorkloadReplayError::Memory)?;
            store
                .map_region(target_id, target.range().start(), target.range().size())
                .map_err(RiscvWorkloadReplayError::Memory)?;
        }
        self.plan
            .to_boot_image()
            .map_err(RiscvWorkloadReplayError::Workload)?
            .load_into_partitioned_store_by_address(&mut store)
            .map_err(RiscvWorkloadReplayError::Boot)?;
        Ok(store)
    }

    fn result_from_run(
        &self,
        run: &RiscvSystemRun,
        controller: &Arc<Mutex<SystemHostController>>,
    ) -> Result<(WorkloadResult, Vec<SystemActionOutcome>), RiscvWorkloadReplayError> {
        let final_tick = run
            .final_tick()
            .ok_or(RiscvWorkloadReplayError::MissingFinalTick)?;
        let mut result = WorkloadResult::new(self.plan.manifest_identity(), final_tick);
        result = match run.stop_reason() {
            RiscvSystemRunStopReason::HostStop(_) => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_HOST)
            }
            RiscvSystemRunStopReason::Idle { .. } => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_IDLE)
            }
        };

        let controller = controller.lock().expect("system host controller lock");
        let host_action_outcomes = controller.run().action_outcomes().to_vec();
        for outcome in &host_action_outcomes {
            if let SystemActionOutcome::Checkpoint { manifest, .. } = outcome {
                result = result.with_checkpoint_label(manifest.label());
            }
        }
        Ok((
            result.with_stats_snapshot(controller.executor().stats().snapshot(final_tick)),
            host_action_outcomes,
        ))
    }
}

fn planned_host_guest_event(
    event: &WorkloadHostEvent,
    index: usize,
    source: GuestSourceId,
) -> Option<GuestEvent> {
    let kind = match event.intent() {
        HostEventIntent::RoiBegin { .. } => GuestEventKind::RoiBegin,
        HostEventIntent::RoiEnd { .. } => GuestEventKind::RoiEnd,
        HostEventIntent::StatsReset { .. } => GuestEventKind::StatsReset,
        HostEventIntent::StatsDump { .. } => GuestEventKind::StatsDump,
        HostEventIntent::Checkpoint { label } => GuestEventKind::Checkpoint {
            label: label.clone(),
        },
        HostEventIntent::Stop { .. } => return None,
    };
    Some(GuestEvent::new(
        GuestEventId::new(PLANNED_HOST_EVENT_ID_BASE + index as u64),
        source,
        kind,
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadReplayOutcome {
    run: RiscvSystemRun,
    result: WorkloadResult,
    host_action_outcomes: Vec<SystemActionOutcome>,
}

impl RiscvWorkloadReplayOutcome {
    pub const fn new(
        run: RiscvSystemRun,
        result: WorkloadResult,
        host_action_outcomes: Vec<SystemActionOutcome>,
    ) -> Self {
        Self {
            run,
            result,
            host_action_outcomes,
        }
    }

    pub const fn run(&self) -> &RiscvSystemRun {
        &self.run
    }

    pub const fn result(&self) -> &WorkloadResult {
        &self.result
    }

    pub fn host_action_outcomes(&self) -> &[SystemActionOutcome] {
        &self.host_action_outcomes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvWorkloadReplayError {
    MissingTopology,
    MissingMemoryTarget,
    MissingRoute { route: WorkloadRouteId },
    MissingFinalTick,
    Workload(WorkloadError),
    Boot(BootError),
    Memory(MemoryError),
    Cpu(CpuError),
    RiscvCluster(rem6_cpu::RiscvClusterError),
    Scheduler(SchedulerError),
    Transport(TransportError),
    System(crate::SystemError),
}

impl fmt::Display for RiscvWorkloadReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTopology => write!(formatter, "workload replay plan is missing topology"),
            Self::MissingMemoryTarget => {
                write!(formatter, "workload replay topology has no memory target")
            }
            Self::MissingRoute { route } => {
                write!(
                    formatter,
                    "workload replay route {} is not declared",
                    route.as_str()
                )
            }
            Self::MissingFinalTick => write!(formatter, "RISC-V run did not report a final tick"),
            Self::Workload(error) => write!(formatter, "{error}"),
            Self::Boot(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Cpu(error) => write!(formatter, "{error}"),
            Self::RiscvCluster(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::System(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvWorkloadReplayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Workload(error) => Some(error),
            Self::Boot(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Cpu(error) => Some(error),
            Self::RiscvCluster(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::System(error) => Some(error),
            Self::MissingTopology
            | Self::MissingMemoryTarget
            | Self::MissingRoute { .. }
            | Self::MissingFinalTick => None,
        }
    }
}

fn memory_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    let response = store
        .lock()
        .expect("workload memory store lock")
        .respond(delivery.request())
        .expect("workload memory response")
        .response()
        .cloned()
        .expect("workload memory response payload");
    TargetOutcome::Respond(response)
}
