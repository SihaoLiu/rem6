use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_accelerator::{AcceleratorEngineId, AcceleratorEngineSnapshot};
use rem6_checkpoint::{CheckpointComponentId, CheckpointManifest};
use rem6_coherence::{PartitionedDramMemoryConfig, PartitionedDramQosState};
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    RiscvCluster, RiscvCore,
};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemorySnapshot, DramTiming,
};
use rem6_fabric::{
    FabricLinkId, FabricModel, FabricPath, FabricPathHop, FabricRouterId, FabricRouterStage,
    VirtualNetworkId,
};
use rem6_gpu::{GpuDeviceId, GpuDeviceSnapshot, GpuDmaCopy, GpuDmaId};
use rem6_kernel::{
    ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionFrontier, PartitionId,
    PartitionedScheduler, Tick, WaitForEdgeKind, WaitForGraph,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    PartitionedMemorySnapshot, PartitionedMemoryStore, TranslationPageMap,
    TranslationPagePermissions, TranslationPageSize,
};
use rem6_net::SinicFifoDeviceSnapshot;
use rem6_stats::StatsRegistry;
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTransport, TargetBatchOutcome,
    TransportEndpointId, TransportError,
};
use rem6_workload::{
    WorkloadCheckpointChunkSummary, WorkloadCheckpointComponentSummary,
    WorkloadCheckpointManifestSummary, WorkloadError, WorkloadGpuDmaCopy,
    WorkloadHostActionSummary, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerCount, WorkloadParallelBatchWorkerLaneRecord, WorkloadReplayPlan,
    WorkloadResolvedResources, WorkloadResult, WorkloadRouteFabric, WorkloadRouteHop,
    WorkloadRouteId, WorkloadSinicPciDeviceSummary, WorkloadTopology,
    WorkloadTrafficTraceReplaySummary, WorkloadWaitForBlockedNodeWindow,
    WorkloadWaitForEdgeKindWindow, WorkloadWaitForTargetNodeWindow,
};

mod cache_response;
mod data_cache_backend;
mod dma_scheduler_evidence;
mod error;
mod guest_memory;
mod linux_handoff;
mod manifest_traffic_trace;
mod memory_backend;
mod payload;
mod planned_data_cache_sync;
mod qos;
mod sinic_mmio_backend;
mod stats;
mod summary;
mod traffic_trace;
mod traffic_trace_fetch;
mod traffic_trace_htm;
mod traffic_trace_records;
mod traffic_trace_sideband_records;
mod traffic_trace_sync;
mod workload_replay_dma;

use self::cache_response::{
    cached_memory_response, cached_target_batch_response, data_cache_agents,
};
use self::data_cache_backend::{
    WorkloadDataCacheBackend, WorkloadDataCacheLineBackend, WorkloadDataCacheLineMemory,
};
use self::dma_scheduler_evidence::{
    dma_scheduler_batch_timeline, dma_scheduler_batch_worker_count_ticks,
    dma_scheduler_batch_worker_counts, dma_scheduler_frontiers,
    dma_scheduler_planned_batch_worker_lanes,
    dma_scheduler_recorded_batch_worker_slot_tick_summaries, dma_scheduler_remote_flows,
    dma_scheduler_remote_sends, DmaSchedulerEvidence,
};
pub use self::error::RiscvWorkloadReplayError;
use self::guest_memory::{
    workload_functional_guest_memory_reader, workload_functional_guest_memory_writer,
};
use self::manifest_traffic_trace::build_traffic_trace_replays;
use self::memory_backend::{memory_response, WorkloadDramBackend, WorkloadMemoryBackend};
use self::planned_data_cache_sync::{
    planned_data_cache_trace_overlap, planned_host_data_cache_checkpoint_action,
    planned_host_data_cache_sync_handler, take_planned_host_data_cache_sync_error,
};
use self::qos::{dram_scheduling_policy, priority_policy, queue_arbiter};
use self::sinic_mmio_backend::WorkloadSinicPciMmioBackend;
use self::stats::{workload_data_access_stats, workload_instruction_stats};
use self::summary::{
    livelock_transition_threshold, parallel_execution_summary, WorkloadReplayActivityRefs,
};
use self::traffic_trace::{
    schedule_traffic_trace_replays_with_fetch_bindings, trace_route_uses_data_cache,
    RiscvWorkloadScheduledTrafficTraceReplay,
};
pub use self::traffic_trace::{
    RiscvWorkloadTraceMemoryFailureRecord, RiscvWorkloadTraceMemoryResponseRecord,
    RiscvWorkloadTrafficTraceReplay, RiscvWorkloadTrafficTraceReplayOutcome,
};
use self::traffic_trace_fetch::traffic_trace_fetch_responder as trace_fetch_responder;
pub use self::traffic_trace_htm::{
    RiscvWorkloadTraceHtmAbortRecord, RiscvWorkloadTraceHtmBeginRecord,
};
pub use self::traffic_trace_sideband_records::{
    RiscvWorkloadTraceCacheFlushRecord, RiscvWorkloadTraceSidebandFailureRecord,
    RiscvWorkloadTraceTlbSyncRecord,
};
pub use self::traffic_trace_sync::{
    RiscvWorkloadTraceL1InvalidationRecord, RiscvWorkloadTraceSyncOutcome,
    RiscvWorkloadTraceSyncRecord,
};
use self::workload_replay_dma::run_accelerator_dma_copies;
use crate::workload_replay_heterogeneous::{
    accelerator_command_kind_counts, accelerator_snapshots, accelerator_wait_for_graph_since,
    accelerator_wait_for_markers, build_accelerator_devices, build_gpu_devices, gpu_snapshots,
    gpu_wait_for_graph_since, gpu_wait_for_markers, merge_wait_for_graph,
    schedule_accelerator_commands, schedule_gpu_kernel_launches, wait_for_blocked_node_windows,
    wait_for_edge_kind_windows, wait_for_target_node_windows, WorkloadAcceleratorActivity,
    WorkloadGpuActivity, WorkloadGpuRuntime,
};
use crate::workload_replay_host::schedule_planned_host_events_with_handler;
use crate::{
    configure_riscv_unrestricted_pmp, DramMemoryCheckpointBank, DramMemoryCheckpointPort,
    ExecutionMode, GuestEventId, GuestSourceId, HostEventPolicy, MemoryStoreCheckpointBank,
    MemoryStoreCheckpointPort, RiscvSystemRun, RiscvSystemRunDriver, RiscvSystemRunStopReason,
    RiscvTrapEventPort, SystemActionOutcome, SystemHostController, SystemHostEventPort,
    TrafficTraceReplayControllerParallelErrors,
};

const DEFAULT_MAX_TURNS: usize = 64;
const WORKLOAD_STOP_REASON_HOST: &str = "host-stop";
const WORKLOAD_STOP_REASON_DEBUG: &str = "debug-stop";
const WORKLOAD_STOP_REASON_IDLE: &str = "idle";
const WORKLOAD_STOP_REASON_TICK_LIMIT: &str = "tick-limit";
const WORKLOAD_STOP_REASON_INSTRUCTION_LIMIT: &str = "instruction-limit";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WorkloadGpuDmaActivity {
    copy_count: usize,
    completion_count: usize,
    active_device_count: usize,
    scheduler_epoch_count: usize,
    scheduler_empty_epoch_count: usize,
    scheduler_dispatch_count: usize,
    scheduler_batch_count: usize,
    scheduler_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    scheduler_batch_worker_count_ticks: Vec<(usize, Tick)>,
    scheduler_planned_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    scheduler_planned_batch_worker_capacity_ticks: Tick,
    scheduler_planned_batch_worker_lanes: Vec<WorkloadParallelBatchWorkerLaneRecord>,
    scheduler_recorded_batch_worker_capacity_ticks: Tick,
    scheduler_recorded_batch_worker_slot_tick_summaries: Vec<(usize, Tick, Tick)>,
    scheduler_initial_frontiers: Vec<PartitionFrontier>,
    scheduler_final_frontiers: Vec<PartitionFrontier>,
    scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    scheduler_remote_sends: Vec<ParallelRemoteSendRecord>,
    wait_for_edge_count: usize,
    wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
    deadlock_diagnostic_count: usize,
}

