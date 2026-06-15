use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore};
use rem6_debug::{GdbRemoteCommand, GdbRemoteFrame, GdbRemotePacket, GdbRemoteThreadId};
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatCsr, RiscvFloatStatus, RiscvGdbXlen, RiscvHartState,
    RiscvStatusCsr, RiscvStatusWord,
};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
};
use rem6_system::{
    apply_riscv_gdb_remote_register_write, handle_riscv_gdb_remote_cluster_packet,
    handle_riscv_gdb_remote_core_packet, handle_riscv_gdb_remote_memory_packet,
    handle_riscv_gdb_remote_packet, handle_riscv_gdb_remote_system_packet,
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
    assert_eq!(registers.len(), rv64_register_hex_offset(74));
    assert_eq!(&registers[rv64_register_hex_range(33)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(64)], b"1032547698badcfe");
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
                &GdbRemotePacket::new(b"p41".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"p42".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"p43".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"P42=03000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(RiscvFloatCsr::Frm.read(hart.float_status()), 3);
}

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_advertised_rv64_csr_registers() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.set_status(RiscvStatusWord::new(0x0008_0000));
    hart.set_supervisor_trap_vector(0x0123_4567_89ab_cdef);
    hart.set_supervisor_scratch(0x0102_0304_0506_0708);
    hart.set_supervisor_exception_pc(0x1122_3344_5566_7788);
    hart.set_supervisor_trap_cause(0x8877_6655_4433_2211);
    hart.set_supervisor_trap_value(0xfedc_ba98_7654_3210);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

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
        b"0000080000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p49".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"P45=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_trap_vector(), 0x1122_3344_5566_7788);
    assert_eq!(RiscvStatusCsr::Sstatus.read(hart.status()), 0x0008_0000);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P46=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_scratch(), 0x1122_3344_5566_7788);

    let registers = packet_payload(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"g".to_vec()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(registers.len(), rv64_register_hex_offset(74));
    assert_eq!(&registers[rv64_register_hex_range(68)], b"0000080000000000");
    assert_eq!(&registers[rv64_register_hex_range(69)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(70)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(73)], b"1032547698badcfe");
}

