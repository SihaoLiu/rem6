use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteAckMode, GdbRemoteCommand, GdbRemoteError, GdbRemoteFeature,
    GdbRemoteFeatureValue, GdbRemoteFrame, GdbRemoteNotification, GdbRemotePacket,
    GdbRemotePacketConfig, GdbRemoteSession,
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
    assert_eq!(escaped_interrupt_payload.checksum(), 0x03);
}

#[test]
fn gdb_remote_packet_decodes_run_length_encoded_response_data() {
    let parsed = GdbRemotePacket::parse_response_frame(b"$0* #7a").unwrap();

    assert_eq!(parsed.payload(), b"0000");
    assert_eq!(parsed.checksum(), 0xc0);
    assert_eq!(parsed.encode_frame(), b"$0000#c0");
}

#[test]
fn gdb_remote_packet_keeps_literal_star_in_command_data() {
    let parsed = GdbRemotePacket::parse_frame(b"$X*#82").unwrap();

    assert_eq!(parsed.payload(), b"X*");
    assert_eq!(parsed.checksum(), 0xdf);
    assert_eq!(
        parsed.encode_frame(),
        vec![b'$', b'X', b'}', 0x0a, b'#', b'd', b'f']
    );
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
    assert_eq!(
        GdbRemotePacket::parse_frame(b"$12:qSupported#d4").unwrap_err(),
        GdbRemoteError::LegacySequenceIdUnsupported,
    );
    assert_eq!(
        GdbRemotePacket::parse_response_frame(b"$* #4a").unwrap_err(),
        GdbRemoteError::RunLengthWithoutPreviousByte,
    );
    assert_eq!(
        GdbRemotePacket::parse_response_frame(b"$0*#5a").unwrap_err(),
        GdbRemoteError::MissingRunLengthCount,
    );
    assert_eq!(
        GdbRemotePacket::parse_response_frame(&[b'$', b'0', b'*', 0x1f, b'#', b'7', b'9'])
            .unwrap_err(),
        GdbRemoteError::InvalidRunLengthCount { byte: 0x1f },
    );
    assert_eq!(
        GdbRemotePacket::parse_response_frame(b"$0*$#7e").unwrap_err(),
        GdbRemoteError::InvalidRunLengthCount { byte: b'$' },
    );
    assert_eq!(
        GdbRemotePacket::parse_response_frame(&[b'$', b'0', b'*', 0x7f, b'#', b'd', b'9'])
            .unwrap_err(),
        GdbRemoteError::InvalidRunLengthCount { byte: 0x7f },
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
    assert_eq!(
        GdbRemotePacket::parse_response_frame_with_config(
            b"$0* #7a",
            GdbRemotePacketConfig::new(3).unwrap(),
        )
        .unwrap_err(),
        GdbRemoteError::PayloadTooLong { len: 4, max: 3 },
    );
}

#[test]
fn gdb_remote_commands_decode_supported_feature_tokens() {
    let supported = GdbRemoteCommand::parse(
        &GdbRemotePacket::parse_frame(
            b"$qSupported:multiprocess+;xmlRegisters=riscv;hwbreak-;QNonStop+#42",
        )
        .unwrap(),
    );

    assert_eq!(
        supported,
        GdbRemoteCommand::QuerySupported {
            features: vec![
                GdbRemoteFeature::new(b"multiprocess".to_vec(), GdbRemoteFeatureValue::Supported),
                GdbRemoteFeature::new(
                    b"xmlRegisters".to_vec(),
                    GdbRemoteFeatureValue::Value(b"riscv".to_vec()),
                ),
                GdbRemoteFeature::new(b"hwbreak".to_vec(), GdbRemoteFeatureValue::Unsupported),
                GdbRemoteFeature::new(b"QNonStop".to_vec(), GdbRemoteFeatureValue::Supported),
            ],
        },
    );
}

#[test]
fn gdb_remote_commands_treat_probe_suffix_as_gdb_feature_data() {
    let supported = GdbRemoteCommand::parse(
        &GdbRemotePacket::parse_frame(b"$qSupported:QNonStop?#d2").unwrap(),
    );

    assert_eq!(
        supported,
        GdbRemoteCommand::QuerySupported {
            features: vec![GdbRemoteFeature::new(
                b"QNonStop?".to_vec(),
                GdbRemoteFeatureValue::Bare,
            )],
        },
    );
}

#[test]
fn gdb_remote_commands_decode_no_ack_requests() {
    let no_ack =
        GdbRemoteCommand::parse(&GdbRemotePacket::parse_frame(b"$QStartNoAckMode#b0").unwrap());
    assert_eq!(no_ack, GdbRemoteCommand::StartNoAckMode);
}

#[test]
fn gdb_remote_commands_preserve_unknown_payloads() {
    let unknown = GdbRemoteCommand::parse(&GdbRemotePacket::parse_frame(b"$vCont?#49").unwrap());
    assert_eq!(unknown, GdbRemoteCommand::Unknown(b"vCont?".to_vec()));
}

#[test]
fn gdb_remote_session_acknowledges_valid_packets_by_default() {
    let mut session = GdbRemoteSession::new(Vec::new());
    let packet = GdbRemotePacket::new(b"vCont?".to_vec()).unwrap();

    assert_eq!(session.ack_mode(), GdbRemoteAckMode::Acknowledged);
    assert_eq!(
        session.handle_packet(&packet).unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(Vec::new()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_supported_features() {
    let mut session = GdbRemoteSession::new(vec![
        GdbRemoteFeature::new(
            b"PacketSize".to_vec(),
            GdbRemoteFeatureValue::Value(b"4000".to_vec()),
        ),
        GdbRemoteFeature::new(
            b"QStartNoAckMode".to_vec(),
            GdbRemoteFeatureValue::Supported,
        ),
    ]);
    let packet = GdbRemotePacket::new(b"qSupported:multiprocess+".to_vec()).unwrap();

    assert_eq!(
        session.handle_packet(&packet).unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::new(b"PacketSize=4000;QStartNoAckMode+".to_vec()).unwrap(),
            ),
        ],
    );
}

#[test]
fn gdb_remote_session_replaces_supported_query_features() {
    let mut session = GdbRemoteSession::new(Vec::new());

    session
        .handle_packet(
            &GdbRemotePacket::new(b"qSupported:multiprocess+;xmlRegisters=riscv".to_vec()).unwrap(),
        )
        .unwrap();
    session
        .handle_packet(&GdbRemotePacket::new(b"qSupported:hwbreak-".to_vec()).unwrap())
        .unwrap();

    assert_eq!(
        session.gdb_features(),
        &[GdbRemoteFeature::new(
            b"hwbreak".to_vec(),
            GdbRemoteFeatureValue::Unsupported,
        )],
    );
}

#[test]
fn gdb_remote_session_switches_to_no_ack_mode() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"QStartNoAckMode".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);
    let packet = GdbRemotePacket::new(b"QStartNoAckMode".to_vec()).unwrap();

    assert_eq!(
        session.handle_packet(&packet).unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(session.ack_mode(), GdbRemoteAckMode::NoAck);
}

#[test]
fn gdb_remote_session_rejects_no_ack_without_advertised_support() {
    let mut session = GdbRemoteSession::new(Vec::new());
    let packet = GdbRemotePacket::new(b"QStartNoAckMode".to_vec()).unwrap();

    assert_eq!(
        session.handle_packet(&packet).unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(Vec::new()).unwrap()),
        ],
    );
    assert_eq!(session.ack_mode(), GdbRemoteAckMode::Acknowledged);
}

