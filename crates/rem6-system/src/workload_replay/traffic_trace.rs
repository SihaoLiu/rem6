use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_cpu::{
    HtmFailureCause, HtmTransactionUid, RiscvClusterHtmAbortOutcome, RiscvClusterHtmBeginOutcome,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, Tick};
use rem6_memory::{Address, MemoryRequestId, ResponseStatus};
use rem6_traffic::{
    TrafficController, TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceCacheKind,
    TrafficTraceDiagnosticKind, TrafficTraceErrorEvent, TrafficTraceErrorKind, TrafficTraceHtmKind,
    TrafficTraceMemoryWriteCompletionRecord, TrafficTraceResponseEvent, TrafficTraceResponseKind,
    TrafficTraceTlbKind,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport, RequestDelivery,
    ResponseDelivery, TargetOutcome,
};
use rem6_workload::{WorkloadRouteId, WorkloadTopology, WorkloadTrafficTraceReplaySummary};

use crate::{
    RiscvCluster, RiscvTraceDiagnosticRecord, RiscvTraceErrorRecord, RiscvTraceHtmAccessRecord,
    TrafficTraceReplayControlEvent, TrafficTraceReplayControlEventContext,
    TrafficTraceReplayControllerParallelErrors, TrafficTraceReplayControllerParallelExecutor,
    TrafficTraceReplayControllerRuntime, TrafficTraceReplayOrder,
    TrafficTraceReplayScheduledSidebandEvent, TrafficTraceReplaySidebandEvent,
    TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetEventContext,
};

use super::data_cache_backend::WorkloadDataCacheBackend;
use super::traffic_trace_records::RiscvWorkloadTraceReplayRecords;
use super::traffic_trace_sideband_records::{
    RiscvWorkloadTraceCacheFlushRecord, RiscvWorkloadTraceTlbSyncRecord,
};
use super::traffic_trace_sync::{
    RiscvWorkloadTraceL1InvalidationRecord, RiscvWorkloadTraceSyncRecord,
};
use super::RiscvWorkloadReplayError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTrafficTraceReplay {
    controller: TrafficController,
    route: WorkloadRouteId,
    control_partition: PartitionId,
    retry_delay: Tick,
}

impl RiscvWorkloadTrafficTraceReplay {
    pub const fn new(
        controller: TrafficController,
        route: WorkloadRouteId,
        control_partition: PartitionId,
    ) -> Self {
        Self {
            controller,
            route,
            control_partition,
            retry_delay: 0,
        }
    }

    pub const fn with_retry_delay(mut self, retry_delay: Tick) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    pub const fn controller(&self) -> &TrafficController {
        &self.controller
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn control_partition(&self) -> PartitionId {
        self.control_partition
    }

    pub const fn retry_delay(&self) -> Tick {
        self.retry_delay
    }
}

pub(super) struct RiscvWorkloadScheduledTrafficTraceReplay {
    route: WorkloadRouteId,
    scheduled_count: usize,
    trace: MemoryTrace,
    response_deliveries: Arc<Mutex<Vec<ResponseDelivery>>>,
    records: RiscvWorkloadTraceReplayRecords,
    executor: TrafficTraceReplayControllerParallelExecutor,
}

impl RiscvWorkloadScheduledTrafficTraceReplay {
    pub(super) fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub(super) fn errors(&self) -> TrafficTraceReplayControllerParallelErrors {
        self.executor.errors()
    }

    pub(super) fn summary(&self) -> WorkloadTrafficTraceReplaySummary {
        let runtime = self
            .executor
            .runtime()
            .lock()
            .expect("traffic trace replay runtime lock")
            .clone();
        let sideband_counts = traffic_trace_replay_sideband_counts(runtime.sideband_events());
        let trace_data_cache_response_count = self
            .records
            .memory_response_snapshot()
            .iter()
            .filter(|record| record.data_cache_response_applied())
            .count();
        let trace_data_cache_error_count = self.records.trace_error_snapshot().len();
        WorkloadTrafficTraceReplaySummary::new(self.route.clone(), self.scheduled_count)
            .with_response_delivery_count(
                self.response_deliveries
                    .lock()
                    .expect("traffic trace replay response lock")
                    .len(),
            )
            .with_memory_trace_event_count(self.trace.snapshot().len())
            .with_memory_write_completion_count(runtime.memory_write_completions().len())
            .with_trace_data_cache_response_count(trace_data_cache_response_count)
            .with_trace_data_cache_error_count(trace_data_cache_error_count)
            .with_memory_failure_count(runtime.memory_failures().len())
            .with_trace_error_count(trace_data_cache_error_count)
            .with_trace_htm_access_count(self.records.trace_htm_access_snapshot().len())
            .with_control_ack_count(runtime.control_acks().len())
            .with_control_failure_count(runtime.control_failures().len())
            .with_sideband_event_count(runtime.sideband_events().len())
            .with_tlb_sync_event_count(sideband_counts.tlb_sync)
            .with_trace_tlb_sync_count(self.records.trace_tlb_sync_snapshot().len())
            .with_cache_flush_event_count(sideband_counts.cache_flush)
            .with_trace_cache_flush_count(self.records.trace_cache_flush_snapshot().len())
            .with_trace_l1_invalidation_count(self.records.trace_l1_invalidation_snapshot().len())
            .with_diagnostic_print_event_count(sideband_counts.diagnostic_print)
            .with_trace_diagnostic_count(self.records.trace_diagnostic_snapshot().len())
            .with_htm_abort_event_count(sideband_counts.htm_abort)
    }

