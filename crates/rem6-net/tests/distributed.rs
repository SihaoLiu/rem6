use rem6_net::{
    DistributedEthernetCodec, DistributedEthernetHeader, DistributedEthernetLinkEndpoint,
    DistributedEthernetLinkTiming, DistributedEthernetMessage, DistributedEthernetMessageKind,
    DistributedEthernetReceiveScheduler, DistributedEthernetReceiveWindow,
    DistributedEthernetReqType, EthernetInterfaceEventKind, EthernetInterfaceRegistry,
    EthernetLinkDelayVariation, EthernetPacket, NetworkError,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

#[test]
fn distributed_ethernet_data_messages_preserve_timing_and_payload() {
    let mut codec = DistributedEthernetCodec::new();
    let packet = packet(&[0xde, 0xad, 0xbe, 0xef])
        .with_wire_length_bytes(64)
        .unwrap();

    let message = DistributedEthernetMessage::data(10, 7, packet.clone()).unwrap();
    let bytes = codec.encode(&message).unwrap();
    assert_eq!(bytes.len(), DistributedEthernetHeader::WIRE_BYTES + 4);
    assert_eq!(&bytes[..4], b"R6DN");
    assert_eq!(codec.record_count(), 1);

    let decoded = DistributedEthernetCodec::decode(&bytes).unwrap();
    assert_eq!(decoded.kind(), DistributedEthernetMessageKind::Data);
    assert_eq!(decoded.send_tick(), 10);
    assert_eq!(decoded.send_delay_ticks(), Some(7));
    assert_eq!(decoded.sync_repeat_ticks(), None);
    assert_eq!(decoded.sim_length_bytes(), Some(64));
    assert_eq!(decoded.packet_length_bytes(), Some(4));
    assert_eq!(decoded.packet().unwrap().payload(), packet.payload());
    assert_eq!(decoded.packet().unwrap().wire_length_bytes(), 64);

    let record = &codec.records()[0];
    assert_eq!(record.sequence(), 0);
    assert_eq!(record.kind(), DistributedEthernetMessageKind::Data);
    assert_eq!(record.wire_bytes(), bytes.len() as u64);
    assert_eq!(record.send_tick(), 10);
}

#[test]
fn distributed_ethernet_sync_messages_preserve_control_requests() {
    let mut codec = DistributedEthernetCodec::new();

    let request = DistributedEthernetMessage::sync_request(
        40,
        100,
        DistributedEthernetReqType::Collective,
        DistributedEthernetReqType::Pending,
        DistributedEthernetReqType::None,
    );
    let request_bytes = codec.encode(&request).unwrap();
    let decoded_request = DistributedEthernetCodec::decode(&request_bytes).unwrap();
    assert_eq!(
        decoded_request.kind(),
        DistributedEthernetMessageKind::SyncRequest
    );
    assert_eq!(decoded_request.send_tick(), 40);
    assert_eq!(decoded_request.sync_repeat_ticks(), Some(100));
    assert_eq!(
        decoded_request.need_checkpoint(),
        Some(DistributedEthernetReqType::Collective)
    );
    assert_eq!(
        decoded_request.need_stop_sync(),
        Some(DistributedEthernetReqType::Pending)
    );
    assert_eq!(
        decoded_request.need_exit(),
        Some(DistributedEthernetReqType::None)
    );
    assert!(decoded_request.packet().is_none());

    let ack = DistributedEthernetMessage::sync_ack(
        44,
        90,
        DistributedEthernetReqType::Immediate,
        DistributedEthernetReqType::None,
        DistributedEthernetReqType::Immediate,
    );
    let ack_bytes = codec.encode(&ack).unwrap();
    let decoded_ack = DistributedEthernetCodec::decode(&ack_bytes).unwrap();
    assert_eq!(decoded_ack.kind(), DistributedEthernetMessageKind::SyncAck);
    assert_eq!(decoded_ack.send_tick(), 44);
    assert_eq!(decoded_ack.sync_repeat_ticks(), Some(90));
    assert_eq!(
        decoded_ack.need_checkpoint(),
        Some(DistributedEthernetReqType::Immediate)
    );
    assert_eq!(
        decoded_ack.need_exit(),
        Some(DistributedEthernetReqType::Immediate)
    );
    assert_eq!(codec.record_count(), 2);
}

#[test]
fn distributed_ethernet_decode_rejects_short_unknown_and_mismatched_payloads() {
    assert!(matches!(
        DistributedEthernetCodec::decode(&[0; 8]),
        Err(NetworkError::DistributedEthernetHeaderTooShort {
            bytes: 8,
            header_bytes: DistributedEthernetHeader::WIRE_BYTES,
        })
    ));

    let mut bad_magic = vec![0; DistributedEthernetHeader::WIRE_BYTES];
    bad_magic[..4].copy_from_slice(b"BAD!");
    assert!(matches!(
        DistributedEthernetCodec::decode(&bad_magic),
        Err(NetworkError::DistributedEthernetBadMagic { magic }) if magic == *b"BAD!"
    ));

    let sync = DistributedEthernetMessage::sync_request(
        1,
        2,
        DistributedEthernetReqType::None,
        DistributedEthernetReqType::None,
        DistributedEthernetReqType::None,
    );
    let mut unknown_type = DistributedEthernetCodec::encode_one(&sync).unwrap();
    unknown_type[4] = 99;
    assert!(matches!(
        DistributedEthernetCodec::decode(&unknown_type),
        Err(NetworkError::UnknownDistributedEthernetMessageKind { kind: 99 })
    ));

    let data = DistributedEthernetMessage::data(3, 4, packet(&[1, 2, 3])).unwrap();
    let mut truncated = DistributedEthernetCodec::encode_one(&data).unwrap();
    truncated.pop();
    assert!(matches!(
        DistributedEthernetCodec::decode(&truncated),
        Err(NetworkError::DistributedEthernetPayloadLengthMismatch {
            expected_bytes: 3,
            actual_bytes: 2,
        })
    ));
}

#[test]
fn distributed_ethernet_codec_snapshot_restores_sequence_and_records() {
    let mut codec = DistributedEthernetCodec::new();
    codec
        .encode(&DistributedEthernetMessage::data(5, 6, packet(&[1])).unwrap())
        .unwrap();
    let snapshot = codec.snapshot();

    codec
        .encode(&DistributedEthernetMessage::sync_ack(
            7,
            8,
            DistributedEthernetReqType::None,
            DistributedEthernetReqType::None,
            DistributedEthernetReqType::Immediate,
        ))
        .unwrap();
    assert_eq!(codec.next_sequence(), 2);

    codec.restore(&snapshot).unwrap();
    assert_eq!(codec.next_sequence(), 1);
    assert_eq!(codec.record_count(), 1);
    let bytes = codec
        .encode(&DistributedEthernetMessage::sync_request(
            9,
            10,
            DistributedEthernetReqType::Immediate,
            DistributedEthernetReqType::None,
            DistributedEthernetReqType::None,
        ))
        .unwrap();
    assert_eq!(codec.records()[1].sequence(), 1);
    assert_eq!(
        DistributedEthernetCodec::decode(&bytes)
            .unwrap()
            .need_checkpoint(),
        Some(DistributedEthernetReqType::Immediate)
    );
}

#[test]
fn distributed_ethernet_receive_scheduler_orders_packets_by_typed_ticks() {
    let mut scheduler = DistributedEthernetReceiveScheduler::new(3);
    let window = DistributedEthernetReceiveWindow::new(9, 17).unwrap();

    let first = scheduler
        .push_data(
            DistributedEthernetMessage::data(10, 5, packet(&[1, 2])).unwrap(),
            11,
            Some(window),
        )
        .unwrap();
    assert_eq!(first.send_tick(), 10);
    assert_eq!(first.send_delay_ticks(), 5);
    assert_eq!(first.receive_tick(), 18);
    assert_eq!(scheduler.next_receive_tick(), Some(18));

    let second = scheduler
        .push_data(
            DistributedEthernetMessage::data(16, 4, packet(&[3])).unwrap(),
            12,
            Some(window),
        )
        .unwrap();
    assert_eq!(second.receive_tick(), 23);
    assert_eq!(scheduler.pending_count(), 2);

    assert!(scheduler.pop_ready(17).unwrap().is_none());
    let first_delivery = scheduler.pop_ready(18).unwrap().unwrap();
    assert_eq!(first_delivery.delivery_tick(), 18);
    assert_eq!(first_delivery.packet().payload(), &[1, 2]);
    assert_eq!(scheduler.previous_receive_tick(), 18);
    assert_eq!(scheduler.next_receive_tick(), Some(23));

    let second_delivery = scheduler.pop_ready(23).unwrap().unwrap();
    assert_eq!(second_delivery.delivery_tick(), 23);
    assert_eq!(second_delivery.packet().payload(), &[3]);
    assert_eq!(scheduler.pending_count(), 0);
}

#[test]
fn distributed_ethernet_receive_scheduler_rejects_bad_windows_and_messages() {
    let mut scheduler = DistributedEthernetReceiveScheduler::new(3);
    let window = DistributedEthernetReceiveWindow::new(9, 17).unwrap();

    assert!(matches!(
        DistributedEthernetReceiveWindow::new(10, 10),
        Err(NetworkError::InvalidDistributedEthernetReceiveWindow {
            previous_sync_tick: 10,
            next_sync_tick: 10,
        })
    ));
    assert!(matches!(
        scheduler.push_data(
            DistributedEthernetMessage::sync_ack(
                10,
                20,
                DistributedEthernetReqType::None,
                DistributedEthernetReqType::None,
                DistributedEthernetReqType::None,
            ),
            9,
            Some(window),
        ),
        Err(NetworkError::DistributedEthernetReceiveMessageNotData {
            kind: DistributedEthernetMessageKind::SyncAck,
        })
    ));
    assert!(matches!(
        scheduler.push_data(
            DistributedEthernetMessage::data(9, 5, packet(&[1])).unwrap(),
            10,
            Some(window),
        ),
        Err(NetworkError::DistributedEthernetSendOutsideReceiveWindow {
            send_tick: 9,
            previous_sync_tick: 9,
        })
    ));
    assert!(matches!(
        scheduler.push_data(
            DistributedEthernetMessage::data(10, 2, packet(&[1])).unwrap(),
            10,
            Some(DistributedEthernetReceiveWindow::new(9, 20).unwrap()),
        ),
        Err(NetworkError::DistributedEthernetReceiveInsideSyncWindow {
            receive_tick: 15,
            next_sync_tick: 20,
        })
    ));
}

#[test]
fn distributed_ethernet_receive_scheduler_rejects_missed_and_out_of_order_packets() {
    let mut scheduler = DistributedEthernetReceiveScheduler::new(3);

    assert!(matches!(
        scheduler.push_data(
            DistributedEthernetMessage::data(10, 2, packet(&[1])).unwrap(),
            15,
            None,
        ),
        Err(NetworkError::DistributedEthernetReceiveMissed {
            current_tick: 15,
            receive_tick: 15,
        })
    ));

    scheduler
        .push_data(
            DistributedEthernetMessage::data(20, 20, packet(&[2])).unwrap(),
            10,
            None,
        )
        .unwrap();
    assert!(matches!(
        scheduler.push_data(
            DistributedEthernetMessage::data(21, 5, packet(&[3])).unwrap(),
            10,
            None,
        ),
        Err(NetworkError::DistributedEthernetReceiveOutOfOrder {
            queued_ready_tick: 40,
            receive_tick: 29,
        })
    ));

    let mut scheduler = DistributedEthernetReceiveScheduler::new(3);
    scheduler
        .push_data(
            DistributedEthernetMessage::data(10, 5, packet(&[4])).unwrap(),
            9,
            None,
        )
        .unwrap();
    scheduler.pop_ready(18).unwrap().unwrap();
    assert!(matches!(
        scheduler.push_data(
            DistributedEthernetMessage::data(12, 10, packet(&[5])).unwrap(),
            19,
            None,
        ),
        Err(NetworkError::DistributedEthernetReceiveWindowTooSmall {
            previous_receive_tick: 18,
            send_delay_ticks: 10,
            receive_tick: 25,
        })
    ));
}

#[test]
fn distributed_ethernet_receive_scheduler_snapshot_restore_resumes_pending_packets() {
    let mut scheduler = DistributedEthernetReceiveScheduler::new(2);
    scheduler
        .push_data(
            DistributedEthernetMessage::data(
                30,
                7,
                packet(&[0xaa]).with_wire_length_bytes(9).unwrap(),
            )
            .unwrap(),
            20,
            None,
        )
        .unwrap();
    let snapshot = scheduler.snapshot();

    scheduler
        .push_data(
            DistributedEthernetMessage::data(40, 2, packet(&[0xbb])).unwrap(),
            21,
            None,
        )
        .unwrap();
    assert_eq!(scheduler.pending_count(), 2);

    scheduler.restore(&snapshot).unwrap();
    assert_eq!(scheduler.pending_count(), 1);
    assert_eq!(scheduler.next_receive_tick(), Some(39));

    let resumed = scheduler.resume_after_restore(100).unwrap();
    assert_eq!(resumed, 1);
    assert_eq!(scheduler.next_receive_tick(), Some(100));
    let delivery = scheduler.pop_ready(100).unwrap().unwrap();
    assert_eq!(delivery.delivery_tick(), 100);
    assert_eq!(delivery.send_tick(), 100);
    assert_eq!(delivery.send_delay_ticks(), 9);
    assert_eq!(delivery.packet().payload(), &[0xaa]);
    assert_eq!(scheduler.pending_count(), 0);
}

#[test]
fn distributed_ethernet_link_endpoint_transmits_typed_messages_and_send_done() {
    let mut registry = EthernetInterfaceRegistry::new();
    let endpoint_interface = registry.register("dist-endpoint").unwrap();
    let peer = registry.register("local-peer").unwrap();
    registry.bind_pair(endpoint_interface, peer).unwrap();

    let timing = DistributedEthernetLinkTiming::new(2, 5).unwrap();
    let mut endpoint = DistributedEthernetLinkEndpoint::new(endpoint_interface, timing);
    let outbound = packet(&[0xde, 0xad]).with_wire_length_bytes(4).unwrap();

    let transmission = endpoint
        .transmit(&mut registry, outbound.clone(), 10)
        .unwrap();
    assert_eq!(transmission.sequence(), 0);
    assert_eq!(transmission.interface(), endpoint_interface);
    assert_eq!(transmission.request_tick(), 10);
    assert_eq!(transmission.send_delay_ticks(), 9);
    assert_eq!(transmission.delay_variation_ticks(), 0);
    assert_eq!(transmission.transmit_done_tick(), 19);
    assert_eq!(endpoint.codec_record_count(), 1);
    assert_eq!(endpoint.busy_until_tick(), Some(19));
    assert!(registry.is_busy(endpoint_interface).unwrap());
    assert!(registry.ask_busy(peer).unwrap());

    let decoded = DistributedEthernetCodec::decode(transmission.encoded_message()).unwrap();
    assert_eq!(decoded.kind(), DistributedEthernetMessageKind::Data);
    assert_eq!(decoded.send_tick(), 10);
    assert_eq!(decoded.send_delay_ticks(), Some(9));
    assert_eq!(decoded.sim_length_bytes(), Some(4));
    assert_eq!(decoded.packet().unwrap().payload(), outbound.payload());

    assert!(endpoint
        .drain_transmit_done(&mut registry, 18)
        .unwrap()
        .is_none());
    let done = endpoint
        .drain_transmit_done(&mut registry, 19)
        .unwrap()
        .unwrap();
    assert_eq!(done.transmission().sequence(), 0);
    assert_eq!(done.event().interface(), peer);
    assert_eq!(done.event().peer(), endpoint_interface);
    assert_eq!(done.event().tick(), 19);
    assert_eq!(done.event().kind(), EthernetInterfaceEventKind::SendDone);
    assert_eq!(registry.send_done_count(peer).unwrap(), 1);
    assert_eq!(registry.last_send_done_tick(peer).unwrap(), Some(19));
    assert!(!registry.is_busy(endpoint_interface).unwrap());
    assert_eq!(endpoint.busy_until_tick(), None);
}

#[test]
fn distributed_ethernet_link_endpoint_rejects_busy_and_restores_snapshot() {
    let mut registry = EthernetInterfaceRegistry::new();
    let endpoint_interface = registry.register("dist-endpoint").unwrap();
    let peer = registry.register("local-peer").unwrap();
    registry.bind_pair(endpoint_interface, peer).unwrap();

    let timing = DistributedEthernetLinkTiming::new(3, 1).unwrap();
    let delay_variation = EthernetLinkDelayVariation::new(4, vec![2]).unwrap();
    let mut endpoint = DistributedEthernetLinkEndpoint::new_with_delay_variation(
        endpoint_interface,
        timing,
        delay_variation,
    );
    endpoint
        .transmit(
            &mut registry,
            packet(&[1, 2]).with_wire_length_bytes(2).unwrap(),
            40,
        )
        .unwrap();
    assert_eq!(endpoint.busy_until_tick(), Some(49));
    assert!(matches!(
        endpoint.transmit(&mut registry, packet(&[3]), 48),
        Err(NetworkError::DistributedEthernetLinkBusy {
            interface,
            request_tick: 48,
            busy_until_tick: 49,
        }) if interface == endpoint_interface
    ));

    let snapshot = endpoint.snapshot();
    let done = endpoint
        .drain_transmit_done(&mut registry, 49)
        .unwrap()
        .unwrap();
    assert_eq!(done.transmission().delay_variation_ticks(), 2);
    assert_eq!(registry.send_done_count(peer).unwrap(), 1);
    assert_eq!(endpoint.busy_until_tick(), None);

    endpoint.restore(&snapshot).unwrap();
    assert_eq!(endpoint.next_sequence(), 1);
    assert_eq!(endpoint.codec_record_count(), 1);
    assert_eq!(endpoint.busy_until_tick(), Some(49));
    assert_eq!(endpoint.pending_transmit_count(), 1);
}

#[test]
fn distributed_ethernet_link_endpoint_schedules_remote_receives_to_interface_peer() {
    let mut registry = EthernetInterfaceRegistry::new();
    let endpoint_interface = registry.register("dist-endpoint").unwrap();
    let peer = registry.register("local-peer").unwrap();
    registry.bind_pair(endpoint_interface, peer).unwrap();

    let timing = DistributedEthernetLinkTiming::new(2, 3).unwrap();
    let mut endpoint = DistributedEthernetLinkEndpoint::new(endpoint_interface, timing);
    let window = DistributedEthernetReceiveWindow::new(9, 16).unwrap();
    let inbound = DistributedEthernetMessage::data(10, 4, packet(&[0xaa, 0xbb])).unwrap();

    let scheduled = endpoint
        .accept_remote_message(inbound, 11, Some(window))
        .unwrap();
    assert_eq!(scheduled.send_tick(), 10);
    assert_eq!(scheduled.send_delay_ticks(), 4);
    assert_eq!(scheduled.link_delay_ticks(), 3);
    assert_eq!(scheduled.receive_tick(), 17);
    assert_eq!(endpoint.next_receive_tick(), Some(17));

    assert!(endpoint
        .drain_ready_receives(&mut registry, 16)
        .unwrap()
        .is_empty());
    let deliveries = endpoint.drain_ready_receives(&mut registry, 17).unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].delivery().delivery_tick(), 17);
    assert_eq!(deliveries[0].send_record().source(), endpoint_interface);
    assert_eq!(deliveries[0].send_record().peer(), Some(peer));
    assert_eq!(deliveries[0].send_record().tick(), 17);
    assert_eq!(
        deliveries[0].send_record().packet().payload(),
        &[0xaa, 0xbb]
    );
    assert_eq!(registry.receive_count(peer).unwrap(), 1);
    assert_eq!(registry.last_receive_tick(peer).unwrap(), Some(17));
    assert_eq!(endpoint.pending_receive_count(), 0);
}
