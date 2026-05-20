use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvError, RiscvExecutionRecord, RiscvHartState,
    RiscvInstruction,
};
use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, SchedulerContext, Tick};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryError, MemoryOperation,
    MemoryRequest, MemoryRequestId, ResponseStatus,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, ResponseDelivery, TargetOutcome,
    TransportEndpointId, TransportError,
};

mod riscv_cluster;

pub use riscv_cluster::{RiscvCluster, RiscvClusterError};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuDataConfig {
    endpoint: TransportEndpointId,
    route: MemoryRouteId,
    line_layout: CacheLineLayout,
}

impl CpuDataConfig {
    pub const fn new(
        endpoint: TransportEndpointId,
        route: MemoryRouteId,
        line_layout: CacheLineLayout,
    ) -> Self {
        Self {
            endpoint,
            route,
            line_layout,
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

    fn has_pending_fetch(&self) -> bool {
        !self
            .state
            .lock()
            .expect("cpu core lock")
            .outstanding
            .is_empty()
    }

    fn set_pc(&self, pc: Address) {
        self.state.lock().expect("cpu core lock").pc = pc;
    }

    fn advance_sequence_past(&self, request: MemoryRequestId) {
        let mut state = self.state.lock().expect("cpu core lock");
        if state.next_sequence <= request.sequence() {
            state.next_sequence = request.sequence() + 1;
        }
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

#[derive(Clone, Debug)]
pub struct RiscvCore {
    core: CpuCore,
    state: Arc<Mutex<RiscvCoreState>>,
}

impl RiscvCore {
    pub fn new(core: CpuCore) -> Self {
        let pc = core.pc().get();
        Self {
            core,
            state: Arc::new(Mutex::new(RiscvCoreState::new(pc))),
        }
    }

    pub fn with_data(core: CpuCore, data: CpuDataConfig) -> Self {
        let core = Self::new(core);
        core.state.lock().expect("riscv core lock").data = Some(data);
        core
    }

    pub fn inner(&self) -> CpuCore {
        self.core.clone()
    }

    pub fn id(&self) -> CpuId {
        self.core.id()
    }

    pub fn partition(&self) -> PartitionId {
        self.core.partition()
    }

    pub fn agent(&self) -> AgentId {
        self.core.agent()
    }

    pub fn fetch_endpoint(&self) -> TransportEndpointId {
        self.core.fetch_endpoint()
    }

    pub fn data_endpoint(&self) -> Option<TransportEndpointId> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data
            .as_ref()
            .map(|data| data.endpoint().clone())
    }

    pub fn pc(&self) -> Address {
        Address::new(self.state.lock().expect("riscv core lock").hart.pc())
    }

    pub fn read_register(&self, register: Register) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .read(register)
    }

    pub fn write_register(&self, register: Register, value: u64) {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .write(register, value);
    }

    pub fn redirect_pc(&self, pc: Address) {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .set_pc(pc.get());
        self.core.set_pc(pc);
    }

    pub fn execution_events(&self) -> Vec<RiscvCpuExecutionEvent> {
        self.state.lock().expect("riscv core lock").events.clone()
    }

    pub fn data_access_events(&self) -> Vec<RiscvDataAccessEvent> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_events
            .clone()
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
        self.core
            .issue_next_fetch(scheduler, transport, trace, responder)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_next_action<F, D>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        if self.core.has_pending_fetch() || self.has_pending_data_access() {
            return Ok(None);
        }

        if let Some(event) = self.execute_next_completed_fetch()? {
            return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(event)));
        }

        if let Some(event) =
            self.issue_next_data_access(scheduler, transport, data_trace, data_responder)?
        {
            return Ok(Some(RiscvCoreDriveAction::DataAccessIssued { event }));
        }

