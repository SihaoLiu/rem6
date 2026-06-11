use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, Immediate, MemoryAccessKind, MemoryWidth, Register,
    RiscvExecutionRecord, RiscvHartState, RiscvInstruction,
};

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

#[test]
fn decoder_accepts_rv64d_load_store_and_add() {
    let cases = [
        (
            i_type(24, 2, 0x3, 5, 0x07),
            RiscvInstruction::FloatLoad {
                rd: freg(5),
                rs1: reg(2),
                offset: Immediate::new(24),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            s_type(-16, 6, 3, 0x3, 0x27),
            RiscvInstruction::FloatStore {
                rs1: reg(3),
                rs2: freg(6),
                offset: Immediate::new(-16),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            r_type(0x01, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatAddD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn hart_executes_faddd_and_records_float_write() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(0), 1.5f64.to_bits());
    hart.write_float(freg(2), 2.25f64.to_bits());

    let record = hart
        .execute(RiscvInstruction::FloatAddD {
            rd: freg(0),
            rs1: freg(0),
            rs2: freg(2),
        })
        .unwrap();

    let expected = 3.75f64.to_bits();
    assert_eq!(hart.read_float(freg(0)), expected);
    assert_eq!(record.pc(), 0x8000);
    assert_eq!(record.next_pc(), 0x8004);
    assert_eq!(
        record.float_register_writes(),
        &[FloatRegisterWrite::new(freg(0), expected)]
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_reports_float_load_store_memory_accesses() {
    let mut hart = RiscvHartState::new(0x9000);
    hart.write(reg(2), 0x8000);
    hart.write_float(freg(0), 1.25f64.to_bits());

    let load = hart
        .execute(RiscvInstruction::FloatLoad {
            rd: freg(0),
            rs1: reg(2),
            offset: Immediate::new(32),
            width: MemoryWidth::Doubleword,
        })
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::FloatLoad {
            rd: freg(0),
            address: 0x8020,
            width: MemoryWidth::Doubleword,
        })
    );

    let store = hart
        .execute(RiscvInstruction::FloatStore {
            rs1: reg(2),
            rs2: freg(0),
            offset: Immediate::new(-8),
            width: MemoryWidth::Doubleword,
        })
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::FloatStore {
            address: 0x7ff8,
            width: MemoryWidth::Doubleword,
            value: 1.25f64.to_bits(),
        })
    );
}

#[test]
fn execution_records_default_to_no_float_writes() {
    let record = RiscvExecutionRecord::new(
        RiscvInstruction::Addi {
            rd: reg(1),
            rs1: reg(0),
            imm: Immediate::new(1),
        },
        0x8000,
        0x8004,
        Vec::new(),
        None,
    );

    assert_eq!(record.float_register_writes(), &[]);
}
