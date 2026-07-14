use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord,
    RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::*;
use crate::{CpuFetchEvent, CpuFetchRecord, RiscvCpuExecutionEvent};

#[test]
fn predicted_control_branch_candidate_has_no_destination_and_keeps_issue_tick() {
    let mut runtime = scalar_load_runtime_with_branch(beq(5, 6));
    let branch = beq(5, 6);
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .expect("independent branch should issue while the load is outstanding");
    assert_eq!(candidate.destination(), None);
    assert_eq!(candidate.issue_tick(11), 11);

    let execution = RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None);
    runtime.record_live_speculative_execution(candidate, &[request(11)], 11, execution.clone());
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(0x8004, 11), branch, execution),
        &[request(11)],
        40,
    );

    let retired = runtime
        .take_live_retired_instruction(request(11))
        .expect("retired branch should retain its early issue record");
    assert_eq!(retired.issue_tick, 11);
}

#[test]
fn unresolved_load_source_rejects_predicted_control_branch_candidate() {
    let runtime = scalar_load_runtime_with_branch(beq(4, 0));

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), beq(4, 0))
        .is_none());
}

#[test]
fn scalar_load_stages_predicted_branch_and_two_descendants() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x8008, 0x800c].map(Address::new)
    );
}

#[test]
fn nested_control_dependencies_follow_immediate_branch() {
    let (runtime, _, _, _) = nested_control_runtime();
    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    let outer = rob[1].sequence();
    let inner = rob[2].sequence();
    let descendant = rob[3].sequence();

    assert_eq!(runtime.live_control_dependencies.get(&inner), Some(&outer));
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant),
        Some(&inner)
    );
}

#[test]
fn three_deep_control_dependencies_follow_immediate_branch() {
    let (runtime, _, _, _) = three_deep_control_runtime();
    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    let outer = rob[1].sequence();
    let middle = rob[2].sequence();
    let inner = rob[3].sequence();

    assert_eq!(runtime.live_control_dependencies.get(&middle), Some(&outer));
    assert_eq!(runtime.live_control_dependencies.get(&inner), Some(&middle));
    assert_eq!(runtime.live_control_window_sequences.len(), 3);
}

#[test]
fn inner_control_uses_staged_outer_ownership_before_execution_record() {
    let (mut runtime, outer, inner, _) = nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .expect("staged predicted-path ownership should not be a data wait");
    assert_eq!(inner_candidate.issue_tick(11), 11);
    assert_eq!(inner_candidate.producer_sequences(), &[outer_sequence]);

    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime.record_live_speculative_execution(
        outer_candidate,
        &[request(11)],
        11,
        RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
    );

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .is_some());
}

#[test]
fn outer_control_validation_preserves_inner_control_chain() {
    let (mut runtime, _, inner, descendant) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    let descendant_sequence = rob[3].sequence();

    runtime.validate_live_speculative_producer(outer_sequence);

    assert!(!runtime
        .live_control_dependencies
        .contains_key(&inner_sequence));
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant_sequence),
        Some(&inner_sequence)
    );
    let inner_record = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.execution.instruction() == inner)
        .unwrap();
    assert!(inner_record.producer_sequences.is_empty());
    assert!(runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.execution.instruction() == descendant));
}

#[test]
fn validated_outer_control_keeps_terminal_inner_timing_window_live() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = beq(5, 6);
    let inner = beq(4, 0);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), outer), (Address::new(0x8008), inner)],
    );
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime.record_live_speculative_execution(
        outer_candidate,
        &[request(11)],
        11,
        RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
    );

    runtime.validate_live_speculative_producer(outer_sequence);

    assert!(runtime.live_control_dependencies.is_empty());
    assert!(runtime.has_live_control_window());
    assert!(runtime
        .live_control_window_sequences
        .contains(&inner_sequence));

    runtime.discard_live_staged_window_from(outer_sequence);

    assert!(!runtime.has_live_control_window());
}

#[test]
fn outer_control_discard_removes_inner_branch_and_descendant() {
    let (mut runtime, outer, inner, descendant) = issued_nested_control_runtime();
    let outer_sequence = runtime.snapshot().reorder_buffer()[1].sequence();

    runtime.discard_live_control_descendants_from(outer_sequence);

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [Address::new(0x8000), Address::new(0x8004)]
    );
    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(
        runtime.live_speculative_executions[0]
            .execution
            .instruction(),
        outer
    );
    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| ![inner, descendant].contains(&issued.execution.instruction())));
}

