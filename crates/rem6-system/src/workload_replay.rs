use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_accelerator::{AcceleratorEngineId, AcceleratorEngineSnapshot, AcceleratorError};
use rem6_boot::BootError;
use rem6_coherence::{
    CpuResponseRecord, HarnessError, LineBackingStore, MesiCpuResponseRecord, MesiHarnessError,
    MoesiCpuResponseRecord, MoesiHarnessError, PartitionedCacheAgentConfig,
    PartitionedDirectoryLineHarness, PartitionedDramMemoryConfig,
    PartitionedMesiDirectoryLineHarness, PartitionedMoesiDirectoryLineHarness,
};
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuError, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemoryError, DramMemorySnapshot,
    DramMemoryWaitForMarker, DramTargetActivity, DramTiming,
};
use rem6_gpu::{GpuDeviceId, GpuDeviceSnapshot, GpuDmaCopy, GpuDmaId, GpuError};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError, WaitForGraph};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId,
    MemoryResponse, MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
    ResponseStatus,
};
use rem6_stats::StatsRegistry;
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId, TransportError,
};
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadError,
    WorkloadGpuDmaCopy, WorkloadHostEvent, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResult, WorkloadRiscvDataCache,
    WorkloadRouteId, WorkloadTopology,
};

mod cache_response;
mod workload_replay_dma;

use self::cache_response::{cached_memory_response, data_cache_agents};
use self::workload_replay_dma::{run_accelerator_dma_copies, WorkloadAcceleratorDmaActivity};
use crate::workload_replay_heterogeneous::{
    accelerator_snapshots, build_accelerator_devices, build_gpu_devices, gpu_snapshots,
    schedule_accelerator_commands, schedule_gpu_kernel_launches, WorkloadAcceleratorActivity,
    WorkloadGpuActivity, WorkloadGpuRuntime,
};
use crate::{
    GuestEvent, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId, HostEventPolicy,
    RiscvDataCacheProtocol, RiscvDataCacheRunRecord, RiscvInstructionStats, RiscvSystemRun,
    RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort, SystemActionOutcome,
    SystemHostController, SystemHostEventPort,
};

const DEFAULT_MAX_TURNS: usize = 64;
const WORKLOAD_STOP_REASON_HOST: &str = "host-stop";
const WORKLOAD_STOP_REASON_IDLE: &str = "idle";
const PLANNED_HOST_EVENT_ID_BASE: u64 = 10_000;

#[derive(Clone)]
enum WorkloadMemoryBackend {
    Store(Arc<Mutex<PartitionedMemoryStore>>),
    Dram(Arc<Mutex<DramMemoryController>>),
}

impl WorkloadMemoryBackend {
    fn memory_snapshot(&self) -> PartitionedMemorySnapshot {
        match self {
            Self::Store(store) => store
                .lock()
                .expect("workload replay memory lock")
                .snapshot(),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .snapshot()
                .store()
                .clone(),
        }
    }

    fn dram_snapshot(&self) -> Option<DramMemorySnapshot> {
        match self {
            Self::Store(_) => None,
            Self::Dram(dram) => Some(dram.lock().expect("workload replay DRAM lock").snapshot()),
        }
    }

    fn dram_target_activities(&self) -> Vec<DramTargetActivity> {
        match self {
            Self::Store(_) => Vec::new(),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .target_activities(),
        }
    }

    fn mark_dram_wait_for(&self) -> Option<DramMemoryWaitForMarker> {
        match self {
            Self::Store(_) => None,
            Self::Dram(dram) => Some(
                dram.lock()
                    .expect("workload replay DRAM lock")
                    .mark_wait_for(),
            ),
        }
    }

    fn dram_wait_for_since(&self, marker: Option<DramMemoryWaitForMarker>) -> WaitForGraph {
        let Some(marker) = marker else {
            return WaitForGraph::new();
        };
        match self {
            Self::Store(_) => WaitForGraph::new(),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .wait_for_graph_since(&marker),
        }
    }

    fn line_data(
        &self,
        target: MemoryTargetId,
        line: Address,
    ) -> Result<Vec<u8>, RiscvWorkloadReplayError> {
        match self {
            Self::Store(store) => store
                .lock()
                .expect("workload replay memory lock")
                .line_data(target, line)
                .map_err(RiscvWorkloadReplayError::Memory),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .line_data(target, line)
                .map_err(RiscvWorkloadReplayError::Dram),
        }
    }

