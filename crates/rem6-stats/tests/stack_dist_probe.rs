use rem6_stats::{
    MemProbePacket, MemProbePacketAccess, MemProbePacketKind, ProbePayload, ProbeRegistry,
    StackDistHistogramSet, StackDistProbe, StackDistProbeConfig, StackDistProbeSnapshot,
    StackDistProbeStats, StackDistProbeUpdate, StatsError,
};

fn config() -> StackDistProbeConfig {
    StackDistProbeConfig::new(64, 64, 16, 32).unwrap()
}

#[test]
fn stack_dist_probe_tracks_read_and_write_reuse_distance() {
    let mut probe = StackDistProbe::new(config());

    assert_eq!(probe.stats(), StackDistProbeStats::new(0, 0, 0, 0, 0));
    assert_eq!(
        probe.observe_packet(&MemProbePacket::read(0x1003)).unwrap(),
        Some(StackDistProbeUpdate::infinite(
            MemProbePacketAccess::Read,
            0x1000,
            1
        ))
    );
    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::write(0x1040))
            .unwrap(),
        Some(StackDistProbeUpdate::infinite(
            MemProbePacketAccess::Write,
            0x1040,
            2
        ))
    );
    assert_eq!(
        probe.observe_packet(&MemProbePacket::read(0x1008)).unwrap(),
        Some(StackDistProbeUpdate::finite(
            MemProbePacketAccess::Read,
            0x1000,
            1,
            0,
            true,
            true,
        ))
    );
    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::write(0x1047))
            .unwrap(),
        Some(StackDistProbeUpdate::finite(
            MemProbePacketAccess::Write,
            0x1040,
            1,
            0,
            true,
            true,
        ))
    );
    assert_eq!(
        probe.observe_packet(&MemProbePacket::read(0x1000)).unwrap(),
        Some(StackDistProbeUpdate::finite(
            MemProbePacketAccess::Read,
            0x1000,
            1,
            0,
            true,
            true,
        ))
    );

    assert_eq!(probe.stack(), &[0x1000, 0x1040]);
    assert_eq!(probe.stats(), StackDistProbeStats::new(2, 3, 2, 3, 3));
    assert_eq!(probe.histograms().read_linear(), &[(1, 2)]);
    assert_eq!(probe.histograms().write_linear(), &[(1, 1)]);
    assert_eq!(probe.histograms().read_log(), &[(0, 2)]);
    assert_eq!(probe.histograms().write_log(), &[(0, 1)]);
}

#[test]
fn stack_dist_probe_uses_one_for_zero_log_distance() {
    let mut probe = StackDistProbe::new(config());

    probe.observe_packet(&MemProbePacket::read(0x1003)).unwrap();
    assert_eq!(
        probe.observe_packet(&MemProbePacket::read(0x1004)).unwrap(),
        Some(StackDistProbeUpdate::finite(
            MemProbePacketAccess::Read,
            0x1000,
            0,
            1,
            true,
            true,
        ))
    );
    assert_eq!(probe.histograms().read_linear(), &[(0, 1)]);
    assert_eq!(probe.histograms().read_log(), &[(1, 1)]);
}

#[test]
fn stack_dist_probe_records_deeper_reuse_distance_and_log_bucket() {
    let mut probe = StackDistProbe::new(config());

    for address in [0x1000, 0x1040, 0x1080, 0x10c0, 0x1100] {
        probe
            .observe_packet(&MemProbePacket::read(address))
            .unwrap();
    }

    assert_eq!(
        probe.observe_packet(&MemProbePacket::read(0x1000)).unwrap(),
        Some(StackDistProbeUpdate::finite(
            MemProbePacketAccess::Read,
            0x1000,
            4,
            2,
            true,
            true,
        ))
    );
    assert_eq!(probe.stack(), &[0x1000, 0x1100, 0x10c0, 0x1080, 0x1040]);
    assert_eq!(probe.histograms().read_linear(), &[(4, 1)]);
    assert_eq!(probe.histograms().read_log(), &[(2, 1)]);
}

#[test]
fn stack_dist_probe_ignores_non_allocating_packets() {
    let mut probe = StackDistProbe::new(config());

    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::request(0x1000))
            .unwrap(),
        None
    );
    assert_eq!(
        probe
            .observe_packet(
                &MemProbePacket::new(0x1000, MemProbePacketKind::Response)
                    .with_access(MemProbePacketAccess::Read),
            )
            .unwrap(),
        None
    );
    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::write(0x1000))
            .unwrap(),
        Some(StackDistProbeUpdate::infinite(
            MemProbePacketAccess::Write,
            0x1000,
            1
        ))
    );
    assert_eq!(probe.stats(), StackDistProbeStats::new(1, 0, 1, 0, 0));
}

#[test]
fn stack_dist_probe_respects_disabled_histograms() {
    let config = StackDistProbeConfig::builder(64, 64)
        .linear_hist_bins(16)
        .log_hist_bins(32)
        .disable_linear_hists(true)
        .disable_log_hists(true)
        .build()
        .unwrap();
    let mut probe = StackDistProbe::new(config);

    probe.observe_packet(&MemProbePacket::read(0x1000)).unwrap();
    assert_eq!(
        probe.observe_packet(&MemProbePacket::read(0x1000)).unwrap(),
        Some(StackDistProbeUpdate::finite(
            MemProbePacketAccess::Read,
            0x1000,
            0,
            1,
            false,
            false,
        ))
    );
    assert_eq!(probe.histograms(), &StackDistHistogramSet::default());
    assert_eq!(probe.stats(), StackDistProbeStats::new(1, 1, 1, 0, 0));
}