impl WorkloadGpuDmaActivity {
    fn with_wait_for_graph(mut self, wait_for: WaitForGraph) -> Self {
        self.wait_for_edge_count = wait_for.edge_count();
        self.wait_for_edge_kind_counts = wait_for.snapshot().edge_kind_counts();
        self.wait_for_edge_kind_windows = wait_for_edge_kind_windows(&wait_for);
        self.wait_for_blocked_node_windows = wait_for_blocked_node_windows(&wait_for);
        self.wait_for_target_node_windows = wait_for_target_node_windows(&wait_for);
        self.deadlock_diagnostic_count = wait_for.deadlock_diagnostic().into_iter().count();
        self
    }
}

#[derive(Clone, Debug)]
pub struct RiscvWorkloadReplay {
    plan: WorkloadReplayPlan,
    resolved_resources: Option<WorkloadResolvedResources>,
    traffic_trace_replays: Vec<RiscvWorkloadTrafficTraceReplay>,
    max_turns: usize,
}

impl RiscvWorkloadReplay {
    pub const fn new(plan: WorkloadReplayPlan) -> Self {
        Self {
            plan,
            resolved_resources: None,
            traffic_trace_replays: Vec::new(),
            max_turns: DEFAULT_MAX_TURNS,
        }
    }

    pub fn with_resolved_resources(mut self, resources: WorkloadResolvedResources) -> Self {
        self.resolved_resources = Some(resources);
        self
    }

    pub const fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    pub fn with_traffic_trace_replay(mut self, replay: RiscvWorkloadTrafficTraceReplay) -> Self {
        self.traffic_trace_replays.push(replay);
        self
    }