    fn insert_line(
        &self,
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    ) -> Result<(), RiscvWorkloadReplayError> {
        match self {
            Self::Store(store) => store
                .lock()
                .expect("workload replay memory lock")
                .insert_line(target, line, data)
                .map_err(RiscvWorkloadReplayError::Memory),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .insert_line(target, line, data)
                .map_err(RiscvWorkloadReplayError::Dram),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WorkloadGpuDmaActivity {
    copy_count: usize,
    completion_count: usize,
    active_device_count: usize,
}

struct WorkloadReplayActivityRefs<'a> {
    gpu: &'a WorkloadGpuActivity,
    gpu_dma: &'a WorkloadGpuDmaActivity,
    accelerator: &'a WorkloadAcceleratorActivity,
    accelerator_dma: &'a WorkloadAcceleratorDmaActivity,
}

enum WorkloadDataCacheHarness {
    Msi(PartitionedDirectoryLineHarness),
    Mesi(PartitionedMesiDirectoryLineHarness),
    Moesi(PartitionedMoesiDirectoryLineHarness),
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
        }
    }

    fn respond(&mut self, delivery: &RequestDelivery) -> Option<TargetOutcome> {
        if delivery.request().line_address() != self.line {
            return None;
        }
        let outcome = match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => workload_msi_data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
            ),
            WorkloadDataCacheHarness::Mesi(harness) => workload_mesi_data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
            ),
            WorkloadDataCacheHarness::Moesi(harness) => workload_moesi_data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
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

trait WorkloadDataCacheResponseRecord {
    fn status(&self) -> ResponseStatus;
    fn data(&self) -> Option<&[u8]>;
    fn tick(&self) -> u64;
    fn request(&self) -> MemoryRequestId;
}

impl WorkloadDataCacheResponseRecord for CpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }

    fn request(&self) -> MemoryRequestId {
        self.request()
    }
}

impl WorkloadDataCacheResponseRecord for MesiCpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }

    fn request(&self) -> MemoryRequestId {
        self.request()
    }
}

impl WorkloadDataCacheResponseRecord for MoesiCpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }

    fn request(&self) -> MemoryRequestId {
        self.request()
    }
}

