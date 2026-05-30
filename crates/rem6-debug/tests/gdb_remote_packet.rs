use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteError, GdbRemoteFrame, GdbRemotePacket, GdbRemotePacketConfig,
};

#[test]
fn gdb_remote_packet_encodes_and_validates_checksum() {
    let packet = GdbRemotePacket::new(b"qSupported".to_vec()).unwrap();

    assert_eq!(packet.payload(), b"qSupported");
    assert_eq!(packet.checksum(), 0x37);
    assert_eq!(packet.encode_frame(), b"$qSupported#37".to_vec());

    let parsed = GdbRemotePacket::parse_frame(b"$qSupported#37").unwrap();
    assert_eq!(parsed, packet);
}

#[test]
fn gdb_remote_packet_escapes_binary_payload_delimiters() {
    let packet = GdbRemotePacket::new(vec![b'#', b'$', b'}', b'*']).unwrap();

    assert_eq!(packet.payload(), b"#$}*");
    assert_eq!(
        packet.encode_frame(),
        vec![b'$', b'}', 0x03, b'}', 0x04, b'}', b']', b'}', 0x0a, b'#', b'6', b'2',],
    );
    assert_eq!(packet.checksum(), 0x62);

    let parsed = GdbRemotePacket::parse_frame(&packet.encode_frame()).unwrap();
    assert_eq!(parsed, packet);

    let escaped_interrupt_payload = GdbRemotePacket::parse_frame(b"$}##a0").unwrap();
    assert_eq!(escaped_interrupt_payload.payload(), &[0x03]);
    assert_eq!(escaped_interrupt_payload.checksum(), 0xa0);
}

#[test]
fn gdb_remote_packet_rejects_malformed_frames_before_payload_mutation() {
    assert_eq!(
        GdbRemotePacket::parse_frame(b"qSupported#37").unwrap_err(),
        GdbRemoteError::MissingPacketStart,
    );
    assert_eq!(
        GdbRemotePacket::parse_frame(b"$qSupported37").unwrap_err(),
        GdbRemoteError::MissingChecksumSeparator,
    );
    assert_eq!(
        GdbRemotePacket::parse_frame(b"$qSupported#3").unwrap_err(),
        GdbRemoteError::ShortChecksum,
    );
    assert_eq!(
        GdbRemotePacket::parse_frame(b"$qSupported#3g").unwrap_err(),
        GdbRemoteError::InvalidChecksumHex { byte: b'g' },
    );
    assert_eq!(
        GdbRemotePacket::parse_frame(b"$qSupported#00").unwrap_err(),
        GdbRemoteError::ChecksumMismatch {
            expected: 0x37,
            actual: 0x00,
        },
    );
    assert_eq!(
        GdbRemotePacket::parse_frame(b"$qSupported#37+").unwrap_err(),
        GdbRemoteError::TrailingBytes { count: 1 },
    );
}

#[test]
fn gdb_remote_packet_config_rejects_unbounded_or_oversized_payloads() {
    assert_eq!(
        GdbRemotePacketConfig::new(0).unwrap_err(),
        GdbRemoteError::ZeroMaxPayloadBytes,
    );

    let config = GdbRemotePacketConfig::new(4).unwrap();
    assert_eq!(config.max_payload_bytes(), 4);
    assert_eq!(
        GdbRemotePacket::parse_frame_with_config(b"$abcde#ef", config).unwrap_err(),
        GdbRemoteError::PayloadTooLong { len: 5, max: 4 },
    );
    assert_eq!(
        GdbRemotePacket::with_config(b"abcde".to_vec(), config).unwrap_err(),
        GdbRemoteError::PayloadTooLong { len: 5, max: 4 },
    );
}

#[test]
fn gdb_remote_frame_parser_reports_ack_interrupt_packet_and_prefix_noise() {
    let parsed_ack = parse_gdb_remote_frame(b"++").unwrap().unwrap();
    assert_eq!(parsed_ack.frame(), &GdbRemoteFrame::Ack);
    assert_eq!(parsed_ack.consumed_bytes(), 1);
    assert_eq!(parsed_ack.skipped_bytes(), b"");

    let parsed_nack = parse_gdb_remote_frame(b"-").unwrap().unwrap();
    assert_eq!(parsed_nack.frame(), &GdbRemoteFrame::NegativeAck);
    assert_eq!(parsed_nack.consumed_bytes(), 1);

    let parsed_interrupt = parse_gdb_remote_frame(&[0x03, b'+']).unwrap().unwrap();
    assert_eq!(parsed_interrupt.frame(), &GdbRemoteFrame::Interrupt);
    assert_eq!(parsed_interrupt.consumed_bytes(), 1);

    let parsed_packet = parse_gdb_remote_frame(b"noise$qSupported#37+")
        .unwrap()
        .unwrap();
    assert_eq!(parsed_packet.skipped_bytes(), b"noise");
    assert_eq!(parsed_packet.consumed_bytes(), b"noise$qSupported#37".len());
    assert_eq!(
        parsed_packet.frame(),
        &GdbRemoteFrame::Packet(GdbRemotePacket::new(b"qSupported".to_vec()).unwrap()),
    );

    assert_eq!(parse_gdb_remote_frame(b"noise").unwrap(), None);
    assert_eq!(
        parse_gdb_remote_frame(b"$qSupported#00").unwrap_err(),
        GdbRemoteError::ChecksumMismatch {
            expected: 0x37,
            actual: 0x00,
        },
    );
}
