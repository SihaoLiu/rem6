use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_accelerator::{AcceleratorEngineId, AcceleratorEngineSnapshot, AcceleratorError};
use rem6_boot::{BootError, BootImage};
use rem6_coherence::{
    ChiHarnessError, HarnessError, LineBackingStore, MesiHarnessError, MoesiHarnessError,
    PartitionedCacheAgentConfig, PartitionedChiDirectoryLineHarness,
    PartitionedDirectoryLineHarness, PartitionedDramMemoryConfig,
    PartitionedMesiDirectoryLineHarness, PartitionedMoesiDirectoryLineHarness,
};
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuError, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemoryError, DramMemorySnapshot,
    DramTiming,
};
use rem6_fabric::{FabricLinkId, FabricModel, FabricPath, FabricPathHop, VirtualNetworkId};
use rem6_gpu::{GpuDeviceId, GpuDeviceSnapshot, GpuDmaCopy, GpuDmaId, GpuError};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError, WaitForGraph};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse, MemoryTargetId, PartitionedMemorySnapshot,
    PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery,
    TargetOutcome, TransportEndpointId, TransportError,
};
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadError, WorkloadGpuDmaCopy,
    WorkloadHostActionSummary, WorkloadHostEvent, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadReplayPlan, WorkloadResolvedResources, WorkloadResult, WorkloadRiscvDataCache,
    WorkloadRouteFabric, WorkloadRouteHop, WorkloadRouteId, WorkloadTopology,
};

mod cache_response;
mod memory_backend;
mod qos;
mod summary;
mod workload_replay_dma;

use self::cache_response::{
    cached_memory_response, cached_target_batch_response, data_cache_agents,
    data_cache_response_result,
};
use self::memory_backend::{memory_response, WorkloadDramBackend, WorkloadMemoryBackend};
use self::qos::{fixed_priority_policy, queue_arbiter};
use self::summary::{parallel_execution_summary, WorkloadReplayActivityRefs};
use self::workload_replay_dma::run_accelerator_dma_copies;
use crate::workload_replay_heterogeneous::{
    accelerator_snapshots, accelerator_wait_for_graph_since, accelerator_wait_for_markers,
    build_accelerator_devices, build_gpu_devices, gpu_snapshots, gpu_wait_for_graph_since,
    gpu_wait_for_markers, merge_wait_for_graph, schedule_accelerator_commands,
    schedule_gpu_kernel_launches, WorkloadAcceleratorActivity, WorkloadGpuActivity,
    WorkloadGpuRuntime,
};
use crate::{
    ExecutionMode, ExecutionModeTarget, GuestEvent, GuestEventDelivery, GuestEventId,
    GuestEventKind, GuestSourceId, HostEventPolicy, RiscvDataCacheProtocol,
    RiscvDataCacheRunRecord, RiscvInstructionStats, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, RiscvTrapEventPort, SystemActionOutcome, SystemHostController,
    SystemHostEventPort,
};

const DEFAULT_MAX_TURNS: usize = 64;
const WORKLOAD_STOP_REASON_HOST: &str = "host-stop";
const WORKLOAD_STOP_REASON_IDLE: &str = "idle";
const PLANNED_HOST_EVENT_ID_BASE: u64 = 10_000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WorkloadGpuDmaActivity {
    copy_count: usize,
    completion_count: usize,
    active_device_count: usize,
    wait_for_edge_count: usize,
    deadlock_diagnostic_count: usize,
}

impl WorkloadGpuDmaActivity {
    fn with_wait_for_graph(mut self, wait_for: WaitForGraph) -> Self {
        self.wait_for_edge_count = wait_for.edge_count();
        self.deadlock_diagnostic_count = wait_for.deadlock_diagnostic().into_iter().count();
        self
    }
}

enum WorkloadDataCacheHarness {
    Msi(PartitionedDirectoryLineHarness),
    Mesi(PartitionedMesiDirectoryLineHarness),
    Moesi(PartitionedMoesiDirectoryLineHarness),
    Chi(PartitionedChiDirectoryLineHarness),
}

enum WorkloadDataCacheLineMemory {
    Line(Vec<u8>),
    Dram(PartitionedDramMemoryConfig),
}

struct WorkloadDataCacheLineBackend {
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    line: Address,
    harness: WorkloadDataCacheHarness,
    records: Vec<RiscvDataCacheRunRecord>,
    error: Option<RiscvWorkloadReplayError>,
}

