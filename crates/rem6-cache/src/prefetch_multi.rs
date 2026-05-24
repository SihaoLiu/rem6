use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::{QueuedPrefetchIssue, QueuedPrefetcher};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiQueuedPrefetcherError {
    NoSources,
}

impl fmt::Display for MultiQueuedPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSources => write!(formatter, "multi queued prefetcher has no sources"),
        }
    }
}

impl Error for MultiQueuedPrefetcherError {}

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
}

impl MultiQueuedPrefetcher {
    pub fn new(sources: Vec<QueuedPrefetcher>) -> Result<Self, MultiQueuedPrefetcherError> {
        if sources.is_empty() {
            return Err(MultiQueuedPrefetcherError::NoSources);
        }

        Ok(Self {
            sources,
            last_chosen_source: 0,
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

    pub fn next_ready_tick(&self) -> Option<u64> {
        self.sources
            .iter()
            .filter_map(QueuedPrefetcher::next_ready_tick)
            .min()
    }

    pub fn issue_ready(&mut self, tick: u64) -> Option<MultiQueuedPrefetchIssue> {
        let mut source_index = (self.last_chosen_source + 1) % self.sources.len();

        for _ in 0..self.sources.len() {
            if self.sources[source_index]
                .next_ready_tick()
                .is_some_and(|ready| ready <= tick)
            {
                let issue = self.sources[source_index].issue_one_ready(tick)?;
                self.last_chosen_source = source_index;
                return Some(MultiQueuedPrefetchIssue {
                    source_index,
                    issue,
                });
            }
            source_index = (source_index + 1) % self.sources.len();
        }

        None
    }
}
