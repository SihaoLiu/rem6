use rem6_stats::{
    MemProbePacket, MemTracePacketRecord, MemTraceProbe, MemTraceProbeConfig, MemTraceProbeHeader,
    MemTraceProbeSnapshot, ProbePayload, ProbeRegistry, StatsError,
};

fn header() -> MemTraceProbeHeader {
    MemTraceProbeHeader::new(
        "l2.mem_trace",
        1_000_000_000,
        vec![(0, "cpu0".to_string()), (1, "dma0".to_string())],
    )
    .unwrap()
}

#[test]
fn mem_trace_probe_records_packet_fields_and_gates_program_counter() {
    let mut trace = MemTraceProbe::new(MemTraceProbeConfig::new(header(), true));

    assert_eq!(trace.header().object_id(), "l2.mem_trace");
    assert_eq!(trace.header().tick_frequency(), 1_000_000_000);
    assert_eq!(trace.records(), &[]);

    let first = MemProbePacket::request(0x1000)
        .with_command(7)
        .with_flags(0x21)
        .with_size(64)
        .with_packet_id(55)
        .with_program_counter(0x8000);
    assert_eq!(
        trace.observe_packet(12, &first).unwrap(),
        &MemTracePacketRecord::new(12, 7, 0x21, 0x1000, 64, Some(0x8000), 55)
    );

    let without_pc = MemProbePacket::response(0x1080)
        .with_command(9)
        .with_flags(0x2)
        .with_size(8)
        .with_packet_id(56)
        .with_program_counter(0);
    assert_eq!(
        trace.observe_packet(13, &without_pc).unwrap(),
        &MemTracePacketRecord::new(13, 9, 0x2, 0x1080, 8, None, 56)
    );

    let mut no_pc_trace = MemTraceProbe::new(MemTraceProbeConfig::new(header(), false));
    assert_eq!(
        no_pc_trace.observe_packet(14, &first).unwrap(),
        &MemTracePacketRecord::new(14, 7, 0x21, 0x1000, 64, None, 55)
    );

    assert_eq!(
        trace.observe_packet(11, &first).unwrap_err(),
        StatsError::MemTraceRecordTimeWentBack {
            previous_tick: 13,
            current_tick: 11,
        }
    );
}

#[test]
fn mem_trace_probe_consumes_matching_probe_events() {
    let mut probes = ProbeRegistry::new();
    let pkt_request = probes.register_point("l2", "PktRequest").unwrap();
    let other_point = probes.register_point("l2", "PktResponse").unwrap();
    probes.add_listener(pkt_request, "mem_trace").unwrap();
    probes.add_listener(other_point, "mem_trace").unwrap();

    let mut trace = MemTraceProbe::new(MemTraceProbeConfig::new(header(), true));
    let packet = MemProbePacket::request(0x2000)
        .with_command(4)
        .with_size(32)
        .with_packet_id(9)
        .with_program_counter(0x9000);

    let wrong_point = probes
        .emit(1, other_point, ProbePayload::MemoryPacket(packet))
        .unwrap();
    assert_eq!(
        trace.observe_probe_event(wrong_point, pkt_request).unwrap(),
        None
    );

    let wrong_payload = probes.emit(2, pkt_request, ProbePayload::Unit).unwrap();
    assert_eq!(
        trace
            .observe_probe_event(wrong_payload, pkt_request)
            .unwrap(),
        None
    );

    let event = probes
        .emit(3, pkt_request, ProbePayload::MemoryPacket(packet))
        .unwrap();
    assert_eq!(
        trace.observe_probe_event(event, pkt_request).unwrap(),
        Some(&MemTracePacketRecord::new(
            3,
            4,
            0,
            0x2000,
            32,
            Some(0x9000),
            9
        ))
    );
    assert_eq!(event.listener_refs()[0].name(), "mem_trace");

    let regressing = probes
        .emit(4, pkt_request, ProbePayload::MemoryPacket(packet))
        .unwrap();
    assert_eq!(
        trace.observe_probe_event(regressing, pkt_request).unwrap(),
        Some(&MemTracePacketRecord::new(
            4,
            4,
            0,
            0x2000,
            32,
            Some(0x9000),
            9
        ))
    );

    let regressing_event = rem6_stats::ProbeEvent::new(
        2,
        99,
        pkt_request,
        Vec::new(),
        ProbePayload::MemoryPacket(packet),
    );
    assert_eq!(
        trace
            .observe_probe_event(&regressing_event, pkt_request)
            .unwrap_err(),
        StatsError::MemTraceRecordTimeWentBack {
            previous_tick: 4,
            current_tick: 2,
        }
    );
}

