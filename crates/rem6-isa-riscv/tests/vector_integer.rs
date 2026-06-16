use rem6_isa_riscv::{
    Register, RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind, RiscvVectorConfig,
    VectorRegister,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (1 << 25) | (u32::from(vs2) << 20) | (u32::from(vs1) << 15) | (u32::from(vd) << 7) | 0x57
}

fn lanes_u32(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn bytes_with_u16(lanes: [u16; 8]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 2..index * 2 + 2].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn bytes_with_u64(lanes: [u64; 2]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

#[test]
fn decoder_accepts_unmasked_vadd_vv() {
    assert_eq!(vadd_vv_type(2, 1, 3), 0x0220_81d7);
    assert_eq!(
        RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap(),
        RiscvInstruction::VectorAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
}

#[test]
fn hart_executes_vadd_vv_for_active_u32_lanes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write(reg(10), 3);
    hart.write_vector(vreg(1), lanes_u32([1, 2, u32::MAX, 40]));
    hart.write_vector(vreg(2), lanes_u32([10, 20, 2, 400]));
    hart.write_vector(
        vreg(3),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xdddd_dddd]),
    );

    hart.execute(RiscvInstruction::decode(vsetvli_type(0xd0, 10, 5)).unwrap())
        .unwrap();
    let record = hart
        .execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(3, 0xd0));
    assert_eq!(hart.pc(), 0x8008);
    assert_eq!(
        hart.read_vector(vreg(3)),
        lanes_u32([11, 22, 1, 0xdddd_dddd])
    );
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );
}

#[test]
fn hart_executes_vadd_vv_for_configured_element_widths() {
    let mut e8 = RiscvHartState::new(0x8100);
    e8.set_vector_config(RiscvVectorConfig::new(5, 0xc0));
    e8.write_vector(
        vreg(1),
        [255, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    );
    e8.write_vector(
        vreg(2),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    );
    e8.write_vector(vreg(3), [0xee; 16]);
    e8.execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        e8.read_vector(vreg(3)),
        [0, 3, 5, 7, 9, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee]
    );

    let mut e16 = RiscvHartState::new(0x8200);
    e16.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    e16.write_vector(
        vreg(1),
        bytes_with_u16([u16::MAX, 10, 30, 40, 50, 60, 70, 80]),
    );
    e16.write_vector(
        vreg(2),
        bytes_with_u16([2, 20, 300, 400, 500, 600, 700, 800]),
    );
    e16.write_vector(vreg(3), bytes_with_u16([0xbbbb; 8]));
    e16.execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        e16.read_vector(vreg(3)),
        bytes_with_u16([1, 30, 0xbbbb, 0xbbbb, 0xbbbb, 0xbbbb, 0xbbbb, 0xbbbb])
    );

    let mut e64 = RiscvHartState::new(0x8300);
    e64.set_vector_config(RiscvVectorConfig::new(1, 0xd8));
    e64.write_vector(vreg(1), bytes_with_u64([u64::MAX, 10]));
    e64.write_vector(vreg(2), bytes_with_u64([3, 20]));
    e64.write_vector(
        vreg(3),
        bytes_with_u64([0xaaaa_aaaa_aaaa_aaaa, 0xbbbb_bbbb_bbbb_bbbb]),
    );
    e64.execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        e64.read_vector(vreg(3)),
        bytes_with_u64([2, 0xbbbb_bbbb_bbbb_bbbb])
    );
}

#[test]
fn hart_executes_vadd_vv_across_lmul2_register_group() {
    let mut hart = RiscvHartState::new(0x8400);
    hart.set_vector_config(RiscvVectorConfig::new(6, 0xd1));
    hart.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));
    hart.write_vector(vreg(3), lanes_u32([5, 6, 7, 8]));
    hart.write_vector(vreg(4), lanes_u32([10, 20, 30, 40]));
    hart.write_vector(vreg(5), lanes_u32([50, 60, 70, 80]));
    hart.write_vector(
        vreg(6),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xaaaa_0003]),
    );
    hart.write_vector(
        vreg(7),
        lanes_u32([0xbbbb_0000, 0xbbbb_0001, 0xbbbb_0002, 0xbbbb_0003]),
    );

    hart.execute(RiscvInstruction::decode(vadd_vv_type(4, 2, 6)).unwrap())
        .unwrap();

    assert_eq!(hart.read_vector(vreg(6)), lanes_u32([11, 22, 33, 44]));
    assert_eq!(
        hart.read_vector(vreg(7)),
        lanes_u32([55, 66, 0xbbbb_0002, 0xbbbb_0003])
    );
}

#[test]
fn hart_traps_vadd_vv_for_unaligned_lmul2_register_group() {
    let mut hart = RiscvHartState::new(0x8500);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd1));
    hart.write_vector(
        vreg(3),
        lanes_u32([0xcccc_0000, 0xcccc_0001, 0xcccc_0002, 0xcccc_0003]),
    );

    let record = hart
        .execute(RiscvInstruction::decode(vadd_vv_type(4, 2, 3)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8500))
    );
    assert_eq!(
        hart.read_vector(vreg(3)),
        lanes_u32([0xcccc_0000, 0xcccc_0001, 0xcccc_0002, 0xcccc_0003])
    );
}

#[test]
fn hart_traps_vadd_vv_when_vector_type_is_invalid() {
    let mut hart = RiscvHartState::new(0x9000);

    let record = hart
        .execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x9000))
    );
    assert_eq!(hart.pc(), 0);
}
