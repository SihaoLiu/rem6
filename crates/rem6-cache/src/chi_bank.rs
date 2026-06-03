use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

mod compressed_tags;
mod sector_tags;
mod snapshot;

pub use snapshot::{ChiCacheBankSnapshot, ChiPendingUncacheableReadSnapshot};

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_chi::{ChiEvent, ChiLineId, ChiState};
use rem6_transport::TargetOutcome;

use crate::{
    post_fill, CacheCompressedTags, CacheCompressedTagsConfig, CacheCompressedTagsError,
    CacheIndexingPolicyKind, CacheReplacementDirectory, CacheReplacementDirectoryConfig,
    CacheReplacementPolicyError, CacheReplacementPolicyKind, CacheSectorTags,
    CacheSectorTagsConfig, CacheSectorTagsError, CacheWriteQueue, CacheWriteQueueConfig,
    CacheWriteQueueEntryKind, CacheWriteQueueError, CacheWriteQueueHandle, CacheWriteQueueIssue,
    CacheWriteQueueUpdate, ChiCacheController, ChiCacheControllerError, ChiCacheControllerResult,
    ChiCacheControllerResultKind, MshrCompletion, MshrHandle, MshrQosClass, MshrQosProfile,
    MshrQueue, MshrQueueConfig, MshrQueueError, MshrTargetSource,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChiCacheBankError {
    Controller(ChiCacheControllerError),
    Mshr(MshrQueueError),
    WriteQueue(CacheWriteQueueError),
    Replacement(CacheReplacementPolicyError),
    SectorTags(CacheSectorTagsError),
    CompressedTags(CacheCompressedTagsError),
    ReplacementDirectoryLayoutMismatch {
        expected: CacheLineLayout,
        actual: CacheLineLayout,
    },
    SectorTagsLayoutMismatch {
        expected: CacheLineLayout,
        actual: CacheLineLayout,
    },
    CompressedTagsLayoutMismatch {
        expected: CacheLineLayout,
        actual: CacheLineLayout,
    },
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
    DirtyReplacementRequiresWriteQueue {
        line: Address,
    },
    TransientReplacementRequiresStableLine {
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
    SnapshotReplacementDirectoryModeMismatch {
        snapshot_has_replacement_directory: bool,
        bank_has_replacement_directory: bool,
    },
    SnapshotSectorTagsModeMismatch {
        snapshot_has_sector_tags: bool,
        bank_has_sector_tags: bool,
    },
    SnapshotCompressedTagsModeMismatch {
        snapshot_has_compressed_tags: bool,
        bank_has_compressed_tags: bool,
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

impl fmt::Display for ChiCacheBankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Controller(error) => write!(formatter, "{error}"),
            Self::Mshr(error) => write!(formatter, "{error}"),
            Self::WriteQueue(error) => write!(formatter, "{error}"),
            Self::Replacement(error) => write!(formatter, "{error}"),
            Self::SectorTags(error) => write!(formatter, "{error}"),
            Self::CompressedTags(error) => write!(formatter, "{error}"),
            Self::ReplacementDirectoryLayoutMismatch { expected, actual } => write!(
                formatter,
                "CHI cache bank replacement directory line size {} cannot attach to bank line size {}",
                actual.bytes(),
                expected.bytes()
            ),
            Self::CompressedTagsLayoutMismatch { expected, actual } => write!(
                formatter,
                "CHI cache bank compressed tags line size {} cannot attach to bank line size {}",
                actual.bytes(),
                expected.bytes()
            ),
            Self::SectorTagsLayoutMismatch { expected, actual } => write!(
                formatter,
                "CHI cache bank sector tags line size {} cannot attach to bank line size {}",
                actual.bytes(),
                expected.bytes()
            ),
            Self::WrongAgent { expected, actual } => write!(
                formatter,
                "CHI cache bank for agent {} cannot accept request from agent {}",
                expected.get(),
                actual.get()
            ),
            Self::WriteQueueDisabled => write!(formatter, "CHI cache bank has no write queue"),
            Self::WriteQueueConflict { line } => write!(
                formatter,
                "CHI cache bank has pending write-queue work for line {:#x}",
                line.get()
            ),
            Self::PendingUncacheableConflict { line } => write!(
                formatter,
                "CHI cache bank has pending uncacheable request for line {:#x}",
                line.get()
            ),
            Self::UncacheableBypassRequiresCleanLine { line } => write!(
                formatter,
                "CHI cache bank cannot bypass uncacheable request over non-clean resident line {:#x}",
                line.get()
            ),
            Self::DirtyReplacementRequiresWriteQueue { line } => write!(
                formatter,
                "CHI cache bank cannot evict dirty line {:#x} without replacement writeback support",
                line.get()
            ),
            Self::TransientReplacementRequiresStableLine { line } => write!(
                formatter,
                "CHI cache bank cannot evict transient line {:#x} through replacement capacity",
                line.get()
            ),
            Self::UnknownPendingFill { response } => write!(
                formatter,
                "CHI cache bank has no pending fill for response {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::UnknownUncacheableWriteResponse { response } => write!(
                formatter,
                "CHI cache bank has no in-flight uncacheable write for response {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::UnknownSnoopLine { line } => write!(
                formatter,
                "CHI cache bank has no resident line {:#x} for snoop",
                line.get()
            ),
            Self::SnapshotIdentityMismatch {
                expected_agent,
                actual_agent,
                expected_layout,
                actual_layout,
            } => write!(
                formatter,
                "CHI cache bank snapshot for agent {}, line size {} cannot restore bank for agent {}, line size {}",
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
                "CHI cache bank snapshot MSHR mode {snapshot_has_mshr} cannot restore bank MSHR mode {bank_has_mshr}"
            ),
            Self::SnapshotWriteQueueModeMismatch {
                snapshot_has_write_queue,
                bank_has_write_queue,
            } => write!(
                formatter,
                "CHI cache bank snapshot write queue mode {snapshot_has_write_queue} cannot restore bank write queue mode {bank_has_write_queue}"
            ),
            Self::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory,
                bank_has_replacement_directory,
            } => write!(
                formatter,
                "CHI cache bank snapshot replacement directory mode {snapshot_has_replacement_directory} cannot restore bank replacement directory mode {bank_has_replacement_directory}"
            ),
            Self::SnapshotSectorTagsModeMismatch {
                snapshot_has_sector_tags,
                bank_has_sector_tags,
            } => write!(
                formatter,
                "CHI cache bank snapshot sector tags mode {snapshot_has_sector_tags} cannot restore bank sector tags mode {bank_has_sector_tags}"
            ),
            Self::SnapshotCompressedTagsModeMismatch {
                snapshot_has_compressed_tags,
                bank_has_compressed_tags,
            } => write!(
                formatter,
                "CHI cache bank snapshot compressed tags mode {snapshot_has_compressed_tags} cannot restore bank compressed tags mode {bank_has_compressed_tags}"
            ),
            Self::SnapshotPendingUncacheableRequestMismatch {
                response,
                operation,
                uncacheable,
            } => write!(
                formatter,
                "CHI cache bank snapshot pending uncacheable request {} from agent {} has operation {:?} and uncacheable flag {}",
                response.sequence(),
                response.agent().get(),
                operation,
                uncacheable
            ),
            Self::SnapshotPendingUncacheableReadWritebackMismatch { response, handle } => write!(
                formatter,
                "CHI cache bank snapshot pending uncacheable read {} from agent {} references invalid blocking writeback {:?}",
                response.sequence(),
                response.agent().get(),
                handle
            ),
            Self::DuplicateSnapshotLine { line } => {
                write!(formatter, "CHI cache bank snapshot repeats line {:#x}", line.get())
            }
            Self::DuplicateSnapshotPendingFill { response } => write!(
                formatter,
                "CHI cache bank snapshot repeats pending fill {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::SnapshotInflightUncacheableWriteMismatch {
                response,
                operation,
                uncacheable,
            } => write!(
                formatter,
                "CHI cache bank snapshot in-flight uncacheable write {} from agent {} has operation {:?} and uncacheable flag {}",
                response.sequence(),
                response.agent().get(),
                operation,
                uncacheable
            ),
            Self::DuplicateSnapshotUncacheableWrite { response } => write!(
                formatter,
                "CHI cache bank snapshot repeats in-flight uncacheable write {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
        }
    }
}

impl Error for ChiCacheBankError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Controller(error) => Some(error),
            Self::Mshr(error) => Some(error),
            Self::WriteQueue(error) => Some(error),
            Self::Replacement(error) => Some(error),
            Self::SectorTags(error) => Some(error),
            Self::CompressedTags(error) => Some(error),
            Self::ReplacementDirectoryLayoutMismatch { .. }
            | Self::SectorTagsLayoutMismatch { .. }
            | Self::CompressedTagsLayoutMismatch { .. }
            | Self::WrongAgent { .. }
            | Self::WriteQueueDisabled
            | Self::WriteQueueConflict { .. }
            | Self::PendingUncacheableConflict { .. }
            | Self::UncacheableBypassRequiresCleanLine { .. }
            | Self::DirtyReplacementRequiresWriteQueue { .. }
            | Self::TransientReplacementRequiresStableLine { .. }
            | Self::UnknownPendingFill { .. }
            | Self::UnknownUncacheableWriteResponse { .. }
            | Self::UnknownSnoopLine { .. }
            | Self::SnapshotIdentityMismatch { .. }
            | Self::SnapshotMshrModeMismatch { .. }
            | Self::SnapshotWriteQueueModeMismatch { .. }
            | Self::SnapshotReplacementDirectoryModeMismatch { .. }
            | Self::SnapshotSectorTagsModeMismatch { .. }
            | Self::SnapshotCompressedTagsModeMismatch { .. }
            | Self::SnapshotPendingUncacheableRequestMismatch { .. }
            | Self::SnapshotPendingUncacheableReadWritebackMismatch { .. }
            | Self::DuplicateSnapshotLine { .. }
            | Self::DuplicateSnapshotPendingFill { .. }
            | Self::SnapshotInflightUncacheableWriteMismatch { .. }
            | Self::DuplicateSnapshotUncacheableWrite { .. } => None,
        }
    }
}

