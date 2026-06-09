use std::collections::BTreeSet;

use crate::probes::{
    MemProbePacket, MemProbePacketAccess, MemProbePacketKind, ProbeEvent, ProbePayload,
    ProbePointId,
};
use crate::StatsError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackDistProbeConfig {
    line_size: u64,
    system_line_size: u64,
    linear_hist_bins: usize,
    log_hist_bins: usize,
    disable_linear_hists: bool,
    disable_log_hists: bool,
}

impl StackDistProbeConfig {
    pub fn new(
        line_size: u64,
        system_line_size: u64,
        linear_hist_bins: usize,
        log_hist_bins: usize,
    ) -> Result<Self, StatsError> {
        Self::builder(line_size, system_line_size)
            .linear_hist_bins(linear_hist_bins)
            .log_hist_bins(log_hist_bins)
            .build()
    }

    pub const fn builder(line_size: u64, system_line_size: u64) -> StackDistProbeConfigBuilder {
        StackDistProbeConfigBuilder::new(line_size, system_line_size)
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn system_line_size(&self) -> u64 {
        self.system_line_size
    }

    pub const fn linear_hist_bins(&self) -> usize {
        self.linear_hist_bins
    }

    pub const fn log_hist_bins(&self) -> usize {
        self.log_hist_bins
    }

    pub const fn disable_linear_hists(&self) -> bool {
        self.disable_linear_hists
    }

    pub const fn disable_log_hists(&self) -> bool {
        self.disable_log_hists
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackDistProbeConfigBuilder {
    line_size: u64,
    system_line_size: u64,
    linear_hist_bins: usize,
    log_hist_bins: usize,
    disable_linear_hists: bool,
    disable_log_hists: bool,
}

impl StackDistProbeConfigBuilder {
    pub const fn new(line_size: u64, system_line_size: u64) -> Self {
        Self {
            line_size,
            system_line_size,
            linear_hist_bins: 16,
            log_hist_bins: 32,
            disable_linear_hists: false,
            disable_log_hists: false,
        }
    }

    pub const fn linear_hist_bins(mut self, bins: usize) -> Self {
        self.linear_hist_bins = bins;
        self
    }

    pub const fn log_hist_bins(mut self, bins: usize) -> Self {
        self.log_hist_bins = bins;
        self
    }

    pub const fn disable_linear_hists(mut self, disabled: bool) -> Self {
        self.disable_linear_hists = disabled;
        self
    }

    pub const fn disable_log_hists(mut self, disabled: bool) -> Self {
        self.disable_log_hists = disabled;
        self
    }

    pub fn build(self) -> Result<StackDistProbeConfig, StatsError> {
        validate_line_size(self.line_size)?;
        validate_system_line_size(self.system_line_size)?;
        if self.system_line_size > self.line_size {
            return Err(StatsError::StackDistLineSizeSmallerThanSystem {
                line_size: self.line_size,
                system_line_size: self.system_line_size,
            });
        }
        validate_histogram_bins("linear", self.linear_hist_bins)?;
        validate_histogram_bins("log", self.log_hist_bins)?;

        Ok(StackDistProbeConfig {
            line_size: self.line_size,
            system_line_size: self.system_line_size,
            linear_hist_bins: self.linear_hist_bins,
            log_hist_bins: self.log_hist_bins,
            disable_linear_hists: self.disable_linear_hists,
            disable_log_hists: self.disable_log_hists,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackDistProbeUpdate {
    access: MemProbePacketAccess,
    aligned_address: u64,
    distance: Option<u64>,
    log_distance: Option<u64>,
    infinite_count: u64,
    sampled_linear: bool,
    sampled_log: bool,
}

impl StackDistProbeUpdate {
    pub const fn infinite(
        access: MemProbePacketAccess,
        aligned_address: u64,
        infinite_count: u64,
    ) -> Self {
        Self {
            access,
            aligned_address,
            distance: None,
            log_distance: None,
            infinite_count,
            sampled_linear: false,
            sampled_log: false,
        }
    }

    pub const fn finite(
        access: MemProbePacketAccess,
        aligned_address: u64,
        distance: u64,
        log_distance: u64,
        sampled_linear: bool,
        sampled_log: bool,
    ) -> Self {
        Self {
            access,
            aligned_address,
            distance: Some(distance),
            log_distance: Some(log_distance),
            infinite_count: 0,
            sampled_linear,
            sampled_log,
        }
    }

    pub const fn access(self) -> MemProbePacketAccess {
        self.access
    }

    pub const fn aligned_address(self) -> u64 {
        self.aligned_address
    }

    pub const fn distance(self) -> Option<u64> {
        self.distance
    }

    pub const fn log_distance(self) -> Option<u64> {
        self.log_distance
    }

    pub const fn infinite_count(self) -> u64 {
        self.infinite_count
    }

    pub const fn sampled_linear(self) -> bool {
        self.sampled_linear
    }

    pub const fn sampled_log(self) -> bool {
        self.sampled_log
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackDistProbeStats {
    infinite_samples: u64,
    finite_samples: u64,
    stack_depth: u64,
    linear_samples: u64,
    log_samples: u64,
}

impl StackDistProbeStats {
    pub const fn new(
        infinite_samples: u64,
        finite_samples: u64,
        stack_depth: u64,
        linear_samples: u64,
        log_samples: u64,
    ) -> Self {
        Self {
            infinite_samples,
            finite_samples,
            stack_depth,
            linear_samples,
            log_samples,
        }
    }

    pub const fn infinite_samples(self) -> u64 {
        self.infinite_samples
    }

    pub const fn finite_samples(self) -> u64 {
        self.finite_samples
    }

    pub const fn stack_depth(self) -> u64 {
        self.stack_depth
    }

    pub const fn linear_samples(self) -> u64 {
        self.linear_samples
    }

    pub const fn log_samples(self) -> u64 {
        self.log_samples
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StackDistHistogramSet {
    read_linear: Vec<(u64, u64)>,
    write_linear: Vec<(u64, u64)>,
    read_log: Vec<(u64, u64)>,
    write_log: Vec<(u64, u64)>,
}

impl StackDistHistogramSet {
    pub fn from_buckets(
        read_linear: Vec<(u64, u64)>,
        write_linear: Vec<(u64, u64)>,
        read_log: Vec<(u64, u64)>,
        write_log: Vec<(u64, u64)>,
    ) -> Self {
        Self {
            read_linear,
            write_linear,
            read_log,
            write_log,
        }
    }

    pub fn read_linear(&self) -> &[(u64, u64)] {
        &self.read_linear
    }

    pub fn write_linear(&self) -> &[(u64, u64)] {
        &self.write_linear
    }

    pub fn read_log(&self) -> &[(u64, u64)] {
        &self.read_log
    }

    pub fn write_log(&self) -> &[(u64, u64)] {
        &self.write_log
    }

    fn observe_linear(
        &mut self,
        access: MemProbePacketAccess,
        distance: u64,
    ) -> Result<(), StatsError> {
        let buckets = match access {
            MemProbePacketAccess::Read => &mut self.read_linear,
            MemProbePacketAccess::Write => &mut self.write_linear,
            MemProbePacketAccess::Other => return Ok(()),
        };
        observe_bucket(buckets, distance)
    }

    fn observe_log(
        &mut self,
        access: MemProbePacketAccess,
        distance: u64,
    ) -> Result<(), StatsError> {
        let buckets = match access {
            MemProbePacketAccess::Read => &mut self.read_log,
            MemProbePacketAccess::Write => &mut self.write_log,
            MemProbePacketAccess::Other => return Ok(()),
        };
        observe_bucket(buckets, distance)
    }

    fn validate(&self) -> Result<(), StatsError> {
        validate_histogram("read_linear", &self.read_linear)?;
        validate_histogram("write_linear", &self.write_linear)?;
        validate_histogram("read_log", &self.read_log)?;
        validate_histogram("write_log", &self.write_log)?;
        Ok(())
    }

    fn linear_sample_count(&self) -> Result<u64, StatsError> {
        histogram_sample_count(&self.read_linear, &self.write_linear)
    }

    fn log_sample_count(&self) -> Result<u64, StatsError> {
        histogram_sample_count(&self.read_log, &self.write_log)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackDistProbeSnapshot {
    config: StackDistProbeConfig,
    stack: Vec<u64>,
    infinite_samples: u64,
    finite_samples: u64,
    histograms: StackDistHistogramSet,
}

impl StackDistProbeSnapshot {
    pub const fn new(
        config: StackDistProbeConfig,
        stack: Vec<u64>,
        infinite_samples: u64,
        finite_samples: u64,
        histograms: StackDistHistogramSet,
    ) -> Self {
        Self {
            config,
            stack,
            infinite_samples,
            finite_samples,
            histograms,
        }
    }

    pub const fn config(&self) -> &StackDistProbeConfig {
        &self.config
    }

    pub fn stack(&self) -> &[u64] {
        &self.stack
    }

    pub const fn infinite_samples(&self) -> u64 {
        self.infinite_samples
    }

    pub const fn finite_samples(&self) -> u64 {
        self.finite_samples
    }

    pub const fn histograms(&self) -> &StackDistHistogramSet {
        &self.histograms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackDistProbe {
    config: StackDistProbeConfig,
    stack: Vec<u64>,
    infinite_samples: u64,
    finite_samples: u64,
    linear_samples: u64,
    log_samples: u64,
    histograms: StackDistHistogramSet,
}

impl StackDistProbe {
    pub fn new(config: StackDistProbeConfig) -> Self {
        Self {
            config,
            stack: Vec::new(),
            infinite_samples: 0,
            finite_samples: 0,
            linear_samples: 0,
            log_samples: 0,
            histograms: StackDistHistogramSet::default(),
        }
    }

    pub fn from_snapshot(snapshot: &StackDistProbeSnapshot) -> Result<Self, StatsError> {
        validate_snapshot_stack(snapshot.config(), snapshot.stack())?;
        validate_snapshot_stack_depth(snapshot.stack(), snapshot.infinite_samples())?;
        snapshot.histograms().validate()?;
        let (linear_samples, log_samples) = validate_snapshot_sample_counts(
            snapshot.config(),
            snapshot.finite_samples(),
            snapshot.histograms(),
        )?;

        Ok(Self {
            config: snapshot.config().clone(),
            stack: snapshot.stack().to_vec(),
            infinite_samples: snapshot.infinite_samples(),
            finite_samples: snapshot.finite_samples(),
            linear_samples,
            log_samples,
            histograms: snapshot.histograms().clone(),
        })
    }

    pub const fn config(&self) -> &StackDistProbeConfig {
        &self.config
    }

    pub fn stack(&self) -> &[u64] {
        &self.stack
    }

    pub const fn histograms(&self) -> &StackDistHistogramSet {
        &self.histograms
    }

    pub fn observe_packet(
        &mut self,
        packet: &MemProbePacket,
    ) -> Result<Option<StackDistProbeUpdate>, StatsError> {
        if packet.kind() != MemProbePacketKind::Request {
            return Ok(None);
        }
        let access = packet.access();
        if !matches!(
            access,
            MemProbePacketAccess::Read | MemProbePacketAccess::Write
        ) {
            return Ok(None);
        }

        let aligned_address = align_down(packet.address(), self.config.line_size());
        let Some(position) = self
            .stack
            .iter()
            .position(|address| *address == aligned_address)
        else {
            self.stack.insert(0, aligned_address);
            self.infinite_samples = self
                .infinite_samples
                .checked_add(1)
                .ok_or(StatsError::StackDistSampleCountOverflow)?;
            return Ok(Some(StackDistProbeUpdate::infinite(
                access,
                aligned_address,
                self.infinite_samples,
            )));
        };

        let distance = position as u64;
        self.stack.remove(position);
        self.stack.insert(0, aligned_address);
        self.finite_samples = self
            .finite_samples
            .checked_add(1)
            .ok_or(StatsError::StackDistSampleCountOverflow)?;

        let log_distance = log_distance(distance);
        let sampled_linear = !self.config.disable_linear_hists();
        if sampled_linear {
            self.histograms.observe_linear(access, distance)?;
            self.linear_samples = self
                .linear_samples
                .checked_add(1)
                .ok_or(StatsError::StackDistSampleCountOverflow)?;
        }
        let sampled_log = !self.config.disable_log_hists();
        if sampled_log {
            self.histograms.observe_log(access, log_distance)?;
            self.log_samples = self
                .log_samples
                .checked_add(1)
                .ok_or(StatsError::StackDistSampleCountOverflow)?;
        }

        Ok(Some(StackDistProbeUpdate::finite(
            access,
            aligned_address,
            distance,
            log_distance,
            sampled_linear,
            sampled_log,
        )))
    }

    pub fn observe_probe_event(
        &mut self,
        event: &ProbeEvent,
        request_point: ProbePointId,
    ) -> Result<Option<StackDistProbeUpdate>, StatsError> {
        if event.point() != request_point {
            return Ok(None);
        }
        let ProbePayload::MemoryPacket(packet) = event.payload() else {
            return Ok(None);
        };
        self.observe_packet(packet)
    }

    pub fn stats(&self) -> StackDistProbeStats {
        StackDistProbeStats::new(
            self.infinite_samples,
            self.finite_samples,
            self.stack.len() as u64,
            self.linear_samples,
            self.log_samples,
        )
    }

    pub fn snapshot(&self) -> StackDistProbeSnapshot {
        StackDistProbeSnapshot::new(
            self.config.clone(),
            self.stack.clone(),
            self.infinite_samples,
            self.finite_samples,
            self.histograms.clone(),
        )
    }
}

fn validate_line_size(line_size: u64) -> Result<(), StatsError> {
    if line_size == 0 || !line_size.is_power_of_two() {
        return Err(StatsError::InvalidStackDistLineSize { line_size });
    }
    Ok(())
}

fn validate_system_line_size(system_line_size: u64) -> Result<(), StatsError> {
    if system_line_size == 0 || !system_line_size.is_power_of_two() {
        return Err(StatsError::InvalidStackDistSystemLineSize { system_line_size });
    }
    Ok(())
}

fn validate_histogram_bins(name: &'static str, bins: usize) -> Result<(), StatsError> {
    if bins == 0 {
        return Err(StatsError::InvalidStackDistHistogramBins { name, bins });
    }
    Ok(())
}

fn validate_snapshot_stack(config: &StackDistProbeConfig, stack: &[u64]) -> Result<(), StatsError> {
    let mut seen = BTreeSet::new();
    for address in stack {
        if *address != align_down(*address, config.line_size()) {
            return Err(StatsError::UnalignedStackDistSnapshotAddress {
                line_size: config.line_size(),
                address: *address,
            });
        }
        if !seen.insert(*address) {
            return Err(StatsError::DuplicateStackDistSnapshotAddress { address: *address });
        }
    }
    Ok(())
}

fn validate_snapshot_stack_depth(stack: &[u64], infinite_samples: u64) -> Result<(), StatsError> {
    let stack_depth = stack.len() as u64;
    if stack_depth != infinite_samples {
        return Err(StatsError::StackDistSnapshotStackDepthMismatch {
            stack_depth,
            infinite_samples,
        });
    }
    Ok(())
}

fn validate_histogram(name: &'static str, buckets: &[(u64, u64)]) -> Result<(), StatsError> {
    let mut seen = BTreeSet::new();
    for (bucket, _) in buckets {
        if !seen.insert(*bucket) {
            return Err(StatsError::DuplicateStackDistHistogramBucket {
                name,
                bucket: *bucket,
            });
        }
    }
    Ok(())
}

fn observe_bucket(buckets: &mut Vec<(u64, u64)>, bucket: u64) -> Result<(), StatsError> {
    if let Some((_, count)) = buckets.iter_mut().find(|(existing, _)| *existing == bucket) {
        *count = count
            .checked_add(1)
            .ok_or(StatsError::StackDistSampleCountOverflow)?;
        return Ok(());
    }

    buckets.push((bucket, 1));
    buckets.sort_unstable_by_key(|(bucket, _)| *bucket);
    Ok(())
}

fn histogram_sample_count(left: &[(u64, u64)], right: &[(u64, u64)]) -> Result<u64, StatsError> {
    let mut total = 0_u64;
    for (_, count) in left.iter().chain(right.iter()) {
        total = total
            .checked_add(*count)
            .ok_or(StatsError::StackDistSampleCountOverflow)?;
    }
    Ok(total)
}

fn validate_snapshot_sample_counts(
    config: &StackDistProbeConfig,
    finite_samples: u64,
    histograms: &StackDistHistogramSet,
) -> Result<(u64, u64), StatsError> {
    let linear_samples = histograms.linear_sample_count()?;
    validate_snapshot_sample_count(
        expected_histogram_samples(config.disable_linear_hists(), finite_samples),
        linear_samples,
    )?;

    let log_samples = histograms.log_sample_count()?;
    validate_snapshot_sample_count(
        expected_histogram_samples(config.disable_log_hists(), finite_samples),
        log_samples,
    )?;

    Ok((linear_samples, log_samples))
}

const fn expected_histogram_samples(disabled: bool, finite_samples: u64) -> u64 {
    if disabled {
        0
    } else {
        finite_samples
    }
}

fn validate_snapshot_sample_count(expected: u64, observed: u64) -> Result<(), StatsError> {
    if observed != expected {
        return Err(StatsError::StackDistSnapshotSampleCountMismatch { expected, observed });
    }
    Ok(())
}

fn align_down(address: u64, bytes: u64) -> u64 {
    address & !(bytes - 1)
}

fn log_distance(distance: u64) -> u64 {
    if distance == 0 {
        1
    } else {
        u64::from(distance.ilog2())
    }
}