#[test]
fn inner_control_discard_preserves_outer_branch() {
    let (mut runtime, outer, inner, _) = issued_nested_control_runtime();
    let inner_sequence = runtime.snapshot().reorder_buffer()[2].sequence();

    runtime.discard_live_control_descendants_from(inner_sequence);

    let instructions = runtime
        .live_speculative_executions
        .iter()
        .map(|issued| issued.execution.instruction())
        .collect::<Vec<_>>();
    assert_eq!(instructions, [outer, inner]);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
}

#[test]
fn middle_control_discard_removes_only_inner_control() {
    let (mut runtime, outer, middle, inner) = issued_three_deep_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let middle_sequence = rob[2].sequence();
    let inner_sequence = rob[3].sequence();

    runtime.discard_live_control_descendants_from(middle_sequence);

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x8008].map(Address::new)
    );
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .map(|issued| issued.execution.instruction())
            .collect::<Vec<_>>(),
        [outer, middle]
    );
    assert!(!runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.execution.instruction() == inner));
    assert_eq!(runtime.live_control_window_sequences.len(), 2);
    assert!(runtime
        .live_control_window_sequences
        .contains(&outer_sequence));
    assert!(runtime
        .live_control_window_sequences
        .contains(&middle_sequence));
    assert!(!runtime
        .live_control_window_sequences
        .contains(&inner_sequence));
}

#[test]
fn split_inner_branch_suffix_replacement_prunes_nested_chain() {
    let (mut runtime, outer, inner, _) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    runtime.validate_live_speculative_producer(outer_sequence);

    let inner_execution = runtime
        .live_speculative_executions
        .iter_mut()
        .find(|issued| issued.sequence == inner_sequence)
        .map(|issued| {
            issued.consumed_requests = vec![request(12), request(14)];
            issued.execution.clone()
        })
        .unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(0x8008, 12), inner, inner_execution),
        &[request(12), request(15)],
        40,
    );

    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(
        runtime.live_speculative_executions[0]
            .execution
            .instruction(),
        outer
    );
    assert!(runtime.live_control_dependencies.is_empty());
}

#[test]
fn predicted_descendants_use_staged_branch_ownership_and_invalidate_with_it() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );

    let branch_sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    let multiply_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), multiply)
        .expect("predicted descendant should use staged branch ownership");
    assert_eq!(multiply_candidate.issue_tick(11), 11);
    assert_eq!(multiply_candidate.producer_sequences(), &[branch_sequence]);

    let branch_execution = RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None);
    let branch_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .unwrap();
    runtime.record_live_speculative_execution(
        branch_candidate,
        &[request(11)],
        11,
        branch_execution.clone(),
    );

    let multiply_execution = RiscvExecutionRecord::new(
        multiply,
        0x8008,
        0x800c,
        vec![RegisterWrite::new(reg(7), 42)],
        None,
    );
    runtime.record_live_speculative_execution(
        multiply_candidate,
        &[request(12)],
        12,
        multiply_execution,
    );
    let dependent_execution = RiscvExecutionRecord::new(
        dependent,
        0x800c,
        0x8010,
        vec![RegisterWrite::new(reg(8), 43)],
        None,
    );
    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), dependent)
        .unwrap();
    runtime.record_live_speculative_execution(
        dependent_candidate,
        &[request(13)],
        14,
        dependent_execution,
    );
    assert_eq!(runtime.live_speculative_executions.len(), 3);

    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(0x8004, 99), branch, branch_execution),
        &[request(99)],
        40,
    );

    assert!(runtime.live_speculative_executions.is_empty());
}

#[test]
fn predicted_mul_wakes_dependent_add_candidate() {
    let mut runtime = O3RuntimeState::default();
    let head = addi(3, 0, 1);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        head,
        0,
        [
            (Address::new(0x8004), multiply),
            (Address::new(0x8008), dependent),
        ],
    );

    let multiply_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), multiply)
        .expect("predicted MUL should be an issue candidate");
    let multiply_execution = RiscvExecutionRecord::new(
        multiply,
        0x8004,
        0x8008,
        vec![RegisterWrite::new(reg(7), 42)],
        None,
    );
    runtime.record_live_speculative_execution(
        multiply_candidate,
        &[request(11)],
        12,
        multiply_execution.clone(),
    );
    assert_eq!(
        runtime.live_speculative_execution_ready_tick(&[request(11)], &multiply_execution),
        Some(14)
    );

    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), dependent)
        .expect("MUL result should wake the dependent scalar descendant");
    assert_eq!(
        dependent_candidate.issue_tick(12),
        12 + crate::riscv_fu_latency::riscv_execute_wait_cycles(multiply)
    );
    assert_eq!(
        dependent_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(7), 42)]
    );
}

