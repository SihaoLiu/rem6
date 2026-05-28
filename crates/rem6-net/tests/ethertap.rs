use rem6_net::{
    EthernetInterfaceRegistry, EthernetPacket, EthernetTapGateway, EthernetTapRecordKind,
    NetworkError,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn stub_frame(bytes: &[u8]) -> Vec<u8> {
    let mut framed = u32::try_from(bytes.len()).unwrap().to_be_bytes().to_vec();
    framed.extend_from_slice(bytes);
    framed
}

#[test]
fn ethernet_tap_stub_frames_real_input_into_simulated_packets() {
    let mut registry = EthernetInterfaceRegistry::new();
    let tap_interface = registry.register("tap.interface").unwrap();
    let peer_interface = registry.register("peer.interface").unwrap();
    registry.bind_pair(tap_interface, peer_interface).unwrap();
    let mut tap = EthernetTapGateway::new(tap_interface, 16).unwrap();

    let first_records = tap
        .ingest_real_stub_bytes(&mut registry, &[0x00, 0x00], 11)
        .unwrap();
    assert!(first_records.is_empty());
    assert_eq!(tap.real_buffered_bytes(), 2);

    let second_records = tap
        .ingest_real_stub_bytes(&mut registry, &[0x00, 0x03, 0xaa, 0xbb, 0xcc], 12)
        .unwrap();
    assert_eq!(second_records.len(), 1);
    let record = &second_records[0];
    assert_eq!(record.kind(), EthernetTapRecordKind::RealToSim);
    assert_eq!(record.interface(), tap_interface);
    assert_eq!(record.peer(), Some(peer_interface));
    assert_eq!(record.tick(), 12);
    assert_eq!(record.packet().payload(), &[0xaa, 0xbb, 0xcc]);
    assert_eq!(record.real_frame_bytes(), &stub_frame(&[0xaa, 0xbb, 0xcc]));
    assert!(!record.queued());
    assert_eq!(tap.real_buffered_bytes(), 0);
    assert_eq!(registry.receive_count(peer_interface).unwrap(), 1);
    assert_eq!(
        registry.last_receive_tick(peer_interface).unwrap(),
        Some(12)
    );
}

#[test]
fn ethernet_tap_queues_real_frames_when_peer_is_busy_and_retransmits_in_order() {
    let mut registry = EthernetInterfaceRegistry::new();
    let tap_interface = registry.register("tap.interface").unwrap();
    let peer_interface = registry.register("peer.interface").unwrap();
    registry.bind_pair(tap_interface, peer_interface).unwrap();
    registry.set_busy(peer_interface, true).unwrap();
    let mut tap = EthernetTapGateway::new(tap_interface, 16).unwrap();

    let records = tap
        .ingest_real_stub_bytes(&mut registry, &stub_frame(&[1, 2]), 20)
        .unwrap();
    assert!(records.is_empty());
    assert_eq!(tap.queued_real_frame_count(), 1);
    assert_eq!(registry.receive_count(peer_interface).unwrap(), 0);

    registry.set_busy(peer_interface, false).unwrap();
    let retransmitted = tap.retransmit(&mut registry, 21).unwrap().unwrap();
    assert_eq!(retransmitted.kind(), EthernetTapRecordKind::RealToSim);
    assert_eq!(retransmitted.sequence(), 0);
    assert_eq!(retransmitted.packet().payload(), &[1, 2]);
    assert_eq!(retransmitted.real_frame_bytes(), &stub_frame(&[1, 2]));
    assert!(!retransmitted.queued());
    assert_eq!(tap.queued_real_frame_count(), 0);
    assert_eq!(registry.receive_count(peer_interface).unwrap(), 1);
    assert_eq!(
        registry.last_receive_tick(peer_interface).unwrap(),
        Some(21)
    );
}

#[test]
fn ethernet_tap_receives_simulated_packets_as_length_prefixed_real_output() {
    let mut registry = EthernetInterfaceRegistry::new();
    let tap_interface = registry.register("tap.interface").unwrap();
    let peer_interface = registry.register("peer.interface").unwrap();
    registry.bind_pair(tap_interface, peer_interface).unwrap();
    let mut tap = EthernetTapGateway::new(tap_interface, 16).unwrap();

    let record = tap
        .receive_simulated(&mut registry, packet(&[0x10, 0x20, 0x30]), 30)
        .unwrap();

    assert_eq!(record.kind(), EthernetTapRecordKind::SimToReal);
    assert_eq!(record.interface(), tap_interface);
    assert_eq!(record.peer(), Some(peer_interface));
    assert_eq!(record.tick(), 30);
    assert_eq!(record.packet().payload(), &[0x10, 0x20, 0x30]);
    assert_eq!(record.real_frame_bytes(), &stub_frame(&[0x10, 0x20, 0x30]));
    assert_eq!(registry.send_done_count(peer_interface).unwrap(), 1);
    assert_eq!(
        registry.last_send_done_tick(peer_interface).unwrap(),
        Some(30)
    );
}

#[test]
fn ethernet_tap_rejects_invalid_stub_frames_without_buffering_them() {
    let mut registry = EthernetInterfaceRegistry::new();
    let tap_interface = registry.register("tap.interface").unwrap();
    let mut tap = EthernetTapGateway::new(tap_interface, 4).unwrap();

    assert!(matches!(
        EthernetTapGateway::new(tap_interface, 0),
        Err(NetworkError::InvalidEthernetTapMaxFrameBytes { max_frame_bytes: 0 })
    ));
    assert!(matches!(
        tap.ingest_real_stub_bytes(&mut registry, &[0, 0, 0, 0], 40),
        Err(NetworkError::EthernetTapEmptyFrame)
    ));
    assert_eq!(tap.real_buffered_bytes(), 0);
    assert!(matches!(
        tap.ingest_real_stub_bytes(&mut registry, &[0, 0, 0, 5], 41),
        Err(NetworkError::EthernetTapFrameTooLarge {
            frame_bytes: 5,
            max_frame_bytes: 4,
        })
    ));
    assert_eq!(tap.real_buffered_bytes(), 0);
}

#[test]
fn ethernet_tap_snapshot_restores_parser_queue_and_sequence_state() {
    let mut registry = EthernetInterfaceRegistry::new();
    let tap_interface = registry.register("tap.interface").unwrap();
    let peer_interface = registry.register("peer.interface").unwrap();
    registry.bind_pair(tap_interface, peer_interface).unwrap();
    registry.set_busy(peer_interface, true).unwrap();
    let mut tap = EthernetTapGateway::new(tap_interface, 16).unwrap();
    tap.ingest_real_stub_bytes(&mut registry, &stub_frame(&[9]), 51)
        .unwrap();
    tap.ingest_real_stub_bytes(&mut registry, &[0, 0], 52)
        .unwrap();
    let snapshot = tap.snapshot();

    registry.set_busy(peer_interface, false).unwrap();
    tap.ingest_real_stub_bytes(&mut registry, &[0, 2, 7, 8], 53)
        .unwrap();
    tap.retransmit(&mut registry, 54).unwrap();
    assert_eq!(tap.next_sequence(), 1);

    tap.restore(&snapshot).unwrap();
    assert_eq!(tap.real_buffered_bytes(), 2);
    assert_eq!(tap.queued_real_frame_count(), 1);
    assert_eq!(tap.next_sequence(), 0);

    registry.set_busy(peer_interface, false).unwrap();
    let records = tap
        .ingest_real_stub_bytes(&mut registry, &[0, 2, 7, 8], 55)
        .unwrap();
    assert!(records.is_empty());
    let retransmitted = tap.retransmit(&mut registry, 56).unwrap().unwrap();
    assert_eq!(retransmitted.sequence(), 0);
    assert_eq!(retransmitted.packet().payload(), &[9]);
    assert_eq!(tap.next_sequence(), 1);
}
