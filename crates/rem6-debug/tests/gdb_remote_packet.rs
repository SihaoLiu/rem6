use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteAckMode, GdbRemoteCommand, GdbRemoteError, GdbRemoteFeature,
    GdbRemoteFeatureValue, GdbRemoteFrame, GdbRemoteNotification, GdbRemotePacket,
    GdbRemotePacketConfig, GdbRemoteRegisterBytes, GdbRemoteSession, GdbRemoteThreadId,
    GdbRemoteThreadOperation,
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
fn gdb_remote_commands_decode_stop_reason_queries() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::parse_frame(b"$?#3f").unwrap());

    assert_eq!(command, GdbRemoteCommand::QueryStopReason);
}

#[test]
fn gdb_remote_commands_decode_thread_selection_requests() {
    let general_any = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"Hg0".to_vec()).unwrap());
    assert_eq!(
        general_any,
        GdbRemoteCommand::SetThread {
            operation: GdbRemoteThreadOperation::General,
            thread: GdbRemoteThreadId::Any,
        },
    );

    let continue_all = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"Hc-1".to_vec()).unwrap());
    assert_eq!(
        continue_all,
        GdbRemoteCommand::SetThread {
            operation: GdbRemoteThreadOperation::Continue,
            thread: GdbRemoteThreadId::All,
        },
    );

    let uppercase = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"Hg1A".to_vec()).unwrap());
    assert_eq!(
        uppercase,
        GdbRemoteCommand::SetThread {
            operation: GdbRemoteThreadOperation::General,
            thread: GdbRemoteThreadId::Id(0x1a),
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_thread_selection_requests() {
    for payload in [
        b"H".as_slice(),
        b"Hx0".as_slice(),
        b"Hg".as_slice(),
        b"Hg-2".as_slice(),
        b"Hg00".as_slice(),
        b"Hgzz".as_slice(),
        b"Hg10000000000000000".as_slice(),
        b"Hgp1.2".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_read_register_requests() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::parse_frame(b"$g#67").unwrap());

    assert_eq!(command, GdbRemoteCommand::ReadRegisters);
}

#[test]
fn gdb_remote_commands_decode_write_registers_requests() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"G78563412".to_vec()).unwrap());

    assert_eq!(
        command,
        GdbRemoteCommand::WriteRegisters {
            bytes: vec![0x78, 0x56, 0x34, 0x12],
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_write_registers_requests() {
    for payload in [
        b"G".as_slice(),
        b"G7".as_slice(),
        b"Gzz".as_slice(),
        b"G78xx".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_single_register_requests() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::parse_frame(b"$p1a#02").unwrap());

    assert_eq!(command, GdbRemoteCommand::ReadRegister { number: 0x1a });

    let uppercase = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"p1A".to_vec()).unwrap());
    assert_eq!(uppercase, GdbRemoteCommand::ReadRegister { number: 0x1a });
}

#[test]
fn gdb_remote_commands_preserve_malformed_single_register_requests() {
    let missing_number = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"p".to_vec()).unwrap());
    assert_eq!(missing_number, GdbRemoteCommand::Unknown(b"p".to_vec()));

    let invalid_number = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"pzz".to_vec()).unwrap());
    assert_eq!(invalid_number, GdbRemoteCommand::Unknown(b"pzz".to_vec()));

    let overflowing_number =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"p10000000000000000".to_vec()).unwrap());
    assert_eq!(
        overflowing_number,
        GdbRemoteCommand::Unknown(b"p10000000000000000".to_vec()),
    );
}

