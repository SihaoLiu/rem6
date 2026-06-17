use rem6_debug::{GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{
    RiscvGdbXlen, RiscvHartState, RiscvStatusCsr, RiscvStatusWord, RiscvVectorFixedPointState,
    RiscvVectorFixedRoundingMode,
};
use rem6_system::{handle_riscv_gdb_remote_packet, riscv_gdb_remote_session_from_hart};

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_advertised_rv64_csr_registers() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.set_status(RiscvStatusWord::new(0x0008_0000));
    hart.set_supervisor_trap_vector(0x0123_4567_89ab_cdef);
    hart.set_supervisor_scratch(0x0102_0304_0506_0708);
    hart.set_supervisor_exception_pc(0x1122_3344_5566_7788);
    hart.set_supervisor_trap_cause(0x8877_6655_4433_2211);
    hart.set_supervisor_trap_value(0xfedc_ba98_7654_3210);
    hart.set_translation_satp(0x1111_2222_3333_4444);
    hart.set_machine_interrupt_enable(0x1010_2020_3030_4040);
    hart.set_machine_trap_vector(0x0204_0608_0a0c_0e10);
    hart.set_machine_scratch(0x0f0e_0d0c_0b0a_0908);
    hart.set_machine_interrupt_pending(0x1111_3333_5555_7777);
    let mut vector_fixed_point =
        RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundToOdd);
    vector_fixed_point.write_vxsat_bit(true);
    hart.set_vector_fixed_point(vector_fixed_point);
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p46".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"p4b".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"p4c".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"4444333322221111",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p52".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"08090a0b0c0d0e0f",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p57".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0100000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p58".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0300000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p59".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0700000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P47=8877665544332211".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"P48=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_scratch(), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P4c=8899aabbccddeeff".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.translation_satp(), 0xffee_ddcc_bbaa_9988);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P52=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.machine_scratch(), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P58=0200000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P57=0000000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert!(!hart.vector_fixed_point().vxsat());
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P59=0500000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(hart.vector_fixed_point().vxsat());

    let registers = packet_payload(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"g".to_vec()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(registers.len(), rv64_register_hex_offset(90));
    assert_eq!(&registers[rv64_register_hex_range(70)], b"0000080000000000");
    assert_eq!(&registers[rv64_register_hex_range(71)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(72)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(75)], b"1032547698badcfe");
    assert_eq!(&registers[rv64_register_hex_range(76)], b"8899aabbccddeeff");
    assert_eq!(&registers[rv64_register_hex_range(80)], b"4040303020201010");
    assert_eq!(&registers[rv64_register_hex_range(81)], b"100e0c0a08060402");
    assert_eq!(&registers[rv64_register_hex_range(82)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(86)], b"7777555533331111");
    assert_eq!(&registers[rv64_register_hex_range(87)], b"0100000000000000");
    assert_eq!(&registers[rv64_register_hex_range(88)], b"0200000000000000");
    assert_eq!(&registers[rv64_register_hex_range(89)], b"0500000000000000");
}

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
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
        70..=90 => (33 * 8) + (32 * 8) + (4 * 4) + ((number - 70) * 8),
        _ => panic!("unexpected RV64 GDB register number {number}"),
    };
    byte_offset as usize * 2
}
