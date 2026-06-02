use rem6_debug::{GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{Register, RiscvGdbXlen, RiscvHartState};
use rem6_system::{riscv_gdb_remote_session, riscv_gdb_remote_session_from_hart};

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

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
