use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_debug::{GdbRemoteCommand, GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{Register, RiscvGdbXlen, RiscvHartState};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_system::{
    apply_riscv_gdb_remote_register_write, handle_riscv_gdb_remote_core_packet,
    handle_riscv_gdb_remote_packet, riscv_gdb_remote_session, riscv_gdb_remote_session_from_core,
    riscv_gdb_remote_session_from_hart, RiscvGdbRegisterWriteError, RiscvGdbRemotePacketError,
};
use rem6_transport::{MemoryRoute, MemoryTransport, TransportEndpointId};

#[test]
fn riscv_gdb_remote_session_advertises_target_description_xfer() {
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qSupported".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::new(b"PacketSize=4000;qXfer:features:read+".to_vec()).unwrap(),
            ),
        ],
    );
}

#[test]
fn riscv_gdb_remote_session_serves_rv64_target_documents() {
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    let target = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,400".to_vec()).unwrap(),
            )
            .unwrap(),
    );
    assert!(target.starts_with(b"l<?xml version=\"1.0\"?>\n"));
    let target = std::str::from_utf8(&target[1..]).unwrap();
    assert!(target.contains("<architecture>riscv</architecture>"));
    assert!(target.contains("<xi:include href=\"riscv-64bit-cpu.xml\"/>"));
    assert!(!target.contains("riscv-32bit-cpu.xml"));

    let cpu = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:riscv-64bit-cpu.xml:0,2000".to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let cpu = std::str::from_utf8(&cpu[1..]).unwrap();
    assert!(cpu.contains("<reg name=\"zero\" bitsize=\"64\" type=\"int\" regnum=\"0\"/>"));
    assert!(cpu.contains("<reg name=\"pc\" bitsize=\"64\" type=\"code_ptr\"/>"));
    assert!(!cpu.contains("bitsize=\"32\""));
}

#[test]
fn riscv_gdb_remote_session_serves_rv32_target_documents() {
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv32);

    let target = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,400".to_vec()).unwrap(),
            )
            .unwrap(),
    );
    let target = std::str::from_utf8(&target[1..]).unwrap();
    assert!(target.contains("<xi:include href=\"riscv-32bit-cpu.xml\"/>"));
    assert!(!target.contains("riscv-64bit-cpu.xml"));

    let cpu = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:riscv-32bit-cpu.xml:0,2000".to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let cpu = std::str::from_utf8(&cpu[1..]).unwrap();
    assert!(cpu.contains("<reg name=\"zero\" bitsize=\"32\" type=\"int\" regnum=\"0\"/>"));
    assert!(cpu.contains("<reg name=\"pc\" bitsize=\"32\" type=\"code_ptr\"/>"));
    assert!(!cpu.contains("bitsize=\"64\""));
}

#[test]
fn riscv_gdb_remote_session_reports_rv64_hart_register_snapshot() {
    let mut hart = RiscvHartState::with_hart_id(0x8877_6655_4433_2211, 0);
    hart.write(Register::new(1).unwrap(), 0x0123_4567_89ab_cdef);
    hart.write(Register::new(10).unwrap(), 0xfedc_ba98_7654_3210);

    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(registers.len(), 33 * 8 * 2);
    assert_eq!(&registers[0..16], b"0000000000000000");
    assert_eq!(&registers[16..32], b"efcdab8967452301");
    assert_eq!(&registers[10 * 16..11 * 16], b"1032547698badcfe");
    assert_eq!(&registers[32 * 16..33 * 16], b"1122334455667788");

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p20".to_vec()).unwrap())
                .unwrap(),
        ),
        b"1122334455667788",
    );
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
                number: 33,
                bytes: vec![0; 8],
            },
        ),
        Err(RiscvGdbRegisterWriteError::UnsupportedRegister { number: 33 }),
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
            &GdbRemotePacket::new(b"P21=0000000000000000".to_vec()).unwrap(),
        ),
        Err(RiscvGdbRemotePacketError::RegisterWrite(
            RiscvGdbRegisterWriteError::UnsupportedRegister { number: 33 },
        )),
    );
    assert_eq!(hart.pc(), 0x1000);
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p21".to_vec()).unwrap())
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
    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(&registers[0..16], b"0000000000000000");
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
fn riscv_gdb_remote_core_packet_handler_rejects_invalid_write_without_session_or_core_mutation() {
    let core = riscv_core(0x8000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &GdbRemotePacket::new(b"P21=0000000000000000".to_vec()).unwrap(),
        ),
        Err(RiscvGdbRemotePacketError::RegisterWrite(
            RiscvGdbRegisterWriteError::UnsupportedRegister { number: 33 },
        )),
    );
    assert_eq!(core.pc(), Address::new(0x8000));
    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p21".to_vec()).unwrap())
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

fn riscv_core(entry: u64) -> RiscvCore {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
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
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
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
