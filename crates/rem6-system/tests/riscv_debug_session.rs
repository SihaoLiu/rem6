use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore};
use rem6_debug::{GdbRemoteCommand, GdbRemoteFrame, GdbRemotePacket, GdbRemoteThreadId};
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatCsr, RiscvFloatStatus, RiscvGdbXlen, RiscvHartState,
    RiscvMachineTrapCsr, RiscvStatusWord, RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode,
    VectorRegister,
};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
};
use rem6_system::{
    apply_riscv_gdb_remote_register_write, handle_riscv_gdb_remote_cluster_packet,
    handle_riscv_gdb_remote_core_packet, handle_riscv_gdb_remote_packet,
    handle_riscv_gdb_remote_system_packet,
    handle_riscv_gdb_remote_system_packet_with_data_translation, riscv_gdb_remote_session,
    riscv_gdb_remote_session_from_cluster, riscv_gdb_remote_session_from_core,
    riscv_gdb_remote_session_from_hart, riscv_gdb_remote_session_with_page_table_dump,
    RiscvGdbRegisterWriteError, RiscvGdbRemotePacketError,
};
use rem6_transport::{MemoryRoute, MemoryTransport, TransportEndpointId};

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_rv64d_float_registers() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.write_float(freg(0), 0x0123_4567_89ab_cdef);
    hart.write_float(freg(31), 0xfedc_ba98_7654_3210);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p21".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"efcdab8967452301",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p40".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"1032547698badcfe",
    );

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P21=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.read_float(freg(0)), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p21".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"8877665544332211",
    );

    let registers = packet_payload(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"g".to_vec()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(registers.len(), rv64_register_hex_offset(124));
    assert_eq!(&registers[rv64_register_hex_range(33)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(64)], b"1032547698badcfe");
}

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_rv64_vector_registers() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.write_vector(vreg(0), vector_bytes(0x10));
    hart.write_vector(vreg(31), vector_bytes(0x80));
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p5a".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"101112131415161718191a1b1c1d1e1f",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p79".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"808182838485868788898a8b8c8d8e8f",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P5a=00112233445566778899aabbccddeeff".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        hart.read_vector(vreg(0)),
        [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ],
    );
}

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_advertised_rv64d_float_csr_registers() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.set_float_status(RiscvFloatStatus::new(0x85));
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p42".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"05000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p43".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"04000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p44".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"85000000",
    );

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P43=03000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(RiscvFloatCsr::Frm.read(hart.float_status()), 3);
}

#[test]
fn riscv_gdb_remote_register_write_applies_single_integer_and_pc_writes() {
    let mut hart = RiscvHartState::new(0x1000);

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv64,
            &mut hart,
            &GdbRemoteCommand::parse(
                &GdbRemotePacket::new(b"P1=efcdab8967452301".to_vec()).unwrap(),
            ),
        ),
        Ok(true),
    );
    assert_eq!(hart.read(Register::new(1).unwrap()), 0x0123_4567_89ab_cdef);

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv64,
            &mut hart,
            &GdbRemoteCommand::parse(
                &GdbRemotePacket::new(b"P20=8877665544332211".to_vec()).unwrap(),
            ),
        ),
        Ok(true),
    );
    assert_eq!(hart.pc(), 0x1122_3344_5566_7788);
}

#[test]
fn riscv_gdb_remote_register_write_reports_invalid_requests() {
    let mut hart = RiscvHartState::new(0x1000);

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv64,
            &mut hart,
            &GdbRemoteCommand::WriteRegister {
                number: 1,
                bytes: vec![0, 1, 2, 3],
            },
        ),
        Err(RiscvGdbRegisterWriteError::InvalidRegisterBytes {
            number: 1,
            expected: 8,
            actual: 4,
        }),
    );
    assert_eq!(hart.read(Register::new(1).unwrap()), 0);

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv64,
            &mut hart,
            &GdbRemoteCommand::WriteRegister {
                number: 124,
                bytes: vec![0; 8],
            },
        ),
        Err(RiscvGdbRegisterWriteError::UnsupportedRegister { number: 124 }),
    );

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv32,
            &mut hart,
            &GdbRemoteCommand::WriteRegisters { bytes: vec![0; 8] },
        ),
        Err(RiscvGdbRegisterWriteError::InvalidRegisterSetBytes {
            expected: 33 * 4 + 32 * 8 + 4 * 4 + 20 * 4 + 32 * 16 + 2 * 4,
            actual: 8,
        }),
    );

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv64,
            &mut hart,
            &GdbRemoteCommand::QueryStopReason,
        ),
        Ok(false),
    );
}