impl From<ChiCacheControllerError> for ChiCacheBankError {
    fn from(error: ChiCacheControllerError) -> Self {
        Self::Controller(error)
    }
}

impl From<MshrQueueError> for ChiCacheBankError {
    fn from(error: MshrQueueError) -> Self {
        Self::Mshr(error)
    }
}

impl From<CacheWriteQueueError> for ChiCacheBankError {
    fn from(error: CacheWriteQueueError) -> Self {
        Self::WriteQueue(error)
    }
}

impl From<CacheReplacementPolicyError> for ChiCacheBankError {
    fn from(error: CacheReplacementPolicyError) -> Self {
        Self::Replacement(error)
    }
}

impl From<CacheSectorTagsError> for ChiCacheBankError {
    fn from(error: CacheSectorTagsError) -> Self {
        Self::SectorTags(error)
    }
}

impl From<CacheCompressedTagsError> for ChiCacheBankError {
    fn from(error: CacheCompressedTagsError) -> Self {
        Self::CompressedTags(error)
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
pub struct ChiCacheBank {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: BTreeMap<Address, ChiCacheController>,
    pending_fills: BTreeMap<MemoryRequestId, PendingBankFill>,
    inflight_uncacheable_writes: BTreeMap<MemoryRequestId, MemoryRequest>,
    mshr: Option<MshrQueue>,
    write_queue: Option<CacheWriteQueue>,
    replacement_directory: Option<CacheReplacementDirectory>,
    sector_tags: Option<CacheSectorTags>,
    compressed_tags: Option<CacheCompressedTags>,
}

impl ChiCacheBank {
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
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: None,
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
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: None,
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
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: None,
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
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: None,
        }
    }

