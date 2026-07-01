use rem6_isa_riscv::{
    AtomicMemoryOp, FloatRegister, Immediate, MemoryAccessKind, MemoryWidth, Register,
    RegisterWrite, RiscvCounterBank, RiscvCounterCsr, RiscvCounterCsrWord, RiscvCounterEnableCsr,
    RiscvCounterEnableCsrInstruction, RiscvCounterSnapshot, RiscvCsrError, RiscvCsrOp,
    RiscvEnvironmentConfigCsr, RiscvEnvironmentConfigCsrInstruction, RiscvError,
    RiscvExecutionRecord, RiscvFenceSet, RiscvGdbXlen, RiscvHartState, RiscvInstruction,
    RiscvMachineIdentityCsr, RiscvMachineInformationCsr, RiscvMachineInformationCsrInstruction,
    RiscvMachineIsaCsr, RiscvMachineTrapCsr, RiscvMemoryOrdering, RiscvPrivilegeMode,
    RiscvPseudoOp, RiscvStatusCsr, RiscvStatusWord, RiscvSupervisorTrapCsr, RiscvSv39AccessContext,
    RiscvSystemEvent, RiscvTranslationCsr, RiscvTranslationCsrInstruction, RiscvTrap,
    RiscvTrapKind,
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

fn csr_type(csr: u32, rs1_or_zimm: u8, funct3: u32, rd: u8) -> u32 {
    (csr << 20) | (u32::from(rs1_or_zimm) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
}

fn machine_identity(csr: RiscvMachineIdentityCsr) -> RiscvMachineInformationCsr {
    RiscvMachineInformationCsr::Identity(csr)
}

fn machine_isa(csr: RiscvMachineIsaCsr) -> RiscvMachineInformationCsr {
    RiscvMachineInformationCsr::Isa(csr)
}

fn sfence_vma_type(rs1: u8, rs2: u8, rd: u8, funct3: u32) -> u32 {
    (0x09 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x73
}

fn gem5_m5op_type(function: u32) -> u32 {
    0x0000_007b | (function << 25)
}

#[test]
fn riscv_hart_state_tracks_sv39_access_context() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_status(RiscvStatusWord::new(0x2));

    assert_eq!(
        hart.sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Machine)
    );

    let context = RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor)
        .with_mxr(true)
        .with_sum(true);
    hart.set_sv39_access_context(context);

    assert_eq!(hart.sv39_access_context(), context);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert!(hart.status().mxr());
    assert!(hart.status().sum());
    assert_eq!(hart.status().bits(), 0x2 | (1 << 18) | (1 << 19));
}

#[test]
fn riscv_hart_state_derives_sv39_access_context_from_status() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_status(RiscvStatusWord::new(0).with_mxr(true).with_sum(true));

    assert_eq!(
        hart.sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor)
            .with_mxr(true)
            .with_sum(true)
    );

    hart.set_status(hart.status().with_sum(false));

    assert_eq!(
        hart.sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor).with_mxr(true)
    );
}

#[test]
fn riscv_hart_state_derives_data_sv39_access_context_from_mprv() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_privilege_mode(RiscvPrivilegeMode::Machine);
    hart.set_status(
        RiscvStatusWord::new(0)
            .with_mprv(true)
            .with_mpp(RiscvPrivilegeMode::Supervisor)
            .with_mxr(true)
            .with_sum(true),
    );

    assert_eq!(
        hart.sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Machine)
            .with_mxr(true)
            .with_sum(true)
    );
    assert_eq!(
        hart.data_sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor)
            .with_mxr(true)
            .with_sum(true)
    );

    hart.set_status(hart.status().with_mprv(false));

    assert_eq!(hart.data_sv39_access_context(), hart.sv39_access_context());
}

#[test]
fn riscv_status_word_tracks_mxr_and_sum_bits() {
    assert_eq!(RiscvStatusWord::new(0).with_sum(true).bits(), 1 << 18);
    assert_eq!(RiscvStatusWord::new(0).with_mxr(true).bits(), 1 << 19);

    let status = RiscvStatusWord::new((1 << 18) | (1 << 19));
    assert!(status.sum());
    assert!(status.mxr());
    assert_eq!(status.with_sum(false).bits(), 1 << 19);
    assert_eq!(status.with_mxr(false).bits(), 1 << 18);
}

#[test]
fn riscv_status_word_tracks_supervisor_return_bits() {
    assert_eq!(RiscvStatusWord::new(0).with_sie(true).bits(), 1 << 1);
    assert_eq!(RiscvStatusWord::new(0).with_spie(true).bits(), 1 << 5);
    assert_eq!(
        RiscvStatusWord::new(0)
            .with_spp(RiscvPrivilegeMode::Supervisor)
            .bits(),
        1 << 8
    );

    let status = RiscvStatusWord::new((1 << 1) | (1 << 5) | (1 << 8));
    assert!(status.sie());
    assert!(status.spie());
    assert_eq!(status.spp(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(
        status.with_spp(RiscvPrivilegeMode::User).bits(),
        (1 << 1) | (1 << 5)
    );
}

#[test]
fn riscv_status_word_tracks_mprv_and_mpp_bits() {
    assert_eq!(RiscvStatusWord::new(0).with_mie(true).bits(), 1 << 3);
    assert_eq!(RiscvStatusWord::new(0).with_mpie(true).bits(), 1 << 7);
    assert_eq!(RiscvStatusWord::new(0).with_mprv(true).bits(), 1 << 17);
    assert_eq!(
        RiscvStatusWord::new(0)
            .with_mpp(RiscvPrivilegeMode::Supervisor)
            .bits(),
        1 << 11
    );
    assert_eq!(
        RiscvStatusWord::new(0)
            .with_mpp(RiscvPrivilegeMode::Machine)
            .bits(),
        3 << 11
    );

    let status = RiscvStatusWord::new((1 << 17) | (3 << 11));
    assert!(!status.mie());
    assert!(!status.mpie());
    assert!(status.mprv());
    assert_eq!(status.mpp(), RiscvPrivilegeMode::Machine);
    assert_eq!(status.with_mpp(RiscvPrivilegeMode::User).bits(), 1 << 17);
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

fn shift_i_type_with_funct6(funct6: u32, shamt: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct6 << 26)
        | (u32::from(shamt) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x13
}

fn shift_i32_type_with_funct7(funct7: u32, shamt: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(shamt & 0x1f) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x1b
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

fn compressed(raw: u16) -> u32 {
    u32::from(raw)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
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
    assert_eq!(counters.read_user(RiscvCounterCsr::Time), 0);
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
    assert_eq!(counters.read_user(RiscvCounterCsr::Time), 0x10);
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
        counters.write_user(RiscvCounterCsr::Time, 7).unwrap_err(),
        RiscvCsrError::ReadOnlyCounterAlias {
            csr: RiscvCounterCsr::Time,
        }
    );
    assert_eq!(
        counters.set_machine(RiscvCounterCsr::Time, 7).unwrap_err(),
        RiscvCsrError::ReadOnlyCounterAlias {
            csr: RiscvCounterCsr::Time,
        }
    );
    assert_eq!(
        RiscvCounterCsr::from_user_address(0xc00).unwrap(),
        RiscvCounterCsr::Cycle
    );
    assert_eq!(
        RiscvCounterCsr::from_user_address(0xc01).unwrap(),
        RiscvCounterCsr::Time
    );
    assert_eq!(
        RiscvCounterCsr::from_machine_address(0xb02).unwrap(),
        RiscvCounterCsr::Instret
    );
    assert_eq!(
        RiscvCounterCsr::from_machine_address(0xc00).unwrap_err(),
        RiscvCsrError::UnknownCounterCsr { address: 0xc00 }
    );
    assert_eq!(
        RiscvCounterCsr::from_machine_address(0xb01).unwrap_err(),
        RiscvCsrError::UnknownCounterCsr { address: 0xb01 }
    );
    assert_eq!(RiscvCounterCsr::Cycle.machine_address(), Some(0xb00));
    assert_eq!(RiscvCounterCsr::Time.machine_address(), None);
    assert_eq!(RiscvCounterCsr::Instret.machine_address(), Some(0xb02));

    counters
        .write_machine(RiscvCounterCsr::Cycle, u64::MAX)
        .unwrap();
    counters.add_cycles(2);
    counters
        .write_machine(RiscvCounterCsr::Instret, 0xfeed)
        .unwrap();
    let snapshot = counters.snapshot();
    assert_eq!(snapshot, RiscvCounterSnapshot::with_time(1, 2, 0xfeed));

    let mut restored = RiscvCounterBank::new();
    restored.restore(&snapshot);
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.read_user(RiscvCounterCsr::Cycle), 1);
    assert_eq!(restored.read_user(RiscvCounterCsr::Time), 2);
    assert_eq!(restored.read_user(RiscvCounterCsr::Instret), 0xfeed);
}

#[test]
fn hart_decodes_and_executes_time_counter_csr() {
    let mut hart = RiscvHartState::new(0x4050);

    let read_time = RiscvInstruction::decode(csr_read_type(0xc01, 5)).unwrap();
    assert_eq!(
        read_time,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(5),
            csr: RiscvCounterCsr::Time,
        }
    );

    let record = hart.execute(read_time).unwrap();

    assert_eq!(record.pc(), 0x4050);
    assert_eq!(record.next_pc(), 0x4054);
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(5), 0)]);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(hart.counter_snapshot().cycle(), 1);
}

#[test]
fn hart_reads_machine_hart_id_csr() {
    let instruction = RiscvInstruction::decode(csr_read_type(0xf14, 5)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::read(
            reg(5),
            machine_identity(RiscvMachineIdentityCsr::HartId),
        ))
    );

    let mut hart = RiscvHartState::with_hart_id(0x2000, 7);
    let record = hart.execute(instruction).unwrap();

    assert_eq!(hart.hart_id(), 7);
    assert_eq!(hart.read(reg(5)), 7);
    assert_eq!(record.next_pc(), 0x2004);
}

#[test]
fn hart_reads_machine_identity_csrs() {
    let vendor_id = RiscvInstruction::decode(csr_read_type(0xf11, 5)).unwrap();
    let architecture_id = RiscvInstruction::decode(csr_read_type(0xf12, 6)).unwrap();
    let implementation_id = RiscvInstruction::decode(csr_read_type(0xf13, 7)).unwrap();
    let config_pointer = RiscvInstruction::decode(csr_read_type(0xf15, 8)).unwrap();

    assert_eq!(
        config_pointer,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::read(
            reg(8),
            machine_identity(RiscvMachineIdentityCsr::ConfigPointer),
        ))
    );

    let mut hart = RiscvHartState::with_hart_id(0x2200, 7);
    hart.execute(vendor_id).unwrap();
    hart.execute(architecture_id).unwrap();
    hart.execute(implementation_id).unwrap();
    let record = hart.execute(config_pointer).unwrap();

    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(hart.read(reg(6)), 0);
    assert_eq!(hart.read(reg(7)), 0);
    assert_eq!(hart.read(reg(8)), 0);
    assert_eq!(record.next_pc(), 0x2210);
}