#[test]
fn riscv_gdb_remote_packet_handler_updates_session_and_hart() {
    let mut hart = RiscvHartState::new(0x1000);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"P1=efcdab8967452301".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(hart.read(Register::new(1).unwrap()), 0x0123_4567_89ab_cdef);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p1".to_vec()).unwrap())
                .unwrap(),
        ),
        b"efcdab8967452301",
    );
}

#[test]
fn riscv_gdb_remote_packet_handler_reports_writeback_errors() {
    let mut hart = RiscvHartState::new(0x1000);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"P7c=0000000000000000".to_vec()).unwrap(),
        ),
        Err(RiscvGdbRemotePacketError::RegisterWrite(
            RiscvGdbRegisterWriteError::UnsupportedRegister { number: 124 },
        )),
    );
    assert_eq!(hart.pc(), 0x1000);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p7c".to_vec()).unwrap())
                .unwrap(),
        ),
        b"E01",
    );
}

#[test]
fn riscv_gdb_remote_packet_handler_does_not_write_after_disconnect() {
    let mut hart = RiscvHartState::new(0x1000);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"D".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"P1=efcdab8967452301".to_vec()).unwrap(),
        )
        .unwrap(),
        Vec::<GdbRemoteFrame>::new(),
    );
    assert_eq!(hart.read(Register::new(1).unwrap()), 0);
}

#[test]
fn riscv_gdb_remote_packet_handler_canonicalizes_session_register_cache() {
    let mut hart = RiscvHartState::new(0x1000);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"P0=efcdab8967452301".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p0".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0000000000000000",
    );

    let bytes = rv64_register_set_write_bytes();

    assert_eq!(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &register_write_packet(&bytes),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p1".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0100000000000010",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p20".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0040008000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p21".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0000000000000020",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p42".to_vec()).unwrap())
                .unwrap(),
        ),
        b"1f000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p43".to_vec()).unwrap())
                .unwrap(),
        ),
        b"03000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p44".to_vec()).unwrap())
                .unwrap(),
        ),
        b"7f000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p45".to_vec()).unwrap())
                .unwrap(),
        ),
        b"00000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p46".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0200000000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p47".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0100000000000030",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p48".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0200000000000030",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p4c".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0600000000000030",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p4d".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0700000000000030",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p52".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0c00000000000030",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p57".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0100000000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p58".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0200000000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p59".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0500000000000000",
    );
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(hart.vector_fixed_point().vxsat());
    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(&registers[0..16], b"0000000000000000");
    assert_eq!(&registers[rv64_register_hex_range(33)], b"0000000000000020");
    assert_eq!(&registers[rv64_register_hex_range(66)], b"1f000000");
    assert_eq!(&registers[rv64_register_hex_range(67)], b"03000000");
    assert_eq!(&registers[rv64_register_hex_range(68)], b"7f000000");
    assert_eq!(&registers[rv64_register_hex_range(69)], b"00000000");
    assert_eq!(&registers[rv64_register_hex_range(70)], b"0200000000000000");
    assert_eq!(&registers[rv64_register_hex_range(71)], b"0100000000000030");
    assert_eq!(&registers[rv64_register_hex_range(72)], b"0200000000000030");
    assert_eq!(&registers[rv64_register_hex_range(76)], b"0600000000000030");
    assert_eq!(&registers[rv64_register_hex_range(77)], b"0700000000000030");
    assert_eq!(&registers[rv64_register_hex_range(82)], b"0c00000000000030");
    assert_eq!(&registers[rv64_register_hex_range(87)], b"0100000000000000");
    assert_eq!(&registers[rv64_register_hex_range(88)], b"0200000000000000");
    assert_eq!(&registers[rv64_register_hex_range(89)], b"0500000000000000");
    assert_eq!(
        &registers[rv64_register_hex_range(90)],
        b"404142434445464748494a4b4c4d4e4f"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(121)],
        b"5f606162636465666768696a6b6c6d6e"
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_serves_page_table_dump_payload() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session_with_page_table_dump(
        RiscvGdbXlen::Rv64,
        b"vpn=0x1000 ppn=0x2000 rwx\n".to_vec(),
    );

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b".".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"vpn=0x1000 ppn=0x2000 rwx\n",
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_serves_translation_map_page_table_dump() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        Address::new(0x4000),
        Address::new(0x8000),
        2,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet_with_data_translation(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &map,
                &GdbRemotePacket::new(b".".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"page_size=0x1000\nvaddr=0x4000 paddr=0x8000 pages=2 flags=r-x scope=non-global\n",
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_patches_software_breakpoints() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00112233",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Z0,1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"73001000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"z0,1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00112233",
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_patches_compressed_software_breakpoints() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Z0,1002,2".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00110290",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"z0,1002,2".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00112233",
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_preserves_original_breakpoint_bytes() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Z0,1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Z0,1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"z0,1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00112233",
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_rejects_unsupported_software_breakpoints() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Z0,1000,8".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E01",
    );
    assert!(session.active_traps().is_empty());
    assert_eq!(session.last_trap_request(), None);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00112233",
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_rejects_unmapped_software_breakpoints() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Z0,2000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E01",
    );
    assert!(session.active_traps().is_empty());
    assert_eq!(session.last_trap_request(), None);
}

