use std::collections::{BTreeMap, VecDeque};

use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};

use crate::{
    common::{
        checked_counter_add, TrafficGeneratorSummary, TrafficRequestEvent, TrafficRequestKind,
        TrafficRng,
    },
    TrafficGeneratorError,
};

const ELEMENT_BYTES: u64 = 8;
const DEFAULT_RNG_STATE: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficGupsConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    start: Address,
    mem_size: u64,
    update_limit: Option<u64>,
    rng_state: u64,
}

impl TrafficGupsConfig {
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        start: Address,
        mem_size: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_memory_range(start, mem_size)?;

        Ok(Self {
            agent,
            line_layout,
            start,
            mem_size,
            update_limit: None,
            rng_state: DEFAULT_RNG_STATE,
        })
    }

    pub const fn with_update_limit(
        mut self,
        update_limit: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        self.update_limit = if update_limit == 0 {
            None
        } else {
            Some(update_limit)
        };
        Ok(self)
    }

    pub const fn with_rng_state(mut self, rng_state: u64) -> Self {
        self.rng_state = rng_state;
        self
    }

    pub const fn agent(self) -> AgentId {
        self.agent
    }

    pub const fn line_layout(self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn start(self) -> Address {
        self.start
    }

    pub const fn mem_size(self) -> u64 {
        self.mem_size
    }

    pub fn element_size(self) -> AccessSize {
        AccessSize::new(ELEMENT_BYTES).expect("GUPS element size is non-zero")
    }

    pub const fn table_size(self) -> u64 {
        self.mem_size / ELEMENT_BYTES
    }

    pub const fn update_limit(self) -> Option<u64> {
        self.update_limit
    }

    pub const fn target_updates(self) -> u64 {
        let default = self.table_size() * 4;
        match self.update_limit {
            Some(limit) if limit < default => limit,
            Some(_) | None => default,
        }
    }

    pub const fn rng_state(self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrafficGupsPendingRead {
    sequence: u64,
    address: Address,
    update_value: u64,
}

impl TrafficGupsPendingRead {
    const fn new(sequence: u64, address: Address, update_value: u64) -> Self {
        Self {
            sequence,
            address,
            update_value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrafficGupsPendingWrite {
    address: Address,
    value: u64,
}

impl TrafficGupsPendingWrite {
    const fn new(address: Address, value: u64) -> Self {
        Self { address, value }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficGupsSnapshot {
    config: TrafficGupsConfig,
    next_sequence: u64,
    reads_created: u64,
    summary: TrafficGeneratorSummary,
    rng_state: u64,
    pending_reads: Vec<TrafficGupsPendingRead>,
    pending_writes: Vec<TrafficGupsPendingWrite>,
}

impl TrafficGupsSnapshot {
    fn new(
        config: TrafficGupsConfig,
        next_sequence: u64,
        reads_created: u64,
        summary: TrafficGeneratorSummary,
        rng_state: u64,
        pending_reads: Vec<TrafficGupsPendingRead>,
        pending_writes: Vec<TrafficGupsPendingWrite>,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_snapshot(
            config,
            next_sequence,
            reads_created,
            &pending_reads,
            &pending_writes,
        )?;

        Ok(Self {
            config,
            next_sequence,
            reads_created,
            summary,
            rng_state,
            pending_reads,
            pending_writes,
        })
    }

    pub const fn config(&self) -> TrafficGupsConfig {
        self.config
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn reads_created(&self) -> u64 {
        self.reads_created
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GupsTrafficGenerator {
    config: TrafficGupsConfig,
    next_sequence: u64,
    reads_created: u64,
    summary: TrafficGeneratorSummary,
    rng: TrafficRng,
    pending_reads: BTreeMap<u64, TrafficGupsPendingRead>,
    pending_writes: VecDeque<TrafficGupsPendingWrite>,
}

impl GupsTrafficGenerator {
    pub fn new(config: TrafficGupsConfig) -> Self {
        Self {
            config,
            next_sequence: 0,
            reads_created: 0,
            summary: TrafficGeneratorSummary::default(),
            rng: TrafficRng::new(config.rng_state()),
            pending_reads: BTreeMap::new(),
            pending_writes: VecDeque::new(),
        }
    }

    pub fn enter(&mut self) {
        self.next_sequence = 0;
        self.reads_created = 0;
        self.summary = TrafficGeneratorSummary::default();
        self.rng = TrafficRng::new(self.config.rng_state());
        self.pending_reads.clear();
        self.pending_writes.clear();
    }

    pub fn restore(snapshot: TrafficGupsSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_snapshot(
            snapshot.config,
            snapshot.next_sequence,
            snapshot.reads_created,
            &snapshot.pending_reads,
            &snapshot.pending_writes,
        )?;

        Ok(Self {
            config: snapshot.config,
            next_sequence: snapshot.next_sequence,
            reads_created: snapshot.reads_created,
            summary: snapshot.summary,
            rng: TrafficRng::new(snapshot.rng_state),
            pending_reads: snapshot
                .pending_reads
                .into_iter()
                .map(|pending| (pending.sequence, pending))
                .collect(),
            pending_writes: snapshot.pending_writes.into(),
        })
    }

    pub fn next_request(
        &mut self,
        tick: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        if let Some(write) = self.pending_writes.pop_front() {
            return self.emit_write(tick, write);
        }
        if self.reads_created >= self.config.target_updates() {
            return Ok(None);
        }

        self.emit_read(tick)
    }

    pub fn schedule_tick(
        &self,
        tick: u64,
        _retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        if !self.pending_writes.is_empty() || self.reads_created < self.config.target_updates() {
            return next_cycle(tick);
        }
        Ok(u64::MAX)
    }

    pub fn complete_read(
        &mut self,
        sequence: u64,
        value: u64,
    ) -> Result<(), TrafficGeneratorError> {
        let pending = self
            .pending_reads
            .remove(&sequence)
            .ok_or(TrafficGeneratorError::TrafficGupsUnknownReadCompletion { sequence })?;
        self.pending_writes.push_back(TrafficGupsPendingWrite::new(
            pending.address,
            value ^ pending.update_value,
        ));
        Ok(())
    }

    pub fn snapshot(&self) -> TrafficGupsSnapshot {
        TrafficGupsSnapshot::new(
            self.config,
            self.next_sequence,
            self.reads_created,
            self.summary,
            self.rng.state(),
            self.pending_reads.values().copied().collect(),
            self.pending_writes.iter().copied().collect(),
        )
        .expect("live GUPS generator state satisfies snapshot invariants")
    }

    pub const fn config(&self) -> TrafficGupsConfig {
        self.config
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn reads_created(&self) -> u64 {
        self.reads_created
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub fn pending_read_count(&self) -> usize {
        self.pending_reads.len()
    }

    pub fn pending_write_count(&self) -> usize {
        self.pending_writes.len()
    }

    pub fn is_complete(&self) -> bool {
        self.reads_created >= self.config.target_updates()
            && self.pending_reads.is_empty()
            && self.pending_writes.is_empty()
    }

    fn emit_read(
        &mut self,
        tick: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let update_value = self.reads_created;
        let next_reads_created = checked_counter_add("gups.reads_created", self.reads_created, 1)?;
        let index = self.rng.next_inclusive(0, self.config.table_size() - 1);
        let address = Address::new(self.config.start().get() + index * ELEMENT_BYTES);
        let event_tick = next_cycle(tick)?;
        let request = self.build_read_request(sequence, address)?;
        let mut summary = self.summary;
        summary.record(event_tick, TrafficRequestKind::Read, ELEMENT_BYTES)?;

        self.pending_reads.insert(
            sequence,
            TrafficGupsPendingRead::new(sequence, address, update_value),
        );
        self.next_sequence = next_sequence;
        self.reads_created = next_reads_created;
        self.summary = summary;

        Ok(Some(TrafficRequestEvent::new(
            event_tick,
            sequence,
            TrafficRequestKind::Read,
            address,
            request,
        )))
    }

    fn emit_write(
        &mut self,
        tick: u64,
        write: TrafficGupsPendingWrite,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let event_tick = next_cycle(tick)?;
        let request = self.build_write_request(sequence, write)?;
        let mut summary = self.summary;
        summary.record(event_tick, TrafficRequestKind::Write, ELEMENT_BYTES)?;

        self.next_sequence = next_sequence;
        self.summary = summary;

        Ok(Some(TrafficRequestEvent::new(
            event_tick,
            sequence,
            TrafficRequestKind::Write,
            write.address,
            request,
        )))
    }

    fn build_read_request(
        &self,
        sequence: u64,
        address: Address,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        MemoryRequest::read_shared(
            MemoryRequestId::new(self.config.agent(), sequence),
            address,
            self.config.element_size(),
            self.config.line_layout(),
        )
        .map_err(Into::into)
    }

    fn build_write_request(
        &self,
        sequence: u64,
        write: TrafficGupsPendingWrite,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        let size = self.config.element_size();
        MemoryRequest::write(
            MemoryRequestId::new(self.config.agent(), sequence),
            write.address,
            size,
            write.value.to_le_bytes().to_vec(),
            ByteMask::full(size)?,
            self.config.line_layout(),
        )
        .map_err(Into::into)
    }
}

fn next_cycle(tick: u64) -> Result<u64, TrafficGeneratorError> {
    tick.checked_add(1)
        .ok_or(TrafficGeneratorError::TickOverflow { tick, delta: 1 })
}

fn validate_memory_range(start: Address, mem_size: u64) -> Result<(), TrafficGeneratorError> {
    if mem_size == 0 {
        return Err(TrafficGeneratorError::TrafficGupsZeroMemorySize);
    }
    if !mem_size.is_multiple_of(ELEMENT_BYTES) {
        return Err(TrafficGeneratorError::TrafficGupsMemorySizeNotMultiple {
            mem_size,
            element_size: ELEMENT_BYTES,
        });
    }
    if start.get().checked_add(mem_size).is_none() {
        return Err(TrafficGeneratorError::TrafficGupsAddressRangeOverflow { start, mem_size });
    }
    Ok(())
}

fn validate_snapshot(
    config: TrafficGupsConfig,
    next_sequence: u64,
    reads_created: u64,
    pending_reads: &[TrafficGupsPendingRead],
    pending_writes: &[TrafficGupsPendingWrite],
) -> Result<(), TrafficGeneratorError> {
    if reads_created > config.target_updates() {
        return Err(
            TrafficGeneratorError::TrafficGupsSnapshotReadCountOutsideTarget {
                reads_created,
                target_updates: config.target_updates(),
            },
        );
    }

    let end = config.start().get() + config.mem_size();
    let mut seen = BTreeMap::new();
    for pending in pending_reads {
        if pending.sequence >= next_sequence {
            return Err(
                TrafficGeneratorError::TrafficGupsSnapshotPendingReadFutureSequence {
                    sequence: pending.sequence,
                    next_sequence,
                },
            );
        }
        if seen.insert(pending.sequence, ()).is_some() {
            return Err(
                TrafficGeneratorError::TrafficGupsSnapshotPendingReadDuplicate {
                    sequence: pending.sequence,
                },
            );
        }
        validate_table_address(config, end, pending.address)?;
    }
    for pending in pending_writes {
        validate_table_address(config, end, pending.address)?;
    }
    Ok(())
}

fn validate_table_address(
    config: TrafficGupsConfig,
    end: u64,
    address: Address,
) -> Result<(), TrafficGeneratorError> {
    let address = address.get();
    if address < config.start().get() || address + ELEMENT_BYTES > end {
        return Err(
            TrafficGeneratorError::TrafficGupsSnapshotAddressOutsideRange {
                address: Address::new(address),
                start: config.start(),
                end: Address::new(end),
            },
        );
    }
    if !(address - config.start().get()).is_multiple_of(ELEMENT_BYTES) {
        return Err(
            TrafficGeneratorError::TrafficGupsSnapshotAddressOutsideRange {
                address: Address::new(address),
                start: config.start(),
                end: Address::new(end),
            },
        );
    }
    Ok(())
}
