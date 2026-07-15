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
    assert_eq!(fixture.issue_tick(SECOND_PC), 22);
    assert_eq!(fixture.issue_tick(THIRD_PC), 23);
}

#[test]
fn scoped_issue_allows_cross_resource_peer() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);

    fixture.schedule_all(20);

    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
}

#[test]
fn scoped_issue_serializes_same_multiply_class() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::SameMultiply);

    fixture.schedule_all(20);

    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 22);
}

#[test]
fn scoped_issue_serializes_mixed_control_kinds_on_branch_port() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::MixedControls);

    fixture.schedule_all(20);

    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 22);
}

#[test]
fn scoped_issue_linked_controls_reserve_writeback_and_serialize_return() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::LinkedControls);
    assert!(fixture.runtime.set_writeback_width(1));

    fixture.schedule_all(20);

    let call = fixture.execution_at(BRANCH_PC);
    let addi = fixture.execution_at(SECOND_PC);
    let return_control = fixture.execution_at(THIRD_PC);
    assert_eq!(call.issue_tick, 20);
    assert_eq!(addi.issue_tick, 20);
    assert_eq!(return_control.issue_tick, 21);
    assert_eq!(call.raw_ready_tick, 20);
    assert_eq!(call.admitted_writeback_tick, 20);
    assert_eq!(call.writeback_slot, Some(0));
    assert_eq!(addi.raw_ready_tick, 20);
    assert_eq!(addi.admitted_writeback_tick, 21);
    assert_eq!(addi.writeback_slot, Some(0));
    assert_eq!(return_control.raw_ready_tick, 21);
    assert_eq!(return_control.admitted_writeback_tick, 21);
    assert_eq!(return_control.writeback_slot, None);
}

#[test]
fn scoped_issue_orders_same_window_call_return_and_descendant() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    assert!(fixture.runtime.set_writeback_width(1));

    fixture.schedule_all(20);

    let call = fixture.execution_at(BRANCH_PC);
    let return_control = fixture.execution_at(SECOND_PC);
    let descendant = fixture.execution_at(THIRD_PC);
    assert_eq!(call.issue_tick, 20);
    assert_eq!(call.admitted_writeback_tick, 20);
    assert_eq!(call.writeback_slot, Some(0));
    assert_eq!(return_control.issue_tick, call.admitted_writeback_tick + 1);
    assert_eq!(return_control.writeback_slot, None);
    assert_eq!(
        descendant.issue_tick,
        return_control.admitted_writeback_tick + 1
    );
    assert!(return_control.producer_sequences.contains(&call.sequence));
    assert!(descendant
        .producer_sequences
        .contains(&return_control.sequence));
    let stats = fixture.runtime.stats();
    assert_eq!(stats.issue_cycles(), 3);
    assert_eq!(stats.issued_rows(), 3);
    assert_eq!(stats.resource_blocked_row_cycles(), 1);
    assert_eq!(stats.dependency_blocked_row_cycles(), 2);
    assert_eq!(stats.max_rows_per_cycle(), 2);
}

#[test]
fn scoped_issue_waits_for_register_producer_ready_tick() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);

    fixture.schedule_all(20);

    let multiply = fixture.execution_at(SECOND_PC);
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
fn scoped_issue_waits_for_admitted_writeback_tick() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::Dependent);
    assert!(fixture.runtime.set_writeback_width(1));

    fixture.schedule_all(20);

    let multiply = fixture.execution_at(SECOND_PC);
    let dependent = fixture.execution_at(THIRD_PC);
    assert_eq!(
        dependent.issue_tick, multiply.admitted_writeback_tick,
        "dependent issue must use the producer's admitted writeback tick"
    );
    assert_eq!(
        dependent.raw_ready_tick, dependent.issue_tick,
        "the dependent ADDI itself has zero execution latency"
    );
    assert_eq!(
        dependent.admitted_writeback_tick,
        multiply.admitted_writeback_tick + 1,
        "the consumer writeback must move to the next free slot after issuing in the producer writeback cycle"
    );
    assert_eq!(multiply.writeback_slot, Some(0));
    assert_eq!(dependent.writeback_slot, Some(0));
}