#[test]
fn hart_reads_and_ignores_writes_to_machine_isa_csr() {
    let read = RiscvInstruction::decode(csr_read_type(0x301, 5)).unwrap();
    let write = RiscvInstruction::decode(csr_type(0x301, 2, 0x1, 6)).unwrap();
    let set = RiscvInstruction::decode(csr_type(0x301, 2, 0x2, 7)).unwrap();
    let clear = RiscvInstruction::decode(csr_type(0x301, 2, 0x3, 8)).unwrap();
    let write_immediate = RiscvInstruction::decode(csr_type(0x301, 0x1f, 0x5, 9)).unwrap();
    let set_immediate = RiscvInstruction::decode(csr_type(0x301, 0x1f, 0x6, 10)).unwrap();
    let clear_immediate = RiscvInstruction::decode(csr_type(0x301, 0x1f, 0x7, 11)).unwrap();

    assert_eq!(
        read,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::read(
            reg(5),
            machine_isa(RiscvMachineIsaCsr::Misa),
        ))
    );
    assert_eq!(
        write,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::register(
            reg(6),
            machine_isa(RiscvMachineIsaCsr::Misa),
            RiscvCsrOp::Write,
            reg(2),
        ))
    );
    assert_eq!(
        set,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::register(
            reg(7),
            machine_isa(RiscvMachineIsaCsr::Misa),
            RiscvCsrOp::Set,
            reg(2),
        ))
    );
    assert_eq!(
        clear,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::register(
            reg(8),
            machine_isa(RiscvMachineIsaCsr::Misa),
            RiscvCsrOp::Clear,
            reg(2),
        ))
    );
    assert_eq!(
        write_immediate,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::immediate(
            reg(9),
            machine_isa(RiscvMachineIsaCsr::Misa),
            RiscvCsrOp::Write,
            0x1f,
        ))
    );
    assert_eq!(
        set_immediate,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::immediate(
            reg(10),
            machine_isa(RiscvMachineIsaCsr::Misa),
            RiscvCsrOp::Set,
            0x1f,
        ))
    );
    assert_eq!(
        clear_immediate,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::immediate(
            reg(11),
            machine_isa(RiscvMachineIsaCsr::Misa),
            RiscvCsrOp::Clear,
            0x1f,
        ))
    );

    let mut hart = RiscvHartState::new(0x2300);
    hart.write(reg(2), 0);
    for instruction in [
        read,
        write,
        set,
        clear,
        write_immediate,
        set_immediate,
        clear_immediate,
    ] {
        hart.execute(instruction).unwrap();
    }

    assert_eq!(hart.read(reg(5)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.read(reg(6)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.read(reg(7)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.read(reg(8)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.read(reg(9)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.read(reg(10)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.read(reg(11)), RiscvMachineIsaCsr::RV64_MISA);
    assert_eq!(hart.pc(), 0x231c);

    let record = hart
        .execute(RiscvInstruction::decode(csr_read_type(0x301, 12)).unwrap())
        .unwrap();

    assert_eq!(
        record.register_writes(),
        &[RegisterWrite::new(reg(12), RiscvMachineIsaCsr::RV64_MISA)]
    );
    assert_eq!(hart.read(reg(12)), RiscvMachineIsaCsr::RV64_MISA);
}

#[test]
fn rv32_hart_reads_and_ignores_writes_to_machine_isa_csr() {
    let read = RiscvInstruction::decode(csr_read_type(0x301, 5)).unwrap();
    let write = RiscvInstruction::decode(csr_type(0x301, 2, 0x1, 6)).unwrap();

    let mut hart = RiscvHartState::new(0x2320);
    hart.set_xlen(RiscvGdbXlen::Rv32);
    hart.write(reg(2), 0);

    let read_record = hart.execute(read).unwrap();
    let write_record = hart.execute(write).unwrap();

    assert_eq!(
        read_record.register_writes(),
        &[RegisterWrite::new(reg(5), RiscvMachineIsaCsr::RV32_MISA)]
    );
    assert_eq!(
        write_record.register_writes(),
        &[RegisterWrite::new(reg(6), RiscvMachineIsaCsr::RV32_MISA)]
    );
    assert_eq!(hart.read(reg(5)), RiscvMachineIsaCsr::RV32_MISA);
    assert_eq!(hart.read(reg(6)), RiscvMachineIsaCsr::RV32_MISA);
    assert_eq!(hart.pc(), 0x2328);
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
fn rv64_hart_traps_rv32_counter_high_word_csrs() {
    let user_high = RiscvInstruction::decode(csr_read_type(0xc80, 5)).unwrap();
    let machine_high = RiscvInstruction::decode(csr_read_type(0xb80, 6)).unwrap();
    assert_eq!(
        user_high,
        RiscvInstruction::ReadCounterCsrWord {
            rd: reg(5),
            csr: RiscvCounterCsrWord::CycleHigh,
        }
    );
    assert_eq!(
        machine_high,
        RiscvInstruction::ReadMachineCounterCsrWord {
            rd: reg(6),
            csr: RiscvCounterCsrWord::CycleHigh,
        }
    );

    let mut user_hart = RiscvHartState::new(0x2440);
    user_hart.set_machine_trap_vector(0x9000);
    let user_record = user_hart.execute(user_high).unwrap();
    assert_eq!(
        user_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x2440))
    );
    assert_eq!(user_record.register_writes(), &[]);
    assert_eq!(user_hart.read(reg(5)), 0);

    let mut machine_hart = RiscvHartState::new(0x2480);
    machine_hart.set_machine_trap_vector(0x9040);
    let machine_record = machine_hart.execute(machine_high).unwrap();
    assert_eq!(
        machine_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x2480))
    );
    assert_eq!(machine_record.register_writes(), &[]);
    assert_eq!(machine_hart.read(reg(6)), 0);
}

#[test]
fn hart_reads_counter_csrs_through_read_only_csr_aliases() {
    let cycle_clear = RiscvInstruction::decode(csr_type(0xc00, 0, 0x3, 5)).unwrap();
    let time_set = RiscvInstruction::decode(csr_type(0xc01, 0, 0x2, 9)).unwrap();
    let time_clear_immediate = RiscvInstruction::decode(csr_type(0xc01, 0, 0x7, 10)).unwrap();
    let instret_set_immediate = RiscvInstruction::decode(csr_type(0xc02, 0, 0x6, 6)).unwrap();
    let cycle_clear_immediate = RiscvInstruction::decode(csr_type(0xb00, 0, 0x7, 7)).unwrap();
    let hart_id_clear = RiscvInstruction::decode(csr_type(0xf14, 0, 0x3, 8)).unwrap();

    assert_eq!(
        cycle_clear,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(5),
            csr: RiscvCounterCsr::Cycle,
        }
    );
    assert_eq!(
        time_set,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(9),
            csr: RiscvCounterCsr::Time,
        }
    );
    assert_eq!(
        time_clear_immediate,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(10),
            csr: RiscvCounterCsr::Time,
        }
    );
    assert_eq!(
        instret_set_immediate,
        RiscvInstruction::ReadCounterCsr {
            rd: reg(6),
            csr: RiscvCounterCsr::Instret,
        }
    );
    assert_eq!(
        cycle_clear_immediate,
        RiscvInstruction::ReadMachineCounterCsr {
            rd: reg(7),
            csr: RiscvCounterCsr::Cycle,
        }
    );
    assert_eq!(
        hart_id_clear,
        RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::read(
            reg(8),
            machine_identity(RiscvMachineIdentityCsr::HartId),
        ))
    );

    let mut hart = RiscvHartState::with_hart_id(0x2800, 11);
    hart.execute(RiscvInstruction::decode(i_type(1, 0, 0x0, 1, 0x13)).unwrap())
        .unwrap();
    hart.execute(cycle_clear).unwrap();
    hart.execute(time_set).unwrap();
    hart.execute(time_clear_immediate).unwrap();
    hart.execute(instret_set_immediate).unwrap();
    hart.execute(cycle_clear_immediate).unwrap();
    hart.execute(hart_id_clear).unwrap();

    assert_eq!(hart.read(reg(5)), 1);
    assert_eq!(hart.read(reg(9)), 2);
    assert_eq!(hart.read(reg(10)), 3);
    assert_eq!(hart.read(reg(6)), 4);
    assert_eq!(hart.read(reg(7)), 5);
    assert_eq!(hart.read(reg(8)), 11);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(7, 7));
}

#[test]
fn decoder_rejects_counter_csr_write_aliases() {
    let cases = [
        csr_type(0xc00, 1, 0x2, 5),
        csr_type(0xc01, 1, 0x2, 5),
        csr_type(0xc02, 1, 0x3, 6),
        csr_type(0xc01, 1, 0x3, 6),
        csr_type(0xc00, 1, 0x6, 7),
        csr_type(0xc01, 1, 0x6, 7),
        csr_type(0xc02, 1, 0x7, 8),
        csr_type(0xc01, 1, 0x7, 8),
    ];

    for raw in cases {
        assert_eq!(
            RiscvInstruction::decode(raw),
            Err(RiscvError::UnknownEncoding { raw })
        );
    }
}

#[test]
fn hart_traps_directly_constructed_machine_time_counter_writes() {
    let mut hart = RiscvHartState::new(0x2c00);
    hart.set_machine_trap_vector(0x9000);
    hart.write(reg(2), 0xfeed);
    hart.execute(RiscvInstruction::decode(i_type(1, 0, 0x0, 1, 0x13)).unwrap())
        .unwrap();

    let record = hart
        .execute(RiscvInstruction::WriteCounterCsr {
            rd: reg(5),
            csr: RiscvCounterCsr::Time,
            rs1: reg(2),
        })
        .unwrap();

    assert_eq!(record.pc(), 0x2c04);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x2c04))
    );
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.machine_exception_pc(), 0x2c04);
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(1, 1));

    hart.set_pc(0x2c08);
    let read_record = hart
        .execute(RiscvInstruction::ReadMachineCounterCsr {
            rd: reg(6),
            csr: RiscvCounterCsr::Time,
        })
        .unwrap();

    assert_eq!(read_record.pc(), 0x2c08);
    assert_eq!(read_record.next_pc(), 0x9000);
    assert_eq!(read_record.register_writes(), &[]);
    assert_eq!(
        read_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x2c08))
    );
    assert_eq!(hart.read(reg(6)), 0);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(1, 1));

    hart.set_pc(0x2c0c);
    let second_record = hart
        .execute(RiscvInstruction::SetCounterCsrImmediate {
            rd: reg(7),
            csr: RiscvCounterCsr::Time,
            zimm: 0x1f,
        })
        .unwrap();

    assert_eq!(second_record.pc(), 0x2c0c);
    assert_eq!(second_record.next_pc(), 0x9000);
    assert_eq!(second_record.register_writes(), &[]);
    assert_eq!(hart.read(reg(7)), 0);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(1, 1));
}