    pub fn run_parallel(&self) -> Result<RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
        self.validate_resolved_resources()?;
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let route_map = self.build_route_map()?;
        let memory = self.load_memory_backend()?;
        let data_cache = self.build_data_cache(&memory)?;
        let sinic_mmio = WorkloadSinicPciMmioBackend::from_topology(topology)?;
        let gpu_devices = build_gpu_devices(topology)?;
        let mut transport = self.build_transport()?;
        if topology.qos_policy().is_some() {
            let batch_memory = memory.clone();
            let batch_data_cache = data_cache.clone();
            let batch_sinic_mmio = sinic_mmio.clone();
            transport = transport.with_direct_target_batch_responder(move |deliveries, context| {
                if deliveries
                    .iter()
                    .any(|delivery| batch_sinic_mmio.accepts_delivery(delivery))
                {
                    return Some(
                        deliveries
                            .into_iter()
                            .map(|delivery| {
                                let request = delivery.request().id();
                                let outcome = batch_sinic_mmio
                                    .respond_parallel(&delivery, context)
                                    .unwrap_or_else(|| {
                                        cached_memory_response(
                                            batch_data_cache.as_ref(),
                                            &batch_memory,
                                            &delivery,
                                        )
                                    });
                                TargetBatchOutcome::new(request, outcome)
                            })
                            .collect(),
                    );
                }
                cached_target_batch_response(batch_data_cache.as_ref(), &batch_memory, deliveries)
            });
        }
        let fabric_activity_start = transport.mark_fabric_activity();
        let fabric_wait_for_start = transport.mark_fabric_wait_for();
        let dram_wait_for_start = memory.mark_dram_wait_for();
        let gpu_dma_activity = self.run_gpu_dma_copies(
            topology,
            &route_map,
            &gpu_devices,
            &transport,
            &memory,
            &data_cache,
        )?;
        let gpu_before = gpu_snapshots(&gpu_devices);
        let accelerator_devices = build_accelerator_devices(topology)?;
        let accelerator_dma_activity = run_accelerator_dma_copies(
            topology,
            &route_map,
            &accelerator_devices,
            &transport,
            &memory,
            &data_cache,
        )?;
        let accelerator_before = accelerator_snapshots(&accelerator_devices);
        let gpu_wait_for_start = gpu_wait_for_markers(&gpu_devices);
        let accelerator_wait_for_start = accelerator_wait_for_markers(&accelerator_devices);
        let cluster = self.build_cluster(&route_map)?;
        let data_translation_page_map = self.build_data_translation_page_map()?;
        let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(
            topology.partition_count(),
            topology.min_remote_delay(),
            topology.parallel_worker_limit(),
        )
        .map_err(RiscvWorkloadReplayError::Scheduler)?;
        let mut stats = StatsRegistry::new();
        let instruction_stats = workload_instruction_stats(topology, &mut stats)?;
        let data_access_stats = workload_data_access_stats(topology)?;
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            stats,
        )));
        attach_workload_memory_checkpoint_bank(&controller, &memory)?;
        sinic_mmio
            .attach_checkpoint_ports(
                controller
                    .lock()
                    .expect("system host controller lock")
                    .executor_mut(),
            )
            .map_err(|error| {
                RiscvWorkloadReplayError::System(crate::SystemError::Checkpoint(error))
            })?;
        let traffic_trace_replay_runs = build_traffic_trace_replays(self)?;
        let traffic_trace_data_cache_routes = self
            .plan
            .traffic_trace_replays()
            .iter()
            .filter(|replay| replay.data_cache())
            .map(|replay| replay.route().clone())
            .collect::<BTreeSet<_>>();
        self.validate_planned_host_data_cache_trace_ticks(
            topology,
            &traffic_trace_replay_runs,
            &traffic_trace_data_cache_routes,
        )?;
        let planned_host_data_cache_sync_errors = Arc::new(Mutex::new(Vec::new()));
        let scheduler_checkpoint_component = CheckpointComponentId::new("scheduler0")
            .expect("static scheduler checkpoint component is nonempty");
        schedule_planned_host_events_with_handler(
            &self.plan,
            &mut scheduler,
            &controller,
            PartitionId::new(topology.host().partition()),
            GuestSourceId::new(topology.host().source()),
            scheduler_checkpoint_component.clone(),
            planned_host_data_cache_sync_handler(
                memory.clone(),
                data_cache.clone(),
                Arc::clone(&planned_host_data_cache_sync_errors),
            ),
        )?;
        let scheduled_traffic_trace_replays = schedule_traffic_trace_replays_with_fetch_bindings(
            &traffic_trace_replay_runs,
            topology,
            &route_map,
            &mut scheduler,
            &transport,
            &data_cache,
            &cluster,
            &traffic_trace_data_cache_routes,
        )?;
        let fetch_bindings = scheduled_traffic_trace_replays.fetch_bindings.clone();
        let traffic_trace_replays = scheduled_traffic_trace_replays.replays;
        let trap_port = RiscvTrapEventPort::new(
            SystemHostEventPort::with_controller(
                PartitionId::new(topology.host().partition()),
                topology.host().latency(),
                Arc::clone(&controller),
            )
            .map_err(RiscvWorkloadReplayError::System)?,
            GuestSourceId::new(topology.host().source()),
        )
        .with_scheduler_checkpoint_component(scheduler_checkpoint_component);
        let mut driver = RiscvSystemRunDriver::with_instruction_stats(trap_port, instruction_stats);
        if let Some(handoff) = self.plan.linux_boot_handoff() {
            driver = driver
                .with_riscv_sbi_firmware_and_functional_guest_memory_reader(
                    workload_functional_guest_memory_reader(
                        memory.clone(),
                        data_cache.clone(),
                        topology,
                    )?,
                )
                .with_riscv_sbi_firmware_and_functional_guest_memory_writer_object(
                    workload_functional_guest_memory_writer(
                        memory.clone(),
                        data_cache.clone(),
                        topology,
                    )?,
                );
            if let Some(input) = self.debug_console_input_payload(handoff)? {
                driver = driver.with_riscv_sbi_debug_console_input(input);
            }
        }
        if let Some(data_access_stats) = data_access_stats {
            driver = driver.with_data_access_stats(data_access_stats);
        }
        let gpu_launch_count =
            schedule_gpu_kernel_launches(topology, &gpu_devices, &mut scheduler)?;
        let accelerator_command_count =
            schedule_accelerator_commands(topology, &accelerator_devices, &mut scheduler)?;
        let run_result = if let Some(page_map) = data_translation_page_map.as_ref() {
            driver.drive_until_host_stop_parallel_with_data_translation(
                &cluster,
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                page_map,
                |cpu| trace_fetch_responder(&cluster, &fetch_bindings, memory.clone(), cpu),
                |_cpu| {
                    let memory = memory.clone();
                    let data_cache = data_cache.clone();
                    let sinic_mmio = sinic_mmio.clone();
                    move |delivery, context| {
                        if let Some(outcome) = sinic_mmio.respond_parallel(&delivery, context) {
                            return outcome;
                        }
                        cached_memory_response(data_cache.as_ref(), &memory, &delivery)
                    }
                },
                self.max_turns,
                |cpu| GuestEventId::new(1_000 + u64::from(cpu.get())),
            )
        } else {
            driver.drive_until_host_stop_parallel(
                &cluster,
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |cpu| trace_fetch_responder(&cluster, &fetch_bindings, memory.clone(), cpu),
                |_cpu| {
                    let memory = memory.clone();
                    let data_cache = data_cache.clone();
                    let sinic_mmio = sinic_mmio.clone();
                    move |delivery, context| {
                        if let Some(outcome) = sinic_mmio.respond_parallel(&delivery, context) {
                            return outcome;
                        }
                        cached_memory_response(data_cache.as_ref(), &memory, &delivery)
                    }
                },
                self.max_turns,
                |cpu| GuestEventId::new(1_000 + u64::from(cpu.get())),
            )
        };
        if let Some(error) = take_data_cache_error(&data_cache) {
            return Err(error);
        }
        if let Some(error) =
            take_planned_host_data_cache_sync_error(&planned_host_data_cache_sync_errors)
        {
            return Err(error);
        }
        let mut run = run_result.map_err(RiscvWorkloadReplayError::System)?;
        if let Some((route, errors)) = traffic_trace_replay_callback_error(&traffic_trace_replays) {
            return Err(RiscvWorkloadReplayError::TrafficTraceReplayCallback { route, errors });
        }
        if let Some(data_cache) = data_cache.as_ref() {
            let final_tick = run
                .final_tick()
                .ok_or(RiscvWorkloadReplayError::MissingFinalTick)?;
            let (
                final_lines,
                records,
                trace_diagnostic_records,
                trace_error_records,
                trace_htm_access_records,
            ) = {
                let data_cache = data_cache.lock().expect("workload data cache lock");
                (
                    data_cache.final_lines(final_tick)?,
                    data_cache.records(),
                    data_cache.trace_diagnostic_records(),
                    data_cache.trace_error_records(),
                    data_cache.trace_htm_access_records(),
                )
            };
            for (target, line, line_data) in final_lines {
                memory.insert_line(target, line, line_data)?;
            }
            if !records.is_empty() {
                run = run.with_data_cache_run_records(records);
            }
            if !trace_diagnostic_records.is_empty() {
                run = run.with_trace_diagnostic_records(trace_diagnostic_records);
            }
            if !trace_error_records.is_empty() {
                run = run
                    .with_data_cache_error_records(trace_error_records.clone())
                    .with_trace_error_records(trace_error_records);
            }
            if !trace_htm_access_records.is_empty() {
                run = run.with_trace_htm_access_records(trace_htm_access_records);
            }
        }
        let dram_target_activities = memory.dram_target_activities();
        if !dram_target_activities.is_empty() {
            run = run.with_dram_activity(dram_target_activities);
        }
        if let Some(fabric_activity) =
            fabric_activity_start.and_then(|marker| transport.fabric_lane_activities_since(marker))
        {
            if !fabric_activity.is_empty() {
                run = run.with_fabric_activity(fabric_activity);
            }
        }
        if let Some(fabric_hop_activity) =
            fabric_activity_start.and_then(|marker| transport.fabric_hop_activities_since(marker))
        {
            if !fabric_hop_activity.is_empty() {
                run = run.with_fabric_hop_activity(fabric_hop_activity);
            }
        }
        if let Some(fabric_wait_for) =
            fabric_wait_for_start.and_then(|marker| transport.fabric_wait_for_graph_since(marker))
        {
            run = run.with_fabric_wait_for(fabric_wait_for);
        }
        run = run.with_dram_wait_for(memory.dram_wait_for_since(dram_wait_for_start));
        let gpu_snapshots = gpu_snapshots(&gpu_devices);
        let gpu_wait_for = gpu_wait_for_graph_since(&gpu_devices, &gpu_wait_for_start);
        let gpu_activity =
            WorkloadGpuActivity::from_snapshots(gpu_launch_count, &gpu_before, &gpu_snapshots)
                .with_wait_for_graph(gpu_wait_for);
        let accelerator_snapshots = accelerator_snapshots(&accelerator_devices);
        let accelerator_wait_for =
            accelerator_wait_for_graph_since(&accelerator_devices, &accelerator_wait_for_start);
        let accelerator_activity = WorkloadAcceleratorActivity::from_snapshots(
            accelerator_command_count,
            accelerator_command_kind_counts(topology.accelerator_commands()),
            &accelerator_before,
            &accelerator_snapshots,
        )
        .with_wait_for_graph(accelerator_wait_for);
        let traffic_trace_replay_summaries = traffic_trace_replays
            .iter()
            .map(RiscvWorkloadScheduledTrafficTraceReplay::summary)
            .collect::<Vec<_>>();
        let (result, host_action_outcomes) = self.result_from_run(
            &run,
            &controller,
            topology,
            WorkloadReplayActivityRefs {
                gpu: &gpu_activity,
                gpu_dma: &gpu_dma_activity,
                accelerator: &accelerator_activity,
                accelerator_dma: &accelerator_dma_activity,
            },
            &traffic_trace_replay_summaries,
        )?;
        self.plan
            .verify_result(&result)
            .map_err(RiscvWorkloadReplayError::Workload)?;
        let traffic_trace_replays = traffic_trace_replays
            .into_iter()
            .map(RiscvWorkloadScheduledTrafficTraceReplay::into_outcome)
            .collect();
        let memory_snapshot = memory.memory_snapshot();
        let dram_snapshot = memory.dram_snapshot();
        let sinic_pci_snapshots = sinic_mmio.snapshots();

        Ok(RiscvWorkloadReplayOutcome::new(
            cluster,
            run,
            result,
            host_action_outcomes,
            memory_snapshot,
            dram_snapshot,
            RiscvWorkloadReplayDeviceSnapshots::new(
                gpu_snapshots,
                accelerator_snapshots,
                sinic_pci_snapshots,
            ),
            traffic_trace_replays,
        ))
    }

    fn validate_resolved_resources(&self) -> Result<(), RiscvWorkloadReplayError> {
        let Some(resources) = &self.resolved_resources else {
            return Ok(());
        };
        let expected = self.plan.manifest_identity();
        let actual = resources.manifest_identity();
        if actual == expected {
            return Ok(());
        }

        Err(RiscvWorkloadReplayError::Workload(
            WorkloadError::ManifestIdentityMismatch { expected, actual },
        ))
    }

    fn build_transport(&self) -> Result<MemoryTransport, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let has_fabric_routes = topology
            .memory_routes()
            .iter()
            .any(|route| route.hops().iter().any(|hop| hop.fabric().is_some()));
        let mut transport = if has_fabric_routes {
            if let Some(policy) = topology.qos_policy() {
                MemoryTransport::with_fabric_qos_priority_policy(
                    FabricModel::new(),
                    queue_arbiter(policy),
                    priority_policy(policy),
                )
            } else {
                MemoryTransport::with_fabric(FabricModel::new())
            }
        } else if let Some(policy) = topology.qos_policy() {
            MemoryTransport::with_qos_priority_policy(
                queue_arbiter(policy),
                priority_policy(policy),
            )
        } else {
            MemoryTransport::new()
        };
        for route in topology.memory_routes() {
            let route = workload_memory_route(route)?;
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
        let fetch_size = AccessSize::new(4).map_err(RiscvWorkloadReplayError::Memory)?;
        let cores = topology
            .riscv_cores()
            .iter()
            .map(|core| {
                let layout = self.instruction_layout(topology, core.entry(), fetch_size)?;
                let route = route_map.get(core.fetch_route()).copied().ok_or_else(|| {
                    RiscvWorkloadReplayError::MissingRoute {
                        route: core.fetch_route().clone(),
                    }
                })?;
                let fetch_endpoint = TransportEndpointId::new(core.fetch_endpoint())
                    .map_err(RiscvWorkloadReplayError::Transport)?;
                let fetch_config = topology.memory_targets().iter().try_fold(
                    CpuFetchConfig::new(fetch_endpoint, route, layout, fetch_size),
                    |config, target| {
                        let target_layout = CacheLineLayout::new(target.line_bytes())
                            .map_err(RiscvWorkloadReplayError::Memory)?;
                        Ok(config.with_line_layout_range(target.range(), target_layout))
                    },
                )?;
                let cpu_core = CpuCore::new(
                    CpuResetState::new(
                        CpuId::new(core.cpu()),
                        PartitionId::new(core.partition()),
                        AgentId::new(core.agent()),
                        core.entry(),
                    ),
                    fetch_config,
                )
                .map_err(RiscvWorkloadReplayError::Cpu)?;
                let Some(data_route) = core.data_route() else {
                    return Ok(RiscvCore::new(cpu_core));
                };
                let Some(data_endpoint) = core.data_endpoint() else {
                    return Ok(RiscvCore::new(cpu_core));
                };
                let data_route = route_map.get(data_route).copied().ok_or_else(|| {
                    RiscvWorkloadReplayError::MissingRoute {
                        route: data_route.clone(),
                    }
                })?;
                let data_endpoint = TransportEndpointId::new(data_endpoint)
                    .map_err(RiscvWorkloadReplayError::Transport)?;
                let data_config = topology.memory_targets().iter().try_fold(
                    CpuDataConfig::new(data_endpoint, data_route, layout),
                    |config, target| {
                        let target_layout = CacheLineLayout::new(target.line_bytes())
                            .map_err(RiscvWorkloadReplayError::Memory)?;
                        Ok(config.with_line_layout_range(target.range(), target_layout))
                    },
                )?;
                let Some(translation) = core.data_translation() else {
                    return Ok(RiscvCore::with_data(cpu_core, data_config));
                };
                let frontend = match translation.tlb() {
                    Some(tlb) => CpuTranslationFrontend::with_tlb(translation.queue(), tlb),
                    None => CpuTranslationFrontend::new(translation.queue()),
                };
                Ok(RiscvCore::with_data_translation(
                    cpu_core,
                    data_config,
                    frontend,
                ))
            })
            .collect::<Result<Vec<_>, RiscvWorkloadReplayError>>()?;

        if let Some(handoff) = self.plan.linux_boot_handoff() {
            apply_linux_boot_handoff(&cores, handoff.dtb_addr());
        }

        RiscvCluster::new(cores).map_err(RiscvWorkloadReplayError::RiscvCluster)
    }

    fn build_data_translation_page_map(
        &self,
    ) -> Result<Option<TranslationPageMap>, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let mut translations = topology.riscv_cores().iter().filter_map(|core| {
            core.data_translation()
                .map(|translation| (core.cpu(), translation))
        });
        let Some((_cpu, first)) = translations.next() else {
            return Ok(None);
        };

        let page_size_bytes = first.page_size_bytes();
        let mut page_map = TranslationPageMap::new(
            TranslationPageSize::new(page_size_bytes)
                .map_err(RiscvWorkloadReplayError::Translation)?,
        );
        for mapping in first.page_mappings() {
            page_map
                .map(
                    mapping.virtual_base(),
                    mapping.physical_base(),
                    mapping.pages(),
                    TranslationPagePermissions::read_write_execute(),
                )
                .map_err(RiscvWorkloadReplayError::Translation)?;
        }

        for (cpu, translation) in translations {
            if translation.page_size_bytes() != page_size_bytes {
                return Err(RiscvWorkloadReplayError::DataTranslationPageSizeMismatch {
                    cpu,
                    expected: page_size_bytes,
                    actual: translation.page_size_bytes(),
                });
            }
            for mapping in translation.page_mappings() {
                page_map
                    .map(
                        mapping.virtual_base(),
                        mapping.physical_base(),
                        mapping.pages(),
                        TranslationPagePermissions::read_write_execute(),
                    )
                    .map_err(RiscvWorkloadReplayError::Translation)?;
            }
        }

        Ok(Some(page_map))
    }

    fn run_gpu_dma_copies(
        &self,
        topology: &WorkloadTopology,
        route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
        devices: &BTreeMap<GpuDeviceId, WorkloadGpuRuntime>,
        transport: &MemoryTransport,
        memory: &WorkloadMemoryBackend,
        data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    ) -> Result<WorkloadGpuDmaActivity, RiscvWorkloadReplayError> {
        if topology.gpu_dma_copies().is_empty() {
            return Ok(WorkloadGpuDmaActivity::default());
        }

        let fabric_wait_for_start = transport.mark_fabric_wait_for();
        let dram_wait_for_start = memory.mark_dram_wait_for();
        let before = gpu_snapshots(devices);
        let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(
            topology.partition_count(),
            topology.min_remote_delay(),
            topology.parallel_worker_limit(),
        )
        .map_err(RiscvWorkloadReplayError::Scheduler)?;
        let mut scheduler_evidence = DmaSchedulerEvidence::default();
        for copy in topology.gpu_dma_copies() {
            let device = GpuDeviceId::new(copy.device());
            let runtime = devices.get(&device).ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingGpuDevice {
                    device: copy.device(),
                })
            })?;
            let dma = self.gpu_dma_copy(topology, route_map, copy)?;
            let read_memory = memory.clone();
            let read_data_cache = data_cache.clone();
            runtime
                .gpu
                .submit_dma_copy_read(
                    &mut scheduler,
                    transport,
                    dma,
                    MemoryTrace::new(),
                    move |delivery, _context| {
                        cached_memory_response(read_data_cache.as_ref(), &read_memory, &delivery)
                    },
                )
                .map_err(RiscvWorkloadReplayError::Gpu)?;
        }
        let read_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvWorkloadReplayError::Scheduler)?;
        scheduler_evidence.merge_run(WorkloadParallelBatchScope::GpuDmaScheduler, &read_run);
        if let Some(error) = take_data_cache_error(data_cache) {
            return Err(error);
        }
        for copy in topology.gpu_dma_copies() {
            let device = GpuDeviceId::new(copy.device());
            let runtime = devices.get(&device).ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingGpuDevice {
                    device: copy.device(),
                })
            })?;
            let write_memory = memory.clone();
            let write_data_cache = data_cache.clone();
            if runtime
                .gpu
                .issue_next_dma_write(
                    &mut scheduler,
                    transport,
                    MemoryTrace::new(),
                    move |delivery, _context| {
                        cached_memory_response(write_data_cache.as_ref(), &write_memory, &delivery)
                    },
                )
                .map_err(RiscvWorkloadReplayError::Gpu)?
                .is_none()
            {
                if let Some(error) = take_data_cache_error(data_cache) {
                    return Err(error);
                }
                return Err(RiscvWorkloadReplayError::MissingGpuDmaWrite { device });
            }
        }
        let write_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvWorkloadReplayError::Scheduler)?;
        scheduler_evidence.merge_run(WorkloadParallelBatchScope::GpuDmaScheduler, &write_run);
        if let Some(error) = take_data_cache_error(data_cache) {
            return Err(error);
        }

        let after = gpu_snapshots(devices);
        let mut active_device_count = 0;
        let mut completion_count = 0;
        for (device, after) in &after {
            let Some(before) = before.get(device) else {
                continue;
            };
            let device_completions = after
                .dma_completions()
                .len()
                .saturating_sub(before.dma_completions().len());
            if device_completions != 0 {
                active_device_count += 1;
            }
            completion_count += device_completions;
        }

        let mut wait_for = memory.dram_wait_for_since(dram_wait_for_start);
        if let Some(fabric_wait_for) =
            fabric_wait_for_start.and_then(|marker| transport.fabric_wait_for_graph_since(marker))
        {
            merge_wait_for_graph(&mut wait_for, fabric_wait_for);
        }

        Ok(WorkloadGpuDmaActivity {
            copy_count: topology.gpu_dma_copies().len(),
            completion_count,
            active_device_count,
            scheduler_epoch_count: scheduler_evidence.epoch_count,
            scheduler_empty_epoch_count: scheduler_evidence.empty_epoch_count,
            scheduler_dispatch_count: scheduler_evidence.dispatch_count,
            scheduler_batch_count: scheduler_evidence.batch_count,
            scheduler_batch_timeline: dma_scheduler_batch_timeline(
                scheduler_evidence.batch_timeline,
            ),
            scheduler_batch_worker_counts: dma_scheduler_batch_worker_counts(
                scheduler_evidence.batch_worker_counts,
            ),
            scheduler_batch_worker_count_ticks: dma_scheduler_batch_worker_count_ticks(
                scheduler_evidence.batch_worker_count_ticks,
            ),
            scheduler_planned_batch_timeline: dma_scheduler_batch_timeline(
                scheduler_evidence.planned_batch_timeline,
            ),
            scheduler_planned_batch_worker_capacity_ticks: scheduler_evidence
                .planned_batch_worker_capacity_ticks,
            scheduler_planned_batch_worker_lanes: dma_scheduler_planned_batch_worker_lanes(
                scheduler_evidence.planned_batch_worker_lanes,
            ),
            scheduler_recorded_batch_worker_capacity_ticks: scheduler_evidence
                .recorded_batch_worker_capacity_ticks,
            scheduler_recorded_batch_worker_slot_tick_summaries:
                dma_scheduler_recorded_batch_worker_slot_tick_summaries(
                    scheduler_evidence.recorded_batch_worker_slot_tick_summaries,
                ),
            scheduler_initial_frontiers: dma_scheduler_frontiers(
                scheduler_evidence.initial_frontiers,
            ),
            scheduler_final_frontiers: dma_scheduler_frontiers(scheduler_evidence.final_frontiers),
            scheduler_remote_flows: dma_scheduler_remote_flows(scheduler_evidence.remote_flows),
            scheduler_remote_sends: dma_scheduler_remote_sends(scheduler_evidence.remote_sends),
            wait_for_edge_count: 0,
            wait_for_edge_kind_counts: BTreeMap::new(),
            wait_for_edge_kind_windows: Vec::new(),
            wait_for_blocked_node_windows: Vec::new(),
            wait_for_target_node_windows: Vec::new(),
            deadlock_diagnostic_count: 0,
        }
        .with_wait_for_graph(wait_for))
    }

    fn gpu_dma_copy(
        &self,
        topology: &WorkloadTopology,
        route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
        copy: &WorkloadGpuDmaCopy,
    ) -> Result<GpuDmaCopy, RiscvWorkloadReplayError> {
        let route = route_map.get(copy.route()).copied().ok_or_else(|| {
            RiscvWorkloadReplayError::MissingRoute {
                route: copy.route().clone(),
            }
        })?;
        let size = AccessSize::new(copy.bytes()).map_err(RiscvWorkloadReplayError::Memory)?;
        let layout = self.gpu_dma_layout(topology, copy, size)?;
        let read_sequence = copy.transfer().checked_mul(2).ok_or(
            RiscvWorkloadReplayError::GpuDmaRequestSequenceOverflow {
                transfer: copy.transfer(),
            },
        )?;
        let write_sequence = read_sequence.checked_add(1).ok_or(
            RiscvWorkloadReplayError::GpuDmaRequestSequenceOverflow {
                transfer: copy.transfer(),
            },
        )?;
        let read_request = MemoryRequest::read_shared(
            MemoryRequestId::new(AgentId::new(copy.agent()), read_sequence),
            copy.source(),
            size,
            layout,
        )
        .map_err(RiscvWorkloadReplayError::Memory)?;

        GpuDmaCopy::new(
            GpuDmaId::new(copy.transfer()),
            route,
            read_request,
            route,
            MemoryRequestId::new(AgentId::new(copy.agent()), write_sequence),
            copy.destination(),
        )
        .map_err(RiscvWorkloadReplayError::Gpu)
    }

    fn gpu_dma_layout(
        &self,
        topology: &WorkloadTopology,
        copy: &WorkloadGpuDmaCopy,
        size: AccessSize,
    ) -> Result<CacheLineLayout, RiscvWorkloadReplayError> {
        let source_range = rem6_memory::AddressRange::new(copy.source(), size)
            .map_err(RiscvWorkloadReplayError::Memory)?;
        let destination_range = rem6_memory::AddressRange::new(copy.destination(), size)
            .map_err(RiscvWorkloadReplayError::Memory)?;
        let target = topology
            .memory_targets()
            .iter()
            .find(|target| {
                target.range().contains_range(source_range)
                    && target.range().contains_range(destination_range)
            })
            .ok_or(RiscvWorkloadReplayError::MissingMemoryTarget)?;
        CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)
    }

    fn build_data_cache(
        &self,
        memory: &WorkloadMemoryBackend,
    ) -> Result<Option<Arc<Mutex<WorkloadDataCacheBackend>>>, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let Some(config) = topology.riscv_data_cache() else {
            return Ok(None);
        };
        let target = topology
            .memory_targets()
            .iter()
            .find(|target| target.target() == config.memory_target())
            .ok_or(RiscvWorkloadReplayError::MissingMemoryTarget)?;
        let layout =
            CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)?;
        let agents = data_cache_agents(topology, self.plan.traffic_trace_replays())?;
        if agents.is_empty() {
            return Err(RiscvWorkloadReplayError::MissingDataCacheAgent);
        }
        let target_id = MemoryTargetId::new(config.memory_target());
        let lines = config
            .line_addresses()
            .iter()
            .map(|line_address| {
                let line = layout.line_address(*line_address);
                let line_data = memory.line_data(target_id, line)?;
                let line_memory = self.data_cache_line_memory(topology, target, line, line_data)?;
                WorkloadDataCacheLineBackend::new(
                    config,
                    layout,
                    *line_address,
                    line_memory,
                    agents.clone(),
                )
            })
            .collect::<Result<Vec<_>, RiscvWorkloadReplayError>>()?;

        Ok(Some(Arc::new(Mutex::new(WorkloadDataCacheBackend::new(
            lines,
        )))))
    }

    fn validate_planned_host_data_cache_trace_ticks(
        &self,
        topology: &WorkloadTopology,
        replays: &[RiscvWorkloadTrafficTraceReplay],
        data_cache_routes: &BTreeSet<WorkloadRouteId>,
    ) -> Result<(), RiscvWorkloadReplayError> {
        let Some(config) = topology.riscv_data_cache() else {
            return Ok(());
        };
        let planned_ticks = self
            .plan
            .host_events()
            .iter()
            .filter_map(|event| {
                planned_host_data_cache_checkpoint_action(event)
                    .map(|action| (event.tick(), action))
            })
            .collect::<BTreeMap<_, _>>();
        let Some(max_planned_tick) = planned_ticks.keys().next_back().copied() else {
            return Ok(());
        };
        let target = topology
            .memory_targets()
            .iter()
            .find(|target| target.target() == config.memory_target())
            .ok_or(RiscvWorkloadReplayError::MissingMemoryTarget)?;
        let layout =
            CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)?;
        let lines = config
            .line_addresses()
            .iter()
            .map(|line| layout.line_address(*line))
            .collect::<BTreeSet<_>>();

        for replay in replays {
            if !trace_route_uses_data_cache(replay.route(), topology, data_cache_routes) {
                continue;
            }
            if let Some(overlap) = planned_data_cache_trace_overlap(
                replay,
                &planned_ticks,
                max_planned_tick,
                layout,
                &lines,
            )? {
                return Err(
                    RiscvWorkloadReplayError::PlannedDataCacheTraceReplayOverlap {
                        action: overlap.action,
                        tick: overlap.tick,
                        route: replay.route().clone(),
                        line: overlap.line,
                    },
                );
            }
        }

        Ok(())
    }

    fn data_cache_line_memory(
        &self,
        topology: &WorkloadTopology,
        target: &WorkloadMemoryTarget,
        line: Address,
        line_data: Vec<u8>,
    ) -> Result<WorkloadDataCacheLineMemory, RiscvWorkloadReplayError> {
        let Some(profile) = target.external_memory_profile().copied() else {
            return Ok(WorkloadDataCacheLineMemory::Line(line_data));
        };

        let memory_route = self.data_cache_memory_route(topology)?;
        let mut dram = DramMemoryController::new();
        let target_id = MemoryTargetId::new(target.target());
        dram.add_profile(profile)
            .map_err(RiscvWorkloadReplayError::Dram)?;
        dram.map_region(target_id, target.range().start(), target.range().size())
            .map_err(RiscvWorkloadReplayError::Dram)?;
        dram.insert_line(target_id, line, line_data)
            .map_err(RiscvWorkloadReplayError::Dram)?;

        let mut memory = PartitionedDramMemoryConfig::new(
            PartitionId::new(memory_route.target_partition()),
            TransportEndpointId::new(memory_route.target_endpoint())
                .map_err(RiscvWorkloadReplayError::Transport)?,
            memory_route.request_latency(),
            memory_route.response_latency(),
            dram,
        );
        if let Some(policy) = topology.qos_policy() {
            memory = memory.with_qos(PartitionedDramQosState::new(
                priority_policy(policy),
                queue_arbiter(policy),
                dram_scheduling_policy(policy),
            ));
        }

        Ok(WorkloadDataCacheLineMemory::Dram(Box::new(memory)))
    }

    fn data_cache_memory_route<'a>(
        &self,
        topology: &'a WorkloadTopology,
    ) -> Result<&'a WorkloadMemoryRoute, RiscvWorkloadReplayError> {
        let Some(config) = topology.riscv_data_cache() else {
            return Err(RiscvWorkloadReplayError::MissingDataCacheAgent);
        };
        let backing_route = config.backing_route();
        topology
            .memory_routes()
            .iter()
            .find(|route| route.id() == backing_route)
            .ok_or_else(|| RiscvWorkloadReplayError::MissingRoute {
                route: backing_route.clone(),
            })
    }

    fn instruction_layout(
        &self,
        topology: &WorkloadTopology,
        entry: Address,
        fetch_size: AccessSize,
    ) -> Result<CacheLineLayout, RiscvWorkloadReplayError> {
        let fetch_range = rem6_memory::AddressRange::new(entry, fetch_size)
            .map_err(RiscvWorkloadReplayError::Memory)?;
        let target = topology
            .memory_targets()
            .iter()
            .find(|target| target.range().contains_range(fetch_range))
            .ok_or(RiscvWorkloadReplayError::MissingMemoryTarget)?;
        CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)
    }

    fn load_memory_backend(&self) -> Result<WorkloadMemoryBackend, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let store = self.load_partitioned_memory_store()?;
        if self.uses_profiled_external_memory()? {
            let dram = self.load_dram_memory(store)?;
            return Ok(WorkloadMemoryBackend::Dram(Arc::new(Mutex::new(
                WorkloadDramBackend::new(dram, topology.qos_policy()),
            ))));
        }

        Ok(WorkloadMemoryBackend::Store(Arc::new(Mutex::new(store))))
    }

    fn uses_profiled_external_memory(&self) -> Result<bool, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        Ok(topology
            .memory_targets()
            .iter()
            .any(|target| target.external_memory_profile().is_some()))
    }

    fn load_dram_memory(
        &self,
        store: PartitionedMemoryStore,
    ) -> Result<DramMemoryController, RiscvWorkloadReplayError> {
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let mut dram = DramMemoryController::new();
        for target in topology.memory_targets() {
            let target_id = MemoryTargetId::new(target.target());
            if let Some(profile) = target.external_memory_profile().copied() {
                dram.add_profile(profile)
                    .map_err(RiscvWorkloadReplayError::Dram)?;
            } else {
                let layout = CacheLineLayout::new(target.line_bytes())
                    .map_err(RiscvWorkloadReplayError::Memory)?;
                let geometry = DramGeometry::new(1, target.line_bytes(), target.line_bytes())
                    .map_err(RiscvWorkloadReplayError::DramModel)?;
                let timing =
                    DramTiming::new(1, 1, 1, 1, 0).map_err(RiscvWorkloadReplayError::DramModel)?;
                dram.add_target(DramControllerConfig::new(
                    target_id, layout, geometry, timing,
                ))
                .map_err(RiscvWorkloadReplayError::Dram)?;
            }
            dram.map_region(target_id, target.range().start(), target.range().size())
                .map_err(RiscvWorkloadReplayError::Dram)?;
        }

        let snapshot = store.snapshot();
        for partition in snapshot.partitions() {
            for line in partition.lines() {
                dram.insert_line(partition.target(), line.line(), line.data().to_vec())
                    .map_err(RiscvWorkloadReplayError::Dram)?;
            }
        }
        Ok(dram)
    }

    fn load_partitioned_memory_store(
        &self,
    ) -> Result<PartitionedMemoryStore, RiscvWorkloadReplayError> {
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
        self.load_linux_device_tree_payload(&mut store)?;
        self.load_linux_initrd_payload(&mut store)?;
        Ok(store)
    }

    fn result_from_run(
        &self,
        run: &RiscvSystemRun,
        controller: &Arc<Mutex<SystemHostController>>,
        topology: &WorkloadTopology,
        activities: WorkloadReplayActivityRefs<'_>,
        traffic_trace_replay_summaries: &[WorkloadTrafficTraceReplaySummary],
    ) -> Result<(WorkloadResult, Vec<SystemActionOutcome>), RiscvWorkloadReplayError> {
        let final_tick = run
            .final_tick()
            .ok_or(RiscvWorkloadReplayError::MissingFinalTick)?;
        let mut result = WorkloadResult::new(self.plan.manifest_identity(), final_tick);
        result = match run.stop_reason() {
            RiscvSystemRunStopReason::HostStop(_) => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_HOST)
            }
            RiscvSystemRunStopReason::DebugStop { .. } => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_DEBUG)
            }
            RiscvSystemRunStopReason::Idle { .. } => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_IDLE)
            }
            RiscvSystemRunStopReason::TickLimit { .. } => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_TICK_LIMIT)
            }
            RiscvSystemRunStopReason::InstructionLimit { .. } => {
                result.with_stop_reason(WORKLOAD_STOP_REASON_INSTRUCTION_LIMIT)
            }
        };

        let controller = controller.lock().expect("system host controller lock");
        if !controller.action_errors().is_empty() {
            return Err(RiscvWorkloadReplayError::HostActionErrors {
                errors: controller.action_errors().to_vec(),
            });
        }
        let host_action_outcomes = controller.run().action_outcomes().to_vec();
        let mut host_action_summary = WorkloadHostActionSummary::default();
        for outcome in &host_action_outcomes {
            match outcome {
                SystemActionOutcome::InjectedCommand { .. } => {
                    host_action_summary.record_injected_command();
                }
                SystemActionOutcome::GuestHostCall { .. } => {
                    host_action_summary.record_guest_host_call();
                }
                SystemActionOutcome::RoiBegin { .. } => {
                    host_action_summary.record_roi_begin();
                }
                SystemActionOutcome::RoiEnd { .. } => {
                    host_action_summary.record_roi_end();
                }
                SystemActionOutcome::StatsReset(_) => {
                    host_action_summary.record_stats_reset();
                }
                SystemActionOutcome::StatsDump { .. } => {
                    host_action_summary.record_stats_dump();
                }
                SystemActionOutcome::Checkpoint { manifest, .. } => {
                    host_action_summary.record_checkpoint();
                    result = result
                        .with_checkpoint_manifest_summary(workload_checkpoint_summary(manifest));
                }
                SystemActionOutcome::CheckpointRestored { tick, manifest, .. } => {
                    host_action_summary.record_checkpoint_restore();
                    result = result.with_restored_checkpoint_manifest_summary(
                        workload_checkpoint_summary_at_tick(manifest, *tick),
                    );
                }
                SystemActionOutcome::ExecutionModeSwitched {
                    tick,
                    target,
                    mode,
                    stats_epoch,
                    stats_reset_tick,
                    state_transfer: _,
                    ..
                } => {
                    result = result.with_execution_mode_switch_stats_scope(
                        *tick,
                        target.as_str(),
                        workload_execution_mode_from_system(*mode),
                        *stats_epoch,
                        *stats_reset_tick,
                    );
                    host_action_summary.record_execution_mode_switch();
                }
                SystemActionOutcome::Stop(_) => {
                    host_action_summary.record_stop();
                }
            }
        }
        Ok((
            result
                .with_parallel_execution_summary(parallel_execution_summary(
                    run,
                    topology,
                    activities,
                    livelock_transition_threshold(self.plan.expected_clean_parallel_diagnostics()),
                ))
                .with_host_action_summary(host_action_summary)
                .with_stats_history_records(
                    controller.executor().stats().history_records().to_vec(),
                )
                .with_stats_snapshot(controller.executor().stats().snapshot(final_tick))
                .with_sinic_pci_device_summaries(
                    topology
                        .sinic_pci_devices()
                        .iter()
                        .map(WorkloadSinicPciDeviceSummary::from_topology_device),
                )
                .with_traffic_trace_replay_summaries(
                    traffic_trace_replay_summaries.iter().cloned(),
                ),
            host_action_outcomes,
        ))
    }
}

