use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::allocation::max_vector_len;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchDirectedPrefetcherConfig {
    line_size: u64,
    latency: u64,
    prefetch_queue_entries: usize,
    translation_queue_entries: usize,
    mark_requests_as_prefetch: bool,
    squash_prefetches: bool,
    cache_snoop: bool,
}

impl FetchDirectedPrefetcherConfig {
    pub fn new(
        line_size: u64,
        latency: u64,
        prefetch_queue_entries: usize,
        translation_queue_entries: usize,
        mark_requests_as_prefetch: bool,
        squash_prefetches: bool,
        cache_snoop: bool,
    ) -> Result<Self, FetchDirectedPrefetcherError> {
        if line_size == 0 {
            return Err(FetchDirectedPrefetcherError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(FetchDirectedPrefetcherError::LineSizeNotPowerOfTwo { line_size });
        }
        if prefetch_queue_entries == 0 {
            return Err(FetchDirectedPrefetcherError::ZeroPrefetchQueueEntries);
        }
        validate_fdp_vector_length(
            "prefetch queue entries",
            prefetch_queue_entries,
            maximum_fdp_prefetch_queue_entries(),
        )?;
        if translation_queue_entries == 0 {
            return Err(FetchDirectedPrefetcherError::ZeroTranslationQueueEntries);
        }
        validate_fdp_vector_length(
            "translation queue entries",
            translation_queue_entries,
            maximum_fdp_translation_queue_entries(),
        )?;

        Ok(Self {
            line_size,
            latency,
            prefetch_queue_entries,
            translation_queue_entries,
            mark_requests_as_prefetch,
            squash_prefetches,
            cache_snoop,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn latency(&self) -> u64 {
        self.latency
    }

    pub const fn prefetch_queue_entries(&self) -> usize {
        self.prefetch_queue_entries
    }

    pub const fn translation_queue_entries(&self) -> usize {
        self.translation_queue_entries
    }

    pub const fn mark_requests_as_prefetch(&self) -> bool {
        self.mark_requests_as_prefetch
    }

    pub const fn squash_prefetches(&self) -> bool {
        self.squash_prefetches
    }

    pub const fn cache_snoop(&self) -> bool {
        self.cache_snoop
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchDirectedPrefetcherError {
    ZeroLineSize,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    ZeroPrefetchQueueEntries,
    ZeroTranslationQueueEntries,
    TargetEndBeforeStart {
        start: Address,
        end: Address,
    },
    ReadyTickOverflow {
        tick: u64,
        latency: u64,
    },
    UnknownTranslation {
        fetch_target_id: u64,
        virtual_block: Address,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<FetchDirectedPrefetcherConfig>,
        actual: Box<FetchDirectedPrefetcherConfig>,
    },
    SnapshotPrefetchQueueTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotTranslationQueueTooLarge {
        entries: usize,
        max_entries: usize,
    },
}

impl fmt::Display for FetchDirectedPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "FDP line size is zero"),
            Self::LineSizeNotPowerOfTwo { line_size } => {
                write!(formatter, "FDP line size {line_size} is not a power of two")
            }
            Self::ZeroPrefetchQueueEntries => {
                write!(formatter, "FDP prefetch queue has no entries")
            }
            Self::ZeroTranslationQueueEntries => {
                write!(formatter, "FDP translation queue has no entries")
            }
            Self::TargetEndBeforeStart { start, end } => {
                write!(formatter, "FDP target end {end:?} precedes start {start:?}")
            }
            Self::ReadyTickOverflow { tick, latency } => write!(
                formatter,
                "FDP ready tick overflows for tick {tick} and latency {latency}"
            ),
            Self::UnknownTranslation {
                fetch_target_id,
                virtual_block,
            } => write!(
                formatter,
                "FDP has no translation for target {fetch_target_id} block {virtual_block:?}"
            ),
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "FDP {field} length {length} exceeds maximum {maximum}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "FDP snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotPrefetchQueueTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "FDP snapshot prefetch queue has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotTranslationQueueTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "FDP snapshot translation queue has {entries} entries for {max_entries} slots"
            ),
        }
    }
}

impl Error for FetchDirectedPrefetcherError {}

fn maximum_fdp_prefetch_queue_entries() -> usize {
    max_vector_len::<FetchDirectedPrefetchQueueEntry>()
        .min(max_vector_len::<FetchDirectedPrefetchQueueEntrySnapshot>())
}

fn maximum_fdp_translation_queue_entries() -> usize {
    max_vector_len::<FetchDirectedTranslationEntry>()
        .min(max_vector_len::<FetchDirectedTranslationEntrySnapshot>())
}

fn validate_fdp_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), FetchDirectedPrefetcherError> {
    if length > maximum {
        return Err(FetchDirectedPrefetcherError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FetchDirectedTarget {
    context: AgentId,
    fetch_target_id: u64,
    start: Address,
    end: Address,
    secure: bool,
}

impl FetchDirectedTarget {
    pub fn new(
        context: AgentId,
        fetch_target_id: u64,
        start: Address,
        end: Address,
        secure: bool,
    ) -> Result<Self, FetchDirectedPrefetcherError> {
        if end.get() < start.get() {
            return Err(FetchDirectedPrefetcherError::TargetEndBeforeStart { start, end });
        }

        Ok(Self {
            context,
            fetch_target_id,
            start,
            end,
            secure,
        })
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn fetch_target_id(&self) -> u64 {
        self.fetch_target_id
    }

    pub const fn start(&self) -> Address {
        self.start
    }

    pub const fn end(&self) -> Address {
        self.end
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FetchDirectedCacheLookup {
    Miss,
    Hit,
    MissQueueHit,
}

impl FetchDirectedCacheLookup {
    pub const fn is_redundant(self) -> bool {
        !matches!(self, Self::Miss)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FetchDirectedTranslation {
    physical_address: Address,
    uncacheable: bool,
    cache_lookup: FetchDirectedCacheLookup,
}

impl FetchDirectedTranslation {
    pub const fn new(
        physical_address: Address,
        uncacheable: bool,
        cache_lookup: FetchDirectedCacheLookup,
    ) -> Self {
        Self {
            physical_address,
            uncacheable,
            cache_lookup,
        }
    }

    pub const fn physical_address(&self) -> Address {
        self.physical_address
    }

    pub const fn uncacheable(&self) -> bool {
        self.uncacheable
    }

    pub const fn cache_lookup(&self) -> FetchDirectedCacheLookup {
        self.cache_lookup
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FetchDirectedTranslationOutcome {
    Queued,
    Canceled,
    Uncacheable,
    Redundant,
    PrefetchQueueFull,
    TranslationFailed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchDirectedInsertSummary {
    identified: u64,
    already_in_prefetch_queue: u64,
    already_in_translation_queue: u64,
    translation_queue_inserts: u64,
    translation_queue_drops: u64,
}

impl FetchDirectedInsertSummary {
    pub const fn identified(&self) -> u64 {
        self.identified
    }

    pub const fn already_in_prefetch_queue(&self) -> u64 {
        self.already_in_prefetch_queue
    }

    pub const fn already_in_translation_queue(&self) -> u64 {
        self.already_in_translation_queue
    }

    pub const fn translation_queue_inserts(&self) -> u64 {
        self.translation_queue_inserts
    }

    pub const fn translation_queue_drops(&self) -> u64 {
        self.translation_queue_drops
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchDirectedRemoveSummary {
    translation_queue_canceled: u64,
    prefetch_queue_removed: u64,
}

impl FetchDirectedRemoveSummary {
    pub const fn translation_queue_canceled(&self) -> u64 {
        self.translation_queue_canceled
    }

    pub const fn prefetch_queue_removed(&self) -> u64 {
        self.prefetch_queue_removed
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchDirectedQueueOccupancySnapshot {
    samples: u64,
    total_entries: u64,
    minimum_entries: Option<u64>,
    maximum_entries: Option<u64>,
    last_entries: Option<u64>,
}

impl FetchDirectedQueueOccupancySnapshot {
    pub const fn samples(&self) -> u64 {
        self.samples
    }

    pub const fn total_entries(&self) -> u64 {
        self.total_entries
    }

    pub const fn minimum_entries(&self) -> Option<u64> {
        self.minimum_entries
    }

    pub const fn maximum_entries(&self) -> Option<u64> {
        self.maximum_entries
    }

    pub const fn last_entries(&self) -> Option<u64> {
        self.last_entries
    }

    fn record(&mut self, entries: usize) {
        let entries = entries as u64;
        self.samples = self.samples.saturating_add(1);
        self.total_entries = self.total_entries.saturating_add(entries);
        self.minimum_entries = Some(
            self.minimum_entries
                .map_or(entries, |value| value.min(entries)),
        );
        self.maximum_entries = Some(
            self.maximum_entries
                .map_or(entries, |value| value.max(entries)),
        );
        self.last_entries = Some(entries);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchDirectedStatsSnapshot {
    prefetches_identified: u64,
    prefetches_squashed: u64,
    prefetches_in_prefetch_queue: u64,
    prefetches_in_translation_queue: u64,
    prefetches_in_cache: u64,
    prefetch_packets_created: u64,
    prefetch_candidates_added: u64,
    prefetches_issued: u64,
    translation_failures: u64,
    translation_successes: u64,
    prefetch_queue_inserts: u64,
    prefetch_queue_pops: u64,
    prefetch_queue_drops: u64,
    translation_queue_inserts: u64,
    translation_queue_pops: u64,
    translation_queue_drops: u64,
    prefetch_queue_occupancy_at_fetch_target_insert: FetchDirectedQueueOccupancySnapshot,
    translation_queue_occupancy_at_fetch_target_insert: FetchDirectedQueueOccupancySnapshot,
}

impl FetchDirectedStatsSnapshot {
    pub const fn prefetches_identified(&self) -> u64 {
        self.prefetches_identified
    }

    pub const fn prefetches_squashed(&self) -> u64 {
        self.prefetches_squashed
    }

    pub const fn prefetches_in_prefetch_queue(&self) -> u64 {
        self.prefetches_in_prefetch_queue
    }

    pub const fn prefetches_in_translation_queue(&self) -> u64 {
        self.prefetches_in_translation_queue
    }

    pub const fn prefetches_in_cache(&self) -> u64 {
        self.prefetches_in_cache
    }

    pub const fn prefetch_packets_created(&self) -> u64 {
        self.prefetch_packets_created
    }

    pub const fn prefetch_candidates_added(&self) -> u64 {
        self.prefetch_candidates_added
    }

    pub const fn prefetches_issued(&self) -> u64 {
        self.prefetches_issued
    }

    pub const fn translation_failures(&self) -> u64 {
        self.translation_failures
    }

    pub const fn translation_successes(&self) -> u64 {
        self.translation_successes
    }

    pub const fn prefetch_queue_inserts(&self) -> u64 {
        self.prefetch_queue_inserts
    }

    pub const fn prefetch_queue_pops(&self) -> u64 {
        self.prefetch_queue_pops
    }

    pub const fn prefetch_queue_drops(&self) -> u64 {
        self.prefetch_queue_drops
    }

    pub const fn translation_queue_inserts(&self) -> u64 {
        self.translation_queue_inserts
    }

    pub const fn translation_queue_pops(&self) -> u64 {
        self.translation_queue_pops
    }

    pub const fn translation_queue_drops(&self) -> u64 {
        self.translation_queue_drops
    }

    pub const fn prefetch_queue_occupancy_at_fetch_target_insert(
        &self,
    ) -> &FetchDirectedQueueOccupancySnapshot {
        &self.prefetch_queue_occupancy_at_fetch_target_insert
    }

    pub const fn translation_queue_occupancy_at_fetch_target_insert(
        &self,
    ) -> &FetchDirectedQueueOccupancySnapshot {
        &self.translation_queue_occupancy_at_fetch_target_insert
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchDirectedPrefetchIssue {
    address: Address,
    virtual_block: Address,
    context: AgentId,
    fetch_target_id: u64,
    secure: bool,
    ready_tick: u64,
    marked_as_prefetch: bool,
}

impl FetchDirectedPrefetchIssue {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn virtual_block(&self) -> Address {
        self.virtual_block
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn fetch_target_id(&self) -> u64 {
        self.fetch_target_id
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn marked_as_prefetch(&self) -> bool {
        self.marked_as_prefetch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchDirectedTranslationEntrySnapshot {
    virtual_block: Address,
    context: AgentId,
    fetch_target_id: u64,
    secure: bool,
    canceled: bool,
}

impl FetchDirectedTranslationEntrySnapshot {
    pub const fn virtual_block(&self) -> Address {
        self.virtual_block
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn fetch_target_id(&self) -> u64 {
        self.fetch_target_id
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn canceled(&self) -> bool {
        self.canceled
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchDirectedPrefetchQueueEntrySnapshot {
    address: Address,
    virtual_block: Address,
    context: AgentId,
    fetch_target_id: u64,
    secure: bool,
    ready_tick: u64,
    marked_as_prefetch: bool,
}

impl FetchDirectedPrefetchQueueEntrySnapshot {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn virtual_block(&self) -> Address {
        self.virtual_block
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn fetch_target_id(&self) -> u64 {
        self.fetch_target_id
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn marked_as_prefetch(&self) -> bool {
        self.marked_as_prefetch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchDirectedPrefetcherSnapshot {
    config: FetchDirectedPrefetcherConfig,
    translation_queue: Vec<FetchDirectedTranslationEntrySnapshot>,
    prefetch_queue: Vec<FetchDirectedPrefetchQueueEntrySnapshot>,
    stats: FetchDirectedStatsSnapshot,
}

impl FetchDirectedPrefetcherSnapshot {
    pub const fn config(&self) -> &FetchDirectedPrefetcherConfig {
        &self.config
    }

    pub fn translation_queue(&self) -> &[FetchDirectedTranslationEntrySnapshot] {
        &self.translation_queue
    }

    pub fn prefetch_queue(&self) -> &[FetchDirectedPrefetchQueueEntrySnapshot] {
        &self.prefetch_queue
    }

    pub const fn stats(&self) -> &FetchDirectedStatsSnapshot {
        &self.stats
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FetchDirectedTranslationEntry {
    virtual_block: Address,
    context: AgentId,
    fetch_target_id: u64,
    secure: bool,
    canceled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FetchDirectedPrefetchQueueEntry {
    address: Address,
    virtual_block: Address,
    context: AgentId,
    fetch_target_id: u64,
    secure: bool,
    ready_tick: u64,
    marked_as_prefetch: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchDirectedPrefetcher {
    config: FetchDirectedPrefetcherConfig,
    translation_queue: VecDeque<FetchDirectedTranslationEntry>,
    prefetch_queue: VecDeque<FetchDirectedPrefetchQueueEntry>,
    stats: FetchDirectedStatsSnapshot,
}

impl FetchDirectedPrefetcher {
    pub fn new(config: FetchDirectedPrefetcherConfig) -> Self {
        Self {
            config,
            translation_queue: VecDeque::new(),
            prefetch_queue: VecDeque::new(),
            stats: FetchDirectedStatsSnapshot::default(),
        }
    }

    pub const fn config(&self) -> &FetchDirectedPrefetcherConfig {
        &self.config
    }

    pub fn notify_fetch_target_insert(
        &mut self,
        target: FetchDirectedTarget,
    ) -> Result<FetchDirectedInsertSummary, FetchDirectedPrefetcherError> {
        let mut summary = FetchDirectedInsertSummary::default();
        let end_block = self.block_address(target.end());
        let mut block = self.block_address(target.start());

        loop {
            if self
                .prefetch_queue
                .iter()
                .any(|entry| entry.virtual_block == block)
            {
                summary.already_in_prefetch_queue += 1;
                self.stats.prefetches_in_prefetch_queue += 1;
            } else if self
                .translation_queue
                .iter()
                .any(|entry| entry.virtual_block == block)
            {
                summary.already_in_translation_queue += 1;
                self.stats.prefetches_in_translation_queue += 1;
            } else {
                summary.identified += 1;
                self.stats.prefetches_identified += 1;

                if self.translation_queue.len() >= self.config.translation_queue_entries() {
                    summary.translation_queue_drops += 1;
                    self.stats.translation_queue_drops += 1;
                } else {
                    self.translation_queue
                        .push_back(FetchDirectedTranslationEntry {
                            virtual_block: block,
                            context: target.context(),
                            fetch_target_id: target.fetch_target_id(),
                            secure: target.secure(),
                            canceled: false,
                        });
                    summary.translation_queue_inserts += 1;
                    self.stats.translation_queue_inserts += 1;
                    self.stats
                        .translation_queue_occupancy_at_fetch_target_insert
                        .record(self.translation_queue.len());
                    self.stats
                        .prefetch_queue_occupancy_at_fetch_target_insert
                        .record(self.prefetch_queue.len());
                }
            }

            if block == end_block {
                break;
            }
            let Some(next) = block.get().checked_add(self.config.line_size()) else {
                break;
            };
            block = Address::new(next);
        }

        Ok(summary)
    }

    pub fn notify_fetch_target_remove(
        &mut self,
        fetch_target_id: u64,
    ) -> FetchDirectedRemoveSummary {
        let mut summary = FetchDirectedRemoveSummary::default();
        if !self.config.squash_prefetches() {
            return summary;
        }

        for entry in &mut self.translation_queue {
            if entry.fetch_target_id == fetch_target_id && !entry.canceled {
                entry.canceled = true;
                summary.translation_queue_canceled += 1;
                self.stats.prefetches_squashed += 1;
            }
        }

        let original_len = self.prefetch_queue.len();
        self.prefetch_queue
            .retain(|entry| entry.fetch_target_id != fetch_target_id);
        summary.prefetch_queue_removed = (original_len - self.prefetch_queue.len()) as u64;
        self.stats.prefetches_squashed += summary.prefetch_queue_removed;
        summary
    }

    pub fn complete_translation(
        &mut self,
        tick: u64,
        fetch_target_id: u64,
        virtual_block: Address,
        translation: Result<FetchDirectedTranslation, ()>,
    ) -> Result<FetchDirectedTranslationOutcome, FetchDirectedPrefetcherError> {
        let virtual_block = self.block_address(virtual_block);
        let Some(position) = self.translation_queue.iter().position(|entry| {
            entry.fetch_target_id == fetch_target_id && entry.virtual_block == virtual_block
        }) else {
            return Err(FetchDirectedPrefetcherError::UnknownTranslation {
                fetch_target_id,
                virtual_block,
            });
        };
        let entry = self
            .translation_queue
            .remove(position)
            .expect("translation queue position was found");
        self.stats.translation_queue_pops += 1;

        let Ok(translation) = translation else {
            self.stats.translation_failures += 1;
            return Ok(FetchDirectedTranslationOutcome::TranslationFailed);
        };

        self.stats.translation_successes += 1;

        if entry.canceled {
            return Ok(FetchDirectedTranslationOutcome::Canceled);
        }
        if translation.uncacheable() {
            return Ok(FetchDirectedTranslationOutcome::Uncacheable);
        }
        if self.config.cache_snoop() && translation.cache_lookup().is_redundant() {
            self.stats.prefetches_in_cache += 1;
            return Ok(FetchDirectedTranslationOutcome::Redundant);
        }
        if self.prefetch_queue.len() >= self.config.prefetch_queue_entries() {
            self.stats.prefetch_queue_drops += 1;
            return Ok(FetchDirectedTranslationOutcome::PrefetchQueueFull);
        }

        let ready_tick = tick.checked_add(self.config.latency()).ok_or(
            FetchDirectedPrefetcherError::ReadyTickOverflow {
                tick,
                latency: self.config.latency(),
            },
        )?;
        self.prefetch_queue
            .push_back(FetchDirectedPrefetchQueueEntry {
                address: self.block_address(translation.physical_address()),
                virtual_block: entry.virtual_block,
                context: entry.context,
                fetch_target_id: entry.fetch_target_id,
                secure: entry.secure,
                ready_tick,
                marked_as_prefetch: self.config.mark_requests_as_prefetch(),
            });
        self.stats.prefetch_packets_created += 1;
        self.stats.prefetch_candidates_added += 1;
        self.stats.prefetch_queue_inserts += 1;
        Ok(FetchDirectedTranslationOutcome::Queued)
    }

    pub fn issue_ready(&mut self, tick: u64) -> Option<FetchDirectedPrefetchIssue> {
        if self
            .prefetch_queue
            .front()
            .is_none_or(|entry| entry.ready_tick > tick)
        {
            return None;
        }

        let entry = self.prefetch_queue.pop_front()?;
        self.stats.prefetch_queue_pops += 1;
        self.stats.prefetches_issued += 1;
        Some(FetchDirectedPrefetchIssue {
            address: entry.address,
            virtual_block: entry.virtual_block,
            context: entry.context,
            fetch_target_id: entry.fetch_target_id,
            secure: entry.secure,
            ready_tick: entry.ready_tick,
            marked_as_prefetch: entry.marked_as_prefetch,
        })
    }

    pub fn next_prefetch_ready_tick(&self) -> Option<u64> {
        self.prefetch_queue.front().map(|entry| entry.ready_tick)
    }

    pub fn translation_queue_len(&self) -> usize {
        self.translation_queue.len()
    }

    pub fn prefetch_queue_len(&self) -> usize {
        self.prefetch_queue.len()
    }

    pub const fn stats(&self) -> &FetchDirectedStatsSnapshot {
        &self.stats
    }

    pub fn snapshot(&self) -> FetchDirectedPrefetcherSnapshot {
        FetchDirectedPrefetcherSnapshot {
            config: self.config.clone(),
            translation_queue: self
                .translation_queue
                .iter()
                .map(|entry| FetchDirectedTranslationEntrySnapshot {
                    virtual_block: entry.virtual_block,
                    context: entry.context,
                    fetch_target_id: entry.fetch_target_id,
                    secure: entry.secure,
                    canceled: entry.canceled,
                })
                .collect(),
            prefetch_queue: self
                .prefetch_queue
                .iter()
                .map(|entry| FetchDirectedPrefetchQueueEntrySnapshot {
                    address: entry.address,
                    virtual_block: entry.virtual_block,
                    context: entry.context,
                    fetch_target_id: entry.fetch_target_id,
                    secure: entry.secure,
                    ready_tick: entry.ready_tick,
                    marked_as_prefetch: entry.marked_as_prefetch,
                })
                .collect(),
            stats: self.stats.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &FetchDirectedPrefetcherSnapshot,
    ) -> Result<(), FetchDirectedPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(FetchDirectedPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.prefetch_queue().len() > self.config.prefetch_queue_entries() {
            return Err(
                FetchDirectedPrefetcherError::SnapshotPrefetchQueueTooLarge {
                    entries: snapshot.prefetch_queue().len(),
                    max_entries: self.config.prefetch_queue_entries(),
                },
            );
        }
        if snapshot.translation_queue().len() > self.config.translation_queue_entries() {
            return Err(
                FetchDirectedPrefetcherError::SnapshotTranslationQueueTooLarge {
                    entries: snapshot.translation_queue().len(),
                    max_entries: self.config.translation_queue_entries(),
                },
            );
        }

        self.translation_queue = snapshot
            .translation_queue()
            .iter()
            .map(|entry| FetchDirectedTranslationEntry {
                virtual_block: entry.virtual_block(),
                context: entry.context(),
                fetch_target_id: entry.fetch_target_id(),
                secure: entry.secure(),
                canceled: entry.canceled(),
            })
            .collect();
        self.prefetch_queue = snapshot
            .prefetch_queue()
            .iter()
            .map(|entry| FetchDirectedPrefetchQueueEntry {
                address: entry.address(),
                virtual_block: entry.virtual_block(),
                context: entry.context(),
                fetch_target_id: entry.fetch_target_id(),
                secure: entry.secure(),
                ready_tick: entry.ready_tick(),
                marked_as_prefetch: entry.marked_as_prefetch(),
            })
            .collect();
        self.stats = snapshot.stats().clone();
        Ok(())
    }

    fn block_address(&self, address: Address) -> Address {
        Address::new(address.get() / self.config.line_size() * self.config.line_size())
    }
}
