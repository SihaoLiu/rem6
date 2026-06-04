use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};

use crate::{
    common::{
        checked_counter_add, TrafficGeneratorSummary, TrafficRequestEvent, TrafficRequestKind,
        TrafficRng,
    },
    TrafficDramAddressMapping, TrafficGeneratorError,
};

const DEFAULT_PERIOD: u64 = 1;
const DEFAULT_READ_PERCENT: u8 = 100;
const DEFAULT_NVM_PERCENT: u8 = 0;
const DEFAULT_RNG_STATE: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficHybridSide {
    Dram,
    Nvm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficHybridSideConfig {
    start: Address,
    end: Address,
    block_size: AccessSize,
    page_or_buffer_size: u64,
    banks: u32,
    banks_util: u32,
    ranks: u32,
    num_seq_packets: u32,
}

impl TrafficHybridSideConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start: Address,
        end: Address,
        block_size: AccessSize,
        page_or_buffer_size: u64,
        banks: u32,
        banks_util: u32,
        ranks: u32,
        num_seq_packets: u32,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_side_geometry(
            start,
            end,
            block_size,
            page_or_buffer_size,
            banks,
            banks_util,
            ranks,
            num_seq_packets,
        )?;

        Ok(Self {
            start,
            end,
            block_size,
            page_or_buffer_size,
            banks,
            banks_util,
            ranks,
            num_seq_packets,
        })
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

    pub const fn ranks(self) -> u32 {
        self.ranks
    }

    pub const fn num_seq_packets(self) -> u32 {
        self.num_seq_packets
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficHybridConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    dram: TrafficHybridSideConfig,
    nvm: TrafficHybridSideConfig,
    address_mapping: TrafficDramAddressMapping,
    min_period: u64,
    max_period: u64,
    read_percent: u8,
    nvm_percent: u8,
    data_limit: Option<u64>,
    elastic_requests: bool,
    rng_state: u64,
}

impl TrafficHybridConfig {
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        dram: TrafficHybridSideConfig,
        nvm: TrafficHybridSideConfig,
        address_mapping: TrafficDramAddressMapping,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_side_cache_line(dram, line_layout)?;
        validate_side_cache_line(nvm, line_layout)?;

        Ok(Self {
            agent,
            line_layout,
            dram,
            nvm,
            address_mapping,
            min_period: DEFAULT_PERIOD,
            max_period: DEFAULT_PERIOD,
            read_percent: DEFAULT_READ_PERCENT,
            nvm_percent: DEFAULT_NVM_PERCENT,
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

    pub fn with_nvm_percent(mut self, nvm_percent: u8) -> Result<Self, TrafficGeneratorError> {
        if nvm_percent > 100 {
            return Err(TrafficGeneratorError::TrafficHybridInvalidNvmPercent { nvm_percent });
        }

        self.nvm_percent = nvm_percent;
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

    pub const fn dram(self) -> TrafficHybridSideConfig {
        self.dram
    }

    pub const fn nvm(self) -> TrafficHybridSideConfig {
        self.nvm
    }

    pub const fn side(self, side: TrafficHybridSide) -> TrafficHybridSideConfig {
        match side {
            TrafficHybridSide::Dram => self.dram,
            TrafficHybridSide::Nvm => self.nvm,
        }
    }

    pub const fn address_mapping(self) -> TrafficDramAddressMapping {
        self.address_mapping
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

    pub const fn nvm_percent(self) -> u8 {
        self.nvm_percent
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
pub struct TrafficHybridSnapshot {
    config: TrafficHybridConfig,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng_state: u64,
    series_remaining: u32,
    current_address: Address,
    current_kind: TrafficRequestKind,
    current_side: TrafficHybridSide,
}

impl TrafficHybridSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: TrafficHybridConfig,
        next_sequence: u64,
        data_manipulated: u64,
        summary: TrafficGeneratorSummary,
        rng_state: u64,
        series_remaining: u32,
        current_address: Address,
        current_kind: TrafficRequestKind,
        current_side: TrafficHybridSide,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_hybrid_snapshot(config, series_remaining, current_side)?;

        Ok(Self {
            config,
            next_sequence,
            data_manipulated,
            summary,
            rng_state,
            series_remaining,
            current_address,
            current_kind,
            current_side,
        })
    }

    pub const fn config(&self) -> TrafficHybridConfig {
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

    pub const fn current_side(&self) -> TrafficHybridSide {
        self.current_side
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HybridTrafficGenerator {
    config: TrafficHybridConfig,
    next_sequence: u64,
    data_manipulated: u64,
    summary: TrafficGeneratorSummary,
    rng: TrafficRng,
    series_remaining: u32,
    current_address: Address,
    current_kind: TrafficRequestKind,
    current_side: TrafficHybridSide,
}

impl HybridTrafficGenerator {
    pub fn new(config: TrafficHybridConfig) -> Self {
        Self {
            config,
            next_sequence: 0,
            data_manipulated: 0,
            summary: TrafficGeneratorSummary::default(),
            rng: TrafficRng::new(config.rng_state()),
            series_remaining: 0,
            current_address: config.dram().start(),
            current_kind: TrafficRequestKind::Read,
            current_side: TrafficHybridSide::Dram,
        }
    }

    pub fn restore(snapshot: TrafficHybridSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_hybrid_snapshot(
            snapshot.config(),
            snapshot.series_remaining(),
            snapshot.current_side(),
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
            current_side: snapshot.current_side(),
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

        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let mut next_rng = self.rng.clone();
        let event_tick = Self::schedule_tick_with(self.config, &mut next_rng, tick, retry_delay)?;
        let mut next_state = self.clone_for_next_series(next_rng);
        if next_state.series_remaining == 0 {
            next_state.start_series()?;
        } else {
            next_state.advance_column()?;
        }

        let side_config = next_state.config.side(next_state.current_side);
        let block_bytes = side_config.block_size().bytes();
        let next_data_manipulated =
            checked_counter_add("data_manipulated", self.data_manipulated, block_bytes)?;
        let address = next_state.current_address;
        let kind = next_state.current_kind;
        let request = self.build_request(sequence, kind, address, side_config.block_size())?;
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
        self.current_side = next_state.current_side;

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
        config: TrafficHybridConfig,
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

    pub fn snapshot(&self) -> TrafficHybridSnapshot {
        TrafficHybridSnapshot::new(
            self.config,
            self.next_sequence,
            self.data_manipulated,
            self.summary,
            self.rng.state(),
            self.series_remaining,
            self.current_address,
            self.current_kind,
            self.current_side,
        )
        .expect("live traffic generator state satisfies snapshot invariants")
    }

    pub const fn config(&self) -> TrafficHybridConfig {
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
            current_side: self.current_side,
        }
    }

    fn start_series(&mut self) -> Result<(), TrafficGeneratorError> {
        self.current_side = self.next_side();
        self.current_kind = self.next_random_kind();
        let side_config = self.config.side(self.current_side);
        self.series_remaining = side_config.num_seq_packets();

        let bank = self
            .rng
            .next_inclusive(0, u64::from(side_config.banks_util() - 1)) as u32;
        let rank = self
            .rng
            .next_inclusive(0, u64::from(side_config.ranks() - 1)) as u32;
        self.current_address = self.start_address_for(side_config, bank, rank)?;
        Ok(())
    }

    fn next_side(&mut self) -> TrafficHybridSide {
        match self.config.nvm_percent() {
            0 => TrafficHybridSide::Dram,
            100 => TrafficHybridSide::Nvm,
            nvm_percent => {
                if self.rng.next_inclusive(0, 100) < u64::from(nvm_percent) {
                    TrafficHybridSide::Nvm
                } else {
                    TrafficHybridSide::Dram
                }
            }
        }
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

    fn start_address_for(
        &mut self,
        side_config: TrafficHybridSideConfig,
        bank: u32,
        rank: u32,
    ) -> Result<Address, TrafficGeneratorError> {
        let block_bytes = side_config.block_size().bytes();
        let mut address = self
            .rng
            .next_inclusive(side_config.start().get(), side_config.end().get() - 1);
        address -= address % block_bytes;

        let columns = columns_per_page_or_buffer(side_config);
        let col = self
            .rng
            .next_inclusive(0, columns - u64::from(side_config.num_seq_packets()));
        Ok(Address::new(insert_bank_rank_col(
            self.config.address_mapping(),
            side_config,
            address,
            bank,
            rank,
            col,
        )))
    }

    fn advance_column(&mut self) -> Result<(), TrafficGeneratorError> {
        let side_config = self.config.side(self.current_side);
        let block_bytes = side_config.block_size().bytes();
        if uses_contiguous_columns(self.config.address_mapping()) {
            self.current_address = Address::new(checked_address_add(
                "hybrid_column",
                self.current_address.get(),
                block_bytes,
            )?);
            return Ok(());
        }

        let columns = columns_per_page_or_buffer(side_config);
        let col = ((self.current_address.get()
            / block_bytes
            / u64::from(side_config.banks())
            / u64::from(side_config.ranks()))
            % columns)
            + 1;
        self.current_address =
            Address::new(replace_col(side_config, self.current_address.get(), col));
        Ok(())
    }

    fn build_request(
        &self,
        sequence: u64,
        kind: TrafficRequestKind,
        address: Address,
        size: AccessSize,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        let id = MemoryRequestId::new(self.config.agent(), sequence);
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
                unreachable!("hybrid traffic generator does not emit maintenance requests")
            }
        }
    }

    fn limit_reached(&self) -> bool {
        self.config
            .data_limit()
            .is_some_and(|limit| self.data_manipulated >= limit)
    }
}

fn validate_hybrid_config(config: TrafficHybridConfig) -> Result<(), TrafficGeneratorError> {
    validate_side_geometry(
        config.dram().start(),
        config.dram().end(),
        config.dram().block_size(),
        config.dram().page_or_buffer_size(),
        config.dram().banks(),
        config.dram().banks_util(),
        config.dram().ranks(),
        config.dram().num_seq_packets(),
    )?;
    validate_side_geometry(
        config.nvm().start(),
        config.nvm().end(),
        config.nvm().block_size(),
        config.nvm().page_or_buffer_size(),
        config.nvm().banks(),
        config.nvm().banks_util(),
        config.nvm().ranks(),
        config.nvm().num_seq_packets(),
    )?;
    validate_side_cache_line(config.dram(), config.line_layout())?;
    validate_side_cache_line(config.nvm(), config.line_layout())
}

fn validate_hybrid_snapshot(
    config: TrafficHybridConfig,
    series_remaining: u32,
    current_side: TrafficHybridSide,
) -> Result<(), TrafficGeneratorError> {
    validate_hybrid_config(config)?;

    let side_config = config.side(current_side);
    if series_remaining > side_config.num_seq_packets() {
        return Err(
            TrafficGeneratorError::TrafficHybridSnapshotSeriesOutsideRange {
                side: current_side,
                series_remaining,
                num_seq_packets: side_config.num_seq_packets(),
            },
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_side_geometry(
    start: Address,
    end: Address,
    block_size: AccessSize,
    page_or_buffer_size: u64,
    banks: u32,
    banks_util: u32,
    ranks: u32,
    num_seq_packets: u32,
) -> Result<(), TrafficGeneratorError> {
    if start.get() >= end.get() {
        return Err(TrafficGeneratorError::EmptyAddressRange { start, end });
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

    validate_address_bits(block_size.bytes(), columns, banks, ranks)?;
    Ok(())
}

fn validate_side_cache_line(
    side_config: TrafficHybridSideConfig,
    line_layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if side_config.block_size().bytes() > line_layout.bytes() {
        return Err(TrafficGeneratorError::BlockSizeExceedsCacheLine {
            block_size: side_config.block_size().bytes(),
            cache_line_size: line_layout.bytes(),
        });
    }

    Ok(())
}

fn validate_address_bits(
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

fn uses_contiguous_columns(mapping: TrafficDramAddressMapping) -> bool {
    matches!(
        mapping,
        TrafficDramAddressMapping::RoRaBaChCo | TrafficDramAddressMapping::RoRaBaCoCh
    )
}

fn insert_bank_rank_col(
    mapping: TrafficDramAddressMapping,
    side_config: TrafficHybridSideConfig,
    address: u64,
    bank: u32,
    rank: u32,
    col: u64,
) -> u64 {
    let block_bits = floor_log2(side_config.block_size().bytes());
    let page_bits = floor_log2(columns_per_page_or_buffer(side_config));
    let bank_bits = floor_log2(u64::from(side_config.banks()));
    let rank_bits = floor_log2(u64::from(side_config.ranks()));

    match mapping {
        TrafficDramAddressMapping::RoRaBaChCo | TrafficDramAddressMapping::RoRaBaCoCh => {
            let with_bank = replace_bits(address, block_bits + page_bits, bank_bits, bank.into());
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

fn replace_col(side_config: TrafficHybridSideConfig, address: u64, col: u64) -> u64 {
    let block_bits = floor_log2(side_config.block_size().bytes());
    let page_bits = floor_log2(columns_per_page_or_buffer(side_config));
    let bank_bits = floor_log2(u64::from(side_config.banks()));
    let rank_bits = floor_log2(u64::from(side_config.ranks()));
    replace_bits(address, block_bits + bank_bits + rank_bits, page_bits, col)
}

fn columns_per_page_or_buffer(side_config: TrafficHybridSideConfig) -> u64 {
    side_config.page_or_buffer_size() / side_config.block_size().bytes()
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
