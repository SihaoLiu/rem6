use rem6_debug::{GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::RiscvGdbXlen;
use rem6_system::riscv_gdb_remote_session;

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

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