#[test]
fn hart_executes_machine_counter_csr_read_modify_write_operations() {
    let write_cycle = RiscvInstruction::decode(csr_type(0xb00, 2, 0x1, 5)).unwrap();
    let set_instret = RiscvInstruction::decode(csr_type(0xb02, 3, 0x2, 6)).unwrap();
    let clear_cycle = RiscvInstruction::decode(csr_type(0xb00, 4, 0x3, 7)).unwrap();
    let write_instret_imm = RiscvInstruction::decode(csr_type(0xb02, 7, 0x5, 8)).unwrap();
    let set_cycle_imm = RiscvInstruction::decode(csr_type(0xb00, 0x10, 0x6, 9)).unwrap();
    let clear_instret_imm = RiscvInstruction::decode(csr_type(0xb02, 1, 0x7, 10)).unwrap();

    assert_eq!(
        write_cycle,
        RiscvInstruction::WriteCounterCsr {
            rd: reg(5),
            csr: RiscvCounterCsr::Cycle,
            rs1: reg(2),
        }
    );
    assert_eq!(
        set_instret,
        RiscvInstruction::SetCounterCsr {
            rd: reg(6),
            csr: RiscvCounterCsr::Instret,
            rs1: reg(3),
        }
    );
    assert_eq!(
        clear_cycle,
        RiscvInstruction::ClearCounterCsr {
            rd: reg(7),
            csr: RiscvCounterCsr::Cycle,
            rs1: reg(4),
        }
    );
    assert_eq!(
        write_instret_imm,
        RiscvInstruction::WriteCounterCsrImmediate {
            rd: reg(8),
            csr: RiscvCounterCsr::Instret,
            zimm: 7,
        }
    );
    assert_eq!(
        set_cycle_imm,
        RiscvInstruction::SetCounterCsrImmediate {
            rd: reg(9),
            csr: RiscvCounterCsr::Cycle,
            zimm: 0x10,
        }
    );
    assert_eq!(
        clear_instret_imm,
        RiscvInstruction::ClearCounterCsrImmediate {
            rd: reg(10),
            csr: RiscvCounterCsr::Instret,
            zimm: 1,
        }
    );

    let mut hart = RiscvHartState::new(0x3000);
    hart.write(reg(2), 0x40);
    hart.write(reg(3), 0x10);
    hart.write(reg(4), 0x3);

    hart.execute(write_cycle).unwrap();
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(
        hart.counter_snapshot(),
        RiscvCounterSnapshot::with_time(0x41, 1, 1)
    );

    hart.execute(set_instret).unwrap();
    assert_eq!(hart.read(reg(6)), 1);
    assert_eq!(
        hart.counter_snapshot(),
        RiscvCounterSnapshot::with_time(0x42, 2, 0x12)
    );

    hart.execute(clear_cycle).unwrap();
    assert_eq!(hart.read(reg(7)), 0x42);
    assert_eq!(
        hart.counter_snapshot(),
        RiscvCounterSnapshot::with_time(0x41, 3, 0x13)
    );

    hart.execute(write_instret_imm).unwrap();
    assert_eq!(hart.read(reg(8)), 0x13);
    assert_eq!(
        hart.counter_snapshot(),
        RiscvCounterSnapshot::with_time(0x42, 4, 8)
    );

    hart.execute(set_cycle_imm).unwrap();
    assert_eq!(hart.read(reg(9)), 0x42);
    assert_eq!(
        hart.counter_snapshot(),
        RiscvCounterSnapshot::with_time(0x53, 5, 9)
    );

    hart.execute(clear_instret_imm).unwrap();
    assert_eq!(hart.read(reg(10)), 9);
    assert_eq!(
        hart.counter_snapshot(),
        RiscvCounterSnapshot::with_time(0x54, 6, 9)
    );
}

#[test]
fn hart_executes_counter_enable_csr_read_modify_write_operations() {
    let read_scounteren = RiscvInstruction::decode(csr_read_type(0x106, 5)).unwrap();
    let write_mcounteren = RiscvInstruction::decode(csr_type(0x306, 2, 0x1, 6)).unwrap();
    let set_scounteren = RiscvInstruction::decode(csr_type(0x106, 3, 0x2, 7)).unwrap();
    let clear_mcounteren = RiscvInstruction::decode(csr_type(0x306, 4, 0x3, 8)).unwrap();
    let write_scounteren_imm = RiscvInstruction::decode(csr_type(0x106, 7, 0x5, 9)).unwrap();
    let set_mcounteren_imm = RiscvInstruction::decode(csr_type(0x306, 0x10, 0x6, 10)).unwrap();
    let clear_scounteren_imm = RiscvInstruction::decode(csr_type(0x106, 1, 0x7, 11)).unwrap();

    assert_eq!(
        read_scounteren,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::read(
            reg(5),
            RiscvCounterEnableCsr::Scounteren,
        ))
    );
    assert_eq!(
        write_mcounteren,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::register(
            reg(6),
            RiscvCounterEnableCsr::Mcounteren,
            RiscvCsrOp::Write,
            reg(2),
        ))
    );
    assert_eq!(
        set_scounteren,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::register(
            reg(7),
            RiscvCounterEnableCsr::Scounteren,
            RiscvCsrOp::Set,
            reg(3),
        ))
    );
    assert_eq!(
        clear_mcounteren,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::register(
            reg(8),
            RiscvCounterEnableCsr::Mcounteren,
            RiscvCsrOp::Clear,
            reg(4),
        ))
    );
    assert_eq!(
        write_scounteren_imm,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::immediate(
            reg(9),
            RiscvCounterEnableCsr::Scounteren,
            RiscvCsrOp::Write,
            7,
        ))
    );
    assert_eq!(
        set_mcounteren_imm,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::immediate(
            reg(10),
            RiscvCounterEnableCsr::Mcounteren,
            RiscvCsrOp::Set,
            0x10,
        ))
    );
    assert_eq!(
        clear_scounteren_imm,
        RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::immediate(
            reg(11),
            RiscvCounterEnableCsr::Scounteren,
            RiscvCsrOp::Clear,
            1,
        ))
    );

    let mut hart = RiscvHartState::new(0x3100);
    hart.set_supervisor_counter_enable(0x2);
    hart.write(reg(2), 0x65);
    hart.write(reg(3), 0x10);
    hart.write(reg(4), 0x60);

    hart.execute(read_scounteren).unwrap();
    assert_eq!(hart.read(reg(5)), 0x2);
    assert_eq!(hart.supervisor_counter_enable(), 0x2);
    assert_eq!(hart.machine_counter_enable(), 0);

    hart.execute(write_mcounteren).unwrap();
    assert_eq!(hart.read(reg(6)), 0);
    assert_eq!(hart.machine_counter_enable(), 0x65);

    hart.execute(set_scounteren).unwrap();
    assert_eq!(hart.read(reg(7)), 0x2);
    assert_eq!(hart.supervisor_counter_enable(), 0x12);

    hart.execute(clear_mcounteren).unwrap();
    assert_eq!(hart.read(reg(8)), 0x65);
    assert_eq!(hart.machine_counter_enable(), 0x5);

    hart.execute(write_scounteren_imm).unwrap();
    assert_eq!(hart.read(reg(9)), 0x12);
    assert_eq!(hart.supervisor_counter_enable(), 0x7);

    hart.execute(set_mcounteren_imm).unwrap();
    assert_eq!(hart.read(reg(10)), 0x5);
    assert_eq!(hart.machine_counter_enable(), 0x15);

    hart.execute(clear_scounteren_imm).unwrap();
    assert_eq!(hart.read(reg(11)), 0x7);
    assert_eq!(hart.supervisor_counter_enable(), 0x6);
}

#[test]
fn hart_machine_cycle_writes_do_not_change_time_counter() {
    let write_cycle = RiscvInstruction::decode(csr_type(0xb00, 2, 0x1, 5)).unwrap();
    let read_time = RiscvInstruction::decode(csr_read_type(0xc01, 6)).unwrap();

    let mut hart = RiscvHartState::new(0x3200);
    hart.write(reg(2), 0x40);

    hart.execute(write_cycle).unwrap();
    hart.execute(read_time).unwrap();

    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(hart.read(reg(6)), 1);
}

#[test]
fn hart_executes_status_csr_read_modify_write_operations() {
    let mut hart = RiscvHartState::new(0x3300);
    hart.set_status(RiscvStatusWord::new(0x2 | (1 << 18)));
    hart.write(reg(2), 1 << 19);
    hart.write(reg(3), 1 << 18);

    let read_mstatus = RiscvInstruction::decode(csr_read_type(0x300, 5)).unwrap();
    let read_sstatus = RiscvInstruction::decode(csr_read_type(0x100, 6)).unwrap();
    let set_sstatus = RiscvInstruction::decode(csr_type(0x100, 2, 0x2, 7)).unwrap();
    let clear_sstatus = RiscvInstruction::decode(csr_type(0x100, 3, 0x3, 8)).unwrap();

    assert_eq!(
        read_mstatus,
        RiscvInstruction::ReadStatusCsr {
            rd: reg(5),
            csr: RiscvStatusCsr::Mstatus,
        }
    );
    assert_eq!(
        read_sstatus,
        RiscvInstruction::ReadStatusCsr {
            rd: reg(6),
            csr: RiscvStatusCsr::Sstatus,
        }
    );

    let read_record = hart.execute(read_mstatus).unwrap();
    assert_eq!(
        read_record.register_writes(),
        &[RegisterWrite::new(reg(5), 0x2 | (1 << 18))]
    );
    hart.execute(read_sstatus).unwrap();
    assert_eq!(hart.read(reg(6)), 0x2 | (1 << 18));

    hart.execute(set_sstatus).unwrap();
    assert_eq!(hart.read(reg(7)), 0x2 | (1 << 18));
    assert_eq!(hart.status().bits(), 0x2 | (1 << 18) | (1 << 19));
    assert_eq!(
        hart.sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Machine)
            .with_mxr(true)
            .with_sum(true)
    );

    hart.execute(clear_sstatus).unwrap();
    assert_eq!(hart.read(reg(8)), 0x2 | (1 << 18) | (1 << 19));
    assert_eq!(hart.status().bits(), 0x2 | (1 << 19));
}