fn attach_workload_memory_checkpoint_bank(
    host_controller: &Arc<Mutex<SystemHostController>>,
    memory: &WorkloadMemoryBackend,
) -> Result<(), RiscvWorkloadReplayError> {
    let component = CheckpointComponentId::new("memory0")
        .map_err(|error| RiscvWorkloadReplayError::System(crate::SystemError::Checkpoint(error)))?;
    let mut host_controller = host_controller.lock().expect("system host controller lock");
    match memory {
        WorkloadMemoryBackend::Store(store) => {
            let bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                component,
                Arc::clone(store),
            )])
            .map_err(|error| {
                RiscvWorkloadReplayError::System(crate::SystemError::Checkpoint(error))
            })?;
            host_controller
                .executor_mut()
                .attach_memory_checkpoint_bank(bank)
                .map_err(|error| {
                    RiscvWorkloadReplayError::System(crate::SystemError::Checkpoint(error))
                })
        }
        WorkloadMemoryBackend::Dram(dram) => {
            let dram_controller = dram
                .lock()
                .expect("workload replay DRAM lock")
                .controller_handle();
            let bank = DramMemoryCheckpointBank::new([DramMemoryCheckpointPort::new(
                component,
                dram_controller,
            )])
            .map_err(|error| {
                RiscvWorkloadReplayError::System(crate::SystemError::Checkpoint(error))
            })?;
            host_controller
                .executor_mut()
                .attach_dram_memory_checkpoint_bank(bank)
                .map_err(|error| {
                    RiscvWorkloadReplayError::System(crate::SystemError::Checkpoint(error))
                })
        }
    }
}

