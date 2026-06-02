use rem6_isa_riscv::{
    AtomicMemoryOp, Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvCounterBank,
    RiscvCounterCsr, RiscvCounterSnapshot, RiscvCsrError, RiscvError, RiscvExecutionRecord,
    RiscvFenceSet, RiscvHartState, RiscvInstruction, RiscvMemoryOrdering, RiscvTrap, RiscvTrapKind,
};

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn atomic_type(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn fence_type(mode: u32, predecessor: u32, successor: u32, funct3: u32) -> u32 {
    (mode << 28) | (predecessor << 24) | (successor << 20) | (funct3 << 12) | 0x0f
}

fn csr_read_type(csr: u32, rd: u8) -> u32 {
    (csr << 20) | (0x2 << 12) | (u32::from(rd) << 7) | 0x73
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn shift_i_type(shamt: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (u32::from(shamt) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x13
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
}

fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
}

fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

#[test]
fn counter_csrs_preserve_machine_writes_and_user_read_aliases() {
    let mut counters = RiscvCounterBank::new();

    counters
        .write_machine(RiscvCounterCsr::Cycle, 0x1234_5678_9abc_def0)
        .unwrap();
    counters
        .write_machine(RiscvCounterCsr::Instret, 0x0102_0304_0506_0708)
        .unwrap();

    assert_eq!(
        counters.read_machine(RiscvCounterCsr::Cycle),
        0x1234_5678_9abc_def0
    );
    assert_eq!(
        counters.read_user(RiscvCounterCsr::Cycle),
        0x1234_5678_9abc_def0
    );
    assert_eq!(
        counters.read_machine(RiscvCounterCsr::Instret),
        0x0102_0304_0506_0708
    );
    assert_eq!(
        counters.read_user(RiscvCounterCsr::Instret),
        0x0102_0304_0506_0708
    );

    counters.add_cycles(0x10);
    counters.retire_instructions(3);
    assert_eq!(
        counters.read_machine(RiscvCounterCsr::Cycle),
        0x1234_5678_9abc_df00
    );
    assert_eq!(
        counters.read_machine(RiscvCounterCsr::Instret),
        0x0102_0304_0506_070b
    );
}

#[test]
fn counter_csrs_reject_user_writes_and_restore_snapshots() {
    let mut counters = RiscvCounterBank::new();
    assert_eq!(
        counters.write_user(RiscvCounterCsr::Cycle, 7).unwrap_err(),
        RiscvCsrError::ReadOnlyCounterAlias {
            csr: RiscvCounterCsr::Cycle,
        }
    );
    assert_eq!(
        RiscvCounterCsr::from_user_address(0xc00).unwrap(),
        RiscvCounterCsr::Cycle
    );
    assert_eq!(
        RiscvCounterCsr::from_machine_address(0xb02).unwrap(),
        RiscvCounterCsr::Instret
    );
    assert_eq!(
        RiscvCounterCsr::from_machine_address(0xc00).unwrap_err(),
        RiscvCsrError::UnknownCounterCsr { address: 0xc00 }
    );

    counters
        .write_machine(RiscvCounterCsr::Cycle, u64::MAX)
        .unwrap();
    counters.add_cycles(2);
    counters
        .write_machine(RiscvCounterCsr::Instret, 0xfeed)
        .unwrap();
    let snapshot = counters.snapshot();
    assert_eq!(snapshot, RiscvCounterSnapshot::new(1, 0xfeed));

    let mut restored = RiscvCounterBank::new();
    restored.restore(&snapshot);
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.read_user(RiscvCounterCsr::Cycle), 1);
    assert_eq!(restored.read_user(RiscvCounterCsr::Instret), 0xfeed);
}

#[test]
fn hart_reads_machine_hart_id_csr() {
    let instruction = RiscvInstruction::decode(csr_read_type(0xf14, 5)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::ReadMachineHartId { rd: reg(5) }
    );

    let mut hart = RiscvHartState::with_hart_id(0x2000, 7);
    let record = hart.execute(instruction).unwrap();

    assert_eq!(hart.hart_id(), 7);
    assert_eq!(hart.read(reg(5)), 7);
    assert_eq!(record.next_pc(), 0x2004);
}

#[test]
fn hart_reads_cycle_and_instret_counter_csrs() {
    let addi = RiscvInstruction::decode(i_type(9, 0, 0x0, 7, 0x13)).unwrap();
    let cycle = RiscvInstruction::decode(csr_read_type(0xc00, 5)).unwrap();
    let instret = RiscvInstruction::decode(csr_read_type(0xc02, 6)).unwrap();
    assert_eq!(
        cycle,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(5),
            csr: RiscvCounterCsr::Cycle,
        }
    );
    assert_eq!(
        instret,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(6),
            csr: RiscvCounterCsr::Instret,
        }
    );

    let mut hart = RiscvHartState::new(0x2400);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(0, 0));
    hart.execute(addi).unwrap();
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(1, 1));

    hart.execute(cycle).unwrap();
    hart.execute(instret).unwrap();

    assert_eq!(hart.read(reg(5)), 1);
    assert_eq!(hart.read(reg(6)), 2);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(3, 3));
}