#[test]
fn rv64_hart_traps_mstatush_status_csr_access() {
    let mut hart = RiscvHartState::new(0x3340);
    hart.set_machine_trap_vector(0x9000);
    hart.set_status(RiscvStatusWord::new(0x1234_5678_000c_0122));
    hart.write(reg(4), 0x89ab_cdef);

    let read_mstatush = RiscvInstruction::decode(csr_read_type(0x310, 9)).unwrap();
    let write_mstatush = RiscvInstruction::decode(csr_type(0x310, 4, 0x1, 10)).unwrap();
    assert_eq!(
        read_mstatush,
        RiscvInstruction::ReadStatusCsr {
            rd: reg(9),
            csr: RiscvStatusCsr::Mstatush,
        }
    );

    let read_record = hart.execute(read_mstatush).unwrap();
    assert_eq!(read_record.pc(), 0x3340);
    assert_eq!(read_record.next_pc(), 0x9000);
    assert_eq!(
        read_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x3340))
    );
    assert_eq!(read_record.register_writes(), &[]);
    assert_eq!(hart.read(reg(9)), 0);
    assert_eq!(hart.status().bits() >> 32, 0x1234_5678);

    hart.set_machine_trap_vector(0x9040);
    let write_record = hart.execute(write_mstatush).unwrap();
    assert_eq!(write_record.pc(), 0x9000);
    assert_eq!(write_record.next_pc(), 0x9040);
    assert_eq!(
        write_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x9000))
    );
    assert_eq!(write_record.register_writes(), &[]);
    assert_eq!(hart.read(reg(10)), 0);
    assert_eq!(hart.status().bits() >> 32, 0x1234_5678);
}

#[test]
fn rv32_hart_executes_mstatush_high_half_status_csr() {
    let mut hart = RiscvHartState::new(0x3380);
    hart.set_xlen(RiscvGdbXlen::Rv32);
    hart.set_status(RiscvStatusWord::new(0x2 | (1 << 18)));
    hart.write(reg(4), 0x89ab_cdef);

    let read_mstatush = RiscvInstruction::decode(csr_read_type(0x310, 9)).unwrap();
    let write_mstatush = RiscvInstruction::decode(csr_type(0x310, 4, 0x1, 10)).unwrap();
    let read_mstatus = RiscvInstruction::decode(csr_read_type(0x300, 11)).unwrap();
    let write_mstatus = RiscvInstruction::decode(csr_type(0x300, 5, 0x1, 12)).unwrap();

    hart.execute(read_mstatush).unwrap();
    assert_eq!(hart.read(reg(9)), 0);

    hart.execute(write_mstatush).unwrap();
    assert_eq!(hart.read(reg(10)), 0);
    assert_eq!(hart.status().bits(), 0x89ab_cdef_0004_0002);

    hart.execute(read_mstatush).unwrap();
    assert_eq!(hart.read(reg(9)), 0x89ab_cdef);

    hart.execute(read_mstatus).unwrap();
    assert_eq!(hart.read(reg(11)), 0x0004_0002);

    hart.write(reg(5), 0x7654_3210);
    hart.execute(write_mstatus).unwrap();
    assert_eq!(hart.read(reg(12)), 0x0004_0002);
    assert_eq!(hart.status().bits(), 0x89ab_cdef_7654_3210);

    hart.execute(read_mstatush).unwrap();
    assert_eq!(hart.read(reg(9)), 0x89ab_cdef);
}

#[test]
fn hart_decodes_and_executes_machine_exception_pc_csr() {
    let mut hart = RiscvHartState::new(0x4000);
    hart.write(reg(2), 0x9000);
    hart.write(reg(3), 0x40);

    let write_mepc = RiscvInstruction::decode(csr_type(0x341, 2, 0x1, 5)).unwrap();
    assert_eq!(
        write_mepc,
        RiscvInstruction::WriteMachineTrapCsr {
            rd: reg(5),
            csr: RiscvMachineTrapCsr::Mepc,
            rs1: reg(2),
        }
    );
    let write_record = hart.execute(write_mepc).unwrap();
    assert_eq!(hart.machine_exception_pc(), 0x9000);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(
        write_record.register_writes(),
        &[RegisterWrite::new(reg(5), 0)]
    );

    let set_mepc = RiscvInstruction::decode(csr_type(0x341, 3, 0x2, 6)).unwrap();
    hart.execute(set_mepc).unwrap();
    assert_eq!(hart.read(reg(6)), 0x9000);
    assert_eq!(hart.machine_exception_pc(), 0x9040);

    let read_mepc = RiscvInstruction::decode(csr_read_type(0x341, 7)).unwrap();
    hart.execute(read_mepc).unwrap();
    assert_eq!(hart.read(reg(7)), 0x9040);
}

#[test]
fn hart_decodes_and_executes_machine_trap_csrs() {
    let mut hart = RiscvHartState::new(0x4100);
    hart.write(reg(2), 0x8001);
    hart.write(reg(3), 11);
    hart.write(reg(4), 0xfeed);
    hart.write(reg(16), 0x5555);

    let write_mtvec = RiscvInstruction::decode(csr_type(0x305, 2, 0x1, 5)).unwrap();
    let write_mcause = RiscvInstruction::decode(csr_type(0x342, 3, 0x1, 6)).unwrap();
    let write_mtval = RiscvInstruction::decode(csr_type(0x343, 4, 0x1, 7)).unwrap();
    let set_mcause_imm = RiscvInstruction::decode(csr_type(0x342, 0x10, 0x6, 8)).unwrap();
    let clear_mtval_imm = RiscvInstruction::decode(csr_type(0x343, 0x0e, 0x7, 9)).unwrap();
    let read_mtvec = RiscvInstruction::decode(csr_read_type(0x305, 10)).unwrap();
    let write_mscratch = RiscvInstruction::decode(csr_type(0x340, 16, 0x1, 17)).unwrap();
    let set_mscratch = RiscvInstruction::decode(csr_type(0x340, 0x01, 0x6, 18)).unwrap();
    let clear_mscratch = RiscvInstruction::decode(csr_type(0x340, 0x04, 0x7, 19)).unwrap();

    assert_eq!(
        write_mtvec,
        RiscvInstruction::WriteMachineTrapCsr {
            rd: reg(5),
            csr: RiscvMachineTrapCsr::Mtvec,
            rs1: reg(2),
        }
    );
    assert_eq!(
        read_mtvec,
        RiscvInstruction::ReadMachineTrapCsr {
            rd: reg(10),
            csr: RiscvMachineTrapCsr::Mtvec,
        }
    );
    assert_eq!(
        write_mscratch,
        RiscvInstruction::WriteMachineTrapCsr {
            rd: reg(17),
            csr: RiscvMachineTrapCsr::Mscratch,
            rs1: reg(16),
        }
    );

    hart.execute(write_mtvec).unwrap();
    assert_eq!(hart.machine_trap_vector(), 0x8001);
    assert_eq!(hart.read(reg(5)), 0);

    hart.execute(write_mcause).unwrap();
    assert_eq!(hart.machine_trap_cause(), 11);
    assert_eq!(hart.read(reg(6)), 0);

    hart.execute(write_mtval).unwrap();
    assert_eq!(hart.machine_trap_value(), 0xfeed);
    assert_eq!(hart.read(reg(7)), 0);

    hart.execute(set_mcause_imm).unwrap();
    assert_eq!(hart.read(reg(8)), 11);
    assert_eq!(hart.machine_trap_cause(), 27);

    hart.execute(clear_mtval_imm).unwrap();
    assert_eq!(hart.read(reg(9)), 0xfeed);
    assert_eq!(hart.machine_trap_value(), 0xfee1);

    hart.execute(read_mtvec).unwrap();
    assert_eq!(hart.read(reg(10)), 0x8001);

    hart.execute(write_mscratch).unwrap();
    assert_eq!(hart.machine_scratch(), 0x5555);
    assert_eq!(hart.read(reg(17)), 0);

    hart.execute(set_mscratch).unwrap();
    assert_eq!(hart.read(reg(18)), 0x5555);
    assert_eq!(hart.machine_scratch(), 0x5555);

    hart.execute(clear_mscratch).unwrap();
    assert_eq!(hart.read(reg(19)), 0x5555);
    assert_eq!(hart.machine_scratch(), 0x5551);
}

#[test]
fn hart_decodes_and_executes_supervisor_trap_csrs() {
    let mut hart = RiscvHartState::new(0x4200);
    hart.write(reg(2), 0x8101);
    hart.write(reg(3), 9);
    hart.write(reg(4), 0xbeef);
    hart.write(reg(11), 0x9000);
    hart.write(reg(12), 0x30);
    hart.write(reg(16), 0xaaaa);

    let write_stvec = RiscvInstruction::decode(csr_type(0x105, 2, 0x1, 5)).unwrap();
    let write_scause = RiscvInstruction::decode(csr_type(0x142, 3, 0x1, 6)).unwrap();
    let write_stval = RiscvInstruction::decode(csr_type(0x143, 4, 0x1, 7)).unwrap();
    let set_scause_imm = RiscvInstruction::decode(csr_type(0x142, 0x10, 0x6, 8)).unwrap();
    let clear_stval_imm = RiscvInstruction::decode(csr_type(0x143, 0x0e, 0x7, 9)).unwrap();
    let read_stvec = RiscvInstruction::decode(csr_read_type(0x105, 10)).unwrap();
    let write_sepc = RiscvInstruction::decode(csr_type(0x141, 11, 0x1, 13)).unwrap();
    let set_sepc = RiscvInstruction::decode(csr_type(0x141, 12, 0x2, 14)).unwrap();
    let read_sepc = RiscvInstruction::decode(csr_read_type(0x141, 15)).unwrap();
    let write_sscratch = RiscvInstruction::decode(csr_type(0x140, 16, 0x1, 17)).unwrap();
    let set_sscratch = RiscvInstruction::decode(csr_type(0x140, 0x02, 0x6, 18)).unwrap();
    let clear_sscratch = RiscvInstruction::decode(csr_type(0x140, 0x08, 0x7, 19)).unwrap();

    assert_eq!(
        write_stvec,
        RiscvInstruction::WriteSupervisorTrapCsr {
            rd: reg(5),
            csr: RiscvSupervisorTrapCsr::Stvec,
            rs1: reg(2),
        }
    );
    assert_eq!(
        read_stvec,
        RiscvInstruction::ReadSupervisorTrapCsr {
            rd: reg(10),
            csr: RiscvSupervisorTrapCsr::Stvec,
        }
    );
    assert_eq!(
        write_sepc,
        RiscvInstruction::WriteSupervisorTrapCsr {
            rd: reg(13),
            csr: RiscvSupervisorTrapCsr::Sepc,
            rs1: reg(11),
        }
    );
    assert_eq!(
        write_sscratch,
        RiscvInstruction::WriteSupervisorTrapCsr {
            rd: reg(17),
            csr: RiscvSupervisorTrapCsr::Sscratch,
            rs1: reg(16),
        }
    );

    hart.execute(write_stvec).unwrap();
    assert_eq!(hart.supervisor_trap_vector(), 0x8101);
    assert_eq!(hart.read(reg(5)), 0);

    hart.execute(write_scause).unwrap();
    assert_eq!(hart.supervisor_trap_cause(), 9);
    assert_eq!(hart.read(reg(6)), 0);

    hart.execute(write_stval).unwrap();
    assert_eq!(hart.supervisor_trap_value(), 0xbeef);
    assert_eq!(hart.read(reg(7)), 0);

    hart.execute(set_scause_imm).unwrap();
    assert_eq!(hart.read(reg(8)), 9);
    assert_eq!(hart.supervisor_trap_cause(), 25);

    hart.execute(clear_stval_imm).unwrap();
    assert_eq!(hart.read(reg(9)), 0xbeef);
    assert_eq!(hart.supervisor_trap_value(), 0xbee1);

    hart.execute(read_stvec).unwrap();
    assert_eq!(hart.read(reg(10)), 0x8101);

    hart.execute(write_sepc).unwrap();
    assert_eq!(hart.supervisor_exception_pc(), 0x9000);
    assert_eq!(hart.read(reg(13)), 0);

    hart.execute(set_sepc).unwrap();
    assert_eq!(hart.read(reg(14)), 0x9000);
    assert_eq!(hart.supervisor_exception_pc(), 0x9030);

    hart.execute(read_sepc).unwrap();
    assert_eq!(hart.read(reg(15)), 0x9030);

    hart.execute(write_sscratch).unwrap();
    assert_eq!(hart.supervisor_scratch(), 0xaaaa);
    assert_eq!(hart.read(reg(17)), 0);

    hart.execute(set_sscratch).unwrap();
    assert_eq!(hart.read(reg(18)), 0xaaaa);
    assert_eq!(hart.supervisor_scratch(), 0xaaaa);

    hart.execute(clear_sscratch).unwrap();
    assert_eq!(hart.read(reg(19)), 0xaaaa);
    assert_eq!(hart.supervisor_scratch(), 0xaaa2);
}

