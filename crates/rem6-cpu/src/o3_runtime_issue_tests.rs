use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction, RiscvHartState,
    RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::o3_runtime_issue::{O3LiveIssueHeadReservation, O3LiveIssueRequest};
use super::*;
use crate::{CpuFetchEvent, CpuFetchRecord, RiscvCpuExecutionEvent};

#[test]
fn scoped_issue_reserves_head_width() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);

    fixture.schedule_all(20);

    assert_eq!(fixture.issue_tick(BRANCH_PC), 21);
    assert_eq!(fixture.issue_tick(MUL_PC), 22);
    assert_eq!(fixture.issue_tick(THIRD_PC), 23);
}

#[test]
fn scoped_issue_allows_cross_resource_peer() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);

    fixture.schedule_all(20);

    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(MUL_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
}

#[test]
fn scoped_issue_serializes_same_multiply_class() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::SameMultiply);

    fixture.schedule_all(20);

    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(MUL_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 22);
}

#[test]
fn scoped_issue_waits_for_register_producer_ready_tick() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);

    fixture.schedule_all(20);

    let multiply = fixture.execution_at(MUL_PC);
    let dependent = fixture.execution_at(THIRD_PC);
    assert_eq!(multiply.issue_tick, 21);
    assert_eq!(
        dependent.issue_tick,
        multiply
            .issue_tick
            .saturating_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                multiply.execution.instruction(),
            ))
    );
}

#[test]
fn scoped_issue_partial_reentry_does_not_overbook_prior_tick() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let branch = fixture.requests[0].clone();

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[branch]);
    fixture.schedule_all(20);

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(MUL_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
}

#[test]
fn scoped_issue_rollback_uses_existing_producer_chain() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    fixture.schedule_all(20);
    let rob = fixture.runtime.snapshot().reorder_buffer().to_vec();
    let branch_sequence = rob[1].sequence();
    let multiply_sequence = rob[2].sequence();
    let dependent_sequence = rob[3].sequence();

    let multiply = fixture.execution_at(MUL_PC);
    assert!(
        multiply.producer_sequences.contains(&branch_sequence),
        "control owner must remain in the producer chain for rollback"
    );
    let dependent = fixture.execution_at(THIRD_PC);
    assert!(
        dependent.producer_sequences.contains(&branch_sequence),
        "dependent row must retain branch rollback ownership"
    );
    assert!(
        dependent.producer_sequences.contains(&multiply_sequence),
        "dependent row must retain data producer ownership"
    );

    let branch_execution = fixture.execution_at(BRANCH_PC).execution.clone();
    fixture.runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(BRANCH_PC, 99), branch(), branch_execution),
        &[request(99)],
        40,
    );

    assert!(fixture.runtime.live_speculative_executions.is_empty());
    assert!(fixture
        .runtime
        .live_control_dependencies
        .keys()
        .all(|sequence| *sequence != dependent_sequence));
}

const LOAD_PC: u64 = 0x8000;
const BRANCH_PC: u64 = 0x8004;
const MUL_PC: u64 = 0x8008;
const THIRD_PC: u64 = 0x800c;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarIssueCase {
    CrossResource,
    SameMultiply,
    Dependent,
}

struct ScalarIssueFixture {
    runtime: O3RuntimeState,
    hart: RiscvHartState,
    head: O3LiveIssueHeadReservation,
    requests: Vec<O3LiveIssueRequest>,
}

impl ScalarIssueFixture {
    fn new(issue_width: usize, case: ScalarIssueCase) -> Self {
        let mut runtime = O3RuntimeState::default();
        runtime.set_issue_width(issue_width);
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 20));
        let younger = match case {
            ScalarIssueCase::CrossResource => [branch(), mul(14, 2, 3), addi(15, 4, 1)],
            ScalarIssueCase::SameMultiply => [branch(), mul(14, 2, 3), mul(15, 4, 5)],
            ScalarIssueCase::Dependent => [branch(), mul(14, 2, 3), addi(15, 14, 5)],
        };
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [BRANCH_PC, MUL_PC, THIRD_PC]
                .into_iter()
                .zip(younger)
                .map(|(pc, instruction)| (Address::new(pc), instruction)),
        );
        let head = runtime
            .live_scalar_memory_head_reservation(load.fetch().request_id())
            .expect("scalar load head reservation");
        let requests = [BRANCH_PC, MUL_PC, THIRD_PC]
            .into_iter()
            .zip(younger)
            .enumerate()
            .map(|(index, (pc, instruction))| {
                O3LiveIssueRequest::new(
                    Address::new(pc),
                    vec![request(11 + index as u64)],
                    decoded(instruction),
                )
            })
            .collect::<Vec<_>>();
        let mut hart = RiscvHartState::new(LOAD_PC);
        for (register, value) in [(2, 7), (3, 11), (4, 17), (5, 2), (6, 1), (7, 2)] {
            hart.write(reg(register), value);
        }
        Self {
            runtime,
            hart,
            head,
            requests,
        }
    }

    fn schedule_all(&mut self, earliest_tick: u64) {
        self.runtime.schedule_live_speculative_issues(
            &self.hart,
            self.head,
            earliest_tick,
            &self.requests,
        );
    }

    fn issue_tick(&self, pc: u64) -> u64 {
        self.execution_at(pc).issue_tick
    }

    fn execution_at(&self, pc: u64) -> &O3LiveSpeculativeExecution {
        self.runtime
            .live_speculative_executions
            .iter()
            .find(|execution| execution.execution.pc() == pc)
            .unwrap_or_else(|| panic!("missing speculative execution at {pc:#x}"))
    }

    fn executions_at(&self, pc: u64) -> usize {
        self.runtime
            .live_speculative_executions
            .iter()
            .filter(|execution| execution.execution.pc() == pc)
            .count()
    }
}

fn scalar_load_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(12),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(LOAD_PC, 10),
        instruction,
        rem6_isa_riscv::RiscvExecutionRecord::new(
            instruction,
            LOAD_PC,
            BRANCH_PC,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(12),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            }),
        ),
    )
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn decoded(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    RiscvInstruction::decode_with_length(raw(instruction)).unwrap()
}

fn raw(instruction: RiscvInstruction) -> u32 {
    match instruction {
        RiscvInstruction::Addi { rd, rs1, imm } => {
            i_type(imm.value(), rs1.index(), 0x0, rd.index(), 0x13)
        }
        RiscvInstruction::Beq { rs1, rs2, offset } => {
            b_type(offset.value(), rs2.index(), rs1.index(), 0b000)
        }
        RiscvInstruction::Mul { rd, rs1, rs2 } => {
            r_type(1, rs2.index(), rs1.index(), 0x0, rd.index(), 0x33)
        }
        _ => panic!("unsupported test instruction: {instruction:?}"),
    }
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn addi(rd: u8, rs1: u8, immediate: i64) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(immediate),
    }
}

fn branch() -> RiscvInstruction {
    RiscvInstruction::Beq {
        rs1: reg(6),
        rs2: reg(7),
        offset: Immediate::new(8),
    }
}

fn mul(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Mul {
        rd: reg(rd),
        rs1: reg(rs1),
        rs2: reg(rs2),
    }
}

fn i_type(imm: i64, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0xfff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn b_type(imm: i64, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    ((imm >> 12) & 0x1) << 31
        | ((imm >> 5) & 0x3f) << 25
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm >> 1) & 0xf) << 8
        | ((imm >> 11) & 0x1) << 7
        | 0x63
}
