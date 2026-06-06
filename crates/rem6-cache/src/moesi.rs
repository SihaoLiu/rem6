use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_moesi::{
    MoesiCacheLine, MoesiError, MoesiEvent, MoesiLineId, MoesiState, MoesiTransition,
};
use rem6_transport::TargetOutcome;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoesiCacheControllerResultKind {
    Hit,
    Miss,
    Fill,
    Snoop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiCacheControllerError {
    WrongLine {
        expected: MoesiLineId,
        actual: MoesiLineId,
    },
    SnapshotIdentityMismatch {
        expected_agent: AgentId,
        actual_agent: AgentId,
        expected_line: MoesiLineId,
        actual_line: MoesiLineId,
        expected_layout: CacheLineLayout,
        actual_layout: CacheLineLayout,
    },
    LineBusy {
        state: MoesiState,
    },
    NoPendingMiss,
    UnexpectedFill {
        expected: MemoryRequestId,
        actual: MemoryRequestId,
    },
    UnexpectedFillEvent {
        event: MoesiEvent,
    },
    LineDataUnavailable {
        line: MoesiLineId,
    },
    LineDataSizeMismatch {
        expected: u64,
        actual: u64,
    },
    AccessOutsideLine {
        line: MoesiLineId,
        address: Address,
        size: AccessSize,
    },
    Memory(MemoryError),
    Protocol(MoesiError),
}

impl fmt::Display for MoesiCacheControllerError {
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

impl Error for MoesiCacheControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiCacheControllerResult {
    kind: MoesiCacheControllerResultKind,
    state: MoesiState,
    transition: Option<MoesiTransition>,
    downstream_request: Option<MemoryRequest>,
    post_fill_downstream_requests: Vec<MemoryRequest>,
    target_outcome: Option<TargetOutcome>,
    target_outcomes: Vec<TargetOutcome>,
}

impl MoesiCacheControllerResult {
    pub(crate) fn new(
        kind: MoesiCacheControllerResultKind,
        state: MoesiState,
        transition: Option<MoesiTransition>,
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

    pub const fn kind(&self) -> MoesiCacheControllerResultKind {
        self.kind
    }

    pub const fn state(&self) -> MoesiState {
        self.state
    }

    pub fn transition(&self) -> Option<&MoesiTransition> {
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
struct PendingMoesiMiss {
    original: MemoryRequest,
    downstream: MemoryRequestId,
}

impl PendingMoesiMiss {
    fn snapshot(&self) -> MoesiPendingMissSnapshot {
        MoesiPendingMissSnapshot::new(self.original.clone(), self.downstream)
    }

    fn from_snapshot(snapshot: &MoesiPendingMissSnapshot) -> Self {
        Self {
            original: snapshot.original.clone(),
            downstream: snapshot.downstream,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiPendingMissSnapshot {
    original: MemoryRequest,
    downstream: MemoryRequestId,
}

impl MoesiPendingMissSnapshot {
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
pub struct MoesiCacheControllerSnapshot {
    line: MoesiCacheLine,
    layout: CacheLineLayout,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<MoesiPendingMissSnapshot>,
}

impl MoesiCacheControllerSnapshot {
    pub fn new(
        line: MoesiCacheLine,
        layout: CacheLineLayout,
        next_sequence: u64,
        data: Option<Vec<u8>>,
        pending: Option<MoesiPendingMissSnapshot>,
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

    pub const fn line(&self) -> MoesiLineId {
        self.line.line()
    }

    pub const fn state(&self) -> MoesiState {
        self.line.state()
    }

    pub fn line_state(&self) -> &MoesiCacheLine {
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

    pub fn pending(&self) -> Option<&MoesiPendingMissSnapshot> {
        self.pending.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiCacheController {
    agent: AgentId,
    layout: CacheLineLayout,
    line: MoesiCacheLine,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<PendingMoesiMiss>,
    locked_reservations: BTreeMap<AgentId, Address>,
}

impl MoesiCacheController {
    pub fn new(agent: AgentId, layout: CacheLineLayout, line_address: Address) -> Self {
        Self {
            agent,
            layout,
            line: MoesiCacheLine::new(agent, MoesiLineId::new(layout.line_address(line_address))),
            next_sequence: 0,
            data: None,
            pending: None,
            locked_reservations: BTreeMap::new(),
        }
    }

    pub fn state(&self) -> MoesiState {
        self.line.state()
    }

    pub fn line(&self) -> MoesiLineId {
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

    pub fn snapshot(&self) -> MoesiCacheControllerSnapshot {
        MoesiCacheControllerSnapshot::new(
            self.line.clone(),
            self.layout,
            self.next_sequence,
            self.data.clone(),
            self.pending.as_ref().map(PendingMoesiMiss::snapshot),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &MoesiCacheControllerSnapshot,
    ) -> Result<(), MoesiCacheControllerError> {
        self.validate_snapshot_identity(snapshot)?;
        self.validate_snapshot_data(snapshot)?;
        self.line = snapshot.line.clone();
        self.next_sequence = snapshot.next_sequence;
        self.data = snapshot.data.clone();
        self.pending = snapshot
            .pending
            .as_ref()
            .map(PendingMoesiMiss::from_snapshot);
        self.locked_reservations.clear();
        Ok(())
    }

    pub fn install_shared(&mut self, data: Vec<u8>) -> Result<(), MoesiCacheControllerError> {
        self.install_stable(MoesiState::Shared, data)
    }

    pub fn install_exclusive(&mut self, data: Vec<u8>) -> Result<(), MoesiCacheControllerError> {
        self.install_stable(MoesiState::Exclusive, data)
    }

    pub fn install_owned(&mut self, data: Vec<u8>) -> Result<(), MoesiCacheControllerError> {
        self.install_stable(MoesiState::Owned, data)
    }

    pub fn install_modified(&mut self, data: Vec<u8>) -> Result<(), MoesiCacheControllerError> {
        self.install_stable(MoesiState::Modified, data)
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<MoesiCacheControllerResult, MoesiCacheControllerError> {
        self.check_request_line(&request)?;
        if request.operation() == MemoryOperation::NoAccess {
            let response = MemoryResponse::completed(&request, None)
                .map_err(MoesiCacheControllerError::Memory)?;
            return Ok(MoesiCacheControllerResult::new(
                MoesiCacheControllerResultKind::Hit,
                self.line.state(),
                None,
                None,
                Some(TargetOutcome::Respond(response)),
            ));
        }
        if self.line.state().is_transient() {
            return Err(MoesiCacheControllerError::LineBusy {
                state: self.line.state(),
            });
        }

        let event = moesi_cpu_event(&request);
        let transition = self
            .line
            .apply(event)
            .map_err(MoesiCacheControllerError::Protocol)?;
        if transition.after().is_transient() {
            let downstream = self.downstream_request(&request, transition.before())?;
            self.pending = Some(PendingMoesiMiss {
                original: request,
                downstream: downstream.id(),
            });
            return Ok(MoesiCacheControllerResult::new(
                MoesiCacheControllerResultKind::Miss,
                self.line.state(),
                Some(transition),
                Some(downstream),
                None,
            ));
        }

        let response = self.complete_cpu_request(&request)?;
        Ok(MoesiCacheControllerResult::new(
            MoesiCacheControllerResultKind::Hit,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
        event: MoesiEvent,
    ) -> Result<MoesiCacheControllerResult, MoesiCacheControllerError> {
        if !event.is_data_response() {
            return Err(MoesiCacheControllerError::UnexpectedFillEvent { event });
        }
        let pending = self
            .pending
            .take()
            .ok_or(MoesiCacheControllerError::NoPendingMiss)?;
        if response.request_id() != pending.downstream {
            let expected = pending.downstream;
            self.pending = Some(pending);
            return Err(MoesiCacheControllerError::UnexpectedFill {
                expected,
                actual: response.request_id(),
            });
        }

        let transition = self
            .line
            .apply(event)
            .map_err(MoesiCacheControllerError::Protocol)?;
        if let Some(data) = response.data() {
            self.install_data(data.to_vec())?;
        }

        let response = self.complete_cpu_request(&pending.original)?;
        Ok(MoesiCacheControllerResult::new(
            MoesiCacheControllerResultKind::Fill,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_snoop(
        &mut self,
        event: MoesiEvent,
    ) -> Result<MoesiCacheControllerResult, MoesiCacheControllerError> {
        let transition = self
            .line
            .apply(event)
            .map_err(MoesiCacheControllerError::Protocol)?;
        if self.line.state() == MoesiState::Invalid {
            self.data = None;
            self.locked_reservations.clear();
        }

        Ok(MoesiCacheControllerResult::new(
            MoesiCacheControllerResultKind::Snoop,
            self.line.state(),
            Some(transition),
            None,
            None,
        ))
    }

    fn install_stable(
        &mut self,
        state: MoesiState,
        data: Vec<u8>,
    ) -> Result<(), MoesiCacheControllerError> {
        self.line
            .force_state(state)
            .map_err(MoesiCacheControllerError::Protocol)?;
        self.install_data(data)
    }

    fn install_data(&mut self, data: Vec<u8>) -> Result<(), MoesiCacheControllerError> {
        if data.len() as u64 != self.layout.bytes() {
            return Err(MoesiCacheControllerError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.locked_reservations.clear();
        self.data = Some(data);
        Ok(())
    }

    fn check_request_line(&self, request: &MemoryRequest) -> Result<(), MoesiCacheControllerError> {
        let actual = MoesiLineId::new(request.line_address());
        let expected = self.line.line();
        if actual != expected {
            return Err(MoesiCacheControllerError::WrongLine { expected, actual });
        }

        Ok(())
    }

    fn downstream_request(
        &mut self,
        request: &MemoryRequest,
        before: MoesiState,
    ) -> Result<MemoryRequest, MoesiCacheControllerError> {
        let id = MemoryRequestId::new(self.agent, self.next_sequence);
        self.next_sequence += 1;
        let line_address = self.line.line().address();
        let line_size =
            AccessSize::new(self.layout.bytes()).map_err(MoesiCacheControllerError::Memory)?;

        let downstream = match (moesi_cpu_event(request), before) {
            (MoesiEvent::CpuRead, _) => {
                MemoryRequest::read_shared(id, line_address, line_size, self.layout)
                    .map_err(MoesiCacheControllerError::Memory)
            }
            (MoesiEvent::CpuWrite, MoesiState::Shared | MoesiState::Owned) => {
                MemoryRequest::upgrade(
                    id,
                    line_address,
                    AccessSize::new(1).map_err(MoesiCacheControllerError::Memory)?,
                    self.layout,
                )
                .map_err(MoesiCacheControllerError::Memory)
            }
            (MoesiEvent::CpuWrite, _) => {
                MemoryRequest::read_unique(id, line_address, line_size, self.layout)
                    .map_err(MoesiCacheControllerError::Memory)
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
    ) -> Result<MemoryResponse, MoesiCacheControllerError> {
        if request.operation() == MemoryOperation::Atomic {
            let data = self.read_slice(request)?;
            let write_data = request
                .atomic_write_data(&data)
                .map_err(MoesiCacheControllerError::Memory)?;
            self.apply_store_data(request, &write_data)?;
            self.clear_locked_reservations(request);
            return MemoryResponse::completed(request, Some(data))
                .map_err(MoesiCacheControllerError::Memory);
        }

        if request.operation() == MemoryOperation::LoadLocked {
            let data = self.read_slice(request)?;
            self.track_load_locked(request);
            return MemoryResponse::completed(request, Some(data))
                .map_err(MoesiCacheControllerError::Memory);
        }

        if request.operation() == MemoryOperation::StoreConditional {
            if !self.store_conditional_allowed(request) {
                return MemoryResponse::store_conditional_failed(request)
                    .map_err(MoesiCacheControllerError::Memory);
            }

            self.apply_store(request)?;
            self.clear_locked_reservations(request);
            return MemoryResponse::completed(request, None)
                .map_err(MoesiCacheControllerError::Memory);
        }

        if request.carries_data() {
            self.apply_store(request)?;
            self.clear_locked_reservations(request);
            return MemoryResponse::completed(request, None)
                .map_err(MoesiCacheControllerError::Memory);
        }

        if request.returns_data() {
            let data = self.read_slice(request)?;
            return MemoryResponse::completed(request, Some(data))
                .map_err(MoesiCacheControllerError::Memory);
        }

        MemoryResponse::completed(request, None).map_err(MoesiCacheControllerError::Memory)
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

    fn apply_store(&mut self, request: &MemoryRequest) -> Result<(), MoesiCacheControllerError> {
        let payload = request.data().ok_or(MoesiCacheControllerError::Memory(
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
    ) -> Result<(), MoesiCacheControllerError> {
        let offset = self.checked_offset(request)?;
        let line = self.line();
        let data = self
            .data
            .as_mut()
            .ok_or(MoesiCacheControllerError::LineDataUnavailable { line })?;
        let mask = request.byte_mask();

        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn read_slice(&self, request: &MemoryRequest) -> Result<Vec<u8>, MoesiCacheControllerError> {
        let offset = self.checked_offset(request)?;
        let data = self
            .data
            .as_ref()
            .ok_or(MoesiCacheControllerError::LineDataUnavailable { line: self.line() })?;
        Ok(data[offset..offset + request.size().bytes() as usize].to_vec())
    }

    fn checked_offset(&self, request: &MemoryRequest) -> Result<usize, MoesiCacheControllerError> {
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        if offset + size > self.layout.bytes() as usize {
            return Err(MoesiCacheControllerError::AccessOutsideLine {
                line: self.line(),
                address: request.range().start(),
                size: request.size(),
            });
        }

        Ok(offset)
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &MoesiCacheControllerSnapshot,
    ) -> Result<(), MoesiCacheControllerError> {
        if self.agent == snapshot.agent()
            && self.line() == snapshot.line()
            && self.layout == snapshot.layout()
        {
            return Ok(());
        }

        Err(MoesiCacheControllerError::SnapshotIdentityMismatch {
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
        snapshot: &MoesiCacheControllerSnapshot,
    ) -> Result<(), MoesiCacheControllerError> {
        if let Some(data) = snapshot.cached_data() {
            if data.len() as u64 != self.layout.bytes() {
                return Err(MoesiCacheControllerError::LineDataSizeMismatch {
                    expected: self.layout.bytes(),
                    actual: data.len() as u64,
                });
            }
        }

        Ok(())
    }
}

fn moesi_cpu_event(request: &MemoryRequest) -> MoesiEvent {
    match request.operation() {
        MemoryOperation::NoAccess => {
            unreachable!("no-access requests bypass MOESI event selection")
        }
        MemoryOperation::InstructionFetch
        | MemoryOperation::ReadShared
        | MemoryOperation::LoadLocked => MoesiEvent::CpuRead,
        MemoryOperation::ReadUnique
        | MemoryOperation::LockedRmwRead
        | MemoryOperation::Write
        | MemoryOperation::CacheBlockZero
        | MemoryOperation::StoreConditional
        | MemoryOperation::LockedRmwWrite
        | MemoryOperation::Upgrade
        | MemoryOperation::Atomic
        | MemoryOperation::PrefetchWrite => MoesiEvent::CpuWrite,
        MemoryOperation::PrefetchRead => MoesiEvent::CpuRead,
        MemoryOperation::WriteClean
        | MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanShared
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate
        | MemoryOperation::InvalidateWritable => MoesiEvent::CpuWrite,
    }
}
