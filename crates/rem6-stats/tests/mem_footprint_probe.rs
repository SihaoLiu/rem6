use rem6_stats::{
    MemFootprintAddressRange, MemFootprintGranularity, MemFootprintProbe, MemFootprintProbeConfig,
    MemFootprintProbeSnapshot, MemFootprintStats, MemProbePacket, MemProbePacketKind, ProbePayload,
    ProbeRegistry, StatsError,
};

fn range(start: u64, bytes: u64) -> MemFootprintAddressRange {
    MemFootprintAddressRange::new(start, bytes).unwrap()
}

fn config() -> MemFootprintProbeConfig {
    MemFootprintProbeConfig::new(64, 4096, vec![range(0x1000, 0x2000), range(0x8000, 0x1000)])
        .unwrap()
}

#[test]
fn mem_footprint_probe_tracks_request_footprint_at_line_and_page_granularity() {
    let mut probe = MemFootprintProbe::new(config());

    assert_eq!(probe.stats().unwrap(), MemFootprintStats::new(0, 0, 0, 0));

    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x1003, MemProbePacketKind::Request))
            .unwrap(),
        Some(MemFootprintStats::new(64, 64, 4096, 4096))
    );
    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x1010, MemProbePacketKind::Request))
            .unwrap(),
        None
    );
    assert_eq!(
        probe.stats().unwrap(),
        MemFootprintStats::new(64, 64, 4096, 4096)
    );

    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x1040, MemProbePacketKind::Request))
            .unwrap(),
        Some(MemFootprintStats::new(128, 128, 4096, 4096))
    );
    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x2000, MemProbePacketKind::Request))
            .unwrap(),
        Some(MemFootprintStats::new(192, 192, 8192, 8192))
    );
}

#[test]
fn mem_footprint_probe_ignores_responses_and_addresses_outside_memory_ranges() {
    let mut probe = MemFootprintProbe::new(config());

    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x1003, MemProbePacketKind::Response))
            .unwrap(),
        None
    );
    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x3000, MemProbePacketKind::Request))
            .unwrap(),
        None
    );
    assert_eq!(probe.stats().unwrap(), MemFootprintStats::new(0, 0, 0, 0));

    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x8000, MemProbePacketKind::Request))
            .unwrap(),
        Some(MemFootprintStats::new(64, 64, 4096, 4096))
    );
}

#[test]
fn mem_footprint_probe_reset_clears_current_footprint_only() {
    let mut probe = MemFootprintProbe::new(config());

    probe
        .observe_packet(&MemProbePacket::new(0x1003, MemProbePacketKind::Request))
        .unwrap();
    probe
        .observe_packet(&MemProbePacket::new(0x1040, MemProbePacketKind::Request))
        .unwrap();
    probe.reset_current();

    assert_eq!(
        probe.stats().unwrap(),
        MemFootprintStats::new(0, 128, 0, 4096)
    );

    assert_eq!(
        probe
            .observe_packet(&MemProbePacket::new(0x2000, MemProbePacketKind::Request))
            .unwrap(),
        Some(MemFootprintStats::new(64, 192, 4096, 8192))
    );
}

#[test]
fn mem_footprint_probe_consumes_matching_probe_events() {
    let mut probes = ProbeRegistry::new();
    let pkt_request = probes.register_point("l2", "PktRequest").unwrap();
    let pkt_response = probes.register_point("l2", "PktResponse").unwrap();
    probes.add_listener(pkt_request, "mem_footprint").unwrap();
    probes.add_listener(pkt_response, "mem_footprint").unwrap();

    let mut footprint = MemFootprintProbe::new(config());

    let wrong_point = probes
        .emit(
            1,
            pkt_response,
            ProbePayload::MemoryPacket(MemProbePacket::new(0x1003, MemProbePacketKind::Request)),
        )
        .unwrap();
    assert_eq!(
        footprint
            .observe_probe_event(wrong_point, pkt_request)
            .unwrap(),
        None
    );

    let wrong_payload = probes.emit(2, pkt_request, ProbePayload::Unit).unwrap();
    assert_eq!(
        footprint
            .observe_probe_event(wrong_payload, pkt_request)
            .unwrap(),
        None
    );

    let request = probes
        .emit(
            3,
            pkt_request,
            ProbePayload::MemoryPacket(MemProbePacket::new(0x1003, MemProbePacketKind::Request)),
        )
        .unwrap();
    assert_eq!(
        footprint.observe_probe_event(request, pkt_request).unwrap(),
        Some(MemFootprintStats::new(64, 64, 4096, 4096))
    );
    assert_eq!(request.listener_refs()[0].name(), "mem_footprint");
}