#[test]
fn riscv_gdb_remote_system_packet_handler_ignores_unknown_software_breakpoint_removal() {
    let cluster = RiscvCluster::new([riscv_core(0x8000)]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"z0,1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert!(session.active_traps().is_empty());
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m1000,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00112233",
    );
}

#[test]
fn riscv_gdb_remote_session_reports_live_core_registers() {
    let core = riscv_core(0x8000);
    core.write_register(Register::new(1).unwrap(), 0x0123_4567_89ab_cdef);

    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(&registers[16..32], b"efcdab8967452301");
    assert_eq!(&registers[32 * 16..33 * 16], b"0080000000000000");
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p20".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0080000000000000",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_updates_core_registers_and_pc() {
    let core = riscv_core(0x8000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &GdbRemotePacket::new(b"P1=efcdab8967452301".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        core.read_register(Register::new(1).unwrap()),
        0x0123_4567_89ab_cdef
    );

    assert_eq!(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &GdbRemotePacket::new(b"P20=8877665544332211".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(core.pc(), Address::new(0x1122_3344_5566_7788));
    assert_eq!(core.inner().pc(), Address::new(0x1122_3344_5566_7788));
}

#[test]
fn riscv_gdb_remote_core_packet_handler_reads_and_writes_advertised_rv64_csr_registers() {
    let core = riscv_core(0x8000);
    core.set_status(RiscvStatusWord::new(0x0008_0000));
    core.set_supervisor_trap_vector(0x0123_4567_89ab_cdef);
    core.set_supervisor_scratch(0x0102_0304_0506_0708);
    core.set_translation_satp(0x1111_2222_3333_4444);
    core.set_machine_trap_csr(RiscvMachineTrapCsr::Mideleg, 0x0000_0000_0000_0aaa);
    core.set_machine_interrupt_enable(0x1010_2020_3030_4040);
    core.set_machine_interrupt_pending(0x1111_3333_5555_7777);
    core.set_machine_trap_csr(RiscvMachineTrapCsr::Mscratch, 0x1111_3333_5555_7777);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p46".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0000080000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P47=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.supervisor_trap_vector(), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p47".to_vec()).unwrap())
                .unwrap(),
        ),
        b"8877665544332211",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P48=8899aabbccddeeff".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.supervisor_scratch(), 0xffee_ddcc_bbaa_9988);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p48".to_vec()).unwrap())
                .unwrap(),
        ),
        b"8899aabbccddeeff",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P4c=0807060504030201".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.translation_satp(), 0x0102_0304_0506_0708);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p4c".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0807060504030201",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P52=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        core.machine_trap_csr(RiscvMachineTrapCsr::Mscratch),
        0x1122_3344_5566_7788
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p52".to_vec()).unwrap())
                .unwrap(),
        ),
        b"8877665544332211",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p7a".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P7a=8808000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.machine_interrupt_enable(), 0x1010_2020_3030_48c8);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p7b".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"2202000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P7b=aa0a000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.machine_interrupt_pending(), 0x1111_3333_5555_7fff);
}

