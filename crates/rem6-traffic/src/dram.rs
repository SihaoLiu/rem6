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
pub enum TrafficDramMode {
    Dram,
    DramRotate,
    Nvm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficDramAddressMapping {
    RoRaBaChCo,
    RoRaBaCoCh,
    RoCoRaBaCh,
}

impl TrafficDramAddressMapping {
    pub const fn from_gem5_code(code: u32) -> Result<Self, TrafficGeneratorError> {
        match code {
            0 => Ok(Self::RoRaBaChCo),
            1 => Ok(Self::RoRaBaCoCh),
            2 => Ok(Self::RoCoRaBaCh),
            mapping => Err(TrafficGeneratorError::TrafficDramUnsupportedAddressMapping { mapping }),
        }
    }

    const fn uses_contiguous_columns(self) -> bool {
        matches!(self, Self::RoRaBaChCo | Self::RoRaBaCoCh)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficDramConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    mode: TrafficDramMode,
    start: Address,
    end: Address,
    block_size: AccessSize,
    page_or_buffer_size: u64,
    banks: u32,
    banks_util: u32,
    address_mapping: TrafficDramAddressMapping,
    ranks: u32,
    num_seq_packets: u32,
    min_period: u64,
    max_period: u64,
    read_percent: u8,
    data_limit: Option<u64>,
    elastic_requests: bool,
    rng_state: u64,
}

impl TrafficDramConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        mode: TrafficDramMode,
        start: Address,
        end: Address,
        block_size: AccessSize,
        page_or_buffer_size: u64,
        banks: u32,
        banks_util: u32,
        address_mapping: TrafficDramAddressMapping,
        ranks: u32,
        num_seq_packets: u32,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_dram_geometry(
            start,
            end,
            block_size,
            line_layout,
            page_or_buffer_size,
            banks,
            banks_util,
            ranks,
            num_seq_packets,
        )?;
        validate_dram_rotation_cycle(mode, DEFAULT_READ_PERCENT, banks_util, ranks)?;