fn apply_linux_boot_handoff(cores: &[RiscvCore], dtb_addr: Address) {
    let Some((boot, secondaries)) = cores.split_first() else {
        return;
    };

    for core in cores {
        configure_riscv_unrestricted_pmp(core)
            .expect("fresh workload core accepts unrestricted supervisor PMP policy");
    }
    boot.start_supervisor_hart(boot.pc(), dtb_addr.get());
    for core in secondaries {
        core.set_hart_stopped();
    }
}

fn workload_memory_route(
    route: &WorkloadMemoryRoute,
) -> Result<MemoryRoute, RiscvWorkloadReplayError> {
    let source = TransportEndpointId::new(route.source_endpoint())
        .map_err(RiscvWorkloadReplayError::Transport)?;
    let source_partition = PartitionId::new(route.source_partition());
    let hops = route
        .hops()
        .iter()
        .map(workload_memory_route_hop)
        .collect::<Result<Vec<_>, RiscvWorkloadReplayError>>()?;
    let route_fabric = route.hops().iter().find_map(WorkloadRouteHop::fabric);
    let route = MemoryRoute::new_path(source, source_partition, hops)
        .map_err(RiscvWorkloadReplayError::Transport)?;

    Ok(match route_fabric {
        Some(fabric) => route.with_virtual_networks(
            VirtualNetworkId::new(fabric.request_virtual_network()),
            VirtualNetworkId::new(fabric.response_virtual_network()),
        ),
        None => route,
    })
}

