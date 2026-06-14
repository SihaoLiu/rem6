use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteAckMode, GdbRemoteAttachKind, GdbRemoteCommand,
    GdbRemoteControlState, GdbRemoteDisconnectRequest, GdbRemoteError, GdbRemoteExecutionControl,
    GdbRemoteFeature, GdbRemoteFeatureValue, GdbRemoteFrame, GdbRemoteNotification,
    GdbRemotePacket, GdbRemotePacketConfig, GdbRemoteRegisterBytes, GdbRemoteResumeKind,
    GdbRemoteResumeRequest, GdbRemoteSession, GdbRemoteStopReply, GdbRemoteThreadId,
    GdbRemoteThreadInfoQuery, GdbRemoteThreadOperation, GdbRemoteTrapKind, GdbRemoteTrapOperation,
    GdbRemoteTrapPoint, GdbRemoteTrapRequest, GdbRemoteXferObject, GdbRemoteXferReadRequest,
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
fn gdb_remote_commands_decode_attach_state_queries() {
    let plain = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qAttached".to_vec()).unwrap());
    assert_eq!(plain, GdbRemoteCommand::QueryAttached { process_id: None });

    let with_process =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qAttached:1A".to_vec()).unwrap());
    assert_eq!(
        with_process,
        GdbRemoteCommand::QueryAttached {
            process_id: Some(0x1a),
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_attach_state_queries() {
    for payload in [
        b"qAttached:".as_slice(),
        b"qAttached:0".as_slice(),
        b"qAttached:zz".as_slice(),
        b"qAttached:10000000000000000".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_current_thread_queries() {
    let current = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qC".to_vec()).unwrap());
    assert_eq!(current, GdbRemoteCommand::QueryCurrentThread);
}

#[test]
fn gdb_remote_commands_decode_symbol_lookup_queries() {
    let plain = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qSymbol".to_vec()).unwrap());
    assert_eq!(plain, GdbRemoteCommand::QuerySymbol);

    let initial_lookup =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qSymbol::".to_vec()).unwrap());
    assert_eq!(initial_lookup, GdbRemoteCommand::QuerySymbol);

    let symbol_reply = GdbRemoteCommand::parse(
        &GdbRemotePacket::new(b"qSymbol:1000:5f7374617274".to_vec()).unwrap(),
    );
    assert_eq!(symbol_reply, GdbRemoteCommand::QuerySymbol);
}

#[test]
fn gdb_remote_commands_decode_monitor_command_queries() {
    let command =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qRcmd,68656c6c6f".to_vec()).unwrap());
    assert_eq!(
        command,
        GdbRemoteCommand::QueryMonitorCommand {
            command: b"hello".to_vec(),
        },
    );

    let colon_command =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qRcmd:65786974".to_vec()).unwrap());
    assert_eq!(
        colon_command,
        GdbRemoteCommand::QueryMonitorCommand {
            command: b"exit".to_vec(),
        },
    );

    let empty = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qRcmd".to_vec()).unwrap());
    assert_eq!(
        empty,
        GdbRemoteCommand::QueryMonitorCommand {
            command: Vec::new(),
        },
    );
}

#[test]
fn gdb_remote_commands_decode_page_table_dump_request() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(b".".to_vec()).unwrap());
    assert_eq!(command, GdbRemoteCommand::DumpPageTable);
}

#[test]
fn gdb_remote_commands_decode_thread_info_queries() {
    let first = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qfThreadInfo".to_vec()).unwrap());
    assert_eq!(
        first,
        GdbRemoteCommand::QueryThreadInfo {
            query: GdbRemoteThreadInfoQuery::First,
        },
    );

    let subsequent =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"qsThreadInfo".to_vec()).unwrap());
    assert_eq!(
        subsequent,
        GdbRemoteCommand::QueryThreadInfo {
            query: GdbRemoteThreadInfoQuery::Subsequent,
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_query_prefix_neighbors() {
    for payload in [
        b"qCRC:1000,4".as_slice(),
        b"qC1".as_slice(),
        b"qRcmdExtra".as_slice(),
        b"qSymbolExtra".as_slice(),
        b"qfThreadInfoExtra".as_slice(),
        b"qsThreadInfoExtra".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_preserve_malformed_monitor_command_queries() {
    for payload in [b"qRcmd,6".as_slice(), b"qRcmd,zz".as_slice()] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_xfer_feature_reads() {
    let command = GdbRemoteCommand::parse(
        &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,10".to_vec()).unwrap(),
    );

    assert_eq!(
        command,
        GdbRemoteCommand::QueryXferRead {
            request: GdbRemoteXferReadRequest::new(
                GdbRemoteXferObject::Features,
                b"target.xml".to_vec(),
                0,
                0x10,
            ),
        },
    );

    let uppercase = GdbRemoteCommand::parse(
        &GdbRemotePacket::new(b"qXfer:features:read:riscv-64bit-cpu.xml:A,F".to_vec()).unwrap(),
    );
    assert_eq!(
        uppercase,
        GdbRemoteCommand::QueryXferRead {
            request: GdbRemoteXferReadRequest::new(
                GdbRemoteXferObject::Features,
                b"riscv-64bit-cpu.xml".to_vec(),
                0x0a,
                0x0f,
            ),
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_xfer_feature_reads() {
    for payload in [
        b"qXfer".as_slice(),
        b"qXfer:features".as_slice(),
        b"qXfer:features:write:target.xml:0,10".as_slice(),
        b"qXfer:auxv:read::0,10".as_slice(),
        b"qXfer:features:read::0,10".as_slice(),
        b"qXfer:features:read:target.xml".as_slice(),
        b"qXfer:features:read:target.xml:0".as_slice(),
        b"qXfer:features:read:target.xml:,10".as_slice(),
        b"qXfer:features:read:target.xml:0,".as_slice(),
        b"qXfer:features:read:target.xml:zz,10".as_slice(),
        b"qXfer:features:read:target.xml:0,zz".as_slice(),
        b"qXfer:features:read:target.xml:0,0".as_slice(),
        b"qXfer:features:read:target.xml:10000000000000000,1".as_slice(),
        b"qXfer:features:read:target.xml:0,10000000000000000".as_slice(),
        b"qXfer:features:read:target.xml:0,10:extra".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
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
fn gdb_remote_commands_decode_thread_alive_requests() {
    let alive = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"T1A".to_vec()).unwrap());

    assert_eq!(
        alive,
        GdbRemoteCommand::ThreadAlive {
            thread: GdbRemoteThreadId::Id(0x1a),
        },
    );

    let any = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"T0".to_vec()).unwrap());
    assert_eq!(
        any,
        GdbRemoteCommand::ThreadAlive {
            thread: GdbRemoteThreadId::Any,
        },
    );

    let all = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"T-1".to_vec()).unwrap());
    assert_eq!(
        all,
        GdbRemoteCommand::ThreadAlive {
            thread: GdbRemoteThreadId::All,
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_thread_alive_requests() {
    for payload in [
        b"T".as_slice(),
        b"Tzz".as_slice(),
        b"Tp1.2".as_slice(),
        b"T10000000000000000".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_resume_requests() {
    let resume = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"c".to_vec()).unwrap());
    assert_eq!(
        resume,
        GdbRemoteCommand::Resume {
            kind: GdbRemoteResumeKind::Continue,
            signal: None,
            address: None,
        },
    );

    let resume_at = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"c1A".to_vec()).unwrap());
    assert_eq!(
        resume_at,
        GdbRemoteCommand::Resume {
            kind: GdbRemoteResumeKind::Continue,
            signal: None,
            address: Some(0x1a),
        },
    );

    let signaled_resume =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"C05;1000".to_vec()).unwrap());
    assert_eq!(
        signaled_resume,
        GdbRemoteCommand::Resume {
            kind: GdbRemoteResumeKind::Continue,
            signal: Some(0x05),
            address: Some(0x1000),
        },
    );

    let step = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"s".to_vec()).unwrap());
    assert_eq!(
        step,
        GdbRemoteCommand::Resume {
            kind: GdbRemoteResumeKind::SingleInstruction,
            signal: None,
            address: None,
        },
    );

    let signaled_step = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"S0b".to_vec()).unwrap());
    assert_eq!(
        signaled_step,
        GdbRemoteCommand::Resume {
            kind: GdbRemoteResumeKind::SingleInstruction,
            signal: Some(0x0b),
            address: None,
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_resume_requests() {
    for payload in [
        b"czz".as_slice(),
        b"c10000000000000000".as_slice(),
        b"C".as_slice(),
        b"C5".as_slice(),
        b"Czz".as_slice(),
        b"C05;".as_slice(),
        b"C05;zz".as_slice(),
        b"C100".as_slice(),
        b"szz".as_slice(),
        b"s10000000000000000".as_slice(),
        b"S".as_slice(),
        b"S0".as_slice(),
        b"Sxz".as_slice(),
        b"S0b;".as_slice(),
        b"S0b;zz".as_slice(),
        b"S0b1000".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_vcont_query() {
    let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"vCont?".to_vec()).unwrap());

    assert_eq!(command, GdbRemoteCommand::QueryResumeActions);
}

#[test]
fn gdb_remote_commands_decode_vcont_resume_requests() {
    let resume_all = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"vCont;c".to_vec()).unwrap());
    assert_eq!(
        resume_all,
        GdbRemoteCommand::ResumeActions {
            requests: vec![GdbRemoteResumeRequest::new(
                GdbRemoteResumeKind::Continue,
                None,
                None,
                GdbRemoteThreadId::All,
            )],
        },
    );

    let mixed =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"vCont;C05:1A;s:2;S0b".to_vec()).unwrap());
    assert_eq!(
        mixed,
        GdbRemoteCommand::ResumeActions {
            requests: vec![
                GdbRemoteResumeRequest::new(
                    GdbRemoteResumeKind::Continue,
                    Some(0x05),
                    None,
                    GdbRemoteThreadId::Id(0x1a),
                ),
                GdbRemoteResumeRequest::new(
                    GdbRemoteResumeKind::SingleInstruction,
                    None,
                    None,
                    GdbRemoteThreadId::Id(2),
                ),
                GdbRemoteResumeRequest::new(
                    GdbRemoteResumeKind::SingleInstruction,
                    Some(0x0b),
                    None,
                    GdbRemoteThreadId::All,
                ),
            ],
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_vcont_requests() {
    for payload in [
        b"vCont".as_slice(),
        b"vCont;".as_slice(),
        b"vCont;c:".as_slice(),
        b"vCont;c:zz".as_slice(),
        b"vCont;C".as_slice(),
        b"vCont;C5".as_slice(),
        b"vCont;Czz:1".as_slice(),
        b"vCont;C05;".as_slice(),
        b"vCont;c:10000000000000000".as_slice(),
        b"vCont;r1000,1004".as_slice(),
        b"vCont;t:1".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_disconnect_requests() {
    let detach = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"D".to_vec()).unwrap());
    assert_eq!(
        detach,
        GdbRemoteCommand::Disconnect {
            request: GdbRemoteDisconnectRequest::Detach { process_id: None },
        },
    );

    let detach_process = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"D;1A".to_vec()).unwrap());
    assert_eq!(
        detach_process,
        GdbRemoteCommand::Disconnect {
            request: GdbRemoteDisconnectRequest::Detach {
                process_id: Some(0x1a),
            },
        },
    );

    let terminate = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"k".to_vec()).unwrap());
    assert_eq!(
        terminate,
        GdbRemoteCommand::Disconnect {
            request: GdbRemoteDisconnectRequest::Terminate,
        },
    );

    let terminate_process =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"vKill;2a".to_vec()).unwrap());
    assert_eq!(
        terminate_process,
        GdbRemoteCommand::Disconnect {
            request: GdbRemoteDisconnectRequest::TerminateProcess { process_id: 0x2a },
        },
    );
}

