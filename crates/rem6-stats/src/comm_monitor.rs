use std::collections::BTreeSet;

use rem6_kernel::Tick;

use crate::probes::{
    MemProbePacket, MemProbePacketAccess, MemProbePacketKind, ProbeEvent, ProbePayload,
    ProbePointId,
};
use crate::StatsError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommMonitorConfig {
    sample_period: Tick,
    tick_frequency: u64,
    burst_length_bins: usize,
    bandwidth_bins: usize,
    latency_bins: usize,
    itt_bins: usize,
    outstanding_bins: usize,
    transaction_bins: usize,
    read_addr_mask: u64,
    write_addr_mask: u64,
    disable_burst_length_hists: bool,
    disable_bandwidth_hists: bool,
    disable_latency_hists: bool,
    disable_itt_dists: bool,
    disable_outstanding_hists: bool,
    disable_transaction_hists: bool,
    disable_addr_dists: bool,
}

impl CommMonitorConfig {
    pub const fn builder(sample_period: Tick) -> CommMonitorConfigBuilder {
        CommMonitorConfigBuilder::new(sample_period)
    }

    pub const fn sample_period(&self) -> Tick {
        self.sample_period
    }

    pub const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub const fn read_addr_mask(&self) -> u64 {
        self.read_addr_mask
    }

    pub const fn write_addr_mask(&self) -> u64 {
        self.write_addr_mask
    }

    pub const fn disable_burst_length_hists(&self) -> bool {
        self.disable_burst_length_hists
    }

    pub const fn disable_bandwidth_hists(&self) -> bool {
        self.disable_bandwidth_hists
    }

    pub const fn disable_latency_hists(&self) -> bool {
        self.disable_latency_hists
    }

    pub const fn disable_itt_dists(&self) -> bool {
        self.disable_itt_dists
    }

    pub const fn disable_outstanding_hists(&self) -> bool {
        self.disable_outstanding_hists
    }

    pub const fn disable_transaction_hists(&self) -> bool {
        self.disable_transaction_hists
    }