impl WorkloadDataCacheLineBackend {
    fn new(
        config: &WorkloadRiscvDataCache,
        layout: CacheLineLayout,
        line_address: Address,
        memory: WorkloadDataCacheLineMemory,
        agents: Vec<PartitionedCacheAgentConfig>,
    ) -> Result<Self, RiscvWorkloadReplayError> {
        let target = MemoryTargetId::new(config.memory_target());
        let line = layout.line_address(line_address);
        let directory_partition = PartitionId::new(config.directory_partition());
        let directory_endpoint = TransportEndpointId::new(config.directory_endpoint())
            .map_err(RiscvWorkloadReplayError::Transport)?;
        let (protocol, harness) = match config.protocol() {
            WorkloadDataCacheProtocol::Msi => (
                RiscvDataCacheProtocol::Msi,
                WorkloadDataCacheHarness::Msi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            PartitionedDirectoryLineHarness::new(
                                layout,
                                line,
                                LineBackingStore::new(layout, line, line_data)
                                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?,
                                directory_partition,
                                directory_endpoint,
                                agents,
                            )
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Mesi => (
                RiscvDataCacheProtocol::Mesi,
                WorkloadDataCacheHarness::Mesi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            LineBackingStore::new(layout, line, line_data)
                                .map_err(MesiHarnessError::Backing)
                                .and_then(|backing| {
                                    PartitionedMesiDirectoryLineHarness::new(
                                        layout,
                                        line,
                                        backing,
                                        directory_partition,
                                        directory_endpoint,
                                        agents,
                                    )
                                })
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedMesiDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Moesi => (
                RiscvDataCacheProtocol::Moesi,
                WorkloadDataCacheHarness::Moesi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            LineBackingStore::new(layout, line, line_data)
                                .map_err(MoesiHarnessError::Backing)
                                .and_then(|backing| {
                                    PartitionedMoesiDirectoryLineHarness::new(
                                        layout,
                                        line,
                                        backing,
                                        directory_partition,
                                        directory_endpoint,
                                        agents,
                                    )
                                })
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedMoesiDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Chi => (
                RiscvDataCacheProtocol::Chi,
                WorkloadDataCacheHarness::Chi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            LineBackingStore::new(layout, line, line_data)
                                .map_err(ChiHarnessError::Backing)
                                .and_then(|backing| {
                                    PartitionedChiDirectoryLineHarness::new(
                                        layout,
                                        line,
                                        backing,
                                        directory_partition,
                                        directory_endpoint,
                                        agents,
                                    )
                                })
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedChiDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)?,
                ),
            ),
        };

        Ok(Self {
            protocol,
            target,
            line,
            harness,
            records: Vec::new(),
            error: None,
        })
    }

    fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.records.clone()
    }

    fn take_error(&mut self) -> Option<RiscvWorkloadReplayError> {
        self.error.take()
    }

    fn target(&self) -> MemoryTargetId {
        self.target
    }

    fn line(&self) -> Address {
        self.line
    }

    fn accepts_delivery(&self, delivery: &RequestDelivery) -> bool {
        delivery.request().operation() != MemoryOperation::InstructionFetch
    }

    fn final_line_data(&self) -> Result<Vec<u8>, RiscvWorkloadReplayError> {
        match &self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?;
                if let Some(data) = snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                {
                    return Ok(data);
                }
                snapshot
                    .backing()
                    .map(|backing| backing.data().to_vec())
                    .or_else(|| {
                        snapshot
                            .dram_memory()
                            .and_then(|dram| dram_snapshot_line_data(dram, self.target, self.line))
                    })
                    .ok_or(RiscvWorkloadReplayError::MissingDataCacheLine)
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)?;
                Ok(snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                    .unwrap_or_else(|| snapshot.backing().data().to_vec()))
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)?;
                Ok(snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                    .unwrap_or_else(|| snapshot.backing().data().to_vec()))
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)?;
                Ok(snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                    .unwrap_or_else(|| snapshot.backing().data().to_vec()))
            }
        }
    }

    fn respond(&mut self, delivery: &RequestDelivery) -> Option<TargetOutcome> {
        if !self.accepts_delivery(delivery) {
            return None;
        }
        if delivery.request().line_address() != self.line {
            return None;
        }
        let outcome = match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::MsiDataCache,
            ),
            WorkloadDataCacheHarness::Mesi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::MesiDataCache,
            ),
            WorkloadDataCacheHarness::Moesi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::MoesiDataCache,
            ),
            WorkloadDataCacheHarness::Chi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::ChiDataCache,
            ),
        };
        Some(match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                self.error = Some(error);
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
        })
    }
}

