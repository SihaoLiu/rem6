use rem6_debug::{GdbRemoteCommand, GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{
    Register, RiscvGdbXlen, RiscvHartState, RiscvStatusWord, RiscvVectorFixedRoundingMode,
};
use rem6_system::{
    apply_riscv_gdb_remote_register_write, handle_riscv_gdb_remote_packet,
    riscv_gdb_remote_session_from_hart,
};

#[test]
fn riscv_gdb_remote_session_reports_rv32_hart_csr_snapshot_and_writes() {
    let mut hart = RiscvHartState::new(0x8877_6655_4433_2211);
    hart.write(Register::new(2).unwrap(), 0x0123_4567_89ab_cdef);
    hart.set_status(RiscvStatusWord::new(0x000c_0122));
    hart.set_supervisor_scratch(0x0102_0304_0506_0708);
    hart.set_translation_satp(0x1111_2222_3333_4444);
    hart.set_machine_scratch(0x0f0e_0d0c_0b0a_0908);

    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv32, &hart);

    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(registers.len(), rv32_register_hex_offset(53));
    assert_eq!(&registers[0..8], b"00000000");
    assert_eq!(&registers[2 * 8..3 * 8], b"efcdab89");
    assert_eq!(&registers[32 * 8..33 * 8], b"11223344");
    assert_eq!(&registers[rv32_register_hex_range(33)], b"22010c00");
    assert_eq!(&registers[rv32_register_hex_range(35)], b"08070605");
    assert_eq!(&registers[rv32_register_hex_range(39)], b"44443333");
    assert_eq!(&registers[rv32_register_hex_range(45)], b"08090a0b");

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
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P22=88776655".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_trap_vector(), 0x5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P27=78563412".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.translation_satp(), 0x1234_5678);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P34=05000000".to_vec()).unwrap(),
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
}

#[test]
fn riscv_gdb_remote_register_write_applies_all_rv32_registers() {
    let mut hart = RiscvHartState::new(0);
    hart.write(Register::new(1).unwrap(), 0xffff_ffff);

    let bytes = rv32_register_set_write_bytes();

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
    assert_eq!(hart.supervisor_scratch(), 0x3000_0002);
    assert_eq!(hart.translation_satp(), 0x3000_0006);
    assert_eq!(hart.machine_scratch(), 0x3000_000c);
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(hart.vector_fixed_point().vxsat());
}

fn rv32_register_hex_range(number: u64) -> std::ops::Range<usize> {
    let start = rv32_register_hex_offset(number);
    let end = rv32_register_hex_offset(number + 1);
    start..end
}

fn rv32_register_hex_offset(number: u64) -> usize {
    let byte_offset = match number {
        0..=53 => number * 4,
        _ => panic!("unsupported RV32 GDB register number"),
    };
    byte_offset as usize * 2
}

fn rv32_register_set_write_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    for register in 0..32_u32 {
        bytes.extend_from_slice(&(0x1000_0000_u32 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&0x8000_0040_u32.to_le_bytes());
    for register in 0..17_u32 {
        bytes.extend_from_slice(&(0x3000_0000_u32 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&5_u32.to_le_bytes());
    bytes
}

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
