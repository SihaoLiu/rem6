use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_msi::{MsiEvent, MsiLineId, MsiState};
use rem6_transport::TargetOutcome;

use crate::{
    CacheControllerError, CacheControllerResult, CacheControllerResultKind, MshrCompletion,
    MshrHandle, MshrQosClass, MshrQueue, MshrQueueConfig, MshrQueueError, MshrQueueSnapshot,
    MshrTargetSource, MsiCacheController, MsiCacheControllerSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MsiCacheBankError {
    Controller(CacheControllerError),
    Mshr(MshrQueueError),
    WrongAgent {
        expected: AgentId,
        actual: AgentId,
    },
    UnknownPendingFill {
        response: MemoryRequestId,
    },
    UnknownSnoopLine {
        line: Address,
    },
    SnapshotIdentityMismatch {
        expected_agent: AgentId,
        actual_agent: AgentId,
        expected_layout: CacheLineLayout,
        actual_layout: CacheLineLayout,
    },
    SnapshotMshrModeMismatch {
        snapshot_has_mshr: bool,
        bank_has_mshr: bool,
    },
    DuplicateSnapshotLine {
        line: Address,
    },
    DuplicateSnapshotPendingFill {
        response: MemoryRequestId,
    },
}

impl fmt::Display for MsiCacheBankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Controller(error) => write!(formatter, "{error}"),
            Self::Mshr(error) => write!(formatter, "{error}"),
            Self::WrongAgent { expected, actual } => write!(
                formatter,
                "MSI cache bank for agent {} cannot accept request from agent {}",
                expected.get(),
                actual.get()
            ),
            Self::UnknownPendingFill { response } => write!(
                formatter,
                "MSI cache bank has no pending fill for response {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::UnknownSnoopLine { line } => write!(
                formatter,
                "MSI cache bank has no resident line {:#x} for snoop",
                line.get()
            ),
            Self::SnapshotIdentityMismatch {
                expected_agent,
                actual_agent,
                expected_layout,
                actual_layout,
            } => write!(
                formatter,
                "MSI cache bank snapshot for agent {}, line size {} cannot restore bank for agent {}, line size {}",
                actual_agent.get(),
                actual_layout.bytes(),
                expected_agent.get(),
                expected_layout.bytes()
            ),
            Self::SnapshotMshrModeMismatch {
                snapshot_has_mshr,
                bank_has_mshr,
            } => write!(
                formatter,
                "MSI cache bank snapshot MSHR mode {snapshot_has_mshr} cannot restore bank MSHR mode {bank_has_mshr}"
            ),
            Self::DuplicateSnapshotLine { line } => {
                write!(formatter, "MSI cache bank snapshot repeats line {:#x}", line.get())
            }
            Self::DuplicateSnapshotPendingFill { response } => write!(
                formatter,
                "MSI cache bank snapshot repeats pending fill {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
        }
    }
}

impl Error for MsiCacheBankError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Controller(error) => Some(error),
            Self::Mshr(error) => Some(error),
            Self::WrongAgent { .. }
            | Self::UnknownPendingFill { .. }
            | Self::UnknownSnoopLine { .. }
            | Self::SnapshotIdentityMismatch { .. }
            | Self::SnapshotMshrModeMismatch { .. }
            | Self::DuplicateSnapshotLine { .. }
            | Self::DuplicateSnapshotPendingFill { .. } => None,
        }
    }
}

impl From<CacheControllerError> for MsiCacheBankError {
    fn from(error: CacheControllerError) -> Self {
        Self::Controller(error)
    }
}