struct WorkloadDataCacheBackend {
    lines: BTreeMap<Address, WorkloadDataCacheLineBackend>,
}

impl WorkloadDataCacheBackend {
    fn new<I>(lines: I) -> Self
    where
        I: IntoIterator<Item = WorkloadDataCacheLineBackend>,
    {
        Self {
            lines: lines.into_iter().map(|line| (line.line(), line)).collect(),
        }
    }

    fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.lines
            .values()
            .flat_map(WorkloadDataCacheLineBackend::records)
            .collect()
    }

    fn take_error(&mut self) -> Option<RiscvWorkloadReplayError> {
        self.lines
            .values_mut()
            .find_map(WorkloadDataCacheLineBackend::take_error)
    }

    fn final_lines(
        &self,
    ) -> Result<Vec<(MemoryTargetId, Address, Vec<u8>)>, RiscvWorkloadReplayError> {
        self.lines
            .values()
            .map(|line| Ok((line.target(), line.line(), line.final_line_data()?)))
            .collect()
    }

    fn respond(&mut self, delivery: &RequestDelivery) -> Option<TargetOutcome> {
        self.lines
            .get_mut(&delivery.request().line_address())
            .and_then(|line| line.respond(delivery))
    }
}

fn workload_instruction_stats(
    topology: &WorkloadTopology,
    stats: &mut StatsRegistry,
) -> Result<RiscvInstructionStats, RiscvWorkloadReplayError> {
    topology
        .riscv_cores()
        .iter()
        .map(|core| {
            let stat = stats
                .register_counter(format!("cpu{}.committed_insts", core.cpu()), "count")
                .map_err(|error| {
                    RiscvWorkloadReplayError::System(crate::SystemError::Stats(error))
                })?;
            Ok((CpuId::new(core.cpu()), stat))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(RiscvInstructionStats::new)
}

fn dram_snapshot_line_data(
    snapshot: &DramMemorySnapshot,
    target: MemoryTargetId,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .store()
        .partitions()
        .iter()
        .find(|partition| partition.target() == target)
        .and_then(|partition| {
            partition
                .lines()
                .iter()
                .find(|candidate| candidate.line() == line)
        })
        .map(|line| line.data().to_vec())
}

fn load_payload_at(
    store: &mut PartitionedMemoryStore,
    address: Address,
    payload: &[u8],
) -> Result<(), RiscvWorkloadReplayError> {
    BootImage::new(address)
        .add_segment(address, payload.to_vec())
        .map_err(RiscvWorkloadReplayError::Boot)?
        .load_into_partitioned_store_by_address(store)
        .map(|_| ())
        .map_err(RiscvWorkloadReplayError::Boot)
}

#[derive(Clone, Debug)]
pub struct RiscvWorkloadReplay {
    plan: WorkloadReplayPlan,
    resolved_resources: Option<WorkloadResolvedResources>,
    max_turns: usize,
}

impl RiscvWorkloadReplay {
    pub const fn new(plan: WorkloadReplayPlan) -> Self {
        Self {
            plan,
            resolved_resources: None,
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

    pub fn run_parallel(&self) -> Result<RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
        self.validate_resolved_resources()?;
        let topology = self
            .plan
            .topology()
            .ok_or(RiscvWorkloadReplayError::MissingTopology)?;
        let route_map = self.build_route_map()?;
        let memory = self.load_memory_backend()?;
        let data_cache = self.build_data_cache(&memory)?;
        let gpu_devices = build_gpu_devices(topology)?;
        let mut transport = self.build_transport()?;
        if topology.qos_policy().is_some() {
            let batch_memory = memory.clone();
            let batch_data_cache = data_cache.clone();
            transport =
                transport.with_direct_target_batch_responder(move |deliveries, _context| {
                    cached_target_batch_response(
                        batch_data_cache.as_ref(),
                        &batch_memory,
                        deliveries,
                    )
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
        let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(
            topology.partition_count(),
            topology.min_remote_delay(),
            topology.parallel_worker_limit(),
        )
        .map_err(RiscvWorkloadReplayError::Scheduler)?;
        let mut stats = StatsRegistry::new();
        let instruction_stats = workload_instruction_stats(topology, &mut stats)?;
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            stats,
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
        let driver = RiscvSystemRunDriver::with_instruction_stats(trap_port, instruction_stats);
        let gpu_launch_count =
            schedule_gpu_kernel_launches(topology, &gpu_devices, &mut scheduler)?;
        let accelerator_command_count =
            schedule_accelerator_commands(topology, &accelerator_devices, &mut scheduler)?;
        let run_result = driver.drive_until_host_stop_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let memory = memory.clone();
                move |delivery, _context| memory_response(&memory, &delivery)
            },
            |_cpu| {
                let memory = memory.clone();
                let data_cache = data_cache.clone();
                move |delivery, _context| {
                    cached_memory_response(data_cache.as_ref(), &memory, &delivery)
                }
            },
            self.max_turns,
            |cpu| GuestEventId::new(1_000 + u64::from(cpu.get())),
        );
        if let Some(data_cache) = data_cache.as_ref() {
            if let Some(error) = data_cache
                .lock()
                .expect("workload data cache lock")
                .take_error()
            {
                return Err(error);
            }
        }
        let mut run = run_result.map_err(RiscvWorkloadReplayError::System)?;
        if let Some(data_cache) = data_cache.as_ref() {
            let (final_lines, records) = {
                let data_cache = data_cache.lock().expect("workload data cache lock");
                (data_cache.final_lines()?, data_cache.records())
            };
            for (target, line, line_data) in final_lines {
                memory.insert_line(target, line, line_data)?;
            }
            if !records.is_empty() {
                run = run.with_data_cache_run_records(records);
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
            &accelerator_before,
            &accelerator_snapshots,
        )
        .with_wait_for_graph(accelerator_wait_for);
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
        )?;
        self.plan
            .verify_result(&result)
            .map_err(RiscvWorkloadReplayError::Workload)?;
        let memory_snapshot = memory.memory_snapshot();
        let dram_snapshot = memory.dram_snapshot();

        Ok(RiscvWorkloadReplayOutcome::new(
            cluster,
            run,
            result,
            host_action_outcomes,
            memory_snapshot,
            dram_snapshot,
            RiscvWorkloadReplayDeviceSnapshots::new(gpu_snapshots, accelerator_snapshots),
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
        let has_fabric_routes = topology
            .memory_routes()
            .iter()
            .any(|route| route.hops().iter().any(|hop| hop.fabric().is_some()));
        let mut transport = if has_fabric_routes {
            if let Some(policy) = topology.qos_policy() {
                MemoryTransport::with_fabric_qos_policy(
                    FabricModel::new(),
                    queue_arbiter(policy),
                    fixed_priority_policy(policy),
                )
            } else {
                MemoryTransport::with_fabric(FabricModel::new())
            }
        } else if let Some(policy) = topology.qos_policy() {
            MemoryTransport::with_qos_policy(queue_arbiter(policy), fixed_priority_policy(policy))
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
                Ok(RiscvCore::with_data(cpu_core, data_config))
            })
            .collect::<Result<Vec<_>, RiscvWorkloadReplayError>>()?;

        RiscvCluster::new(cores).map_err(RiscvWorkloadReplayError::RiscvCluster)
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
        scheduler
            .run_until_idle_parallel()
            .map_err(RiscvWorkloadReplayError::Scheduler)?;
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
                return Err(RiscvWorkloadReplayError::MissingGpuDmaWrite { device });
            }
        }
        scheduler
            .run_until_idle_parallel()
            .map_err(RiscvWorkloadReplayError::Scheduler)?;

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
            wait_for_edge_count: 0,
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
        let agents = data_cache_agents(topology)?;
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

        Ok(WorkloadDataCacheLineMemory::Dram(
            PartitionedDramMemoryConfig::new(
                PartitionId::new(memory_route.target_partition()),
                TransportEndpointId::new(memory_route.target_endpoint())
                    .map_err(RiscvWorkloadReplayError::Transport)?,
                memory_route.request_latency(),
                memory_route.response_latency(),
                dram,
            ),
        ))
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

    fn load_linux_device_tree_payload(
        &self,
        store: &mut PartitionedMemoryStore,
    ) -> Result<(), RiscvWorkloadReplayError> {
        let Some(handoff) = self.plan.linux_boot_handoff() else {
            return Ok(());
        };
        let Some(resource) = handoff.device_tree_resource() else {
            return Ok(());
        };
        let payload = self
            .resolved_resources
            .as_ref()
            .and_then(|resources| resources.linux_device_tree_data(handoff))
            .ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingResourcePayload {
                    resource: resource.clone(),
                })
            })?;

        load_payload_at(store, handoff.dtb_addr(), payload)
    }

    fn load_linux_initrd_payload(
        &self,
        store: &mut PartitionedMemoryStore,
    ) -> Result<(), RiscvWorkloadReplayError> {
        let Some(handoff) = self.plan.linux_boot_handoff() else {
            return Ok(());
        };
        let Some(initrd) = handoff.initrd() else {
            return Ok(());
        };
        let payload = self
            .resolved_resources
            .as_ref()
            .and_then(|resources| resources.linux_initrd_data(handoff))
            .ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingResourcePayload {
                    resource: initrd.resource().clone(),
                })
            })?;

        load_payload_at(store, initrd.start(), payload)
    }

    fn result_from_run(
        &self,
        run: &RiscvSystemRun,
        controller: &Arc<Mutex<SystemHostController>>,
        topology: &WorkloadTopology,
        activities: WorkloadReplayActivityRefs<'_>,
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
        let mut host_action_summary = WorkloadHostActionSummary::default();
        for outcome in &host_action_outcomes {
            match outcome {
                SystemActionOutcome::InjectedCommand { .. } => {
                    host_action_summary.record_injected_command();
                }
                SystemActionOutcome::StatsReset(_) => {
                    host_action_summary.record_stats_reset();
                }
                SystemActionOutcome::StatsSnapshot(_) => {
                    host_action_summary.record_stats_snapshot();
                }
                SystemActionOutcome::Checkpoint { manifest, .. } => {
                    host_action_summary.record_checkpoint();
                    result = result.with_checkpoint_label(manifest.label());
                }
                SystemActionOutcome::CheckpointRestored { manifest, .. } => {
                    host_action_summary.record_checkpoint_restore();
                    result = result.with_restored_checkpoint_label(manifest.label());
                }
                SystemActionOutcome::ExecutionModeSwitched {
                    tick,
                    target,
                    mode,
                    stats_epoch,
                    stats_reset_tick,
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
                    run, topology, activities,
                ))
                .with_host_action_summary(host_action_summary)
                .with_stats_snapshot(controller.executor().stats().snapshot(final_tick)),
            host_action_outcomes,
        ))
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
    FabricPath::new([hop])
        .map_err(TransportError::Fabric)
        .map_err(RiscvWorkloadReplayError::Transport)
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
        HostEventIntent::SwitchExecutionMode { target, mode } => {
            GuestEventKind::ExecutionModeSwitch {
                target: ExecutionModeTarget::new(target.clone()),
                mode: workload_execution_mode(mode),
            }
        }
        HostEventIntent::Checkpoint { label } => GuestEventKind::Checkpoint {
            label: label.clone(),
        },
        HostEventIntent::RestoreCheckpoint { label } => GuestEventKind::RestoreCheckpoint {
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

fn workload_execution_mode(mode: &rem6_workload::WorkloadExecutionMode) -> ExecutionMode {
    match mode {
        rem6_workload::WorkloadExecutionMode::Functional => ExecutionMode::Functional,
        rem6_workload::WorkloadExecutionMode::Timing => ExecutionMode::Timing,
        rem6_workload::WorkloadExecutionMode::Detailed => ExecutionMode::Detailed,
    }
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

#[derive(Clone, Debug)]
pub struct RiscvWorkloadReplayOutcome {
    cluster: RiscvCluster,
    run: RiscvSystemRun,
    result: WorkloadResult,
    host_action_outcomes: Vec<SystemActionOutcome>,
    memory_snapshot: PartitionedMemorySnapshot,
    dram_snapshot: Option<DramMemorySnapshot>,
    device_snapshots: RiscvWorkloadReplayDeviceSnapshots,
}

#[derive(Clone, Debug, Default)]
pub struct RiscvWorkloadReplayDeviceSnapshots {
    gpu_snapshots: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
    accelerator_snapshots: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
}

impl RiscvWorkloadReplayDeviceSnapshots {
    pub fn new(
        gpu_snapshots: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
        accelerator_snapshots: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
    ) -> Self {
        Self {
            gpu_snapshots,
            accelerator_snapshots,
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
}

impl RiscvWorkloadReplayOutcome {
    pub fn new(
        cluster: RiscvCluster,
        run: RiscvSystemRun,
        result: WorkloadResult,
        host_action_outcomes: Vec<SystemActionOutcome>,
        memory_snapshot: PartitionedMemorySnapshot,
        dram_snapshot: Option<DramMemorySnapshot>,
        device_snapshots: RiscvWorkloadReplayDeviceSnapshots,
    ) -> Self {
        Self {
            cluster,
            run,
            result,
            host_action_outcomes,
            memory_snapshot,
            dram_snapshot,
            device_snapshots,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvWorkloadReplayError {
    MissingTopology,
    MissingMemoryTarget,
    MissingDataCacheAgent,
    MissingDataCacheLine,
    MissingDataCacheResponse { request: MemoryRequestId },
    GpuDmaRequestSequenceOverflow { transfer: u64 },
    MissingGpuDmaWrite { device: GpuDeviceId },
    AcceleratorDmaRequestSequenceOverflow { transfer: u64 },
    MissingAcceleratorDmaWrite { engine: AcceleratorEngineId },
    MissingRoute { route: WorkloadRouteId },
    MissingFinalTick,
    Workload(WorkloadError),
    Boot(BootError),
    Dram(DramMemoryError),
    DramModel(rem6_dram::DramError),
    Memory(MemoryError),
    Gpu(GpuError),
    Accelerator(AcceleratorError),
    MsiDataCache(HarnessError),
    MesiDataCache(MesiHarnessError),
    MoesiDataCache(MoesiHarnessError),
    ChiDataCache(ChiHarnessError),
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
            Self::MissingDataCacheAgent => {
                write!(
                    formatter,
                    "workload replay data cache has no RISC-V data agents"
                )
            }
            Self::MissingDataCacheLine => {
                write!(formatter, "workload replay data cache has no line data")
            }
            Self::MissingDataCacheResponse { request } => write!(
                formatter,
                "workload replay data cache did not record response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::GpuDmaRequestSequenceOverflow { transfer } => write!(
                formatter,
                "workload replay GPU DMA transfer {transfer} cannot be mapped to request sequences"
            ),
            Self::MissingGpuDmaWrite { device } => write!(
                formatter,
                "workload replay GPU device {} has no pending DMA write",
                device.get()
            ),
            Self::AcceleratorDmaRequestSequenceOverflow { transfer } => write!(
                formatter,
                "workload replay accelerator DMA transfer {transfer} cannot be mapped to request sequences"
            ),
            Self::MissingAcceleratorDmaWrite { engine } => write!(
                formatter,
                "workload replay accelerator engine {} has no pending DMA write",
                engine.get()
            ),
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
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::DramModel(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Gpu(error) => write!(formatter, "{error}"),
            Self::Accelerator(error) => write!(formatter, "{error}"),
            Self::MsiDataCache(error) => write!(formatter, "{error}"),
            Self::MesiDataCache(error) => write!(formatter, "{error}"),
            Self::MoesiDataCache(error) => write!(formatter, "{error}"),
            Self::ChiDataCache(error) => write!(formatter, "{error}"),
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
            Self::Dram(error) => Some(error),
            Self::DramModel(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Accelerator(error) => Some(error),
            Self::MsiDataCache(error) => Some(error),
            Self::MesiDataCache(error) => Some(error),
            Self::MoesiDataCache(error) => Some(error),
            Self::ChiDataCache(error) => Some(error),
            Self::Cpu(error) => Some(error),
            Self::RiscvCluster(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::System(error) => Some(error),
            Self::MissingTopology
            | Self::MissingMemoryTarget
            | Self::MissingDataCacheAgent
            | Self::MissingDataCacheLine
            | Self::MissingDataCacheResponse { .. }
            | Self::GpuDmaRequestSequenceOverflow { .. }
            | Self::MissingGpuDmaWrite { .. }
            | Self::AcceleratorDmaRequestSequenceOverflow { .. }
            | Self::MissingAcceleratorDmaWrite { .. }
            | Self::MissingRoute { .. }
            | Self::MissingFinalTick => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_response_harness<H: cache_response::WorkloadDataCacheResponseHarness>() {}

    #[test]
    fn all_data_cache_protocol_harnesses_share_response_adapter() {
        assert_response_harness::<PartitionedDirectoryLineHarness>();
        assert_response_harness::<PartitionedMesiDirectoryLineHarness>();
        assert_response_harness::<PartitionedMoesiDirectoryLineHarness>();
        assert_response_harness::<PartitionedChiDirectoryLineHarness>();
    }
}