#[test]
fn hart_decodes_and_executes_environment_config_csrs() {
    let mut hart = RiscvHartState::new(0x3480);
    hart.set_supervisor_environment_config(0x1200);
    hart.write(reg(2), 0x00ff);
    hart.write(reg(3), 0x0f0f);
    hart.write(reg(4), 0x00f0);

    let read = RiscvInstruction::decode(csr_read_type(0x10a, 5)).unwrap();
    let write = RiscvInstruction::decode(csr_type(0x10a, 2, 0x1, 6)).unwrap();
    let set = RiscvInstruction::decode(csr_type(0x10a, 3, 0x2, 7)).unwrap();
    let clear = RiscvInstruction::decode(csr_type(0x10a, 4, 0x3, 8)).unwrap();
    let write_immediate = RiscvInstruction::decode(csr_type(0x10a, 0x1d, 0x5, 9)).unwrap();
    let set_immediate = RiscvInstruction::decode(csr_type(0x10a, 0x02, 0x6, 10)).unwrap();
    let clear_immediate = RiscvInstruction::decode(csr_type(0x10a, 0x01, 0x7, 11)).unwrap();
    let set_read_only = RiscvInstruction::decode(csr_type(0x10a, 0, 0x2, 12)).unwrap();
    let clear_read_only = RiscvInstruction::decode(csr_type(0x10a, 0, 0x3, 13)).unwrap();
    let set_immediate_read_only = RiscvInstruction::decode(csr_type(0x10a, 0, 0x6, 14)).unwrap();
    let clear_immediate_read_only = RiscvInstruction::decode(csr_type(0x10a, 0, 0x7, 15)).unwrap();
    let read_menvcfg = RiscvInstruction::decode(csr_read_type(0x30a, 16)).unwrap();

    assert_eq!(
        read,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::read(
            reg(5),
            RiscvEnvironmentConfigCsr::Senvcfg
        ))
    );
    assert_eq!(
        write,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::register(
            reg(6),
            RiscvEnvironmentConfigCsr::Senvcfg,
            RiscvCsrOp::Write,
            reg(2)
        ))
    );
    assert_eq!(
        set_read_only,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::read(
            reg(12),
            RiscvEnvironmentConfigCsr::Senvcfg
        ))
    );
    assert_eq!(
        clear_read_only,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::read(
            reg(13),
            RiscvEnvironmentConfigCsr::Senvcfg
        ))
    );
    assert_eq!(
        set_immediate_read_only,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::read(
            reg(14),
            RiscvEnvironmentConfigCsr::Senvcfg
        ))
    );
    assert_eq!(
        clear_immediate_read_only,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::read(
            reg(15),
            RiscvEnvironmentConfigCsr::Senvcfg
        ))
    );
    assert_eq!(
        read_menvcfg,
        RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::read(
            reg(16),
            RiscvEnvironmentConfigCsr::Menvcfg
        ))
    );

    hart.execute(read).unwrap();
    assert_eq!(hart.read(reg(5)), 0x1200);
    assert_eq!(hart.supervisor_environment_config(), 0x1200);

    hart.execute(write).unwrap();
    assert_eq!(hart.read(reg(6)), 0x1200);
    assert_eq!(hart.supervisor_environment_config(), 0x00ff);

    hart.execute(set).unwrap();
    assert_eq!(hart.read(reg(7)), 0x00ff);
    assert_eq!(hart.supervisor_environment_config(), 0x0fff);

    hart.execute(clear).unwrap();
    assert_eq!(hart.read(reg(8)), 0x0fff);
    assert_eq!(hart.supervisor_environment_config(), 0x0f0f);

    hart.execute(write_immediate).unwrap();
    assert_eq!(hart.read(reg(9)), 0x0f0f);
    assert_eq!(hart.supervisor_environment_config(), 0x1d);

    hart.execute(set_immediate).unwrap();
    assert_eq!(hart.read(reg(10)), 0x1d);
    assert_eq!(hart.supervisor_environment_config(), 0x1f);

    hart.execute(clear_immediate).unwrap();
    assert_eq!(hart.read(reg(11)), 0x1f);
    assert_eq!(hart.supervisor_environment_config(), 0x1e);

    for instruction in [
        set_read_only,
        clear_read_only,
        set_immediate_read_only,
        clear_immediate_read_only,
    ] {
        let before_read = hart.supervisor_environment_config();
        hart.execute(instruction).unwrap();
        assert_eq!(hart.supervisor_environment_config(), before_read);
    }
    assert_eq!(hart.read(reg(12)), 0x1e);
    assert_eq!(hart.read(reg(13)), 0x1e);
    assert_eq!(hart.read(reg(14)), 0x1e);
    assert_eq!(hart.read(reg(15)), 0x1e);

    hart.set_machine_environment_config(0x3400);
    hart.execute(read_menvcfg).unwrap();
    assert_eq!(hart.read(reg(16)), 0x3400);
    assert_eq!(hart.machine_environment_config(), 0x3400);
}

#[test]
fn hart_executes_satp_csr_write_and_exposes_address_space() {
    let write_satp = RiscvInstruction::decode(csr_type(0x180, 2, 0x1, 5)).unwrap();
    assert_eq!(
        write_satp,
        RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::register(
            reg(5),
            RiscvTranslationCsr::Satp,
            RiscvCsrOp::Write,
            reg(2),
        ))
    );

    let mut hart = RiscvHartState::new(0x3400);
    let satp = (8_u64 << 60) | (0x2a_u64 << 44) | 0x12345;
    hart.write(reg(2), satp);
    let record = hart.execute(write_satp).unwrap();

    assert_eq!(record.next_pc(), 0x3404);
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(5), 0)]);
    assert_eq!(hart.translation_address_space(), 0x2a);
    assert_eq!(hart.translation_satp(), satp);

    let read_satp = RiscvInstruction::decode(csr_read_type(0x180, 6)).unwrap();
    let read_record = hart.execute(read_satp).unwrap();
    assert_eq!(
        read_record.register_writes(),
        &[RegisterWrite::new(reg(6), satp)]
    );
}