#[test]
fn gdb_remote_commands_decode_single_register_write_requests() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"P1a=78563412".to_vec()).unwrap());

    assert_eq!(
        command,
        GdbRemoteCommand::WriteRegister {
            number: 0x1a,
            bytes: vec![0x78, 0x56, 0x34, 0x12],
        },
    );

    let uppercase = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"P1A=DeAd".to_vec()).unwrap());
    assert_eq!(
        uppercase,
        GdbRemoteCommand::WriteRegister {
            number: 0x1a,
            bytes: vec![0xde, 0xad],
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_single_register_write_requests() {
    for payload in [
        b"P".as_slice(),
        b"P1a".as_slice(),
        b"P=7856".as_slice(),
        b"Pzz=7856".as_slice(),
        b"P1a=".as_slice(),
        b"P1a=7".as_slice(),
        b"P1a=zz".as_slice(),
        b"P10000000000000000=78".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_memory_read_requests() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"m1000,10".to_vec()).unwrap());

    assert_eq!(
        command,
        GdbRemoteCommand::ReadMemory {
            address: 0x1000,
            length: 0x10,
        },
    );

    let uppercase = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"m1A,4".to_vec()).unwrap());
    assert_eq!(
        uppercase,
        GdbRemoteCommand::ReadMemory {
            address: 0x1a,
            length: 4,
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_memory_read_requests() {
    for payload in [
        b"m".as_slice(),
        b"m1000".as_slice(),
        b"m,10".as_slice(),
        b"m1000,".as_slice(),
        b"mzz,10".as_slice(),
        b"m1000,zz".as_slice(),
        b"m10000000000000000,1".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_memory_write_requests() {
    let command =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"M1000,4:7f454c46".to_vec()).unwrap());

    assert_eq!(
        command,
        GdbRemoteCommand::WriteMemory {
            address: 0x1000,
            bytes: vec![0x7f, 0x45, 0x4c, 0x46],
        },
    );

    let uppercase = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"M1A,2:DeAd".to_vec()).unwrap());
    assert_eq!(
        uppercase,
        GdbRemoteCommand::WriteMemory {
            address: 0x1a,
            bytes: vec![0xde, 0xad],
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_memory_write_requests() {
    for payload in [
        b"M".as_slice(),
        b"M1000,4".as_slice(),
        b"M1000:7f".as_slice(),
        b"M,1:7f".as_slice(),
        b"M1000,:7f".as_slice(),
        b"Mzz,1:7f".as_slice(),
        b"M1000,zz:7f".as_slice(),
        b"M1000,1:7".as_slice(),
        b"M1000,1:zz".as_slice(),
        b"M1000,2:7f".as_slice(),
        b"M10000000000000000,1:7f".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
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
fn gdb_remote_session_reports_default_stop_reason() {
    let mut session = GdbRemoteSession::new(Vec::new());
    let packet = GdbRemotePacket::new(b"?".to_vec()).unwrap();

    assert_eq!(
        session.handle_packet(&packet).unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"S05".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_register_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_register_bytes(GdbRemoteRegisterBytes::new(vec![
        0x34, 0x12, 0xef, 0xbe, 0xad, 0xde,
    ]));

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"3412efbeadde".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_writes_register_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"G78563412".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"78563412".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_write_register_response_config_errors() {
    let mut session =
        GdbRemoteSession::with_response_config(Vec::new(), GdbRemotePacketConfig::new(1).unwrap());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"G7856".to_vec()).unwrap())
            .unwrap_err(),
        GdbRemoteError::PayloadTooLong { len: 2, max: 1 },
    );
}

#[test]
fn gdb_remote_session_records_thread_selection() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(session.general_thread(), GdbRemoteThreadId::Any);
    assert_eq!(session.continue_thread(), GdbRemoteThreadId::Any);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"Hg1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(session.general_thread(), GdbRemoteThreadId::Id(0x1a));
    assert_eq!(session.continue_thread(), GdbRemoteThreadId::Any);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"Hc-1".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(session.general_thread(), GdbRemoteThreadId::Id(0x1a));
    assert_eq!(session.continue_thread(), GdbRemoteThreadId::All);
}

#[test]
fn gdb_remote_session_reports_single_register_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_register_value(
        0x1a,
        GdbRemoteRegisterBytes::new(vec![0x78, 0x56, 0x34, 0x12]),
    );

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"78563412".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_unavailable_single_register_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_register_unavailable(0x1a, 4);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"xxxxxxxx".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_error_single_register_response_when_missing() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_writes_single_register_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_register_value(0x1a, GdbRemoteRegisterBytes::new(vec![0, 0, 0, 0]));

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"P1a=78563412".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"78563412".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_writes_unavailable_single_register_as_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_register_unavailable(0x1a, 4);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"P1a=7856".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"7856".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_memory_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_memory_bytes(0x1000, vec![0x7f, 0x45, 0x4c, 0x46]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"m1001,2".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"454c".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_rejects_memory_reads_that_exceed_response_packet_limit() {
    let mut session =
        GdbRemoteSession::with_response_config(Vec::new(), GdbRemotePacketConfig::new(6).unwrap());
    session.set_memory_bytes(0x1000, vec![0x7f, 0x45, 0x4c, 0x46]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_memory_read_error_when_missing() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_memory_bytes(0x1000, vec![0x7f, 0x45]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_memory_read_error_before_huge_allocation() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"m0,ffffffffffffffff".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_memory_read_error_on_address_overflow() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_memory_bytes(u64::MAX, vec![0xaa]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"mffffffffffffffff,2".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_writes_memory_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_memory_bytes(0x1000, vec![0x00, 0x00, 0x00, 0x00]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"M1001,2:abcd".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"00abcd00".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_rejects_overflowing_memory_writes_without_partial_update() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_memory_bytes(u64::MAX, vec![0xaa]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"Mffffffffffffffff,2:0102".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"mffffffffffffffff,1".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"aa".to_vec()).unwrap()),
        ],
    );
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