#[test]
fn gdb_remote_commands_preserve_malformed_disconnect_requests() {
    for payload in [
        b"D;".as_slice(),
        b"D1a".as_slice(),
        b"D;0".as_slice(),
        b"D;zz".as_slice(),
        b"D;10000000000000000".as_slice(),
        b"k1".as_slice(),
        b"vKill".as_slice(),
        b"vKill;".as_slice(),
        b"vKill;0".as_slice(),
        b"vKill;zz".as_slice(),
        b"vKill;1;2".as_slice(),
        b"vKill;10000000000000000".as_slice(),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(command, GdbRemoteCommand::Unknown(payload.to_vec()));
    }
}

#[test]
fn gdb_remote_commands_decode_trap_requests() {
    let software = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"Z0,1000,4".to_vec()).unwrap());
    assert_eq!(
        software,
        GdbRemoteCommand::Trap {
            request: GdbRemoteTrapRequest::new(
                GdbRemoteTrapOperation::Insert,
                GdbRemoteTrapKind::SoftwareBreakpoint,
                0x1000,
                4,
            ),
        },
    );

    let hardware = GdbRemoteCommand::parse(&GdbRemotePacket::new(b"z1,1A,2".to_vec()).unwrap());
    assert_eq!(
        hardware,
        GdbRemoteCommand::Trap {
            request: GdbRemoteTrapRequest::new(
                GdbRemoteTrapOperation::Remove,
                GdbRemoteTrapKind::HardwareBreakpoint,
                0x1a,
                2,
            ),
        },
    );

    for (payload, kind) in [
        (b"Z2,2000,8".as_slice(), GdbRemoteTrapKind::WriteWatchpoint),
        (b"Z3,2000,8".as_slice(), GdbRemoteTrapKind::ReadWatchpoint),
        (b"z4,2000,8".as_slice(), GdbRemoteTrapKind::AccessWatchpoint),
    ] {
        let command = GdbRemoteCommand::parse(&GdbRemotePacket::new(payload.to_vec()).unwrap());
        assert_eq!(
            command,
            GdbRemoteCommand::Trap {
                request: GdbRemoteTrapRequest::new(
                    if payload[0] == b'Z' {
                        GdbRemoteTrapOperation::Insert
                    } else {
                        GdbRemoteTrapOperation::Remove
                    },
                    kind,
                    0x2000,
                    8,
                ),
            },
        );
    }
}