#[test]
fn mem_trace_probe_header_rejects_ambiguous_requestor_metadata() {
    assert_eq!(
        MemTraceProbeHeader::new("", 1, vec![]).unwrap_err(),
        StatsError::EmptyMemTraceObjectId
    );
    assert_eq!(
        MemTraceProbeHeader::new("trace", 0, vec![]).unwrap_err(),
        StatsError::InvalidMemTraceTickFrequency { frequency: 0 }
    );
    assert_eq!(
        MemTraceProbeHeader::new(
            "trace",
            1,
            vec![(0, "cpu0".to_string()), (0, "dma0".to_string())],
        )
        .unwrap_err(),
        StatsError::DuplicateMemTraceRequestor { requestor: 0 }
    );
    assert_eq!(
        MemTraceProbeHeader::new("trace", 1, vec![(1, String::new())]).unwrap_err(),
        StatsError::EmptyMemTraceRequestorName { requestor: 1 }
    );
}

#[test]
fn mem_trace_probe_snapshot_round_trips_records_and_rejects_time_regression() {
    let mut trace = MemTraceProbe::new(MemTraceProbeConfig::new(header(), true));
    trace
        .observe_packet(
            10,
            &MemProbePacket::request(0x1000)
                .with_command(1)
                .with_size(4)
                .with_packet_id(1),
        )
        .unwrap();
    trace
        .observe_packet(
            12,
            &MemProbePacket::request(0x1008)
                .with_command(1)
                .with_size(4)
                .with_packet_id(2),
        )
        .unwrap();

    let snapshot = trace.snapshot();
    assert_eq!(
        snapshot,
        MemTraceProbeSnapshot::new(
            header(),
            true,
            vec![
                MemTracePacketRecord::new(10, 1, 0, 0x1000, 4, None, 1),
                MemTracePacketRecord::new(12, 1, 0, 0x1008, 4, None, 2),
            ],
        )
    );

    let restored = MemTraceProbe::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    assert_eq!(
        MemTraceProbe::from_snapshot(&MemTraceProbeSnapshot::new(
            header(),
            true,
            vec![
                MemTracePacketRecord::new(12, 1, 0, 0x1000, 4, None, 1),
                MemTracePacketRecord::new(10, 1, 0, 0x1008, 4, None, 2),
            ],
        ))
        .unwrap_err(),
        StatsError::MemTraceRecordTimeWentBack {
            previous_tick: 12,
            current_tick: 10,
        }
    );
    assert_eq!(
        MemTraceProbe::from_snapshot(&MemTraceProbeSnapshot::new(
            header(),
            false,
            vec![MemTracePacketRecord::new(
                12,
                1,
                0,
                0x1000,
                4,
                Some(0x8000),
                1
            ),],
        ))
        .unwrap_err(),
        StatsError::MemTraceSnapshotUnexpectedProgramCounter {
            tick: 12,
            program_counter: 0x8000,
        }
    );
    assert_eq!(
        MemTraceProbe::from_snapshot(&MemTraceProbeSnapshot::new(
            header(),
            true,
            vec![MemTracePacketRecord::new(12, 1, 0, 0x1000, 4, Some(0), 1),],
        ))
        .unwrap_err(),
        StatsError::MemTraceSnapshotZeroProgramCounter { tick: 12 }
    );
}