#[test]
fn hart_executes_satp_csr_read_modify_write_operations() {
    let mut hart = RiscvHartState::new(0x3500);
    let initial_satp = (8_u64 << 60) | (0x12_u64 << 44) | 0x100;
    let set_mask = (0x03_u64 << 44) | 0x20;
    let clear_mask = (0x10_u64 << 44) | 0x100;
    hart.write(reg(2), initial_satp);
    hart.write(reg(3), set_mask);
    hart.write(reg(4), clear_mask);

    let write = RiscvInstruction::decode(csr_type(0x180, 2, 0x1, 5)).unwrap();
    let set = RiscvInstruction::decode(csr_type(0x180, 3, 0x2, 6)).unwrap();
    let clear = RiscvInstruction::decode(csr_type(0x180, 4, 0x3, 7)).unwrap();
    let write_immediate = RiscvInstruction::decode(csr_type(0x180, 0x1f, 0x5, 8)).unwrap();
    let set_immediate = RiscvInstruction::decode(csr_type(0x180, 0x10, 0x6, 9)).unwrap();
    let clear_immediate = RiscvInstruction::decode(csr_type(0x180, 0x01, 0x7, 10)).unwrap();
    let set_read_only = RiscvInstruction::decode(csr_type(0x180, 0, 0x2, 11)).unwrap();
    let clear_read_only = RiscvInstruction::decode(csr_type(0x180, 0, 0x3, 12)).unwrap();
    let set_immediate_read_only = RiscvInstruction::decode(csr_type(0x180, 0, 0x6, 13)).unwrap();
    let clear_immediate_read_only = RiscvInstruction::decode(csr_type(0x180, 0, 0x7, 14)).unwrap();

    assert_eq!(
        set_read_only,
        RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::read(
            reg(11),
            RiscvTranslationCsr::Satp,
        ))
    );
    assert_eq!(
        clear_read_only,
        RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::read(
            reg(12),
            RiscvTranslationCsr::Satp,
        ))
    );
    assert_eq!(
        set_immediate_read_only,
        RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::read(
            reg(13),
            RiscvTranslationCsr::Satp,
        ))
    );
    assert_eq!(
        clear_immediate_read_only,
        RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::read(
            reg(14),
            RiscvTranslationCsr::Satp,
        ))
    );
    assert!(matches!(
        RiscvInstruction::decode(csr_type(0x180, 1, 0x4, 15)),
        Err(RiscvError::UnknownEncoding { .. })
    ));
    assert!(matches!(
        RiscvInstruction::decode(csr_type(0x181, 1, 0x1, 15)),
        Err(RiscvError::UnknownEncoding { .. })
    ));

    hart.execute(write).unwrap();
    assert_eq!(hart.translation_satp(), initial_satp);
    assert_eq!(hart.read(reg(5)), 0);

    hart.execute(set).unwrap();
    assert_eq!(hart.read(reg(6)), initial_satp);
    assert_eq!(hart.translation_satp(), initial_satp | set_mask);

    let before_clear = hart.translation_satp();
    hart.execute(clear).unwrap();
    assert_eq!(hart.read(reg(7)), before_clear);
    assert_eq!(hart.translation_satp(), before_clear & !clear_mask);

    let before_immediate_write = hart.translation_satp();
    hart.execute(write_immediate).unwrap();
    assert_eq!(hart.read(reg(8)), before_immediate_write);
    assert_eq!(hart.translation_satp(), 0x1f);

    hart.execute(set_immediate).unwrap();
    assert_eq!(hart.read(reg(9)), 0x1f);
    assert_eq!(hart.translation_satp(), 0x1f);

    hart.execute(clear_immediate).unwrap();
    assert_eq!(hart.read(reg(10)), 0x1f);
    assert_eq!(hart.translation_satp(), 0x1e);

    for instruction in [
        set_read_only,
        clear_read_only,
        set_immediate_read_only,
        clear_immediate_read_only,
    ] {
        let before_read = hart.translation_satp();
        hart.execute(instruction).unwrap();
        assert_eq!(hart.translation_satp(), before_read);
    }
    assert_eq!(hart.read(reg(11)), 0x1e);
    assert_eq!(hart.read(reg(12)), 0x1e);
    assert_eq!(hart.read(reg(13)), 0x1e);
    assert_eq!(hart.read(reg(14)), 0x1e);
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
        RiscvInstruction::decode(i_type(-1, 3, 0x2, 4, 0x13)).unwrap(),
        RiscvInstruction::Slti {
            rd: reg(4),
            rs1: reg(3),
            imm: Immediate::new(-1),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(-1, 3, 0x3, 4, 0x13)).unwrap(),
        RiscvInstruction::Sltiu {
            rd: reg(4),
            rs1: reg(3),
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
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x1, 4, 0x33)).unwrap(),
        RiscvInstruction::Sll {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x2, 4, 0x33)).unwrap(),
        RiscvInstruction::Slt {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x3, 4, 0x33)).unwrap(),
        RiscvInstruction::Sltu {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x4, 4, 0x33)).unwrap(),
        RiscvInstruction::Xor {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x5, 4, 0x33)).unwrap(),
        RiscvInstruction::Srl {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x20, 6, 5, 0x5, 4, 0x33)).unwrap(),
        RiscvInstruction::Sra {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x6, 4, 0x33)).unwrap(),
        RiscvInstruction::Or {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x7, 4, 0x33)).unwrap(),
        RiscvInstruction::And {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
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
        RiscvInstruction::decode(shift_i_type_with_funct6(0x10, 4, 3, 0x5, 3)).unwrap(),
        RiscvInstruction::Srai {
            rd: reg(3),
            rs1: reg(3),
            shamt: 4,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(i_type(-1, 3, 0x0, 4, 0x1b)).unwrap(),
        RiscvInstruction::Addiw {
            rd: reg(4),
            rs1: reg(3),
            imm: Immediate::new(-1),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(shift_i32_type_with_funct7(0x00, 4, 3, 0x1, 4)).unwrap(),
        RiscvInstruction::Slliw {
            rd: reg(4),
            rs1: reg(3),
            shamt: 4,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(shift_i32_type_with_funct7(0x00, 4, 3, 0x5, 4)).unwrap(),
        RiscvInstruction::Srliw {
            rd: reg(4),
            rs1: reg(3),
            shamt: 4,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(shift_i32_type_with_funct7(0x20, 4, 3, 0x5, 4)).unwrap(),
        RiscvInstruction::Sraiw {
            rd: reg(4),
            rs1: reg(3),
            shamt: 4,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x0, 4, 0x3b)).unwrap(),
        RiscvInstruction::Addw {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x20, 6, 5, 0x0, 4, 0x3b)).unwrap(),
        RiscvInstruction::Subw {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x1, 4, 0x3b)).unwrap(),
        RiscvInstruction::Sllw {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x00, 6, 5, 0x5, 4, 0x3b)).unwrap(),
        RiscvInstruction::Srlw {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x20, 6, 5, 0x5, 4, 0x3b)).unwrap(),
        RiscvInstruction::Sraw {
            rd: reg(4),
            rs1: reg(5),
            rs2: reg(6),
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
        RiscvInstruction::decode(b_type(12, 6, 5, 0x4)).unwrap(),
        RiscvInstruction::Blt {
            rs1: reg(5),
            rs2: reg(6),
            offset: Immediate::new(12),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(b_type(-12, 6, 5, 0x5)).unwrap(),
        RiscvInstruction::Bge {
            rs1: reg(5),
            rs2: reg(6),
            offset: Immediate::new(-12),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(b_type(20, 6, 5, 0x6)).unwrap(),
        RiscvInstruction::Bltu {
            rs1: reg(5),
            rs2: reg(6),
            offset: Immediate::new(20),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(b_type(-20, 6, 5, 0x7)).unwrap(),
        RiscvInstruction::Bgeu {
            rs1: reg(5),
            rs2: reg(6),
            offset: Immediate::new(-20),
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
fn decoder_decompresses_rv64c_integer_control_and_memory_instructions() {
    let cases = [
        (
            compressed(0x441d),
            RiscvInstruction::Addi {
                rd: reg(8),
                rs1: reg(0),
                imm: Immediate::new(7),
            },
        ),
        (
            compressed(0x0405),
            RiscvInstruction::Addi {
                rd: reg(8),
                rs1: reg(8),
                imm: Immediate::new(1),
            },
        ),
        (
            compressed(0x0804),
            RiscvInstruction::Addi {
                rd: reg(9),
                rs1: reg(2),
                imm: Immediate::new(16),
            },
        ),
        (
            compressed(0x6c88),
            RiscvInstruction::Load {
                rd: reg(10),
                rs1: reg(9),
                offset: Immediate::new(24),
                width: MemoryWidth::Doubleword,
                signed: true,
            },
        ),
        (
            compressed(0xec88),
            RiscvInstruction::Store {
                rs1: reg(9),
                rs2: reg(10),
                offset: Immediate::new(24),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            compressed(0xa009),
            RiscvInstruction::Jal {
                rd: reg(0),
                offset: Immediate::new(2),
            },
        ),
        (
            compressed(0xe081),
            RiscvInstruction::Bne {
                rs1: reg(9),
                rs2: reg(0),
                offset: Immediate::new(0),
            },
        ),
        (
            compressed(0x9426),
            RiscvInstruction::Add {
                rd: reg(8),
                rs1: reg(8),
                rs2: reg(9),
            },
        ),
    ];

    for (raw, expected) in cases {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        assert_eq!(decoded.instruction(), expected);
        assert_eq!(decoded.bytes(), 2);
    }
}

#[test]
fn decoder_decompresses_rv64c_double_float_memory_instructions() {
    let cases = [
        (
            compressed(0x24a8),
            RiscvInstruction::FloatLoad {
                rd: freg(10),
                rs1: reg(9),
                offset: Immediate::new(72),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            compressed(0xa4a8),
            RiscvInstruction::FloatStore {
                rs1: reg(9),
                rs2: freg(10),
                offset: Immediate::new(72),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            compressed(0x21ae),
            RiscvInstruction::FloatLoad {
                rd: freg(3),
                rs1: reg(2),
                offset: Immediate::new(200),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            compressed(0xb592),
            RiscvInstruction::FloatStore {
                rs1: reg(2),
                rs2: freg(4),
                offset: Immediate::new(232),
                width: MemoryWidth::Doubleword,
            },
        ),
    ];

    for (raw, expected) in cases {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        assert_eq!(decoded.instruction(), expected);
        assert_eq!(decoded.bytes(), 2);
    }
}

#[test]
fn decoder_without_length_rejects_compressed_halfwords() {
    assert_eq!(
        RiscvInstruction::decode(compressed(0x441d)).unwrap_err(),
        RiscvError::CompressedNotSupported {
            raw: compressed(0x441d)
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

    hart.write(reg(7), (-2_i64) as u64);
    hart.write(reg(8), 1);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 8, 7, 0x2, 9, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(9)), 1);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 8, 7, 0x3, 10, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(10)), 0);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 7, 8, 0x3, 10, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(10)), 1);

    hart.write(reg(17), 0x00ff_00ff);
    hart.write(reg(18), 0x0f0f_0f0f);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 18, 17, 0x4, 19, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(19)), 0x0ff0_0ff0);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 18, 17, 0x6, 20, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(20)), 0x0fff_0fff);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 18, 17, 0x7, 21, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(21)), 0x000f_000f);

    hart.write(reg(22), 1);
    hart.write(reg(23), 68);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 23, 22, 0x1, 24, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(24)), 0x10);
    hart.write(reg(25), 0x100);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 23, 25, 0x5, 26, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(26)), 0x10);
    hart.write(reg(27), 0xffff_ffff_ffff_ff00);
    hart.execute(RiscvInstruction::decode(r_type(0x20, 23, 27, 0x5, 28, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(28)), 0xffff_ffff_ffff_fff0);

    hart.execute(RiscvInstruction::decode(i_type(-1, 0, 0x0, 4, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(4)), u64::MAX);
    hart.execute(RiscvInstruction::decode(i_type(1, 4, 0x0, 5, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 0);

    hart.write(reg(11), (-3_i64) as u64);
    hart.execute(RiscvInstruction::decode(i_type(-2, 11, 0x2, 12, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(12)), 1);
    hart.execute(RiscvInstruction::decode(i_type(-4, 11, 0x2, 13, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(13)), 0);

    hart.write(reg(14), 5);
    hart.execute(RiscvInstruction::decode(i_type(-1, 14, 0x3, 15, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(15)), 1);
    hart.write(reg(14), u64::MAX);
    hart.execute(RiscvInstruction::decode(i_type(-1, 14, 0x3, 15, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(15)), 0);

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

    hart.write(reg(16), 0xffff_ffff_ffff_f000);
    hart.execute(RiscvInstruction::decode(shift_i_type_with_funct6(0x10, 4, 16, 0x5, 16)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(16)), 0xffff_ffff_ffff_ff00);
}

#[test]
fn hart_executes_word_integer_operations_with_sign_extension() {
    let mut hart = RiscvHartState::new(0x9000);

    hart.write(reg(1), 0x7fff_ffff);
    hart.execute(RiscvInstruction::decode(i_type(1, 1, 0x0, 2, 0x1b)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(2)), 0xffff_ffff_8000_0000);

    hart.write(reg(3), 0x4000_0000);
    hart.execute(RiscvInstruction::decode(shift_i32_type_with_funct7(0x00, 1, 3, 0x1, 4)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(4)), 0xffff_ffff_8000_0000);

    hart.write(reg(5), 0x8000_0000);
    hart.execute(RiscvInstruction::decode(shift_i32_type_with_funct7(0x00, 4, 5, 0x5, 6)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(6)), 0x0800_0000);
    hart.execute(RiscvInstruction::decode(shift_i32_type_with_funct7(0x20, 4, 5, 0x5, 7)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(7)), 0xffff_ffff_f800_0000);

    hart.write(reg(8), 0x7fff_ffff);
    hart.write(reg(9), 1);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 9, 8, 0x0, 10, 0x3b)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(10)), 0xffff_ffff_8000_0000);

    hart.write(reg(11), 0);
    hart.write(reg(12), 1);
    hart.execute(RiscvInstruction::decode(r_type(0x20, 12, 11, 0x0, 13, 0x3b)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(13)), u64::MAX);

    hart.write(reg(14), 1);
    hart.write(reg(15), 36);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 15, 14, 0x1, 16, 0x3b)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(16)), 0x10);

    hart.write(reg(17), 0x100);
    hart.execute(RiscvInstruction::decode(r_type(0x00, 15, 17, 0x5, 18, 0x3b)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(18)), 0x10);

    hart.write(reg(19), 0xffff_ffff_ffff_ff00);
    hart.execute(RiscvInstruction::decode(r_type(0x20, 15, 19, 0x5, 20, 0x3b)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(20)), 0xffff_ffff_ffff_fff0);
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
fn hart_executes_integer_branch_comparisons() {
    assert_branch_next_pc(0x4, (-2_i64) as u64, 1, 0x2010);
    assert_branch_next_pc(0x4, 3, (-1_i64) as u64, 0x2004);
    assert_branch_next_pc(0x5, 2, (-1_i64) as u64, 0x2010);
    assert_branch_next_pc(0x5, (-2_i64) as u64, 1, 0x2004);
    assert_branch_next_pc(0x6, 1, u64::MAX, 0x2010);
    assert_branch_next_pc(0x6, u64::MAX, 1, 0x2004);
    assert_branch_next_pc(0x7, u64::MAX, 1, 0x2010);
    assert_branch_next_pc(0x7, 1, u64::MAX, 0x2004);
}

fn assert_branch_next_pc(funct3: u32, left: u64, right: u64, expected_pc: u64) {
    let mut hart = RiscvHartState::new(0x2000);

    hart.write(reg(1), left);
    hart.write(reg(2), right);

    let outcome = hart
        .execute(RiscvInstruction::decode(b_type(16, 2, 1, funct3)).unwrap())
        .unwrap();

    assert_eq!(outcome.next_pc(), expected_pc);
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
fn hart_reports_wait_for_interrupt_as_system_event() {
    let instruction = RiscvInstruction::decode(0x1050_0073).unwrap();
    assert_eq!(instruction, RiscvInstruction::WaitForInterrupt);

    let mut hart = RiscvHartState::new(0xc000);
    hart.write(reg(1), 0x1234);

    let record = hart.execute(instruction).unwrap();

    assert_eq!(record.next_pc(), 0xc004);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
    assert_eq!(record.trap(), None);
    assert_eq!(
        record.system_event(),
        Some(&RiscvSystemEvent::WaitForInterrupt { pc: 0xc000 })
    );
    assert_eq!(hart.pc(), 0xc004);
    assert_eq!(hart.read(reg(1)), 0x1234);
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(1, 1));

    for raw in [0x1050_00f3, 0x1050_8073] {
        assert_eq!(
            RiscvInstruction::decode(raw),
            Err(RiscvError::UnknownEncoding { raw })
        );
    }
}

#[test]
fn hart_reports_sfence_vma_as_system_event() {
    let instruction = RiscvInstruction::decode(sfence_vma_type(5, 6, 0, 0)).unwrap();
    assert_eq!(
        instruction,
        RiscvInstruction::SfenceVma {
            rs1: reg(5),
            rs2: reg(6),
        }
    );

    let mut hart = RiscvHartState::new(0xd000);
    hart.write(reg(5), 0xffff_0000_8000_1000);
    hart.write(reg(6), 0x2a);

    let record = hart.execute(instruction).unwrap();

    assert_eq!(record.next_pc(), 0xd004);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
    assert_eq!(record.trap(), None);
    assert_eq!(
        record.system_event(),
        Some(&RiscvSystemEvent::SfenceVma {
            pc: 0xd000,
            virtual_address: Some(0xffff_0000_8000_1000),
            address_space: Some(0x2a),
        })
    );
    assert_eq!(hart.counter_snapshot(), RiscvCounterSnapshot::new(1, 1));

    let all_scope = RiscvInstruction::decode(sfence_vma_type(0, 0, 0, 0)).unwrap();
    let all_scope_record = hart.execute(all_scope).unwrap();
    assert_eq!(
        all_scope_record.system_event(),
        Some(&RiscvSystemEvent::SfenceVma {
            pc: 0xd004,
            virtual_address: None,
            address_space: None,
        })
    );
    assert_eq!(hart.pc(), 0xd008);

    for raw in [sfence_vma_type(0, 0, 1, 0), sfence_vma_type(0, 0, 0, 1)] {
        assert_eq!(
            RiscvInstruction::decode(raw),
            Err(RiscvError::UnknownEncoding { raw })
        );
    }
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
fn hart_takes_machine_trap_for_environment_call() {
    let mut hart = RiscvHartState::new(0x7000);
    hart.set_machine_trap_vector(0x8001);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_status(RiscvStatusWord::new(0).with_mie(true));

    let ecall = hart
        .execute(RiscvInstruction::decode(0x0000_0073).unwrap())
        .unwrap();

    assert_eq!(ecall.pc(), 0x7000);
    assert_eq!(ecall.next_pc(), 0x8000);
    assert_eq!(hart.pc(), 0x8000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7000);
    assert_eq!(hart.machine_trap_cause(), 9);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::Supervisor);
    assert!(hart.status().mpie());
    assert!(!hart.status().mie());
    assert_eq!(
        ecall.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x7000))
    );
    assert_eq!(ecall.register_writes(), &[]);
    assert_eq!(ecall.memory_access(), None);

    let mret = hart
        .execute(RiscvInstruction::decode(0x3020_0073).unwrap())
        .unwrap();
    assert_eq!(mret.next_pc(), 0x7000);
    assert_eq!(hart.pc(), 0x7000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert!(hart.status().mie());
}

#[test]
fn hart_takes_machine_trap_for_breakpoint() {
    let mut hart = RiscvHartState::new(0x7100);
    hart.set_machine_trap_vector(0x9000);
    hart.set_status(RiscvStatusWord::new(0).with_mie(false));

    let ebreak = hart
        .execute(RiscvInstruction::decode(0x0010_0073).unwrap())
        .unwrap();

    assert_eq!(ebreak.pc(), 0x7100);
    assert_eq!(ebreak.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7100);
    assert_eq!(hart.machine_trap_cause(), 3);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::Machine);
    assert!(!hart.status().mpie());
    assert!(!hart.status().mie());
    assert_eq!(
        ebreak.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::Breakpoint, 0x7100))
    );
    assert_eq!(ebreak.register_writes(), &[]);
    assert_eq!(ebreak.memory_access(), None);
}