fn traffic_trace_replay_callback_error(
    replays: &[RiscvWorkloadScheduledTrafficTraceReplay],
) -> Option<(WorkloadRouteId, TrafficTraceReplayControllerParallelErrors)> {
    replays.iter().find_map(|replay| {
        let errors = replay.errors();
        (!errors.is_empty()).then(|| (replay.route().clone(), errors))
    })
}

fn workload_memory_route_hop(
    hop: &WorkloadRouteHop,
) -> Result<MemoryRouteHop, RiscvWorkloadReplayError> {
    let endpoint =
        TransportEndpointId::new(hop.endpoint()).map_err(RiscvWorkloadReplayError::Transport)?;
    let mut route_hop = MemoryRouteHop::new(
        endpoint,
        PartitionId::new(hop.partition()),
        hop.request_latency(),
        hop.response_latency(),
    )
    .map_err(RiscvWorkloadReplayError::Transport)?;

    if let Some(fabric) = hop.fabric() {
        route_hop = route_hop.with_request_fabric_path(workload_route_fabric_path(
            fabric,
            hop.request_latency(),
            VirtualNetworkId::new(fabric.request_virtual_network()),
        )?);
        route_hop = route_hop.with_response_fabric_path(workload_route_fabric_path(
            fabric,
            hop.response_latency(),
            VirtualNetworkId::new(fabric.response_virtual_network()),
        )?);
    }

    Ok(route_hop)
}

