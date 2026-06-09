use rem6_stats::{
    CommMonitor, CommMonitorConfig, CommMonitorHistograms, CommMonitorPendingRequest,
    CommMonitorSnapshot, CommMonitorStats, MemProbePacket, MemProbePacketAccess,
    MemProbePacketKind, ProbePayload, ProbeRegistry, StatsError,
};

fn read(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::read(address)
        .with_packet_id(id)
        .with_size(size)
}

fn write(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::write(address)
        .with_packet_id(id)
        .with_size(size)
}

fn read_response(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::new(address, MemProbePacketKind::Response)
        .with_access(MemProbePacketAccess::Read)
        .with_packet_id(id)
        .with_size(size)
}

fn write_response(id: u64, address: u64, size: u64) -> MemProbePacket {
    MemProbePacket::new(address, MemProbePacketKind::Response)
        .with_access(MemProbePacketAccess::Write)
        .with_packet_id(id)
        .with_size(size)
}

fn config() -> CommMonitorConfig {
    CommMonitorConfig::builder(100)
        .tick_frequency(1000)
        .read_addr_mask(0xff0)
        .write_addr_mask(0xff0)
        .build()
        .unwrap()
}

#[test]
fn comm_monitor_tracks_request_response_latency_and_bandwidth() {
    let mut monitor = CommMonitor::new(config());

    assert_eq!(
        monitor.stats(),
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0)
    );
    assert_eq!(
        monitor.observe_timing_request(10, &read(1, 0x1234, 16), true),
        Ok(Some(CommMonitorPendingRequest::new(
            1,
            MemProbePacketAccess::Read,
            10,
            16
        )))
    );
    assert_eq!(
        monitor.observe_timing_request(18, &write(2, 0x1298, 8), true),
        Ok(Some(CommMonitorPendingRequest::new(
            2,
            MemProbePacketAccess::Write,
            18,
            8
        )))
    );
    assert_eq!(
        monitor.stats(),
        CommMonitorStats::new(1, 1, 0, 8, 0, 8, 1, 1)
    );

    assert_eq!(
        monitor.observe_timing_response(30, &read_response(1, 0x1234, 16)),
        Ok(Some(20))
    );
    assert_eq!(
        monitor.observe_timing_response(39, &write_response(2, 0x1298, 8)),
        Ok(Some(21))
    );
    assert_eq!(
        monitor.stats(),
        CommMonitorStats::new(1, 1, 16, 8, 16, 8, 0, 0)
    );

    assert_eq!(monitor.histograms().read_burst_lengths(), &[(16, 1)]);
    assert_eq!(monitor.histograms().write_burst_lengths(), &[(8, 1)]);
    assert_eq!(monitor.histograms().read_latencies(), &[(20, 1)]);
    assert_eq!(monitor.histograms().write_latencies(), &[(21, 1)]);
    assert_eq!(monitor.histograms().request_to_request_times(), &[(8, 1)]);
    assert_eq!(monitor.histograms().read_addresses(), &[(0x230, 1)]);
    assert_eq!(monitor.histograms().write_addresses(), &[(0x290, 1)]);
}

#[test]
fn comm_monitor_tracks_inter_transaction_times_by_access_kind() {
    let mut monitor = CommMonitor::new(config());

    monitor
        .observe_timing_request(10, &read(1, 0x1000, 4), false)
        .unwrap();
    monitor
        .observe_timing_request(17, &read(2, 0x1040, 4), false)
        .unwrap();
    monitor
        .observe_timing_request(30, &write(3, 0x1080, 4), false)
        .unwrap();
    monitor
        .observe_timing_request(42, &write(4, 0x10c0, 4), false)
        .unwrap();

    assert_eq!(monitor.histograms().read_to_read_times(), &[(7, 1)]);
    assert_eq!(monitor.histograms().write_to_write_times(), &[(12, 1)]);
    assert_eq!(
        monitor.histograms().request_to_request_times(),
        &[(7, 1), (12, 1), (13, 1)]
    );
    assert_eq!(
        monitor.stats(),
        CommMonitorStats::new(2, 2, 0, 8, 0, 8, 0, 0)
    );
}

