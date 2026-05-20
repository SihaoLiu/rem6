use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, SchedulerContext, Tick};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, ResponseStatus,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, ResponseDelivery, TargetOutcome,
    TransportEndpointId, TransportError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CpuId(u32);

impl CpuId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuResetState {
    cpu: CpuId,
    partition: PartitionId,
    agent: AgentId,
    entry: Address,
}

impl CpuResetState {
    pub const fn new(cpu: CpuId, partition: PartitionId, agent: AgentId, entry: Address) -> Self {
        Self {
            cpu,
            partition,
            agent,
            entry,
        }
    }

    pub fn from_boot_image(
        cpu: CpuId,
        partition: PartitionId,
        agent: AgentId,
        image: &BootImage,
    ) -> Self {
        Self::new(cpu, partition, agent, image.entry())
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFetchConfig {
    endpoint: TransportEndpointId,
    route: MemoryRouteId,
    line_layout: CacheLineLayout,
    width: AccessSize,
}

impl CpuFetchConfig {
    pub const fn new(
        endpoint: TransportEndpointId,
        route: MemoryRouteId,
        line_layout: CacheLineLayout,
        width: AccessSize,
    ) -> Self {
        Self {
            endpoint,
            route,
            line_layout,
            width,
        }
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn width(&self) -> AccessSize {
        self.width
    }
}

#[derive(Clone)]
pub struct CpuCore {
    state: Arc<Mutex<CpuCoreState>>,
}

impl CpuCore {
    pub fn new(reset: CpuResetState, fetch: CpuFetchConfig) -> Result<Self, CpuError> {
        Ok(Self {
            state: Arc::new(Mutex::new(CpuCoreState::new(reset, fetch))),
        })
    }

    pub fn id(&self) -> CpuId {
        self.state.lock().expect("cpu core lock").reset.cpu()
    }

    pub fn partition(&self) -> PartitionId {
        self.state.lock().expect("cpu core lock").reset.partition()
    }

    pub fn agent(&self) -> AgentId {
        self.state.lock().expect("cpu core lock").reset.agent()
    }

    pub fn pc(&self) -> Address {
        self.state.lock().expect("cpu core lock").pc
    }

    pub fn fetch_endpoint(&self) -> TransportEndpointId {
        self.state
            .lock()
            .expect("cpu core lock")
            .fetch
            .endpoint()
            .clone()
    }

    pub fn fetch_route(&self) -> MemoryRouteId {
        self.state.lock().expect("cpu core lock").fetch.route()
    }

    pub fn next_sequence(&self) -> u64 {
        self.state.lock().expect("cpu core lock").next_sequence
    }

    pub fn fetch_events(&self) -> Vec<CpuFetchEvent> {
        self.state.lock().expect("cpu core lock").events.clone()
    }

    pub fn issue_next_fetch<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let issue = self.prepare_fetch(scheduler.now(), transport)?;
        let request = MemoryRequest::instruction_fetch(
            issue.request_id,
            issue.pc,
            issue.size,
            issue.line_layout,
        )
        .map_err(CpuError::Memory)?;

        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                issue.route,
                request,
                trace,
                responder,
                move |delivery| core.record_response(delivery),
            )
            .map_err(CpuError::Transport)?;

        self.record_issue(issue);
        Ok(event)
    }

    fn prepare_fetch(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<OutstandingFetch, CpuError> {
        let state = self.state.lock().expect("cpu core lock");
        let route = transport
            .route(state.fetch.route())
            .ok_or(CpuError::Transport(TransportError::UnknownRoute {
                route: state.fetch.route(),
            }))?;

        if route.source_partition() != state.reset.partition() {
            return Err(CpuError::RoutePartitionMismatch {
                route: state.fetch.route(),
                expected: state.reset.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source() != state.fetch.endpoint() {
            return Err(CpuError::RouteEndpointMismatch {
                route: state.fetch.route(),
                expected: state.fetch.endpoint().clone(),
                actual: route.source().clone(),
            });
        }

        let line_offset = state.fetch.line_layout().line_offset(state.pc);
        if line_offset + state.fetch.width().bytes() > state.fetch.line_layout().bytes() {
            return Err(CpuError::FetchCrossesLine {
                pc: state.pc,
                size: state.fetch.width(),
                line_size: state.fetch.line_layout().bytes(),
            });
        }

        Ok(OutstandingFetch {
            tick,
            partition: state.reset.partition(),
            route: state.fetch.route(),
            endpoint: state.fetch.endpoint().clone(),
            request_id: MemoryRequestId::new(state.reset.agent(), state.next_sequence),
            pc: state.pc,
            size: state.fetch.width(),
            line_layout: state.fetch.line_layout(),
        })
    }

    fn record_issue(&self, issue: OutstandingFetch) {
        let mut state = self.state.lock().expect("cpu core lock");
        state.next_sequence += 1;
        state
            .outstanding
            .insert(issue.request_id, issue.clone_without_layout());
        state.events.push(CpuFetchEvent::issued(CpuFetchRecord::new(
            issue.tick,
            issue.partition,
            issue.route,
            issue.endpoint,
            issue.request_id,
            issue.pc,
            issue.size,
        )));
    }

    fn record_response(&self, delivery: ResponseDelivery) {
        let mut state = self.state.lock().expect("cpu core lock");
        let Some(fetch) = state.outstanding.remove(&delivery.response().request_id()) else {
            return;
        };

        match delivery.response().status() {
            ResponseStatus::Completed => {
                let data = delivery.response().data().unwrap_or_default().to_vec();
                if let Some(next_pc) = fetch.pc.get().checked_add(data.len() as u64) {
                    state.pc = Address::new(next_pc);
                }
                state.events.push(CpuFetchEvent::completed(
                    fetch.record(
                        delivery.tick(),
                        delivery.route(),
                        delivery.endpoint().clone(),
                    ),
                    data,
                ));
            }
            ResponseStatus::Retry => {
                state.events.push(CpuFetchEvent::retry(fetch.record(
                    delivery.tick(),
                    delivery.route(),
                    delivery.endpoint().clone(),
                )));
            }
        }
    }
}

impl fmt::Debug for CpuCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.lock().expect("cpu core lock");
        formatter
            .debug_struct("CpuCore")
            .field("cpu", &state.reset.cpu())
            .field("partition", &state.reset.partition())
            .field("pc", &state.pc)
            .field("next_sequence", &state.next_sequence)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct CpuCluster {
    cores: BTreeMap<CpuId, CpuCore>,
}

impl CpuCluster {
    pub fn new<I>(cores: I) -> Result<Self, CpuClusterError>
    where
        I: IntoIterator<Item = CpuCore>,
    {
        let mut by_cpu = BTreeMap::new();
        let mut by_agent = BTreeMap::new();
        let mut by_endpoint = BTreeMap::new();

        for core in cores {
            let cpu = core.id();
            if by_cpu.contains_key(&cpu) {
                return Err(CpuClusterError::DuplicateCpu { cpu });
            }

            let agent = core.agent();
            if let Some(existing) = by_agent.insert(agent, cpu) {
                return Err(CpuClusterError::DuplicateAgent {
                    agent,
                    existing,
                    duplicate: cpu,
                });
            }

            let endpoint = core.fetch_endpoint();
            if let Some(existing) = by_endpoint.insert(endpoint.clone(), cpu) {
                return Err(CpuClusterError::DuplicateFetchEndpoint {
                    endpoint,
                    existing,
                    duplicate: cpu,
                });
            }

            by_cpu.insert(cpu, core);
        }

        Ok(Self { cores: by_cpu })
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }

    pub fn core_ids(&self) -> Vec<CpuId> {
        self.cores.keys().copied().collect()
    }

    pub fn core(&self, cpu: CpuId) -> Result<CpuCore, CpuClusterError> {
        self.cores
            .get(&cpu)
            .cloned()
            .ok_or(CpuClusterError::UnknownCpu { cpu })
    }

    pub fn issue_next_fetch<F>(
        &self,
        cpu: CpuId,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuClusterError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.core(cpu)?
            .issue_next_fetch(scheduler, transport, trace, responder)
            .map_err(CpuClusterError::Cpu)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CpuCoreState {
    reset: CpuResetState,
    fetch: CpuFetchConfig,
    pc: Address,
    next_sequence: u64,
    outstanding: BTreeMap<MemoryRequestId, IssuedFetch>,
    events: Vec<CpuFetchEvent>,
}

impl CpuCoreState {
    fn new(reset: CpuResetState, fetch: CpuFetchConfig) -> Self {
        let pc = reset.entry();
        Self {
            reset,
            fetch,
            pc,
            next_sequence: 0,
            outstanding: BTreeMap::new(),
            events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutstandingFetch {
    tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request_id: MemoryRequestId,
    pc: Address,
    size: AccessSize,
    line_layout: CacheLineLayout,
}

impl OutstandingFetch {
    fn clone_without_layout(&self) -> IssuedFetch {
        IssuedFetch {
            partition: self.partition,
            request: self.request_id,
            pc: self.pc,
            size: self.size,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IssuedFetch {
    partition: PartitionId,
    request: MemoryRequestId,
    pc: Address,
    size: AccessSize,
}

impl IssuedFetch {
    fn record(
        &self,
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
    ) -> CpuFetchRecord {
        CpuFetchRecord::new(
            tick,
            self.partition,
            route,
            endpoint,
            self.request,
            self.pc,
            self.size,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuFetchEventKind {
    Issued,
    Completed,
    Retry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFetchRecord {
    tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request: MemoryRequestId,
    pc: Address,
    size: AccessSize,
}

impl CpuFetchRecord {
    pub fn new(
        tick: Tick,
        partition: PartitionId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        request: MemoryRequestId,
        pc: Address,
        size: AccessSize,
    ) -> Self {
        Self {
            tick,
            partition,
            route,
            endpoint,
            request,
            pc,
            size,
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.request
    }

    pub fn pc(&self) -> Address {
        self.pc
    }

    pub fn size(&self) -> AccessSize {
        self.size
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFetchEvent {
    record: CpuFetchRecord,
    kind: CpuFetchEventKind,
    data: Option<Vec<u8>>,
}

impl CpuFetchEvent {
    pub fn issued(record: CpuFetchRecord) -> Self {
        Self {
            record,
            kind: CpuFetchEventKind::Issued,
            data: None,
        }
    }

    pub fn completed(record: CpuFetchRecord, data: Vec<u8>) -> Self {
        Self {
            record,
            kind: CpuFetchEventKind::Completed,
            data: Some(data),
        }
    }

    pub fn retry(record: CpuFetchRecord) -> Self {
        Self {
            record,
            kind: CpuFetchEventKind::Retry,
            data: None,
        }
    }

    pub fn tick(&self) -> Tick {
        self.record.tick()
    }

    pub fn partition(&self) -> PartitionId {
        self.record.partition()
    }

    pub fn route(&self) -> MemoryRouteId {
        self.record.route()
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        self.record.endpoint()
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.record.request_id()
    }

    pub fn pc(&self) -> Address {
        self.record.pc()
    }

    pub fn size(&self) -> AccessSize {
        self.record.size()
    }

    pub fn kind(&self) -> CpuFetchEventKind {
        self.kind
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuError {
    FetchCrossesLine {
        pc: Address,
        size: AccessSize,
        line_size: u64,
    },
    RouteEndpointMismatch {
        route: MemoryRouteId,
        expected: TransportEndpointId,
        actual: TransportEndpointId,
    },
    RoutePartitionMismatch {
        route: MemoryRouteId,
        expected: PartitionId,
        actual: PartitionId,
    },
    Memory(MemoryError),
    Transport(TransportError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuClusterError {
    DuplicateCpu {
        cpu: CpuId,
    },
    DuplicateAgent {
        agent: AgentId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateFetchEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    UnknownCpu {
        cpu: CpuId,
    },
    Cpu(CpuError),
}

impl fmt::Display for CpuClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCpu { cpu } => {
                write!(formatter, "CPU {} is already registered", cpu.get())
            }
            Self::DuplicateAgent {
                agent,
                existing,
                duplicate,
            } => write!(
                formatter,
                "agent {} is assigned to CPU {} and CPU {}",
                agent.get(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateFetchEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "fetch endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::UnknownCpu { cpu } => write!(formatter, "CPU {} is not registered", cpu.get()),
            Self::Cpu(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuClusterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for CpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FetchCrossesLine {
                pc,
                size,
                line_size,
            } => write!(
                formatter,
                "instruction fetch at {:#x} for {} bytes crosses a {line_size}-byte line",
                pc.get(),
                size.bytes()
            ),
            Self::RouteEndpointMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU fetch route {} starts at endpoint {} but CPU endpoint is {}",
                route.get(),
                actual.as_str(),
                expected.as_str()
            ),
            Self::RoutePartitionMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU fetch route {} starts on partition {} but CPU partition is {}",
                route.get(),
                actual.index(),
                expected.index()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

pub fn is_fetch_request(request: &MemoryRequest) -> bool {
    request.operation() == MemoryOperation::InstructionFetch
}