#[test]
fn hart_records_compressed_trap_instruction_length() {
    let mut hart = RiscvHartState::new(0x7200);
    hart.set_machine_trap_vector(0x9001);

    let decoded = RiscvInstruction::decode_with_length(compressed(0x9002)).unwrap();
    assert_eq!(decoded.instruction(), RiscvInstruction::Ebreak);
    assert_eq!(decoded.bytes(), 2);

    let record = hart.execute_decoded(decoded).unwrap();

    assert_eq!(record.pc(), 0x7200);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(record.instruction_bytes(), 2);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.machine_exception_pc(), 0x7200);
    assert_eq!(hart.machine_trap_cause(), 3);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::Breakpoint, 0x7200))
    );
}

#[test]
fn hart_records_compressed_interrupted_instruction_length() {
    let mut hart = RiscvHartState::new(0x7300);
    hart.set_machine_trap_vector(0x9400);
    hart.set_machine_interrupt_enable(1 << 1);
    hart.set_machine_interrupt_pending(1 << 1);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);

    let decoded = RiscvInstruction::decode_with_length(compressed(0x0001)).unwrap();
    assert_eq!(decoded.bytes(), 2);

    let record = hart.execute_decoded(decoded).unwrap();

    assert_eq!(record.pc(), 0x7300);
    assert_eq!(record.next_pc(), 0x9400);
    assert_eq!(record.instruction_bytes(), 2);
    assert_eq!(hart.pc(), 0x9400);
    assert_eq!(hart.machine_exception_pc(), 0x7300);
    assert_eq!(hart.machine_trap_cause(), (1_u64 << 63) | 1);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::Interrupt { code: 1 },
            0x7300
        ))
    );
    assert_eq!(record.register_writes(), &[]);
}

#[test]
fn hart_decodes_and_records_gem5_work_marker_pseudo_ops() {
    let work_begin = RiscvInstruction::decode(gem5_m5op_type(0x5a)).unwrap();
    let work_end = RiscvInstruction::decode(gem5_m5op_type(0x5b)).unwrap();

    assert_eq!(
        work_begin,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::WorkBegin)
    );
    assert_eq!(
        work_end,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::WorkEnd)
    );

    let mut hart = RiscvHartState::new(0x7200);
    hart.write(reg(10), 0x51);
    hart.write(reg(11), 0x9);

    let begin = hart.execute(work_begin).unwrap();
    assert_eq!(begin.pc(), 0x7200);
    assert_eq!(begin.next_pc(), 0x7204);
    assert_eq!(hart.pc(), 0x7204);
    assert_eq!(begin.register_writes(), &[RegisterWrite::new(reg(10), 0)]);
    assert_eq!(hart.read(reg(10)), 0);
    assert_eq!(begin.memory_access(), None);
    assert_eq!(
        begin.system_event(),
        Some(&RiscvSystemEvent::Gem5WorkBegin {
            pc: 0x7200,
            work_id: 0x51,
            thread_id: 0x9,
        })
    );

    hart.write(reg(10), 0x52);
    hart.write(reg(11), 0xa);
    let end = hart.execute(work_end).unwrap();

    assert_eq!(end.pc(), 0x7204);
    assert_eq!(end.next_pc(), 0x7208);
    assert_eq!(hart.pc(), 0x7208);
    assert_eq!(end.register_writes(), &[RegisterWrite::new(reg(10), 0)]);
    assert_eq!(hart.read(reg(10)), 0);
    assert_eq!(end.memory_access(), None);
    assert_eq!(
        end.system_event(),
        Some(&RiscvSystemEvent::Gem5WorkEnd {
            pc: 0x7204,
            work_id: 0x52,
            thread_id: 0xa,
        })
    );
}

