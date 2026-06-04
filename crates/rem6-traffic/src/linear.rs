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

const DEFAULT_PERIOD: u64 = 1;
const DEFAULT_READ_PERCENT: u8 = 100;
const DEFAULT_RNG_STATE: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficLinearConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    start: Address,
    end: Address,
    block_size: AccessSize,
    min_period: u64,
    max_period: u64,
    read_percent: u8,
    data_limit: Option<u64>,
    elastic_requests: bool,
    rng_state: u64,
}

impl TrafficLinearConfig {
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        start: Address,
        end: Address,
        block_size: AccessSize,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_range(start, end, block_size, line_layout)?;

        Ok(Self {
            agent,
            line_layout,
            start,
            end,
            block_size,
            min_period: DEFAULT_PERIOD,
            max_period: DEFAULT_PERIOD,
            read_percent: DEFAULT_READ_PERCENT,
            data_limit: None,
            elastic_requests: true,
            rng_state: DEFAULT_RNG_STATE,
        })
    }

    pub fn with_period(
        mut self,
        min_period: u64,
        max_period: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        if min_period > max_period {
            return Err(TrafficGeneratorError::InvertedPeriod {
                min_period,
                max_period,
            });
        }

        self.min_period = min_period;
        self.max_period = max_period;
        Ok(self)
    }

    pub fn with_read_percent(mut self, read_percent: u8) -> Result<Self, TrafficGeneratorError> {
        if read_percent > 100 {
            return Err(TrafficGeneratorError::InvalidReadPercent { read_percent });
        }

        self.read_percent = read_percent;
        Ok(self)
    }

    pub const fn with_data_limit(mut self, data_limit: u64) -> Result<Self, TrafficGeneratorError> {
        self.data_limit = if data_limit == 0 {
            None
        } else {
            Some(data_limit)
        };
        Ok(self)
    }

    pub const fn with_elastic_requests(mut self, elastic_requests: bool) -> Self {
        self.elastic_requests = elastic_requests;
        self
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

    pub const fn end(self) -> Address {
        self.end
    }

    pub const fn block_size(self) -> AccessSize {
        self.block_size
    }

    pub const fn min_period(self) -> u64 {
        self.min_period
    }

    pub const fn max_period(self) -> u64 {
        self.max_period
    }

    pub const fn read_percent(self) -> u8 {
        self.read_percent
    }

    pub const fn data_limit(self) -> Option<u64> {
        self.data_limit
    }

    pub const fn elastic_requests(self) -> bool {
        self.elastic_requests
    }

    pub const fn rng_state(self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficLinearSnapshot {
    config: TrafficLinearConfig,
    next_address: Address,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng_state: u64,
}

impl TrafficLinearSnapshot {
    pub fn new(
        config: TrafficLinearConfig,
        next_address: Address,
        next_sequence: u64,
        data_manipulated: u64,
        summary: TrafficGeneratorSummary,
        rng_state: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_snapshot(config, next_address, data_manipulated)?;

        Ok(Self {
            config,
            next_address,
            next_sequence,
            data_manipulated,
            summary,
            rng_state,
        })
    }

    pub const fn config(&self) -> TrafficLinearConfig {
        self.config
    }

    pub const fn next_address(&self) -> Address {
        self.next_address
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn data_manipulated(&self) -> u64 {
        self.data_manipulated
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinearTrafficGenerator {
    config: TrafficLinearConfig,
    next_address: Address,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng: TrafficRng,
}

impl LinearTrafficGenerator {
    pub fn new(config: TrafficLinearConfig) -> Self {
        Self {
            config,
            next_address: config.start(),
            next_sequence: 0,
            data_manipulated: 0,
            summary: TrafficGeneratorSummary::default(),
            rng: TrafficRng::new(config.rng_state()),
        }
    }

    pub fn restore(snapshot: TrafficLinearSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_snapshot(
            snapshot.config(),
            snapshot.next_address(),
            snapshot.data_manipulated(),
        )?;

        Ok(Self {
            config: snapshot.config(),
            next_address: snapshot.next_address(),
            next_sequence: snapshot.next_sequence(),
            data_manipulated: snapshot.data_manipulated(),
            summary: snapshot.summary(),
            rng: TrafficRng::new(snapshot.rng_state()),
        })
    }

    pub fn enter(&mut self) {
        self.next_address = self.config.start();
        self.data_manipulated = 0;
    }

    pub fn next_request(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        if self.limit_reached() {
            return Ok(None);
        }

        let block_bytes = self.config.block_size().bytes();
        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let next_data_manipulated =
            checked_counter_add("data_manipulated", self.data_manipulated, block_bytes)?;
        let mut next_rng = self.rng.clone();
        let event_tick = Self::schedule_tick_with(self.config, &mut next_rng, tick, retry_delay)?;
        let kind = Self::next_kind_with(self.config, &mut next_rng);
        let address = self.next_address;
        let request = self.build_request(sequence, kind, address)?;
        let mut next_summary = self.summary;
        next_summary.record(event_tick, kind, block_bytes)?;

        self.advance_cursor();
        self.next_sequence = next_sequence;
        self.data_manipulated = next_data_manipulated;
        self.summary = next_summary;
        self.rng = next_rng;

        Ok(Some(TrafficRequestEvent::new(
            event_tick, sequence, kind, address, request,
        )))
    }

    pub fn schedule_tick(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        if self.limit_reached() {
            return Ok(u64::MAX);
        }

        Self::schedule_tick_with(self.config, &mut self.rng, tick, retry_delay)
    }

    fn schedule_tick_with(
        config: TrafficLinearConfig,
        rng: &mut TrafficRng,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        let mut wait = Self::next_wait_with(config, rng);
        if !config.elastic_requests() {
            wait = wait.saturating_sub(retry_delay);
        }

        tick.checked_add(wait)
            .ok_or(TrafficGeneratorError::TickOverflow { tick, delta: wait })
    }

    pub fn snapshot(&self) -> TrafficLinearSnapshot {
        TrafficLinearSnapshot::new(
            self.config,
            self.next_address,
            self.next_sequence,
            self.data_manipulated,
            self.summary,
            self.rng.state(),
        )
        .expect("live traffic generator state satisfies snapshot invariants")
    }

    pub const fn config(&self) -> TrafficLinearConfig {
        self.config
    }

    pub const fn next_address(&self) -> Address {
        self.next_address
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn data_manipulated(&self) -> u64 {
        self.data_manipulated
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    fn build_request(
        &self,
        sequence: u64,
        kind: TrafficRequestKind,
        address: Address,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        let id = MemoryRequestId::new(self.config.agent(), sequence);
        let size = self.config.block_size();
        let layout = self.config.line_layout();

        match kind {
            TrafficRequestKind::Read => {
                MemoryRequest::read_shared(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Write => {
                let mask = ByteMask::full(size)?;
                let data_len = usize::try_from(mask.len())
                    .expect("byte mask length fits usize after construction");
                let data = vec![self.config.agent().get() as u8; data_len];
                MemoryRequest::write(id, address, size, data, mask, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance => {
                unreachable!("linear traffic generator does not emit maintenance requests")
            }
        }
    }

    fn next_kind_with(config: TrafficLinearConfig, rng: &mut TrafficRng) -> TrafficRequestKind {
        match config.read_percent() {
            0 => TrafficRequestKind::Write,
            100 => TrafficRequestKind::Read,
            read_percent => {
                if rng.next_inclusive(0, 100) < u64::from(read_percent) {
                    TrafficRequestKind::Read
                } else {
                    TrafficRequestKind::Write
                }
            }
        }
    }

    fn next_wait_with(config: TrafficLinearConfig, rng: &mut TrafficRng) -> u64 {
        rng.next_inclusive(config.min_period(), config.max_period())
    }

    fn advance_cursor(&mut self) {
        let next = self.next_address.get() + self.config.block_size().bytes();
        self.next_address = if next >= self.config.end().get() {
            self.config.start()
        } else {
            Address::new(next)
        };
    }

    fn limit_reached(&self) -> bool {
        self.config
            .data_limit()
            .is_some_and(|limit| self.data_manipulated >= limit)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficRandomConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    start: Address,
    end: Address,
    block_size: AccessSize,
    min_period: u64,
    max_period: u64,
    read_percent: u8,
    data_limit: Option<u64>,
    elastic_requests: bool,
    rng_state: u64,
}

impl TrafficRandomConfig {
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        start: Address,
        end: Address,
        block_size: AccessSize,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_random_range(start, end, block_size, line_layout)?;

        Ok(Self {
            agent,
            line_layout,
            start,
            end,
            block_size,
            min_period: DEFAULT_PERIOD,
            max_period: DEFAULT_PERIOD,
            read_percent: DEFAULT_READ_PERCENT,
            data_limit: None,
            elastic_requests: true,
            rng_state: DEFAULT_RNG_STATE,
        })
    }

    pub fn with_period(
        mut self,
        min_period: u64,
        max_period: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        if min_period > max_period {
            return Err(TrafficGeneratorError::InvertedPeriod {
                min_period,
                max_period,
            });
        }

        self.min_period = min_period;
        self.max_period = max_period;
        Ok(self)
    }

    pub fn with_read_percent(mut self, read_percent: u8) -> Result<Self, TrafficGeneratorError> {
        if read_percent > 100 {
            return Err(TrafficGeneratorError::InvalidReadPercent { read_percent });
        }

        self.read_percent = read_percent;
        Ok(self)
    }

    pub const fn with_data_limit(mut self, data_limit: u64) -> Result<Self, TrafficGeneratorError> {
        self.data_limit = if data_limit == 0 {
            None
        } else {
            Some(data_limit)
        };
        Ok(self)
    }

    pub const fn with_elastic_requests(mut self, elastic_requests: bool) -> Self {
        self.elastic_requests = elastic_requests;
        self
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

    pub const fn end(self) -> Address {
        self.end
    }

    pub const fn block_size(self) -> AccessSize {
        self.block_size
    }

    pub const fn min_period(self) -> u64 {
        self.min_period
    }

    pub const fn max_period(self) -> u64 {
        self.max_period
    }

    pub const fn read_percent(self) -> u8 {
        self.read_percent
    }

    pub const fn data_limit(self) -> Option<u64> {
        self.data_limit
    }

    pub const fn elastic_requests(self) -> bool {
        self.elastic_requests
    }

    pub const fn rng_state(self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficStridedConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    start: Address,
    end: Address,
    offset: u64,
    block_size: AccessSize,
    superblock_size: u64,
    stride_size: u64,
    min_period: u64,
    max_period: u64,
    read_percent: u8,
    data_limit: Option<u64>,
    elastic_requests: bool,
    rng_state: u64,
}

impl TrafficStridedConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        start: Address,
        end: Address,
        offset: u64,
        block_size: AccessSize,
        superblock_size: u64,
        stride_size: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_strided_range(
            start,
            end,
            offset,
            block_size,
            line_layout,
            superblock_size,
            stride_size,
        )?;

        Ok(Self {
            agent,
            line_layout,
            start,
            end,
            offset,
            block_size,
            superblock_size,
            stride_size,
            min_period: DEFAULT_PERIOD,
            max_period: DEFAULT_PERIOD,
            read_percent: DEFAULT_READ_PERCENT,
            data_limit: None,
            elastic_requests: true,
            rng_state: DEFAULT_RNG_STATE,
        })
    }

    pub fn with_period(
        mut self,
        min_period: u64,
        max_period: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        if min_period > max_period {
            return Err(TrafficGeneratorError::InvertedPeriod {
                min_period,
                max_period,
            });
        }

        self.min_period = min_period;
        self.max_period = max_period;
        Ok(self)
    }

    pub fn with_read_percent(mut self, read_percent: u8) -> Result<Self, TrafficGeneratorError> {
        if read_percent > 100 {
            return Err(TrafficGeneratorError::InvalidReadPercent { read_percent });
        }

        self.read_percent = read_percent;
        Ok(self)
    }

    pub const fn with_data_limit(mut self, data_limit: u64) -> Result<Self, TrafficGeneratorError> {
        self.data_limit = if data_limit == 0 {
            None
        } else {
            Some(data_limit)
        };
        Ok(self)
    }

    pub const fn with_elastic_requests(mut self, elastic_requests: bool) -> Self {
        self.elastic_requests = elastic_requests;
        self
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

    pub const fn end(self) -> Address {
        self.end
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn block_size(self) -> AccessSize {
        self.block_size
    }

    pub const fn superblock_size(self) -> u64 {
        self.superblock_size
    }

    pub const fn stride_size(self) -> u64 {
        self.stride_size
    }

    pub const fn min_period(self) -> u64 {
        self.min_period
    }

    pub const fn max_period(self) -> u64 {
        self.max_period
    }

    pub const fn read_percent(self) -> u8 {
        self.read_percent
    }

    pub const fn data_limit(self) -> Option<u64> {
        self.data_limit
    }

    pub const fn elastic_requests(self) -> bool {
        self.elastic_requests
    }

    pub const fn rng_state(self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStridedSnapshot {
    config: TrafficStridedConfig,
    next_address: Address,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng_state: u64,
}

impl TrafficStridedSnapshot {
    pub fn new(
        config: TrafficStridedConfig,
        next_address: Address,
        next_sequence: u64,
        data_manipulated: u64,
        summary: TrafficGeneratorSummary,
        rng_state: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_strided_snapshot(config, next_address)?;

        Ok(Self {
            config,
            next_address,
            next_sequence,
            data_manipulated,
            summary,
            rng_state,
        })
    }

    pub const fn config(&self) -> TrafficStridedConfig {
        self.config
    }

    pub const fn next_address(&self) -> Address {
        self.next_address
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn data_manipulated(&self) -> u64 {
        self.data_manipulated
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficRandomSnapshot {
    config: TrafficRandomConfig,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng_state: u64,
}

impl TrafficRandomSnapshot {
    pub fn new(
        config: TrafficRandomConfig,
        next_sequence: u64,
        data_manipulated: u64,
        summary: TrafficGeneratorSummary,
        rng_state: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_random_range(
            config.start(),
            config.end(),
            config.block_size(),
            config.line_layout(),
        )?;

        Ok(Self {
            config,
            next_sequence,
            data_manipulated,
            summary,
            rng_state,
        })
    }

    pub const fn config(&self) -> TrafficRandomConfig {
        self.config
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn data_manipulated(&self) -> u64 {
        self.data_manipulated
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RandomTrafficGenerator {
    config: TrafficRandomConfig,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng: TrafficRng,
}

impl RandomTrafficGenerator {
    pub fn new(config: TrafficRandomConfig) -> Self {
        Self {
            config,
            next_sequence: 0,
            data_manipulated: 0,
            summary: TrafficGeneratorSummary::default(),
            rng: TrafficRng::new(config.rng_state()),
        }
    }

    pub fn restore(snapshot: TrafficRandomSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_random_range(
            snapshot.config().start(),
            snapshot.config().end(),
            snapshot.config().block_size(),
            snapshot.config().line_layout(),
        )?;

        Ok(Self {
            config: snapshot.config(),
            next_sequence: snapshot.next_sequence(),
            data_manipulated: snapshot.data_manipulated(),
            summary: snapshot.summary(),
            rng: TrafficRng::new(snapshot.rng_state()),
        })
    }

    pub fn enter(&mut self) {
        self.data_manipulated = 0;
    }

    pub fn next_request(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        if self.limit_reached() {
            return Ok(None);
        }

        let block_bytes = self.config.block_size().bytes();
        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let next_data_manipulated =
            checked_counter_add("data_manipulated", self.data_manipulated, block_bytes)?;
        let mut next_rng = self.rng.clone();
        let event_tick = Self::schedule_tick_with(self.config, &mut next_rng, tick, retry_delay)?;
        let kind = Self::next_kind_with(self.config, &mut next_rng);
        let address = Self::next_address_with(self.config, &mut next_rng);
        let request = self.build_request(sequence, kind, address)?;
        let mut next_summary = self.summary;
        next_summary.record(event_tick, kind, block_bytes)?;

        self.next_sequence = next_sequence;
        self.data_manipulated = next_data_manipulated;
        self.summary = next_summary;
        self.rng = next_rng;

        Ok(Some(TrafficRequestEvent::new(
            event_tick, sequence, kind, address, request,
        )))
    }

    pub fn schedule_tick(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        if self.limit_reached() {
            return Ok(u64::MAX);
        }

        Self::schedule_tick_with(self.config, &mut self.rng, tick, retry_delay)
    }

    fn schedule_tick_with(
        config: TrafficRandomConfig,
        rng: &mut TrafficRng,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        let mut wait = Self::next_wait_with(config, rng);
        if !config.elastic_requests() {
            wait = wait.saturating_sub(retry_delay);
        }

        tick.checked_add(wait)
            .ok_or(TrafficGeneratorError::TickOverflow { tick, delta: wait })
    }

    pub fn snapshot(&self) -> TrafficRandomSnapshot {
        TrafficRandomSnapshot::new(
            self.config,
            self.next_sequence,
            self.data_manipulated,
            self.summary,
            self.rng.state(),
        )
        .expect("live traffic generator state satisfies snapshot invariants")
    }

    pub const fn config(&self) -> TrafficRandomConfig {
        self.config
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn data_manipulated(&self) -> u64 {
        self.data_manipulated
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    fn build_request(
        &self,
        sequence: u64,
        kind: TrafficRequestKind,
        address: Address,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        let id = MemoryRequestId::new(self.config.agent(), sequence);
        let size = self.config.block_size();
        let layout = self.config.line_layout();

        match kind {
            TrafficRequestKind::Read => {
                MemoryRequest::read_shared(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Write => {
                let mask = ByteMask::full(size)?;
                let data_len = usize::try_from(mask.len())
                    .expect("byte mask length fits usize after construction");
                let data = vec![self.config.agent().get() as u8; data_len];
                MemoryRequest::write(id, address, size, data, mask, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance => {
                unreachable!("random traffic generator does not emit maintenance requests")
            }
        }
    }

    fn next_kind_with(config: TrafficRandomConfig, rng: &mut TrafficRng) -> TrafficRequestKind {
        match config.read_percent() {
            0 => TrafficRequestKind::Write,
            100 => TrafficRequestKind::Read,
            read_percent => {
                if rng.next_inclusive(0, 100) < u64::from(read_percent) {
                    TrafficRequestKind::Read
                } else {
                    TrafficRequestKind::Write
                }
            }
        }
    }

    fn next_wait_with(config: TrafficRandomConfig, rng: &mut TrafficRng) -> u64 {
        rng.next_inclusive(config.min_period(), config.max_period())
    }

    fn next_address_with(config: TrafficRandomConfig, rng: &mut TrafficRng) -> Address {
        let sampled = rng.next_inclusive(config.start().get(), config.end().get() - 1);
        Address::new(sampled - (sampled % config.block_size().bytes()))
    }

    fn limit_reached(&self) -> bool {
        self.config
            .data_limit()
            .is_some_and(|limit| self.data_manipulated >= limit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridedTrafficGenerator {
    config: TrafficStridedConfig,
    next_address: Address,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng: TrafficRng,
}

impl StridedTrafficGenerator {
    pub fn new(config: TrafficStridedConfig) -> Self {
        Self {
            config,
            next_address: strided_base_address(config)
                .expect("validated strided traffic config has an in-range base address"),
            next_sequence: 0,
            data_manipulated: 0,
            summary: TrafficGeneratorSummary::default(),
            rng: TrafficRng::new(config.rng_state()),
        }
    }

    pub fn restore(snapshot: TrafficStridedSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_strided_snapshot(snapshot.config(), snapshot.next_address())?;

        Ok(Self {
            config: snapshot.config(),
            next_address: snapshot.next_address(),
            next_sequence: snapshot.next_sequence(),
            data_manipulated: snapshot.data_manipulated(),
            summary: snapshot.summary(),
            rng: TrafficRng::new(snapshot.rng_state()),
        })
    }

    pub fn enter(&mut self) {
        self.next_address = strided_base_address(self.config)
            .expect("validated strided traffic config has an in-range base address");
        self.data_manipulated = 0;
    }

    pub fn next_request(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        if self.limit_reached() {
            return Ok(None);
        }

        let block_bytes = self.config.block_size().bytes();
        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let next_data_manipulated =
            checked_counter_add("data_manipulated", self.data_manipulated, block_bytes)?;
        let mut next_rng = self.rng.clone();
        let event_tick = Self::schedule_tick_with(self.config, &mut next_rng, tick, retry_delay)?;
        let kind = Self::next_kind_with(self.config, &mut next_rng);
        let address = self.next_address;
        let next_address = Self::advance_address_after(self.config, address)?;
        let request = self.build_request(sequence, kind, address)?;
        let mut next_summary = self.summary;
        next_summary.record(event_tick, kind, block_bytes)?;

        self.next_address = next_address;
        self.next_sequence = next_sequence;
        self.data_manipulated = next_data_manipulated;
        self.summary = next_summary;
        self.rng = next_rng;

        Ok(Some(TrafficRequestEvent::new(
            event_tick, sequence, kind, address, request,
        )))
    }

    pub fn schedule_tick(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        if self.limit_reached() {
            return Ok(u64::MAX);
        }

        Self::schedule_tick_with(self.config, &mut self.rng, tick, retry_delay)
    }

    fn schedule_tick_with(
        config: TrafficStridedConfig,
        rng: &mut TrafficRng,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        let mut wait = Self::next_wait_with(config, rng);
        if !config.elastic_requests() {
            wait = wait.saturating_sub(retry_delay);
        }

        tick.checked_add(wait)
            .ok_or(TrafficGeneratorError::TickOverflow { tick, delta: wait })
    }

    pub fn snapshot(&self) -> TrafficStridedSnapshot {
        TrafficStridedSnapshot::new(
            self.config,
            self.next_address,
            self.next_sequence,
            self.data_manipulated,
            self.summary,
            self.rng.state(),
        )
        .expect("live traffic generator state satisfies snapshot invariants")
    }

    pub const fn config(&self) -> TrafficStridedConfig {
        self.config
    }

    pub const fn next_address(&self) -> Address {
        self.next_address
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn data_manipulated(&self) -> u64 {
        self.data_manipulated
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    fn build_request(
        &self,
        sequence: u64,
        kind: TrafficRequestKind,
        address: Address,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        let id = MemoryRequestId::new(self.config.agent(), sequence);
        let size = self.config.block_size();
        let layout = self.config.line_layout();

        match kind {
            TrafficRequestKind::Read => {
                MemoryRequest::read_shared(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Write => {
                let mask = ByteMask::full(size)?;
                let data_len = usize::try_from(mask.len())
                    .expect("byte mask length fits usize after construction");
                let data = vec![self.config.agent().get() as u8; data_len];
                MemoryRequest::write(id, address, size, data, mask, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance => {
                unreachable!("strided traffic generator does not emit maintenance requests")
            }
        }
    }

    fn next_kind_with(config: TrafficStridedConfig, rng: &mut TrafficRng) -> TrafficRequestKind {
        match config.read_percent() {
            0 => TrafficRequestKind::Write,
            100 => TrafficRequestKind::Read,
            read_percent => {
                if rng.next_inclusive(0, 100) < u64::from(read_percent) {
                    TrafficRequestKind::Read
                } else {
                    TrafficRequestKind::Write
                }
            }
        }
    }

    fn next_wait_with(config: TrafficStridedConfig, rng: &mut TrafficRng) -> u64 {
        rng.next_inclusive(config.min_period(), config.max_period())
    }

    fn advance_address_after(
        config: TrafficStridedConfig,
        current: Address,
    ) -> Result<Address, TrafficGeneratorError> {
        let base = strided_base_address(config)?;
        let block_bytes = config.block_size().bytes();
        let mut next = checked_address_add("strided_cursor", current.get(), block_bytes)?;

        if (next - base.get()).is_multiple_of(config.superblock_size()) {
            next = checked_address_add(
                "strided_cursor",
                next,
                config.stride_size() - config.superblock_size(),
            )?;
        }

        if next >= config.end().get() {
            Ok(base)
        } else {
            Ok(Address::new(next))
        }
    }

    fn limit_reached(&self) -> bool {
        self.config
            .data_limit()
            .is_some_and(|limit| self.data_manipulated >= limit)
    }
}

fn checked_address_add(
    label: &'static str,
    value: u64,
    increment: u64,
) -> Result<u64, TrafficGeneratorError> {
    value
        .checked_add(increment)
        .ok_or(TrafficGeneratorError::AddressOverflow {
            label,
            value,
            increment,
        })
}

fn validate_range(
    start: Address,
    end: Address,
    block_size: AccessSize,
    line_layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if start.get() >= end.get() {
        return Err(TrafficGeneratorError::EmptyAddressRange { start, end });
    }

    let range_size = end.get() - start.get();
    if block_size.bytes() > range_size {
        return Err(TrafficGeneratorError::BlockSizeExceedsRange {
            block_size: block_size.bytes(),
            range_size,
        });
    }

    if block_size.bytes() > line_layout.bytes() {
        return Err(TrafficGeneratorError::BlockSizeExceedsCacheLine {
            block_size: block_size.bytes(),
            cache_line_size: line_layout.bytes(),
        });
    }

    if !range_size.is_multiple_of(block_size.bytes()) {
        return Err(TrafficGeneratorError::BlockSizeDoesNotDivideRange {
            block_size: block_size.bytes(),
            range_size,
        });
    }

    Ok(())
}

fn validate_random_range(
    start: Address,
    end: Address,
    block_size: AccessSize,
    line_layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if start.get() >= end.get() {
        return Err(TrafficGeneratorError::EmptyAddressRange { start, end });
    }

    if block_size.bytes() > line_layout.bytes() {
        return Err(TrafficGeneratorError::BlockSizeExceedsCacheLine {
            block_size: block_size.bytes(),
            cache_line_size: line_layout.bytes(),
        });
    }

    Ok(())
}

fn validate_strided_range(
    start: Address,
    end: Address,
    offset: u64,
    block_size: AccessSize,
    line_layout: CacheLineLayout,
    superblock_size: u64,
    stride_size: u64,
) -> Result<(), TrafficGeneratorError> {
    validate_random_range(start, end, block_size, line_layout)?;

    if superblock_size == 0 {
        return Err(TrafficGeneratorError::ZeroSuperblockSize);
    }

    if stride_size == 0 {
        return Err(TrafficGeneratorError::ZeroStrideSize);
    }

    let block_bytes = block_size.bytes();
    if !superblock_size.is_multiple_of(block_bytes) {
        return Err(
            TrafficGeneratorError::SuperblockSizeNotMultipleOfBlockSize {
                superblock_size,
                block_size: block_bytes,
            },
        );
    }

    if !offset.is_multiple_of(superblock_size) {
        return Err(TrafficGeneratorError::OffsetNotMultipleOfSuperblock {
            offset,
            superblock_size,
        });
    }

    if !stride_size.is_multiple_of(superblock_size) {
        return Err(TrafficGeneratorError::StrideSizeNotMultipleOfSuperblock {
            stride_size,
            superblock_size,
        });
    }

    let first = checked_address_add("strided_start", start.get(), offset)?;
    if first < start.get() || first >= end.get() {
        return Err(TrafficGeneratorError::StridedOffsetOutsideRange {
            next_address: Address::new(first),
            start,
            end,
        });
    }

    Ok(())
}

fn strided_base_address(config: TrafficStridedConfig) -> Result<Address, TrafficGeneratorError> {
    checked_address_add("strided_start", config.start().get(), config.offset()).map(Address::new)
}

fn validate_strided_snapshot(
    config: TrafficStridedConfig,
    next_address: Address,
) -> Result<(), TrafficGeneratorError> {
    validate_strided_range(
        config.start(),
        config.end(),
        config.offset(),
        config.block_size(),
        config.line_layout(),
        config.superblock_size(),
        config.stride_size(),
    )?;

    let base = strided_base_address(config)?;
    if next_address.get() < base.get() || config.end().get() <= next_address.get() {
        return Err(TrafficGeneratorError::SnapshotCursorOutsideRange {
            next_address,
            start: base,
            end: config.end(),
        });
    }

    let cursor_offset = next_address.get() - base.get();
    let stride_offset = cursor_offset % config.stride_size();
    if stride_offset >= config.superblock_size()
        || !stride_offset.is_multiple_of(config.block_size().bytes())
    {
        return Err(TrafficGeneratorError::SnapshotCursorOutsideBlockGrid {
            next_address,
            start: base,
            block_size: config.block_size().bytes(),
        });
    }

    Ok(())
}

fn validate_snapshot(
    config: TrafficLinearConfig,
    next_address: Address,
    _data_manipulated: u64,
) -> Result<(), TrafficGeneratorError> {
    validate_range(
        config.start(),
        config.end(),
        config.block_size(),
        config.line_layout(),
    )?;
    if next_address.get() < config.start().get() || config.end().get() <= next_address.get() {
        return Err(TrafficGeneratorError::SnapshotCursorOutsideRange {
            next_address,
            start: config.start(),
            end: config.end(),
        });
    }
    let cursor_offset = next_address.get() - config.start().get();
    if !cursor_offset.is_multiple_of(config.block_size().bytes()) {
        return Err(TrafficGeneratorError::SnapshotCursorOutsideBlockGrid {
            next_address,
            start: config.start(),
            block_size: config.block_size().bytes(),
        });
    }
    Ok(())
}