#[test]
fn gdb_remote_session_omits_acknowledgements_after_no_ack_mode() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"QStartNoAckMode".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);

    session
        .handle_packet(&GdbRemotePacket::new(b"QStartNoAckMode".to_vec()).unwrap())
        .unwrap();

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qSupported".to_vec()).unwrap())
            .unwrap(),
        vec![GdbRemoteFrame::Packet(
            GdbRemotePacket::new(b"QStartNoAckMode+".to_vec()).unwrap()
        )],
    );
}

#[test]
fn gdb_remote_session_retransmits_last_response_after_negative_acknowledgement() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"PacketSize".to_vec(),
        GdbRemoteFeatureValue::Value(b"4000".to_vec()),
    )]);

    session
        .handle_packet(&GdbRemotePacket::new(b"qSupported".to_vec()).unwrap())
        .unwrap();

    assert_eq!(
        session.handle_frame(&GdbRemoteFrame::NegativeAck).unwrap(),
        vec![GdbRemoteFrame::Packet(
            GdbRemotePacket::new(b"PacketSize=4000".to_vec()).unwrap(),
        )],
    );
}

#[test]
fn gdb_remote_session_handles_packet_frames() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"PacketSize".to_vec(),
        GdbRemoteFeatureValue::Value(b"4000".to_vec()),
    )]);
    let frame = GdbRemoteFrame::Packet(GdbRemotePacket::new(b"qSupported".to_vec()).unwrap());

    assert_eq!(
        session.handle_frame(&frame).unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"PacketSize=4000".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_ignores_acknowledgement_frames_without_replies() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session.handle_frame(&GdbRemoteFrame::Ack).unwrap(),
        Vec::new()
    );
    assert_eq!(
        session.handle_frame(&GdbRemoteFrame::NegativeAck).unwrap(),
        Vec::new(),
    );
    assert_eq!(session.ack_mode(), GdbRemoteAckMode::Acknowledged);
    assert!(!session.interrupt_requested());
}

