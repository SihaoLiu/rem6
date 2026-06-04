use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_mesi::{
    MesiCacheLine, MesiError, MesiEvent, MesiLineId, MesiState, MesiTransition,
};
use rem6_transport::TargetOutcome;

use crate::downstream;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MesiCacheControllerResultKind {
    Hit,
    Miss,
    Fill,
    Snoop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MesiCacheControllerError {
    WrongLine {
        expected: MesiLineId,
        actual: MesiLineId,
    },
    SnapshotIdentityMismatch {
        expected_agent: AgentId,
        actual_agent: AgentId,
        expected_line: MesiLineId,
        actual_line: MesiLineId,
        expected_layout: CacheLineLayout,
        actual_layout: CacheLineLayout,
    },
    LineBusy {
        state: MesiState,
    },
    NoPendingMiss,
    UnexpectedFill {
        expected: MemoryRequestId,
        actual: MemoryRequestId,
    },
    UnexpectedFillEvent {
        event: MesiEvent,
    },
    LineDataUnavailable {
        line: MesiLineId,
    },
    LineDataSizeMismatch {
        expected: u64,
        actual: u64,
    },
    AccessOutsideLine {
        line: MesiLineId,
        address: Address,
        size: AccessSize,
    },
    Memory(MemoryError),
    Protocol(MesiError),
}

impl fmt::Display for MesiCacheControllerError {
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

impl Error for MesiCacheControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiCacheControllerResult {
    kind: MesiCacheControllerResultKind,
    state: MesiState,
    transition: Option<MesiTransition>,
    downstream_request: Option<MemoryRequest>,
    post_fill_downstream_requests: Vec<MemoryRequest>,
    target_outcome: Option<TargetOutcome>,
    target_outcomes: Vec<TargetOutcome>,
}

impl MesiCacheControllerResult {
    pub(crate) fn new(
        kind: MesiCacheControllerResultKind,
        state: MesiState,
        transition: Option<MesiTransition>,
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

    pub const fn kind(&self) -> MesiCacheControllerResultKind {
        self.kind
    }

    pub const fn state(&self) -> MesiState {
        self.state
    }

    pub fn transition(&self) -> Option<&MesiTransition> {
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
struct PendingMesiMiss {
    original: MemoryRequest,
    downstream: MemoryRequestId,
}

impl PendingMesiMiss {
    fn snapshot(&self) -> MesiPendingMissSnapshot {
        MesiPendingMissSnapshot::new(self.original.clone(), self.downstream)
    }

    fn from_snapshot(snapshot: &MesiPendingMissSnapshot) -> Self {
        Self {
            original: snapshot.original.clone(),
            downstream: snapshot.downstream,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiPendingMissSnapshot {
    original: MemoryRequest,
    downstream: MemoryRequestId,
}

impl MesiPendingMissSnapshot {
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
pub struct MesiCacheControllerSnapshot {
    line: MesiCacheLine,
    layout: CacheLineLayout,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<MesiPendingMissSnapshot>,
}

impl MesiCacheControllerSnapshot {
    pub fn new(
        line: MesiCacheLine,
        layout: CacheLineLayout,
        next_sequence: u64,
        data: Option<Vec<u8>>,
        pending: Option<MesiPendingMissSnapshot>,
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

    pub const fn line(&self) -> MesiLineId {
        self.line.line()
    }

    pub const fn state(&self) -> MesiState {
        self.line.state()
    }

    pub fn line_state(&self) -> &MesiCacheLine {
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

    pub fn pending(&self) -> Option<&MesiPendingMissSnapshot> {
        self.pending.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiCacheController {
    agent: AgentId,
    layout: CacheLineLayout,
    line: MesiCacheLine,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<PendingMesiMiss>,
}

impl MesiCacheController {
    pub fn new(agent: AgentId, layout: CacheLineLayout, line_address: Address) -> Self {
        Self {
            agent,
            layout,
            line: MesiCacheLine::new(agent, MesiLineId::new(layout.line_address(line_address))),
            next_sequence: 0,
            data: None,
            pending: None,
        }
    }

    pub fn state(&self) -> MesiState {
        self.line.state()
    }

    pub fn line(&self) -> MesiLineId {
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

    pub fn snapshot(&self) -> MesiCacheControllerSnapshot {
        MesiCacheControllerSnapshot::new(
            self.line.clone(),
            self.layout,
            self.next_sequence,
            self.data.clone(),
            self.pending.as_ref().map(PendingMesiMiss::snapshot),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &MesiCacheControllerSnapshot,
    ) -> Result<(), MesiCacheControllerError> {
        self.validate_snapshot_identity(snapshot)?;
        self.validate_snapshot_data(snapshot)?;
        self.line = snapshot.line.clone();
        self.next_sequence = snapshot.next_sequence;
        self.data = snapshot.data.clone();
        self.pending = snapshot
            .pending
            .as_ref()
            .map(PendingMesiMiss::from_snapshot);
        Ok(())
    }

    pub fn install_shared(&mut self, data: Vec<u8>) -> Result<(), MesiCacheControllerError> {
        self.install_stable(MesiState::Shared, data)
    }

    pub fn install_exclusive(&mut self, data: Vec<u8>) -> Result<(), MesiCacheControllerError> {
        self.install_stable(MesiState::Exclusive, data)
    }

    pub fn install_modified(&mut self, data: Vec<u8>) -> Result<(), MesiCacheControllerError> {
        self.install_stable(MesiState::Modified, data)
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<MesiCacheControllerResult, MesiCacheControllerError> {
        self.check_request_line(&request)?;
        if self.line.state().is_transient() {
            return Err(MesiCacheControllerError::LineBusy {
                state: self.line.state(),
            });
        }

        let event = mesi_cpu_event(&request);
        let transition = self
            .line
            .apply(event)
            .map_err(MesiCacheControllerError::Protocol)?;
        if transition.after().is_transient() {
            let downstream = self.downstream_request(&request, transition.before())?;
            self.pending = Some(PendingMesiMiss {
                original: request,
                downstream: downstream.id(),
            });
            return Ok(MesiCacheControllerResult::new(
                MesiCacheControllerResultKind::Miss,
                self.line.state(),
                Some(transition),
                Some(downstream),
                None,
            ));
        }

        let response = self.complete_cpu_request(&request)?;
        Ok(MesiCacheControllerResult::new(
            MesiCacheControllerResultKind::Hit,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
        event: MesiEvent,
    ) -> Result<MesiCacheControllerResult, MesiCacheControllerError> {
        if !event.is_data_response() {
            return Err(MesiCacheControllerError::UnexpectedFillEvent { event });
        }
        let pending = self
            .pending
            .take()
            .ok_or(MesiCacheControllerError::NoPendingMiss)?;
        if response.request_id() != pending.downstream {
            let expected = pending.downstream;
            self.pending = Some(pending);
            return Err(MesiCacheControllerError::UnexpectedFill {
                expected,
                actual: response.request_id(),
            });
        }

        let transition = self
            .line
            .apply(event)
            .map_err(MesiCacheControllerError::Protocol)?;
        if let Some(data) = response.data() {
            self.install_data(data.to_vec())?;
        }
        if pending.original.carries_data() {
            self.apply_store(&pending.original)?;
        }

        let response = self.complete_cpu_request(&pending.original)?;
        Ok(MesiCacheControllerResult::new(
            MesiCacheControllerResultKind::Fill,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_snoop(
        &mut self,
        event: MesiEvent,
    ) -> Result<MesiCacheControllerResult, MesiCacheControllerError> {
        let transition = self
            .line
            .apply(event)
            .map_err(MesiCacheControllerError::Protocol)?;
        if self.line.state() == MesiState::Invalid {
            self.data = None;
        }

        Ok(MesiCacheControllerResult::new(
            MesiCacheControllerResultKind::Snoop,
            self.line.state(),
            Some(transition),
            None,
            None,
        ))
    }

    fn install_stable(
        &mut self,
        state: MesiState,
        data: Vec<u8>,
    ) -> Result<(), MesiCacheControllerError> {
        self.line
            .force_state(state)
            .map_err(MesiCacheControllerError::Protocol)?;
        self.install_data(data)
    }

    fn install_data(&mut self, data: Vec<u8>) -> Result<(), MesiCacheControllerError> {
        if data.len() as u64 != self.layout.bytes() {
            return Err(MesiCacheControllerError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = Some(data);
        Ok(())
    }

    fn check_request_line(&self, request: &MemoryRequest) -> Result<(), MesiCacheControllerError> {
        let actual = MesiLineId::new(request.line_address());
        let expected = self.line.line();
        if actual != expected {
            return Err(MesiCacheControllerError::WrongLine { expected, actual });
        }

        Ok(())
    }

    fn downstream_request(
        &mut self,
        request: &MemoryRequest,
        before: MesiState,
    ) -> Result<MemoryRequest, MesiCacheControllerError> {
        let id = MemoryRequestId::new(self.agent, self.next_sequence);
        self.next_sequence += 1;
        let line_address = self.line.line().address();
        let line_size =
            AccessSize::new(self.layout.bytes()).map_err(MesiCacheControllerError::Memory)?;

        let downstream = match (mesi_cpu_event(request), before) {
            (MesiEvent::CpuRead, _) => {
                MemoryRequest::read_shared(id, line_address, line_size, self.layout)
                    .map_err(MesiCacheControllerError::Memory)
            }
            (MesiEvent::CpuWrite, MesiState::Shared) => MemoryRequest::upgrade(
                id,
                line_address,
                AccessSize::new(1).map_err(MesiCacheControllerError::Memory)?,
                self.layout,
            )
            .map_err(MesiCacheControllerError::Memory),
            (MesiEvent::CpuWrite, _) => {
                MemoryRequest::read_unique(id, line_address, line_size, self.layout)
                    .map_err(MesiCacheControllerError::Memory)
            }
            _ => unreachable!("only CPU requests create downstream requests"),
        }?;
        Ok(downstream::with_source_attributes(downstream, request))
    }

    fn complete_cpu_request(
        &mut self,
        request: &MemoryRequest,
    ) -> Result<MemoryResponse, MesiCacheControllerError> {
        if request.operation() == MemoryOperation::Atomic {
            let data = self.read_slice(request)?;
            let write_data = request
                .atomic_write_data(&data)
                .map_err(MesiCacheControllerError::Memory)?;
            self.apply_store_data(request, &write_data)?;
            return MemoryResponse::completed(request, Some(data))
                .map_err(MesiCacheControllerError::Memory);
        }

        if request.carries_data() {
            self.apply_store(request)?;
            return MemoryResponse::completed(request, None)
                .map_err(MesiCacheControllerError::Memory);
        }

        if request.returns_data() {
            let data = self.read_slice(request)?;
            return MemoryResponse::completed(request, Some(data))
                .map_err(MesiCacheControllerError::Memory);
        }

        MemoryResponse::completed(request, None).map_err(MesiCacheControllerError::Memory)
    }

    fn apply_store(&mut self, request: &MemoryRequest) -> Result<(), MesiCacheControllerError> {
        let payload = request.data().ok_or(MesiCacheControllerError::Memory(
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
    ) -> Result<(), MesiCacheControllerError> {
        let offset = self.checked_offset(request)?;
        let line = self.line();
        let data = self
            .data
            .as_mut()
            .ok_or(MesiCacheControllerError::LineDataUnavailable { line })?;
        let mask = request.byte_mask();

        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn read_slice(&self, request: &MemoryRequest) -> Result<Vec<u8>, MesiCacheControllerError> {
        let offset = self.checked_offset(request)?;
        let data = self
            .data
            .as_ref()
            .ok_or(MesiCacheControllerError::LineDataUnavailable { line: self.line() })?;
        Ok(data[offset..offset + request.size().bytes() as usize].to_vec())
    }

    fn checked_offset(&self, request: &MemoryRequest) -> Result<usize, MesiCacheControllerError> {
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        if offset + size > self.layout.bytes() as usize {
            return Err(MesiCacheControllerError::AccessOutsideLine {
                line: self.line(),
                address: request.range().start(),
                size: request.size(),
            });
        }

        Ok(offset)
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &MesiCacheControllerSnapshot,
    ) -> Result<(), MesiCacheControllerError> {
        if self.agent == snapshot.agent()
            && self.line() == snapshot.line()
            && self.layout == snapshot.layout()
        {
            return Ok(());
        }

        Err(MesiCacheControllerError::SnapshotIdentityMismatch {
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
        snapshot: &MesiCacheControllerSnapshot,
    ) -> Result<(), MesiCacheControllerError> {
        if let Some(data) = snapshot.cached_data() {
            if data.len() as u64 != self.layout.bytes() {
                return Err(MesiCacheControllerError::LineDataSizeMismatch {
                    expected: self.layout.bytes(),
                    actual: data.len() as u64,
                });
            }
        }

        Ok(())
    }
}

fn mesi_cpu_event(request: &MemoryRequest) -> MesiEvent {
    match request.operation() {
        MemoryOperation::InstructionFetch | MemoryOperation::ReadShared => MesiEvent::CpuRead,
        MemoryOperation::ReadUnique
        | MemoryOperation::Write
        | MemoryOperation::Upgrade
        | MemoryOperation::Atomic
        | MemoryOperation::PrefetchWrite => MesiEvent::CpuWrite,
        MemoryOperation::PrefetchRead => MesiEvent::CpuRead,
        MemoryOperation::WriteClean
        | MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanShared
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate
        | MemoryOperation::InvalidateWritable => MesiEvent::CpuWrite,
    }
}
