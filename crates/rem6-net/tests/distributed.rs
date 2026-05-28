use rem6_net::{
    DistributedEthernetCodec, DistributedEthernetHeader, DistributedEthernetMessage,
    DistributedEthernetMessageKind, DistributedEthernetReqType, EthernetPacket, NetworkError,
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