fn workload_route_fabric_path(
    fabric: &WorkloadRouteFabric,
    latency: u64,
    virtual_network: VirtualNetworkId,
) -> Result<FabricPath, RiscvWorkloadReplayError> {
    let link = FabricLinkId::new(fabric.link())
        .map_err(TransportError::Fabric)
        .map_err(RiscvWorkloadReplayError::Transport)?;
    let hop = FabricPathHop::new(link, latency, fabric.bandwidth_bytes_per_tick())
        .map_err(TransportError::Fabric)
        .map_err(RiscvWorkloadReplayError::Transport)?
        .with_virtual_network(virtual_network);
    let hop = if let Some(credit_depth) = fabric.credit_depth() {
        hop.with_credit_depth(credit_depth)
            .map_err(TransportError::Fabric)
            .map_err(RiscvWorkloadReplayError::Transport)?
    } else {
        hop
    };
    let hop = if let Some(router_stage) = fabric.router_stage() {
        let router = FabricRouterId::new(router_stage.router())
            .map_err(TransportError::Fabric)
            .map_err(RiscvWorkloadReplayError::Transport)?;
        let router_stage = FabricRouterStage::new(
            router,
            router_stage.input_port(),
            router_stage.output_port(),
            router_stage.virtual_channel(),
            router_stage.latency(),
        )
        .map_err(TransportError::Fabric)
        .map_err(RiscvWorkloadReplayError::Transport)?;
        hop.with_router_stage(router_stage)
    } else {
        hop
    };
    FabricPath::new([hop])
        .map_err(TransportError::Fabric)
        .map_err(RiscvWorkloadReplayError::Transport)
}