#[test]
fn riscv_gdb_remote_core_packet_handler_reads_and_writes_rv64_vector_fixed_point_csrs() {
    let core = riscv_core(0x8000);
    let mut fixed = RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundToOdd);
    fixed.write_vxsat_bit(true);
    core.set_vector_fixed_point(fixed);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p57".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0100000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p58".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0300000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p59".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0700000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P59=0500000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        core.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(core.vector_fixed_point().vxsat());
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p59".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0500000000000000",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_applies_all_register_vector_fixed_point_csrs() {
    let core = riscv_core(0x8000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);
    let bytes = rv64_register_set_write_bytes();

    assert_eq!(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &register_write_packet(&bytes),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        core.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(core.vector_fixed_point().vxsat());
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p57".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0100000000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p58".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0200000000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p59".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0500000000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p5a".to_vec()).unwrap())
                .unwrap(),
        ),
        b"404142434445464748494a4b4c4d4e4f",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p79".to_vec()).unwrap())
                .unwrap(),
        ),
        b"5f606162636465666768696a6b6c6d6e",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_reads_and_writes_rv64_vector_registers() {
    let core = riscv_core(0x8000);
    core.write_vector_register(vreg(0), vector_bytes(0x20));
    core.write_vector_register(vreg(31), vector_bytes(0xa0));
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p5a".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"202122232425262728292a2b2c2d2e2f",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p79".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"a0a1a2a3a4a5a6a7a8a9aaabacadaeaf",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P79=ffeeddccbbaa99887766554433221100".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        core.read_vector_register(vreg(31)),
        [
            0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22,
            0x11, 0x00,
        ],
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_reads_and_writes_advertised_rv64d_float_csr_registers() {
    let core = riscv_core(0x8000);
    core.set_float_status(RiscvFloatStatus::new(0x85));
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p42".to_vec()).unwrap())
                .unwrap(),
        ),
        b"05000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P44=a4000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(RiscvFloatCsr::Fcsr.read(core.float_status()), 0xa4);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p44".to_vec()).unwrap())
                .unwrap(),
        ),
        b"a4000000",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_rejects_invalid_write_without_session_or_core_mutation() {
    let core = riscv_core(0x8000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &GdbRemotePacket::new(b"P7c=0000000000000000".to_vec()).unwrap(),
        ),
        Err(RiscvGdbRemotePacketError::RegisterWrite(
            RiscvGdbRegisterWriteError::UnsupportedRegister { number: 124 },
        )),
    );
    assert_eq!(core.pc(), Address::new(0x8000));
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p7c".to_vec()).unwrap())
                .unwrap(),
        ),
        b"E01",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_refreshes_before_live_reads() {
    let core = riscv_core(0x8000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    core.write_register(Register::new(1).unwrap(), 0x0123_4567_89ab_cdef);
    core.redirect_pc(Address::new(0x9000));

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"efcdab8967452301",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p20".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0090000000000000",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_does_not_write_after_disconnect() {
    let core = riscv_core(0x8000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert!(handle_riscv_gdb_remote_core_packet(
        RiscvGdbXlen::Rv64,
        &mut session,
        &core,
        &GdbRemotePacket::new(b"D".to_vec()).unwrap(),
    )
    .is_ok());
    assert_eq!(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &GdbRemotePacket::new(b"P1=efcdab8967452301".to_vec()).unwrap(),
        )
        .unwrap(),
        Vec::<GdbRemoteFrame>::new(),
    );
    assert_eq!(core.read_register(Register::new(1).unwrap()), 0);
}

#[test]
fn riscv_gdb_remote_session_from_cluster_reports_core_threads() {
    let core0 = riscv_core_with_id(0, 0x8000);
    core0.write_register(Register::new(1).unwrap(), 0x0123_4567_89ab_cdef);
    let core2 = riscv_core_with_id(2, 0x9000);
    let cluster = RiscvCluster::new([core2, core0]).unwrap();

    let mut session = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, &cluster).unwrap();

    assert_eq!(session.thread_ids(), &[1, 3]);
    assert_eq!(session.current_thread_id(), 1);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"qC".to_vec()).unwrap())
                .unwrap(),
        ),
        b"QC1",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"qfThreadInfo".to_vec()).unwrap())
                .unwrap(),
        ),
        b"m1,3",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"T3".to_vec()).unwrap())
                .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"T2".to_vec()).unwrap())
                .unwrap(),
        ),
        b"E01",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p1".to_vec()).unwrap())
                .unwrap(),
        ),
        b"efcdab8967452301",
    );
}

