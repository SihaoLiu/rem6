use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_mesi::{
    MesiCacheLine, MesiError, MesiEvent, MesiLineId, MesiState, MesiTransition,
};
use rem6_protocol_msi::{MsiCacheLine, MsiError, MsiEvent, MsiLineId, MsiState, MsiTransition};
use rem6_transport::TargetOutcome;

mod bank;
mod chi;
mod chi_bank;
mod mesi_bank;
mod moesi;
mod moesi_bank;
mod mshr;
mod prefetch;
mod prefetch_ampm;
mod prefetch_bop;
mod prefetch_dcpt;
mod prefetch_fdp;
mod prefetch_indirect_memory;
mod prefetch_isb;
mod prefetch_multi;
mod prefetch_pif;
mod prefetch_sbooe;
mod prefetch_signature_path;
mod prefetch_sms;
mod prefetch_stems;
mod prefetch_throttle;
mod replacement;
mod write_queue;

pub use bank::{MsiCacheBank, MsiCacheBankError, MsiCacheBankSnapshot};
pub use chi::{
    ChiCacheController, ChiCacheControllerError, ChiCacheControllerResult,
    ChiCacheControllerResultKind, ChiCacheControllerSnapshot, ChiPendingMissSnapshot,
};
pub use chi_bank::{ChiCacheBank, ChiCacheBankError, ChiCacheBankSnapshot};
pub use mesi_bank::{MesiCacheBank, MesiCacheBankError, MesiCacheBankSnapshot};
pub use moesi::{
    MoesiCacheController, MoesiCacheControllerError, MoesiCacheControllerResult,
    MoesiCacheControllerResultKind, MoesiCacheControllerSnapshot, MoesiPendingMissSnapshot,
};
pub use moesi_bank::{MoesiCacheBank, MoesiCacheBankError, MoesiCacheBankSnapshot};
pub use mshr::{
    MshrCompletion, MshrEntry, MshrHandle, MshrQosClass, MshrQosProfile, MshrQueue,
    MshrQueueConfig, MshrQueueError, MshrQueueSnapshot, MshrQueueUpdate, MshrTarget,
    MshrTargetSource,
};
pub use prefetch::{
    PrefetchCandidate, QueuedPrefetchConfig, QueuedPrefetchDemandAccess,
    QueuedPrefetchEnqueueResult, QueuedPrefetchEntrySnapshot, QueuedPrefetchFullPolicy,
    QueuedPrefetchIssue, QueuedPrefetchRedundantLine, QueuedPrefetchResidency, QueuedPrefetcher,
    QueuedPrefetcherError, QueuedPrefetcherSnapshot, StridePrefetchAccess, StridePrefetchCandidate,
    StridePrefetchContextSnapshot, StridePrefetchEntrySnapshot, StridePrefetcher,
    StridePrefetcherConfig, StridePrefetcherError, StridePrefetcherSnapshot, TaggedPrefetchAccess,
    TaggedPrefetchCandidate, TaggedPrefetcher, TaggedPrefetcherConfig, TaggedPrefetcherError,
    TaggedPrefetcherSnapshot,
};
pub use prefetch_ampm::{
    AmpmAccessMapEntrySnapshot, AmpmAccessMapState, AmpmEpochConfig, AmpmEpochReport,
    AmpmEpochStats, AmpmPrefetchAccess, AmpmPrefetchCandidate, AmpmPrefetcher,
    AmpmPrefetcherConfig, AmpmPrefetcherError, AmpmPrefetcherSnapshot, AmpmRatio,
};
pub use prefetch_bop::{
    BopDelayQueueConfig, BopDelayQueueEntrySnapshot, BopPrefetchAccess, BopPrefetchCandidate,
    BopPrefetcher, BopPrefetcherConfig, BopPrefetcherConfigOptions, BopPrefetcherError,
    BopPrefetcherSnapshot,
};
pub use prefetch_dcpt::{
    DcptPrefetchAccess, DcptPrefetchCandidate, DcptPrefetchContextSnapshot,
    DcptPrefetchEntrySnapshot, DcptPrefetcher, DcptPrefetcherConfig, DcptPrefetcherError,
    DcptPrefetcherSnapshot,
};
pub use prefetch_fdp::{
    FetchDirectedCacheLookup, FetchDirectedInsertSummary, FetchDirectedPrefetchIssue,
    FetchDirectedPrefetchQueueEntrySnapshot, FetchDirectedPrefetcher,
    FetchDirectedPrefetcherConfig, FetchDirectedPrefetcherError, FetchDirectedPrefetcherSnapshot,
    FetchDirectedRemoveSummary, FetchDirectedStatsSnapshot, FetchDirectedTarget,
    FetchDirectedTranslation, FetchDirectedTranslationEntrySnapshot,
    FetchDirectedTranslationOutcome,
};
pub use prefetch_indirect_memory::{
    IndirectMemoryPatternDetectorEntrySnapshot, IndirectMemoryPrefetchAccess,
    IndirectMemoryPrefetchCandidate, IndirectMemoryPrefetchEntrySnapshot,
    IndirectMemoryPrefetchKeySnapshot, IndirectMemoryPrefetchKind, IndirectMemoryPrefetcher,
    IndirectMemoryPrefetcherConfig, IndirectMemoryPrefetcherError,
    IndirectMemoryPrefetcherSnapshot,
};
pub use prefetch_isb::{
    IrregularStreamBufferAccess, IrregularStreamBufferCandidate, IrregularStreamBufferConfig,
    IrregularStreamBufferError, IrregularStreamBufferMappingEntrySnapshot,
    IrregularStreamBufferMappingKeySnapshot, IrregularStreamBufferMappingSnapshot,
    IrregularStreamBufferPrefetcher, IrregularStreamBufferSnapshot,
    IrregularStreamBufferTrainingEntrySnapshot, IrregularStreamBufferTrainingKeySnapshot,
};
pub use prefetch_multi::{
    MultiQueuedPrefetchIssue, MultiQueuedPrefetcher, MultiQueuedPrefetcherError,
};
pub use prefetch_pif::{
    PifCompactorEntrySnapshot, PifHistoryEntrySnapshot, PifIndexEntrySnapshot, PifPrefetchAccess,
    PifPrefetchCandidate, PifPrefetcher, PifPrefetcherConfig, PifPrefetcherError,
    PifPrefetcherSnapshot,
};
pub use prefetch_sbooe::{
    SbooePrefetchAccess, SbooePrefetchCandidate, SbooePrefetcher, SbooePrefetcherConfig,
    SbooePrefetcherError, SbooePrefetcherSnapshot, SbooeSandboxEntrySnapshot, SbooeSandboxSnapshot,
};
pub use prefetch_signature_path::{
    SignaturePathPatternEntrySnapshot, SignaturePathPatternStrideSnapshot,
    SignaturePathPrefetchAccess, SignaturePathPrefetchCandidate, SignaturePathPrefetcher,
    SignaturePathPrefetcherConfig, SignaturePathPrefetcherConfigOptions,
    SignaturePathPrefetcherError, SignaturePathPrefetcherSnapshot, SignaturePathRatio,
    SignaturePathSignatureEntrySnapshot,
};
pub use prefetch_sms::{
    SmsActiveEntrySnapshot, SmsFilterEntrySnapshot, SmsPatternEntrySnapshot, SmsPrefetchAccess,
    SmsPrefetchCandidate, SmsPrefetcher, SmsPrefetcherConfig, SmsPrefetcherError,
    SmsPrefetcherSnapshot,
};
pub use prefetch_stems::{
    StemsActiveGenerationKeySnapshot, StemsCacheResidency, StemsGenerationEntrySnapshot,
    StemsPatternSequenceEntrySnapshot, StemsPatternSequenceKeySnapshot, StemsPrefetchAccess,
    StemsPrefetchCandidate, StemsPrefetcher, StemsPrefetcherConfig, StemsPrefetcherError,
    StemsPrefetcherSnapshot, StemsRegionMissOrderBufferEntrySnapshot, StemsSequenceEntrySnapshot,
};
pub use prefetch_throttle::{
    QueuedPrefetchThrottle, QueuedPrefetchThrottleConfig, QueuedPrefetchThrottleError,
    QueuedPrefetchThrottleSnapshot,
};
pub use replacement::{
    CacheReplacementPolicyConfig, CacheReplacementPolicyError, CacheReplacementPolicyKind,
    ReplacementDecision, ReplacementEntry, ReplacementSet, ReplacementSetSnapshot,
    ReplacementUpdate,
};
pub use write_queue::{
    CacheCleanReplacementPolicy, CacheReplacementVictim, CacheReplacementVictimState,
    CacheWriteQueue, CacheWriteQueueConfig, CacheWriteQueueEntry, CacheWriteQueueEntryKind,
    CacheWriteQueueError, CacheWriteQueueHandle, CacheWriteQueueIssue, CacheWriteQueueSnapshot,
    CacheWriteQueueUpdate,
};

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
    SnapshotIdentityMismatch {
        expected_agent: AgentId,
        actual_agent: AgentId,
        expected_line: MsiLineId,
        actual_line: MsiLineId,
        expected_layout: CacheLineLayout,
        actual_layout: CacheLineLayout,
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
    target_outcomes: Vec<TargetOutcome>,
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
            target_outcomes: target_outcome.iter().cloned().collect(),
            target_outcome,
        }
    }

    pub(crate) fn with_target_outcomes(mut self, target_outcomes: Vec<TargetOutcome>) -> Self {
        self.target_outcome = target_outcomes.first().cloned();
        self.target_outcomes = target_outcomes;
        self
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

    pub fn target_outcomes(&self) -> &[TargetOutcome] {
        &self.target_outcomes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingMiss {
    original: MemoryRequest,
    downstream: MemoryRequestId,
    fill_event: MsiEvent,
}

impl PendingMiss {
    fn snapshot(&self) -> MsiPendingMissSnapshot {
        MsiPendingMissSnapshot::new(self.original.clone(), self.downstream, self.fill_event)
    }

    fn from_snapshot(snapshot: &MsiPendingMissSnapshot) -> Self {
        Self {
            original: snapshot.original.clone(),
            downstream: snapshot.downstream,
            fill_event: snapshot.fill_event,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiPendingMissSnapshot {
    original: MemoryRequest,
    downstream: MemoryRequestId,
    fill_event: MsiEvent,
}

impl MsiPendingMissSnapshot {
    pub fn new(original: MemoryRequest, downstream: MemoryRequestId, fill_event: MsiEvent) -> Self {
        Self {
            original,
            downstream,
            fill_event,
        }
    }

    pub const fn original(&self) -> &MemoryRequest {
        &self.original
    }

    pub const fn downstream(&self) -> MemoryRequestId {
        self.downstream
    }

    pub const fn fill_event(&self) -> MsiEvent {
        self.fill_event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiCacheControllerSnapshot {
    line: MsiCacheLine,
    layout: CacheLineLayout,
    next_sequence: u64,
    data: Option<Vec<u8>>,
    pending: Option<MsiPendingMissSnapshot>,
}

impl MsiCacheControllerSnapshot {
    pub fn new(
        line: MsiCacheLine,
        layout: CacheLineLayout,
        next_sequence: u64,
        data: Option<Vec<u8>>,
        pending: Option<MsiPendingMissSnapshot>,
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

    pub const fn line(&self) -> MsiLineId {
        self.line.line()
    }

    pub const fn state(&self) -> MsiState {
        self.line.state()
    }

    pub fn line_state(&self) -> &MsiCacheLine {
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

    pub fn pending(&self) -> Option<&MsiPendingMissSnapshot> {
        self.pending.as_ref()
    }
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

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) fn set_next_sequence(&mut self, next_sequence: u64) {
        self.next_sequence = next_sequence;
    }

    pub fn cached_data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn snapshot(&self) -> MsiCacheControllerSnapshot {
        MsiCacheControllerSnapshot::new(
            self.line.clone(),
            self.layout,
            self.next_sequence,
            self.data.clone(),
            self.pending.as_ref().map(PendingMiss::snapshot),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &MsiCacheControllerSnapshot,
    ) -> Result<(), CacheControllerError> {
        self.validate_snapshot_identity(snapshot)?;
        self.validate_snapshot_data(snapshot)?;
        self.line = snapshot.line.clone();
        self.next_sequence = snapshot.next_sequence;
        self.data = snapshot.data.clone();
        self.pending = snapshot.pending.as_ref().map(PendingMiss::from_snapshot);
        Ok(())
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
        if request.operation() == MemoryOperation::Atomic {
            let data = self.read_slice(request)?;
            self.apply_store(request)?;
            return MemoryResponse::completed(request, Some(data))
                .map_err(CacheControllerError::Memory);
        }

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

    fn validate_snapshot_identity(
        &self,
        snapshot: &MsiCacheControllerSnapshot,
    ) -> Result<(), CacheControllerError> {
        if self.agent == snapshot.agent()
            && self.line() == snapshot.line()
            && self.layout == snapshot.layout()
        {
            return Ok(());
        }

        Err(CacheControllerError::SnapshotIdentityMismatch {
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
        snapshot: &MsiCacheControllerSnapshot,
    ) -> Result<(), CacheControllerError> {
        if let Some(data) = snapshot.cached_data() {
            if data.len() as u64 != self.layout.bytes() {
                return Err(CacheControllerError::LineDataSizeMismatch {
                    expected: self.layout.bytes(),
                    actual: data.len() as u64,
                });
            }
        }

        Ok(())
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
    target_outcome: Option<TargetOutcome>,
    target_outcomes: Vec<TargetOutcome>,
}

impl MesiCacheControllerResult {
    fn new(
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
            target_outcomes: target_outcome.iter().cloned().collect(),
            target_outcome,
        }
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

        match (mesi_cpu_event(request), before) {
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
        }
    }

    fn complete_cpu_request(
        &mut self,
        request: &MemoryRequest,
    ) -> Result<MemoryResponse, MesiCacheControllerError> {
        if request.operation() == MemoryOperation::Atomic {
            let data = self.read_slice(request)?;
            self.apply_store(request)?;
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
        let offset = self.checked_offset(request)?;
        let line = self.line();
        let data = self
            .data
            .as_mut()
            .ok_or(MesiCacheControllerError::LineDataUnavailable { line })?;
        let payload = request.data().ok_or(MesiCacheControllerError::Memory(
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
        MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate => MesiEvent::CpuWrite,
    }
}
