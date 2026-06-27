use rem6_debug::{GdbRemoteCommand, GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatCsr, RiscvFloatStatus, RiscvGdbXlen, RiscvHartState,
    RiscvStatusWord, RiscvVectorConfig, RiscvVectorFixedRoundingMode, VectorRegister,
};
use rem6_system::{
    apply_riscv_gdb_remote_register_write, handle_riscv_gdb_remote_packet,
    riscv_gdb_remote_session_from_hart,
};

#[test]
fn riscv_gdb_remote_session_reports_rv32_hart_csr_snapshot_and_writes() {
    let mut hart = RiscvHartState::with_hart_id(0x8877_6655_4433_2211, 3);
    hart.write(Register::new(2).unwrap(), 0x0123_4567_89ab_cdef);
    hart.set_status(RiscvStatusWord::new(0x000c_0122));
    hart.set_supervisor_scratch(0x0102_0304_0506_0708);
    hart.set_translation_satp(0x1111_2222_3333_4444);
    hart.set_machine_interrupt_delegation(0x0000_0aaa);
    hart.set_machine_interrupt_enable(0x0000_4040);
    hart.set_machine_interrupt_pending(0x0000_7777);
    hart.set_machine_scratch(0x0f0e_0d0c_0b0a_0908);
    hart.set_supervisor_environment_config(0x0102_0304);

    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv32, &hart);

    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(registers.len(), rv32_register_hex_offset(147));
    assert_eq!(&registers[0..8], b"00000000");
    assert_eq!(&registers[2 * 8..3 * 8], b"efcdab89");
    assert_eq!(&registers[32 * 8..33 * 8], b"11223344");
    assert_eq!(&registers[rv32_register_hex_range(70)], b"22010c00");
    assert_eq!(&registers[rv32_register_hex_range(72)], b"08070605");
    assert_eq!(&registers[rv32_register_hex_range(76)], b"44443333");
    assert_eq!(&registers[rv32_register_hex_range(82)], b"08090a0b");
    assert_eq!(&registers[rv32_register_hex_range(126)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(127)], b"03000000");
    assert_eq!(&registers[rv32_register_hex_range(128)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(129)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(130)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(131)], b"2d111440");
    assert_eq!(&registers[rv32_register_hex_range(135)], b"04030201");
    assert_eq!(&registers[rv32_register_hex_range(136)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(137)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(138)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(139)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(140)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(141)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(142)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(146)], b"00000000");

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
                &GdbRemotePacket::new(b"P47=88776655".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"P4c=78563412".to_vec()).unwrap(),
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
                &GdbRemotePacket::new(b"p7a".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p7b".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"22020000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p83".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"2d111440",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p87".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"04030201",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P83=00000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P87=88776655".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_environment_config(), 0x5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p83".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"2d111440",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P7a=88080000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.machine_interrupt_enable(), 0x0000_48c8);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P7b=aa0a0000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.machine_interrupt_pending(), 0x0000_7fff);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P59=05000000".to_vec()).unwrap(),
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
fn riscv_gdb_remote_packet_handler_reads_and_writes_rv32d_float_registers_and_csrs() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.write_float(freg(0), 0x0123_4567_89ab_cdef);
    hart.write_float(freg(31), 0xfedc_ba98_7654_3210);
    hart.set_float_status(RiscvFloatStatus::new(0x85));
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv32, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p45".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00000000",
    );

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P43=03000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(RiscvFloatCsr::Frm.read(hart.float_status()), 3);

    let registers = packet_payload(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv32,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"g".to_vec()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(registers.len(), rv32_register_hex_offset(147));
    assert_eq!(&registers[rv32_register_hex_range(33)], b"8877665544332211");
    assert_eq!(&registers[rv32_register_hex_range(64)], b"1032547698badcfe");
    assert_eq!(&registers[rv32_register_hex_range(67)], b"03000000");
    assert_eq!(&registers[rv32_register_hex_range(69)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(126)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(127)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(128)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(129)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(130)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(131)], b"2d111440");
    assert_eq!(&registers[rv32_register_hex_range(135)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(136)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(137)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(138)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(139)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(141)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(142)], b"00000000");
    assert_eq!(&registers[rv32_register_hex_range(146)], b"00000000");
}

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_rv32_vector_registers() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.write_vector(vreg(0), vector_bytes(0x10));
    hart.write_vector(vreg(31), vector_bytes(0x80));
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv32, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
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
                RiscvGdbXlen::Rv32,
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
    assert_eq!(hart.read_float(freg(0)), 0x2000_0000_0000_0000);
    assert_eq!(hart.read_float(freg(31)), 0x2000_0000_0000_001f);
    assert_eq!(RiscvFloatCsr::Fcsr.read(hart.float_status()), 0x85);
    assert_eq!(hart.supervisor_scratch(), 0x3000_0002);
    assert_eq!(hart.translation_satp(), 0x3000_0006);
    assert_eq!(hart.machine_scratch(), 0x3000_000c);
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(hart.vector_fixed_point().vxsat());
    assert_eq!(hart.read_vector(vreg(0)), vector_bytes(0x40));
    assert_eq!(hart.read_vector(vreg(31)), vector_bytes(0x5f));
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(9, 0xc8));
    assert_eq!(hart.supervisor_environment_config(), 0x33);
}

fn rv32_register_hex_range(number: u64) -> std::ops::Range<usize> {
    let start = rv32_register_hex_offset(number);
    let end = rv32_register_hex_offset(number + 1);
    start..end
}

fn rv32_register_hex_offset(number: u64) -> usize {
    let byte_offset = match number {
        0..=32 => number * 4,
        33..=65 => (33 * 4) + ((number - 33) * 8),
        66..=69 => (33 * 4) + (32 * 8) + ((number - 66) * 4),
        70..=89 => (33 * 4) + (32 * 8) + (4 * 4) + ((number - 70) * 4),
        90..=121 => (33 * 4) + (32 * 8) + (4 * 4) + (20 * 4) + ((number - 90) * 16),
        122..=147 => (33 * 4) + (32 * 8) + (4 * 4) + (20 * 4) + (32 * 16) + ((number - 122) * 4),
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
    for register in 0..32_u64 {
        bytes.extend_from_slice(&(0x2000_0000_0000_0000_u64 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0x85_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for register in 0..17_u32 {
        bytes.extend_from_slice(&(0x3000_0000_u32 + register).to_le_bytes());
    }
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&5_u32.to_le_bytes());
    for register in 0..32_u8 {
        bytes.extend_from_slice(&vector_bytes(0x40 + register));
    }
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&7_u32.to_le_bytes());
    bytes.extend_from_slice(&11_u32.to_le_bytes());
    bytes.extend_from_slice(&7_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&9_u32.to_le_bytes());
    bytes.extend_from_slice(&0xc8_u32.to_le_bytes());
    bytes.extend_from_slice(&0x10_u32.to_le_bytes());
    bytes.extend_from_slice(&0x33_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes
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

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