#[test]
fn decoder_extracts_rv64i_fields_and_immediates() {
    assert_eq!(
        RiscvInstruction::decode(i_type(-1, 0, 0x0, 5, 0x13)).unwrap(),
        RiscvInstruction::Addi {
            rd: reg(5),
            rs1: reg(0),
            imm: Immediate::new(-1),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(u_type(0x1234_5000, 10, 0x37)).unwrap(),
        RiscvInstruction::Lui {
            rd: reg(10),
            imm: Immediate::new(0x1234_5000),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x20, 1, 2, 0x0, 3, 0x33)).unwrap(),
        RiscvInstruction::Sub {
            rd: reg(3),
            rs1: reg(2),
            rs2: reg(1),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(0x180, 3, 0x6, 3, 0x13)).unwrap(),
        RiscvInstruction::Ori {
            rd: reg(3),
            rs1: reg(3),
            imm: Immediate::new(0x180),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(-2048, 3, 0x6, 3, 0x13)).unwrap(),
        RiscvInstruction::Ori {
            rd: reg(3),
            rs1: reg(3),
            imm: Immediate::new(-2048),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(0x02, 3, 0x4, 3, 0x13)).unwrap(),
        RiscvInstruction::Xori {
            rd: reg(3),
            rs1: reg(3),
            imm: Immediate::new(0x02),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(-2, 3, 0x4, 3, 0x13)).unwrap(),
        RiscvInstruction::Xori {
            rd: reg(3),
            rs1: reg(3),
            imm: Immediate::new(-2),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(0x03, 3, 0x7, 3, 0x13)).unwrap(),
        RiscvInstruction::Andi {
            rd: reg(3),
            rs1: reg(3),
            imm: Immediate::new(0x03),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(-16, 3, 0x7, 3, 0x13)).unwrap(),
        RiscvInstruction::Andi {
            rd: reg(3),
            rs1: reg(3),
            imm: Immediate::new(-16),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(shift_i_type(40, 3, 0x1, 3)).unwrap(),
        RiscvInstruction::Slli {
            rd: reg(3),
            rs1: reg(3),
            shamt: 40,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(shift_i_type(1, 3, 0x5, 3)).unwrap(),
        RiscvInstruction::Srli {
            rd: reg(3),
            rs1: reg(3),
            shamt: 1,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(b_type(-8, 6, 5, 0x1)).unwrap(),
        RiscvInstruction::Bne {
            rs1: reg(5),
            rs2: reg(6),
            offset: Immediate::new(-8),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(j_type(2048, 1)).unwrap(),
        RiscvInstruction::Jal {
            rd: reg(1),
            offset: Immediate::new(2048),
        }
    );
}

#[test]
fn hart_executes_integer_register_operations_and_keeps_zero_readonly() {
    let mut hart = RiscvHartState::new(0x8000);

    let first = hart
        .execute(RiscvInstruction::decode(i_type(5, 0, 0x0, 1, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.pc(), 0x8004);
    assert_eq!(hart.read(reg(1)), 5);
    assert_eq!(
        first,
        RiscvExecutionRecord::new(
            RiscvInstruction::Addi {
                rd: reg(1),
                rs1: reg(0),
                imm: Immediate::new(5),
            },
            0x8000,
            0x8004,
            vec![rem6_isa_riscv::RegisterWrite::new(reg(1), 5)],
            None,
        )
    );

    let ignored = hart
        .execute(RiscvInstruction::decode(i_type(7, 1, 0x0, 0, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(0)), 0);
    assert_eq!(ignored.register_writes(), &[]);

    hart.execute(RiscvInstruction::decode(r_type(0, 1, 1, 0x0, 2, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(2)), 10);

    hart.execute(RiscvInstruction::decode(r_type(0x20, 1, 2, 0x0, 3, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(3)), 5);

    hart.execute(RiscvInstruction::decode(i_type(-1, 0, 0x0, 4, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(4)), u64::MAX);
    hart.execute(RiscvInstruction::decode(i_type(1, 4, 0x0, 5, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 0);

    hart.execute(RiscvInstruction::decode(i_type(1, 0, 0x0, 6, 0x13)).unwrap())
        .unwrap();
    hart.execute(RiscvInstruction::decode(shift_i_type(40, 6, 0x1, 6)).unwrap())
        .unwrap();
    hart.execute(RiscvInstruction::decode(i_type(0x180, 6, 0x6, 6, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(6)), (1_u64 << 40) | 0x180);

    hart.write(reg(7), 0b1010);
    hart.execute(RiscvInstruction::decode(i_type(0b1100, 7, 0x4, 7, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(7)), 0b0110);
    hart.execute(RiscvInstruction::decode(i_type(0b0011, 7, 0x7, 7, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(7)), 0b0010);
    hart.execute(RiscvInstruction::decode(shift_i_type(1, 7, 0x5, 7)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(7)), 0b0001);

    hart.write(reg(8), 0x55aa_00ff);
    hart.execute(RiscvInstruction::decode(i_type(-2, 8, 0x4, 8, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(8)), 0xffff_ffff_aa55_ff01);

    hart.write(reg(9), 0x1234_5678_9abc_def0);
    hart.execute(RiscvInstruction::decode(i_type(-16, 9, 0x7, 9, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(9)), 0x1234_5678_9abc_def0 & !0x0f);

    hart.write(reg(10), 0);
    hart.execute(RiscvInstruction::decode(i_type(-2048, 10, 0x6, 10, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(10)), 0xffff_ffff_ffff_f800);
}

#[test]
fn hart_executes_upper_immediate_jumps_and_branches() {
    let mut hart = RiscvHartState::new(0x1000);

    hart.execute(RiscvInstruction::decode(u_type(0x1234_5000, 4, 0x37)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(4)), 0x1234_5000);

    hart.execute(RiscvInstruction::decode(u_type(0x0001_0000, 5, 0x17)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 0x0001_1004);

    hart.write(reg(6), 9);
    hart.write(reg(7), 9);
    let taken = hart
        .execute(RiscvInstruction::decode(b_type(16, 7, 6, 0x0)).unwrap())
        .unwrap();
    assert_eq!(taken.next_pc(), 0x1018);

    let jump = hart
        .execute(RiscvInstruction::decode(j_type(-8, 1)).unwrap())
        .unwrap();
    assert_eq!(jump.next_pc(), 0x1010);
    assert_eq!(hart.read(reg(1)), 0x101c);

    hart.write(reg(8), 0x2003);
    let jalr = hart
        .execute(RiscvInstruction::decode(i_type(4, 8, 0x0, 1, 0x67)).unwrap())
        .unwrap();
    assert_eq!(jalr.next_pc(), 0x2006);
    assert_eq!(hart.read(reg(1)), 0x1014);
}

#[test]
fn hart_reports_memory_accesses_without_mutating_memory() {
    let mut hart = RiscvHartState::new(0x4000);
    hart.write(reg(2), 0x8000);
    hart.write(reg(3), 0x1122_3344_5566_7788);

    let load = hart
        .execute(RiscvInstruction::decode(i_type(24, 2, 0x3, 9, 0x03)).unwrap())
        .unwrap();
    assert_eq!(load.next_pc(), 0x4004);
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::Load {
            rd: reg(9),
            address: 0x8018,
            width: MemoryWidth::Doubleword,
            signed: true,
        })
    );
    assert_eq!(hart.read(reg(9)), 0);

    let store = hart
        .execute(RiscvInstruction::decode(s_type(-16, 3, 2, 0x2)).unwrap())
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::Store {
            address: 0x7ff0,
            width: MemoryWidth::Word,
            value: 0x1122_3344_5566_7788,
        })
    );
}

#[test]
fn hart_reports_load_reserved_access_without_mutating_register() {
    let mut hart = RiscvHartState::new(0x5000);
    hart.write(reg(2), 0x9008);

    let instruction =
        RiscvInstruction::decode(atomic_type(0x02, true, false, 0, 2, 0x3, 5)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::LoadReserved {
            rd: reg(5),
            rs1: reg(2),
            width: MemoryWidth::Doubleword,
            acquire: true,
            release: false,
        }
    );

    let load_reserved = hart.execute(instruction).unwrap();
    assert_eq!(load_reserved.next_pc(), 0x5004);
    assert_eq!(
        load_reserved.memory_access(),
        Some(&MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            acquire: true,
            release: false,
        })
    );
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_reports_store_conditional_access_without_mutating_register() {
    let mut hart = RiscvHartState::new(0x6000);
    hart.write(reg(2), 0x9008);
    hart.write(reg(6), 0x0102_0304_0506_0708);

    let instruction =
        RiscvInstruction::decode(atomic_type(0x03, false, true, 6, 2, 0x3, 7)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::StoreConditional {
            rd: reg(7),
            rs1: reg(2),
            rs2: reg(6),
            width: MemoryWidth::Doubleword,
            acquire: false,
            release: true,
        }
    );

    let store_conditional = hart.execute(instruction).unwrap();
    assert_eq!(store_conditional.next_pc(), 0x6004);
    assert_eq!(
        store_conditional.memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x0102_0304_0506_0708,
            acquire: false,
            release: true,
        })
    );
    assert_eq!(hart.read(reg(7)), 0);
}

#[test]
fn hart_reports_atomic_swap_access_without_mutating_register() {
    let mut hart = RiscvHartState::new(0x6400);
    hart.write(reg(2), 0x9008);
    hart.write(reg(6), 0x0102_0304_0506_0708);

    let instruction =
        RiscvInstruction::decode(atomic_type(0x01, true, true, 6, 2, 0x3, 7)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::AtomicMemory {
            rd: reg(7),
            rs1: reg(2),
            rs2: reg(6),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Swap,
            acquire: true,
            release: true,
        }
    );

    let atomic = hart.execute(instruction).unwrap();
    assert_eq!(atomic.next_pc(), 0x6404);
    assert_eq!(
        atomic.memory_access(),
        Some(&MemoryAccessKind::AtomicMemory {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Swap,
            value: 0x0102_0304_0506_0708,
            acquire: true,
            release: true,
        })
    );
    assert_eq!(hart.read(reg(7)), 0);
}

#[test]
fn hart_reports_atomic_add_access_without_mutating_register() {
    let mut hart = RiscvHartState::new(0x6800);
    hart.write(reg(2), 0x9008);
    hart.write(reg(6), 0x0102_0304_0506_0708);

    let instruction =
        RiscvInstruction::decode(atomic_type(0x00, false, true, 6, 2, 0x3, 7)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::AtomicMemory {
            rd: reg(7),
            rs1: reg(2),
            rs2: reg(6),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Add,
            acquire: false,
            release: true,
        }
    );

    let atomic = hart.execute(instruction).unwrap();
    assert_eq!(atomic.next_pc(), 0x6804);
    assert_eq!(
        atomic.memory_access(),
        Some(&MemoryAccessKind::AtomicMemory {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Add,
            value: 0x0102_0304_0506_0708,
            acquire: false,
            release: true,
        })
    );
    assert_eq!(hart.read(reg(7)), 0);
}

#[test]
fn hart_reports_atomic_logical_accesses_without_mutating_register() {
    let cases = [
        (0x04, AtomicMemoryOp::Xor, 0x6c00),
        (0x08, AtomicMemoryOp::Or, 0x7000),
        (0x0c, AtomicMemoryOp::And, 0x7400),
    ];

    for (funct5, op, pc) in cases {
        let mut hart = RiscvHartState::new(pc);
        hart.write(reg(2), 0x9008);
        hart.write(reg(6), 0x0ff0_f00f_5555_3333);

        let instruction =
            RiscvInstruction::decode(atomic_type(funct5, true, false, 6, 2, 0x3, 7)).unwrap();
        assert_eq!(
            instruction,
            RiscvInstruction::AtomicMemory {
                rd: reg(7),
                rs1: reg(2),
                rs2: reg(6),
                width: MemoryWidth::Doubleword,
                op,
                acquire: true,
                release: false,
            }
        );

        let atomic = hart.execute(instruction).unwrap();
        assert_eq!(atomic.next_pc(), pc + 4);
        assert_eq!(
            atomic.memory_access(),
            Some(&MemoryAccessKind::AtomicMemory {
                rd: reg(7),
                address: 0x9008,
                width: MemoryWidth::Doubleword,
                op,
                value: 0x0ff0_f00f_5555_3333,
                acquire: true,
                release: false,
            })
        );
        assert_eq!(hart.read(reg(7)), 0);
    }
}

#[test]
fn hart_reports_atomic_min_max_accesses_without_mutating_register() {
    let cases = [
        (0x10, AtomicMemoryOp::MinSigned, 0x7800),
        (0x14, AtomicMemoryOp::MaxSigned, 0x7c00),
        (0x18, AtomicMemoryOp::MinUnsigned, 0x8000),
        (0x1c, AtomicMemoryOp::MaxUnsigned, 0x8400),
    ];

    for (funct5, op, pc) in cases {
        let mut hart = RiscvHartState::new(pc);
        hart.write(reg(2), 0x9008);
        hart.write(reg(6), 7);

        let instruction =
            RiscvInstruction::decode(atomic_type(funct5, false, true, 6, 2, 0x3, 7)).unwrap();
        assert_eq!(
            instruction,
            RiscvInstruction::AtomicMemory {
                rd: reg(7),
                rs1: reg(2),
                rs2: reg(6),
                width: MemoryWidth::Doubleword,
                op,
                acquire: false,
                release: true,
            }
        );

        let atomic = hart.execute(instruction).unwrap();
        assert_eq!(atomic.next_pc(), pc + 4);
        assert_eq!(
            atomic.memory_access(),
            Some(&MemoryAccessKind::AtomicMemory {
                rd: reg(7),
                address: 0x9008,
                width: MemoryWidth::Doubleword,
                op,
                value: 7,
                acquire: false,
                release: true,
            })
        );
        assert_eq!(hart.read(reg(7)), 0);
    }
}

#[test]
fn hart_reports_word_reserved_accesses_without_mutating_register() {
    let mut hart = RiscvHartState::new(0x8800);
    hart.write(reg(2), 0x9008);
    hart.write(reg(6), 0x0102_0304_8506_0708);

    let load_reserved =
        RiscvInstruction::decode(atomic_type(0x02, true, false, 0, 2, 0x2, 5)).unwrap();
    assert_eq!(
        load_reserved,
        RiscvInstruction::LoadReserved {
            rd: reg(5),
            rs1: reg(2),
            width: MemoryWidth::Word,
            acquire: true,
            release: false,
        }
    );
    let load_record = hart.execute(load_reserved).unwrap();
    assert_eq!(load_record.next_pc(), 0x8804);
    assert_eq!(
        load_record.memory_access(),
        Some(&MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Word,
            acquire: true,
            release: false,
        })
    );
    assert_eq!(hart.read(reg(5)), 0);

    let store_conditional =
        RiscvInstruction::decode(atomic_type(0x03, false, true, 6, 2, 0x2, 7)).unwrap();
    assert_eq!(
        store_conditional,
        RiscvInstruction::StoreConditional {
            rd: reg(7),
            rs1: reg(2),
            rs2: reg(6),
            width: MemoryWidth::Word,
            acquire: false,
            release: true,
        }
    );
    let store_record = hart.execute(store_conditional).unwrap();
    assert_eq!(store_record.next_pc(), 0x8808);
    assert_eq!(
        store_record.memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Word,
            value: 0x0102_0304_8506_0708,
            acquire: false,
            release: true,
        })
    );
    assert_eq!(hart.read(reg(7)), 0);
}

#[test]
fn hart_reports_word_atomic_accesses_without_mutating_register() {
    let cases = [
        (0x00, AtomicMemoryOp::Add, 0x8c00),
        (0x01, AtomicMemoryOp::Swap, 0x9000),
        (0x04, AtomicMemoryOp::Xor, 0x9400),
        (0x08, AtomicMemoryOp::Or, 0x9800),
        (0x0c, AtomicMemoryOp::And, 0x9c00),
        (0x10, AtomicMemoryOp::MinSigned, 0xa000),
        (0x14, AtomicMemoryOp::MaxSigned, 0xa400),
        (0x18, AtomicMemoryOp::MinUnsigned, 0xa800),
        (0x1c, AtomicMemoryOp::MaxUnsigned, 0xac00),
    ];

    for (funct5, op, pc) in cases {
        let mut hart = RiscvHartState::new(pc);
        hart.write(reg(2), 0x9008);
        hart.write(reg(6), 0x0102_0304_8506_0708);

        let instruction =
            RiscvInstruction::decode(atomic_type(funct5, true, true, 6, 2, 0x2, 7)).unwrap();
        assert_eq!(
            instruction,
            RiscvInstruction::AtomicMemory {
                rd: reg(7),
                rs1: reg(2),
                rs2: reg(6),
                width: MemoryWidth::Word,
                op,
                acquire: true,
                release: true,
            }
        );

        let atomic = hart.execute(instruction).unwrap();
        assert_eq!(atomic.next_pc(), pc + 4);
        assert_eq!(
            atomic.memory_access(),
            Some(&MemoryAccessKind::AtomicMemory {
                rd: reg(7),
                address: 0x9008,
                width: MemoryWidth::Word,
                op,
                value: 0x0102_0304_8506_0708,
                acquire: true,
                release: true,
            })
        );
        assert_eq!(hart.read(reg(7)), 0);
    }
}

#[test]
fn hart_reports_fence_barriers_without_memory_or_register_side_effects() {
    let mut hart = RiscvHartState::new(0xb000);
    hart.write(reg(1), 0x1234);

    let fence = RiscvInstruction::decode(fence_type(0, 0b1010, 0b0101, 0x0)).unwrap();
    assert_eq!(
        fence,
        RiscvInstruction::Fence {
            predecessor: RiscvFenceSet::new(true, false, true, false),
            successor: RiscvFenceSet::new(false, true, false, true),
            mode: 0,
        }
    );
    let fence_record = hart.execute(fence).unwrap();
    assert_eq!(fence_record.next_pc(), 0xb004);
    assert_eq!(fence_record.register_writes(), &[]);
    assert_eq!(fence_record.memory_access(), None);
    assert_eq!(hart.read(reg(1)), 0x1234);

    let fence_i = RiscvInstruction::decode(fence_type(0, 0, 0, 0x1)).unwrap();
    assert_eq!(fence_i, RiscvInstruction::FenceI);
    let fence_i_record = hart.execute(fence_i).unwrap();
    assert_eq!(fence_i_record.next_pc(), 0xb008);
    assert_eq!(fence_i_record.register_writes(), &[]);
    assert_eq!(fence_i_record.memory_access(), None);
    assert_eq!(hart.read(reg(1)), 0x1234);
}

#[test]
fn atomic_memory_accesses_report_aq_rl_barrier_ordering() {
    let no_ordering = MemoryAccessKind::Load {
        rd: reg(5),
        address: 0x9008,
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    assert_eq!(no_ordering.memory_ordering(), RiscvMemoryOrdering::none());

    let release_only = MemoryAccessKind::StoreConditional {
        rd: reg(7),
        address: 0x9008,
        width: MemoryWidth::Doubleword,
        value: 0x11,
        acquire: false,
        release: true,
    };
    assert_eq!(
        release_only.memory_ordering(),
        RiscvMemoryOrdering::new(Some(RiscvFenceSet::memory()), None)
    );

    let acquire_only = MemoryAccessKind::LoadReserved {
        rd: reg(5),
        address: 0x9008,
        width: MemoryWidth::Doubleword,
        acquire: true,
        release: false,
    };
    assert_eq!(
        acquire_only.memory_ordering(),
        RiscvMemoryOrdering::new(None, Some(RiscvFenceSet::memory()))
    );

    let acquire_release = MemoryAccessKind::AtomicMemory {
        rd: reg(7),
        address: 0x9008,
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Swap,
        value: 0x22,
        acquire: true,
        release: true,
    };
    assert_eq!(
        acquire_release.memory_ordering(),
        RiscvMemoryOrdering::new(Some(RiscvFenceSet::memory()), Some(RiscvFenceSet::memory()))
    );
}

#[test]
fn hart_records_environment_and_breakpoint_traps_without_advancing_pc() {
    let mut hart = RiscvHartState::new(0x7000);

    let ecall = hart
        .execute(RiscvInstruction::decode(0x0000_0073).unwrap())
        .unwrap();
    assert_eq!(hart.pc(), 0x7000);
    assert_eq!(
        ecall.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x7000))
    );
    assert_eq!(ecall.register_writes(), &[]);
    assert_eq!(ecall.memory_access(), None);

    let ebreak = hart
        .execute(RiscvInstruction::decode(0x0010_0073).unwrap())
        .unwrap();
    assert_eq!(hart.pc(), 0x7000);
    assert_eq!(
        ebreak.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::Breakpoint, 0x7000))
    );
}

#[test]
fn decoder_rejects_compressed_and_unknown_encodings() {
    assert_eq!(
        RiscvInstruction::decode(0x0000_0001).unwrap_err(),
        RiscvError::CompressedNotSupported { raw: 0x0000_0001 }
    );
    assert_eq!(
        RiscvInstruction::decode(0xffff_ffff).unwrap_err(),
        RiscvError::UnknownEncoding { raw: 0xffff_ffff }
    );
    assert_eq!(
        Register::new(32).unwrap_err(),
        RiscvError::InvalidRegister { index: 32 }
    );
}