#[test]
fn riscv_gdb_remote_cluster_packet_handler_uses_selected_thread_core_registers() {
    let core0 = riscv_core_with_id(0, 0x8000);
    core0.write_register(Register::new(1).unwrap(), 0x0102_0304_0506_0708);
    let core2 = riscv_core_with_id(2, 0x9000);
    core2.write_register(Register::new(1).unwrap(), 0x1112_1314_1516_1718);
    let cluster = RiscvCluster::new([core2, core0]).unwrap();
    let mut session = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, &cluster).unwrap();

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"p1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0807060504030201",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hg3".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"p1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"1817161514131211",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"P1=a7a6a5a4a3a2a1a0".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );

    assert_eq!(
        cluster
            .core(CpuId::new(2))
            .unwrap()
            .read_register(Register::new(1).unwrap()),
        0xa0a1_a2a3_a4a5_a6a7,
    );
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(1).unwrap()),
        0x0102_0304_0506_0708,
    );
}

#[test]
fn riscv_gdb_remote_cluster_packet_handler_rejects_unknown_thread_selection() {
    let core0 = riscv_core_with_id(0, 0x8000);
    core0.write_register(Register::new(1).unwrap(), 0x0102_0304_0506_0708);
    let core2 = riscv_core_with_id(2, 0x9000);
    let cluster = RiscvCluster::new([core2, core0]).unwrap();
    let mut session = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, &cluster).unwrap();

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hg2".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E04",
    );
    assert_eq!(session.general_thread(), GdbRemoteThreadId::Any);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"p1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0807060504030201",
    );
}

#[test]
fn riscv_gdb_remote_cluster_packet_handler_applies_gem5_thread_selection_rules() {
    let core0 = riscv_core_with_id(0, 0x8000);
    let core2 = riscv_core_with_id(2, 0x9000);
    let cluster = RiscvCluster::new([core2, core0]).unwrap();
    let mut session = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, &cluster).unwrap();

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hg-1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E03",
    );
    assert_eq!(session.general_thread(), GdbRemoteThreadId::Any);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hc0".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E02",
    );
    assert_eq!(session.continue_thread(), GdbRemoteThreadId::Any);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hc3".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E02",
    );
    assert_eq!(session.continue_thread(), GdbRemoteThreadId::Any);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hc-1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(session.continue_thread(), GdbRemoteThreadId::All);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hg3".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(session.current_thread_id(), 3);
    assert_eq!(session.general_thread(), GdbRemoteThreadId::Id(3));
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"qC".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"QC3",
    );

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hg0".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(session.current_thread_id(), 3);
    assert_eq!(session.general_thread(), GdbRemoteThreadId::Any);
}

#[test]
fn riscv_gdb_remote_cluster_packet_handler_records_error_response_for_retransmit() {
    let core0 = riscv_core_with_id(0, 0x8000);
    let core2 = riscv_core_with_id(2, 0x9000);
    let cluster = RiscvCluster::new([core2, core0]).unwrap();
    let mut session = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, &cluster).unwrap();

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"qC".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"QC1",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_cluster_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &GdbRemotePacket::new(b"Hg2".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"E04",
    );
    assert_eq!(
        session.handle_frame(&GdbRemoteFrame::NegativeAck).unwrap(),
        vec![GdbRemoteFrame::Packet(
            GdbRemotePacket::new(b"E04".to_vec()).unwrap()
        )],
    );
}

