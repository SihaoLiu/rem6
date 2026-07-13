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
    runtime.record_live_speculative_execution(
        multiply_candidate,
        &[request(11)],
        12,
        RiscvExecutionRecord::new(
            multiply,
            0x8004,
            0x8008,
            vec![RegisterWrite::new(reg(7), 42)],
            None,
        ),
    );

    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), dependent)
        .expect("MUL result should wake the dependent scalar descendant");
    assert_eq!(dependent_candidate.issue_tick(12), 13);
    assert_eq!(
        dependent_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(7), 42)]
    );
}

#[test]
fn discarding_control_descendants_removes_younger_rename_state() {
    let mut runtime = O3RuntimeState::default();
    let head = addi(3, 0, 1);
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        head,
        0,
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );
    let branch_sequence = runtime.snapshot().reorder_buffer()[1].sequence();

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

fn mul(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Mul {
        rd: reg(rd),
        rs1: reg(rs1),
        rs2: reg(rs2),
    }
}