    pub(super) fn into_outcome(self) -> RiscvWorkloadTrafficTraceReplayOutcome {
        RiscvWorkloadTrafficTraceReplayOutcome {
            route: self.route,
            scheduled_count: self.scheduled_count,
            runtime: self
                .executor
                .runtime()
                .lock()
                .expect("traffic trace replay runtime lock")
                .clone(),
            errors: self.executor.errors(),
            response_deliveries: self
                .response_deliveries
                .lock()
                .expect("traffic trace replay response lock")
                .clone(),
            memory_trace_events: self.trace.snapshot(),
            memory_response_records: self.records.memory_response_snapshot(),
            memory_write_completion_records: self.records.memory_write_completion_snapshot(),
            memory_failure_records: self.records.memory_failure_snapshot(),
            trace_tlb_sync_records: self.records.trace_tlb_sync_snapshot(),
            trace_cache_flush_records: self.records.trace_cache_flush_snapshot(),
            trace_l1_invalidation_records: self.records.trace_l1_invalidation_snapshot(),
            trace_error_records: self.records.trace_error_snapshot(),
            trace_htm_access_records: self.records.trace_htm_access_snapshot(),
            trace_diagnostic_records: self.records.trace_diagnostic_snapshot(),
            sync_records: self.records.sync_snapshot(),
            htm_begin_records: self.records.htm_begin_snapshot(),
            htm_abort_records: self.records.htm_abort_snapshot(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TrafficTraceReplaySidebandCounts {
    tlb_sync: usize,
    cache_flush: usize,
    diagnostic_print: usize,
    htm_abort: usize,
}

fn traffic_trace_replay_sideband_counts(
    events: &[TrafficTraceReplayScheduledSidebandEvent],
) -> TrafficTraceReplaySidebandCounts {
    events.iter().fold(
        TrafficTraceReplaySidebandCounts::default(),
        |mut counts, event| {
            match event.event() {
                TrafficTraceReplaySidebandEvent::Tlb(event) => match event.kind() {
                    TrafficTraceTlbKind::ExternalSync => counts.tlb_sync += 1,
                },
                TrafficTraceReplaySidebandEvent::Cache(event) => match event.kind() {
                    TrafficTraceCacheKind::Flush => counts.cache_flush += 1,
                },
                TrafficTraceReplaySidebandEvent::Diagnostic(event) => match event.kind() {
                    TrafficTraceDiagnosticKind::Print => counts.diagnostic_print += 1,
                },
                TrafficTraceReplaySidebandEvent::Htm(event) => match event.kind() {
                    TrafficTraceHtmKind::Request => {}
                    TrafficTraceHtmKind::Abort => counts.htm_abort += 1,
                },
            }
            counts
        },
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTrafficTraceReplayOutcome {
    route: WorkloadRouteId,
    scheduled_count: usize,
    runtime: TrafficTraceReplayControllerRuntime,
    errors: TrafficTraceReplayControllerParallelErrors,
    response_deliveries: Vec<ResponseDelivery>,
    memory_trace_events: Vec<MemoryTraceEvent>,
    memory_response_records: Vec<RiscvWorkloadTraceMemoryResponseRecord>,
    memory_write_completion_records: Vec<RiscvWorkloadTraceMemoryWriteCompletionRecord>,
    memory_failure_records: Vec<RiscvWorkloadTraceMemoryFailureRecord>,
    trace_tlb_sync_records: Vec<RiscvWorkloadTraceTlbSyncRecord>,
    trace_cache_flush_records: Vec<RiscvWorkloadTraceCacheFlushRecord>,
    trace_l1_invalidation_records: Vec<RiscvWorkloadTraceL1InvalidationRecord>,
    trace_error_records: Vec<RiscvTraceErrorRecord>,
    trace_htm_access_records: Vec<RiscvTraceHtmAccessRecord>,
    trace_diagnostic_records: Vec<RiscvTraceDiagnosticRecord>,
    sync_records: Vec<RiscvWorkloadTraceSyncRecord>,
    htm_begin_records: Vec<RiscvWorkloadTraceHtmBeginRecord>,
    htm_abort_records: Vec<RiscvWorkloadTraceHtmAbortRecord>,
}

impl RiscvWorkloadTrafficTraceReplayOutcome {
    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    pub const fn runtime(&self) -> &TrafficTraceReplayControllerRuntime {
        &self.runtime
    }

    pub const fn errors(&self) -> &TrafficTraceReplayControllerParallelErrors {
        &self.errors
    }

    pub fn response_deliveries(&self) -> &[ResponseDelivery] {
        &self.response_deliveries
    }

    pub fn memory_trace_events(&self) -> &[MemoryTraceEvent] {
        &self.memory_trace_events
    }

    pub fn memory_response_records(&self) -> &[RiscvWorkloadTraceMemoryResponseRecord] {
        &self.memory_response_records
    }

    pub fn memory_write_completion_records(
        &self,
    ) -> &[RiscvWorkloadTraceMemoryWriteCompletionRecord] {
        &self.memory_write_completion_records
    }

    pub fn memory_failure_records(&self) -> &[RiscvWorkloadTraceMemoryFailureRecord] {
        &self.memory_failure_records
    }

    pub fn trace_tlb_sync_records(&self) -> &[RiscvWorkloadTraceTlbSyncRecord] {
        &self.trace_tlb_sync_records
    }

    pub fn trace_cache_flush_records(&self) -> &[RiscvWorkloadTraceCacheFlushRecord] {
        &self.trace_cache_flush_records
    }

    pub fn trace_l1_invalidation_records(&self) -> &[RiscvWorkloadTraceL1InvalidationRecord] {
        &self.trace_l1_invalidation_records
    }

    pub fn trace_error_records(&self) -> &[RiscvTraceErrorRecord] {
        &self.trace_error_records
    }

    pub fn trace_htm_access_records(&self) -> &[RiscvTraceHtmAccessRecord] {
        &self.trace_htm_access_records
    }

    pub fn trace_diagnostic_records(&self) -> &[RiscvTraceDiagnosticRecord] {
        &self.trace_diagnostic_records
    }

    pub fn sync_records(&self) -> &[RiscvWorkloadTraceSyncRecord] {
        &self.sync_records
    }

    pub fn htm_begin_records(&self) -> &[RiscvWorkloadTraceHtmBeginRecord] {
        &self.htm_begin_records
    }

    pub fn htm_abort_records(&self) -> &[RiscvWorkloadTraceHtmAbortRecord] {
        &self.htm_abort_records
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceMemoryResponseRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    request_id: MemoryRequestId,
    kind: TrafficTraceResponseKind,
    status: ResponseStatus,
    address: Option<Address>,
    line: Address,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
    response_data_bytes: Option<u64>,
    trace_data_bytes: Option<u64>,
    data_cache_response_applied: bool,
}

impl RiscvWorkloadTraceMemoryResponseRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn from_trace_response(
        tick: Tick,
        request_id: MemoryRequestId,
        event: TrafficTraceResponseEvent,
        request_line: Address,
        status: ResponseStatus,
        response_data_bytes: Option<u64>,
        trace_data_bytes: Option<u64>,
        data_cache_response_applied: bool,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            request_id,
            kind: event.kind(),
            status,
            address: event.address().or(Some(request_line)),
            line: request_line,
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
            response_data_bytes,
            trace_data_bytes,
            data_cache_response_applied,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn kind(&self) -> TrafficTraceResponseKind {
        self.kind
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }

    pub const fn response_data_bytes(&self) -> Option<u64> {
        self.response_data_bytes
    }

    pub const fn trace_data_bytes(&self) -> Option<u64> {
        self.trace_data_bytes
    }

    pub const fn data_cache_response_applied(&self) -> bool {
        self.data_cache_response_applied
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceMemoryWriteCompletionRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    request_id: MemoryRequestId,
    kind: TrafficTraceResponseKind,
    address: Option<Address>,
    line: Address,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl RiscvWorkloadTraceMemoryWriteCompletionRecord {
    pub fn from_memory_write_completion(
        tick: Tick,
        record: TrafficTraceMemoryWriteCompletionRecord,
    ) -> Self {
        let response = record.response();
        Self {
            tick,
            trace_tick: response.tick(),
            sequence: response.sequence(),
            request_id: record.request_id(),
            kind: response.kind(),
            address: response.address().or(Some(record.request_line())),
            line: record.request_line(),
            size_bytes: response.size_bytes(),
            trace_packet_id: response.trace_packet_id(),
            trace_pc: response.trace_pc(),
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn kind(&self) -> TrafficTraceResponseKind {
        self.kind
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceMemoryFailureRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    request_id: MemoryRequestId,
    error: TrafficTraceErrorKind,
    address: Option<Address>,
    line: Address,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl RiscvWorkloadTraceMemoryFailureRecord {
    pub fn from_trace_error(
        tick: Tick,
        request_id: MemoryRequestId,
        event: TrafficTraceErrorEvent,
        request_line: Address,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            request_id,
            error: event.kind(),
            address: event.address().or(Some(request_line)),
            line: request_line,
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn error(&self) -> TrafficTraceErrorKind {
        self.error
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceHtmBeginRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
    cluster_outcome: RiscvClusterHtmBeginOutcome,
}

impl RiscvWorkloadTraceHtmBeginRecord {
    fn new(
        tick: Tick,
        event: rem6_traffic::TrafficTraceHtmEvent,
        cluster_outcome: RiscvClusterHtmBeginOutcome,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            address: event.address(),
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
            cluster_outcome,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }

    pub const fn cluster_outcome(&self) -> &RiscvClusterHtmBeginOutcome {
        &self.cluster_outcome
    }

    pub const fn begin_uid(&self) -> Option<HtmTransactionUid> {
        match &self.cluster_outcome {
            RiscvClusterHtmBeginOutcome::Begun { begin, .. } => Some(begin.uid()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceHtmAbortRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
    cluster_outcome: RiscvClusterHtmAbortOutcome,
}

impl RiscvWorkloadTraceHtmAbortRecord {
    fn new(
        tick: Tick,
        event: rem6_traffic::TrafficTraceHtmEvent,
        cluster_outcome: RiscvClusterHtmAbortOutcome,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            address: event.address(),
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
            cluster_outcome,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }

    pub const fn cluster_outcome(&self) -> &RiscvClusterHtmAbortOutcome {
        &self.cluster_outcome
    }
}

pub(super) fn schedule_traffic_trace_replays(
    replays: &[RiscvWorkloadTrafficTraceReplay],
    topology: &WorkloadTopology,
    route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    cluster: &RiscvCluster,
) -> Result<Vec<RiscvWorkloadScheduledTrafficTraceReplay>, RiscvWorkloadReplayError> {
    let mut scheduled_replays = Vec::new();
    for replay in replays {
        let route = route_map.get(replay.route()).copied().ok_or_else(|| {
            RiscvWorkloadReplayError::MissingRoute {
                route: replay.route().clone(),
            }
        })?;
        let trace = MemoryTrace::new();
        let response_deliveries = Arc::new(Mutex::new(Vec::new()));
        let records = RiscvWorkloadTraceReplayRecords::default();
        let mut controller = replay.controller().clone();
        let start_batch = controller
            .start(scheduler.now())
            .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
        let data_cache_consumer = trace_data_cache_consumer(
            replay.route(),
            route,
            topology,
            data_cache,
            cluster,
            records.clone(),
        );
        let executor =
            traffic_trace_replay_executor(controller, replay.retry_delay(), data_cache_consumer);

        let mut scheduled_count = schedule_workload_traffic_trace_batch(
            &executor,
            &start_batch,
            scheduler,
            transport,
            route,
            trace.clone(),
            replay.control_partition(),
            Arc::clone(&response_deliveries),
        )?;
        let response_log = Arc::clone(&response_deliveries);
        scheduled_count += executor
            .schedule_controller_parallel(
                scheduler,
                transport,
                route,
                trace.clone(),
                replay.control_partition(),
                move |delivery| {
                    response_log
                        .lock()
                        .expect("traffic trace replay response lock")
                        .push(delivery);
                },
            )
            .map_err(RiscvWorkloadReplayError::TrafficTraceReplay)?;
        scheduled_replays.push(RiscvWorkloadScheduledTrafficTraceReplay {
            route: replay.route().clone(),
            scheduled_count,
            trace,
            response_deliveries,
            records,
            executor,
        });
    }
    Ok(scheduled_replays)
}

fn traffic_trace_replay_executor(
    controller: TrafficController,
    retry_delay: Tick,
    data_cache: WorkloadTraceDataCacheConsumer,
) -> TrafficTraceReplayControllerParallelExecutor {
    let executor =
        TrafficTraceReplayControllerParallelExecutor::new(controller).with_retry_delay(retry_delay);
    executor
        .with_target_request_sink({
            let data_cache = data_cache.clone();
            move |order, request| {
                data_cache.register_request(order, request);
            }
        })
        .with_target_event_sink({
            let data_cache = data_cache.clone();
            move |order, delivery, event_context| {
                data_cache.register_target_event(order, delivery.tick(), event_context);
            }
        })
        .with_target_completion_sink({
            let data_cache = data_cache.clone();
            move |order, delivery, event_context| {
                data_cache.complete_target_event(order, delivery, event_context);
            }
        })
        .with_memory_write_completion_sink({
            let data_cache = data_cache.clone();
            move |tick, record| {
                data_cache.record_memory_write_completion(tick, record);
            }
        })
        .with_sideband_sink({
            let data_cache = data_cache.clone();
            move |tick, event| {
                data_cache.record_sideband(tick, event);
            }
        })
        .with_control_event_sink({
            let data_cache = data_cache.clone();
            move |tick, event_context| {
                data_cache.register_control_event(tick, event_context);
            }
        })
        .with_control_completion_sink(move |tick, event_context| {
            data_cache.complete_control_event(tick, event_context);
        })
}

fn trace_data_cache_consumer(
    route: &WorkloadRouteId,
    memory_route: MemoryRouteId,
    topology: &WorkloadTopology,
    data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    cluster: &RiscvCluster,
    records: RiscvWorkloadTraceReplayRecords,
) -> WorkloadTraceDataCacheConsumer {
    let data_cache = if trace_route_uses_data_cache(route, topology) {
        data_cache.clone()
    } else {
        None
    };
    let data_translation_cluster =
        trace_route_uses_data_translation(route, topology).then(|| cluster.clone());
    let htm_cluster = trace_route_uses_riscv_data_core(route, topology).then(|| cluster.clone());
    WorkloadTraceDataCacheConsumer::new(
        memory_route,
        data_cache,
        data_translation_cluster,
        htm_cluster,
        records,
    )
}

fn trace_route_uses_data_cache(route: &WorkloadRouteId, topology: &WorkloadTopology) -> bool {
    if topology.riscv_data_cache().is_none() {
        return false;
    }
    topology
        .riscv_cores()
        .iter()
        .filter_map(|core| core.data_route())
        .any(|data_route| data_route == route)
        || topology
            .gpu_dma_copies()
            .iter()
            .any(|copy| copy.route() == route)
        || topology
            .accelerator_dma_copies()
            .iter()
            .any(|copy| copy.route() == route)
}

fn trace_route_uses_data_translation(route: &WorkloadRouteId, topology: &WorkloadTopology) -> bool {
    topology.riscv_cores().iter().any(|core| {
        core.data_translation().is_some()
            && core
                .data_route()
                .is_some_and(|data_route| data_route == route)
    })
}

fn trace_route_uses_riscv_data_core(route: &WorkloadRouteId, topology: &WorkloadTopology) -> bool {
    topology.riscv_cores().iter().any(|core| {
        core.data_route()
            .is_some_and(|data_route| data_route == route)
    })
}

#[derive(Clone)]
struct WorkloadTraceDataCacheConsumer {
    inner: Arc<Mutex<WorkloadTraceDataCacheConsumerInner>>,
}

struct WorkloadTraceDataCacheControl {
    tick: Tick,
    order: TrafficTraceReplayOrder,
    event_context: TrafficTraceReplayControlEventContext,
}

#[derive(Clone, Copy)]
enum WorkloadTraceDataCachePendingEventIndex {
    Sideband(usize),
    Control(usize),
}

impl WorkloadTraceDataCacheConsumer {
    fn new(
        route: MemoryRouteId,
        data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
        data_translation_cluster: Option<RiscvCluster>,
        htm_cluster: Option<RiscvCluster>,
        records: RiscvWorkloadTraceReplayRecords,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(WorkloadTraceDataCacheConsumerInner {
                route,
                data_cache,
                data_translation_cluster,
                htm_cluster,
                records,
                active_htm_transactions: Vec::new(),
                pending_requests: BTreeSet::new(),
                pending_control_orders: BTreeSet::new(),
                pending_sidebands: Vec::new(),
                pending_controls: Vec::new(),
            })),
        }
    }

    fn register_request(&self, order: TrafficTraceReplayOrder, _request: MemoryRequestId) {
        self.inner
            .lock()
            .expect("workload trace data cache consumer lock")
            .pending_requests
            .insert(order);
    }

    fn register_target_event(
        &self,
        request_order: TrafficTraceReplayOrder,
        now: Tick,
        event_context: &TrafficTraceReplayTargetEventContext,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner.replace_pending_request_order(
            request_order,
            target_event_order(request_order, event_context),
            now,
        );
    }

    fn complete_target_event(
        &self,
        request_order: TrafficTraceReplayOrder,
        delivery: &RequestDelivery,
        event_context: &TrafficTraceReplayTargetEventContext,
    ) {
        let completion_order = target_event_order(request_order, event_context);
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner.apply_events_before(completion_order);
        match event_context.event() {
            TrafficTraceReplayTargetEvent::MemoryResponse(_) => {
                let data_cache_response_status =
                    target_event_response_status(event_context.event());
                let response_data_bytes = target_event_response_data_bytes(event_context.event());
                let trace_response_data_bytes = event_context
                    .trace_response_data()
                    .map(|data| data.len() as u64);
                let target_completion_tick =
                    target_completion_tick(delivery.tick(), event_context.event());
                let mut data_cache_response_applied = false;
                if data_cache_response_status.is_some()
                    && data_cache_response_status != Some(ResponseStatus::StoreConditionalFailed)
                {
                    if let Some(data_cache) = inner.data_cache.as_ref() {
                        let cache_outcome = data_cache
                            .lock()
                            .expect("workload data cache lock")
                            .respond(delivery);
                        data_cache_response_applied = cache_outcome
                            .as_ref()
                            .and_then(target_outcome_response_status)
                            == Some(ResponseStatus::Completed);
                    }
                }
                if let Some(response) = event_context.trace_response() {
                    let response_record =
                        RiscvWorkloadTraceMemoryResponseRecord::from_trace_response(
                            target_completion_tick,
                            delivery.request().id(),
                            response,
                            delivery.request().line_address(),
                            data_cache_response_status.expect(
                                "trace response target event carries memory response status",
                            ),
                            response_data_bytes,
                            trace_response_data_bytes,
                            data_cache_response_applied,
                        );
                    inner.apply_trace_response(response_record, response);
                }
            }
            TrafficTraceReplayTargetEvent::MemoryFailure { record, .. } => {
                if let Some(error) = event_context.trace_error() {
                    let record = *record;
                    inner.apply_trace_error(
                        record.tick(),
                        record.failure().request_id(),
                        error,
                        Some(delivery.request().line_address()),
                    );
                }
            }
        }
        inner.pending_requests.remove(&completion_order);
        inner.apply_ready_events(completion_order.tick().max(delivery.tick()));
    }

    fn record_memory_write_completion(
        &self,
        tick: Tick,
        record: TrafficTraceMemoryWriteCompletionRecord,
    ) {
        let inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner.records.record_memory_write_completion(
            RiscvWorkloadTraceMemoryWriteCompletionRecord::from_memory_write_completion(
                tick, record,
            ),
        );
    }

    fn record_sideband(&self, tick: Tick, event: TrafficTraceReplaySidebandEvent) {
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner
            .pending_sidebands
            .push(WorkloadTraceDataCacheSideband {
                tick,
                order: TrafficTraceReplayOrder::new(event.tick(), event.sequence()),
                event,
            });
        inner.apply_ready_events(tick);
    }

    fn register_control_event(
        &self,
        now: Tick,
        event_context: &TrafficTraceReplayControlEventContext,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner
            .pending_control_orders
            .insert(control_event_order(event_context));
        inner.apply_ready_events(now);
    }

    fn complete_control_event(
        &self,
        tick: Tick,
        event_context: &TrafficTraceReplayControlEventContext,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner.pending_controls.push(WorkloadTraceDataCacheControl {
            tick,
            order: control_event_order(event_context),
            event_context: event_context.clone(),
        });
        inner.apply_ready_events(tick);
    }
}

struct WorkloadTraceDataCacheConsumerInner {
    route: MemoryRouteId,
    data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    data_translation_cluster: Option<RiscvCluster>,
    htm_cluster: Option<RiscvCluster>,
    records: RiscvWorkloadTraceReplayRecords,
    active_htm_transactions: Vec<HtmTransactionUid>,
    pending_requests: BTreeSet<TrafficTraceReplayOrder>,
    pending_control_orders: BTreeSet<TrafficTraceReplayOrder>,
    pending_sidebands: Vec<WorkloadTraceDataCacheSideband>,
    pending_controls: Vec<WorkloadTraceDataCacheControl>,
}

impl WorkloadTraceDataCacheConsumerInner {
    fn replace_pending_request_order(
        &mut self,
        request_order: TrafficTraceReplayOrder,
        completion_order: TrafficTraceReplayOrder,
        now: Tick,
    ) {
        if self.pending_requests.remove(&request_order) {
            self.pending_requests.insert(completion_order);
        }
        self.apply_ready_events(now);
    }

    fn apply_events_before(&mut self, request_order: TrafficTraceReplayOrder) {
        while let Some((order, index)) =
            self.next_pending_event_index(|order, _tick| order <= request_order)
        {
            if self.is_blocked(order) {
                return;
            }
            self.apply_pending_event(index);
        }
    }

    fn apply_ready_events(&mut self, now: Tick) {
        while let Some((order, index)) = self.next_pending_event_index(|_order, tick| tick <= now) {
            if self.is_blocked(order) {
                return;
            }
            self.apply_pending_event(index);
        }
    }

    fn next_pending_event_index(
        &self,
        ready: impl Fn(TrafficTraceReplayOrder, Tick) -> bool,
    ) -> Option<(
        TrafficTraceReplayOrder,
        WorkloadTraceDataCachePendingEventIndex,
    )> {
        let sidebands = self
            .pending_sidebands
            .iter()
            .enumerate()
            .filter(|(_, sideband)| ready(sideband.order, sideband.tick))
            .map(|(index, sideband)| {
                (
                    sideband.order,
                    WorkloadTraceDataCachePendingEventIndex::Sideband(index),
                )
            });
        let controls = self
            .pending_controls
            .iter()
            .enumerate()
            .filter(|(_, control)| ready(control.order, control.tick))
            .map(|(index, control)| {
                (
                    control.order,
                    WorkloadTraceDataCachePendingEventIndex::Control(index),
                )
            });
        sidebands.chain(controls).min_by_key(|(order, _)| *order)
    }

    fn is_blocked(&self, order: TrafficTraceReplayOrder) -> bool {
        self.pending_requests
            .iter()
            .next()
            .is_some_and(|request_order| *request_order < order)
            || self
                .pending_control_orders
                .iter()
                .next()
                .is_some_and(|control_order| *control_order < order)
    }

    fn apply_pending_event(&mut self, index: WorkloadTraceDataCachePendingEventIndex) {
        match index {
            WorkloadTraceDataCachePendingEventIndex::Sideband(index) => {
                let sideband = self.pending_sidebands.remove(index);
                self.apply_sideband(sideband.tick, sideband.event);
            }
            WorkloadTraceDataCachePendingEventIndex::Control(index) => {
                let control = self.pending_controls.remove(index);
                self.pending_control_orders.remove(&control.order);
                self.apply_control_event(control.tick, &control.event_context);
            }
        }
    }

    fn apply_sideband(&mut self, tick: Tick, event: TrafficTraceReplaySidebandEvent) {
        match event {
            TrafficTraceReplaySidebandEvent::Cache(cache) => {
                if let Some(data_cache) = self.data_cache.as_ref() {
                    if let Some(application) = data_cache
                        .lock()
                        .expect("workload data cache lock")
                        .apply_trace_cache_event(cache)
                    {
                        self.records.record_trace_cache_flush(
                            RiscvWorkloadTraceCacheFlushRecord::from_trace_cache_event(
                                tick,
                                cache,
                                application,
                            ),
                        );
                    }
                }
            }
            TrafficTraceReplaySidebandEvent::Diagnostic(diagnostic) => {
                if let Some(data_cache) = self.data_cache.as_ref() {
                    if let Some(record) = data_cache
                        .lock()
                        .expect("workload data cache lock")
                        .apply_trace_diagnostic_event(tick, diagnostic)
                    {
                        self.records.record_trace_diagnostic(record);
                    }
                }
            }
            TrafficTraceReplaySidebandEvent::Tlb(tlb) => {
                if matches!(tlb.kind(), TrafficTraceTlbKind::ExternalSync) {
                    if let Some(cluster) = self.data_translation_cluster.as_ref() {
                        if let Some(flushed_entry_count) =
                            cluster.flush_data_translation_tlbs_for_data_route(self.route)
                        {
                            self.records.record_trace_tlb_sync(
                                RiscvWorkloadTraceTlbSyncRecord::from_trace_tlb_event(
                                    tick,
                                    tlb,
                                    flushed_entry_count,
                                ),
                            );
                        }
                    }
                }
            }
            TrafficTraceReplaySidebandEvent::Htm(htm) => {
                if matches!(htm.kind(), TrafficTraceHtmKind::Abort) {
                    let active_transaction_uid = self.active_htm_transactions.last().copied();
                    let cause = active_transaction_uid
                        .and_then(|transaction_uid| {
                            self.data_cache.as_ref().map(|data_cache| {
                                data_cache
                                    .lock()
                                    .expect("workload data cache lock")
                                    .trace_htm_abort_cause(self.route, transaction_uid)
                            })
                        })
                        .unwrap_or(HtmFailureCause::Other);
                    let cluster_outcome = self.htm_cluster.as_ref().map_or(
                        RiscvClusterHtmAbortOutcome::NoMatchingDataRoute { route: self.route },
                        |cluster| cluster.abort_htm_transaction_for_data_route(self.route, cause),
                    );
                    if let RiscvClusterHtmAbortOutcome::Aborted { abort, .. } = &cluster_outcome {
                        if let Some(data_cache) = self.data_cache.as_ref() {
                            data_cache
                                .lock()
                                .expect("workload data cache lock")
                                .restore_trace_htm_rollback(self.route, abort.uid());
                        }
                    } else if let Some(transaction_uid) = active_transaction_uid {
                        if let Some(data_cache) = self.data_cache.as_ref() {
                            data_cache
                                .lock()
                                .expect("workload data cache lock")
                                .discard_trace_htm_transaction(self.route, transaction_uid);
                        }
                    }
                    clear_htm_transactions_after_trace_abort(
                        &mut self.active_htm_transactions,
                        &cluster_outcome,
                    );
                    self.records
                        .record_htm_abort(RiscvWorkloadTraceHtmAbortRecord::new(
                            tick,
                            htm,
                            cluster_outcome,
                        ));
                }
            }
        }
    }

    fn apply_control_event(
        &mut self,
        tick: Tick,
        event_context: &TrafficTraceReplayControlEventContext,
    ) {
        if let Some(sync) = event_context.trace_sync() {
            let trace_order = event_context.trace_order();
            match event_context.event() {
                TrafficTraceReplayControlEvent::ControlAck { .. } => {
                    if sync.invalidates_l1() {
                        if let Some(data_cache) = self.data_cache.as_ref() {
                            let invalidated_line_count = data_cache
                                .lock()
                                .expect("workload data cache lock")
                                .invalidate_trace_l1_from_sync(tick, sync);
                            self.records.record_trace_l1_invalidation(
                                RiscvWorkloadTraceL1InvalidationRecord::from_trace_sync_invalidation(
                                    tick,
                                    sync,
                                    trace_order,
                                    invalidated_line_count,
                                ),
                            );
                        }
                    }
                    self.records.record_sync(RiscvWorkloadTraceSyncRecord::ack(
                        tick,
                        sync,
                        trace_order,
                    ));
                }
                TrafficTraceReplayControlEvent::ControlFailure { record, .. } => {
                    self.records
                        .record_sync(RiscvWorkloadTraceSyncRecord::failure(
                            tick,
                            sync,
                            trace_order,
                            record.failure().error(),
                        ));
                }
            }
            return;
        }
        if !matches!(
            event_context.event(),
            TrafficTraceReplayControlEvent::ControlAck { .. }
        ) {
            return;
        }
        let Some(htm) = event_context.trace_htm() else {
            return;
        };
        if !matches!(htm.kind(), TrafficTraceHtmKind::Request) {
            return;
        }
        let cluster_outcome = self.htm_cluster.as_ref().map_or(
            RiscvClusterHtmBeginOutcome::NoMatchingDataRoute { route: self.route },
            |cluster| cluster.begin_htm_transaction_for_data_route(self.route),
        );
        if let RiscvClusterHtmBeginOutcome::Begun { begin, .. } = &cluster_outcome {
            self.active_htm_transactions.push(begin.uid());
            if let Some(data_cache) = self.data_cache.as_ref() {
                data_cache
                    .lock()
                    .expect("workload data cache lock")
                    .capture_trace_htm_rollback(self.route, begin.uid());
            }
        }
        self.records
            .record_htm_begin(RiscvWorkloadTraceHtmBeginRecord::new(
                tick,
                htm,
                cluster_outcome,
            ));
    }

    fn apply_trace_response(
        &mut self,
        record: RiscvWorkloadTraceMemoryResponseRecord,
        event: TrafficTraceResponseEvent,
    ) {
        self.records.record_memory_response(record);
        if let Some(data_cache) = self.data_cache.as_ref() {
            let mut data_cache = data_cache.lock().expect("workload data cache lock");
            if let Some(transaction_uid) = self.active_htm_transactions.last().copied() {
                for htm_access in data_cache.record_trace_htm_access_event(
                    record.tick(),
                    self.route,
                    transaction_uid,
                    event,
                    record.data_cache_response_applied(),
                ) {
                    self.records.record_trace_htm_access(htm_access);
                }
            } else {
                data_cache.record_trace_htm_write_conflict_event(
                    self.route,
                    None,
                    event,
                    record.data_cache_response_applied(),
                );
            }
            data_cache.apply_trace_response_event(event);
        }
    }

    fn apply_trace_error(
        &mut self,
        tick: Tick,
        request_id: MemoryRequestId,
        event: TrafficTraceErrorEvent,
        fallback_address: Option<Address>,
    ) {
        if let Some(request_line) = fallback_address {
            self.records.record_memory_failure(
                RiscvWorkloadTraceMemoryFailureRecord::from_trace_error(
                    tick,
                    request_id,
                    event,
                    request_line,
                ),
            );
        }
        if let Some(data_cache) = self.data_cache.as_ref() {
            if let Some(record) = data_cache
                .lock()
                .expect("workload data cache lock")
                .record_trace_error_event(tick, request_id, event, fallback_address)
            {
                self.records.record_trace_error(record);
            }
        }
    }
}

fn clear_htm_transactions_after_trace_abort(
    active: &mut Vec<HtmTransactionUid>,
    outcome: &RiscvClusterHtmAbortOutcome,
) {
    match outcome {
        RiscvClusterHtmAbortOutcome::Aborted { abort, .. } => {
            clear_aborted_htm_transaction(active, abort.uid());
        }
        RiscvClusterHtmAbortOutcome::NoMatchingDataRoute { .. }
        | RiscvClusterHtmAbortOutcome::NoActiveTransaction { .. }
        | RiscvClusterHtmAbortOutcome::Failed { .. } => active.clear(),
    }
}

fn clear_aborted_htm_transaction(active: &mut Vec<HtmTransactionUid>, uid: HtmTransactionUid) {
    match active.iter().position(|active_uid| *active_uid == uid) {
        Some(index) => active.truncate(index),
        None => active.clear(),
    }
}

struct WorkloadTraceDataCacheSideband {
    tick: Tick,
    order: TrafficTraceReplayOrder,
    event: TrafficTraceReplaySidebandEvent,
}

fn target_event_order(
    request_order: TrafficTraceReplayOrder,
    event_context: &TrafficTraceReplayTargetEventContext,
) -> TrafficTraceReplayOrder {
    if let Some(response) = event_context.trace_response() {
        return TrafficTraceReplayOrder::new(response.tick(), response.sequence());
    }
    if let Some(error) = event_context.trace_error() {
        return TrafficTraceReplayOrder::new(error.tick(), error.sequence());
    }
    request_order
}

fn control_event_order(
    event_context: &TrafficTraceReplayControlEventContext,
) -> TrafficTraceReplayOrder {
    event_context.trace_order()
}

fn target_event_response_status(event: &TrafficTraceReplayTargetEvent) -> Option<ResponseStatus> {
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => {
            target_outcome_response_status(outcome)
        }
        TrafficTraceReplayTargetEvent::MemoryFailure { .. } => None,
    }
}

fn target_event_response_data_bytes(event: &TrafficTraceReplayTargetEvent) -> Option<u64> {
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => {
            target_outcome_response_data_bytes(outcome)
        }
        TrafficTraceReplayTargetEvent::MemoryFailure { .. } => None,
    }
}

fn target_outcome_response_status(outcome: &TargetOutcome) -> Option<ResponseStatus> {
    match outcome {
        TargetOutcome::Respond(response) | TargetOutcome::RespondAfter { response, .. } => {
            Some(response.status())
        }
        TargetOutcome::NoResponse => None,
    }
}

fn target_outcome_response_data_bytes(outcome: &TargetOutcome) -> Option<u64> {
    match outcome {
        TargetOutcome::Respond(response) | TargetOutcome::RespondAfter { response, .. } => {
            response.data().map(|data| data.len() as u64)
        }
        TargetOutcome::NoResponse => None,
    }
}

fn target_completion_tick(delivery_tick: Tick, event: &TrafficTraceReplayTargetEvent) -> Tick {
    delivery_tick
        .checked_add(event.target_delay())
        .expect("validated trace replay target completion tick")
}

#[allow(clippy::too_many_arguments)]
fn schedule_workload_traffic_trace_batch(
    executor: &TrafficTraceReplayControllerParallelExecutor,
    batch: &TrafficControllerEventBatch,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    route: MemoryRouteId,
    trace: MemoryTrace,
    control_partition: PartitionId,
    response_deliveries: Arc<Mutex<Vec<ResponseDelivery>>>,
) -> Result<usize, RiscvWorkloadReplayError> {
    if batch.is_empty() {
        return Ok(0);
    }

    if batch.request().is_some() {
        return executor
            .submit_batch_request_parallel(
                batch,
                scheduler,
                transport,
                route,
                trace,
                move |delivery| {
                    response_deliveries
                        .lock()
                        .expect("traffic trace replay response lock")
                        .push(delivery);
                },
            )
            .map(usize::from)
            .map_err(RiscvWorkloadReplayError::TrafficTraceReplay);
    }

    if workload_traffic_trace_batch_requires_control_response(batch) {
        return executor
            .schedule_batch_control_parallel(batch, scheduler, control_partition)
            .map(usize::from)
            .map_err(RiscvWorkloadReplayError::TrafficTraceReplay);
    }

    executor
        .record_batch_parallel(
            batch,
            scheduler,
            control_partition,
            workload_traffic_trace_batch_replay_tick(batch),
        )
        .map_err(RiscvWorkloadReplayError::TrafficTraceReplay)
}

fn workload_traffic_trace_batch_requires_control_response(
    batch: &TrafficControllerEventBatch,
) -> bool {
    batch
        .trace_sync()
        .is_some_and(|sync| sync.requires_response())
        || batch.trace_htm().is_some_and(|htm| htm.requires_response())
}

fn workload_traffic_trace_batch_replay_tick(batch: &TrafficControllerEventBatch) -> Tick {
    batch
        .events()
        .iter()
        .map(workload_traffic_trace_event_tick)
        .min()
        .unwrap_or(0)
}

fn workload_traffic_trace_event_tick(event: &TrafficControllerEvent) -> Tick {
    match event {
        TrafficControllerEvent::Request(request) => request.tick(),
        TrafficControllerEvent::Transition(transition) => transition.tick(),
        TrafficControllerEvent::Exit(exit) => exit.tick(),
        TrafficControllerEvent::TraceExit(_) => Tick::MAX,
        TrafficControllerEvent::TraceSync(sync) => sync.tick(),
        TrafficControllerEvent::TraceTlb(tlb) => tlb.tick(),
        TrafficControllerEvent::TraceCache(cache) => cache.tick(),
        TrafficControllerEvent::TraceHtm(htm) => htm.tick(),
        TrafficControllerEvent::TraceDiagnostic(diagnostic) => diagnostic.tick(),
        TrafficControllerEvent::TraceResponse(response) => response.tick(),
        TrafficControllerEvent::TraceError(error) => error.tick(),
        TrafficControllerEvent::TraceResponseMatch(response) => response.response().tick(),
        TrafficControllerEvent::TraceErrorMatch(error) => error.error().tick(),
        TrafficControllerEvent::TraceReplayAction(action) => action.tick(),
    }
}

#[cfg(test)]
mod tests {
    use rem6_cpu::CpuId;

    use super::*;

    #[test]
    fn trace_abort_boundary_clears_active_htm_transaction_without_cpu_abort() {
        let mut active = vec![HtmTransactionUid::new(7)];

        clear_htm_transactions_after_trace_abort(
            &mut active,
            &RiscvClusterHtmAbortOutcome::NoActiveTransaction {
                cpu: CpuId::new(0),
                route: MemoryRouteId::new(3),
            },
        );

        assert!(active.is_empty());
    }
}