#[test]
fn gdb_remote_session_records_interrupt_frames_without_acknowledgement() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session.handle_frame(&GdbRemoteFrame::Interrupt).unwrap(),
        Vec::new(),
    );
    assert!(session.interrupt_requested());
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

#[test]
fn gdb_remote_frame_parser_reports_notification_packets_without_acknowledgement() {
    let parsed = parse_gdb_remote_frame(b"noise%Stop:T05#99+")
        .unwrap()
        .unwrap();

    assert_eq!(parsed.skipped_bytes(), b"noise");
    assert_eq!(parsed.consumed_bytes(), b"noise%Stop:T05#99".len());
    assert_eq!(
        parsed.frame(),
        &GdbRemoteFrame::Notification(GdbRemoteNotification::new(b"Stop:T05".to_vec()).unwrap()),
    );
}

#[test]
fn gdb_remote_frame_parser_ignores_corrupted_notifications_before_later_frames() {
    let parsed = parse_gdb_remote_frame(b"%Stop:T05#00$qSupported#37")
        .unwrap()
        .unwrap();

    assert_eq!(parsed.skipped_bytes(), b"%Stop:T05#00");
    assert_eq!(parsed.consumed_bytes(), b"%Stop:T05#00$qSupported#37".len());
    assert_eq!(
        parsed.frame(),
        &GdbRemoteFrame::Packet(GdbRemotePacket::new(b"qSupported".to_vec()).unwrap()),
    );
}

#[test]
fn gdb_remote_frame_parser_resynchronizes_after_interrupted_notifications() {
    let parsed_packet = parse_gdb_remote_frame(b"%bad$qSupported#37")
        .unwrap()
        .unwrap();

    assert_eq!(parsed_packet.skipped_bytes(), b"%bad");
    assert_eq!(parsed_packet.consumed_bytes(), b"%bad$qSupported#37".len());
    assert_eq!(
        parsed_packet.frame(),
        &GdbRemoteFrame::Packet(GdbRemotePacket::new(b"qSupported".to_vec()).unwrap()),
    );

    let parsed_notification = parse_gdb_remote_frame(b"%bad%Stop:T05#99")
        .unwrap()
        .unwrap();

    assert_eq!(parsed_notification.skipped_bytes(), b"%bad");
    assert_eq!(
        parsed_notification.consumed_bytes(),
        b"%bad%Stop:T05#99".len()
    );
    assert_eq!(
        parsed_notification.frame(),
        &GdbRemoteFrame::Notification(GdbRemoteNotification::new(b"Stop:T05".to_vec()).unwrap()),
    );
}