    pub fn new_with_replacement_directory(
        agent: AgentId,
        layout: CacheLineLayout,
        replacement_config: CacheReplacementDirectoryConfig,
    ) -> Result<Self, ChiCacheBankError> {
        if replacement_config.line_layout() != layout {
            return Err(ChiCacheBankError::ReplacementDirectoryLayoutMismatch {
                expected: layout,
                actual: replacement_config.line_layout(),
            });
        }
        Ok(Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: None,
            write_queue: None,
            replacement_directory: Some(CacheReplacementDirectory::new(replacement_config)),
            sector_tags: None,
            compressed_tags: None,
        })
    }

    pub fn new_with_indexed_replacement_directory(
        agent: AgentId,
        layout: CacheLineLayout,
        replacement_kind: CacheReplacementPolicyKind,
        indexing_kind: CacheIndexingPolicyKind,
        sets: usize,
        ways: usize,
    ) -> Result<Self, ChiCacheBankError> {
        let replacement_config = CacheReplacementDirectoryConfig::new_with_indexing(
            replacement_kind,
            indexing_kind,
            layout,
            sets,
            ways,
        )?;
        Self::new_with_replacement_directory(agent, layout, replacement_config)
    }

    pub fn new_with_sector_tags(
        agent: AgentId,
        layout: CacheLineLayout,
        sector_tags_config: CacheSectorTagsConfig,
    ) -> Result<Self, ChiCacheBankError> {
        if sector_tags_config.line_layout() != layout {
            return Err(ChiCacheBankError::SectorTagsLayoutMismatch {
                expected: layout,
                actual: sector_tags_config.line_layout(),
            });
        }
        Ok(Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: None,
            write_queue: None,
            replacement_directory: None,
            sector_tags: Some(CacheSectorTags::new(sector_tags_config)),
            compressed_tags: None,
        })
    }

    pub fn new_with_compressed_tags(
        agent: AgentId,
        layout: CacheLineLayout,
        compressed_tags_config: CacheCompressedTagsConfig,
    ) -> Result<Self, ChiCacheBankError> {
        if compressed_tags_config.line_layout() != layout {
            return Err(ChiCacheBankError::CompressedTagsLayoutMismatch {
                expected: layout,
                actual: compressed_tags_config.line_layout(),
            });
        }
        Ok(Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
            inflight_uncacheable_writes: BTreeMap::new(),
            mshr: None,
            write_queue: None,
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: Some(CacheCompressedTags::new(compressed_tags_config)),
        })
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

    pub fn replacement_way_for(&self, address: Address) -> Option<(usize, usize)> {
        if let Some(directory) = &self.replacement_directory {
            return directory.way_for(address);
        }
        self.sector_tags
            .as_ref()
            .and_then(|sector_tags| {
                sector_tags
                    .find(address)
                    .map(|lookup| (lookup.set(), lookup.way()))
            })
            .or_else(|| {
                self.compressed_tags.as_ref().and_then(|compressed_tags| {
                    compressed_tags
                        .find(address)
                        .map(|lookup| (lookup.set(), lookup.way()))
                })
            })
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

    pub fn state(&self, address: Address) -> Option<ChiState> {
        self.lines
            .get(&self.layout.line_address(address))
            .map(ChiCacheController::state)
    }

    pub fn cached_data(&self, address: Address) -> Option<&[u8]> {
        self.lines
            .get(&self.layout.line_address(address))
            .and_then(ChiCacheController::cached_data)
    }

    pub fn snapshot(&self) -> ChiCacheBankSnapshot {
        let lines = self
            .lines
            .values()
            .map(ChiCacheController::snapshot)
            .collect();
        let snapshot = match &self.mshr {
            Some(mshr) => ChiCacheBankSnapshot::new_with_mshr(
                self.agent,
                self.layout,
                self.next_sequence,
                lines,
                mshr.snapshot(),
            ),
            None => ChiCacheBankSnapshot::new(self.agent, self.layout, self.next_sequence, lines),
        };
        let snapshot = match &self.write_queue {
            Some(write_queue) => snapshot.with_write_queue(write_queue.snapshot()),
            None => snapshot,
        };
        let snapshot = match &self.replacement_directory {
            Some(replacement_directory) => {
                snapshot.with_replacement_directory(replacement_directory.snapshot())
            }
            None => snapshot,
        };
        let snapshot = match &self.sector_tags {
            Some(sector_tags) => snapshot.with_sector_tags(sector_tags.snapshot()),
            None => snapshot,
        };
        let snapshot = match &self.compressed_tags {
            Some(compressed_tags) => snapshot.with_compressed_tags(compressed_tags.snapshot()),
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
                } => Some(ChiPendingUncacheableReadSnapshot::new(
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

    pub fn restore(&mut self, snapshot: &ChiCacheBankSnapshot) -> Result<(), ChiCacheBankError> {
        if snapshot.agent() != self.agent || snapshot.layout() != self.layout {
            return Err(ChiCacheBankError::SnapshotIdentityMismatch {
                expected_agent: self.agent,
                actual_agent: snapshot.agent(),
                expected_layout: self.layout,
                actual_layout: snapshot.layout(),
            });
        }
        if snapshot.replacement_directory().is_some() && snapshot.sector_tags().is_some() {
            return Err(ChiCacheBankError::SnapshotSectorTagsModeMismatch {
                snapshot_has_sector_tags: true,
                bank_has_sector_tags: self.sector_tags.is_some(),
            });
        }
        if snapshot.compressed_tags().is_some()
            && (snapshot.replacement_directory().is_some() || snapshot.sector_tags().is_some())
        {
            return Err(ChiCacheBankError::SnapshotCompressedTagsModeMismatch {
                snapshot_has_compressed_tags: true,
                bank_has_compressed_tags: self.compressed_tags.is_some(),
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
                return Err(ChiCacheBankError::SnapshotMshrModeMismatch {
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
                return Err(ChiCacheBankError::SnapshotWriteQueueModeMismatch {
                    snapshot_has_write_queue: true,
                    bank_has_write_queue: false,
                });
            }
        };

        let restored_replacement_directory = match (
            &self.replacement_directory,
            snapshot.replacement_directory(),
        ) {
            (Some(directory), Some(snapshot_directory)) => {
                let mut restored = CacheReplacementDirectory::new(directory.config().clone());
                restored.restore(snapshot_directory)?;
                Some(restored)
            }
            (Some(_), None) => {
                return Err(
                    ChiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                        snapshot_has_replacement_directory: false,
                        bank_has_replacement_directory: true,
                    },
                );
            }
            (None, None) => None,
            (None, Some(_)) => {
                return Err(
                    ChiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                        snapshot_has_replacement_directory: true,
                        bank_has_replacement_directory: false,
                    },
                );
            }
        };

        let restored_sector_tags = match (&self.sector_tags, snapshot.sector_tags()) {
            (Some(sector_tags), Some(snapshot_sector_tags)) => {
                let mut restored = CacheSectorTags::new(sector_tags.config().clone());
                restored.restore(snapshot_sector_tags)?;
                Some(restored)
            }
            (Some(_), None) => {
                return Err(ChiCacheBankError::SnapshotSectorTagsModeMismatch {
                    snapshot_has_sector_tags: false,
                    bank_has_sector_tags: true,
                });
            }
            (None, None) => None,
            (None, Some(_)) => {
                return Err(ChiCacheBankError::SnapshotSectorTagsModeMismatch {
                    snapshot_has_sector_tags: true,
                    bank_has_sector_tags: false,
                });
            }
        };

        let restored_compressed_tags = match (&self.compressed_tags, snapshot.compressed_tags()) {
            (Some(compressed_tags), Some(snapshot_compressed_tags)) => {
                let mut restored = CacheCompressedTags::new(compressed_tags.config().clone());
                restored.restore(snapshot_compressed_tags)?;
                Some(restored)
            }
            (Some(_), None) => {
                return Err(ChiCacheBankError::SnapshotCompressedTagsModeMismatch {
                    snapshot_has_compressed_tags: false,
                    bank_has_compressed_tags: true,
                });
            }
            (None, None) => None,
            (None, Some(_)) => {
                return Err(ChiCacheBankError::SnapshotCompressedTagsModeMismatch {
                    snapshot_has_compressed_tags: true,
                    bank_has_compressed_tags: false,
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
                    ChiCacheBankError::SnapshotInflightUncacheableWriteMismatch {
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
                return Err(ChiCacheBankError::DuplicateSnapshotUncacheableWrite {
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
                return Err(ChiCacheBankError::DuplicateSnapshotPendingFill { response });
            }
        }
        for line_snapshot in snapshot.lines() {
            let line = line_snapshot.line().address();
            let mut controller = ChiCacheController::new(self.agent, self.layout, line);
            controller.restore(line_snapshot)?;
            if lines.insert(line, controller).is_some() {
                return Err(ChiCacheBankError::DuplicateSnapshotLine { line });
            }
            if let Some(pending) = line_snapshot.pending() {
                let response = pending.downstream();
                let mshr = restored_mshr
                    .as_ref()
                    .and_then(|mshr| mshr.find_line(line))
                    .map(|entry| entry.handle());
                let pending = PendingBankFill::Line { line, mshr };
                if pending_fills.insert(response, pending).is_some() {
                    return Err(ChiCacheBankError::DuplicateSnapshotPendingFill { response });
                }
            }
        }

        self.next_sequence = snapshot.next_sequence();
        self.lines = lines;
        self.pending_fills = pending_fills;
        self.inflight_uncacheable_writes = inflight_uncacheable_writes;
        self.mshr = restored_mshr;
        self.write_queue = restored_write_queue;
        self.replacement_directory = restored_replacement_directory;
        self.sector_tags = restored_sector_tags;
        self.compressed_tags = restored_compressed_tags;
        Ok(())
    }

    fn validate_pending_uncacheable_read_snapshot(
        &self,
        request: &MemoryRequest,
        blocked_by: Option<CacheWriteQueueHandle>,
        restored_write_queue: Option<&CacheWriteQueue>,
    ) -> Result<(), ChiCacheBankError> {
        if !request.is_uncacheable()
            || !request.requires_response()
            || request.operation() == MemoryOperation::Write
        {
            return Err(
                ChiCacheBankError::SnapshotPendingUncacheableRequestMismatch {
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
                ChiCacheBankError::SnapshotPendingUncacheableReadWritebackMismatch {
                    response: request.id(),
                    handle,
                },
            )
        }
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        self.accept_cpu_request_inner(request, None)
    }

    pub fn accept_cpu_request_with_qos(
        &mut self,
        request: MemoryRequest,
        qos: MshrQosClass,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        self.accept_cpu_request_inner(request, Some(qos))
    }

    fn accept_cpu_request_inner(
        &mut self,
        request: MemoryRequest,
        qos: Option<MshrQosClass>,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        self.validate_request_agent(&request)?;
        let line = request.line_address();
        if self.pending_atomic_conflict(line) {
            return Err(ChiCacheBankError::PendingUncacheableConflict { line });
        }
        if !request.is_uncacheable() {
            if let Some(result) = self.accept_write_queue_conflict(&request)? {
                return Ok(result);
            }
        }
        if request.is_uncacheable() {
            if self.write_queue_pending_conflict(line, false).is_some() {
                return Err(ChiCacheBankError::WriteQueueConflict { line });
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
                .map_or(ChiState::Invalid, ChiCacheController::state);
            self.mshr
                .as_mut()
                .expect("MSHR lookup succeeded but bank has no MSHR queue")
                .allocate_or_merge_optional_qos(request, 0, MshrTargetSource::Demand, true, qos)?;
            return Ok(ChiCacheControllerResult::new(
                ChiCacheControllerResultKind::Miss,
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
            .or_insert_with(|| ChiCacheController::new(self.agent, self.layout, line));
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
        } else if result.kind() == ChiCacheControllerResultKind::Hit {
            self.touch_replacement_line(line)?;
        }
        Ok(result)
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
        event: ChiEvent,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        self.accept_fill_inner(response, event, None)
    }

    pub fn accept_fill_with_compressed_size_bits(
        &mut self,
        response: MemoryResponse,
        event: ChiEvent,
        compressed_size_bits: usize,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        self.accept_fill_inner(response, event, Some(compressed_size_bits))
    }

    fn accept_fill_inner(
        &mut self,
        response: MemoryResponse,
        event: ChiEvent,
        compressed_size_bits: Option<usize>,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        let response_id = response.request_id();
        let pending = self.pending_fills.get(&response_id).cloned().ok_or(
            ChiCacheBankError::UnknownPendingFill {
                response: response_id,
            },
        )?;
        let (line, mshr) = match pending {
            PendingBankFill::Line { line, mshr } => (line, mshr),
            PendingBankFill::Uncacheable { original, .. } => {
                let outcome = crate::downstream::uncacheable_fill_outcome(&original, response)
                    .map_err(ChiCacheControllerError::Memory)?;
                self.pending_fills.remove(&response_id);
                return Ok(ChiCacheControllerResult::new(
                    ChiCacheControllerResultKind::Fill,
                    ChiState::Invalid,
                    None,
                    None,
                    Some(outcome),
                ));
            }
        };
        let post_fill_downstream_requests = match mshr {
            Some(handle) => post_fill::downstream_requests_for_mshr(
                self.mshr
                    .as_ref()
                    .ok_or(ChiCacheBankError::SnapshotMshrModeMismatch {
                        snapshot_has_mshr: true,
                        bank_has_mshr: false,
                    })?,
                handle,
            )?,
            None => Vec::new(),
        };
        post_fill::preflight_downstream_requests(
            self.write_queue.as_ref(),
            &post_fill_downstream_requests,
            ChiCacheBankError::WriteQueueDisabled,
            |request| self.validate_write_queue_request(request),
        )?;
        let controller_snapshot = self
            .lines
            .get(&line)
            .expect("pending fill references an existing CHI cache line")
            .snapshot();
        let replacement_snapshot = self
            .replacement_directory
            .as_ref()
            .map(CacheReplacementDirectory::snapshot);
        let sector_tags_snapshot = self.sector_tags.as_ref().map(CacheSectorTags::snapshot);
        let compressed_tags_snapshot = self
            .compressed_tags
            .as_ref()
            .map(CacheCompressedTags::snapshot);
        let controller = self
            .lines
            .get_mut(&line)
            .expect("pending fill references an existing CHI cache line");
        let result = controller.accept_fill(response, event)?;
        if let Err(error) = self.install_replacement_line(line, compressed_size_bits) {
            self.lines
                .get_mut(&line)
                .expect("pending fill references an existing CHI cache line")
                .restore(&controller_snapshot)?;
            if let (Some(directory), Some(snapshot)) =
                (&mut self.replacement_directory, &replacement_snapshot)
            {
                directory.restore(snapshot)?;
            }
            if let (Some(sector_tags), Some(snapshot)) =
                (&mut self.sector_tags, &sector_tags_snapshot)
            {
                sector_tags.restore(snapshot)?;
            }
            if let (Some(compressed_tags), Some(snapshot)) =
                (&mut self.compressed_tags, &compressed_tags_snapshot)
            {
                compressed_tags.restore(snapshot)?;
            }
            return Err(error);
        }
        self.pending_fills.remove(&response_id);

        let completion = match mshr {
            Some(handle) => {
                let mshr =
                    self.mshr
                        .as_mut()
                        .ok_or(ChiCacheBankError::SnapshotMshrModeMismatch {
                            snapshot_has_mshr: true,
                            bank_has_mshr: false,
                        })?;
                Some(mshr.complete(handle)?)
            }
            None => None,
        };

        if let Some(completion) = completion {
            let post_fill_downstream_requests = completion.post_fill_downstream_requests();
            post_fill::enqueue_downstream_requests(
                self.write_queue.as_mut(),
                &post_fill_downstream_requests,
                ChiCacheBankError::WriteQueueDisabled,
            )?;
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
        event: ChiEvent,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        let line = self.layout.line_address(address);
        let controller = self
            .lines
            .get_mut(&line)
            .ok_or(ChiCacheBankError::UnknownSnoopLine { line })?;
        controller.accept_snoop(event).map_err(Into::into)
    }

    pub fn enqueue_writeback(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, ChiCacheBankError> {
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
    ) -> Result<CacheWriteQueueUpdate, ChiCacheBankError> {
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
    ) -> Result<CacheWriteQueueUpdate, ChiCacheBankError> {
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
    ) -> Result<CacheWriteQueueUpdate, ChiCacheBankError> {
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_reserved_uncacheable_write(request, secure, ready_tick)
            .map_err(Into::into)
    }

    pub fn issue_write_queue(
        &mut self,
        tick: u64,
    ) -> Result<Option<CacheWriteQueueIssue>, ChiCacheBankError> {
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
    ) -> Result<TargetOutcome, ChiCacheBankError> {
        let response_id = response.request_id();
        let original = self.inflight_uncacheable_writes.get(&response_id).ok_or(
            ChiCacheBankError::UnknownUncacheableWriteResponse {
                response: response_id,
            },
        )?;
        let outcome = crate::downstream::uncacheable_write_response_outcome(original, response)
            .map_err(ChiCacheControllerError::Memory)
            .map_err(ChiCacheBankError::from)?;
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
    ) -> Result<Option<Vec<u8>>, ChiCacheBankError> {
        self.write_queue_ref()?
            .satisfy_read(address, size, secure)
            .map_err(Into::into)
    }

    fn validate_request_agent(&self, request: &MemoryRequest) -> Result<(), ChiCacheBankError> {
        let actual = request.id().agent();
        if actual != self.agent {
            return Err(ChiCacheBankError::WrongAgent {
                expected: self.agent,
                actual,
            });
        }

        Ok(())
    }

    fn validate_write_queue_request(
        &self,
        request: &MemoryRequest,
    ) -> Result<(), ChiCacheBankError> {
        self.validate_request_agent(request)?;
        let actual = request.line_layout();
        if actual != self.layout {
            return Err(
                ChiCacheControllerError::Memory(MemoryError::LineLayoutMismatch {
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
    ) -> Result<Option<ChiCacheControllerResult>, ChiCacheBankError> {
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
                    .map_or(ChiState::Invalid, ChiCacheController::state);
                let response = MemoryResponse::completed(request, Some(data))
                    .map_err(ChiCacheControllerError::Memory)?;
                return Ok(Some(ChiCacheControllerResult::new(
                    ChiCacheControllerResultKind::Hit,
                    state,
                    None,
                    None,
                    Some(TargetOutcome::Respond(response)),
                )));
            }
        }
        Err(ChiCacheBankError::WriteQueueConflict { line })
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
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
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
            return Ok(ChiCacheControllerResult::new(
                ChiCacheControllerResultKind::Miss,
                ChiState::Invalid,
                None,
                None,
                None,
            ));
        }
        if !self.can_bypass_uncacheable_resident_line(line) {
            return Err(ChiCacheBankError::UncacheableBypassRequiresCleanLine { line });
        }
        self.remove_resident_line(line)?;
        self.pending_fills.insert(
            request.id(),
            PendingBankFill::Uncacheable {
                original: request.clone(),
                blocked_by: None,
            },
        );
        Ok(ChiCacheControllerResult::new(
            ChiCacheControllerResultKind::Miss,
            ChiState::Invalid,
            None,
            Some(request),
            None,
        ))
    }

    fn accept_uncacheable_write_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
        let line = request.line_address();
        if self
            .enqueue_dirty_resident_writeback_and_remove(&request, line, 2)?
            .is_some()
        {
            self.write_queue_mut()?
                .enqueue_uncacheable_write(request, false, 0)?;
            return Ok(Self::uncacheable_write_result());
        }
        if !self.can_bypass_uncacheable_resident_line(line) {
            return Err(ChiCacheBankError::UncacheableBypassRequiresCleanLine { line });
        }
        self.validate_write_queue_request(&request)?;
        self.write_queue_mut()?
            .enqueue_uncacheable_write(request, false, 0)?;
        self.remove_resident_line(line)?;
        Ok(Self::uncacheable_write_result())
    }

    fn can_bypass_uncacheable_resident_line(&self, line: Address) -> bool {
        matches!(
            self.lines.get(&line).map(ChiCacheController::state),
            None | Some(ChiState::Invalid | ChiState::SharedClean | ChiState::UniqueClean)
        )
    }

    fn uncacheable_write_result() -> ChiCacheControllerResult {
        ChiCacheControllerResult::new(
            ChiCacheControllerResultKind::Miss,
            ChiState::Invalid,
            None,
            None,
            None,
        )
    }

    fn remove_resident_line(&mut self, line: Address) -> Result<(), ChiCacheBankError> {
        self.lines.remove(&line);
        if let Some(directory) = &mut self.replacement_directory {
            directory.remove_resident_line(line)?;
        }
        if let Some(sector_tags) = &mut self.sector_tags {
            sector_tags.invalidate(line)?;
        }
        if let Some(compressed_tags) = &mut self.compressed_tags {
            compressed_tags.invalidate(line)?;
        }
        Ok(())
    }

    fn enqueue_dirty_resident_writeback_and_remove(
        &mut self,
        request: &MemoryRequest,
        line: Address,
        required_entries: usize,
    ) -> Result<Option<CacheWriteQueueHandle>, ChiCacheBankError> {
        let Some(writeback) = self.dirty_resident_writeback_request(line)? else {
            return Ok(None);
        };
        self.validate_write_queue_request(request)?;
        self.require_write_queue_entries(required_entries)?;
        let update = self
            .write_queue_mut()?
            .enqueue_writeback(writeback, false, 0)?;
        self.next_sequence += 1;
        self.remove_resident_line(line)?;
        Ok(Some(update.handle()))
    }

    fn dirty_resident_writeback_request(
        &self,
        line: Address,
    ) -> Result<Option<MemoryRequest>, ChiCacheBankError> {
        let Some(controller) = self.lines.get(&line) else {
            return Ok(None);
        };
        if !controller.state().is_dirty() {
            return Ok(None);
        }
        let data = controller
            .cached_data()
            .ok_or(ChiCacheControllerError::LineDataUnavailable {
                line: ChiLineId::new(line),
            })?
            .to_vec();
        MemoryRequest::writeback_dirty(
            MemoryRequestId::new(self.agent, self.next_sequence),
            line,
            data,
            self.layout,
        )
        .map(Some)
        .map_err(ChiCacheControllerError::Memory)
        .map_err(Into::into)
    }

    fn require_write_queue_entries(&self, entries: usize) -> Result<(), ChiCacheBankError> {
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

    fn write_queue_ref(&self) -> Result<&CacheWriteQueue, ChiCacheBankError> {
        self.write_queue
            .as_ref()
            .ok_or(ChiCacheBankError::WriteQueueDisabled)
    }

    fn write_queue_mut(&mut self) -> Result<&mut CacheWriteQueue, ChiCacheBankError> {
        self.write_queue
            .as_mut()
            .ok_or(ChiCacheBankError::WriteQueueDisabled)
    }

    fn touch_replacement_line(&mut self, line: Address) -> Result<(), ChiCacheBankError> {
        if let Some(directory) = &mut self.replacement_directory {
            directory.touch(line)?;
        }
        if let Some(sector_tags) = &mut self.sector_tags {
            sector_tags.access(line)?;
        }
        if let Some(compressed_tags) = &mut self.compressed_tags {
            compressed_tags.access(line)?;
        }
        Ok(())
    }

    fn install_replacement_line(
        &mut self,
        line: Address,
        compressed_size_bits: Option<usize>,
    ) -> Result<(), ChiCacheBankError> {
        let Some(directory) = &mut self.replacement_directory else {
            return self.install_tag_backend_line(line, compressed_size_bits);
        };
        let install = directory.install(line)?;
        let Some(evicted_line) = install.evicted_line() else {
            return Ok(());
        };
        if evicted_line == self.layout.line_address(line) {
            return Ok(());
        }
        if self
            .lines
            .get(&evicted_line)
            .is_some_and(|controller| controller.state().is_dirty())
        {
            return Err(ChiCacheBankError::DirtyReplacementRequiresWriteQueue {
                line: evicted_line,
            });
        }
        if self
            .lines
            .get(&evicted_line)
            .is_some_and(|controller| controller.state().is_transient())
        {
            return Err(ChiCacheBankError::TransientReplacementRequiresStableLine {
                line: evicted_line,
            });
        }
        self.lines.remove(&evicted_line);
        self.pending_fills.retain(|_, pending| {
            !matches!(
                pending,
                PendingBankFill::Line { line, .. } if *line == evicted_line
            )
        });
        Ok(())
    }

    fn install_tag_backend_line(
        &mut self,
        line: Address,
        compressed_size_bits: Option<usize>,
    ) -> Result<(), ChiCacheBankError> {
        if self.sector_tags.is_some() {
            return self.install_sector_tag_line(line);
        }
        self.install_compressed_tag_line(line, compressed_size_bits)
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

        match self.lines.get(&line).map(ChiCacheController::state) {
            None | Some(ChiState::Invalid) => true,
            Some(ChiState::SharedClean | ChiState::SharedDirty) => request_is_chi_write(request),
            Some(ChiState::UniqueClean | ChiState::UniqueDirty) => false,
            Some(state) if state.is_transient() => false,
            Some(_) => false,
        }
    }

    fn target_outcomes_for_completion(
        &self,
        completion: &MshrCompletion,
    ) -> Result<Vec<TargetOutcome>, ChiCacheBankError> {
        completion
            .local_targets()
            .map(|target| self.complete_mshr_target(target.request()))
            .collect()
    }

    fn complete_mshr_target(
        &self,
        request: &MemoryRequest,
    ) -> Result<TargetOutcome, ChiCacheBankError> {
        if !request.requires_response() {
            return Ok(TargetOutcome::NoResponse);
        }

        let data = if request.returns_data() {
            Some(self.cached_slice_for_request(request)?)
        } else {
            None
        };
        let response =
            MemoryResponse::completed(request, data).map_err(ChiCacheControllerError::Memory)?;
        Ok(TargetOutcome::Respond(response))
    }

    fn cached_slice_for_request(
        &self,
        request: &MemoryRequest,
    ) -> Result<Vec<u8>, ChiCacheControllerError> {
        let line = request.line_address();
        let data = self
            .lines
            .get(&line)
            .and_then(ChiCacheController::cached_data)
            .ok_or(ChiCacheControllerError::LineDataUnavailable {
                line: ChiLineId::new(line),
            })?;
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        if offset + size > data.len() {
            return Err(ChiCacheControllerError::AccessOutsideLine {
                line: ChiLineId::new(line),
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

fn request_is_chi_write(request: &MemoryRequest) -> bool {
    !matches!(
        request.operation(),
        MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::PrefetchRead
    )
}