#[test]
fn riscv_gdb_remote_session_reports_rv32_hart_register_snapshot_and_session_writes() {
    let mut hart = RiscvHartState::new(0x8877_6655_4433_2211);
    hart.write(Register::new(2).unwrap(), 0x0123_4567_89ab_cdef);

    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv32, &hart);

    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(registers.len(), 33 * 4 * 2);
    assert_eq!(&registers[0..8], b"00000000");
    assert_eq!(&registers[2 * 8..3 * 8], b"efcdab89");
    assert_eq!(&registers[32 * 8..33 * 8], b"11223344");

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"P20=78563412".to_vec()).unwrap())
                .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p20".to_vec()).unwrap())
                .unwrap(),
        ),
        b"78563412",
    );
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
fn riscv_gdb_remote_register_write_applies_all_rv32_registers() {
    let mut hart = RiscvHartState::new(0);
    hart.write(Register::new(1).unwrap(), 0xffff_ffff);

    let mut bytes = Vec::new();
    for register in 0..32_u32 {
        bytes.extend_from_slice(&(0x1000_0000_u32 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&0x8000_0040_u32.to_le_bytes());

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv32,
            &mut hart,
            &GdbRemoteCommand::WriteRegisters { bytes },
        ),
        Ok(true),
    );
    assert_eq!(hart.read(Register::new(0).unwrap()), 0);
    assert_eq!(hart.read(Register::new(1).unwrap()), 0x1000_0001);
    assert_eq!(hart.read(Register::new(31).unwrap()), 0x1000_001f);
    assert_eq!(hart.pc(), 0x8000_0040);
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
                number: 74,
                bytes: vec![0; 8],
            },
        ),
        Err(RiscvGdbRegisterWriteError::UnsupportedRegister { number: 74 }),
    );

    assert_eq!(
        apply_riscv_gdb_remote_register_write(
            RiscvGdbXlen::Rv32,
            &mut hart,
            &GdbRemoteCommand::WriteRegisters { bytes: vec![0; 8] },
        ),
        Err(RiscvGdbRegisterWriteError::InvalidRegisterSetBytes {
            expected: 33 * 4,
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
            &GdbRemotePacket::new(b"P4a=0000000000000000".to_vec()).unwrap(),
        ),
        Err(RiscvGdbRemotePacketError::RegisterWrite(
            RiscvGdbRegisterWriteError::UnsupportedRegister { number: 74 },
        )),
    );
    assert_eq!(hart.pc(), 0x1000);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p4a".to_vec()).unwrap())
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
    bytes.extend_from_slice(&0x000c_0122_u64.to_le_bytes());
    for register in 1..6_u64 {
        bytes.extend_from_slice(&(0x3000_0000_0000_0000_u64 + register).to_le_bytes());
    }

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
                .handle_packet(&GdbRemotePacket::new(b"p41".to_vec()).unwrap())
                .unwrap(),
        ),
        b"1f000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p42".to_vec()).unwrap())
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
        b"22010c0000000000",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p45".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0100000000000030",
    );
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p46".to_vec()).unwrap())
                .unwrap(),
        ),
        b"0200000000000030",
    );
    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(&registers[0..16], b"0000000000000000");
    assert_eq!(&registers[rv64_register_hex_range(33)], b"0000000000000020");
    assert_eq!(&registers[rv64_register_hex_range(65)], b"1f000000");
    assert_eq!(&registers[rv64_register_hex_range(66)], b"03000000");
    assert_eq!(&registers[rv64_register_hex_range(68)], b"22010c0000000000");
    assert_eq!(&registers[rv64_register_hex_range(69)], b"0100000000000030");
    assert_eq!(&registers[rv64_register_hex_range(70)], b"0200000000000030");
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
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p44".to_vec()).unwrap())
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
                &GdbRemotePacket::new(b"P45=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.supervisor_trap_vector(), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p45".to_vec()).unwrap())
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
                &GdbRemotePacket::new(b"P46=8899aabbccddeeff".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(core.supervisor_scratch(), 0xffee_ddcc_bbaa_9988);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p46".to_vec()).unwrap())
                .unwrap(),
        ),
        b"8899aabbccddeeff",
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
                .handle_packet(&GdbRemotePacket::new(b"p41".to_vec()).unwrap())
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
                &GdbRemotePacket::new(b"P43=a4000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(RiscvFloatCsr::Fcsr.read(core.float_status()), 0xa4);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p43".to_vec()).unwrap())
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
            &GdbRemotePacket::new(b"P4a=0000000000000000".to_vec()).unwrap(),
        ),
        Err(RiscvGdbRemotePacketError::RegisterWrite(
            RiscvGdbRegisterWriteError::UnsupportedRegister { number: 74 },
        )),
    );
    assert_eq!(core.pc(), Address::new(0x8000));
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p4a".to_vec()).unwrap())
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

#[test]
fn riscv_gdb_remote_memory_packet_handler_reads_partitioned_store_across_lines() {
    let mut store = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_memory_packet(
                &mut session,
                &mut store,
                &GdbRemotePacket::new(b"m100e,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"eeff1122",
    );
}

#[test]
fn riscv_gdb_remote_memory_packet_handler_writes_partitioned_store_across_lines() {
    let mut store = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        handle_riscv_gdb_remote_memory_packet(
            &mut session,
            &mut store,
            &GdbRemotePacket::new(b"M100e,4:aabbccdd".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_memory_packet(
                &mut session,
                &mut store,
                &GdbRemotePacket::new(b"m100c,8".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"ccddaabbccdd3344",
    );
}

#[test]
fn riscv_gdb_remote_memory_packet_handler_rejects_invalid_write_without_partial_update() {
    let mut store = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        handle_riscv_gdb_remote_memory_packet(
            &mut session,
            &mut store,
            &GdbRemotePacket::new(b"M101f,2:aabb".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_memory_packet(
                &mut session,
                &mut store,
                &GdbRemotePacket::new(b"m100c,8".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"ccddeeff11223344",
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

fn rv64_register_hex_range(number: u64) -> std::ops::Range<usize> {
    let start = rv64_register_hex_offset(number);
    let end = rv64_register_hex_offset(number + 1);
    start..end
}

fn rv64_register_hex_offset(number: u64) -> usize {
    let byte_offset = match number {
        0..=65 => number * 8,
        66..=68 => (65 * 8) + ((number - 65) * 4),
        69..=74 => (65 * 8) + (3 * 4) + ((number - 68) * 8),
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

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