fn workload_msi_data_cache_response_result(
    protocol: RiscvDataCacheProtocol,
    records: &mut Vec<RiscvDataCacheRunRecord>,
    harness: &mut PartitionedDirectoryLineHarness,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvWorkloadReplayError> {
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(RiscvWorkloadReplayError::MsiDataCache)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(RiscvWorkloadReplayError::MsiDataCache)?;
    let response_record = workload_data_cache_response_record(
        &harness.cpu_responses(),
        responses_before,
        delivery.request().id(),
    )?;
    records.push(RiscvDataCacheRunRecord::new(protocol, run));

    workload_data_cache_response_record_to_target_outcome(
        delivery.request(),
        start_tick,
        &response_record,
    )
}

fn workload_mesi_data_cache_response_result(
    protocol: RiscvDataCacheProtocol,
    records: &mut Vec<RiscvDataCacheRunRecord>,
    harness: &mut PartitionedMesiDirectoryLineHarness,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvWorkloadReplayError> {
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(RiscvWorkloadReplayError::MesiDataCache)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(RiscvWorkloadReplayError::MesiDataCache)?;
    let response_record = workload_data_cache_response_record(
        &harness.cpu_responses(),
        responses_before,
        delivery.request().id(),
    )?;
    records.push(RiscvDataCacheRunRecord::new(protocol, run));

    workload_data_cache_response_record_to_target_outcome(
        delivery.request(),
        start_tick,
        &response_record,
    )
}

fn workload_moesi_data_cache_response_result(
    protocol: RiscvDataCacheProtocol,
    records: &mut Vec<RiscvDataCacheRunRecord>,
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvWorkloadReplayError> {
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(RiscvWorkloadReplayError::MoesiDataCache)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(RiscvWorkloadReplayError::MoesiDataCache)?;
    let response_record = workload_data_cache_response_record(
        &harness.cpu_responses(),
        responses_before,
        delivery.request().id(),
    )?;
    records.push(RiscvDataCacheRunRecord::new(protocol, run));

    workload_data_cache_response_record_to_target_outcome(
        delivery.request(),
        start_tick,
        &response_record,
    )
}

fn workload_data_cache_response_record<R>(
    responses: &[R],
    responses_before: usize,
    request: MemoryRequestId,
) -> Result<R, RiscvWorkloadReplayError>
where
    R: Clone + WorkloadDataCacheResponseRecord,
{
    responses
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == request)
        .cloned()
        .ok_or(RiscvWorkloadReplayError::MissingDataCacheResponse { request })
}

fn workload_data_cache_response_record_to_target_outcome<R>(
    request: &MemoryRequest,
    start_tick: u64,
    record: &R,
) -> Result<TargetOutcome, RiscvWorkloadReplayError>
where
    R: WorkloadDataCacheResponseRecord,
{
    let response = workload_data_cache_response_record_to_memory_response(request, record)?;
    let delay = record.tick().saturating_sub(start_tick);
    if delay == 0 {
        Ok(TargetOutcome::Respond(response))
    } else {
        Ok(TargetOutcome::RespondAfter { delay, response })
    }
}

fn workload_data_cache_response_record_to_memory_response<R>(
    request: &MemoryRequest,
    record: &R,
) -> Result<MemoryResponse, RiscvWorkloadReplayError>
where
    R: WorkloadDataCacheResponseRecord,
{
    match record.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(request, record.data().map(<[u8]>::to_vec))
                .map_err(RiscvWorkloadReplayError::Memory)
        }
        ResponseStatus::Retry => Ok(MemoryResponse::retry(request)),
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
        let memory = self.load_memory_backend()?;
        let data_cache = self.build_data_cache(&memory)?;
        let gpu_devices = build_gpu_devices(topology)?;
        let transport = self.build_transport()?;
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
        if let Some(fabric_wait_for) =
            fabric_wait_for_start.and_then(|marker| transport.fabric_wait_for_graph_since(marker))
        {
            run = run.with_fabric_wait_for(fabric_wait_for);
        }
        run = run.with_dram_wait_for(memory.dram_wait_for_since(dram_wait_for_start));
        let gpu_snapshots = gpu_snapshots(&gpu_devices);
        let gpu_activity =
            WorkloadGpuActivity::from_snapshots(gpu_launch_count, &gpu_before, &gpu_snapshots);
        let accelerator_snapshots = accelerator_snapshots(&accelerator_devices);
        let accelerator_activity = WorkloadAcceleratorActivity::from_snapshots(
            accelerator_command_count,
            &accelerator_before,
            &accelerator_snapshots,
        );
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
                Ok(RiscvCore::with_data(
                    cpu_core,
                    CpuDataConfig::new(
                        TransportEndpointId::new(data_endpoint)
                            .map_err(RiscvWorkloadReplayError::Transport)?,
                        data_route,
                        layout,
                    ),
                ))
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

        Ok(WorkloadGpuDmaActivity {
            copy_count: topology.gpu_dma_copies().len(),
            completion_count,
            active_device_count,
        })
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
        let Some(data_route) = topology
            .riscv_cores()
            .iter()
            .filter_map(|core| core.data_route())
            .next()
        else {
            return Err(RiscvWorkloadReplayError::MissingDataCacheAgent);
        };
        topology
            .memory_routes()
            .iter()
            .find(|route| route.id() == data_route)
            .ok_or_else(|| RiscvWorkloadReplayError::MissingRoute {
                route: data_route.clone(),
            })
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

    fn load_memory_backend(&self) -> Result<WorkloadMemoryBackend, RiscvWorkloadReplayError> {
        let store = self.load_partitioned_memory_store()?;
        if self.uses_profiled_external_memory()? {
            let dram = self.load_dram_memory(store)?;
            return Ok(WorkloadMemoryBackend::Dram(Arc::new(Mutex::new(dram))));
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
        Ok(store)
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
        for outcome in &host_action_outcomes {
            if let SystemActionOutcome::Checkpoint { manifest, .. } = outcome {
                result = result.with_checkpoint_label(manifest.label());
            }
        }
        Ok((
            result
                .with_parallel_execution_summary(parallel_execution_summary(
                    run, topology, activities,
                ))
                .with_stats_snapshot(controller.executor().stats().snapshot(final_tick)),
            host_action_outcomes,
        ))
    }
}

fn parallel_execution_summary(
    run: &RiscvSystemRun,
    topology: &WorkloadTopology,
    activities: WorkloadReplayActivityRefs<'_>,
) -> WorkloadParallelExecutionSummary {
    let scheduler = run.parallel_scheduler_profile();
    let cpu_activities = run.cpu_activities();
    let riscv_fetch_issue_count = cpu_activities
        .values()
        .map(|activity| activity.fetch_issue_count())
        .sum();
    let riscv_committed_instruction_count = cpu_activities
        .values()
        .map(|activity| activity.instruction_execution_count())
        .sum();
    let riscv_data_access_issue_count = cpu_activities
        .values()
        .map(|activity| activity.data_access_issue_count())
        .sum();
    let riscv_scheduled_trap_count = cpu_activities
        .values()
        .map(|activity| activity.scheduled_trap_count())
        .sum();
    WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(
            scheduler.epoch_count(),
            scheduler.empty_epoch_count(),
            scheduler.dispatch_count(),
            scheduler.batch_count(),
        )
        .with_scheduler_partitions(
            run.active_parallel_scheduler_partition_count(),
            run.max_parallel_scheduler_workers(),
        )
        .with_riscv_core_counts(
            topology.riscv_cores().len(),
            cpu_activities.len(),
            riscv_fetch_issue_count,
            riscv_committed_instruction_count,
            riscv_data_access_issue_count,
            riscv_scheduled_trap_count,
        )
        .with_data_cache_parallel_counts(
            run.data_cache_run_count(),
            run.data_cache_parallel_scheduler_epoch_count(),
            run.data_cache_parallel_scheduler_dispatch_count(),
            run.data_cache_parallel_scheduler_batch_count(),
            run.data_cache_parallel_scheduler_max_workers(),
        )
        .with_data_cache_run_attribution(
            run.attributed_data_cache_parallel_run_count(),
            run.unattributed_data_cache_parallel_run_count(),
        )
        .with_data_cache_protocol_counts(run.data_cache_protocol_counts().into_iter().map(
            |(protocol, run_count)| {
                WorkloadDataCacheProtocolCount::new(
                    workload_data_cache_protocol(protocol),
                    run_count,
                )
            },
        ))
        .with_data_cache_diagnostics(
            run.data_cache_wait_for_edge_count(),
            run.data_cache_deadlock_diagnostic_count(),
        )
        .with_resource_diagnostics(
            run.fabric_wait_for_edge_count(),
            run.fabric_deadlock_diagnostic_count(),
            run.dram_wait_for_edge_count(),
            run.dram_deadlock_diagnostic_count(),
        )
        .with_gpu_compute_counts(
            activities.gpu.kernel_launch_count,
            activities.gpu.trace_event_count,
            activities.gpu.workgroup_completion_count,
            activities.gpu.active_device_count,
        )
        .with_gpu_dma_counts(
            activities.gpu_dma.copy_count,
            activities.gpu_dma.completion_count,
            activities.gpu_dma.active_device_count,
        )
        .with_accelerator_compute_counts(
            activities.accelerator.command_count,
            activities.accelerator.trace_event_count,
            activities.accelerator.completion_count,
            activities.accelerator.active_device_count,
        )
        .with_accelerator_dma_counts(
            activities.accelerator_dma.copy_count,
            activities.accelerator_dma.completion_count,
            activities.accelerator_dma.active_device_count,
        )
}

fn workload_data_cache_protocol(protocol: RiscvDataCacheProtocol) -> WorkloadDataCacheProtocol {
    match protocol {
        RiscvDataCacheProtocol::Msi => WorkloadDataCacheProtocol::Msi,
        RiscvDataCacheProtocol::Mesi => WorkloadDataCacheProtocol::Mesi,
        RiscvDataCacheProtocol::Moesi => WorkloadDataCacheProtocol::Moesi,
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

fn memory_response(memory: &WorkloadMemoryBackend, delivery: &RequestDelivery) -> TargetOutcome {
    match memory {
        WorkloadMemoryBackend::Store(store) => {
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
        WorkloadMemoryBackend::Dram(dram) => {
            let outcome = dram
                .lock()
                .expect("workload DRAM lock")
                .accept(delivery.tick(), delivery.request())
                .expect("workload DRAM response");
            let Some(response) = outcome.response().cloned() else {
                return TargetOutcome::NoResponse;
            };
            let delay = outcome
                .ready_cycle()
                .checked_sub(delivery.tick())
                .expect("workload DRAM response is not ready before request arrival");
            if delay == 0 {
                TargetOutcome::Respond(response)
            } else {
                TargetOutcome::RespondAfter { delay, response }
            }
        }
    }
}
