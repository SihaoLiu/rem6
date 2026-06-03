use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryRequest};

use crate::{
    CacheCompressedTagsSnapshot, CacheReplacementDirectorySnapshot, CacheSectorTagsSnapshot,
    CacheWriteQueueHandle, CacheWriteQueueSnapshot, MshrQosProfile, MshrQueueSnapshot,
    MsiCacheControllerSnapshot,
};

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
    replacement_directory: Option<CacheReplacementDirectorySnapshot>,
    sector_tags: Option<CacheSectorTagsSnapshot>,
    compressed_tags: Option<CacheCompressedTagsSnapshot>,
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
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: None,
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
            replacement_directory: None,
            sector_tags: None,
            compressed_tags: None,
            inflight_uncacheable_writes: Vec::new(),
            pending_uncacheable_reads: Vec::new(),
        }
    }

    pub fn with_write_queue(mut self, write_queue: CacheWriteQueueSnapshot) -> Self {
        self.write_queue = Some(write_queue);
        self
    }

    pub fn with_replacement_directory(
        mut self,
        replacement_directory: CacheReplacementDirectorySnapshot,
    ) -> Self {
        self.replacement_directory = Some(replacement_directory);
        self
    }

    pub fn with_sector_tags(mut self, sector_tags: CacheSectorTagsSnapshot) -> Self {
        self.sector_tags = Some(sector_tags);
        self
    }

    pub fn with_compressed_tags(mut self, compressed_tags: CacheCompressedTagsSnapshot) -> Self {
        self.compressed_tags = Some(compressed_tags);
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

    pub fn replacement_directory(&self) -> Option<&CacheReplacementDirectorySnapshot> {
        self.replacement_directory.as_ref()
    }

    pub fn sector_tags(&self) -> Option<&CacheSectorTagsSnapshot> {
        self.sector_tags.as_ref()
    }

    pub fn compressed_tags(&self) -> Option<&CacheCompressedTagsSnapshot> {
        self.compressed_tags.as_ref()
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