#[test]
fn mem_footprint_probe_config_rejects_ambiguous_geometry() {
    assert_eq!(
        MemFootprintProbeConfig::new(96, 4096, vec![range(0x1000, 0x1000)]).unwrap_err(),
        StatsError::InvalidMemFootprintGranularity {
            name: "cache_line".to_string(),
            bytes: 96,
        }
    );
    assert_eq!(
        MemFootprintProbeConfig::new(64, 0, vec![range(0x1000, 0x1000)]).unwrap_err(),
        StatsError::InvalidMemFootprintGranularity {
            name: "page".to_string(),
            bytes: 0,
        }
    );
    assert_eq!(
        MemFootprintProbeConfig::new(4096, 64, vec![range(0x1000, 0x1000)]).unwrap_err(),
        StatsError::MemFootprintGranularityOrder {
            cache_line_size: 4096,
            page_size: 64,
        }
    );
    assert_eq!(
        MemFootprintProbeConfig::new(64, 4096, Vec::new()).unwrap_err(),
        StatsError::EmptyMemFootprintAddressMap
    );
    assert_eq!(
        MemFootprintAddressRange::new(0x1000, 0).unwrap_err(),
        StatsError::EmptyMemFootprintAddressRange
    );
    assert_eq!(
        MemFootprintAddressRange::new(u64::MAX, 2).unwrap_err(),
        StatsError::MemFootprintAddressRangeOverflow {
            start: u64::MAX,
            bytes: 2,
        }
    );
    assert_eq!(
        MemFootprintProbeConfig::new(64, 4096, vec![range(0x1000, 0x1000), range(0x1800, 0x1000)])
            .unwrap_err(),
        StatsError::OverlappingMemFootprintAddressRange {
            existing_start: 0x1000,
            existing_end: 0x2000,
            requested_start: 0x1800,
            requested_end: 0x2800,
        }
    );
}

#[test]
fn mem_footprint_probe_snapshot_round_trips_and_rejects_ambiguous_state() {
    let mut probe = MemFootprintProbe::new(config());
    probe
        .observe_packet(&MemProbePacket::new(0x1003, MemProbePacketKind::Request))
        .unwrap();
    probe.reset_current();
    probe
        .observe_packet(&MemProbePacket::new(0x2000, MemProbePacketKind::Request))
        .unwrap();

    let snapshot = probe.snapshot();
    assert_eq!(
        snapshot,
        MemFootprintProbeSnapshot::new(
            vec![0x2000],
            vec![0x1000, 0x2000],
            vec![0x2000],
            vec![0x1000, 0x2000],
        )
    );

    let restored = MemFootprintProbe::from_snapshot(config(), &snapshot).unwrap();
    assert_eq!(
        restored.stats().unwrap(),
        MemFootprintStats::new(64, 128, 4096, 8192)
    );

    assert_eq!(
        MemFootprintProbe::from_snapshot(
            config(),
            &MemFootprintProbeSnapshot::new(vec![0x1010], vec![0x1010], Vec::new(), Vec::new(),),
        )
        .unwrap_err(),
        StatsError::UnalignedMemFootprintSnapshotAddress {
            granularity: MemFootprintGranularity::CacheLine,
            address: 0x1010,
        }
    );
    assert_eq!(
        MemFootprintProbe::from_snapshot(
            config(),
            &MemFootprintProbeSnapshot::new(vec![0x2000], vec![0x1000], Vec::new(), Vec::new(),),
        )
        .unwrap_err(),
        StatsError::MemFootprintSnapshotCurrentNotInTotal {
            granularity: MemFootprintGranularity::CacheLine,
            address: 0x2000,
        }
    );
    assert_eq!(
        MemFootprintProbe::from_snapshot(
            config(),
            &MemFootprintProbeSnapshot::new(vec![0x3000], vec![0x3000], Vec::new(), Vec::new(),),
        )
        .unwrap_err(),
        StatsError::MemFootprintSnapshotAddressOutsideMemory {
            granularity: MemFootprintGranularity::CacheLine,
            address: 0x3000,
        }
    );
    assert_eq!(
        MemFootprintProbe::from_snapshot(
            config(),
            &MemFootprintProbeSnapshot::new(vec![0x1000], vec![0x1000], Vec::new(), Vec::new(),),
        )
        .unwrap_err(),
        StatsError::MemFootprintSnapshotGranularityMismatch {
            finer: MemFootprintGranularity::CacheLine,
            coarser: MemFootprintGranularity::Page,
            address: 0x1000,
            parent: 0x1000,
        }
    );
    assert_eq!(
        MemFootprintProbe::from_snapshot(
            config(),
            &MemFootprintProbeSnapshot::new(Vec::new(), Vec::new(), vec![0x1000], vec![0x1000],),
        )
        .unwrap_err(),
        StatsError::MemFootprintSnapshotGranularityMismatch {
            finer: MemFootprintGranularity::CacheLine,
            coarser: MemFootprintGranularity::Page,
            address: 0x1000,
            parent: 0x1000,
        }
    );
    let equal_granularity_config =
        MemFootprintProbeConfig::new(64, 64, vec![range(0x1000, 0x1000)]).unwrap();
    assert_eq!(
        MemFootprintProbe::from_snapshot(
            equal_granularity_config,
            &MemFootprintProbeSnapshot::new(
                vec![0x1000],
                vec![0x1000],
                vec![0x1040],
                vec![0x1040],
            ),
        )
        .unwrap_err(),
        StatsError::MemFootprintSnapshotGranularityMismatch {
            finer: MemFootprintGranularity::CacheLine,
            coarser: MemFootprintGranularity::Page,
            address: 0x1000,
            parent: 0x1000,
        }
    );
}