#[test]
fn comm_monitor_samples_periodic_window_counters() {
    let mut monitor = CommMonitor::new(config());

    monitor
        .observe_timing_request(10, &read(1, 0x1000, 16), true)
        .unwrap();
    monitor
        .observe_timing_request(20, &write(2, 0x1040, 8), true)
        .unwrap();
    monitor
        .observe_timing_response(40, &read_response(1, 0x1000, 16))
        .unwrap();

    assert_eq!(monitor.sample_periodic(50), Ok(None));
    assert_eq!(
        monitor.sample_periodic(100),
        Ok(Some(CommMonitorStats::new(1, 1, 16, 8, 16, 8, 0, 1)))
    );
    assert_eq!(monitor.histograms().read_transactions(), &[(1, 1)]);
    assert_eq!(monitor.histograms().write_transactions(), &[(1, 1)]);
    assert_eq!(monitor.histograms().read_bandwidth(), &[(160, 1)]);
    assert_eq!(monitor.histograms().write_bandwidth(), &[(80, 1)]);
    assert_eq!(monitor.histograms().outstanding_reads(), &[(0, 1)]);
    assert_eq!(monitor.histograms().outstanding_writes(), &[(1, 1)]);
    assert_eq!(
        monitor.stats(),
        CommMonitorStats::new(0, 0, 0, 0, 16, 8, 0, 1)
    );
}

#[test]
fn comm_monitor_ignores_non_read_write_packets_and_disabled_stats() {
    let config = CommMonitorConfig::builder(100)
        .disable_burst_length_hists(true)
        .disable_bandwidth_hists(true)
        .disable_latency_hists(true)
        .disable_itt_dists(true)
        .disable_outstanding_hists(true)
        .disable_transaction_hists(true)
        .disable_addr_dists(true)
        .build()
        .unwrap();
    let mut monitor = CommMonitor::new(config);

    let other = MemProbePacket::request(0x1000)
        .with_packet_id(1)
        .with_size(8);
    assert_eq!(monitor.observe_timing_request(10, &other, true), Ok(None));
    assert_eq!(
        monitor.observe_timing_request(12, &read(2, 0x1000, 8), true),
        Ok(None)
    );
    assert_eq!(
        monitor.observe_timing_response(20, &read_response(2, 0x1000, 8)),
        Ok(None)
    );
    assert_eq!(
        monitor.sample_periodic(100),
        Ok(Some(CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0)))
    );
    assert_eq!(monitor.histograms(), &CommMonitorHistograms::default());
}

#[test]
fn comm_monitor_consumes_matching_probe_events() {
    let mut probes = ProbeRegistry::new();
    let pkt_request = probes.register_point("bus0", "PktRequest").unwrap();
    let pkt_response = probes.register_point("bus0", "PktResponse").unwrap();
    probes.add_listener(pkt_request, "comm_monitor").unwrap();
    probes.add_listener(pkt_response, "comm_monitor").unwrap();

    let mut monitor = CommMonitor::new(config());

    let wrong_point = probes
        .emit(
            1,
            pkt_response,
            ProbePayload::MemoryPacket(read(1, 0x1000, 4)),
        )
        .unwrap();
    assert_eq!(
        monitor.observe_request_probe_event(wrong_point, pkt_request, true),
        Ok(None)
    );
    let wrong_payload = probes.emit(2, pkt_request, ProbePayload::Unit).unwrap();
    assert_eq!(
        monitor.observe_request_probe_event(wrong_payload, pkt_request, true),
        Ok(None)
    );

    let request = probes
        .emit(
            10,
            pkt_request,
            ProbePayload::MemoryPacket(read(1, 0x1000, 4)),
        )
        .unwrap();
    assert_eq!(
        monitor.observe_request_probe_event(request, pkt_request, true),
        Ok(Some(CommMonitorPendingRequest::new(
            1,
            MemProbePacketAccess::Read,
            10,
            4
        )))
    );
    assert_eq!(request.listener_refs()[0].name(), "comm_monitor");

    let response = probes
        .emit(
            17,
            pkt_response,
            ProbePayload::MemoryPacket(read_response(1, 0x1000, 4)),
        )
        .unwrap();
    assert_eq!(
        monitor.observe_response_probe_event(response, pkt_response),
        Ok(Some(7))
    );
}

#[test]
fn comm_monitor_rejects_ambiguous_runtime_state() {
    let mut monitor = CommMonitor::new(config());

    monitor
        .observe_timing_request(10, &read(1, 0x1000, 4), true)
        .unwrap();
    let before_duplicate = monitor.snapshot();
    assert_eq!(
        monitor.observe_timing_request(12, &read(1, 0x1040, 4), true),
        Err(StatsError::DuplicateCommMonitorPendingRequest { packet_id: 1 })
    );
    assert_eq!(monitor.snapshot(), before_duplicate);
    assert_eq!(
        monitor.observe_timing_response(9, &read_response(1, 0x1000, 4)),
        Err(StatsError::CommMonitorResponseTimeWentBack {
            packet_id: 1,
            request_tick: 10,
            response_tick: 9,
        })
    );
    assert_eq!(
        monitor.observe_timing_response(12, &write_response(1, 0x1000, 4)),
        Err(StatsError::CommMonitorResponseAccessMismatch {
            packet_id: 1,
            request_access: MemProbePacketAccess::Read,
            response_access: MemProbePacketAccess::Write,
        })
    );
    assert_eq!(
        monitor.observe_timing_response(12, &read_response(9, 0x1000, 4)),
        Err(StatsError::UnknownCommMonitorPendingRequest { packet_id: 9 })
    );

    let mut time_monitor = CommMonitor::new(config());
    time_monitor
        .observe_timing_request(10, &read(1, 0x1000, 4), false)
        .unwrap();
    let before_backward = time_monitor.snapshot();
    assert_eq!(
        time_monitor.observe_timing_request(9, &read(2, 0x1040, 4), false),
        Err(StatsError::CommMonitorRequestTimeWentBack {
            previous_tick: 10,
            current_tick: 9,
        })
    );
    assert_eq!(time_monitor.snapshot(), before_backward);
}

