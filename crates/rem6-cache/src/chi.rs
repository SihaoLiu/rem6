use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_chi::{ChiCacheLine, ChiError, ChiEvent, ChiLineId, ChiState, ChiTransition};
use rem6_transport::TargetOutcome;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChiCacheControllerResultKind {
    Hit,
    Miss,
    Fill,
    Snoop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChiCacheControllerError {
    WrongLine {
        expected: ChiLineId,
        actual: ChiLineId,
    },
    SnapshotIdentityMismatch {
        expected_agent: AgentId,
        actual_agent: AgentId,
        expected_line: ChiLineId,
        actual_line: ChiLineId,
        expected_layout: CacheLineLayout,
        actual_layout: CacheLineLayout,
    },
    LineBusy {
        state: ChiState,
    },
    NoPendingMiss,
    UnexpectedFill {
        expected: MemoryRequestId,
        actual: MemoryRequestId,
    },
    UnexpectedFillEvent {
        event: ChiEvent,
    },
    LineDataUnavailable {
        line: ChiLineId,
    },
    LineDataSizeMismatch {
        expected: u64,
        actual: u64,
    },
    AccessOutsideLine {
        line: ChiLineId,
        address: Address,
        size: AccessSize,
    },
    Memory(MemoryError),
    Protocol(ChiError),
}

impl fmt::Display for ChiCacheControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLine { expected, actual } => write!(
                formatter,
                "request for line {:#x} reached controller for line {:#x}",
                actual.address().get(),
                expected.address().get()
            ),
            Self::SnapshotIdentityMismatch {
                expected_agent,
                actual_agent,
                expected_line,
                actual_line,
                expected_layout,
                actual_layout,
            } => write!(
                formatter,
                "snapshot for agent {:?}, line {:#x}, line size {} cannot restore controller for agent {:?}, line {:#x}, line size {}",
                actual_agent,
                actual_line.address().get(),
                actual_layout.bytes(),
                expected_agent,
                expected_line.address().get(),
                expected_layout.bytes()
            ),
            Self::LineBusy { state } => write!(formatter, "cache line is busy in {state:?}"),
            Self::NoPendingMiss => write!(formatter, "cache line has no pending miss"),
            Self::UnexpectedFill { expected, actual } => write!(
                formatter,
                "fill response {:?} does not match pending request {:?}",
                actual, expected
            ),
            Self::UnexpectedFillEvent { event } => {
                write!(formatter, "fill event {event:?} is not a data response")
            }
            Self::LineDataUnavailable { line } => {
                write!(
                    formatter,
                    "line {:#x} has no installed data",
                    line.address().get()
                )
            }
            Self::LineDataSizeMismatch { expected, actual } => write!(
                formatter,
                "line data has {actual} bytes but cache line expects {expected}"
            ),
            Self::AccessOutsideLine {
                line,
                address,
                size,
            } => write!(
                formatter,
                "access {:#x}+{} is outside cache line {:#x}",
                address.get(),
                size.bytes(),
                line.address().get()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ChiCacheControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiCacheControllerResult {
    kind: ChiCacheControllerResultKind,
    state: ChiState,
    transition: Option<ChiTransition>,
    downstream_request: Option<MemoryRequest>,
    post_fill_downstream_requests: Vec<MemoryRequest>,
    target_outcome: Option<TargetOutcome>,
    target_outcomes: Vec<TargetOutcome>,
}

impl ChiCacheControllerResult {
    pub(crate) fn new(
        kind: ChiCacheControllerResultKind,
        state: ChiState,
        transition: Option<ChiTransition>,
        downstream_request: Option<MemoryRequest>,
        target_outcome: Option<TargetOutcome>,
    ) -> Self {
        Self {
            kind,
            state,
            transition,
            downstream_request,
            post_fill_downstream_requests: Vec::new(),
            target_outcomes: target_outcome.iter().cloned().collect(),
            target_outcome,
        }
    }

    pub(crate) fn with_post_fill_downstream_requests(
        mut self,
        requests: Vec<MemoryRequest>,
    ) -> Self {
        self.post_fill_downstream_requests = requests;
        self
    }

    pub(crate) fn with_target_outcomes(mut self, target_outcomes: Vec<TargetOutcome>) -> Self {
        self.target_outcome = target_outcomes.first().cloned();
        self.target_outcomes = target_outcomes;
        self
    }

    pub const fn kind(&self) -> ChiCacheControllerResultKind {
        self.kind
    }

    pub const fn state(&self) -> ChiState {
        self.state
    }

    pub fn transition(&self) -> Option<&ChiTransition> {
        self.transition.as_ref()
    }

    pub fn downstream_request(&self) -> Option<&MemoryRequest> {
        self.downstream_request.as_ref()
    }

    pub fn post_fill_downstream_requests(&self) -> &[MemoryRequest] {
        &self.post_fill_downstream_requests
    }

    pub fn target_outcome(&self) -> Option<&TargetOutcome> {
        self.target_outcome.as_ref()
    }

    pub fn target_outcomes(&self) -> &[TargetOutcome] {
        &self.target_outcomes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingChiMiss {
    original: MemoryRequest,
    downstream: MemoryRequestId,
}

impl PendingChiMiss {
    fn snapshot(&self) -> ChiPendingMissSnapshot {
        ChiPendingMissSnapshot::new(self.original.clone(), self.downstream)
    }

    fn from_snapshot(snapshot: &ChiPendingMissSnapshot) -> Self {
        Self {
            original: snapshot.original.clone(),
            downstream: snapshot.downstream,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiPendingMissSnapshot {
    original: MemoryRequest,
    downstream: MemoryRequestId,
}

impl ChiPendingMissSnapshot {
    pub fn new(original: MemoryRequest, downstream: MemoryRequestId) -> Self {
        Self {
            original,
            downstream,
        }
    }

    pub const fn original(&self) -> &MemoryRequest {
        &self.original
    }

    pub const fn downstream(&self) -> MemoryRequestId {
        self.downstream
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiCacheControllerSnapshot {
    line: ChiCacheLine,
    layout: CacheLineLayout,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<ChiPendingMissSnapshot>,
}

impl ChiCacheControllerSnapshot {
    pub fn new(
        line: ChiCacheLine,
        layout: CacheLineLayout,
        next_sequence: u64,
        data: Option<Vec<u8>>,
        pending: Option<ChiPendingMissSnapshot>,
    ) -> Self {
        Self {
            line,
            layout,
            next_sequence,
            data,
            pending,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.line.agent()
    }

    pub const fn line(&self) -> ChiLineId {
        self.line.line()
    }

    pub const fn state(&self) -> ChiState {
        self.line.state()
    }

    pub fn line_state(&self) -> &ChiCacheLine {
        &self.line
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn cached_data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn pending(&self) -> Option<&ChiPendingMissSnapshot> {
        self.pending.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiCacheController {
    agent: AgentId,
    layout: CacheLineLayout,
    line: ChiCacheLine,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<PendingChiMiss>,
    locked_reservations: BTreeMap<AgentId, Address>,
}

impl ChiCacheController {
    pub fn new(agent: AgentId, layout: CacheLineLayout, line_address: Address) -> Self {
        Self {
            agent,
            layout,
            line: ChiCacheLine::new(agent, ChiLineId::new(layout.line_address(line_address))),
            next_sequence: 0,
            data: None,
            pending: None,
            locked_reservations: BTreeMap::new(),
        }
    }

    pub fn state(&self) -> ChiState {
        self.line.state()
    }

    pub fn line(&self) -> ChiLineId {
        self.line.line()
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) fn set_next_sequence(&mut self, next_sequence: u64) {
        self.next_sequence = next_sequence;
    }

    pub fn cached_data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn snapshot(&self) -> ChiCacheControllerSnapshot {
        ChiCacheControllerSnapshot::new(
            self.line.clone(),
            self.layout,
            self.next_sequence,
            self.data.clone(),
            self.pending.as_ref().map(PendingChiMiss::snapshot),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &ChiCacheControllerSnapshot,
    ) -> Result<(), ChiCacheControllerError> {
        self.validate_snapshot_identity(snapshot)?;
        self.validate_snapshot_data(snapshot)?;
        self.line = snapshot.line.clone();
        self.next_sequence = snapshot.next_sequence;
        self.data = snapshot.data.clone();
        self.pending = snapshot.pending.as_ref().map(PendingChiMiss::from_snapshot);
        self.locked_reservations.clear();
        Ok(())
    }

    pub fn install_shared_clean(&mut self, data: Vec<u8>) -> Result<(), ChiCacheControllerError> {
        self.install_stable(ChiState::SharedClean, data)
    }

    pub fn install_shared_dirty(&mut self, data: Vec<u8>) -> Result<(), ChiCacheControllerError> {
        self.install_stable(ChiState::SharedDirty, data)
    }

    pub fn install_unique_clean(&mut self, data: Vec<u8>) -> Result<(), ChiCacheControllerError> {
        self.install_stable(ChiState::UniqueClean, data)
    }

    pub fn install_unique_dirty(&mut self, data: Vec<u8>) -> Result<(), ChiCacheControllerError> {
        self.install_stable(ChiState::UniqueDirty, data)
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<ChiCacheControllerResult, ChiCacheControllerError> {
        self.check_request_line(&request)?;
        if request.operation() == MemoryOperation::NoAccess {
            let response = MemoryResponse::completed(&request, None)
                .map_err(ChiCacheControllerError::Memory)?;
            return Ok(ChiCacheControllerResult::new(
                ChiCacheControllerResultKind::Hit,
                self.line.state(),
                None,
                None,
                Some(TargetOutcome::Respond(response)),
            ));
        }
        if self.line.state().is_transient() {
            return Err(ChiCacheControllerError::LineBusy {
                state: self.line.state(),
            });
        }

        let event = chi_cpu_event(&request);
        let transition = self
            .line
            .apply(event)
            .map_err(ChiCacheControllerError::Protocol)?;
        if transition.after().is_transient() {
            let downstream = self.downstream_request(&request, transition.before())?;
            self.pending = Some(PendingChiMiss {
                original: request,
                downstream: downstream.id(),
            });
            return Ok(ChiCacheControllerResult::new(
                ChiCacheControllerResultKind::Miss,
                self.line.state(),
                Some(transition),
                Some(downstream),
                None,
            ));
        }

        let response = self.complete_cpu_request(&request)?;
        Ok(ChiCacheControllerResult::new(
            ChiCacheControllerResultKind::Hit,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
        event: ChiEvent,
    ) -> Result<ChiCacheControllerResult, ChiCacheControllerError> {
        if !event.is_data_response() {
            return Err(ChiCacheControllerError::UnexpectedFillEvent { event });
        }
        let pending = self
            .pending
            .take()
            .ok_or(ChiCacheControllerError::NoPendingMiss)?;
        if response.request_id() != pending.downstream {
            let expected = pending.downstream;
            self.pending = Some(pending);
            return Err(ChiCacheControllerError::UnexpectedFill {
                expected,
                actual: response.request_id(),
            });
        }

        let transition = self
            .line
            .apply(event)
            .map_err(ChiCacheControllerError::Protocol)?;
        if let Some(data) = response.data() {
            self.install_data(data.to_vec())?;
        }

        let response = self.complete_cpu_request(&pending.original)?;
        Ok(ChiCacheControllerResult::new(
            ChiCacheControllerResultKind::Fill,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_snoop(
        &mut self,
        event: ChiEvent,
    ) -> Result<ChiCacheControllerResult, ChiCacheControllerError> {
        let transition = self
            .line
            .apply(event)
            .map_err(ChiCacheControllerError::Protocol)?;
        if self.line.state() == ChiState::Invalid {
            self.data = None;
            self.locked_reservations.clear();
        }

        Ok(ChiCacheControllerResult::new(
            ChiCacheControllerResultKind::Snoop,
            self.line.state(),
            Some(transition),
            None,
            None,
        ))
    }

    fn install_stable(
        &mut self,
        state: ChiState,
        data: Vec<u8>,
    ) -> Result<(), ChiCacheControllerError> {
        self.line
            .force_state(state)
            .map_err(ChiCacheControllerError::Protocol)?;
        self.install_data(data)
    }

    fn install_data(&mut self, data: Vec<u8>) -> Result<(), ChiCacheControllerError> {
        if data.len() as u64 != self.layout.bytes() {
            return Err(ChiCacheControllerError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.locked_reservations.clear();
        self.data = Some(data);
        Ok(())
    }

    fn check_request_line(&self, request: &MemoryRequest) -> Result<(), ChiCacheControllerError> {
        let actual = ChiLineId::new(request.line_address());
        let expected = self.line.line();
        if actual != expected {
            return Err(ChiCacheControllerError::WrongLine { expected, actual });
        }

        Ok(())
    }

    fn downstream_request(
        &mut self,
        request: &MemoryRequest,
        before: ChiState,
    ) -> Result<MemoryRequest, ChiCacheControllerError> {
        let id = MemoryRequestId::new(self.agent, self.next_sequence);
        self.next_sequence += 1;
        let line_address = self.line.line().address();
        let line_size =
            AccessSize::new(self.layout.bytes()).map_err(ChiCacheControllerError::Memory)?;

        let downstream = match (chi_cpu_event(request), before) {
            (ChiEvent::CpuRead, _) => {
                MemoryRequest::read_shared(id, line_address, line_size, self.layout)
                    .map_err(ChiCacheControllerError::Memory)
            }
            (ChiEvent::CpuWrite, ChiState::SharedClean | ChiState::SharedDirty) => {
                MemoryRequest::upgrade(
                    id,
                    line_address,
                    AccessSize::new(1).map_err(ChiCacheControllerError::Memory)?,
                    self.layout,
                )
                .map_err(ChiCacheControllerError::Memory)
            }
            (ChiEvent::CpuWrite, _) => {
                MemoryRequest::read_unique(id, line_address, line_size, self.layout)
                    .map_err(ChiCacheControllerError::Memory)
            }
            _ => unreachable!("only CPU requests create downstream requests"),
        }?;
        Ok(crate::downstream::with_source_attributes(
            downstream, request,
        ))
    }

    fn complete_cpu_request(
        &mut self,
        request: &MemoryRequest,
    ) -> Result<MemoryResponse, ChiCacheControllerError> {
        if request.operation() == MemoryOperation::Atomic {
            let data = self.read_slice(request)?;
            let write_data = request
                .atomic_write_data(&data)
                .map_err(ChiCacheControllerError::Memory)?;
            self.apply_store_data(request, &write_data)?;
            self.clear_locked_reservations(request);
            return MemoryResponse::completed(request, Some(data))
                .map_err(ChiCacheControllerError::Memory);
        }

        if request.operation() == MemoryOperation::LoadLocked {
            let data = self.read_slice(request)?;
            self.track_load_locked(request);
            return MemoryResponse::completed(request, Some(data))
                .map_err(ChiCacheControllerError::Memory);
        }

        if request.operation() == MemoryOperation::StoreConditional {
            if !self.store_conditional_allowed(request) {
                return MemoryResponse::store_conditional_failed(request)
                    .map_err(ChiCacheControllerError::Memory);
            }

            self.apply_store(request)?;
            self.clear_locked_reservations(request);
            return MemoryResponse::completed(request, None)
                .map_err(ChiCacheControllerError::Memory);
        }

        if request.carries_data() {
            self.apply_store(request)?;
            self.clear_locked_reservations(request);
            return MemoryResponse::completed(request, None)
                .map_err(ChiCacheControllerError::Memory);
        }

        if request.returns_data() {
            let data = self.read_slice(request)?;
            return MemoryResponse::completed(request, Some(data))
                .map_err(ChiCacheControllerError::Memory);
        }

        MemoryResponse::completed(request, None).map_err(ChiCacheControllerError::Memory)
    }

    fn track_load_locked(&mut self, request: &MemoryRequest) {
        self.locked_reservations
            .insert(request.id().agent(), request.llsc_reservation_address());
    }

    fn store_conditional_allowed(&self, request: &MemoryRequest) -> bool {
        self.locked_reservations
            .get(&request.id().agent())
            .is_some_and(|reserved| *reserved == request.llsc_reservation_address())
    }

    fn clear_locked_reservations(&mut self, request: &MemoryRequest) {
        self.locked_reservations
            .retain(|_, reserved| !request.overlaps_llsc_reservation(*reserved));
    }

    fn apply_store(&mut self, request: &MemoryRequest) -> Result<(), ChiCacheControllerError> {
        let payload = request.data().ok_or(ChiCacheControllerError::Memory(
            MemoryError::MissingRequestData {
                request: request.id(),
            },
        ))?;
        self.apply_store_data(request, payload)
    }

    fn apply_store_data(
        &mut self,
        request: &MemoryRequest,
        payload: &[u8],
    ) -> Result<(), ChiCacheControllerError> {
        let offset = self.checked_offset(request)?;
        let line = self.line();
        let data = self
            .data
            .as_mut()
            .ok_or(ChiCacheControllerError::LineDataUnavailable { line })?;
        let mask = request.byte_mask();

        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn read_slice(&self, request: &MemoryRequest) -> Result<Vec<u8>, ChiCacheControllerError> {
        let offset = self.checked_offset(request)?;
        let data = self
            .data
            .as_ref()
            .ok_or(ChiCacheControllerError::LineDataUnavailable { line: self.line() })?;
        Ok(data[offset..offset + request.size().bytes() as usize].to_vec())
    }

    fn checked_offset(&self, request: &MemoryRequest) -> Result<usize, ChiCacheControllerError> {
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        if offset + size > self.layout.bytes() as usize {
            return Err(ChiCacheControllerError::AccessOutsideLine {
                line: self.line(),
                address: request.range().start(),
                size: request.size(),
            });
        }

        Ok(offset)
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &ChiCacheControllerSnapshot,
    ) -> Result<(), ChiCacheControllerError> {
        if self.agent == snapshot.agent()
            && self.line() == snapshot.line()
            && self.layout == snapshot.layout()
        {
            return Ok(());
        }

        Err(ChiCacheControllerError::SnapshotIdentityMismatch {
            expected_agent: self.agent,
            actual_agent: snapshot.agent(),
            expected_line: self.line(),
            actual_line: snapshot.line(),
            expected_layout: self.layout,
            actual_layout: snapshot.layout(),
        })
    }

    fn validate_snapshot_data(
        &self,
        snapshot: &ChiCacheControllerSnapshot,
    ) -> Result<(), ChiCacheControllerError> {
        if let Some(data) = snapshot.cached_data() {
            if data.len() as u64 != self.layout.bytes() {
                return Err(ChiCacheControllerError::LineDataSizeMismatch {
                    expected: self.layout.bytes(),
                    actual: data.len() as u64,
                });
            }
        }

        Ok(())
    }
}

fn chi_cpu_event(request: &MemoryRequest) -> ChiEvent {
    match request.operation() {
        MemoryOperation::NoAccess => unreachable!("no-access requests bypass CHI event selection"),
        MemoryOperation::InstructionFetch
        | MemoryOperation::ReadShared
        | MemoryOperation::LoadLocked => ChiEvent::CpuRead,
        MemoryOperation::ReadUnique
        | MemoryOperation::LockedRmwRead
        | MemoryOperation::Write
        | MemoryOperation::CacheBlockZero
        | MemoryOperation::StoreConditional
        | MemoryOperation::StoreConditionalUpgrade
        | MemoryOperation::StoreConditionalUpgradeFail
        | MemoryOperation::LockedRmwWrite
        | MemoryOperation::Upgrade
        | MemoryOperation::Atomic
        | MemoryOperation::PrefetchWrite => ChiEvent::CpuWrite,
        MemoryOperation::PrefetchRead => ChiEvent::CpuRead,
        MemoryOperation::WriteClean
        | MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanShared
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate
        | MemoryOperation::InvalidateWritable => ChiEvent::CpuWrite,
    }
}