#[test]
fn riscv_gdb_remote_system_packet_handler_combines_cluster_registers_and_memory() {
    let core0 = riscv_core_with_id(0, 0x8000);
    core0.write_register(Register::new(1).unwrap(), 0x0102_0304_0506_0708);
    let core2 = riscv_core_with_id(2, 0x9000);
    core2.write_register(Register::new(1).unwrap(), 0x1112_1314_1516_1718);
    let cluster = RiscvCluster::new([core2, core0]).unwrap();
    let mut memory = debug_memory_store();
    let mut session = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, &cluster).unwrap();

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"Hg3".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"p1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"1817161514131211",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m100e,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"eeff1122",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"M100e,4:aabbccdd".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"m100c,8".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"ccddaabbccdd3344",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_system_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &cluster,
                &mut memory,
                &GdbRemotePacket::new(b"p1".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"1817161514131211",
    );
}

fn riscv_core(entry: u64) -> RiscvCore {
    riscv_core_with_id(0, entry)
}

fn riscv_core_with_id(cpu: u32, entry: u64) -> RiscvCore {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint(&format!("cpu{cpu}.ifetch")),
                PartitionId::new(cpu),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(cpu),
                AgentId::new(7 + cpu),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(&format!("cpu{cpu}.ifetch")),
                route,
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vector_bytes(base: u8) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = base.wrapping_add(offset as u8);
    }
    bytes
}

fn rv64_register_hex_range(number: u64) -> std::ops::Range<usize> {
    let start = rv64_register_hex_offset(number);
    let end = rv64_register_hex_offset(number + 1);
    start..end
}

fn rv64_register_hex_offset(number: u64) -> usize {
    let byte_offset = match number {
        0..=32 => number * 8,
        33..=65 => (33 * 8) + ((number - 33) * 8),
        66..=69 => (33 * 8) + (32 * 8) + ((number - 66) * 4),
        70..=89 => (33 * 8) + (32 * 8) + (4 * 4) + ((number - 70) * 8),
        90..=122 => (33 * 8) + (32 * 8) + (4 * 4) + (20 * 8) + ((number - 90) * 16),
        123..=124 => (33 * 8) + (32 * 8) + (4 * 4) + (20 * 8) + (32 * 16) + ((number - 122) * 8),
        _ => panic!("unsupported RV64 GDB register number"),
    };
    byte_offset as usize * 2
}

fn debug_memory_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(0);
    let layout = CacheLineLayout::new(16).unwrap();
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout).unwrap();
    store
        .map_region(target, Address::new(0x1000), AccessSize::new(0x20).unwrap())
        .unwrap();
    store
        .insert_line(
            target,
            Address::new(0x1000),
            vec![
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ],
        )
        .unwrap();
    store
        .insert_line(
            target,
            Address::new(0x1010),
            vec![
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
                0xff, 0x00,
            ],
        )
        .unwrap();
    store
}

fn register_write_packet(bytes: &[u8]) -> GdbRemotePacket {
    let mut payload = b"G".to_vec();
    for byte in bytes {
        payload.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    GdbRemotePacket::new(payload).unwrap()
}

fn rv64_register_set_write_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    for register in 0..32_u64 {
        bytes.extend_from_slice(&(0x1000_0000_0000_0000_u64 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&0x8000_4000_u64.to_le_bytes());
    for register in 0..32_u64 {
        bytes.extend_from_slice(&(0x2000_0000_0000_0000_u64 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&0x1f_u32.to_le_bytes());
    bytes.extend_from_slice(&0x03_u32.to_le_bytes());
    bytes.extend_from_slice(&0x7f_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0x000c_0122_u64.to_le_bytes());
    for register in 1..17_u64 {
        bytes.extend_from_slice(&(0x3000_0000_0000_0000_u64 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&2_u64.to_le_bytes());
    bytes.extend_from_slice(&5_u64.to_le_bytes());
    for register in 0..32_u8 {
        bytes.extend_from_slice(&vector_bytes(0x40 + register));
    }
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes
}

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