#[test]
fn gdb_remote_commands_preserve_malformed_trap_requests() {
    for payload in [
        b"Z".as_slice(),
        b"z".as_slice(),
        b"Z5,1000,4".as_slice(),
        b"Z0".as_slice(),
        b"Z0,1000".as_slice(),
        b"Z0,1000,".as_slice(),
        b"Z0,,4".as_slice(),
        b"Z0,zz,4".as_slice(),
        b"Z0,1000,zz".as_slice(),
        b"Z0,10000000000000000,4".as_slice(),
        b"Z0,1000,10000000000000000".as_slice(),
        b"Z0,1000,4;X1,00".as_slice(),
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
    let unknown =
        GdbRemoteCommand::parse(&GdbRemotePacket::new(b"vMustReplyEmpty".to_vec()).unwrap());
    assert_eq!(
        unknown,
        GdbRemoteCommand::Unknown(b"vMustReplyEmpty".to_vec()),
    );
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
fn gdb_remote_session_reports_attach_state() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(session.attach_kind(), GdbRemoteAttachKind::Attached);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qAttached".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"1".to_vec()).unwrap()),
        ],
    );

    session.set_attach_kind(GdbRemoteAttachKind::Created);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qAttached:1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"0".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_current_thread() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(session.current_thread_id(), 1);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qC".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"QC1".to_vec()).unwrap()),
        ],
    );

    assert!(!session.set_current_thread_id(0x1a));
    assert_eq!(session.current_thread_id(), 1);
    assert!(session.set_thread_ids(vec![1, 0x1a]));
    assert!(session.set_current_thread_id(0x1a));
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qC".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"QC1a".to_vec()).unwrap()),
        ],
    );
    assert!(!session.set_current_thread_id(0));
    assert_eq!(session.current_thread_id(), 0x1a);
}