impl From<MshrQueueError> for MsiCacheBankError {
    fn from(error: MshrQueueError) -> Self {
        Self::Mshr(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiCacheBankSnapshot {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: Vec<MsiCacheControllerSnapshot>,
    mshr: Option<MshrQueueSnapshot>,
}

impl MsiCacheBankSnapshot {
    pub fn new(
        agent: AgentId,
        layout: CacheLineLayout,
        next_sequence: u64,
        lines: Vec<MsiCacheControllerSnapshot>,
    ) -> Self {
        Self {
            agent,
            layout,
            next_sequence,
            lines,
            mshr: None,
        }
    }

    pub fn new_with_mshr(
        agent: AgentId,
        layout: CacheLineLayout,
        next_sequence: u64,
        lines: Vec<MsiCacheControllerSnapshot>,
        mshr: MshrQueueSnapshot,
    ) -> Self {
        Self {
            agent,
            layout,
            next_sequence,
            lines,
            mshr: Some(mshr),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn lines(&self) -> &[MsiCacheControllerSnapshot] {
        &self.lines
    }

    pub fn mshr(&self) -> Option<&MshrQueueSnapshot> {
        self.mshr.as_ref()
    }

    pub fn line_addresses(&self) -> Vec<Address> {
        self.lines
            .iter()
            .map(|line| line.line().address())
            .collect()
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingBankFill {
    line: Address,
    mshr: Option<MshrHandle>,
}

#[derive(Clone, Debug)]
pub struct MsiCacheBank {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: BTreeMap<Address, MsiCacheController>,
    pending_fills: BTreeMap<MemoryRequestId, PendingBankFill>,
    mshr: Option<MshrQueue>,
}

impl MsiCacheBank {
    pub fn new(agent: AgentId, layout: CacheLineLayout) -> Self {
        Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            mshr: None,
        }
    }

    pub fn new_with_mshr(
        agent: AgentId,
        layout: CacheLineLayout,
        mshr_config: MshrQueueConfig,
    ) -> Self {
        Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            mshr: Some(MshrQueue::new(mshr_config)),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn pending_fill_count(&self) -> usize {
        self.pending_fills.len()
    }

    pub fn line_addresses(&self) -> Vec<Address> {
        self.lines.keys().copied().collect()
    }

    pub fn pending_fill_line(&self, response: MemoryRequestId) -> Option<Address> {
        self.pending_fills
            .get(&response)
            .map(|pending| pending.line)
    }

    pub fn mshr_allocated_count(&self) -> usize {
        self.mshr.as_ref().map_or(0, MshrQueue::allocated_count)
    }

    pub fn mshr_target_count(&self, address: Address) -> Option<usize> {
        let line = self.layout.line_address(address);
        self.mshr
            .as_ref()
            .and_then(|mshr| mshr.find_line(line))
            .map(|entry| entry.target_count())
    }

    pub fn mshr_effective_qos(&self, address: Address) -> Option<MshrQosClass> {
        let line = self.layout.line_address(address);
        self.mshr
            .as_ref()
            .and_then(|mshr| mshr.find_line(line))
            .and_then(|entry| entry.effective_qos())
    }

    pub fn state(&self, address: Address) -> Option<MsiState> {
        self.lines
            .get(&self.layout.line_address(address))
            .map(MsiCacheController::state)
    }

    pub fn cached_data(&self, address: Address) -> Option<&[u8]> {
        self.lines
            .get(&self.layout.line_address(address))
            .and_then(MsiCacheController::cached_data)
    }

    pub fn snapshot(&self) -> MsiCacheBankSnapshot {
        let lines = self
            .lines
            .values()
            .map(MsiCacheController::snapshot)
            .collect();
        match &self.mshr {
            Some(mshr) => MsiCacheBankSnapshot::new_with_mshr(
                self.agent,
                self.layout,
                self.next_sequence,
                lines,
                mshr.snapshot(),
            ),
            None => MsiCacheBankSnapshot::new(self.agent, self.layout, self.next_sequence, lines),
        }
    }

    pub fn restore(&mut self, snapshot: &MsiCacheBankSnapshot) -> Result<(), MsiCacheBankError> {
        if snapshot.agent() != self.agent || snapshot.layout() != self.layout {
            return Err(MsiCacheBankError::SnapshotIdentityMismatch {
                expected_agent: self.agent,
                actual_agent: snapshot.agent(),
                expected_layout: self.layout,
                actual_layout: snapshot.layout(),
            });
        }

        let restored_mshr = match (&self.mshr, snapshot.mshr()) {
            (Some(mshr), Some(snapshot_mshr)) => {
                let mut restored = MshrQueue::new(mshr.config().clone());
                restored.restore(snapshot_mshr)?;
                Some(restored)
            }
            (Some(mshr), None) => Some(MshrQueue::new(mshr.config().clone())),
            (None, None) => None,
            (None, Some(_)) => {
                return Err(MsiCacheBankError::SnapshotMshrModeMismatch {
                    snapshot_has_mshr: true,
                    bank_has_mshr: false,
                });
            }
        };

        let mut lines = BTreeMap::new();
        let mut pending_fills = BTreeMap::new();
        for line_snapshot in snapshot.lines() {
            let line = line_snapshot.line().address();
            let mut controller = MsiCacheController::new(self.agent, self.layout, line);
            controller.restore(line_snapshot)?;
            if lines.insert(line, controller).is_some() {
                return Err(MsiCacheBankError::DuplicateSnapshotLine { line });
            }
            if let Some(pending) = line_snapshot.pending() {
                let response = pending.downstream();
                let mshr = restored_mshr
                    .as_ref()
                    .and_then(|mshr| mshr.find_line(line))
                    .map(|entry| entry.handle());
                let pending = PendingBankFill { line, mshr };
                if pending_fills.insert(response, pending).is_some() {
                    return Err(MsiCacheBankError::DuplicateSnapshotPendingFill { response });
                }
            }
        }

        self.next_sequence = snapshot.next_sequence();
        self.lines = lines;
        self.pending_fills = pending_fills;
        self.mshr = restored_mshr;
        Ok(())
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        self.accept_cpu_request_inner(request, None)
    }

    pub fn accept_cpu_request_with_qos(
        &mut self,
        request: MemoryRequest,
        qos: MshrQosClass,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        self.accept_cpu_request_inner(request, Some(qos))
    }

    fn accept_cpu_request_inner(
        &mut self,
        request: MemoryRequest,
        qos: Option<MshrQosClass>,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        self.validate_request_agent(&request)?;
        let line = request.line_address();
        if self.can_merge_pending_read_miss(line, &request) {
            let state = self
                .lines
                .get(&line)
                .map_or(MsiState::Invalid, MsiCacheController::state);
            self.mshr
                .as_mut()
                .expect("MSHR lookup succeeded but bank has no MSHR queue")
                .allocate_or_merge_optional_qos(request, 0, MshrTargetSource::Demand, true, qos)?;
            return Ok(CacheControllerResult::new(
                CacheControllerResultKind::Miss,
                state,
                None,
                None,
                None,
            ));
        }

        if self.should_preflight_mshr_allocation(line, &request) {
            self.mshr
                .as_ref()
                .expect("MSHR preflight requested but bank has no MSHR queue")
                .can_allocate_entry(MshrTargetSource::Demand)?;
        }

        let original = request.clone();
        let controller = self
            .lines
            .entry(line)
            .or_insert_with(|| MsiCacheController::new(self.agent, self.layout, line));
        controller.set_next_sequence(self.next_sequence);
        let result = controller.accept_cpu_request(request)?;
        self.next_sequence = controller.next_sequence();
        if let Some(downstream) = result.downstream_request() {
            let mshr = match &mut self.mshr {
                Some(mshr) => Some(
                    mshr.allocate_or_merge_optional_qos(
                        original,
                        0,
                        MshrTargetSource::Demand,
                        true,
                        qos,
                    )?
                    .handle(),
                ),
                None => None,
            };
            self.pending_fills
                .insert(downstream.id(), PendingBankFill { line, mshr });
        }
        Ok(result)
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        let response_id = response.request_id();
        let pending =
            *self
                .pending_fills
                .get(&response_id)
                .ok_or(MsiCacheBankError::UnknownPendingFill {
                    response: response_id,
                })?;
        let controller = self
            .lines
            .get_mut(&pending.line)
            .expect("pending fill references an existing MSI cache line");
        let result = controller.accept_fill(response)?;
        self.pending_fills.remove(&response_id);

        let completion = match pending.mshr {
            Some(handle) => {
                let mshr =
                    self.mshr
                        .as_mut()
                        .ok_or(MsiCacheBankError::SnapshotMshrModeMismatch {
                            snapshot_has_mshr: true,
                            bank_has_mshr: false,
                        })?;
                Some(mshr.complete(handle)?)
            }
            None => None,
        };

        if let Some(completion) = completion {
            let outcomes = self.target_outcomes_for_completion(&completion)?;
            return Ok(result.with_target_outcomes(outcomes));
        }

        Ok(result)
    }

    pub fn accept_snoop(
        &mut self,
        address: Address,
        event: MsiEvent,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        let line = self.layout.line_address(address);
        let controller = self
            .lines
            .get_mut(&line)
            .ok_or(MsiCacheBankError::UnknownSnoopLine { line })?;
        controller.accept_snoop(event).map_err(Into::into)
    }

    fn validate_request_agent(&self, request: &MemoryRequest) -> Result<(), MsiCacheBankError> {
        let actual = request.id().agent();
        if actual != self.agent {
            return Err(MsiCacheBankError::WrongAgent {
                expected: self.agent,
                actual,
            });
        }

        Ok(())
    }

    fn can_merge_pending_read_miss(&self, line: Address, request: &MemoryRequest) -> bool {
        request_is_coalescible_read(request)
            && self
                .mshr
                .as_ref()
                .and_then(|mshr| mshr.find_line(line))
                .is_some()
    }

    fn should_preflight_mshr_allocation(&self, line: Address, request: &MemoryRequest) -> bool {
        if self.mshr.is_none()
            || self
                .mshr
                .as_ref()
                .is_some_and(|mshr| mshr.find_line(line).is_some())
        {
            return false;
        }

        match self.lines.get(&line).map(MsiCacheController::state) {
            None | Some(MsiState::Invalid) => true,
            Some(MsiState::Shared) => request_is_msi_write(request),
            Some(MsiState::Modified) => false,
            Some(state) if state.is_transient() => false,
            Some(_) => false,
        }
    }

    fn target_outcomes_for_completion(
        &self,
        completion: &MshrCompletion,
    ) -> Result<Vec<TargetOutcome>, MsiCacheBankError> {
        completion
            .targets()
            .iter()
            .map(|target| self.complete_mshr_target(target.request()))
            .collect()
    }

    fn complete_mshr_target(
        &self,
        request: &MemoryRequest,
    ) -> Result<TargetOutcome, MsiCacheBankError> {
        if !request.requires_response() {
            return Ok(TargetOutcome::NoResponse);
        }

        let data = if request.returns_data() {
            Some(self.cached_slice_for_request(request)?)
        } else {
            None
        };
        let response =
            MemoryResponse::completed(request, data).map_err(CacheControllerError::Memory)?;
        Ok(TargetOutcome::Respond(response))
    }

    fn cached_slice_for_request(
        &self,
        request: &MemoryRequest,
    ) -> Result<Vec<u8>, CacheControllerError> {
        let line = request.line_address();
        let data = self
            .lines
            .get(&line)
            .and_then(MsiCacheController::cached_data)
            .ok_or(CacheControllerError::LineDataUnavailable {
                line: MsiLineId::new(line),
            })?;
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        if offset + size > data.len() {
            return Err(CacheControllerError::AccessOutsideLine {
                line: MsiLineId::new(line),
                address: request.range().start(),
                size: request.size(),
            });
        }

        Ok(data[offset..offset + size].to_vec())
    }
}

fn request_is_coalescible_read(request: &MemoryRequest) -> bool {
    matches!(
        request.operation(),
        MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::PrefetchRead
    ) && request.line_span() == 1
}

fn request_is_msi_write(request: &MemoryRequest) -> bool {
    !matches!(
        request.operation(),
        MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::PrefetchRead
    )
}