        let event = self
            .issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)
            .map_err(RiscvCpuError::Cpu)?;
        Ok(Some(RiscvCoreDriveAction::FetchIssued { event }))
    }

    pub fn execute_next_completed_fetch(
        &self,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(fetch) = fetch_events.into_iter().find(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
        }) else {
            return Ok(None);
        };

        let architectural = Address::new(state.hart.pc());
        if fetch.pc() != architectural {
            return Err(RiscvCpuError::PcMismatch {
                fetch: fetch.pc(),
                architectural,
            });
        }

        let data = fetch.data().ok_or(RiscvCpuError::MissingFetchData {
            request: fetch.request_id(),
        })?;
        if data.len() != 4 {
            return Err(RiscvCpuError::InvalidFetchWidth {
                request: fetch.request_id(),
                bytes: data.len() as u64,
            });
        }
        let raw = u32::from_le_bytes(data.try_into().expect("fetch width checked"));
        let instruction = RiscvInstruction::decode(raw).map_err(RiscvCpuError::Isa)?;
        let execution = state
            .hart
            .execute(instruction)
            .map_err(RiscvCpuError::Isa)?;
        let next_pc = Address::new(execution.next_pc());
        self.core.set_pc(next_pc);

        let event = RiscvCpuExecutionEvent::new(fetch.clone(), instruction, execution);
        state.executed_fetches.insert(fetch.request_id());
        state.events.push(event.clone());
        Ok(Some(event))
    }

    pub fn issue_next_data_access<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let Some(issue) = self.prepare_data_access(scheduler.now(), transport)? else {
            return Ok(None);
        };
        let request = issue.memory_request()?;

        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                issue.route,
                request,
                trace,
                responder,
                move |delivery| core.record_data_response(delivery),
            )
            .map_err(RiscvCpuError::Transport)?;

        self.record_data_issue(issue);
        Ok(Some(event))
    }

    fn prepare_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let state = self.state.lock().expect("riscv core lock");
        let Some((fetch_request, access)) = state.events.iter().find_map(|event| {
            let fetch_request = event.fetch().request_id();
            if state.issued_data_for_fetches.contains(&fetch_request) {
                return None;
            }
            event
                .execution()
                .memory_access()
                .map(|access| (fetch_request, access.clone()))
        }) else {
            return Ok(None);
        };

        let data = state.data.clone().ok_or(RiscvCpuError::MissingDataConfig {
            fetch: fetch_request,
        })?;
        let route = transport
            .route(data.route())
            .ok_or(RiscvCpuError::Transport(TransportError::UnknownRoute {
                route: data.route(),
            }))?;
        if route.source_partition() != self.core.partition() {
            return Err(RiscvCpuError::DataRoutePartitionMismatch {
                route: data.route(),
                expected: self.core.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source() != data.endpoint() {
            return Err(RiscvCpuError::DataRouteEndpointMismatch {
                route: data.route(),
                expected: data.endpoint().clone(),
                actual: route.source().clone(),
            });
        }

        let size = memory_width_size(access_width(&access))?;
        let address = Address::new(access_address(&access));
        let line_offset = data.line_layout().line_offset(address);
        if line_offset + size.bytes() > data.line_layout().bytes() {
            return Err(RiscvCpuError::DataAccessCrossesLine {
                address,
                size,
                line_size: data.line_layout().bytes(),
            });
        }

        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());

        Ok(Some(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            route: data.route(),
            endpoint: data.endpoint().clone(),
            request_id,
            fetch_request,
            access,
            size,
            line_layout: data.line_layout(),
        }))
    }

    fn record_data_issue(&self, issue: OutstandingDataAccess) {
        self.core.advance_sequence_past(issue.request_id);
        let mut state = self.state.lock().expect("riscv core lock");
        state.issued_data_for_fetches.insert(issue.fetch_request);
        state
            .outstanding_data
            .insert(issue.request_id, issue.clone_without_layout());
        state.data_events.push(RiscvDataAccessEvent::issued(
            issue.record(issue.tick, issue.endpoint.clone()),
        ));
    }

    fn record_data_response(&self, delivery: ResponseDelivery) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state
            .outstanding_data
            .remove(&delivery.response().request_id())
        else {
            return;
        };

        match delivery.response().status() {
            ResponseStatus::Completed => {
                let data = delivery.response().data().map(ToOwned::to_owned);
                if let MemoryAccessKind::Load {
                    rd, width, signed, ..
                } = access.access
                {
                    let value = load_response_value(
                        data.as_deref().expect("load response data"),
                        width,
                        signed,
                    );
                    state.hart.write(rd, value);
                }
                state.data_events.push(RiscvDataAccessEvent::completed(
                    access.record(delivery.tick(), delivery.endpoint().clone()),
                    data,
                ));
            }
            ResponseStatus::Retry => {
                state.data_events.push(RiscvDataAccessEvent::retry(
                    access.record(delivery.tick(), delivery.endpoint().clone()),
                ));
            }
        }
    }

    fn has_pending_data_access(&self) -> bool {
        !self
            .state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvCoreState {
    hart: RiscvHartState,
    data: Option<CpuDataConfig>,
    executed_fetches: BTreeSet<MemoryRequestId>,
    issued_data_for_fetches: BTreeSet<MemoryRequestId>,
    outstanding_data: BTreeMap<MemoryRequestId, IssuedDataAccess>,
    events: Vec<RiscvCpuExecutionEvent>,
    data_events: Vec<RiscvDataAccessEvent>,
}

impl RiscvCoreState {
    fn new(pc: u64) -> Self {
        Self {
            hart: RiscvHartState::new(pc),
            data: None,
            executed_fetches: BTreeSet::new(),
            issued_data_for_fetches: BTreeSet::new(),
            outstanding_data: BTreeMap::new(),
            events: Vec::new(),
            data_events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutstandingDataAccess {
    tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request_id: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
    line_layout: CacheLineLayout,
}

impl OutstandingDataAccess {
    fn memory_request(&self) -> Result<MemoryRequest, RiscvCpuError> {
        match &self.access {
            MemoryAccessKind::Load { address, .. } => MemoryRequest::read_shared(
                self.request_id,
                Address::new(*address),
                self.size,
                self.line_layout,
            )
            .map_err(RiscvCpuError::Memory),
            MemoryAccessKind::Store { address, value, .. } => MemoryRequest::write(
                self.request_id,
                Address::new(*address),
                self.size,
                store_bytes(*value, self.size),
                ByteMask::full(self.size).map_err(RiscvCpuError::Memory)?,
                self.line_layout,
            )
            .map_err(RiscvCpuError::Memory),
        }
    }

    fn clone_without_layout(&self) -> IssuedDataAccess {
        IssuedDataAccess {
            partition: self.partition,
            route: self.route,
            request: self.request_id,
            fetch_request: self.fetch_request,
            access: self.access.clone(),
            size: self.size,
        }
    }

    fn record(&self, tick: Tick, endpoint: TransportEndpointId) -> RiscvDataAccessRecord {
        self.clone_without_layout().record(tick, endpoint)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IssuedDataAccess {
    partition: PartitionId,
    route: MemoryRouteId,
    request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
}

impl IssuedDataAccess {
    fn record(&self, tick: Tick, endpoint: TransportEndpointId) -> RiscvDataAccessRecord {
        RiscvDataAccessRecord::new(
            tick,
            self.partition,
            self.route,
            endpoint,
            self.request,
            self.fetch_request,
            self.access.clone(),
            self.size,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCpuExecutionEvent {
    fetch: CpuFetchEvent,
    instruction: RiscvInstruction,
    execution: RiscvExecutionRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreDriveAction {
    FetchIssued { event: PartitionEventId },
    InstructionExecuted(RiscvCpuExecutionEvent),
    DataAccessIssued { event: PartitionEventId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvDataAccessEventKind {
    Issued,
    Completed,
    Retry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataAccessRecord {
    tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
}

impl RiscvDataAccessRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tick: Tick,
        partition: PartitionId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        request: MemoryRequestId,
        fetch_request: MemoryRequestId,
        access: MemoryAccessKind,
        size: AccessSize,
    ) -> Self {
        Self {
            tick,
            partition,
            route,
            endpoint,
            request,
            fetch_request,
            access,
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

    pub fn fetch_request_id(&self) -> MemoryRequestId {
        self.fetch_request
    }

    pub fn access(&self) -> &MemoryAccessKind {
        &self.access
    }

    pub fn size(&self) -> AccessSize {
        self.size
    }

    pub fn operation(&self) -> MemoryOperation {
        match self.access {
            MemoryAccessKind::Load { .. } => MemoryOperation::ReadShared,
            MemoryAccessKind::Store { .. } => MemoryOperation::Write,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataAccessEvent {
    record: RiscvDataAccessRecord,
    kind: RiscvDataAccessEventKind,
    data: Option<Vec<u8>>,
}

impl RiscvDataAccessEvent {
    pub fn issued(record: RiscvDataAccessRecord) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Issued,
            data: None,
        }
    }

    pub fn completed(record: RiscvDataAccessRecord, data: Option<Vec<u8>>) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Completed,
            data,
        }
    }

    pub fn retry(record: RiscvDataAccessRecord) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Retry,
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

    pub fn fetch_request_id(&self) -> MemoryRequestId {
        self.record.fetch_request_id()
    }

    pub fn access(&self) -> &MemoryAccessKind {
        self.record.access()
    }

    pub fn size(&self) -> AccessSize {
        self.record.size()
    }

    pub fn operation(&self) -> MemoryOperation {
        self.record.operation()
    }

    pub fn kind(&self) -> RiscvDataAccessEventKind {
        self.kind
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

impl RiscvCpuExecutionEvent {
    pub const fn new(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
    ) -> Self {
        Self {
            fetch,
            instruction,
            execution,
        }
    }

    pub fn fetch(&self) -> &CpuFetchEvent {
        &self.fetch
    }

    pub fn fetch_pc(&self) -> Address {
        self.fetch.pc()
    }

    pub const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub fn execution(&self) -> &RiscvExecutionRecord {
        &self.execution
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCpuError {
    MissingDataConfig {
        fetch: MemoryRequestId,
    },
    MissingFetchData {
        request: MemoryRequestId,
    },
    InvalidFetchWidth {
        request: MemoryRequestId,
        bytes: u64,
    },
    PcMismatch {
        fetch: Address,
        architectural: Address,
    },
    DataAccessCrossesLine {
        address: Address,
        size: AccessSize,
        line_size: u64,
    },
    DataRouteEndpointMismatch {
        route: MemoryRouteId,
        expected: TransportEndpointId,
        actual: TransportEndpointId,
    },
    DataRoutePartitionMismatch {
        route: MemoryRouteId,
        expected: PartitionId,
        actual: PartitionId,
    },
    Cpu(CpuError),
    Isa(RiscvError),
    Memory(MemoryError),
    Transport(TransportError),
}

impl fmt::Display for RiscvCpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDataConfig { fetch } => write!(
                formatter,
                "fetch response {} from agent {} needs a data route for memory access",
                fetch.sequence(),
                fetch.agent().get()
            ),
            Self::MissingFetchData { request } => write!(
                formatter,
                "fetch response {} from agent {} has no instruction bytes",
                request.sequence(),
                request.agent().get()
            ),
            Self::InvalidFetchWidth { request, bytes } => write!(
                formatter,
                "fetch response {} from agent {} has {bytes} bytes instead of 4",
                request.sequence(),
                request.agent().get()
            ),
            Self::PcMismatch {
                fetch,
                architectural,
            } => write!(
                formatter,
                "fetch pc {:#x} does not match architectural pc {:#x}",
                fetch.get(),
                architectural.get()
            ),
            Self::DataAccessCrossesLine {
                address,
                size,
                line_size,
            } => write!(
                formatter,
                "data access at {:#x} for {} bytes crosses a {line_size}-byte line",
                address.get(),
                size.bytes()
            ),
            Self::DataRouteEndpointMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU data route {} starts at endpoint {} but CPU data endpoint is {}",
                route.get(),
                actual.as_str(),
                expected.as_str()
            ),
            Self::DataRoutePartitionMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU data route {} starts on partition {} but CPU partition is {}",
                route.get(),
                actual.index(),
                expected.index()
            ),
            Self::Cpu(error) => write!(formatter, "{error}"),
            Self::Isa(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvCpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Isa(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

fn access_width(access: &MemoryAccessKind) -> MemoryWidth {
    match access {
        MemoryAccessKind::Load { width, .. } | MemoryAccessKind::Store { width, .. } => *width,
    }
}

fn access_address(access: &MemoryAccessKind) -> u64 {
    match access {
        MemoryAccessKind::Load { address, .. } | MemoryAccessKind::Store { address, .. } => {
            *address
        }
    }
}

fn memory_width_size(width: MemoryWidth) -> Result<AccessSize, RiscvCpuError> {
    let bytes = match width {
        MemoryWidth::Byte => 1,
        MemoryWidth::Halfword => 2,
        MemoryWidth::Word => 4,
        MemoryWidth::Doubleword => 8,
    };
    AccessSize::new(bytes).map_err(RiscvCpuError::Memory)
}

fn store_bytes(value: u64, size: AccessSize) -> Vec<u8> {
    value.to_le_bytes()[..size.bytes() as usize].to_vec()
}

fn load_response_value(data: &[u8], width: MemoryWidth, signed: bool) -> u64 {
    let raw = data.iter().enumerate().fold(0u64, |value, (shift, byte)| {
        value | (u64::from(*byte) << (shift * 8))
    });
    if !signed || width == MemoryWidth::Doubleword {
        return raw;
    }

    let bits = data.len() as u32 * 8;
    let sign_bit = 1u64 << (bits - 1);
    if raw & sign_bit == 0 {
        raw
    } else {
        raw | (!0u64 << bits)
    }
}

pub fn is_fetch_request(request: &MemoryRequest) -> bool {
    request.operation() == MemoryOperation::InstructionFetch
}