#[test]
fn hart_decodes_and_records_gem5_exit_fail_pseudo_ops() {
    let exit = RiscvInstruction::decode(gem5_m5op_type(0x21)).unwrap();
    let fail = RiscvInstruction::decode(gem5_m5op_type(0x22)).unwrap();

    assert_eq!(exit, RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Exit));
    assert_eq!(fail, RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Fail));

    let mut hart = RiscvHartState::new(0x7300);
    hart.write(reg(10), 0);
    let exit_record = hart.execute(exit).unwrap();
    assert_eq!(exit_record.pc(), 0x7300);
    assert_eq!(exit_record.next_pc(), 0x7304);
    assert_eq!(hart.pc(), 0x7304);
    assert_eq!(
        exit_record.system_event(),
        Some(&RiscvSystemEvent::Gem5Exit {
            pc: 0x7300,
            delay: 0,
        })
    );
    assert_eq!(
        exit_record.register_writes(),
        &[RegisterWrite::new(reg(10), 0)]
    );
    assert_eq!(hart.read(reg(10)), 0);

    hart.write(reg(10), 3);
    hart.write(reg(11), 7);
    let fail_record = hart.execute(fail).unwrap();
    assert_eq!(fail_record.pc(), 0x7304);
    assert_eq!(fail_record.next_pc(), 0x7308);
    assert_eq!(hart.pc(), 0x7308);
    assert_eq!(
        fail_record.system_event(),
        Some(&RiscvSystemEvent::Gem5Fail {
            pc: 0x7304,
            delay: 3,
            code: 7,
        })
    );
    assert_eq!(
        fail_record.register_writes(),
        &[RegisterWrite::new(reg(10), 0)]
    );
    assert_eq!(hart.read(reg(10)), 0);
}

#[test]
fn hart_decodes_and_executes_gem5_sum_pseudo_op() {
    let sum = RiscvInstruction::decode(gem5_m5op_type(0x23)).unwrap();

    assert_eq!(sum, RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Sum));

    let mut hart = RiscvHartState::new(0x7310);
    for (index, value) in (10..=15).zip([1, 2, 3, 4, 5, 6]) {
        hart.write(reg(index), value);
    }

    let record = hart.execute(sum).unwrap();
    assert_eq!(record.pc(), 0x7310);
    assert_eq!(record.next_pc(), 0x7314);
    assert_eq!(hart.pc(), 0x7314);
    assert_eq!(record.system_event(), None);
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(10), 21)]);
    assert_eq!(hart.read(reg(10)), 21);
}

#[test]
fn hart_decodes_and_records_gem5_stats_pseudo_ops() {
    let reset = RiscvInstruction::decode(gem5_m5op_type(0x40)).unwrap();
    let dump = RiscvInstruction::decode(gem5_m5op_type(0x41)).unwrap();
    let dump_reset = RiscvInstruction::decode(gem5_m5op_type(0x42)).unwrap();

    assert_eq!(
        reset,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::ResetStats)
    );
    assert_eq!(
        dump,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::DumpStats)
    );
    assert_eq!(
        dump_reset,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::DumpResetStats)
    );

    let mut hart = RiscvHartState::new(0x7400);
    hart.write(reg(10), 4);
    hart.write(reg(11), 0x20);
    let reset_record = hart.execute(reset).unwrap();
    assert_eq!(reset_record.pc(), 0x7400);
    assert_eq!(reset_record.next_pc(), 0x7404);
    assert_eq!(hart.pc(), 0x7404);
    assert_eq!(
        reset_record.system_event(),
        Some(&RiscvSystemEvent::Gem5ResetStats {
            pc: 0x7400,
            delay: 4,
            period: 0x20,
        })
    );
    assert_eq!(
        reset_record.register_writes(),
        &[RegisterWrite::new(reg(10), 0)]
    );
    assert_eq!(reset_record.memory_access(), None);
    assert_eq!(hart.read(reg(10)), 0);

    hart.write(reg(10), 5);
    hart.write(reg(11), 0x21);
    let dump_record = hart.execute(dump).unwrap();
    assert_eq!(dump_record.pc(), 0x7404);
    assert_eq!(dump_record.next_pc(), 0x7408);
    assert_eq!(hart.pc(), 0x7408);
    assert_eq!(
        dump_record.system_event(),
        Some(&RiscvSystemEvent::Gem5DumpStats {
            pc: 0x7404,
            delay: 5,
            period: 0x21,
        })
    );
    assert_eq!(
        dump_record.register_writes(),
        &[RegisterWrite::new(reg(10), 0)]
    );
    assert_eq!(dump_record.memory_access(), None);
    assert_eq!(hart.read(reg(10)), 0);

    hart.write(reg(10), 6);
    hart.write(reg(11), 0x22);
    let dump_reset_record = hart.execute(dump_reset).unwrap();
    assert_eq!(dump_reset_record.pc(), 0x7408);
    assert_eq!(dump_reset_record.next_pc(), 0x740c);
    assert_eq!(hart.pc(), 0x740c);
    assert_eq!(
        dump_reset_record.system_event(),
        Some(&RiscvSystemEvent::Gem5DumpResetStats {
            pc: 0x7408,
            delay: 6,
            period: 0x22,
        })
    );
    assert_eq!(
        dump_reset_record.register_writes(),
        &[RegisterWrite::new(reg(10), 0)]
    );
    assert_eq!(dump_reset_record.memory_access(), None);
    assert_eq!(hart.read(reg(10)), 0);
}

#[test]
fn hart_decodes_and_records_gem5_checkpoint_pseudo_op() {
    let checkpoint = RiscvInstruction::decode(gem5_m5op_type(0x43)).unwrap();

    assert_eq!(
        checkpoint,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Checkpoint)
    );

    let mut hart = RiscvHartState::new(0x7500);
    hart.write(reg(10), 2);
    hart.write(reg(11), 0x30);
    let record = hart.execute(checkpoint).unwrap();

    assert_eq!(record.pc(), 0x7500);
    assert_eq!(record.next_pc(), 0x7504);
    assert_eq!(hart.pc(), 0x7504);
    assert_eq!(
        record.system_event(),
        Some(&RiscvSystemEvent::Gem5Checkpoint {
            pc: 0x7500,
            delay: 2,
            period: 0x30,
        })
    );
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(10), 0)]);
    assert_eq!(record.memory_access(), None);
    assert_eq!(hart.read(reg(10)), 0);
}

#[test]
fn hart_decodes_and_records_gem5_switch_cpu_pseudo_op() {
    let switch_cpu = RiscvInstruction::decode(gem5_m5op_type(0x52)).unwrap();

    assert_eq!(
        switch_cpu,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::SwitchCpu)
    );

    let mut hart = RiscvHartState::new(0x7580);
    hart.write(reg(10), 0x41);
    hart.write(reg(11), 0x42);
    let record = hart.execute(switch_cpu).unwrap();

    assert_eq!(record.pc(), 0x7580);
    assert_eq!(record.next_pc(), 0x7584);
    assert_eq!(hart.pc(), 0x7584);
    assert_eq!(
        record.system_event(),
        Some(&RiscvSystemEvent::Gem5SwitchCpu { pc: 0x7580 })
    );
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(10), 0)]);
    assert_eq!(record.memory_access(), None);
    assert_eq!(hart.read(reg(10)), 0);
    assert_eq!(hart.read(reg(11)), 0x42);
}

#[test]
fn hart_decodes_and_records_gem5_hypercall_pseudo_op() {
    let hypercall = RiscvInstruction::decode(gem5_m5op_type(0x71)).unwrap();

    assert_eq!(
        hypercall,
        RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Hypercall)
    );

    let mut hart = RiscvHartState::new(0x7600);
    hart.write(reg(10), 0x7101);
    hart.write(reg(11), 0x22);
    hart.write(reg(12), 0x33);
    hart.write(reg(13), 0x44);
    hart.write(reg(14), 0x55);
    hart.write(reg(15), 0x66);
    let record = hart.execute(hypercall).unwrap();

    assert_eq!(record.pc(), 0x7600);
    assert_eq!(record.next_pc(), 0x7604);
    assert_eq!(hart.pc(), 0x7604);
    assert_eq!(
        record.system_event(),
        Some(&RiscvSystemEvent::Gem5Hypercall {
            pc: 0x7600,
            selector: 0x7101,
            arguments: [0x22, 0x33, 0x44, 0x55, 0x66],
        })
    );
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(10), 0)]);
    assert_eq!(record.memory_access(), None);
    assert_eq!(hart.read(reg(10)), 0);
    assert_eq!(hart.read(reg(11)), 0x22);
}

#[test]
fn hart_executes_machine_return_from_machine_mode() {
    let mut hart = RiscvHartState::new(0x7000);
    hart.set_machine_exception_pc(0x9000);
    hart.set_status(
        RiscvStatusWord::new(0)
            .with_mpp(RiscvPrivilegeMode::Supervisor)
            .with_mpie(true)
            .with_mie(false)
            .with_mprv(true),
    );

    let instruction = RiscvInstruction::decode(0x3020_0073).unwrap();
    assert_eq!(instruction, RiscvInstruction::MachineReturn);

    let record = hart.execute(instruction).unwrap();

    assert_eq!(record.pc(), 0x7000);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert!(hart.status().mie());
    assert!(hart.status().mpie());
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::User);
    assert!(!hart.status().mprv());
    assert_eq!(record.trap(), None);
    assert_eq!(record.system_event(), None);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_executes_supervisor_return_from_supervisor_mode() {
    let mut hart = RiscvHartState::new(0x7200);
    hart.set_supervisor_exception_pc(0x9300);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_status(
        RiscvStatusWord::new(0)
            .with_spp(RiscvPrivilegeMode::User)
            .with_spie(true)
            .with_sie(false)
            .with_mprv(true),
    );

    let instruction = RiscvInstruction::decode(0x1020_0073).unwrap();
    assert_eq!(instruction, RiscvInstruction::SupervisorReturn);

    let record = hart.execute(instruction).unwrap();

    assert_eq!(record.pc(), 0x7200);
    assert_eq!(record.next_pc(), 0x9300);
    assert_eq!(hart.pc(), 0x9300);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::User);
    assert!(hart.status().sie());
    assert!(hart.status().spie());
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(!hart.status().mprv());
    assert_eq!(record.trap(), None);
    assert_eq!(record.system_event(), None);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
}

#[test]
fn decoder_rejects_reserved_compressed_and_unknown_encodings() {
    assert_eq!(
        RiscvInstruction::decode_with_length(0x0000_0000).unwrap_err(),
        RiscvError::UnknownEncoding { raw: 0x0000_0000 }
    );
    assert_eq!(
        RiscvInstruction::decode(0x0000_0000).unwrap_err(),
        RiscvError::CompressedNotSupported { raw: 0x0000_0000 }
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
