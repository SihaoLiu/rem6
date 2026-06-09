use std::collections::BTreeSet;
use std::fmt;

use crate::probes::{MemProbePacket, MemProbePacketKind, ProbeEvent, ProbePayload, ProbePointId};
use crate::StatsError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemFootprintGranularity {
    CacheLine,
    Page,
}

impl fmt::Display for MemFootprintGranularity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CacheLine => write!(formatter, "cache-line"),
            Self::Page => write!(formatter, "page"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemFootprintAddressRange {
    start: u64,
    end: u64,
}

impl MemFootprintAddressRange {
    pub fn new(start: u64, bytes: u64) -> Result<Self, StatsError> {
        if bytes == 0 {
            return Err(StatsError::EmptyMemFootprintAddressRange);
        }
        let end = start
            .checked_add(bytes)
            .ok_or(StatsError::MemFootprintAddressRangeOverflow { start, bytes })?;
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }

    pub const fn contains(self, address: u64) -> bool {
        self.start <= address && address < self.end
    }

    const fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemFootprintProbeConfig {
    cache_line_size: u64,
    page_size: u64,
    ranges: Vec<MemFootprintAddressRange>,
    cache_line_capacity: u64,
    page_capacity: u64,
}

impl MemFootprintProbeConfig {
    pub fn new(
        cache_line_size: u64,
        page_size: u64,
        mut ranges: Vec<MemFootprintAddressRange>,
    ) -> Result<Self, StatsError> {
        validate_granularity("cache_line", cache_line_size)?;
        validate_granularity("page", page_size)?;
        if page_size < cache_line_size {
            return Err(StatsError::MemFootprintGranularityOrder {
                cache_line_size,
                page_size,
            });
        }
        if ranges.is_empty() {
            return Err(StatsError::EmptyMemFootprintAddressMap);
        }

        ranges.sort_by_key(|range| (range.start(), range.end()));
        for index in 1..ranges.len() {
            let existing = ranges[index - 1];
            let requested = ranges[index];
            if existing.overlaps(requested) {
                return Err(StatsError::OverlappingMemFootprintAddressRange {
                    existing_start: existing.start(),
                    existing_end: existing.end(),
                    requested_start: requested.start(),
                    requested_end: requested.end(),
                });
            }
        }

        let cache_line_capacity =
            aligned_bucket_capacity(&ranges, cache_line_size, MemFootprintGranularity::CacheLine)?;
        let page_capacity =
            aligned_bucket_capacity(&ranges, page_size, MemFootprintGranularity::Page)?;

        Ok(Self {
            cache_line_size,
            page_size,
            ranges,
            cache_line_capacity,
            page_capacity,
        })
    }

    pub const fn cache_line_size(&self) -> u64 {
        self.cache_line_size
    }

    pub const fn page_size(&self) -> u64 {
        self.page_size
    }

    pub fn ranges(&self) -> &[MemFootprintAddressRange] {
        &self.ranges
    }

    pub fn contains_address(&self, address: u64) -> bool {
        self.ranges.iter().any(|range| range.contains(address))
    }

    fn bucket_overlaps_memory(&self, address: u64, granularity: MemFootprintGranularity) -> bool {
        let bytes = self.granularity_size(granularity);
        let Some(end) = address.checked_add(bytes) else {
            return false;
        };
        self.ranges
            .iter()
            .any(|range| address < range.end() && range.start() < end)
    }

    const fn granularity_size(&self, granularity: MemFootprintGranularity) -> u64 {
        match granularity {
            MemFootprintGranularity::CacheLine => self.cache_line_size,
            MemFootprintGranularity::Page => self.page_size,
        }
    }

    const fn granularity_capacity(&self, granularity: MemFootprintGranularity) -> u64 {
        match granularity {
            MemFootprintGranularity::CacheLine => self.cache_line_capacity,
            MemFootprintGranularity::Page => self.page_capacity,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemFootprintStats {
    cache_line: u64,
    cache_line_total: u64,
    page: u64,
    page_total: u64,
}

impl MemFootprintStats {
    pub const fn new(cache_line: u64, cache_line_total: u64, page: u64, page_total: u64) -> Self {
        Self {
            cache_line,
            cache_line_total,
            page,
            page_total,
        }
    }

    pub const fn cache_line(self) -> u64 {
        self.cache_line
    }

    pub const fn cache_line_total(self) -> u64 {
        self.cache_line_total
    }

    pub const fn page(self) -> u64 {
        self.page
    }

    pub const fn page_total(self) -> u64 {
        self.page_total
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemFootprintProbeSnapshot {
    cache_lines: Vec<u64>,
    cache_lines_total: Vec<u64>,
    pages: Vec<u64>,
    pages_total: Vec<u64>,
}

impl MemFootprintProbeSnapshot {
    pub fn new(
        cache_lines: Vec<u64>,
        cache_lines_total: Vec<u64>,
        pages: Vec<u64>,
        pages_total: Vec<u64>,
    ) -> Self {
        Self {
            cache_lines,
            cache_lines_total,
            pages,
            pages_total,
        }
    }

    pub fn cache_lines(&self) -> &[u64] {
        &self.cache_lines
    }

    pub fn cache_lines_total(&self) -> &[u64] {
        &self.cache_lines_total
    }

    pub fn pages(&self) -> &[u64] {
        &self.pages
    }

    pub fn pages_total(&self) -> &[u64] {
        &self.pages_total
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemFootprintProbe {
    config: MemFootprintProbeConfig,
    cache_lines: BTreeSet<u64>,
    cache_lines_total: BTreeSet<u64>,
    pages: BTreeSet<u64>,
    pages_total: BTreeSet<u64>,
}

impl MemFootprintProbe {
    pub fn new(config: MemFootprintProbeConfig) -> Self {
        Self {
            config,
            cache_lines: BTreeSet::new(),
            cache_lines_total: BTreeSet::new(),
            pages: BTreeSet::new(),
            pages_total: BTreeSet::new(),
        }
    }

    pub fn from_snapshot(
        config: MemFootprintProbeConfig,
        snapshot: &MemFootprintProbeSnapshot,
    ) -> Result<Self, StatsError> {
        let cache_lines = restore_snapshot_set(
            &config,
            snapshot.cache_lines(),
            MemFootprintGranularity::CacheLine,
        )?;
        let cache_lines_total = restore_snapshot_set(
            &config,
            snapshot.cache_lines_total(),
            MemFootprintGranularity::CacheLine,
        )?;
        let pages = restore_snapshot_set(&config, snapshot.pages(), MemFootprintGranularity::Page)?;
        let pages_total = restore_snapshot_set(
            &config,
            snapshot.pages_total(),
            MemFootprintGranularity::Page,
        )?;

        validate_current_subset_total(
            &cache_lines,
            &cache_lines_total,
            MemFootprintGranularity::CacheLine,
        )?;
        validate_current_subset_total(&pages, &pages_total, MemFootprintGranularity::Page)?;
        validate_line_page_consistency(&config, &cache_lines, &pages)?;
        validate_line_page_consistency(&config, &cache_lines_total, &pages_total)?;
        validate_page_line_consistency(&config, &pages, &cache_lines)?;
        validate_page_line_consistency(&config, &pages_total, &cache_lines_total)?;

        Ok(Self {
            config,
            cache_lines,
            cache_lines_total,
            pages,
            pages_total,
        })
    }

    pub fn config(&self) -> &MemFootprintProbeConfig {
        &self.config
    }

    pub fn observe_packet(
        &mut self,
        packet: &MemProbePacket,
    ) -> Result<Option<MemFootprintStats>, StatsError> {
        if packet.kind() != MemProbePacketKind::Request
            || !self.config.contains_address(packet.address())
        {
            return Ok(None);
        }

        let cache_line = align_down(packet.address(), self.config.cache_line_size());
        let page = align_down(packet.address(), self.config.page_size());
        let changed = self.cache_lines.insert(cache_line)
            | self.cache_lines_total.insert(cache_line)
            | self.pages.insert(page)
            | self.pages_total.insert(page);
        validate_set_capacity(
            self.cache_lines.len(),
            self.config
                .granularity_capacity(MemFootprintGranularity::CacheLine),
            MemFootprintGranularity::CacheLine,
        )?;
        validate_set_capacity(
            self.pages.len(),
            self.config
                .granularity_capacity(MemFootprintGranularity::Page),
            MemFootprintGranularity::Page,
        )?;

        if changed {
            Ok(Some(self.stats()?))
        } else {
            Ok(None)
        }
    }

    pub fn observe_probe_event(
        &mut self,
        event: &ProbeEvent,
        request_point: ProbePointId,
    ) -> Result<Option<MemFootprintStats>, StatsError> {
        if event.point() != request_point {
            return Ok(None);
        }
        let ProbePayload::MemoryPacket(packet) = event.payload() else {
            return Ok(None);
        };
        self.observe_packet(packet)
    }

    pub fn reset_current(&mut self) {
        self.cache_lines.clear();
        self.pages.clear();
    }

    pub fn stats(&self) -> Result<MemFootprintStats, StatsError> {
        Ok(MemFootprintStats::new(
            footprint_bytes(
                self.cache_lines.len(),
                self.config.cache_line_size(),
                MemFootprintGranularity::CacheLine,
            )?,
            footprint_bytes(
                self.cache_lines_total.len(),
                self.config.cache_line_size(),
                MemFootprintGranularity::CacheLine,
            )?,
            footprint_bytes(
                self.pages.len(),
                self.config.page_size(),
                MemFootprintGranularity::Page,
            )?,
            footprint_bytes(
                self.pages_total.len(),
                self.config.page_size(),
                MemFootprintGranularity::Page,
            )?,
        ))
    }

    pub fn snapshot(&self) -> MemFootprintProbeSnapshot {
        MemFootprintProbeSnapshot::new(
            self.cache_lines.iter().copied().collect(),
            self.cache_lines_total.iter().copied().collect(),
            self.pages.iter().copied().collect(),
            self.pages_total.iter().copied().collect(),
        )
    }
}

fn validate_granularity(name: &str, bytes: u64) -> Result<(), StatsError> {
    if bytes == 0 || !bytes.is_power_of_two() {
        return Err(StatsError::InvalidMemFootprintGranularity {
            name: name.to_string(),
            bytes,
        });
    }
    Ok(())
}

fn align_down(address: u64, bytes: u64) -> u64 {
    address & !(bytes - 1)
}

fn aligned_bucket_capacity(
    ranges: &[MemFootprintAddressRange],
    bytes: u64,
    granularity: MemFootprintGranularity,
) -> Result<u64, StatsError> {
    let mut buckets = Vec::new();
    for range in ranges {
        let start = align_down(range.start(), bytes);
        let last = align_down(range.end() - 1, bytes);
        let end = last
            .checked_add(bytes)
            .ok_or(StatsError::MemFootprintValueOverflow { granularity })?;
        buckets.push((start, end));
    }
    buckets.sort_unstable();

    let mut total = 0_u64;
    let mut current: Option<(u64, u64)> = None;
    for bucket in buckets {
        match current {
            Some((start, end)) if bucket.0 <= end => {
                current = Some((start, end.max(bucket.1)));
            }
            Some((start, end)) => {
                total = add_bucket_span(total, start, end, bytes, granularity)?;
                current = Some(bucket);
            }
            None => current = Some(bucket),
        }
    }
    if let Some((start, end)) = current {
        total = add_bucket_span(total, start, end, bytes, granularity)?;
    }
    Ok(total)
}

fn add_bucket_span(
    total: u64,
    start: u64,
    end: u64,
    bytes: u64,
    granularity: MemFootprintGranularity,
) -> Result<u64, StatsError> {
    total
        .checked_add((end - start) / bytes)
        .ok_or(StatsError::MemFootprintValueOverflow { granularity })
}

fn restore_snapshot_set(
    config: &MemFootprintProbeConfig,
    addresses: &[u64],
    granularity: MemFootprintGranularity,
) -> Result<BTreeSet<u64>, StatsError> {
    let bytes = config.granularity_size(granularity);
    let mut set = BTreeSet::new();
    for address in addresses {
        if *address != align_down(*address, bytes) {
            return Err(StatsError::UnalignedMemFootprintSnapshotAddress {
                granularity,
                address: *address,
            });
        }
        if !config.bucket_overlaps_memory(*address, granularity) {
            return Err(StatsError::MemFootprintSnapshotAddressOutsideMemory {
                granularity,
                address: *address,
            });
        }
        if !set.insert(*address) {
            return Err(StatsError::DuplicateMemFootprintSnapshotAddress {
                granularity,
                address: *address,
            });
        }
    }
    validate_set_capacity(
        set.len(),
        config.granularity_capacity(granularity),
        granularity,
    )?;
    Ok(set)
}

fn validate_current_subset_total(
    current: &BTreeSet<u64>,
    total: &BTreeSet<u64>,
    granularity: MemFootprintGranularity,
) -> Result<(), StatsError> {
    for address in current {
        if !total.contains(address) {
            return Err(StatsError::MemFootprintSnapshotCurrentNotInTotal {
                granularity,
                address: *address,
            });
        }
    }
    Ok(())
}

fn validate_line_page_consistency(
    config: &MemFootprintProbeConfig,
    cache_lines: &BTreeSet<u64>,
    pages: &BTreeSet<u64>,
) -> Result<(), StatsError> {
    for line in cache_lines {
        let page = align_down(*line, config.page_size());
        if !pages.contains(&page) {
            return Err(StatsError::MemFootprintSnapshotGranularityMismatch {
                finer: MemFootprintGranularity::CacheLine,
                coarser: MemFootprintGranularity::Page,
                address: *line,
                parent: page,
            });
        }
    }
    Ok(())
}

fn validate_page_line_consistency(
    config: &MemFootprintProbeConfig,
    pages: &BTreeSet<u64>,
    cache_lines: &BTreeSet<u64>,
) -> Result<(), StatsError> {
    for page in pages {
        let page_end =
            page.checked_add(config.page_size())
                .ok_or(StatsError::MemFootprintValueOverflow {
                    granularity: MemFootprintGranularity::Page,
                })?;
        if !cache_lines
            .range(*page..page_end)
            .any(|line| align_down(*line, config.page_size()) == *page)
        {
            return Err(StatsError::MemFootprintSnapshotGranularityMismatch {
                finer: MemFootprintGranularity::CacheLine,
                coarser: MemFootprintGranularity::Page,
                address: *page,
                parent: *page,
            });
        }
    }
    Ok(())
}

fn validate_set_capacity(
    set_len: usize,
    capacity: u64,
    granularity: MemFootprintGranularity,
) -> Result<(), StatsError> {
    if set_len as u64 > capacity {
        return Err(StatsError::MemFootprintSnapshotExceedsMemory {
            granularity,
            observed: set_len as u64,
            capacity,
        });
    }
    Ok(())
}

fn footprint_bytes(
    set_len: usize,
    bytes: u64,
    granularity: MemFootprintGranularity,
) -> Result<u64, StatsError> {
    (set_len as u64)
        .checked_mul(bytes)
        .ok_or(StatsError::MemFootprintValueOverflow { granularity })
}