#[test]
fn issue_arbitration_width_one_records_scheduler_decisions() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);

    fixture.schedule_all(20);

    let stats = fixture.runtime.stats();
    assert_eq!(stats.issued_rows(), 3);
    assert!(stats.issue_cycles() >= 3);
    assert!(stats.resource_blocked_row_cycles() > 0);
    assert_eq!(stats.dependency_blocked_row_cycles(), 0);
    assert_eq!(stats.max_rows_per_cycle(), 1);
}

#[test]
fn issue_arbitration_records_dependency_blocked_row_cycles() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);

    fixture.schedule_all(20);

    assert!(fixture.runtime.stats().dependency_blocked_row_cycles() > 0);
}

#[test]
fn issue_arbitration_reset_stats_clears_scheduler_counters() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);
    fixture.schedule_all(20);
    assert!(!fixture.runtime.live_issue_cycle_ticks.is_empty());

    fixture.runtime.reset_stats();

    let stats = fixture.runtime.stats();
    assert_eq!(stats.issue_cycles(), 0);
    assert_eq!(stats.issued_rows(), 0);
    assert_eq!(stats.resource_blocked_row_cycles(), 0);
    assert_eq!(stats.dependency_blocked_row_cycles(), 0);
    assert_eq!(stats.max_rows_per_cycle(), 0);
    assert!(fixture.runtime.live_issue_cycle_ticks.is_empty());
}

#[test]
fn scoped_issue_tracks_long_fu_head_dependency() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_issue_width(1);
    let head_instruction = div(3, 1, 2);
    let independent = addi(4, 0, 5);
    let dependent = addi(5, 3, -11);
    let head_sequence = runtime
        .stage_live_retire_window(
            Address::new(LOAD_PC),
            head_instruction,
            31,
            [
                (Address::new(BRANCH_PC), independent),
                (Address::new(SECOND_PC), dependent),
            ],
        )
        .unwrap();
    let head = O3LiveIssueHeadReservation::for_instruction(head_sequence, 12, head_instruction);
    let mut hart = RiscvHartState::new(LOAD_PC);
    hart.write(reg(1), 84);
    hart.write(reg(2), 7);
    let mut head_hart = hart.clone();
    head_hart.set_pc(LOAD_PC);
    let head_execution = head_hart
        .execute_decoded(decoded(head_instruction))
        .unwrap();
    assert!(runtime
        .record_live_issue_head_execution(head, &[request(10)], head_execution,)
        .unwrap());
    let requests = [
        O3LiveIssueRequest::new(
            Address::new(BRANCH_PC),
            vec![request(11)],
            decoded(independent),
        ),
        O3LiveIssueRequest::new(
            Address::new(SECOND_PC),
            vec![request(12)],
            decoded(dependent),
        ),
    ];

    runtime
        .schedule_live_speculative_issues(&hart, head, 14, &requests)
        .unwrap();

    assert_eq!(runtime.live_speculative_executions[0].issue_tick, 12);
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .find(|execution| execution.execution.pc() == BRANCH_PC)
            .unwrap()
            .issue_tick,
        14
    );
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .find(|execution| execution.execution.pc() == SECOND_PC)
            .unwrap()
            .issue_tick,
        31
    );

    runtime.validate_live_speculative_producer(head_sequence);
    let dependent_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|execution| execution.execution.pc() == SECOND_PC)
        .unwrap()
        .execution
        .clone();
    let dependent_event =
        RiscvCpuExecutionEvent::new(fetch_event(SECOND_PC, 12), dependent, dependent_execution);
    let dependent_entry = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .copied()
        .find(|entry| entry.pc() == Address::new(SECOND_PC))
        .unwrap();
    assert_eq!(
        runtime.take_live_speculative_issue_timing_at(
            dependent_entry,
            &dependent_event,
            &[request(12)],
            0
        ),
        Some((31, 32)),
        "exact fetch identity must consume scheduler-owned issue and admitted writeback ticks"
    );
}

