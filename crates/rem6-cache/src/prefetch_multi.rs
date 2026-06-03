use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::{
    QueuedPrefetchIssue, QueuedPrefetchStatsSnapshot, QueuedPrefetcher, QueuedPrefetcherError,
    QueuedPrefetcherSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiQueuedPrefetcherError {
    NoSources,
    SnapshotSourceCountMismatch {
        expected: usize,
        actual: usize,
    },
    SnapshotLastChosenSourceOutOfRange {
        last_chosen_source: usize,
        source_count: usize,
    },
    SnapshotSourceRestore {
        source_index: usize,
        source: QueuedPrefetcherError,
    },
}

impl fmt::Display for MultiQueuedPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSources => write!(formatter, "multi queued prefetcher has no sources"),
            Self::SnapshotSourceCountMismatch { expected, actual } => write!(
                formatter,
                "multi queued prefetcher snapshot has {actual} sources for {expected} live sources"
            ),
            Self::SnapshotLastChosenSourceOutOfRange {
                last_chosen_source,
                source_count,
            } => write!(
                formatter,
                "multi queued prefetcher snapshot last chosen source {last_chosen_source} is outside {source_count} sources"
            ),
            Self::SnapshotSourceRestore {
                source_index,
                source,
            } => write!(
                formatter,
                "multi queued prefetcher snapshot source {source_index} failed restore: {source}"
            ),
        }
    }
}

impl Error for MultiQueuedPrefetcherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SnapshotSourceRestore { source, .. } => Some(source),
            Self::NoSources
            | Self::SnapshotSourceCountMismatch { .. }
            | Self::SnapshotLastChosenSourceOutOfRange { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiQueuedPrefetcherSnapshot {
    sources: Vec<QueuedPrefetcherSnapshot>,
    last_chosen_source: usize,
    stats: QueuedPrefetchStatsSnapshot,
}

impl MultiQueuedPrefetcherSnapshot {
    pub fn sources(&self) -> &[QueuedPrefetcherSnapshot] {
        &self.sources
    }

    pub const fn last_chosen_source(&self) -> usize {
        self.last_chosen_source
    }

    pub const fn stats(&self) -> &QueuedPrefetchStatsSnapshot {
        &self.stats
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiQueuedPrefetchIssue {
    source_index: usize,
    issue: QueuedPrefetchIssue,
}

impl MultiQueuedPrefetchIssue {
    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    pub const fn issue(&self) -> &QueuedPrefetchIssue {
        &self.issue
    }

    pub const fn address(&self) -> Address {
        self.issue.address()
    }

    pub const fn context(&self) -> AgentId {
        self.issue.context()
    }

    pub const fn ready_tick(&self) -> u64 {
        self.issue.ready_tick()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiQueuedPrefetcher {
    sources: Vec<QueuedPrefetcher>,
    last_chosen_source: usize,
    stats: QueuedPrefetchStatsSnapshot,
}

impl MultiQueuedPrefetcher {
    pub fn new(sources: Vec<QueuedPrefetcher>) -> Result<Self, MultiQueuedPrefetcherError> {
        if sources.is_empty() {
            return Err(MultiQueuedPrefetcherError::NoSources);
        }

        Ok(Self {
            sources,
            last_chosen_source: 0,
            stats: QueuedPrefetchStatsSnapshot::default(),
        })
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn source(&self, index: usize) -> Option<&QueuedPrefetcher> {
        self.sources.get(index)
    }

    pub fn source_mut(&mut self, index: usize) -> Option<&mut QueuedPrefetcher> {
        self.sources.get_mut(index)
    }

    pub const fn stats(&self) -> &QueuedPrefetchStatsSnapshot {
        &self.stats
    }

    pub fn snapshot(&self) -> MultiQueuedPrefetcherSnapshot {
        MultiQueuedPrefetcherSnapshot {
            sources: self
                .sources
                .iter()
                .map(QueuedPrefetcher::snapshot)
                .collect(),
            last_chosen_source: self.last_chosen_source,
            stats: self.stats.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &MultiQueuedPrefetcherSnapshot,
    ) -> Result<(), MultiQueuedPrefetcherError> {
        if snapshot.sources().len() != self.sources.len() {
            return Err(MultiQueuedPrefetcherError::SnapshotSourceCountMismatch {
                expected: self.sources.len(),
                actual: snapshot.sources().len(),
            });
        }
        if snapshot.last_chosen_source() >= self.sources.len() {
            return Err(
                MultiQueuedPrefetcherError::SnapshotLastChosenSourceOutOfRange {
                    last_chosen_source: snapshot.last_chosen_source(),
                    source_count: self.sources.len(),
                },
            );
        }

        let mut sources = self.sources.clone();
        for (source_index, (source, source_snapshot)) in
            sources.iter_mut().zip(snapshot.sources()).enumerate()
        {
            source.restore(source_snapshot).map_err(|source| {
                MultiQueuedPrefetcherError::SnapshotSourceRestore {
                    source_index,
                    source,
                }
            })?;
        }

        self.sources = sources;
        self.last_chosen_source = snapshot.last_chosen_source();
        self.stats = snapshot.stats().clone();
        Ok(())
    }

    pub fn next_ready_tick(&self) -> Option<u64> {
        self.sources
            .iter()
            .filter_map(QueuedPrefetcher::next_ready_tick)
            .min()
    }

    pub fn issue_ready(&mut self, tick: u64) -> Option<MultiQueuedPrefetchIssue> {
        self.last_chosen_source = (self.last_chosen_source + 1) % self.sources.len();
        let mut source_index = self.last_chosen_source;

        for _ in 0..self.sources.len() {
            if self.sources[source_index]
                .next_ready_tick()
                .is_some_and(|ready| ready <= tick)
            {
                let issue = self.sources[source_index].issue_one_ready(tick)?;
                self.stats.record_issued(1);
                return Some(MultiQueuedPrefetchIssue {
                    source_index,
                    issue,
                });
            }
            source_index = (source_index + 1) % self.sources.len();
        }

        None
    }

    pub fn record_prefetch_unused(&mut self) {
        for source in &mut self.sources {
            source.record_prefetch_unused();
        }
    }

    pub fn record_demand_mshr_miss(&mut self) {
        for source in &mut self.sources {
            source.record_demand_mshr_miss();
        }
    }
}