        Ok(Self {
            agent,
            line_layout,
            mode,
            start,
            end,
            block_size,
            page_or_buffer_size,
            banks,
            banks_util,
            address_mapping,
            ranks,
            num_seq_packets,
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

        if self.mode == TrafficDramMode::DramRotate && !matches!(read_percent, 0 | 50 | 100) {
            return Err(
                TrafficGeneratorError::TrafficDramRotateUnsupportedReadPercent { read_percent },
            );
        }

        validate_dram_rotation_cycle(self.mode, read_percent, self.banks_util, self.ranks)?;
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

    pub const fn mode(self) -> TrafficDramMode {
        self.mode
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

    pub const fn page_or_buffer_size(self) -> u64 {
        self.page_or_buffer_size
    }

    pub const fn banks(self) -> u32 {
        self.banks
    }

    pub const fn banks_util(self) -> u32 {
        self.banks_util
    }

    pub const fn address_mapping(self) -> TrafficDramAddressMapping {
        self.address_mapping
    }

    pub const fn ranks(self) -> u32 {
        self.ranks
    }

    pub const fn num_seq_packets(self) -> u32 {
        self.num_seq_packets
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

    fn max_seq_count_per_rank(self) -> u32 {
        u32::try_from(max_seq_count_per_rank(
            self.mode,
            self.read_percent,
            self.banks_util,
        ))
        .expect("DRAM rotate cycle was validated when config was built")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficDramSnapshot {
    config: TrafficDramConfig,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng_state: u64,
    series_remaining: u32,
    current_address: Address,
    current_kind: TrafficRequestKind,
    rotate_sequence_count: u32,
}

impl TrafficDramSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: TrafficDramConfig,
        next_sequence: u64,
        data_manipulated: u64,
        summary: TrafficGeneratorSummary,
        rng_state: u64,
        series_remaining: u32,
        current_address: Address,
        current_kind: TrafficRequestKind,
        rotate_sequence_count: u32,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_dram_snapshot(config, series_remaining, rotate_sequence_count)?;

        Ok(Self {
            config,
            next_sequence,
            data_manipulated,
            summary,
            rng_state,
            series_remaining,
            current_address,
            current_kind,
            rotate_sequence_count,
        })
    }

    pub const fn config(&self) -> TrafficDramConfig {
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

    pub const fn series_remaining(&self) -> u32 {
        self.series_remaining
    }

    pub const fn current_address(&self) -> Address {
        self.current_address
    }

    pub const fn current_kind(&self) -> TrafficRequestKind {
        self.current_kind
    }

    pub const fn rotate_sequence_count(&self) -> u32 {
        self.rotate_sequence_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramTrafficGenerator {
    config: TrafficDramConfig,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng: TrafficRng,
    series_remaining: u32,
    current_address: Address,
    current_kind: TrafficRequestKind,
    rotate_sequence_count: u32,
}

impl DramTrafficGenerator {
    pub fn new(config: TrafficDramConfig) -> Self {
        Self {
            config,
            next_sequence: 0,
            data_manipulated: 0,
            summary: TrafficGeneratorSummary::default(),
            rng: TrafficRng::new(config.rng_state()),
            series_remaining: 0,
            current_address: config.start(),
            current_kind: TrafficRequestKind::Read,
            rotate_sequence_count: 0,
        }
    }

    pub fn restore(snapshot: TrafficDramSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_dram_snapshot(
            snapshot.config(),
            snapshot.series_remaining(),
            snapshot.rotate_sequence_count(),
        )?;

        Ok(Self {
            config: snapshot.config(),
            next_sequence: snapshot.next_sequence(),
            data_manipulated: snapshot.data_manipulated(),
            summary: snapshot.summary(),
            rng: TrafficRng::new(snapshot.rng_state()),
            series_remaining: snapshot.series_remaining(),
            current_address: snapshot.current_address(),
            current_kind: snapshot.current_kind(),
            rotate_sequence_count: snapshot.rotate_sequence_count(),
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
        let mut next_state = self.clone_for_next_series(next_rng);
        if next_state.series_remaining == 0 {
            next_state.start_series()?;
        } else {
            next_state.advance_column()?;
        }
        let address = next_state.current_address;
        let kind = next_state.current_kind;
        let request = self.build_request(sequence, kind, address)?;
        let mut next_summary = self.summary;
        next_summary.record(event_tick, kind, block_bytes)?;

        next_state.series_remaining -= 1;
        self.next_sequence = next_sequence;
        self.data_manipulated = next_data_manipulated;
        self.summary = next_summary;
        self.rng = next_state.rng;
        self.series_remaining = next_state.series_remaining;
        self.current_address = next_state.current_address;
        self.current_kind = next_state.current_kind;
        self.rotate_sequence_count = next_state.rotate_sequence_count;

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
        config: TrafficDramConfig,
        rng: &mut TrafficRng,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        let mut wait = rng.next_inclusive(config.min_period(), config.max_period());
        if !config.elastic_requests() {
            wait = wait.saturating_sub(retry_delay);
        }

        tick.checked_add(wait)
            .ok_or(TrafficGeneratorError::TickOverflow { tick, delta: wait })
    }

    pub fn snapshot(&self) -> TrafficDramSnapshot {
        TrafficDramSnapshot::new(
            self.config,
            self.next_sequence,
            self.data_manipulated,
            self.summary,
            self.rng.state(),
            self.series_remaining,
            self.current_address,
            self.current_kind,
            self.rotate_sequence_count,
        )
        .expect("live traffic generator state satisfies snapshot invariants")
    }

    pub const fn config(&self) -> TrafficDramConfig {
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

    fn clone_for_next_series(&self, rng: TrafficRng) -> Self {
        Self {
            config: self.config,
            next_sequence: self.next_sequence,
            data_manipulated: self.data_manipulated,
            summary: self.summary,
            rng,
            series_remaining: self.series_remaining,
            current_address: self.current_address,
            current_kind: self.current_kind,
            rotate_sequence_count: self.rotate_sequence_count,
        }
    }

    fn start_series(&mut self) -> Result<(), TrafficGeneratorError> {
        self.series_remaining = self.config.num_seq_packets();
        let (bank, rank) = match self.config.mode() {
            TrafficDramMode::Dram | TrafficDramMode::Nvm => {
                self.current_kind = self.next_random_kind();
                let bank = self
                    .rng
                    .next_inclusive(0, u64::from(self.config.banks_util() - 1))
                    as u32;
                let rank = self
                    .rng
                    .next_inclusive(0, u64::from(self.config.ranks() - 1))
                    as u32;
                (bank, rank)
            }
            TrafficDramMode::DramRotate => self.next_rotating_series(),
        };

        self.current_address = self.start_address_for(bank, rank)?;
        Ok(())
    }

    fn next_random_kind(&mut self) -> TrafficRequestKind {
        match self.config.read_percent() {
            0 => TrafficRequestKind::Write,
            100 => TrafficRequestKind::Read,
            read_percent => {
                if self.rng.next_inclusive(0, 100) < u64::from(read_percent) {
                    TrafficRequestKind::Read
                } else {
                    TrafficRequestKind::Write
                }
            }
        }
    }

    fn next_rotating_series(&mut self) -> (u32, u32) {
        let rotates_kind = self
            .rotate_sequence_count
            .is_multiple_of(self.config.banks_util());
        match self.config.read_percent() {
            0 => self.current_kind = TrafficRequestKind::Write,
            100 => self.current_kind = TrafficRequestKind::Read,
            50 if rotates_kind => {
                self.current_kind = match self.current_kind {
                    TrafficRequestKind::Read => TrafficRequestKind::Write,
                    TrafficRequestKind::Write => TrafficRequestKind::Read,
                };
            }
            _ => {}
        }

        let max_seq_count_per_rank = self.config.max_seq_count_per_rank();
        let bank = self.rotate_sequence_count % self.config.banks_util();
        let rank = (self.rotate_sequence_count / max_seq_count_per_rank) % self.config.ranks();
        self.rotate_sequence_count =
            (self.rotate_sequence_count + 1) % (self.config.ranks() * max_seq_count_per_rank);
        (bank, rank)
    }

    fn start_address_for(
        &mut self,
        bank: u32,
        rank: u32,
    ) -> Result<Address, TrafficGeneratorError> {
        let block_bytes = self.config.block_size().bytes();
        let mut address = self
            .rng
            .next_inclusive(self.config.start().get(), self.config.end().get() - 1);
        address -= address % block_bytes;

        let columns = self.columns_per_page_or_buffer();
        let col = self
            .rng
            .next_inclusive(0, columns - u64::from(self.config.num_seq_packets()));
        Ok(Address::new(
            self.insert_bank_rank_col(address, bank, rank, col),
        ))
    }

    fn advance_column(&mut self) -> Result<(), TrafficGeneratorError> {
        let block_bytes = self.config.block_size().bytes();
        if self.config.address_mapping().uses_contiguous_columns() {
            self.current_address = Address::new(checked_address_add(
                "dram_column",
                self.current_address.get(),
                block_bytes,
            )?);
            return Ok(());
        }

        let columns = self.columns_per_page_or_buffer();
        let col = ((self.current_address.get()
            / block_bytes
            / u64::from(self.config.banks())
            / u64::from(self.config.ranks()))
            % columns)
            + 1;
        self.current_address = Address::new(self.replace_col(self.current_address.get(), col));
        Ok(())
    }

    fn insert_bank_rank_col(&self, address: u64, bank: u32, rank: u32, col: u64) -> u64 {
        let block_bits = floor_log2(self.config.block_size().bytes());
        let page_bits = floor_log2(self.columns_per_page_or_buffer());
        let bank_bits = floor_log2(u64::from(self.config.banks()));
        let rank_bits = floor_log2(u64::from(self.config.ranks()));

        match self.config.address_mapping() {
            TrafficDramAddressMapping::RoRaBaChCo | TrafficDramAddressMapping::RoRaBaCoCh => {
                let with_bank =
                    replace_bits(address, block_bits + page_bits, bank_bits, bank.into());
                let with_col = replace_bits(with_bank, block_bits, page_bits, col);
                replace_bits(
                    with_col,
                    block_bits + page_bits + bank_bits,
                    rank_bits,
                    rank.into(),
                )
            }
            TrafficDramAddressMapping::RoCoRaBaCh => {
                let with_bank = replace_bits(address, block_bits, bank_bits, bank.into());
                let with_col = replace_bits(
                    with_bank,
                    block_bits + bank_bits + rank_bits,
                    page_bits,
                    col,
                );
                replace_bits(with_col, block_bits + bank_bits, rank_bits, rank.into())
            }
        }
    }

    fn replace_col(&self, address: u64, col: u64) -> u64 {
        let block_bits = floor_log2(self.config.block_size().bytes());
        let page_bits = floor_log2(self.columns_per_page_or_buffer());
        let bank_bits = floor_log2(u64::from(self.config.banks()));
        let rank_bits = floor_log2(u64::from(self.config.ranks()));
        replace_bits(address, block_bits + bank_bits + rank_bits, page_bits, col)
    }

    fn columns_per_page_or_buffer(&self) -> u64 {
        self.config.page_or_buffer_size() / self.config.block_size().bytes()
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
        }
    }

    fn limit_reached(&self) -> bool {
        self.config
            .data_limit()
            .is_some_and(|limit| self.data_manipulated >= limit)
    }
}

fn validate_dram_config(config: TrafficDramConfig) -> Result<(), TrafficGeneratorError> {
    validate_dram_geometry(
        config.start(),
        config.end(),
        config.block_size(),
        config.line_layout(),
        config.page_or_buffer_size(),
        config.banks(),
        config.banks_util(),
        config.ranks(),
        config.num_seq_packets(),
    )?;
    validate_dram_rotation_cycle(
        config.mode(),
        config.read_percent(),
        config.banks_util(),
        config.ranks(),
    )
}

fn validate_dram_snapshot(
    config: TrafficDramConfig,
    series_remaining: u32,
    rotate_sequence_count: u32,
) -> Result<(), TrafficGeneratorError> {
    validate_dram_config(config)?;

    if series_remaining > config.num_seq_packets() {
        return Err(
            TrafficGeneratorError::TrafficDramSnapshotSeriesOutsideRange {
                series_remaining,
                num_seq_packets: config.num_seq_packets(),
            },
        );
    }

    if let Some(cycle_size) = dram_rotation_cycle_size(
        config.mode(),
        config.read_percent(),
        config.banks_util(),
        config.ranks(),
    )? {
        if u64::from(rotate_sequence_count) >= cycle_size {
            return Err(
                TrafficGeneratorError::TrafficDramSnapshotRotateSequenceOutsideCycle {
                    rotate_sequence_count,
                    cycle_size,
                },
            );
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_dram_geometry(
    start: Address,
    end: Address,
    block_size: AccessSize,
    line_layout: CacheLineLayout,
    page_or_buffer_size: u64,
    banks: u32,
    banks_util: u32,
    ranks: u32,
    num_seq_packets: u32,
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

    if page_or_buffer_size < block_size.bytes() {
        return Err(TrafficGeneratorError::TrafficDramGeometryTooSmall {
            field: "page_or_buffer_size",
            value: page_or_buffer_size,
            minimum: block_size.bytes(),
        });
    }

    if banks == 0 {
        return Err(TrafficGeneratorError::TrafficDramGeometryTooSmall {
            field: "banks",
            value: 0,
            minimum: 1,
        });
    }

    if banks_util == 0 {
        return Err(TrafficGeneratorError::TrafficDramGeometryTooSmall {
            field: "banks_util",
            value: 0,
            minimum: 1,
        });
    }

    if ranks == 0 {
        return Err(TrafficGeneratorError::TrafficDramGeometryTooSmall {
            field: "ranks",
            value: 0,
            minimum: 1,
        });
    }

    if num_seq_packets == 0 {
        return Err(TrafficGeneratorError::TrafficDramGeometryTooSmall {
            field: "num_seq_packets",
            value: 0,
            minimum: 1,
        });
    }

    if banks_util > banks {
        return Err(
            TrafficGeneratorError::TrafficDramBanksUtilExceedsAvailable { banks_util, banks },
        );
    }

    if !page_or_buffer_size.is_multiple_of(block_size.bytes()) {
        return Err(TrafficGeneratorError::TrafficDramGeometryNotMultiple {
            field: "page_or_buffer_size",
            value: page_or_buffer_size,
            factor: block_size.bytes(),
        });
    }

    for (field, value) in [
        ("block_size", block_size.bytes()),
        ("page_or_buffer_size", page_or_buffer_size),
        ("banks", u64::from(banks)),
        ("ranks", u64::from(ranks)),
    ] {
        if !value.is_power_of_two() {
            return Err(TrafficGeneratorError::TrafficDramGeometryNotPowerOfTwo { field, value });
        }
    }

    let columns = page_or_buffer_size / block_size.bytes();
    if columns < u64::from(num_seq_packets) {
        return Err(TrafficGeneratorError::TrafficDramSeriesExceedsPage {
            num_seq_packets,
            columns_per_page_or_buffer: columns,
        });
    }

    validate_dram_address_bits(block_size.bytes(), columns, banks, ranks)?;

    Ok(())
}

fn validate_dram_address_bits(
    block_bytes: u64,
    columns_per_page_or_buffer: u64,
    banks: u32,
    ranks: u32,
) -> Result<(), TrafficGeneratorError> {
    let block_bits = floor_log2(block_bytes);
    let page_bits = floor_log2(columns_per_page_or_buffer);
    let bank_bits = floor_log2(u64::from(banks));
    let rank_bits = floor_log2(u64::from(ranks));
    let total_bits = block_bits + page_bits + bank_bits + rank_bits;
    if total_bits > u64::BITS {
        return Err(TrafficGeneratorError::TrafficDramAddressBitWidthTooLarge {
            block_bits,
            page_bits,
            bank_bits,
            rank_bits,
        });
    }

    Ok(())
}

fn validate_dram_rotation_cycle(
    mode: TrafficDramMode,
    read_percent: u8,
    banks_util: u32,
    ranks: u32,
) -> Result<(), TrafficGeneratorError> {
    if mode != TrafficDramMode::DramRotate {
        return Ok(());
    }

    let max_seq_count_per_rank = max_seq_count_per_rank(mode, read_percent, banks_util);
    let cycle_size = dram_rotation_cycle_size(mode, read_percent, banks_util, ranks)?
        .expect("DRAM rotate cycle size exists for DRAM_ROTATE mode");
    if cycle_size > u64::from(u32::MAX) {
        return Err(TrafficGeneratorError::TrafficDramRotationCycleTooLarge {
            ranks,
            max_seq_count_per_rank,
        });
    }

    Ok(())
}

fn dram_rotation_cycle_size(
    mode: TrafficDramMode,
    read_percent: u8,
    banks_util: u32,
    ranks: u32,
) -> Result<Option<u64>, TrafficGeneratorError> {
    if mode != TrafficDramMode::DramRotate {
        return Ok(None);
    }

    let max_seq_count_per_rank = max_seq_count_per_rank(mode, read_percent, banks_util);
    let cycle_size = max_seq_count_per_rank.checked_mul(u64::from(ranks)).ok_or(
        TrafficGeneratorError::TrafficDramRotationCycleTooLarge {
            ranks,
            max_seq_count_per_rank,
        },
    )?;
    Ok(Some(cycle_size))
}

fn max_seq_count_per_rank(mode: TrafficDramMode, read_percent: u8, banks_util: u32) -> u64 {
    if mode == TrafficDramMode::DramRotate && read_percent == 50 {
        u64::from(banks_util) * 2
    } else {
        u64::from(banks_util)
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

fn floor_log2(value: u64) -> u32 {
    u64::BITS - 1 - value.leading_zeros()
}

fn replace_bits(value: u64, low_bit: u32, width: u32, replacement: u64) -> u64 {
    if width == 0 {
        return value;
    }

    debug_assert!(width < u64::BITS);
    debug_assert!(low_bit < u64::BITS);
    debug_assert!(low_bit + width <= u64::BITS);
    let mask = ((1_u64 << width) - 1) << low_bit;
    (value & !mask) | ((replacement << low_bit) & mask)
}