#[test]
fn scoped_issue_partial_reentry_does_not_overbook_prior_tick() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let branch_request = fixture.requests[0].clone();

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[branch_request])
        .unwrap();
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &fixture.requests[1..])
        .unwrap();

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
}

#[test]
fn scoped_issue_partial_reentry_keeps_unknown_return_owner_unresolved() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    assert!(fixture.runtime.set_writeback_width(1));
    let call_request = fixture.requests[0].clone();
    let return_request = fixture.requests[1].clone();
    let descendant_request = fixture.requests[2].clone();

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[call_request])
        .unwrap();
    fixture
        .runtime
        .schedule_live_speculative_issues(
            &fixture.hart,
            fixture.head,
            20,
            &[descendant_request.clone()],
        )
        .unwrap();

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 0);
    assert_eq!(fixture.executions_at(THIRD_PC), 0);

    fixture
        .runtime
        .schedule_live_speculative_issues(
            &fixture.hart,
            fixture.head,
            20,
            &[return_request, descendant_request],
        )
        .unwrap();

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 1);
    assert_eq!(fixture.executions_at(THIRD_PC), 1);
    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 22);
}

#[test]
fn scoped_issue_partial_reentry_rejects_wrong_no_destination_control_at_return_pc() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    assert!(fixture.runtime.set_writeback_width(1));
    let call_request = fixture.requests[0].clone();
    let descendant_request = fixture.requests[2].clone();
    let wrong_return_request =
        O3LiveIssueRequest::new(Address::new(SECOND_PC), vec![request(12)], decoded(jal()));

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &[call_request])
        .unwrap();
    fixture
        .runtime
        .schedule_live_speculative_issues(
            &fixture.hart,
            fixture.head,
            20,
            &[wrong_return_request, descendant_request],
        )
        .unwrap();

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 0);
    assert_eq!(fixture.executions_at(THIRD_PC), 0);
}

#[test]
fn scoped_issue_reversed_return_duplicate_uses_bound_fetch_identity() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    assert!(fixture.runtime.set_writeback_width(1));
    let real_return_request = fixture.requests[1].clone();
    let real_return_identity = vec![request(12)];
    let wrong_return_request = O3LiveIssueRequest::new(
        Address::new(SECOND_PC),
        vec![request(99)],
        decoded(jalr_return(1)),
    );
    let requests = [
        fixture.requests[0].clone(),
        wrong_return_request,
        real_return_request.clone(),
        fixture.requests[2].clone(),
    ];

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &requests)
        .unwrap();

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
    assert_eq!(fixture.executions_at(SECOND_PC), 1);
    assert_eq!(fixture.executions_at(THIRD_PC), 1);
    assert_eq!(
        fixture.execution_at(SECOND_PC).consumed_requests,
        real_return_identity
    );
    assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
    assert_eq!(fixture.issue_tick(SECOND_PC), 21);
    assert_eq!(fixture.issue_tick(THIRD_PC), 22);
}

#[test]
fn scoped_issue_deduplicates_repeated_request_for_one_rob_row() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let branch_request = fixture.requests[0].clone();
    let duplicate = O3LiveIssueRequest::new(
        Address::new(BRANCH_PC),
        vec![request(11)],
        decoded(branch()),
    );

    fixture
        .runtime
        .schedule_live_speculative_issues(
            &fixture.hart,
            fixture.head,
            20,
            &[branch_request, duplicate],
        )
        .unwrap();

    assert_eq!(fixture.executions_at(BRANCH_PC), 1);
}

