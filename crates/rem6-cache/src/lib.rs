use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::{MsiCacheLine, MsiError, MsiEvent, MsiLineId, MsiState, MsiTransition};
use rem6_transport::TargetOutcome;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheControllerResultKind {
    Hit,
    Miss,
    Fill,
    Snoop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheControllerError {
    WrongLine {
        expected: MsiLineId,
        actual: MsiLineId,
    },
    LineBusy {
        state: MsiState,
    },
    NoPendingMiss,
    UnexpectedFill {
        expected: MemoryRequestId,
        actual: MemoryRequestId,
    },
    LineDataUnavailable {
        line: MsiLineId,
    },
    LineDataSizeMismatch {
        expected: u64,
        actual: u64,
    },
    AccessOutsideLine {
        line: MsiLineId,
        address: Address,
        size: AccessSize,
    },
    Memory(MemoryError),
    Protocol(MsiError),
}

impl fmt::Display for CacheControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLine { expected, actual } => write!(
                formatter,
                "request for line {:#x} reached controller for line {:#x}",
                actual.address().get(),
                expected.address().get()
            ),
            Self::LineBusy { state } => write!(formatter, "cache line is busy in {state:?}"),
            Self::NoPendingMiss => write!(formatter, "cache line has no pending miss"),
            Self::UnexpectedFill { expected, actual } => write!(
                formatter,
                "fill response {:?} does not match pending request {:?}",
                actual, expected
            ),
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

impl Error for CacheControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheControllerResult {
    kind: CacheControllerResultKind,
    state: MsiState,
    transition: Option<MsiTransition>,
    downstream_request: Option<MemoryRequest>,
    target_outcome: Option<TargetOutcome>,
}

impl CacheControllerResult {
    fn new(
        kind: CacheControllerResultKind,
        state: MsiState,
        transition: Option<MsiTransition>,
        downstream_request: Option<MemoryRequest>,
        target_outcome: Option<TargetOutcome>,
    ) -> Self {
        Self {
            kind,
            state,
            transition,
            downstream_request,
            target_outcome,
        }
    }

    pub const fn kind(&self) -> CacheControllerResultKind {
        self.kind
    }

    pub const fn state(&self) -> MsiState {
        self.state
    }

    pub fn transition(&self) -> Option<&MsiTransition> {
        self.transition.as_ref()
    }

    pub fn downstream_request(&self) -> Option<&MemoryRequest> {
        self.downstream_request.as_ref()
    }

    pub fn target_outcome(&self) -> Option<&TargetOutcome> {
        self.target_outcome.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingMiss {
    original: MemoryRequest,
    downstream: MemoryRequestId,
    fill_event: MsiEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiCacheController {
    agent: AgentId,
    layout: CacheLineLayout,
    line: MsiCacheLine,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<PendingMiss>,
}

impl MsiCacheController {
    pub fn new(agent: AgentId, layout: CacheLineLayout, line_address: Address) -> Self {
        Self {
            agent,
            layout,
            line: MsiCacheLine::new(agent, MsiLineId::new(layout.line_address(line_address))),
            next_sequence: 0,
            data: None,
            pending: None,
        }
    }

    pub fn state(&self) -> MsiState {
        self.line.state()
    }

    pub fn line(&self) -> MsiLineId {
        self.line.line()
    }

    pub fn cached_data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn install_shared(&mut self, data: Vec<u8>) -> Result<(), CacheControllerError> {
        self.install_stable(MsiState::Shared, data)
    }

    pub fn install_modified(&mut self, data: Vec<u8>) -> Result<(), CacheControllerError> {
        self.install_stable(MsiState::Modified, data)
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<CacheControllerResult, CacheControllerError> {
        self.check_request_line(&request)?;
        if self.line.state().is_transient() {
            return Err(CacheControllerError::LineBusy {
                state: self.line.state(),
            });
        }

        let event = cpu_event(&request);
        let transition = self
            .line
            .apply(event)
            .map_err(CacheControllerError::Protocol)?;
        if transition.after().is_transient() {
            let downstream = self.downstream_request(&request, transition.before())?;
            let fill_event = match transition.after() {
                MsiState::InvalidToShared => MsiEvent::DataShared,
                MsiState::InvalidToModified | MsiState::SharedToModified => MsiEvent::DataModified,
                _ => unreachable!("miss transition is transient"),
            };
            self.pending = Some(PendingMiss {
                original: request,
                downstream: downstream.id(),
                fill_event,
            });
            return Ok(CacheControllerResult::new(
                CacheControllerResultKind::Miss,
                self.line.state(),
                Some(transition),
                Some(downstream),
                None,
            ));
        }

        let response = self.complete_cpu_request(&request)?;
        Ok(CacheControllerResult::new(
            CacheControllerResultKind::Hit,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
    ) -> Result<CacheControllerResult, CacheControllerError> {
        let pending = self
            .pending
            .take()
            .ok_or(CacheControllerError::NoPendingMiss)?;
        if response.request_id() != pending.downstream {
            let expected = pending.downstream;
            self.pending = Some(pending);
            return Err(CacheControllerError::UnexpectedFill {
                expected,
                actual: response.request_id(),
            });
        }

        let transition = self
            .line
            .apply(pending.fill_event)
            .map_err(CacheControllerError::Protocol)?;
        if let Some(data) = response.data() {
            self.install_data(data.to_vec())?;
        }
        if pending.original.carries_data() {
            self.apply_store(&pending.original)?;
        }

        let response = self.complete_cpu_request(&pending.original)?;
        Ok(CacheControllerResult::new(
            CacheControllerResultKind::Fill,
            self.line.state(),
            Some(transition),
            None,
            Some(TargetOutcome::Respond(response)),
        ))
    }

    pub fn accept_snoop(
        &mut self,
        event: MsiEvent,
    ) -> Result<CacheControllerResult, CacheControllerError> {
        let transition = self
            .line
            .apply(event)
            .map_err(CacheControllerError::Protocol)?;
        if self.line.state() == MsiState::Invalid {
            self.data = None;
        }

        Ok(CacheControllerResult::new(
            CacheControllerResultKind::Snoop,
            self.line.state(),
            Some(transition),
            None,
            None,
        ))
    }

    fn install_stable(
        &mut self,
        state: MsiState,
        data: Vec<u8>,
    ) -> Result<(), CacheControllerError> {
        self.line
            .force_state(state)
            .map_err(CacheControllerError::Protocol)?;
        self.install_data(data)
    }

    fn install_data(&mut self, data: Vec<u8>) -> Result<(), CacheControllerError> {
        if data.len() as u64 != self.layout.bytes() {
            return Err(CacheControllerError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = Some(data);
        Ok(())
    }

    fn check_request_line(&self, request: &MemoryRequest) -> Result<(), CacheControllerError> {
        let actual = MsiLineId::new(request.line_address());
        let expected = self.line.line();
        if actual != expected {
            return Err(CacheControllerError::WrongLine { expected, actual });
        }

        Ok(())
    }

    fn downstream_request(
        &mut self,
        request: &MemoryRequest,
        before: MsiState,
    ) -> Result<MemoryRequest, CacheControllerError> {
        let id = MemoryRequestId::new(self.agent, self.next_sequence);
        self.next_sequence += 1;
        let line_address = self.line.line().address();
        let line_size =
            AccessSize::new(self.layout.bytes()).map_err(CacheControllerError::Memory)?;

        match (cpu_event(request), before) {
            (MsiEvent::CpuRead, _) => {
                MemoryRequest::read_shared(id, line_address, line_size, self.layout)
                    .map_err(CacheControllerError::Memory)
            }
            (MsiEvent::CpuWrite, MsiState::Shared) => MemoryRequest::upgrade(
                id,
                line_address,
                AccessSize::new(1).map_err(CacheControllerError::Memory)?,
                self.layout,
            )
            .map_err(CacheControllerError::Memory),
            (MsiEvent::CpuWrite, _) => {
                MemoryRequest::read_unique(id, line_address, line_size, self.layout)
                    .map_err(CacheControllerError::Memory)
            }
            _ => unreachable!("only CPU requests create downstream requests"),
        }
    }

    fn complete_cpu_request(
        &mut self,
        request: &MemoryRequest,
    ) -> Result<MemoryResponse, CacheControllerError> {
        if request.carries_data() {
            self.apply_store(request)?;
            return MemoryResponse::completed(request, None).map_err(CacheControllerError::Memory);
        }

        if request.returns_data() {
            let data = self.read_slice(request)?;
            return MemoryResponse::completed(request, Some(data))
                .map_err(CacheControllerError::Memory);
        }

        MemoryResponse::completed(request, None).map_err(CacheControllerError::Memory)
    }

    fn apply_store(&mut self, request: &MemoryRequest) -> Result<(), CacheControllerError> {
        let offset = self.checked_offset(request)?;
        let line = self.line();
        let data = self
            .data
            .as_mut()
            .ok_or(CacheControllerError::LineDataUnavailable { line })?;
        let payload = request.data().ok_or(CacheControllerError::Memory(
            MemoryError::MissingRequestData {
                request: request.id(),
            },
        ))?;
        let mask = request.byte_mask();

        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn read_slice(&self, request: &MemoryRequest) -> Result<Vec<u8>, CacheControllerError> {
        let offset = self.checked_offset(request)?;
        let data = self
            .data
            .as_ref()
            .ok_or(CacheControllerError::LineDataUnavailable { line: self.line() })?;
        Ok(data[offset..offset + request.size().bytes() as usize].to_vec())
    }

    fn checked_offset(&self, request: &MemoryRequest) -> Result<usize, CacheControllerError> {
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        if offset + size > self.layout.bytes() as usize {
            return Err(CacheControllerError::AccessOutsideLine {
                line: self.line(),
                address: request.range().start(),
                size: request.size(),
            });
        }

        Ok(offset)
    }
}

fn cpu_event(request: &MemoryRequest) -> MsiEvent {
    match request.operation() {
        MemoryOperation::InstructionFetch | MemoryOperation::ReadShared => MsiEvent::CpuRead,
        MemoryOperation::ReadUnique
        | MemoryOperation::Write
        | MemoryOperation::Upgrade
        | MemoryOperation::Atomic
        | MemoryOperation::PrefetchWrite => MsiEvent::CpuWrite,
        MemoryOperation::PrefetchRead => MsiEvent::CpuRead,
        MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate => MsiEvent::CpuWrite,
    }
}