#[test]
fn gdb_remote_session_reports_no_symbol_lookup_needed() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qSymbol::".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_records_monitor_command_without_packet_response() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(session.last_monitor_command(), None);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qRcmd,65786974".to_vec()).unwrap())
            .unwrap(),
        vec![GdbRemoteFrame::Ack],
    );
    assert_eq!(session.last_monitor_command(), Some(b"exit".as_slice()));
}

#[test]
fn gdb_remote_session_reports_page_table_dump() {
    let mut missing = GdbRemoteSession::new(Vec::new());
    assert_eq!(
        missing
            .handle_packet(&GdbRemotePacket::new(b".".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );

    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_page_table_dump(b"vpn=0x1000 ppn=0x2000 rwx\n".to_vec());
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b".".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::new(b"vpn=0x1000 ppn=0x2000 rwx\n".to_vec()).unwrap()
            ),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_oversized_page_table_dump_as_error_payload() {
    let mut session =
        GdbRemoteSession::with_response_config(Vec::new(), GdbRemotePacketConfig::new(3).unwrap());
    session.set_page_table_dump(b"abcd".to_vec());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b".".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::with_config(b"E01".to_vec(), session.response_config()).unwrap()
            ),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_thread_info_sequence() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(session.thread_ids(), &[1]);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qfThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"m1".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qsThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"l".to_vec()).unwrap()),
        ],
    );

    assert!(session.set_thread_ids(vec![1, 0x1a, 0xff]));
    assert_eq!(session.thread_ids(), &[1, 0x1a, 0xff]);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qfThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"m1,1a,ff".to_vec()).unwrap()),
        ],
    );
    assert!(!session.set_thread_ids(Vec::new()));
    assert!(!session.set_thread_ids(vec![1, 0]));
    assert!(!session.set_thread_ids(vec![1, 0x1a, 1]));
    assert_eq!(session.thread_ids(), &[1, 0x1a, 0xff]);

    assert!(session.set_current_thread_id(0xff));
    assert!(session.set_thread_ids(vec![0x1a, 0xff]));
    assert_eq!(session.current_thread_id(), 0xff);
    assert!(session.set_thread_ids(vec![0x1a]));
    assert_eq!(session.current_thread_id(), 0x1a);
}