fn workload_execution_mode_from_system(
    mode: ExecutionMode,
) -> rem6_workload::WorkloadExecutionMode {
    match mode {
        ExecutionMode::Functional => rem6_workload::WorkloadExecutionMode::Functional,
        ExecutionMode::Timing => rem6_workload::WorkloadExecutionMode::Timing,
        ExecutionMode::Detailed => rem6_workload::WorkloadExecutionMode::Detailed,
    }
}

fn workload_checkpoint_summary(manifest: &CheckpointManifest) -> WorkloadCheckpointManifestSummary {
    workload_checkpoint_summary_at_tick(manifest, manifest.tick())
}

fn workload_checkpoint_summary_at_tick(
    manifest: &CheckpointManifest,
    tick: Tick,
) -> WorkloadCheckpointManifestSummary {
    let summary = manifest.summary();
    WorkloadCheckpointManifestSummary::with_component_summaries(
        manifest.label(),
        tick,
        summary.component_summaries().iter().map(|component| {
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                component.component().as_str(),
                component.chunk_summaries().iter().map(|chunk| {
                    WorkloadCheckpointChunkSummary::new(chunk.name(), chunk.payload_bytes())
                }),
            )
        }),
    )
}

pub(in crate::workload_replay) fn take_data_cache_error(
    data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
) -> Option<RiscvWorkloadReplayError> {
    data_cache.as_ref().and_then(|data_cache| {
        data_cache
            .lock()
            .expect("workload data cache lock")
            .take_error()
    })
}

#[derive(Clone, Debug)]
pub struct RiscvWorkloadReplayOutcome {
    cluster: RiscvCluster,
    run: RiscvSystemRun,
    result: WorkloadResult,
    host_action_outcomes: Vec<SystemActionOutcome>,
    memory_snapshot: PartitionedMemorySnapshot,
    dram_snapshot: Option<DramMemorySnapshot>,
    device_snapshots: RiscvWorkloadReplayDeviceSnapshots,
    traffic_trace_replays: Vec<RiscvWorkloadTrafficTraceReplayOutcome>,
}

#[derive(Clone, Debug, Default)]
pub struct RiscvWorkloadReplayDeviceSnapshots {
    gpu_snapshots: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
    accelerator_snapshots: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
    sinic_pci_snapshots: BTreeMap<u32, SinicFifoDeviceSnapshot>,
}

impl RiscvWorkloadReplayDeviceSnapshots {
    pub fn new(
        gpu_snapshots: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
        accelerator_snapshots: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
        sinic_pci_snapshots: BTreeMap<u32, SinicFifoDeviceSnapshot>,
    ) -> Self {
        Self {
            gpu_snapshots,
            accelerator_snapshots,
            sinic_pci_snapshots,
        }
    }

    pub fn gpu_snapshots(&self) -> &BTreeMap<GpuDeviceId, GpuDeviceSnapshot> {
        &self.gpu_snapshots
    }

    pub fn gpu_snapshot(&self, device: GpuDeviceId) -> Option<&GpuDeviceSnapshot> {
        self.gpu_snapshots.get(&device)
    }

    pub fn accelerator_snapshots(
        &self,
    ) -> &BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot> {
        &self.accelerator_snapshots
    }

    pub fn accelerator_snapshot(
        &self,
        engine: AcceleratorEngineId,
    ) -> Option<&AcceleratorEngineSnapshot> {
        self.accelerator_snapshots.get(&engine)
    }

    pub fn sinic_pci_snapshots(&self) -> &BTreeMap<u32, SinicFifoDeviceSnapshot> {
        &self.sinic_pci_snapshots
    }

    pub fn sinic_pci_snapshot(&self, nic: u32) -> Option<&SinicFifoDeviceSnapshot> {
        self.sinic_pci_snapshots.get(&nic)
    }
}

impl RiscvWorkloadReplayOutcome {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cluster: RiscvCluster,
        run: RiscvSystemRun,
        result: WorkloadResult,
        host_action_outcomes: Vec<SystemActionOutcome>,
        memory_snapshot: PartitionedMemorySnapshot,
        dram_snapshot: Option<DramMemorySnapshot>,
        device_snapshots: RiscvWorkloadReplayDeviceSnapshots,
        traffic_trace_replays: Vec<RiscvWorkloadTrafficTraceReplayOutcome>,
    ) -> Self {
        Self {
            cluster,
            run,
            result,
            host_action_outcomes,
            memory_snapshot,
            dram_snapshot,
            device_snapshots,
            traffic_trace_replays,
        }
    }

    pub const fn cluster(&self) -> &RiscvCluster {
        &self.cluster
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

    pub const fn memory_snapshot(&self) -> &PartitionedMemorySnapshot {
        &self.memory_snapshot
    }

    pub const fn dram_snapshot(&self) -> Option<&DramMemorySnapshot> {
        self.dram_snapshot.as_ref()
    }

    pub const fn device_snapshots(&self) -> &RiscvWorkloadReplayDeviceSnapshots {
        &self.device_snapshots
    }

    pub fn traffic_trace_replays(&self) -> &[RiscvWorkloadTrafficTraceReplayOutcome] {
        &self.traffic_trace_replays
    }

    pub fn gpu_snapshots(&self) -> &BTreeMap<GpuDeviceId, GpuDeviceSnapshot> {
        self.device_snapshots.gpu_snapshots()
    }

    pub fn gpu_snapshot(&self, device: GpuDeviceId) -> Option<&GpuDeviceSnapshot> {
        self.device_snapshots.gpu_snapshot(device)
    }

    pub fn accelerator_snapshots(
        &self,
    ) -> &BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot> {
        self.device_snapshots.accelerator_snapshots()
    }

    pub fn accelerator_snapshot(
        &self,
        engine: AcceleratorEngineId,
    ) -> Option<&AcceleratorEngineSnapshot> {
        self.device_snapshots.accelerator_snapshot(engine)
    }

    pub fn sinic_pci_snapshots(&self) -> &BTreeMap<u32, SinicFifoDeviceSnapshot> {
        self.device_snapshots.sinic_pci_snapshots()
    }

    pub fn sinic_pci_snapshot(&self, nic: u32) -> Option<&SinicFifoDeviceSnapshot> {
        self.device_snapshots.sinic_pci_snapshot(nic)
    }
}

#[cfg(test)]
#[path = "workload_replay/tests.rs"]
mod tests;
