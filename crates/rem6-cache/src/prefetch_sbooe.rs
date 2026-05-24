use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbooePrefetcherConfig {
    line_size: u64,
    sequential_prefetchers: usize,
    sandbox_entries: usize,
    score_threshold_pct: u32,
    score_threshold: u32,
    latency_buffer_entries: usize,
}

impl SbooePrefetcherConfig {
    pub fn new(
        line_size: u64,
        sequential_prefetchers: usize,
        sandbox_entries: usize,
        score_threshold_pct: u32,
        latency_buffer_entries: usize,
    ) -> Result<Self, SbooePrefetcherError> {
        if line_size == 0 {
            return Err(SbooePrefetcherError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(SbooePrefetcherError::LineSizeNotPowerOfTwo { line_size });
        }
        if sequential_prefetchers == 0 {
            return Err(SbooePrefetcherError::ZeroSequentialPrefetchers);
        }
        if sandbox_entries == 0 {
            return Err(SbooePrefetcherError::ZeroSandboxEntries);
        }
        if score_threshold_pct > 100 {
            return Err(SbooePrefetcherError::ScoreThresholdOutOfRange {
                score_threshold_pct,
            });
        }
        if latency_buffer_entries == 0 {
            return Err(SbooePrefetcherError::ZeroLatencyBufferEntries);
        }

        let score_threshold = (sandbox_entries as u128 * score_threshold_pct as u128) / 100;

        Ok(Self {
            line_size,
            sequential_prefetchers,
            sandbox_entries,
            score_threshold_pct,
            score_threshold: score_threshold.min(u32::MAX as u128) as u32,
            latency_buffer_entries,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn sequential_prefetchers(&self) -> usize {
        self.sequential_prefetchers
    }

    pub const fn sandbox_entries(&self) -> usize {
        self.sandbox_entries
    }

    pub const fn score_threshold_pct(&self) -> u32 {
        self.score_threshold_pct
    }

    pub const fn score_threshold(&self) -> u32 {
        self.score_threshold
    }

    pub const fn latency_buffer_entries(&self) -> usize {
        self.latency_buffer_entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SbooePrefetcherError {
    ZeroLineSize,
    ZeroSequentialPrefetchers,
    ZeroSandboxEntries,
    ZeroLatencyBufferEntries,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    ScoreThresholdOutOfRange {
        score_threshold_pct: u32,
    },
    SnapshotConfigMismatch {
        expected: Box<SbooePrefetcherConfig>,
        actual: Box<SbooePrefetcherConfig>,
    },
    SnapshotSandboxShapeMismatch {
        sandboxes: usize,
        expected: usize,
    },
    SnapshotSandboxEntryShapeMismatch {
        sandbox: usize,
        entries: usize,
        max_entries: usize,
    },
    SnapshotLatencyBufferShapeMismatch {
        entries: usize,
        max_entries: usize,
    },
}

impl fmt::Display for SbooePrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "SBOOE line size is zero"),
            Self::ZeroSequentialPrefetchers => {
                write!(formatter, "SBOOE has no sequential prefetchers")
            }
            Self::ZeroSandboxEntries => write!(formatter, "SBOOE sandbox has no entries"),
            Self::ZeroLatencyBufferEntries => {
                write!(formatter, "SBOOE latency buffer has no entries")
            }
            Self::LineSizeNotPowerOfTwo { line_size } => {
                write!(formatter, "SBOOE line size {line_size} is not a power of two")
            }
            Self::ScoreThresholdOutOfRange {
                score_threshold_pct,
            } => write!(
                formatter,
                "SBOOE score threshold {score_threshold_pct}% is outside 0..=100"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "SBOOE snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotSandboxShapeMismatch {
                sandboxes,
                expected,
            } => write!(
                formatter,
                "SBOOE snapshot has {sandboxes} sandboxes instead of {expected}"
            ),
            Self::SnapshotSandboxEntryShapeMismatch {
                sandbox,
                entries,
                max_entries,
            } => write!(
                formatter,
                "SBOOE snapshot sandbox {sandbox} has {entries} entries but accepts at most {max_entries}"
            ),
            Self::SnapshotLatencyBufferShapeMismatch {
                entries,
                max_entries,
            } => write!(
                formatter,
                "SBOOE snapshot latency buffer has {entries} entries but accepts at most {max_entries}"
            ),
        }
    }
}

impl Error for SbooePrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SbooePrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl SbooePrefetchAccess {
    pub const fn new(requestor: AgentId, pc: u64, address: Address, secure: bool) -> Self {
        Self {
            requestor,
            pc,
            address,
            secure,
        }
    }

    pub const fn requestor(&self) -> AgentId {
        self.requestor
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbooePrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    sandbox_stride: i32,
    stride: i64,
    degree_index: u32,
}

impl SbooePrefetchCandidate {
    fn new(
        address: Address,
        access: SbooePrefetchAccess,
        sandbox_stride: i32,
        stride: i64,
    ) -> Self {
        Self {
            address,
            source_address: access.address(),
            context: access.requestor(),
            pc: access.pc(),
            secure: access.secure(),
            sandbox_stride,
            stride,
            degree_index: 1,
        }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn sandbox_stride(&self) -> i32 {
        self.sandbox_stride
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for SbooePrefetchCandidate {
    fn address(&self) -> Address {
        self.address()
    }

    fn source_address(&self) -> Address {
        self.source_address()
    }

    fn context(&self) -> AgentId {
        self.context()
    }

    fn pc(&self) -> u64 {
        self.pc()
    }

    fn secure(&self) -> bool {
        self.secure()
    }

    fn stride(&self) -> i64 {
        self.stride()
    }

    fn degree_index(&self) -> u32 {
        self.degree_index()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SbooeSandboxEntrySnapshot {
    line: u64,
    expected_arrival_tick: u64,
}

impl SbooeSandboxEntrySnapshot {
    const fn new(line: u64, expected_arrival_tick: u64) -> Self {
        Self {
            line,
            expected_arrival_tick,
        }
    }

    pub const fn line(&self) -> u64 {
        self.line
    }

    pub const fn expected_arrival_tick(&self) -> u64 {
        self.expected_arrival_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbooeSandboxSnapshot {
    stride: i32,
    sandbox_score: u32,
    late_score: u32,
    entries: Vec<SbooeSandboxEntrySnapshot>,
}

impl SbooeSandboxSnapshot {
    pub const fn stride(&self) -> i32 {
        self.stride
    }

    pub const fn sandbox_score(&self) -> u32 {
        self.sandbox_score
    }

    pub const fn late_score(&self) -> u32 {
        self.late_score
    }

    pub fn entries(&self) -> &[SbooeSandboxEntrySnapshot] {
        &self.entries
    }

    pub fn score(&self) -> u32 {
        self.sandbox_score.saturating_sub(self.late_score)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbooePrefetcherSnapshot {
    config: SbooePrefetcherConfig,
    sandboxes: Vec<SbooeSandboxSnapshot>,
    best_sandbox: Option<usize>,
    accesses: u64,
    demand_addresses: Vec<(Address, u64)>,
    latency_buffer: Vec<u64>,
    latency_buffer_sum: u64,
    average_access_latency: u64,
    last_candidates: Vec<SbooePrefetchCandidate>,
}

impl SbooePrefetcherSnapshot {
    pub const fn config(&self) -> &SbooePrefetcherConfig {
        &self.config
    }

    pub fn sandboxes(&self) -> &[SbooeSandboxSnapshot] {
        &self.sandboxes
    }

    pub const fn best_sandbox(&self) -> Option<usize> {
        self.best_sandbox
    }

    pub const fn accesses(&self) -> u64 {
        self.accesses
    }

    pub fn demand_addresses(&self) -> &[(Address, u64)] {
        &self.demand_addresses
    }

    pub fn latency_buffer(&self) -> &[u64] {
        &self.latency_buffer
    }

    pub const fn latency_buffer_sum(&self) -> u64 {
        self.latency_buffer_sum
    }

    pub const fn average_access_latency(&self) -> u64 {
        self.average_access_latency
    }

    pub fn last_candidates(&self) -> &[SbooePrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SbooeSandbox {
    stride: i32,
    sandbox_score: u32,
    late_score: u32,
    entries: VecDeque<SbooeSandboxEntrySnapshot>,
}

impl SbooeSandbox {
    fn new(stride: i32) -> Self {
        Self {
            stride,
            sandbox_score: 0,
            late_score: 0,
            entries: VecDeque::new(),
        }
    }

    fn from_snapshot(snapshot: &SbooeSandboxSnapshot) -> Self {
        Self {
            stride: snapshot.stride(),
            sandbox_score: snapshot.sandbox_score(),
            late_score: snapshot.late_score(),
            entries: snapshot.entries().iter().copied().collect(),
        }
    }

    fn snapshot(&self) -> SbooeSandboxSnapshot {
        SbooeSandboxSnapshot {
            stride: self.stride,
            sandbox_score: self.sandbox_score,
            late_score: self.late_score,
            entries: self.entries.iter().copied().collect(),
        }
    }

    fn score(&self) -> u32 {
        self.sandbox_score.saturating_sub(self.late_score)
    }

    fn access(&mut self, line: u64, expected_arrival_tick: u64, tick: u64, max_entries: usize) {
        if self.entries.iter().any(|entry| entry.line() == line) {
            self.sandbox_score = self.sandbox_score.saturating_add(1);
            if self
                .entries
                .iter()
                .any(|entry| entry.line() == line && entry.expected_arrival_tick() > tick)
            {
                self.late_score = self.late_score.saturating_add(1);
            }
        }

        if let Some(next_line) = offset_line(line, self.stride) {
            if self.entries.len() == max_entries {
                self.entries.pop_front();
            }
            self.entries.push_back(SbooeSandboxEntrySnapshot::new(
                next_line,
                expected_arrival_tick,
            ));
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbooePrefetcher {
    config: SbooePrefetcherConfig,
    sandboxes: Vec<SbooeSandbox>,
    best_sandbox: Option<usize>,
    accesses: u64,
    demand_addresses: BTreeMap<Address, u64>,
    latency_buffer: VecDeque<u64>,
    latency_buffer_sum: u64,
    average_access_latency: u64,
    last_candidates: Vec<SbooePrefetchCandidate>,
}

impl SbooePrefetcher {
    pub fn new(config: SbooePrefetcherConfig) -> Self {
        let sandboxes = (0..config.sequential_prefetchers())
            .map(|index| SbooeSandbox::new(index as i32 - 1))
            .collect();
        Self {
            config,
            sandboxes,
            best_sandbox: None,
            accesses: 0,
            demand_addresses: BTreeMap::new(),
            latency_buffer: VecDeque::new(),
            latency_buffer_sum: 0,
            average_access_latency: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &SbooePrefetcherConfig {
        &self.config
    }

    pub fn observe_at(
        &mut self,
        tick: u64,
        access: SbooePrefetchAccess,
    ) -> Result<&[SbooePrefetchCandidate], SbooePrefetcherError> {
        self.last_candidates.clear();
        self.demand_addresses
            .entry(access.address())
            .or_insert(tick);
        let line = access.address().get() >> self.config.line_size().ilog2();
        let expected_arrival_tick = tick.saturating_add(self.average_access_latency);

        for index in 0..self.sandboxes.len() {
            self.sandboxes[index].access(
                line,
                expected_arrival_tick,
                tick,
                self.config.sandbox_entries(),
            );
            if self.best_sandbox.is_none()
                || self.sandboxes[index].score()
                    > self.sandboxes[self.best_sandbox.expect("best sandbox exists")].score()
            {
                self.best_sandbox = Some(index);
            }
        }

        self.accesses = self.accesses.saturating_add(1);
        if self.accesses >= self.sandboxes.len() as u64 {
            self.push_candidate(access, line);
        }

        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> SbooePrefetcherSnapshot {
        SbooePrefetcherSnapshot {
            config: self.config.clone(),
            sandboxes: self.sandboxes.iter().map(SbooeSandbox::snapshot).collect(),
            best_sandbox: self.best_sandbox,
            accesses: self.accesses,
            demand_addresses: self
                .demand_addresses
                .iter()
                .map(|(address, tick)| (*address, *tick))
                .collect(),
            latency_buffer: self.latency_buffer.iter().copied().collect(),
            latency_buffer_sum: self.latency_buffer_sum,
            average_access_latency: self.average_access_latency,
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &SbooePrefetcherSnapshot,
    ) -> Result<(), SbooePrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(SbooePrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.sandboxes().len() != self.config.sequential_prefetchers() {
            return Err(SbooePrefetcherError::SnapshotSandboxShapeMismatch {
                sandboxes: snapshot.sandboxes().len(),
                expected: self.config.sequential_prefetchers(),
            });
        }
        for (index, sandbox) in snapshot.sandboxes().iter().enumerate() {
            if sandbox.entries().len() > self.config.sandbox_entries() {
                return Err(SbooePrefetcherError::SnapshotSandboxEntryShapeMismatch {
                    sandbox: index,
                    entries: sandbox.entries().len(),
                    max_entries: self.config.sandbox_entries(),
                });
            }
        }
        if snapshot.latency_buffer().len() > self.config.latency_buffer_entries() {
            return Err(SbooePrefetcherError::SnapshotLatencyBufferShapeMismatch {
                entries: snapshot.latency_buffer().len(),
                max_entries: self.config.latency_buffer_entries(),
            });
        }

        self.sandboxes = snapshot
            .sandboxes()
            .iter()
            .map(SbooeSandbox::from_snapshot)
            .collect();
        self.best_sandbox = snapshot.best_sandbox();
        self.accesses = snapshot.accesses();
        self.demand_addresses = snapshot.demand_addresses().iter().copied().collect();
        self.latency_buffer = snapshot.latency_buffer().iter().copied().collect();
        self.latency_buffer_sum = snapshot.latency_buffer_sum();
        self.average_access_latency = snapshot.average_access_latency();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn observe_fill_at(&mut self, tick: u64, address: Address) {
        let Some(start_tick) = self.demand_addresses.remove(&address) else {
            return;
        };
        let elapsed = tick.saturating_sub(start_tick);
        if self.latency_buffer.len() == self.config.latency_buffer_entries() {
            if let Some(oldest) = self.latency_buffer.pop_front() {
                self.latency_buffer_sum = self.latency_buffer_sum.saturating_sub(oldest);
            }
        }
        self.latency_buffer.push_back(elapsed);
        self.latency_buffer_sum = self.latency_buffer_sum.saturating_add(elapsed);
        self.average_access_latency = self.latency_buffer_sum / self.latency_buffer.len() as u64;
    }

    pub fn best_sandbox_stride(&self) -> Option<i32> {
        self.best_sandbox.map(|index| self.sandboxes[index].stride)
    }

    pub fn sandbox_scores(&self) -> Vec<u32> {
        self.sandboxes.iter().map(SbooeSandbox::score).collect()
    }

    pub fn sandbox_raw_scores(&self) -> Vec<u32> {
        self.sandboxes
            .iter()
            .map(|sandbox| sandbox.sandbox_score)
            .collect()
    }

    pub fn sandbox_late_scores(&self) -> Vec<u32> {
        self.sandboxes
            .iter()
            .map(|sandbox| sandbox.late_score)
            .collect()
    }

    pub fn pending_demand_count(&self) -> usize {
        self.demand_addresses.len()
    }

    pub const fn average_access_latency(&self) -> u64 {
        self.average_access_latency
    }

    pub fn last_candidates(&self) -> &[SbooePrefetchCandidate] {
        &self.last_candidates
    }

    fn push_candidate(&mut self, access: SbooePrefetchAccess, line: u64) {
        let Some(best_index) = self.best_sandbox else {
            return;
        };
        let sandbox = &self.sandboxes[best_index];
        if sandbox.score() <= self.config.score_threshold() {
            return;
        }
        let Some(candidate_line) = offset_line(line, sandbox.stride) else {
            return;
        };
        let Some(address) = candidate_line.checked_mul(self.config.line_size()) else {
            return;
        };
        let stride = (sandbox.stride as i128) * self.config.line_size() as i128;
        self.last_candidates.push(SbooePrefetchCandidate::new(
            Address::new(address),
            access,
            sandbox.stride,
            stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
        ));
    }
}

fn offset_line(line: u64, stride: i32) -> Option<u64> {
    if stride >= 0 {
        line.checked_add(stride as u64)
    } else {
        line.checked_sub(stride.unsigned_abs() as u64)
    }
}