#[test]
fn gdb_remote_session_paginates_thread_info_by_payload_limit() {
    let mut session =
        GdbRemoteSession::with_response_config(Vec::new(), GdbRemotePacketConfig::new(5).unwrap());
    assert!(session.set_thread_ids(vec![1, 2, 3, 4, 5]));

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qfThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::with_config(b"m1,2".to_vec(), session.response_config()).unwrap()
            ),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qsThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::with_config(b"m3,4".to_vec(), session.response_config()).unwrap()
            ),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qsThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::with_config(b"m5".to_vec(), session.response_config()).unwrap()
            ),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qsThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::with_config(b"l".to_vec(), session.response_config()).unwrap()
            ),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qfThreadInfo".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::with_config(b"m1,2".to_vec(), session.response_config()).unwrap()
            ),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_thread_alive_status() {
    let mut session = GdbRemoteSession::new(Vec::new());
    assert!(session.set_thread_ids(vec![1, 0x1a]));

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"T1A".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"T0".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"T-1".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"T2".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_records_resume_requests() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_stop_reply(GdbRemoteStopReply::signal(0x0b));

    assert_eq!(session.last_resume_request(), None);
    assert_eq!(session.last_resume_requests(), &[]);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"c1000".to_vec()).unwrap())
            .unwrap(),
        vec![GdbRemoteFrame::Ack],
    );
    assert_eq!(
        session.last_resume_request(),
        Some(&GdbRemoteResumeRequest::new(
            GdbRemoteResumeKind::Continue,
            None,
            Some(0x1000),
            GdbRemoteThreadId::Any,
        )),
    );
    assert_eq!(
        session.last_resume_requests(),
        &[GdbRemoteResumeRequest::new(
            GdbRemoteResumeKind::Continue,
            None,
            Some(0x1000),
            GdbRemoteThreadId::Any,
        )],
    );

    assert!(session
        .handle_packet(&GdbRemotePacket::new(b"Hc1a".to_vec()).unwrap())
        .is_ok());
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"S05".to_vec()).unwrap())
            .unwrap(),
        vec![GdbRemoteFrame::Ack],
    );
    assert_eq!(
        session.last_resume_request(),
        Some(&GdbRemoteResumeRequest::new(
            GdbRemoteResumeKind::SingleInstruction,
            Some(0x05),
            None,
            GdbRemoteThreadId::Id(0x1a),
        )),
    );
}

#[test]
fn gdb_remote_session_reports_vcont_actions() {
    let mut unsupported = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        unsupported
            .handle_packet(&GdbRemotePacket::new(b"vCont?".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(Vec::new()).unwrap()),
        ],
    );

    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"vContSupported".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"vCont?".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"vCont;c;C;s;S".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_records_vcont_resume_actions() {
    let mut unsupported = GdbRemoteSession::new(Vec::new());
    assert_eq!(
        unsupported
            .handle_packet(&GdbRemotePacket::new(b"vCont;c:1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(Vec::new()).unwrap()),
        ],
    );
    assert_eq!(unsupported.last_resume_requests(), &[]);

    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"vContSupported".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);
    session.set_stop_reply(GdbRemoteStopReply::signal(0x0b));

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"vCont;c:1a;S05".to_vec()).unwrap())
            .unwrap(),
        vec![GdbRemoteFrame::Ack],
    );
    assert_eq!(
        session.last_resume_requests(),
        &[
            GdbRemoteResumeRequest::new(
                GdbRemoteResumeKind::Continue,
                None,
                None,
                GdbRemoteThreadId::Id(0x1a),
            ),
            GdbRemoteResumeRequest::new(
                GdbRemoteResumeKind::SingleInstruction,
                Some(0x05),
                None,
                GdbRemoteThreadId::All,
            ),
        ],
    );
    assert_eq!(
        session.last_resume_request(),
        Some(&GdbRemoteResumeRequest::new(
            GdbRemoteResumeKind::SingleInstruction,
            Some(0x05),
            None,
            GdbRemoteThreadId::All,
        )),
    );
}

