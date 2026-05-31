use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::{MsiEvent, MsiLineId, MsiState};
use rem6_transport::TargetOutcome;

use crate::{
    CacheControllerError, CacheControllerResult, CacheControllerResultKind, CacheWriteQueue,
    CacheWriteQueueConfig, CacheWriteQueueEntryKind, CacheWriteQueueError, CacheWriteQueueHandle,
    CacheWriteQueueIssue, CacheWriteQueueSnapshot, CacheWriteQueueUpdate, MshrCompletion,
    MshrHandle, MshrQosClass, MshrQosProfile, MshrQueue, MshrQueueConfig, MshrQueueError,
    MshrQueueSnapshot, MshrTargetSource, MsiCacheController, MsiCacheControllerSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MsiCacheBankError {
    Controller(CacheControllerError),
    Mshr(MshrQueueError),
    WriteQueue(CacheWriteQueueError),
    WrongAgent {
        expected: AgentId,
        actual: AgentId,
    },
    WriteQueueDisabled,
    WriteQueueConflict {
        line: Address,
    },
    PendingUncacheableConflict {
        line: Address,
    },
    UncacheableBypassRequiresCleanLine {
        line: Address,
    },
    UnknownPendingFill {
        response: MemoryRequestId,
    },
    UnknownUncacheableWriteResponse {
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
    SnapshotWriteQueueModeMismatch {
        snapshot_has_write_queue: bool,
        bank_has_write_queue: bool,
    },
    SnapshotPendingUncacheableRequestMismatch {
        response: MemoryRequestId,
        operation: MemoryOperation,
        uncacheable: bool,
    },
    SnapshotPendingUncacheableReadWritebackMismatch {
        response: MemoryRequestId,
        handle: CacheWriteQueueHandle,
    },
    DuplicateSnapshotLine {
        line: Address,
    },
    DuplicateSnapshotPendingFill {
        response: MemoryRequestId,
    },
    SnapshotInflightUncacheableWriteMismatch {
        response: MemoryRequestId,
        operation: MemoryOperation,
        uncacheable: bool,
    },
    DuplicateSnapshotUncacheableWrite {
        response: MemoryRequestId,
    },
}

impl fmt::Display for MsiCacheBankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Controller(error) => write!(formatter, "{error}"),
            Self::Mshr(error) => write!(formatter, "{error}"),
            Self::WriteQueue(error) => write!(formatter, "{error}"),
            Self::WrongAgent { expected, actual } => write!(
                formatter,
                "MSI cache bank for agent {} cannot accept request from agent {}",
                expected.get(),
                actual.get()
            ),
            Self::WriteQueueDisabled => write!(formatter, "MSI cache bank has no write queue"),
            Self::WriteQueueConflict { line } => write!(
                formatter,
                "MSI cache bank has pending write-queue work for line {:#x}",
                line.get()
            ),
            Self::PendingUncacheableConflict { line } => write!(
                formatter,
                "MSI cache bank has pending uncacheable request for line {:#x}",
                line.get()
            ),
            Self::UncacheableBypassRequiresCleanLine { line } => write!(
                formatter,
                "MSI cache bank cannot bypass uncacheable request over non-clean resident line {:#x}",
                line.get()
            ),
            Self::UnknownPendingFill { response } => write!(
                formatter,
                "MSI cache bank has no pending fill for response {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::UnknownUncacheableWriteResponse { response } => write!(
                formatter,
                "MSI cache bank has no in-flight uncacheable write for response {} from agent {}",
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
            Self::SnapshotWriteQueueModeMismatch {
                snapshot_has_write_queue,
                bank_has_write_queue,
            } => write!(
                formatter,
                "MSI cache bank snapshot write queue mode {snapshot_has_write_queue} cannot restore bank write queue mode {bank_has_write_queue}"
            ),
            Self::SnapshotPendingUncacheableRequestMismatch {
                response,
                operation,
                uncacheable,
            } => write!(
                formatter,
                "MSI cache bank snapshot pending uncacheable request {} from agent {} has operation {:?} and uncacheable flag {}",
                response.sequence(),
                response.agent().get(),
                operation,
                uncacheable
            ),
            Self::SnapshotPendingUncacheableReadWritebackMismatch { response, handle } => write!(
                formatter,
                "MSI cache bank snapshot pending uncacheable read {} from agent {} references invalid blocking writeback {:?}",
                response.sequence(),
                response.agent().get(),
                handle
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
            Self::SnapshotInflightUncacheableWriteMismatch {
                response,
                operation,
                uncacheable,
            } => write!(
                formatter,
                "MSI cache bank snapshot in-flight uncacheable write {} from agent {} has operation {:?} and uncacheable flag {}",
                response.sequence(),
                response.agent().get(),
                operation,
                uncacheable
            ),
            Self::DuplicateSnapshotUncacheableWrite { response } => write!(
                formatter,
                "MSI cache bank snapshot repeats in-flight uncacheable write {} from agent {}",
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
            Self::WriteQueue(error) => Some(error),
            Self::WrongAgent { .. }
            | Self::WriteQueueDisabled
            | Self::WriteQueueConflict { .. }
            | Self::PendingUncacheableConflict { .. }
            | Self::UncacheableBypassRequiresCleanLine { .. }
            | Self::UnknownPendingFill { .. }
            | Self::UnknownUncacheableWriteResponse { .. }
            | Self::UnknownSnoopLine { .. }
            | Self::SnapshotIdentityMismatch { .. }
            | Self::SnapshotMshrModeMismatch { .. }
            | Self::SnapshotWriteQueueModeMismatch { .. }
            | Self::SnapshotPendingUncacheableRequestMismatch { .. }
            | Self::SnapshotPendingUncacheableReadWritebackMismatch { .. }
            | Self::DuplicateSnapshotLine { .. }
            | Self::DuplicateSnapshotPendingFill { .. }
            | Self::SnapshotInflightUncacheableWriteMismatch { .. }
            | Self::DuplicateSnapshotUncacheableWrite { .. } => None,
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

impl From<CacheWriteQueueError> for MsiCacheBankError {
    fn from(error: CacheWriteQueueError) -> Self {
        Self::WriteQueue(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiPendingUncacheableReadSnapshot {
    request: MemoryRequest,
    blocked_by: Option<CacheWriteQueueHandle>,
}

impl MsiPendingUncacheableReadSnapshot {
    pub fn new(request: MemoryRequest, blocked_by: Option<CacheWriteQueueHandle>) -> Self {
        Self {
            request,
            blocked_by,
        }
    }

    pub const fn request(&self) -> &MemoryRequest {
        &self.request
    }

    pub const fn blocked_by(&self) -> Option<CacheWriteQueueHandle> {
        self.blocked_by
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiCacheBankSnapshot {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: Vec<MsiCacheControllerSnapshot>,
    mshr: Option<MshrQueueSnapshot>,
    write_queue: Option<CacheWriteQueueSnapshot>,
    inflight_uncacheable_writes: Vec<MemoryRequest>,
    pending_uncacheable_reads: Vec<MsiPendingUncacheableReadSnapshot>,
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
            write_queue: None,
            inflight_uncacheable_writes: Vec::new(),
            pending_uncacheable_reads: Vec::new(),
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
            write_queue: None,
            inflight_uncacheable_writes: Vec::new(),
            pending_uncacheable_reads: Vec::new(),
        }
    }

    pub fn with_write_queue(mut self, write_queue: CacheWriteQueueSnapshot) -> Self {
        self.write_queue = Some(write_queue);
        self
    }

    pub fn with_inflight_uncacheable_writes(mut self, writes: Vec<MemoryRequest>) -> Self {
        self.inflight_uncacheable_writes = writes;
        self
    }

    pub fn with_pending_uncacheable_reads(
        mut self,
        reads: Vec<MsiPendingUncacheableReadSnapshot>,
    ) -> Self {
        self.pending_uncacheable_reads = reads;
        self
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

    pub fn write_queue(&self) -> Option<&CacheWriteQueueSnapshot> {
        self.write_queue.as_ref()
    }

    pub fn inflight_uncacheable_writes(&self) -> &[MemoryRequest] {
        &self.inflight_uncacheable_writes
    }

    pub fn inflight_uncacheable_write_count(&self) -> usize {
        self.inflight_uncacheable_writes.len()
    }

    pub fn pending_uncacheable_reads(&self) -> &[MsiPendingUncacheableReadSnapshot] {
        &self.pending_uncacheable_reads
    }

    pub fn pending_uncacheable_read_count(&self) -> usize {
        self.pending_uncacheable_reads.len()
    }

    pub fn mshr_qos_profile(&self) -> Option<MshrQosProfile> {
        self.mshr.as_ref().map(MshrQueueSnapshot::qos_profile)
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

    pub fn dirty_line_addresses(&self) -> Vec<Address> {
        self.lines
            .iter()
            .filter(|line| line.state().is_modified())
            .map(|line| line.line().address())
            .collect()
    }

    pub fn dirty_line_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.state().is_modified())
            .count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingBankFill {
    Line {
        line: Address,
        mshr: Option<MshrHandle>,
    },
    Uncacheable {
        original: MemoryRequest,
        blocked_by: Option<CacheWriteQueueHandle>,
    },
}

#[derive(Clone, Debug)]
pub struct MsiCacheBank {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: BTreeMap<Address, MsiCacheController>,
    pending_fills: BTreeMap<MemoryRequestId, PendingBankFill>,
    inflight_uncacheable_writes: BTreeMap<MemoryRequestId, MemoryRequest>,
    mshr: Option<MshrQueue>,
    write_queue: Option<CacheWriteQueue>,
}

impl MsiCacheBank {
    pub fn new(agent: AgentId, layout: CacheLineLayout) -> Self {
        Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: None,
            write_queue: None,
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
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: Some(MshrQueue::new(mshr_config)),
            write_queue: None,
        }
    }

    pub fn new_with_write_queue(
        agent: AgentId,
        layout: CacheLineLayout,
        write_queue_config: CacheWriteQueueConfig,
    ) -> Self {
        Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: None,
            write_queue: Some(CacheWriteQueue::new(write_queue_config)),
        }
    }

    pub fn new_with_mshr_and_write_queue(
        agent: AgentId,
        layout: CacheLineLayout,
        mshr_config: MshrQueueConfig,
        write_queue_config: CacheWriteQueueConfig,
    ) -> Self {
        Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: Some(MshrQueue::new(mshr_config)),
            write_queue: Some(CacheWriteQueue::new(write_queue_config)),
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

    pub fn inflight_uncacheable_write_count(&self) -> usize {
        self.inflight_uncacheable_writes.len()
    }

    pub fn line_addresses(&self) -> Vec<Address> {
        self.lines.keys().copied().collect()
    }

    pub fn pending_fill_line(&self, response: MemoryRequestId) -> Option<Address> {
        self.pending_fills
            .get(&response)
            .and_then(|pending| match pending {
                PendingBankFill::Line { line, .. } => Some(*line),
                PendingBankFill::Uncacheable { .. } => None,
            })
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

    pub fn mshr_qos_profile(&self) -> Option<MshrQosProfile> {
        self.mshr.as_ref().map(MshrQueue::qos_profile)
    }

    pub fn write_queue_allocated_count(&self) -> usize {
        self.write_queue
            .as_ref()
            .map_or(0, CacheWriteQueue::allocated_count)
    }

    pub fn write_queue_next_ready_tick(&self) -> Option<u64> {
        self.write_queue
            .as_ref()
            .and_then(CacheWriteQueue::next_ready_tick)
    }

    pub fn write_queue_ready_handles(&self, tick: u64) -> Vec<CacheWriteQueueHandle> {
        self.write_queue
            .as_ref()
            .map_or_else(Vec::new, |queue| queue.ready_handles(tick))
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
        let snapshot = match &self.mshr {
            Some(mshr) => MsiCacheBankSnapshot::new_with_mshr(
                self.agent,
                self.layout,
                self.next_sequence,
                lines,
                mshr.snapshot(),
            ),
            None => MsiCacheBankSnapshot::new(self.agent, self.layout, self.next_sequence, lines),
        };
        let snapshot = match &self.write_queue {
            Some(write_queue) => snapshot.with_write_queue(write_queue.snapshot()),
            None => snapshot,
        };
        let pending_uncacheable_reads = self
            .pending_fills
            .values()
            .filter_map(|pending| match pending {
                PendingBankFill::Line { .. } => None,
                PendingBankFill::Uncacheable {
                    original,
                    blocked_by,
                } => Some(MsiPendingUncacheableReadSnapshot::new(
                    original.clone(),
                    *blocked_by,
                )),
            })
            .collect();
        snapshot
            .with_inflight_uncacheable_writes(
                self.inflight_uncacheable_writes.values().cloned().collect(),
            )
            .with_pending_uncacheable_reads(pending_uncacheable_reads)
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

        let restored_write_queue = match (&self.write_queue, snapshot.write_queue()) {
            (Some(write_queue), Some(snapshot_write_queue)) => {
                let mut restored = CacheWriteQueue::new(write_queue.config().clone());
                restored.restore(snapshot_write_queue)?;
                Some(restored)
            }
            (Some(write_queue), None) => Some(CacheWriteQueue::new(write_queue.config().clone())),
            (None, None) => None,
            (None, Some(_)) => {
                return Err(MsiCacheBankError::SnapshotWriteQueueModeMismatch {
                    snapshot_has_write_queue: true,
                    bank_has_write_queue: false,
                });
            }
        };

        let mut lines = BTreeMap::new();
        let mut pending_fills = BTreeMap::new();
        let mut inflight_uncacheable_writes = BTreeMap::new();
        for request in snapshot.inflight_uncacheable_writes() {
            if !request.is_uncacheable()
                || !request.requires_response()
                || request.operation() != MemoryOperation::Write
            {
                return Err(
                    MsiCacheBankError::SnapshotInflightUncacheableWriteMismatch {
                        response: request.id(),
                        operation: request.operation(),
                        uncacheable: request.is_uncacheable(),
                    },
                );
            }
            self.validate_write_queue_request(request)?;
            if inflight_uncacheable_writes
                .insert(request.id(), request.clone())
                .is_some()
            {
                return Err(MsiCacheBankError::DuplicateSnapshotUncacheableWrite {
                    response: request.id(),
                });
            }
        }
        for pending_read in snapshot.pending_uncacheable_reads() {
            let request = pending_read.request();
            self.validate_pending_uncacheable_read_snapshot(
                request,
                pending_read.blocked_by(),
                restored_write_queue.as_ref(),
            )?;
            let response = request.id();
            let pending = PendingBankFill::Uncacheable {
                original: request.clone(),
                blocked_by: pending_read.blocked_by(),
            };
            if pending_fills.insert(response, pending).is_some() {
                return Err(MsiCacheBankError::DuplicateSnapshotPendingFill { response });
            }
        }
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
                let pending = PendingBankFill::Line { line, mshr };
                if pending_fills.insert(response, pending).is_some() {
                    return Err(MsiCacheBankError::DuplicateSnapshotPendingFill { response });
                }
            }
        }

        self.next_sequence = snapshot.next_sequence();
        self.lines = lines;
        self.pending_fills = pending_fills;
        self.inflight_uncacheable_writes = inflight_uncacheable_writes;
        self.mshr = restored_mshr;
        self.write_queue = restored_write_queue;
        Ok(())
    }

    fn validate_pending_uncacheable_read_snapshot(
        &self,
        request: &MemoryRequest,
        blocked_by: Option<CacheWriteQueueHandle>,
        restored_write_queue: Option<&CacheWriteQueue>,
    ) -> Result<(), MsiCacheBankError> {
        if !request.is_uncacheable()
            || !request.requires_response()
            || request.operation() == MemoryOperation::Write
        {
            return Err(
                MsiCacheBankError::SnapshotPendingUncacheableRequestMismatch {
                    response: request.id(),
                    operation: request.operation(),
                    uncacheable: request.is_uncacheable(),
                },
            );
        }
        self.validate_write_queue_request(request)?;
        let Some(handle) = blocked_by else {
            return Ok(());
        };
        let valid = restored_write_queue
            .and_then(|write_queue| write_queue.entry(handle).ok())
            .is_some_and(|entry| {
                entry.kind() == CacheWriteQueueEntryKind::WritebackDirty
                    && entry.request().line_address() == request.line_address()
            });
        if valid {
            Ok(())
        } else {
            Err(
                MsiCacheBankError::SnapshotPendingUncacheableReadWritebackMismatch {
                    response: request.id(),
                    handle,
                },
            )
        }
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
        if self.pending_atomic_conflict(line) {
            return Err(MsiCacheBankError::PendingUncacheableConflict { line });
        }
        if !request.is_uncacheable() {
            if let Some(result) = self.accept_write_queue_conflict(&request)? {
                return Ok(result);
            }
        }
        if request.is_uncacheable() {
            if self.write_queue_pending_conflict(line, false).is_some() {
                return Err(MsiCacheBankError::WriteQueueConflict { line });
            }
            if request.operation() == MemoryOperation::Write {
                return self.accept_uncacheable_write_request(request);
            }
            return self.accept_uncacheable_request(request);
        }

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
                .insert(downstream.id(), PendingBankFill::Line { line, mshr });
        }
        Ok(result)
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        let response_id = response.request_id();
        let pending = self.pending_fills.get(&response_id).cloned().ok_or(
            MsiCacheBankError::UnknownPendingFill {
                response: response_id,
            },
        )?;
        let (line, mshr) = match pending {
            PendingBankFill::Line { line, mshr } => (line, mshr),
            PendingBankFill::Uncacheable { original, .. } => {
                let outcome = crate::downstream::uncacheable_fill_outcome(&original, response)
                    .map_err(CacheControllerError::Memory)?;
                self.pending_fills.remove(&response_id);
                return Ok(CacheControllerResult::new(
                    CacheControllerResultKind::Fill,
                    MsiState::Invalid,
                    None,
                    None,
                    Some(outcome),
                ));
            }
        };
        let controller = self
            .lines
            .get_mut(&line)
            .expect("pending fill references an existing MSI cache line");
        let result = controller.accept_fill(response)?;
        self.pending_fills.remove(&response_id);

        let completion = match mshr {
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
            let post_fill_downstream_requests = completion.post_fill_downstream_requests();
            let outcomes = self.target_outcomes_for_completion(&completion)?;
            return Ok(result
                .with_target_outcomes(outcomes)
                .with_post_fill_downstream_requests(post_fill_downstream_requests));
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

    pub fn enqueue_writeback(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, MsiCacheBankError> {
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_writeback(request, secure, ready_tick)
            .map_err(Into::into)
    }

    pub fn enqueue_reserved_writeback(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, MsiCacheBankError> {
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_reserved_writeback(request, secure, ready_tick)
            .map_err(Into::into)
    }

    pub fn enqueue_uncacheable_write(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, MsiCacheBankError> {
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_uncacheable_write(request, secure, ready_tick)
            .map_err(Into::into)
    }

    pub fn enqueue_reserved_uncacheable_write(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, MsiCacheBankError> {
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_reserved_uncacheable_write(request, secure, ready_tick)
            .map_err(Into::into)
    }

    pub fn issue_write_queue(
        &mut self,
        tick: u64,
    ) -> Result<Option<CacheWriteQueueIssue>, MsiCacheBankError> {
        let issue = self.write_queue_mut()?.issue_next(tick)?;
        let issue = issue.map(|issue| self.attach_post_issue_uncacheable_read(issue));
        if let Some(issue) = &issue {
            if issue.kind() == CacheWriteQueueEntryKind::UncacheableWrite {
                self.inflight_uncacheable_writes
                    .insert(issue.request().id(), issue.request().clone());
            }
        }
        Ok(issue)
    }

    fn attach_post_issue_uncacheable_read(
        &mut self,
        issue: CacheWriteQueueIssue,
    ) -> CacheWriteQueueIssue {
        if issue.kind() != CacheWriteQueueEntryKind::WritebackDirty {
            return issue;
        }
        let handle = issue.handle();
        let read = self
            .pending_fills
            .values_mut()
            .find_map(|pending| match pending {
                PendingBankFill::Uncacheable {
                    original,
                    blocked_by,
                } => {
                    if *blocked_by == Some(handle)
                        && original.line_address() == issue.request().line_address()
                    {
                        *blocked_by = None;
                        Some(original.clone())
                    } else {
                        None
                    }
                }
                PendingBankFill::Line { .. } => None,
            });
        match read {
            Some(request) => issue.with_post_issue_downstream_request(request),
            None => issue,
        }
    }

    pub fn accept_uncacheable_write_response(
        &mut self,
        response: MemoryResponse,
    ) -> Result<TargetOutcome, MsiCacheBankError> {
        let response_id = response.request_id();
        let original = self.inflight_uncacheable_writes.get(&response_id).ok_or(
            MsiCacheBankError::UnknownUncacheableWriteResponse {
                response: response_id,
            },
        )?;
        let outcome = crate::downstream::uncacheable_write_response_outcome(original, response)
            .map_err(CacheControllerError::Memory)
            .map_err(MsiCacheBankError::from)?;
        self.inflight_uncacheable_writes.remove(&response_id);
        Ok(outcome)
    }

    pub fn write_queue_find_match(
        &self,
        line: Address,
        secure: bool,
        ignore_uncacheable: bool,
    ) -> Option<CacheWriteQueueHandle> {
        let line = self.layout.line_address(line);
        self.write_queue
            .as_ref()
            .and_then(|queue| queue.find_match(line, secure, ignore_uncacheable))
    }

    pub fn write_queue_pending_conflict(
        &self,
        line: Address,
        secure: bool,
    ) -> Option<CacheWriteQueueHandle> {
        let line = self.layout.line_address(line);
        self.write_queue
            .as_ref()
            .and_then(|queue| queue.pending_conflict(line, secure))
    }

    pub fn write_queue_satisfy_read(
        &self,
        address: Address,
        size: AccessSize,
        secure: bool,
    ) -> Result<Option<Vec<u8>>, MsiCacheBankError> {
        self.write_queue_ref()?
            .satisfy_read(address, size, secure)
            .map_err(Into::into)
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

    fn validate_write_queue_request(
        &self,
        request: &MemoryRequest,
    ) -> Result<(), MsiCacheBankError> {
        self.validate_request_agent(request)?;
        let actual = request.line_layout();
        if actual != self.layout {
            return Err(
                CacheControllerError::Memory(MemoryError::LineLayoutMismatch {
                    request: request.id(),
                    expected: self.layout,
                    actual,
                })
                .into(),
            );
        }

        Ok(())
    }

    fn accept_write_queue_conflict(
        &self,
        request: &MemoryRequest,
    ) -> Result<Option<CacheControllerResult>, MsiCacheBankError> {
        let line = request.line_address();
        if self.write_queue_pending_conflict(line, false).is_none() {
            return Ok(None);
        }
        if request.returns_data() && !request.carries_data() {
            if let Some(data) = self.write_queue_ref()?.satisfy_read(
                request.range().start(),
                request.size(),
                false,
            )? {
                let state = self
                    .lines
                    .get(&line)
                    .map_or(MsiState::Invalid, MsiCacheController::state);
                let response = MemoryResponse::completed(request, Some(data))
                    .map_err(CacheControllerError::Memory)?;
                return Ok(Some(CacheControllerResult::new(
                    CacheControllerResultKind::Hit,
                    state,
                    None,
                    None,
                    Some(TargetOutcome::Respond(response)),
                )));
            }
        }
        Err(MsiCacheBankError::WriteQueueConflict { line })
    }

    fn pending_atomic_conflict(&self, line: Address) -> bool {
        self.pending_fills.values().any(|pending| match pending {
            PendingBankFill::Uncacheable { original, .. } => {
                original.line_address() == line && original.operation() == MemoryOperation::Atomic
            }
            PendingBankFill::Line { .. } => false,
        })
    }

    fn accept_uncacheable_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        let line = request.line_address();
        if let Some(writeback) =
            self.enqueue_dirty_resident_writeback_and_remove(&request, line, 1)?
        {
            self.pending_fills.insert(
                request.id(),
                PendingBankFill::Uncacheable {
                    original: request.clone(),
                    blocked_by: Some(writeback),
                },
            );
            return Ok(CacheControllerResult::new(
                CacheControllerResultKind::Miss,
                MsiState::Invalid,
                None,
                None,
                None,
            ));
        }
        if !self.can_bypass_uncacheable_resident_line(line) {
            return Err(MsiCacheBankError::UncacheableBypassRequiresCleanLine { line });
        }
        self.lines.remove(&line);
        self.pending_fills.insert(
            request.id(),
            PendingBankFill::Uncacheable {
                original: request.clone(),
                blocked_by: None,
            },
        );
        Ok(CacheControllerResult::new(
            CacheControllerResultKind::Miss,
            MsiState::Invalid,
            None,
            Some(request),
            None,
        ))
    }

    fn accept_uncacheable_write_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        let line = request.line_address();
        if self
            .enqueue_dirty_resident_writeback_and_remove(&request, line, 2)?
            .is_some()
        {
            self.write_queue_mut()?
                .enqueue_uncacheable_write(request, false, 0)?;
            return Ok(CacheControllerResult::new(
                CacheControllerResultKind::Miss,
                MsiState::Invalid,
                None,
                None,
                None,
            ));
        }
        if !self.can_bypass_uncacheable_resident_line(line) {
            return Err(MsiCacheBankError::UncacheableBypassRequiresCleanLine { line });
        }
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_uncacheable_write(request, false, 0)?;
        self.lines.remove(&line);
        Ok(CacheControllerResult::new(
            CacheControllerResultKind::Miss,
            MsiState::Invalid,
            None,
            None,
            None,
        ))
    }

    fn can_bypass_uncacheable_resident_line(&self, line: Address) -> bool {
        matches!(
            self.lines.get(&line).map(MsiCacheController::state),
            None | Some(MsiState::Invalid | MsiState::Shared)
        )
    }

    fn enqueue_dirty_resident_writeback_and_remove(
        &mut self,
        request: &MemoryRequest,
        line: Address,
        required_entries: usize,
    ) -> Result<Option<CacheWriteQueueHandle>, MsiCacheBankError> {
        let Some(writeback) = self.dirty_resident_writeback_request(line)? else {
            return Ok(None);
        };
        self.validate_write_queue_request(request)?;
        self.require_write_queue_entries(required_entries)?;
        let update = self
            .write_queue_mut()?
            .enqueue_writeback(writeback, false, 0)?;
        self.next_sequence += 1;
        self.lines.remove(&line);
        Ok(Some(update.handle()))
    }

    fn dirty_resident_writeback_request(
        &self,
        line: Address,
    ) -> Result<Option<MemoryRequest>, MsiCacheBankError> {
        let Some(controller) = self.lines.get(&line) else {
            return Ok(None);
        };
        if !controller.state().is_modified() {
            return Ok(None);
        }
        let data = controller
            .cached_data()
            .ok_or(CacheControllerError::LineDataUnavailable {
                line: MsiLineId::new(line),
            })?
            .to_vec();
        let request = MemoryRequest::writeback_dirty(
            MemoryRequestId::new(self.agent, self.next_sequence),
            line,
            data,
            self.layout,
        )
        .map_err(CacheControllerError::Memory)?;
        Ok(Some(request))
    }

    fn require_write_queue_entries(&self, entries: usize) -> Result<(), MsiCacheBankError> {
        let queue = self.write_queue_ref()?;
        if queue.allocated_count() + entries > queue.config().entries() {
            return Err(CacheWriteQueueError::EntrySlotsFull {
                entries: queue.config().entries(),
                reserve: queue.config().reserve(),
            }
            .into());
        }
        Ok(())
    }

    fn write_queue_ref(&self) -> Result<&CacheWriteQueue, MsiCacheBankError> {
        self.write_queue
            .as_ref()
            .ok_or(MsiCacheBankError::WriteQueueDisabled)
    }

    fn write_queue_mut(&mut self) -> Result<&mut CacheWriteQueue, MsiCacheBankError> {
        self.write_queue
            .as_mut()
            .ok_or(MsiCacheBankError::WriteQueueDisabled)
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
            .local_targets()
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