#[test]
fn scoped_issue_records_selected_rows_in_sequence_order() {
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::CrossResource);
    fixture.requests.reverse();

    fixture.schedule_all(20);

    let expected = fixture.runtime.snapshot().reorder_buffer()[1..]
        .iter()
        .map(|entry| entry.sequence())
        .collect::<Vec<_>>();
    let actual = fixture
        .runtime
        .live_speculative_executions
        .iter()
        .map(|execution| execution.sequence)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn scoped_issue_rollback_uses_existing_producer_chain() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    fixture.schedule_all(20);
    let rob = fixture.runtime.snapshot().reorder_buffer().to_vec();
    let branch_sequence = rob[1].sequence();
    let multiply_sequence = rob[2].sequence();
    let dependent_sequence = rob[3].sequence();

    let multiply = fixture.execution_at(SECOND_PC);
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
const SECOND_PC: u64 = 0x8008;
const THIRD_PC: u64 = 0x800c;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarIssueCase {
    CrossResource,
    SameMultiply,
    Dependent,
    MixedControls,
    LinkedControls,
    SameWindowLinkReturn,
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
            ScalarIssueCase::MixedControls => [jal(), branch(), jalr()],
            ScalarIssueCase::LinkedControls => [jal_link(1), addi(14, 2, 3), jalr_return(5)],
            ScalarIssueCase::SameWindowLinkReturn => [jal_link(1), jalr_return(1), addi(14, 0, 7)],
        };
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [BRANCH_PC, SECOND_PC, THIRD_PC]
                .into_iter()
                .zip(younger)
                .map(|(pc, instruction)| (Address::new(pc), instruction)),
        );
        let head = runtime
            .live_scalar_memory_head_reservation(load.fetch().request_id())
            .expect("scalar load head reservation");
        let requests = [BRANCH_PC, SECOND_PC, THIRD_PC]
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
        for (index, (pc, instruction)) in [BRANCH_PC, SECOND_PC, THIRD_PC]
            .into_iter()
            .zip(younger)
            .enumerate()
        {
            assert!(runtime.bind_live_staged_fetch_identity(
                Address::new(pc),
                instruction,
                &[request(11 + index as u64)],
            ));
        }
        let mut hart = RiscvHartState::new(LOAD_PC);
        for (register, value) in [
            (2, 7),
            (3, 11),
            (4, 17),
            (5, 2),
            (6, 1),
            (7, 2),
            (9, THIRD_PC + 4),
        ] {
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
        self.runtime
            .schedule_live_speculative_issues(&self.hart, self.head, earliest_tick, &self.requests)
            .unwrap();
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
        RiscvInstruction::Div { rd, rs1, rs2 } => {
            r_type(1, rs2.index(), rs1.index(), 0x4, rd.index(), 0x33)
        }
        RiscvInstruction::Jal { rd, offset } => j_type(offset.value(), rd.index()),
        RiscvInstruction::Jalr { rd, rs1, offset } => {
            i_type(offset.value(), rs1.index(), 0x0, rd.index(), 0x67)
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

fn jal() -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: reg(0),
        offset: Immediate::new(4),
    }
}

fn jal_link(rd: u8) -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: reg(rd),
        offset: Immediate::new(4),
    }
}

fn jalr() -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(0),
        rs1: reg(9),
        offset: Immediate::new(0),
    }
}

fn jalr_return(rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(0),
        rs1: reg(rs1),
        offset: Immediate::new(0),
    }
}

fn mul(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Mul {
        rd: reg(rd),
        rs1: reg(rs1),
        rs2: reg(rs2),
    }
}

fn div(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Div {
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

fn j_type(imm: i64, rd: u8) -> u32 {
    let imm = imm as u32;
    ((imm >> 20) & 0x1) << 31
        | ((imm >> 1) & 0x3ff) << 21
        | ((imm >> 11) & 0x1) << 20
        | ((imm >> 12) & 0xff) << 12
        | (u32::from(rd) << 7)
        | 0x6f
}