#[test]
fn gdb_remote_session_records_resume_requests_in_no_ack_mode_without_stop_reply() {
    let mut session = GdbRemoteSession::new(vec![
        GdbRemoteFeature::new(
            b"QStartNoAckMode".to_vec(),
            GdbRemoteFeatureValue::Supported,
        ),
        GdbRemoteFeature::new(b"vContSupported".to_vec(), GdbRemoteFeatureValue::Supported),
    ]);
    session
        .handle_packet(&GdbRemotePacket::new(b"QStartNoAckMode".to_vec()).unwrap())
        .unwrap();

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"vCont;s:1".to_vec()).unwrap())
            .unwrap(),
        Vec::<GdbRemoteFrame>::new(),
    );
    assert_eq!(
        session.last_resume_requests(),
        &[GdbRemoteResumeRequest::new(
            GdbRemoteResumeKind::SingleInstruction,
            None,
            None,
            GdbRemoteThreadId::Id(1),
        )],
    );
}

#[test]
fn gdb_remote_session_tracks_control_state_across_resume_traps_and_interrupts() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"vContSupported".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);
    let breakpoint = GdbRemoteTrapPoint::new(GdbRemoteTrapKind::HardwareBreakpoint, 0x1000, 4);
    let watchpoint = GdbRemoteTrapPoint::new(GdbRemoteTrapKind::AccessWatchpoint, 0x2000, 8);

    assert_eq!(session.control_state(), &GdbRemoteControlState::Stopped);
    session
        .handle_packet(&GdbRemotePacket::new(b"Z1,1000,4".to_vec()).unwrap())
        .unwrap();
    session
        .handle_packet(&GdbRemotePacket::new(b"Z4,2000,8".to_vec()).unwrap())
        .unwrap();
    assert_eq!(session.active_traps(), &[breakpoint, watchpoint]);
    assert_eq!(session.control_state(), &GdbRemoteControlState::Stopped);

    session
        .handle_packet(&GdbRemotePacket::new(b"vCont;s:1".to_vec()).unwrap())
        .unwrap();
    assert_eq!(
        session.control_state(),
        &GdbRemoteControlState::SingleInstruction {
            requests: vec![GdbRemoteResumeRequest::new(
                GdbRemoteResumeKind::SingleInstruction,
                None,
                None,
                GdbRemoteThreadId::Id(1),
            )],
        },
    );

    session.set_stop_reply(GdbRemoteStopReply::signal(0x05));
    assert_eq!(session.control_state(), &GdbRemoteControlState::Stopped);

    session
        .handle_packet(&GdbRemotePacket::new(b"c1000".to_vec()).unwrap())
        .unwrap();
    assert_eq!(
        session.control_state(),
        &GdbRemoteControlState::Continue {
            requests: vec![GdbRemoteResumeRequest::new(
                GdbRemoteResumeKind::Continue,
                None,
                Some(0x1000),
                GdbRemoteThreadId::Any,
            )],
        },
    );
    assert_eq!(session.active_traps(), &[breakpoint, watchpoint]);

    session.handle_frame(&GdbRemoteFrame::Interrupt).unwrap();
    assert_eq!(session.control_state(), &GdbRemoteControlState::Interrupted);
}

#[test]
fn gdb_remote_session_seeds_fixed_width_single_register_cache_from_flat_bytes() {
    let mut session = GdbRemoteSession::new(Vec::new());
    session.set_register_bytes(GdbRemoteRegisterBytes::new(vec![
        0x00, 0x01, 0x02, 0x03, 0x10, 0x11, 0x12, 0x13, 0x20, 0x21, 0x22, 0x23,
    ]));

    assert!(session.cache_fixed_width_register_values(0, 4, 3));
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p0".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"00010203".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"p2".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"20212223".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_records_disconnect_requests() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert!(!session.is_disconnected());
    assert_eq!(session.last_disconnect_request(), None);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"D;1a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert!(session.is_disconnected());
    assert_eq!(
        session.last_disconnect_request(),
        Some(&GdbRemoteDisconnectRequest::Detach {
            process_id: Some(0x1a),
        }),
    );

    let mut session = GdbRemoteSession::new(Vec::new());
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"vKill;2a".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session.last_disconnect_request(),
        Some(&GdbRemoteDisconnectRequest::TerminateProcess { process_id: 0x2a }),
    );
}

