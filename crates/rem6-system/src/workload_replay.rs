use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootError;
use rem6_coherence::{
    CpuResponseRecord, HarnessError, LineBackingStore, MesiCpuResponseRecord, MesiHarnessError,
    MoesiCpuResponseRecord, MoesiHarnessError, PartitionedCacheAgentConfig,
    PartitionedDirectoryLineHarness, PartitionedMesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarness,
};
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuError, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemoryError, DramMemorySnapshot,
    DramTargetActivity, DramTiming,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
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
    WorkloadHostEvent, WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResult,
    WorkloadRiscvDataCache, WorkloadRouteId, WorkloadTopology,
};

use crate::{
    GuestEvent, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId, HostEventPolicy,
    RiscvDataCacheProtocol, RiscvDataCacheRunRecord, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, RiscvTrapEventPort, SystemActionOutcome, SystemHostController,
    SystemHostEventPort,
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

enum WorkloadDataCacheHarness {
    Msi(PartitionedDirectoryLineHarness),
    Mesi(PartitionedMesiDirectoryLineHarness),
    Moesi(PartitionedMoesiDirectoryLineHarness),
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
        line_data: Vec<u8>,
        agents: Vec<PartitionedCacheAgentConfig>,
    ) -> Result<Self, RiscvWorkloadReplayError> {
        let target = MemoryTargetId::new(config.memory_target());
        let line = layout.line_address(line_address);
        let backing = LineBackingStore::new(layout, line, line_data)
            .map_err(RiscvWorkloadReplayError::MsiDataCache)?;
        let directory_partition = PartitionId::new(config.directory_partition());
        let directory_endpoint = TransportEndpointId::new(config.directory_endpoint())
            .map_err(RiscvWorkloadReplayError::Transport)?;
        let (protocol, harness) = match config.protocol() {
            WorkloadDataCacheProtocol::Msi => (
                RiscvDataCacheProtocol::Msi,
                WorkloadDataCacheHarness::Msi(
                    PartitionedDirectoryLineHarness::new(
                        layout,
                        line,
                        backing,
                        directory_partition,
                        directory_endpoint,
                        agents,
                    )
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Mesi => (
                RiscvDataCacheProtocol::Mesi,
                WorkloadDataCacheHarness::Mesi(
                    PartitionedMesiDirectoryLineHarness::new(
                        layout,
                        line,
                        backing,
                        directory_partition,
                        directory_endpoint,
                        agents,
                    )
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Moesi => (
                RiscvDataCacheProtocol::Moesi,
                WorkloadDataCacheHarness::Moesi(
                    PartitionedMoesiDirectoryLineHarness::new(
                        layout,
                        line,
                        backing,
                        directory_partition,
                        directory_endpoint,
                        agents,
                    )
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
                    if let Some(data_cache) = data_cache.as_ref() {
                        if let Some(outcome) = data_cache
                            .lock()
                            .expect("workload data cache lock")
                            .respond(&delivery)
                        {
                            return outcome;
                        }
                    }
                    memory_response(&memory, &delivery)
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
        let (result, host_action_outcomes) = self.result_from_run(&run, &controller)?;
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
        let agents = self.data_cache_agents(topology)?;
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
                WorkloadDataCacheLineBackend::new(
                    config,
                    layout,
                    *line_address,
                    line_data,
                    agents.clone(),
                )
            })
            .collect::<Result<Vec<_>, RiscvWorkloadReplayError>>()?;

        Ok(Some(Arc::new(Mutex::new(WorkloadDataCacheBackend::new(
            lines,
        )))))
    }

    fn data_cache_agents(
        &self,
        topology: &WorkloadTopology,
    ) -> Result<Vec<PartitionedCacheAgentConfig>, RiscvWorkloadReplayError> {
        topology
            .riscv_cores()
            .iter()
            .filter_map(|core| {
                let data_endpoint = core.data_endpoint()?;
                let data_route = core.data_route()?;
                Some((core, data_endpoint, data_route))
            })
            .map(|(core, data_endpoint, data_route)| {
                let route = topology
                    .memory_routes()
                    .iter()
                    .find(|route| route.id() == data_route)
                    .ok_or_else(|| RiscvWorkloadReplayError::MissingRoute {
                        route: data_route.clone(),
                    })?;
                Ok(PartitionedCacheAgentConfig::new(
                    AgentId::new(core.agent()),
                    PartitionId::new(route.source_partition()),
                    TransportEndpointId::new(data_endpoint)
                        .map_err(RiscvWorkloadReplayError::Transport)?,
                    route.request_latency(),
                    route.response_latency(),
                ))
            })
            .collect()
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
                .with_parallel_execution_summary(parallel_execution_summary(run))
                .with_stats_snapshot(controller.executor().stats().snapshot(final_tick)),
            host_action_outcomes,
        ))
    }
}

fn parallel_execution_summary(run: &RiscvSystemRun) -> WorkloadParallelExecutionSummary {
    let scheduler = run.parallel_scheduler_profile();
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
}

impl RiscvWorkloadReplayOutcome {
    pub const fn new(
        cluster: RiscvCluster,
        run: RiscvSystemRun,
        result: WorkloadResult,
        host_action_outcomes: Vec<SystemActionOutcome>,
        memory_snapshot: PartitionedMemorySnapshot,
        dram_snapshot: Option<DramMemorySnapshot>,
    ) -> Self {
        Self {
            cluster,
            run,
            result,
            host_action_outcomes,
            memory_snapshot,
            dram_snapshot,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvWorkloadReplayError {
    MissingTopology,
    MissingMemoryTarget,
    MissingDataCacheAgent,
    MissingDataCacheLine,
    MissingDataCacheResponse { request: MemoryRequestId },
    MissingRoute { route: WorkloadRouteId },
    MissingFinalTick,
    Workload(WorkloadError),
    Boot(BootError),
    Dram(DramMemoryError),
    DramModel(rem6_dram::DramError),
    Memory(MemoryError),
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