    pub const fn disable_addr_dists(&self) -> bool {
        self.disable_addr_dists
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommMonitorConfigBuilder {
    sample_period: Tick,
    tick_frequency: u64,
    burst_length_bins: usize,
    bandwidth_bins: usize,
    latency_bins: usize,
    itt_bins: usize,
    outstanding_bins: usize,
    transaction_bins: usize,
    read_addr_mask: u64,
    write_addr_mask: u64,
    disable_burst_length_hists: bool,
    disable_bandwidth_hists: bool,
    disable_latency_hists: bool,
    disable_itt_dists: bool,
    disable_outstanding_hists: bool,
    disable_transaction_hists: bool,
    disable_addr_dists: bool,
}

impl CommMonitorConfigBuilder {
    pub const fn new(sample_period: Tick) -> Self {
        Self {
            sample_period,
            tick_frequency: 1,
            burst_length_bins: 20,
            bandwidth_bins: 20,
            latency_bins: 20,
            itt_bins: 20,
            outstanding_bins: 20,
            transaction_bins: 20,
            read_addr_mask: u64::MAX,
            write_addr_mask: u64::MAX,
            disable_burst_length_hists: false,
            disable_bandwidth_hists: false,
            disable_latency_hists: false,
            disable_itt_dists: false,
            disable_outstanding_hists: false,
            disable_transaction_hists: false,
            disable_addr_dists: true,
        }
    }

    pub const fn burst_length_bins(mut self, bins: usize) -> Self {
        self.burst_length_bins = bins;
        self
    }

    pub const fn tick_frequency(mut self, tick_frequency: u64) -> Self {
        self.tick_frequency = tick_frequency;
        self
    }

    pub const fn bandwidth_bins(mut self, bins: usize) -> Self {
        self.bandwidth_bins = bins;
        self
    }

    pub const fn latency_bins(mut self, bins: usize) -> Self {
        self.latency_bins = bins;
        self
    }

    pub const fn itt_bins(mut self, bins: usize) -> Self {
        self.itt_bins = bins;
        self
    }

    pub const fn outstanding_bins(mut self, bins: usize) -> Self {
        self.outstanding_bins = bins;
        self
    }

    pub const fn transaction_bins(mut self, bins: usize) -> Self {
        self.transaction_bins = bins;
        self
    }

    pub const fn read_addr_mask(mut self, mask: u64) -> Self {
        self.read_addr_mask = mask;
        self.disable_addr_dists = false;
        self
    }

    pub const fn write_addr_mask(mut self, mask: u64) -> Self {
        self.write_addr_mask = mask;
        self.disable_addr_dists = false;
        self
    }

    pub const fn disable_burst_length_hists(mut self, disabled: bool) -> Self {
        self.disable_burst_length_hists = disabled;
        self
    }

    pub const fn disable_bandwidth_hists(mut self, disabled: bool) -> Self {
        self.disable_bandwidth_hists = disabled;
        self
    }

    pub const fn disable_latency_hists(mut self, disabled: bool) -> Self {
        self.disable_latency_hists = disabled;
        self
    }

    pub const fn disable_itt_dists(mut self, disabled: bool) -> Self {
        self.disable_itt_dists = disabled;
        self
    }

    pub const fn disable_outstanding_hists(mut self, disabled: bool) -> Self {
        self.disable_outstanding_hists = disabled;
        self
    }

    pub const fn disable_transaction_hists(mut self, disabled: bool) -> Self {
        self.disable_transaction_hists = disabled;
        self
    }

    pub const fn disable_addr_dists(mut self, disabled: bool) -> Self {
        self.disable_addr_dists = disabled;
        self
    }

    pub fn build(self) -> Result<CommMonitorConfig, StatsError> {
        if self.sample_period == 0 {
            return Err(StatsError::InvalidCommMonitorSamplePeriod {
                sample_period: self.sample_period,
            });
        }
        if self.tick_frequency == 0 {
            return Err(StatsError::InvalidCommMonitorTickFrequency {
                tick_frequency: self.tick_frequency,
            });
        }
        validate_bins("burst_length", self.burst_length_bins)?;
        validate_bins("bandwidth", self.bandwidth_bins)?;
        validate_bins("latency", self.latency_bins)?;
        validate_bins("itt", self.itt_bins)?;
        validate_bins("outstanding", self.outstanding_bins)?;
        validate_bins("transaction", self.transaction_bins)?;

        Ok(CommMonitorConfig {
            sample_period: self.sample_period,
            tick_frequency: self.tick_frequency,
            burst_length_bins: self.burst_length_bins,
            bandwidth_bins: self.bandwidth_bins,
            latency_bins: self.latency_bins,
            itt_bins: self.itt_bins,
            outstanding_bins: self.outstanding_bins,
            transaction_bins: self.transaction_bins,
            read_addr_mask: self.read_addr_mask,
            write_addr_mask: self.write_addr_mask,
            disable_burst_length_hists: self.disable_burst_length_hists,
            disable_bandwidth_hists: self.disable_bandwidth_hists,
            disable_latency_hists: self.disable_latency_hists,
            disable_itt_dists: self.disable_itt_dists,
            disable_outstanding_hists: self.disable_outstanding_hists,
            disable_transaction_hists: self.disable_transaction_hists,
            disable_addr_dists: self.disable_addr_dists,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommMonitorStats {
    read_transactions: u64,
    write_transactions: u64,
    read_bytes: u64,
    written_bytes: u64,
    total_read_bytes: u64,
    total_written_bytes: u64,
    outstanding_reads: u64,
    outstanding_writes: u64,
}

impl CommMonitorStats {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        read_transactions: u64,
        write_transactions: u64,
        read_bytes: u64,
        written_bytes: u64,
        total_read_bytes: u64,
        total_written_bytes: u64,
        outstanding_reads: u64,
        outstanding_writes: u64,
    ) -> Self {
        Self {
            read_transactions,
            write_transactions,
            read_bytes,
            written_bytes,
            total_read_bytes,
            total_written_bytes,
            outstanding_reads,
            outstanding_writes,
        }
    }

    pub const fn read_transactions(self) -> u64 {
        self.read_transactions
    }

    pub const fn write_transactions(self) -> u64 {
        self.write_transactions
    }

    pub const fn read_bytes(self) -> u64 {
        self.read_bytes
    }

    pub const fn written_bytes(self) -> u64 {
        self.written_bytes
    }

    pub const fn total_read_bytes(self) -> u64 {
        self.total_read_bytes
    }

    pub const fn total_written_bytes(self) -> u64 {
        self.total_written_bytes
    }

    pub const fn outstanding_reads(self) -> u64 {
        self.outstanding_reads
    }

    pub const fn outstanding_writes(self) -> u64 {
        self.outstanding_writes
    }

    fn reset_window(&mut self) {
        self.read_transactions = 0;
        self.write_transactions = 0;
        self.read_bytes = 0;
        self.written_bytes = 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommMonitorPendingRequest {
    packet_id: u64,
    access: MemProbePacketAccess,
    request_tick: Tick,
    size: u64,
}

impl CommMonitorPendingRequest {
    pub const fn new(
        packet_id: u64,
        access: MemProbePacketAccess,
        request_tick: Tick,
        size: u64,
    ) -> Self {
        Self {
            packet_id,
            access,
            request_tick,
            size,
        }
    }

    pub const fn packet_id(self) -> u64 {
        self.packet_id
    }

    pub const fn access(self) -> MemProbePacketAccess {
        self.access
    }

    pub const fn request_tick(self) -> Tick {
        self.request_tick
    }

    pub const fn size(self) -> u64 {
        self.size
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommMonitorHistograms {
    read_burst_lengths: Vec<(u64, u64)>,
    write_burst_lengths: Vec<(u64, u64)>,
    read_bandwidth: Vec<(u64, u64)>,
    write_bandwidth: Vec<(u64, u64)>,
    read_latencies: Vec<(u64, u64)>,
    write_latencies: Vec<(u64, u64)>,
    read_to_read_times: Vec<(u64, u64)>,
    write_to_write_times: Vec<(u64, u64)>,
    request_to_request_times: Vec<(u64, u64)>,
    outstanding_reads: Vec<(u64, u64)>,
    outstanding_writes: Vec<(u64, u64)>,
    read_transactions: Vec<(u64, u64)>,
    write_transactions: Vec<(u64, u64)>,
    read_addresses: Vec<(u64, u64)>,
    write_addresses: Vec<(u64, u64)>,
}

impl CommMonitorHistograms {
    #[allow(clippy::too_many_arguments)]
    pub fn from_buckets(
        read_burst_lengths: Vec<(u64, u64)>,
        write_burst_lengths: Vec<(u64, u64)>,
        read_bandwidth: Vec<(u64, u64)>,
        write_bandwidth: Vec<(u64, u64)>,
        read_latencies: Vec<(u64, u64)>,
        write_latencies: Vec<(u64, u64)>,
        read_to_read_times: Vec<(u64, u64)>,
        write_to_write_times: Vec<(u64, u64)>,
        request_to_request_times: Vec<(u64, u64)>,
        outstanding_reads: Vec<(u64, u64)>,
        outstanding_writes: Vec<(u64, u64)>,
        read_transactions: Vec<(u64, u64)>,
        write_transactions: Vec<(u64, u64)>,
    ) -> Self {
        Self {
            read_burst_lengths,
            write_burst_lengths,
            read_bandwidth,
            write_bandwidth,
            read_latencies,
            write_latencies,
            read_to_read_times,
            write_to_write_times,
            request_to_request_times,
            outstanding_reads,
            outstanding_writes,
            read_transactions,
            write_transactions,
            read_addresses: Vec::new(),
            write_addresses: Vec::new(),
        }
    }

    pub fn read_burst_lengths(&self) -> &[(u64, u64)] {
        &self.read_burst_lengths
    }

    pub fn write_burst_lengths(&self) -> &[(u64, u64)] {
        &self.write_burst_lengths
    }

    pub fn read_bandwidth(&self) -> &[(u64, u64)] {
        &self.read_bandwidth
    }

    pub fn write_bandwidth(&self) -> &[(u64, u64)] {
        &self.write_bandwidth
    }

    pub fn read_latencies(&self) -> &[(u64, u64)] {
        &self.read_latencies
    }

    pub fn write_latencies(&self) -> &[(u64, u64)] {
        &self.write_latencies
    }

    pub fn read_to_read_times(&self) -> &[(u64, u64)] {
        &self.read_to_read_times
    }

    pub fn write_to_write_times(&self) -> &[(u64, u64)] {
        &self.write_to_write_times
    }

    pub fn request_to_request_times(&self) -> &[(u64, u64)] {
        &self.request_to_request_times
    }

    pub fn outstanding_reads(&self) -> &[(u64, u64)] {
        &self.outstanding_reads
    }

    pub fn outstanding_writes(&self) -> &[(u64, u64)] {
        &self.outstanding_writes
    }

    pub fn read_transactions(&self) -> &[(u64, u64)] {
        &self.read_transactions
    }

    pub fn write_transactions(&self) -> &[(u64, u64)] {
        &self.write_transactions
    }

    pub fn read_addresses(&self) -> &[(u64, u64)] {
        &self.read_addresses
    }

    pub fn write_addresses(&self) -> &[(u64, u64)] {
        &self.write_addresses
    }

    fn validate(&self) -> Result<(), StatsError> {
        validate_histogram("read_burst_lengths", &self.read_burst_lengths)?;
        validate_histogram("write_burst_lengths", &self.write_burst_lengths)?;
        validate_histogram("read_bandwidth", &self.read_bandwidth)?;
        validate_histogram("write_bandwidth", &self.write_bandwidth)?;
        validate_histogram("read_latencies", &self.read_latencies)?;
        validate_histogram("write_latencies", &self.write_latencies)?;
        validate_histogram("read_to_read_times", &self.read_to_read_times)?;
        validate_histogram("write_to_write_times", &self.write_to_write_times)?;
        validate_histogram("request_to_request_times", &self.request_to_request_times)?;
        validate_histogram("outstanding_reads", &self.outstanding_reads)?;
        validate_histogram("outstanding_writes", &self.outstanding_writes)?;
        validate_histogram("read_transactions", &self.read_transactions)?;
        validate_histogram("write_transactions", &self.write_transactions)?;
        validate_histogram("read_addresses", &self.read_addresses)?;
        validate_histogram("write_addresses", &self.write_addresses)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommMonitorSnapshot {
    config: CommMonitorConfig,
    last_sample_tick: Tick,
    pending: Vec<CommMonitorPendingRequest>,
    stats: CommMonitorStats,
    histograms: CommMonitorHistograms,
    time_of_last_read: Tick,
    time_of_last_write: Tick,
    time_of_last_request: Tick,
}

impl CommMonitorSnapshot {
    pub const fn new(
        config: CommMonitorConfig,
        last_sample_tick: Tick,
        pending: Vec<CommMonitorPendingRequest>,
        stats: CommMonitorStats,
        histograms: CommMonitorHistograms,
    ) -> Self {
        Self::with_last_ticks(
            config,
            last_sample_tick,
            pending,
            stats,
            histograms,
            0,
            0,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    const fn with_last_ticks(
        config: CommMonitorConfig,
        last_sample_tick: Tick,
        pending: Vec<CommMonitorPendingRequest>,
        stats: CommMonitorStats,
        histograms: CommMonitorHistograms,
        time_of_last_read: Tick,
        time_of_last_write: Tick,
        time_of_last_request: Tick,
    ) -> Self {
        Self {
            config,
            last_sample_tick,
            pending,
            stats,
            histograms,
            time_of_last_read,
            time_of_last_write,
            time_of_last_request,
        }
    }

    pub const fn config(&self) -> &CommMonitorConfig {
        &self.config
    }

    pub const fn last_sample_tick(&self) -> Tick {
        self.last_sample_tick
    }

    pub fn pending(&self) -> &[CommMonitorPendingRequest] {
        &self.pending
    }

    pub const fn stats(&self) -> CommMonitorStats {
        self.stats
    }

    pub const fn histograms(&self) -> &CommMonitorHistograms {
        &self.histograms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommMonitor {
    config: CommMonitorConfig,
    last_sample_tick: Tick,
    pending: Vec<CommMonitorPendingRequest>,
    stats: CommMonitorStats,
    histograms: CommMonitorHistograms,
    time_of_last_read: Tick,
    time_of_last_write: Tick,
    time_of_last_request: Tick,
}

impl CommMonitor {
    pub fn new(config: CommMonitorConfig) -> Self {
        Self {
            config,
            last_sample_tick: 0,
            pending: Vec::new(),
            stats: CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0),
            histograms: CommMonitorHistograms::default(),
            time_of_last_read: 0,
            time_of_last_write: 0,
            time_of_last_request: 0,
        }
    }

    pub fn from_snapshot(snapshot: &CommMonitorSnapshot) -> Result<Self, StatsError> {
        validate_pending_requests(snapshot.pending())?;
        validate_snapshot_outstanding(snapshot.config(), snapshot.pending(), snapshot.stats())?;
        snapshot.histograms().validate()?;
        Ok(Self {
            config: snapshot.config().clone(),
            last_sample_tick: snapshot.last_sample_tick(),
            pending: snapshot.pending().to_vec(),
            stats: snapshot.stats(),
            histograms: snapshot.histograms().clone(),
            time_of_last_read: snapshot.time_of_last_read,
            time_of_last_write: snapshot.time_of_last_write,
            time_of_last_request: snapshot.time_of_last_request,
        })
    }

    pub const fn config(&self) -> &CommMonitorConfig {
        &self.config
    }

    pub const fn stats(&self) -> CommMonitorStats {
        self.stats
    }

    pub const fn histograms(&self) -> &CommMonitorHistograms {
        &self.histograms
    }

    pub fn pending(&self) -> &[CommMonitorPendingRequest] {
        &self.pending
    }

    pub fn observe_timing_request(
        &mut self,
        tick: Tick,
        packet: &MemProbePacket,
        expects_response: bool,
    ) -> Result<Option<CommMonitorPendingRequest>, StatsError> {
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

        let tracks_pending = expects_response && self.should_track_pending();
        if tracks_pending && self.has_pending_packet(packet.packet_id()) {
            return Err(StatsError::DuplicateCommMonitorPendingRequest {
                packet_id: packet.packet_id(),
            });
        }
        self.validate_request_order(tick, access)?;

        self.update_request_stats(tick, packet, access)?;
        if !tracks_pending {
            return Ok(None);
        }

        let pending =
            CommMonitorPendingRequest::new(packet.packet_id(), access, tick, packet.size());
        self.insert_pending(pending)?;
        Ok(Some(pending))
    }

    pub fn observe_timing_response(
        &mut self,
        tick: Tick,
        packet: &MemProbePacket,
    ) -> Result<Option<Tick>, StatsError> {
        if packet.kind() != MemProbePacketKind::Response {
            return Ok(None);
        }
        let access = packet.access();
        if !matches!(
            access,
            MemProbePacketAccess::Read | MemProbePacketAccess::Write
        ) {
            return Ok(None);
        }
        if self.pending.is_empty() && self.no_response_stats_enabled() {
            return Ok(None);
        }

        let Some(index) = self
            .pending
            .iter()
            .position(|pending| pending.packet_id() == packet.packet_id())
        else {
            return Err(StatsError::UnknownCommMonitorPendingRequest {
                packet_id: packet.packet_id(),
            });
        };
        let pending = self.pending[index];
        if pending.access() != access {
            return Err(StatsError::CommMonitorResponseAccessMismatch {
                packet_id: packet.packet_id(),
                request_access: pending.access(),
                response_access: access,
            });
        }
        if tick < pending.request_tick() {
            return Err(StatsError::CommMonitorResponseTimeWentBack {
                packet_id: packet.packet_id(),
                request_tick: pending.request_tick(),
                response_tick: tick,
            });
        }
        self.pending.remove(index);

        let latency = tick - pending.request_tick();
        self.update_response_stats(packet, access, latency)?;
        Ok(Some(latency))
    }

    pub fn observe_request_probe_event(
        &mut self,
        event: &ProbeEvent,
        request_point: ProbePointId,
        expects_response: bool,
    ) -> Result<Option<CommMonitorPendingRequest>, StatsError> {
        if event.point() != request_point {
            return Ok(None);
        }
        let ProbePayload::MemoryPacket(packet) = event.payload() else {
            return Ok(None);
        };
        self.observe_timing_request(event.tick(), packet, expects_response)
    }

    pub fn observe_response_probe_event(
        &mut self,
        event: &ProbeEvent,
        response_point: ProbePointId,
    ) -> Result<Option<Tick>, StatsError> {
        if event.point() != response_point {
            return Ok(None);
        }
        let ProbePayload::MemoryPacket(packet) = event.payload() else {
            return Ok(None);
        };
        self.observe_timing_response(event.tick(), packet)
    }

    pub fn sample_periodic(&mut self, tick: Tick) -> Result<Option<CommMonitorStats>, StatsError> {
        let next_sample_tick = self
            .last_sample_tick
            .checked_add(self.config.sample_period())
            .ok_or(StatsError::CommMonitorSamplePeriodOverflow {
                last_sample_tick: self.last_sample_tick,
                sample_period: self.config.sample_period(),
            })?;
        if tick < next_sample_tick {
            return Ok(None);
        }

        let sampled = self.stats();
        if !self.config.disable_transaction_hists() {
            observe_bucket(
                &mut self.histograms.read_transactions,
                sampled.read_transactions(),
            )?;
            observe_bucket(
                &mut self.histograms.write_transactions,
                sampled.write_transactions(),
            )?;
        }
        if !self.config.disable_bandwidth_hists() {
            observe_bucket(
                &mut self.histograms.read_bandwidth,
                bytes_per_second(
                    sampled.read_bytes(),
                    self.config.sample_period(),
                    self.config.tick_frequency(),
                    "read_bandwidth",
                )?,
            )?;
            observe_bucket(
                &mut self.histograms.write_bandwidth,
                bytes_per_second(
                    sampled.written_bytes(),
                    self.config.sample_period(),
                    self.config.tick_frequency(),
                    "write_bandwidth",
                )?,
            )?;
        }
        if !self.config.disable_outstanding_hists() {
            observe_bucket(
                &mut self.histograms.outstanding_reads,
                sampled.outstanding_reads(),
            )?;
            observe_bucket(
                &mut self.histograms.outstanding_writes,
                sampled.outstanding_writes(),
            )?;
        }

        self.stats.reset_window();
        self.last_sample_tick = tick;
        Ok(Some(sampled))
    }

    pub fn snapshot(&self) -> CommMonitorSnapshot {
        CommMonitorSnapshot::with_last_ticks(
            self.config.clone(),
            self.last_sample_tick,
            self.pending.clone(),
            self.stats,
            self.histograms.clone(),
            self.time_of_last_read,
            self.time_of_last_write,
            self.time_of_last_request,
        )
    }

    fn update_request_stats(
        &mut self,
        tick: Tick,
        packet: &MemProbePacket,
        access: MemProbePacketAccess,
    ) -> Result<(), StatsError> {
        match access {
            MemProbePacketAccess::Read => {
                if !self.config.disable_transaction_hists() {
                    self.stats.read_transactions =
                        checked_add(self.stats.read_transactions, 1, "read_transactions")?;
                }
                if !self.config.disable_burst_length_hists() {
                    observe_bucket(&mut self.histograms.read_burst_lengths, packet.size())?;
                }
                if !self.config.disable_addr_dists() {
                    observe_bucket(
                        &mut self.histograms.read_addresses,
                        packet.address() & self.config.read_addr_mask(),
                    )?;
                }
                self.update_read_itt(tick)?;
            }
            MemProbePacketAccess::Write => {
                if !self.config.disable_transaction_hists() {
                    self.stats.write_transactions =
                        checked_add(self.stats.write_transactions, 1, "write_transactions")?;
                }
                if !self.config.disable_burst_length_hists() {
                    observe_bucket(&mut self.histograms.write_burst_lengths, packet.size())?;
                }
                if !self.config.disable_bandwidth_hists() {
                    self.stats.written_bytes =
                        checked_add(self.stats.written_bytes, packet.size(), "written_bytes")?;
                    self.stats.total_written_bytes = checked_add(
                        self.stats.total_written_bytes,
                        packet.size(),
                        "total_written_bytes",
                    )?;
                }
                if !self.config.disable_addr_dists() {
                    observe_bucket(
                        &mut self.histograms.write_addresses,
                        packet.address() & self.config.write_addr_mask(),
                    )?;
                }
                self.update_write_itt(tick)?;
            }
            MemProbePacketAccess::Other => {}
        }
        Ok(())
    }

    fn update_response_stats(
        &mut self,
        packet: &MemProbePacket,
        access: MemProbePacketAccess,
        latency: Tick,
    ) -> Result<(), StatsError> {
        match access {
            MemProbePacketAccess::Read => {
                if !self.config.disable_outstanding_hists() && self.stats.outstanding_reads > 0 {
                    self.stats.outstanding_reads -= 1;
                }
                if !self.config.disable_latency_hists() {
                    observe_bucket(&mut self.histograms.read_latencies, latency)?;
                }
                if !self.config.disable_bandwidth_hists() {
                    self.stats.read_bytes =
                        checked_add(self.stats.read_bytes, packet.size(), "read_bytes")?;
                    self.stats.total_read_bytes = checked_add(
                        self.stats.total_read_bytes,
                        packet.size(),
                        "total_read_bytes",
                    )?;
                }
            }
            MemProbePacketAccess::Write => {
                if !self.config.disable_outstanding_hists() && self.stats.outstanding_writes > 0 {
                    self.stats.outstanding_writes -= 1;
                }
                if !self.config.disable_latency_hists() {
                    observe_bucket(&mut self.histograms.write_latencies, latency)?;
                }
            }
            MemProbePacketAccess::Other => {}
        }
        Ok(())
    }

    fn update_read_itt(&mut self, tick: Tick) -> Result<(), StatsError> {
        if self.config.disable_itt_dists() {
            return Ok(());
        }
        validate_request_time(self.time_of_last_read, tick)?;
        validate_request_time(self.time_of_last_request, tick)?;
        if self.time_of_last_read != 0 {
            observe_bucket(
                &mut self.histograms.read_to_read_times,
                tick - self.time_of_last_read,
            )?;
        }
        self.time_of_last_read = tick;
        self.update_request_to_request_itt(tick)
    }

    fn update_write_itt(&mut self, tick: Tick) -> Result<(), StatsError> {
        if self.config.disable_itt_dists() {
            return Ok(());
        }
        validate_request_time(self.time_of_last_write, tick)?;
        validate_request_time(self.time_of_last_request, tick)?;
        if self.time_of_last_write != 0 {
            observe_bucket(
                &mut self.histograms.write_to_write_times,
                tick - self.time_of_last_write,
            )?;
        }
        self.time_of_last_write = tick;
        self.update_request_to_request_itt(tick)
    }

    fn update_request_to_request_itt(&mut self, tick: Tick) -> Result<(), StatsError> {
        if self.time_of_last_request != 0 {
            observe_bucket(
                &mut self.histograms.request_to_request_times,
                tick - self.time_of_last_request,
            )?;
        }
        self.time_of_last_request = tick;
        Ok(())
    }

    fn insert_pending(&mut self, pending: CommMonitorPendingRequest) -> Result<(), StatsError> {
        if self.has_pending_packet(pending.packet_id()) {
            return Err(StatsError::DuplicateCommMonitorPendingRequest {
                packet_id: pending.packet_id(),
            });
        }
        match pending.access() {
            MemProbePacketAccess::Read if !self.config.disable_outstanding_hists() => {
                self.stats.outstanding_reads =
                    checked_add(self.stats.outstanding_reads, 1, "outstanding_reads")?;
            }
            MemProbePacketAccess::Write if !self.config.disable_outstanding_hists() => {
                self.stats.outstanding_writes =
                    checked_add(self.stats.outstanding_writes, 1, "outstanding_writes")?;
            }
            _ => {}
        }
        self.pending.push(pending);
        Ok(())
    }

    fn validate_request_order(
        &self,
        tick: Tick,
        access: MemProbePacketAccess,
    ) -> Result<(), StatsError> {
        if self.config.disable_itt_dists() {
            return Ok(());
        }
        match access {
            MemProbePacketAccess::Read => {
                validate_request_time(self.time_of_last_read, tick)?;
                validate_request_time(self.time_of_last_request, tick)
            }
            MemProbePacketAccess::Write => {
                validate_request_time(self.time_of_last_write, tick)?;
                validate_request_time(self.time_of_last_request, tick)
            }
            MemProbePacketAccess::Other => Ok(()),
        }
    }

    fn has_pending_packet(&self, packet_id: u64) -> bool {
        self.pending
            .iter()
            .any(|existing| existing.packet_id() == packet_id)
    }

    fn should_track_pending(&self) -> bool {
        !self.config.disable_latency_hists()
            || !self.config.disable_outstanding_hists()
            || !self.config.disable_bandwidth_hists()
    }

    fn no_response_stats_enabled(&self) -> bool {
        self.config.disable_latency_hists()
            && self.config.disable_outstanding_hists()
            && self.config.disable_bandwidth_hists()
    }
}

fn validate_bins(name: &'static str, bins: usize) -> Result<(), StatsError> {
    if bins == 0 {
        return Err(StatsError::InvalidCommMonitorHistogramBins { name, bins });
    }
    Ok(())
}

fn validate_request_time(previous_tick: Tick, current_tick: Tick) -> Result<(), StatsError> {
    if previous_tick != 0 && current_tick < previous_tick {
        return Err(StatsError::CommMonitorRequestTimeWentBack {
            previous_tick,
            current_tick,
        });
    }
    Ok(())
}

fn validate_pending_requests(pending: &[CommMonitorPendingRequest]) -> Result<(), StatsError> {
    let mut seen = BTreeSet::new();
    for request in pending {
        if !matches!(
            request.access(),
            MemProbePacketAccess::Read | MemProbePacketAccess::Write
        ) {
            return Err(StatsError::InvalidCommMonitorPendingAccess {
                packet_id: request.packet_id(),
                access: request.access(),
            });
        }
        if !seen.insert(request.packet_id()) {
            return Err(StatsError::DuplicateCommMonitorPendingRequest {
                packet_id: request.packet_id(),
            });
        }
    }
    Ok(())
}

fn validate_snapshot_outstanding(
    config: &CommMonitorConfig,
    pending: &[CommMonitorPendingRequest],
    stats: CommMonitorStats,
) -> Result<(), StatsError> {
    if config.disable_outstanding_hists() {
        return Ok(());
    }

    let mut reads = 0;
    let mut writes = 0;
    for request in pending {
        match request.access() {
            MemProbePacketAccess::Read => reads = checked_add(reads, 1, "pending_reads")?,
            MemProbePacketAccess::Write => writes = checked_add(writes, 1, "pending_writes")?,
            MemProbePacketAccess::Other => {}
        }
    }

    if stats.outstanding_reads() != reads {
        return Err(StatsError::CommMonitorSnapshotOutstandingMismatch {
            access: MemProbePacketAccess::Read,
            expected: reads,
            observed: stats.outstanding_reads(),
        });
    }
    if stats.outstanding_writes() != writes {
        return Err(StatsError::CommMonitorSnapshotOutstandingMismatch {
            access: MemProbePacketAccess::Write,
            expected: writes,
            observed: stats.outstanding_writes(),
        });
    }
    Ok(())
}

fn validate_histogram(name: &'static str, buckets: &[(u64, u64)]) -> Result<(), StatsError> {
    let mut seen = BTreeSet::new();
    for (bucket, _) in buckets {
        if !seen.insert(*bucket) {
            return Err(StatsError::DuplicateCommMonitorHistogramBucket {
                name,
                bucket: *bucket,
            });
        }
    }
    Ok(())
}

fn observe_bucket(buckets: &mut Vec<(u64, u64)>, bucket: u64) -> Result<(), StatsError> {
    if let Some((_, count)) = buckets.iter_mut().find(|(existing, _)| *existing == bucket) {
        *count = checked_add(*count, 1, "histogram_bucket")?;
        return Ok(());
    }

    buckets.push((bucket, 1));
    buckets.sort_unstable_by_key(|(bucket, _)| *bucket);
    Ok(())
}

fn bytes_per_second(
    bytes: u64,
    sample_period: Tick,
    tick_frequency: u64,
    name: &'static str,
) -> Result<u64, StatsError> {
    bytes
        .checked_mul(tick_frequency)
        .ok_or(StatsError::CommMonitorCounterOverflow { name })
        .map(|scaled| scaled / sample_period)
}

fn checked_add(left: u64, right: u64, name: &'static str) -> Result<u64, StatsError> {
    left.checked_add(right)
        .ok_or(StatsError::CommMonitorCounterOverflow { name })
}