#[test]
fn gdb_remote_session_ignores_packets_after_disconnect() {
    let mut session = GdbRemoteSession::new(Vec::new());

    session
        .handle_packet(&GdbRemotePacket::new(b"D".to_vec()).unwrap())
        .unwrap();
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qC".to_vec()).unwrap())
            .unwrap(),
        Vec::<GdbRemoteFrame>::new(),
    );
    assert_eq!(
        session.last_disconnect_request(),
        Some(&GdbRemoteDisconnectRequest::Detach { process_id: None }),
    );
}

#[test]
fn gdb_remote_session_records_terminate_without_response_packet() {
    let mut session = GdbRemoteSession::new(Vec::new());

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"k".to_vec()).unwrap())
            .unwrap(),
        vec![GdbRemoteFrame::Ack],
    );
    assert!(session.is_disconnected());
    assert_eq!(
        session.last_disconnect_request(),
        Some(&GdbRemoteDisconnectRequest::Terminate),
    );
}

#[test]
fn gdb_remote_session_records_trap_requests_idempotently() {
    let mut session = GdbRemoteSession::new(Vec::new());
    let point = GdbRemoteTrapPoint::new(GdbRemoteTrapKind::HardwareBreakpoint, 0x1000, 4);

    assert_eq!(session.active_traps(), &[]);
    assert_eq!(session.last_trap_request(), None);
    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"Z1,1000,4".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(session.active_traps(), &[point]);
    assert_eq!(
        session.last_trap_request(),
        Some(&GdbRemoteTrapRequest::new(
            GdbRemoteTrapOperation::Insert,
            GdbRemoteTrapKind::HardwareBreakpoint,
            0x1000,
            4,
        )),
    );

    assert!(session
        .handle_packet(&GdbRemotePacket::new(b"Z1,1000,4".to_vec()).unwrap())
        .is_ok());
    assert_eq!(session.active_traps(), &[point]);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"z1,1000,4".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(session.active_traps(), &[]);
    assert_eq!(
        session.last_trap_request(),
        Some(&GdbRemoteTrapRequest::new(
            GdbRemoteTrapOperation::Remove,
            GdbRemoteTrapKind::HardwareBreakpoint,
            0x1000,
            4,
        )),
    );

    assert!(session
        .handle_packet(&GdbRemotePacket::new(b"z1,1000,4".to_vec()).unwrap())
        .is_ok());
    assert_eq!(session.active_traps(), &[]);
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
    let packet = GdbRemotePacket::new(b"qCRC:1000,4".to_vec()).unwrap();

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
fn gdb_remote_session_reports_xfer_feature_chunks() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"qXfer:features:read".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);
    assert!(session.set_xfer_feature(b"target.xml".to_vec(), b"abcdef".to_vec()));

    assert_eq!(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,3".to_vec()).unwrap(),
            )
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"mabc".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:3,3".to_vec()).unwrap(),
            )
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"ldef".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:6,4".to_vec()).unwrap(),
            )
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"l".to_vec()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_requires_advertised_xfer_feature_support() {
    let mut session = GdbRemoteSession::new(Vec::new());
    assert!(session.set_xfer_feature(b"target.xml".to_vec(), b"abcdef".to_vec()));

    assert_eq!(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,3".to_vec()).unwrap(),
            )
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(Vec::new()).unwrap()),
        ],
    );
}