#[test]
fn discarding_control_descendants_removes_younger_rename_state() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );
    let branch_sequence = runtime.snapshot().reorder_buffer()[1].sequence();

    let branch_execution = RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None);
    let branch_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .unwrap();
    runtime.record_live_speculative_execution(
        branch_candidate,
        &[request(11)],
        11,
        branch_execution.clone(),
    );
    let multiply_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), multiply)
        .unwrap();
    runtime.record_live_speculative_execution(
        multiply_candidate,
        &[request(12)],
        12,
        RiscvExecutionRecord::new(
            multiply,
            0x8008,
            0x800c,
            vec![RegisterWrite::new(reg(7), 42)],
            None,
        ),
    );
    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), dependent)
        .unwrap();
    runtime.record_live_speculative_execution(
        dependent_candidate,
        &[request(13)],
        14,
        RiscvExecutionRecord::new(
            dependent,
            0x800c,
            0x8010,
            vec![RegisterWrite::new(reg(8), 43)],
            None,
        ),
    );
    assert_eq!(runtime.live_speculative_executions.len(), 3);

    runtime.discard_live_control_descendants_from(branch_sequence);

    let snapshot = runtime.snapshot();
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [Address::new(0x8000), Address::new(0x8004)]
    );
    assert!(snapshot
        .rename_map()
        .iter()
        .all(|entry| !matches!(entry.architectural(), 7 | 8)));
    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(
        runtime.live_speculative_executions[0].sequence,
        branch_sequence
    );
}

#[test]
fn staged_window_truncation_prunes_control_dependencies() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), beq(5, 6)),
            (Address::new(0x8008), mul(7, 1, 2)),
            (Address::new(0x800c), addi(8, 7, 1)),
        ],
    );
    assert_eq!(runtime.live_control_dependencies.len(), 2);
    let load_sequence = runtime.snapshot().reorder_buffer()[0].sequence();

    runtime.discard_live_staged_window_from(load_sequence);

    assert!(runtime.live_control_dependencies.is_empty());
    assert!(!runtime.has_live_control_window());
}

fn scalar_load_runtime_with_branch(branch: RiscvInstruction) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), branch)],
    );
    runtime
}

fn nested_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = beq(5, 6);
    let inner = beq(7, 8);
    let descendant = mul(9, 1, 2);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), outer),
            (Address::new(0x8008), inner),
            (Address::new(0x800c), descendant),
        ],
    );
    (runtime, outer, inner, descendant)
}

fn issued_nested_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let (mut runtime, outer, inner, descendant) = nested_control_runtime();
    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime.record_live_speculative_execution(
        outer_candidate,
        &[request(11)],
        11,
        RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
    );

    let inner_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .unwrap();
    runtime.record_live_speculative_execution(
        inner_candidate,
        &[request(12)],
        12,
        RiscvExecutionRecord::new(inner, 0x8008, 0x800c, Vec::new(), None),
    );

    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), descendant)
        .unwrap();
    runtime.record_live_speculative_execution(
        descendant_candidate,
        &[request(13)],
        13,
        RiscvExecutionRecord::new(
            descendant,
            0x800c,
            0x8010,
            vec![RegisterWrite::new(reg(9), 42)],
            None,
        ),
    );
    (runtime, outer, inner, descendant)
}

fn three_deep_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = bne(5, 6);
    let middle = blt(7, 8);
    let inner = bgeu(9, 10);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), outer),
            (Address::new(0x8008), middle),
            (Address::new(0x800c), inner),
        ],
    );
    (runtime, outer, middle, inner)
}

fn issued_three_deep_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let (mut runtime, outer, middle, inner) = three_deep_control_runtime();
    for (pc, next_pc, instruction, sequence) in [
        (0x8004, 0x8008, outer, 11),
        (0x8008, 0x800c, middle, 12),
        (0x800c, 0x8010, inner, 13),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(sequence)],
            sequence,
            RiscvExecutionRecord::new(instruction, pc, next_pc, Vec::new(), None),
        );
    }
    (runtime, outer, middle, inner)
}

fn scalar_load_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(4),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8000,
            0x8004,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(4),
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

fn beq(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Beq {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn bne(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Bne {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn blt(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Blt {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn bgeu(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Bgeu {
        rs1: reg(rs1),
        rs2: reg(rs2),
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