#[test]
fn comm_monitor_config_and_snapshot_reject_ambiguous_state() {
    assert_eq!(
        CommMonitorConfig::builder(0).build(),
        Err(StatsError::InvalidCommMonitorSamplePeriod { sample_period: 0 })
    );
    assert_eq!(
        CommMonitorConfig::builder(100).tick_frequency(0).build(),
        Err(StatsError::InvalidCommMonitorTickFrequency { tick_frequency: 0 })
    );
    assert_eq!(
        CommMonitorConfig::builder(100).burst_length_bins(0).build(),
        Err(StatsError::InvalidCommMonitorHistogramBins {
            name: "burst_length",
            bins: 0,
        })
    );

    let snapshot = CommMonitorSnapshot::new(
        config(),
        0,
        vec![CommMonitorPendingRequest::new(
            1,
            MemProbePacketAccess::Read,
            10,
            4,
        )],
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0),
        CommMonitorHistograms::default(),
    );
    assert_eq!(
        CommMonitor::from_snapshot(&snapshot),
        Err(StatsError::CommMonitorSnapshotOutstandingMismatch {
            access: MemProbePacketAccess::Read,
            expected: 1,
            observed: 0,
        })
    );

    let valid_snapshot = CommMonitorSnapshot::new(
        config(),
        0,
        vec![CommMonitorPendingRequest::new(
            1,
            MemProbePacketAccess::Read,
            10,
            4,
        )],
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 1, 0),
        CommMonitorHistograms::default(),
    );
    let restored = CommMonitor::from_snapshot(&valid_snapshot).unwrap();
    assert_eq!(restored.snapshot(), valid_snapshot);

    let duplicate_pending = CommMonitorSnapshot::new(
        config(),
        0,
        vec![
            CommMonitorPendingRequest::new(1, MemProbePacketAccess::Read, 10, 4),
            CommMonitorPendingRequest::new(1, MemProbePacketAccess::Write, 12, 4),
        ],
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0),
        CommMonitorHistograms::default(),
    );
    assert_eq!(
        CommMonitor::from_snapshot(&duplicate_pending),
        Err(StatsError::DuplicateCommMonitorPendingRequest { packet_id: 1 })
    );

    let duplicate_bucket = CommMonitorSnapshot::new(
        config(),
        0,
        Vec::new(),
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0),
        CommMonitorHistograms::from_buckets(
            vec![(4, 1), (4, 2)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    );

    let invalid_access_snapshot = CommMonitorSnapshot::new(
        config(),
        0,
        vec![CommMonitorPendingRequest::new(
            1,
            MemProbePacketAccess::Other,
            10,
            4,
        )],
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0),
        CommMonitorHistograms::default(),
    );
    assert_eq!(
        CommMonitor::from_snapshot(&invalid_access_snapshot),
        Err(StatsError::InvalidCommMonitorPendingAccess {
            packet_id: 1,
            access: MemProbePacketAccess::Other,
        })
    );
    assert_eq!(
        CommMonitor::from_snapshot(&duplicate_bucket),
        Err(StatsError::DuplicateCommMonitorHistogramBucket {
            name: "read_burst_lengths",
            bucket: 4,
        })
    );

    let overflow_snapshot = CommMonitorSnapshot::new(
        config(),
        u64::MAX - 10,
        Vec::new(),
        CommMonitorStats::new(0, 0, 0, 0, 0, 0, 0, 0),
        CommMonitorHistograms::default(),
    );
    let mut restored_overflow = CommMonitor::from_snapshot(&overflow_snapshot).unwrap();
    assert_eq!(
        restored_overflow.sample_periodic(u64::MAX),
        Err(StatsError::CommMonitorSamplePeriodOverflow {
            last_sample_tick: u64::MAX - 10,
            sample_period: 100,
        })
    );
}