#[test]
fn gdb_remote_session_reports_xfer_feature_errors_without_partial_data() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"qXfer:features:read".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);

    assert!(!session.set_xfer_feature(Vec::new(), b"abcdef".to_vec()));
    assert!(session.set_xfer_feature(b"target.xml".to_vec(), b"abcdef".to_vec()));

    for payload in [
        b"qXfer:features:read:missing.xml:0,3".as_slice(),
        b"qXfer:features:read:target.xml:7,3".as_slice(),
    ] {
        assert_eq!(
            session
                .handle_packet(&GdbRemotePacket::new(payload.to_vec()).unwrap())
                .unwrap(),
            vec![
                GdbRemoteFrame::Ack,
                GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
            ],
        );
    }
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
fn gdb_remote_session_handles_multiple_frames_from_byte_stream() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"vContSupported".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);
    let mut input = b"noise".to_vec();
    input.extend_from_slice(
        &GdbRemotePacket::new(b"Z1,1000,4".to_vec())
            .unwrap()
            .encode_frame(),
    );
    input.extend_from_slice(
        &GdbRemotePacket::new(b"vCont;s:1".to_vec())
            .unwrap()
            .encode_frame(),
    );
    input.push(0x03);
    input.extend_from_slice(b"tail");

    let result = session.handle_bytes(&input).unwrap();

    assert_eq!(result.consumed_bytes(), input.len());
    assert_eq!(result.skipped_bytes(), b"noisetail");
    assert_eq!(
        result.frames(),
        &[
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
            GdbRemoteFrame::Ack,
        ],
    );
    assert_eq!(
        session.active_traps(),
        &[GdbRemoteTrapPoint::new(
            GdbRemoteTrapKind::HardwareBreakpoint,
            0x1000,
            4,
        )],
    );
    assert_eq!(session.control_state(), &GdbRemoteControlState::Interrupted);
}

#[test]
fn gdb_remote_session_exposes_packet_stream_execution_control() {
    let mut session = GdbRemoteSession::new(vec![GdbRemoteFeature::new(
        b"vContSupported".to_vec(),
        GdbRemoteFeatureValue::Supported,
    )]);
    let breakpoint = GdbRemoteTrapPoint::new(GdbRemoteTrapKind::SoftwareBreakpoint, 0x1000, 4);
    let watchpoint = GdbRemoteTrapPoint::new(GdbRemoteTrapKind::WriteWatchpoint, 0x2000, 8);

    let mut first_stream = Vec::new();
    for payload in [
        b"Z0,1000,4".as_slice(),
        b"Z2,2000,8".as_slice(),
        b"vCont;c:1".as_slice(),
    ] {
        first_stream.extend_from_slice(
            &GdbRemotePacket::new(payload.to_vec())
                .unwrap()
                .encode_frame(),
        );
    }

    let first_result = session.handle_bytes(&first_stream).unwrap();

    assert_eq!(
        first_result.frames(),
        &[
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
            GdbRemoteFrame::Ack,
        ],
    );
    assert_eq!(
        session.execution_control(),
        &GdbRemoteExecutionControl::new(
            GdbRemoteControlState::Continue {
                requests: vec![GdbRemoteResumeRequest::new(
                    GdbRemoteResumeKind::Continue,
                    None,
                    None,
                    GdbRemoteThreadId::Id(1),
                )],
            },
            vec![breakpoint, watchpoint],
        ),
    );

    session.set_stop_reply(GdbRemoteStopReply::signal(0x05));
    let mut second_stream = Vec::new();
    for payload in [b"z0,1000,4".as_slice(), b"s".as_slice()] {
        second_stream.extend_from_slice(
            &GdbRemotePacket::new(payload.to_vec())
                .unwrap()
                .encode_frame(),
        );
    }

    let second_result = session.handle_bytes(&second_stream).unwrap();

    assert_eq!(
        second_result.frames(),
        &[
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
            GdbRemoteFrame::Ack,
        ],
    );
    assert_eq!(
        session.execution_control(),
        &GdbRemoteExecutionControl::new(
            GdbRemoteControlState::SingleInstruction {
                requests: vec![GdbRemoteResumeRequest::new(
                    GdbRemoteResumeKind::SingleInstruction,
                    None,
                    None,
                    GdbRemoteThreadId::Any,
                )],
            },
            vec![watchpoint],
        ),
    );
}

#[test]
fn gdb_remote_session_consumes_noise_only_byte_streams() {
    let mut session = GdbRemoteSession::new(Vec::new());

    let result = session.handle_bytes(b"noise").unwrap();

    assert_eq!(result.consumed_bytes(), b"noise".len());
    assert_eq!(result.skipped_bytes(), b"noise");
    assert!(result.frames().is_empty());
}

#[test]
fn gdb_remote_session_consumes_prefix_before_incomplete_packet() {
    let mut session = GdbRemoteSession::new(Vec::new());

    let result = session.handle_bytes(b"noise$qSupported").unwrap();

    assert_eq!(result.consumed_bytes(), b"noise".len());
    assert_eq!(result.skipped_bytes(), b"noise");
    assert!(result.frames().is_empty());
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