#[test]
fn stack_dist_probe_consumes_matching_probe_events() {
    let mut probes = ProbeRegistry::new();
    let pkt_request = probes.register_point("l2", "PktRequest").unwrap();
    let pkt_response = probes.register_point("l2", "PktResponse").unwrap();
    probes.add_listener(pkt_request, "stack_dist").unwrap();
    probes.add_listener(pkt_response, "stack_dist").unwrap();

    let mut probe = StackDistProbe::new(config());
    let packet = MemProbePacket::read(0x2003);

    let wrong_point = probes
        .emit(1, pkt_response, ProbePayload::MemoryPacket(packet))
        .unwrap();
    assert_eq!(
        probe.observe_probe_event(wrong_point, pkt_request).unwrap(),
        None
    );

    let wrong_payload = probes.emit(2, pkt_request, ProbePayload::Unit).unwrap();
    assert_eq!(
        probe
            .observe_probe_event(wrong_payload, pkt_request)
            .unwrap(),
        None
    );

    let event = probes
        .emit(3, pkt_request, ProbePayload::MemoryPacket(packet))
        .unwrap();
    assert_eq!(
        probe.observe_probe_event(event, pkt_request).unwrap(),
        Some(StackDistProbeUpdate::infinite(
            MemProbePacketAccess::Read,
            0x2000,
            1
        ))
    );
    assert_eq!(event.listener_refs()[0].name(), "stack_dist");
}

#[test]
fn stack_dist_probe_config_rejects_ambiguous_geometry() {
    assert_eq!(
        StackDistProbeConfig::new(0, 64, 16, 32).unwrap_err(),
        StatsError::InvalidStackDistLineSize { line_size: 0 }
    );
    assert_eq!(
        StackDistProbeConfig::new(96, 64, 16, 32).unwrap_err(),
        StatsError::InvalidStackDistLineSize { line_size: 96 }
    );
    assert_eq!(
        StackDistProbeConfig::new(64, 128, 16, 32).unwrap_err(),
        StatsError::StackDistLineSizeSmallerThanSystem {
            line_size: 64,
            system_line_size: 128,
        }
    );
    assert_eq!(
        StackDistProbeConfig::new(64, 64, 0, 32).unwrap_err(),
        StatsError::InvalidStackDistHistogramBins {
            name: "linear",
            bins: 0,
        }
    );
    assert_eq!(
        StackDistProbeConfig::new(64, 64, 16, 0).unwrap_err(),
        StatsError::InvalidStackDistHistogramBins {
            name: "log",
            bins: 0,
        }
    );
}

#[test]
fn stack_dist_probe_snapshot_round_trips_and_rejects_ambiguous_state() {
    let mut probe = StackDistProbe::new(config());
    probe.observe_packet(&MemProbePacket::read(0x1000)).unwrap();
    probe
        .observe_packet(&MemProbePacket::write(0x1040))
        .unwrap();
    probe.observe_packet(&MemProbePacket::read(0x1000)).unwrap();

    let snapshot = probe.snapshot();
    assert_eq!(
        snapshot,
        StackDistProbeSnapshot::new(
            config(),
            vec![0x1000, 0x1040],
            2,
            1,
            StackDistHistogramSet::from_buckets(vec![(1, 1)], Vec::new(), vec![(0, 1)], Vec::new()),
        )
    );

    let restored = StackDistProbe::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    assert_eq!(
        StackDistProbe::from_snapshot(&StackDistProbeSnapshot::new(
            config(),
            vec![0x1010],
            0,
            0,
            StackDistHistogramSet::default(),
        ))
        .unwrap_err(),
        StatsError::UnalignedStackDistSnapshotAddress {
            line_size: 64,
            address: 0x1010,
        }
    );
    assert_eq!(
        StackDistProbe::from_snapshot(&StackDistProbeSnapshot::new(
            config(),
            vec![0x1000, 0x1000],
            0,
            0,
            StackDistHistogramSet::default(),
        ))
        .unwrap_err(),
        StatsError::DuplicateStackDistSnapshotAddress { address: 0x1000 }
    );
    assert_eq!(
        StackDistProbe::from_snapshot(&StackDistProbeSnapshot::new(
            config(),
            vec![0x1000],
            0,
            0,
            StackDistHistogramSet::default(),
        ))
        .unwrap_err(),
        StatsError::StackDistSnapshotStackDepthMismatch {
            stack_depth: 1,
            infinite_samples: 0,
        }
    );
    assert_eq!(
        StackDistProbe::from_snapshot(&StackDistProbeSnapshot::new(
            config(),
            Vec::new(),
            0,
            0,
            StackDistHistogramSet::from_buckets(
                vec![(0, 1), (0, 2)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        ))
        .unwrap_err(),
        StatsError::DuplicateStackDistHistogramBucket {
            name: "read_linear",
            bucket: 0,
        }
    );
    assert_eq!(
        StackDistProbe::from_snapshot(&StackDistProbeSnapshot::new(
            config(),
            Vec::new(),
            0,
            1,
            StackDistHistogramSet::from_buckets(vec![(0, 2)], Vec::new(), Vec::new(), Vec::new()),
        ))
        .unwrap_err(),
        StatsError::StackDistSnapshotSampleCountMismatch {
            expected: 1,
            observed: 2,
        }
    );
}
