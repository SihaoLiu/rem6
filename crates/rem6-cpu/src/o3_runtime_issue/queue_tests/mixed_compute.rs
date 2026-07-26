use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, MemoryAccessKind, RiscvFloatRoundingMode, RiscvSystemEvent,
    RiscvTrap, RiscvTrapKind, RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::*;
use crate::O3IssueOpClass;

struct MixedComputeIssueFixture {
    runtime: O3RuntimeState,
}

impl MixedComputeIssueFixture {
    fn new() -> Self {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.set_issue_width(4));
        Self { runtime }
    }

    fn stage_and_bind(
        &mut self,
        pc: u64,
        instruction: RiscvInstruction,
        request_sequence: u64,
    ) -> u64 {
        let sequence = self
            .runtime
            .stage_live_instruction(Address::new(pc), instruction, 0)
            .unwrap();
        assert!(self.runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            mixed_decoded(instruction),
            &[request(request_sequence)],
            20,
        ));
        sequence
    }

    fn materialize(&self) -> O3LiveIssueQueue {
        materialized_queue(&self.runtime)
    }
}

#[test]
fn live_issue_queue_materializes_mixed_compute_classes() {
    let mut fixture = MixedComputeIssueFixture::new();
    let fp = fixture.stage_and_bind(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let vector = fixture.stage_and_bind(SECOND_PC, vector_move_to_scalar(11, 3), 12);
    let queue = fixture.materialize();

    assert_eq!(
        queue.entry(fp).unwrap().scheduling().op_class(),
        O3IssueOpClass::Float
    );
    assert_eq!(
        queue.entry(vector).unwrap().scheduling().op_class(),
        O3IssueOpClass::Vector
    );
    assert_eq!(
        live_issue_trace_name(float_add_s(4, 1, 2)),
        Some("scalar_float")
    );
    assert_eq!(
        live_issue_trace_name(vector_move_to_scalar(11, 3)),
        Some("vector_to_scalar")
    );
}

#[test]
fn live_issue_queue_admits_scalar_fp_live_source_producers() {
    let mut fixture = MixedComputeIssueFixture::new();
    let producer = fixture.stage_and_bind(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let dependent = fixture.stage_and_bind(SECOND_PC, float_mul_s(5, 4, 3), 12);

    assert!(fixture
        .runtime
        .enqueue_bound_live_issue_sequence_at(dependent, 20));
    assert!(fixture
        .runtime
        .live_issue
        .resident_sequences()
        .contains(&dependent));
    let queue = fixture.materialize();
    let producers = queue
        .entry(dependent)
        .unwrap()
        .scheduling()
        .data_producers();
    assert_eq!(producers.len(), 1);
    assert_eq!(producers[0].sequence(), producer);
}

#[test]
fn live_issue_queue_rejects_live_vector_source_producers() {
    let mut fixture = MixedComputeIssueFixture::new();
    let producer_instruction = addi(3, 0, 1);
    fixture
        .runtime
        .stage_live_instruction_with_rename_destination(
            Address::new(BRANCH_PC),
            producer_instruction,
            0,
            Some((O3RegisterClass::Vector, 3)),
        )
        .unwrap();
    bind_mixed(&mut fixture.runtime, BRANCH_PC, producer_instruction, 11);
    let dependent = fixture.stage_and_bind(SECOND_PC, vector_move_to_scalar(11, 3), 12);

    assert!(fixture
        .runtime
        .enqueue_bound_live_issue_sequence_at(dependent, 20));
    assert!(!fixture
        .runtime
        .live_issue
        .resident_sequences()
        .contains(&dependent));
    assert!(fixture.materialize().entry(dependent).is_none());
}

#[test]
fn live_issue_queue_uses_typed_source_identities() {
    let mut fixture = MixedComputeIssueFixture::new();
    fixture.stage_and_bind(BRANCH_PC, addi(4, 0, 1), 11);
    let fp = fixture.stage_and_bind(SECOND_PC, float_add_s(5, 4, 3), 12);

    let queue = fixture.materialize();
    assert_eq!(
        queue.entry(fp).unwrap().scheduling().op_class(),
        O3IssueOpClass::Float
    );
}

#[test]
fn live_issue_queue_rejects_mismatched_compute_rename_destination() {
    let mut runtime = O3RuntimeState::default();
    let instruction = float_add_s(4, 1, 2);
    let (sequence, _) = runtime
        .stage_live_instruction_with_rename_destination(
            Address::new(BRANCH_PC),
            instruction,
            0,
            Some((O3RegisterClass::Integer, 4)),
        )
        .unwrap();
    bind_mixed(&mut runtime, BRANCH_PC, instruction, 11);

    assert!(runtime.enqueue_bound_live_issue_sequence_at(sequence, 20));
    assert!(!runtime.live_issue.resident_sequences().contains(&sequence));
    assert!(materialized_queue(&runtime).entry(sequence).is_none());
}

#[test]
fn live_issue_queue_preserves_integer_producer_forwarding() {
    let mut runtime = O3RuntimeState::default();
    let producer_instruction = addi(4, 0, 7);
    let consumer_instruction = addi(5, 4, 2);
    let producer = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), producer_instruction, 0)
        .unwrap();
    let consumer = runtime
        .stage_live_instruction(Address::new(SECOND_PC), consumer_instruction, 0)
        .unwrap();
    bind_mixed(&mut runtime, BRANCH_PC, producer_instruction, 11);
    bind_mixed(&mut runtime, SECOND_PC, consumer_instruction, 12);

    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), producer_instruction)
        .unwrap();
    assert!(runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(11)],
            20,
            int_record(producer_instruction, BRANCH_PC, reg(4), 7),
        )
        .unwrap());
    let consumer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(SECOND_PC), consumer_instruction)
        .unwrap();

    assert_eq!(consumer_candidate.producer_sequences(), &[producer]);
    assert_eq!(
        consumer_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(4), 7)]
    );
    assert_eq!(consumer_candidate.sequence(), consumer);
}

#[test]
fn live_issue_candidate_result_validation_is_destination_class_exact() {
    let fp = float_add_s(4, 1, 2);
    let vmv = vector_move_to_scalar(11, 3);
    let int = addi(4, 0, 1);

    assert_candidate_recording(fp, fp_record(fp, f(4)), true);
    assert_candidate_recording(fp, int_record(fp, BRANCH_PC, reg(4), 1), false);
    assert_candidate_recording(vmv, int_record(vmv, BRANCH_PC, reg(11), 1), true);
    assert_candidate_recording(vmv, int_and_fp_record(vmv, reg(11), f(11)), false);
    assert_candidate_recording(int, int_record(int, BRANCH_PC, reg(4), 1), true);
    assert_candidate_recording(int, int_and_fp_record(int, reg(4), f(4)), false);
}

#[test]
fn live_issue_candidate_result_validation_rejects_wrong_shapes() {
    let instruction = float_add_s(4, 1, 2);
    for execution in [
        fp_record_at(instruction, SECOND_PC, f(4), 4),
        fp_record_with_bytes(instruction, 2, f(4)),
        fp_record_with_next_pc(instruction, BRANCH_PC + 8, f(4)),
        fp_record(float_mul_s(4, 1, 2), f(4)),
        fp_record(instruction, f(5)),
        trap_record(instruction),
        system_record(instruction),
        memory_record(instruction),
    ] {
        assert_candidate_recording(instruction, execution, false);
    }
}

#[test]
fn live_issue_head_result_validation_matches_candidate_validation() {
    let fp = float_add_s(4, 1, 2);
    let vmv = vector_move_to_scalar(11, 3);
    let int = addi(4, 0, 1);

    assert_head_recording(fp, fp_record(fp, f(4)), true);
    assert_head_recording(fp, int_record(fp, BRANCH_PC, reg(4), 1), false);
    assert_head_recording(vmv, int_record(vmv, BRANCH_PC, reg(11), 1), true);
    assert_head_recording(vmv, int_and_fp_record(vmv, reg(11), f(11)), false);
    assert_head_recording(int, int_and_fp_record(int, reg(4), f(4)), false);
    assert_head_recording(fp, memory_record(fp), false);
}

fn assert_candidate_recording(
    instruction: RiscvInstruction,
    execution: RiscvExecutionRecord,
    expected: bool,
) {
    let mut runtime = O3RuntimeState::default();
    runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    bind_mixed(&mut runtime, BRANCH_PC, instruction, 11);
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), instruction)
        .unwrap();

    assert_eq!(
        runtime
            .record_live_speculative_execution(candidate, &[request(11)], 20, execution)
            .unwrap(),
        expected
    );
}

fn assert_head_recording(
    instruction: RiscvInstruction,
    execution: RiscvExecutionRecord,
    expected: bool,
) {
    let mut runtime = O3RuntimeState::default();
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    bind_mixed(&mut runtime, BRANCH_PC, instruction, 11);
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);

    assert_eq!(
        runtime
            .record_live_issue_head_execution(head, &[request(11)], execution)
            .unwrap(),
        expected
    );
}

fn bind_mixed(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
) {
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        mixed_decoded(instruction),
        &[request(request_sequence)],
        20,
    ));
}

fn mixed_decoded(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    RiscvInstruction::decode_with_length(mixed_raw(instruction)).unwrap()
}

fn mixed_raw(instruction: RiscvInstruction) -> u32 {
    match instruction {
        RiscvInstruction::FloatAddS { rd, rs1, rs2, .. } => {
            fp_r_type(0, rs2.index(), rs1.index(), 0, rd.index())
        }
        RiscvInstruction::FloatMulS { rd, rs1, rs2, .. } => {
            fp_r_type(0b0001000, rs2.index(), rs1.index(), 0, rd.index())
        }
        RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
            rd,
            vs2,
        }) => vector_mvv_type(0b010000, vs2.index(), 0, rd.index()),
        _ => raw(instruction),
    }
}

fn live_issue_trace_name(instruction: RiscvInstruction) -> Option<&'static str> {
    super::super::super::o3_runtime_issue::queue::live_issue_trace_class(instruction)
        .map(O3LiveIssueTraceClass::name)
}

fn float_add_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatAddS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn float_mul_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn vector_move_to_scalar(rd: u8, vs2: u8) -> RiscvInstruction {
    RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
        rd: reg(rd),
        vs2: v(vs2),
    })
}

fn int_record(i: RiscvInstruction, pc: u64, r: Register, value: u64) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes(
        i,
        4,
        pc,
        pc + 4,
        vec![RegisterWrite::new(r, value)],
        None,
    )
}

fn fp_record(instruction: RiscvInstruction, register: FloatRegister) -> RiscvExecutionRecord {
    fp_record_at(instruction, BRANCH_PC, register, 4)
}

fn fp_record_at(i: RiscvInstruction, pc: u64, r: FloatRegister, bytes: u8) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
        i,
        bytes,
        pc,
        pc + u64::from(bytes),
        Vec::new(),
        vec![FloatRegisterWrite::new(r, 0x3fc0_0000)],
        None,
    )
}

fn fp_record_with_bytes(i: RiscvInstruction, bytes: u8, r: FloatRegister) -> RiscvExecutionRecord {
    fp_record_at(i, BRANCH_PC, r, bytes)
}

fn fp_record_with_next_pc(
    i: RiscvInstruction,
    next_pc: u64,
    r: FloatRegister,
) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
        i,
        4,
        BRANCH_PC,
        next_pc,
        Vec::new(),
        vec![FloatRegisterWrite::new(r, 0x3fc0_0000)],
        None,
    )
}

fn int_and_fp_record(i: RiscvInstruction, r: Register, fr: FloatRegister) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
        i,
        4,
        BRANCH_PC,
        BRANCH_PC + 4,
        vec![RegisterWrite::new(r, 1)],
        vec![FloatRegisterWrite::new(fr, 1)],
        None,
    )
}

fn trap_record(instruction: RiscvInstruction) -> RiscvExecutionRecord {
    RiscvExecutionRecord::with_trap_with_instruction_bytes(
        instruction,
        4,
        BRANCH_PC,
        BRANCH_PC + 4,
        RiscvTrap::new(RiscvTrapKind::IllegalInstruction, BRANCH_PC),
    )
}

fn system_record(instruction: RiscvInstruction) -> RiscvExecutionRecord {
    RiscvExecutionRecord::with_system_event_and_register_writes_with_instruction_bytes(
        instruction,
        4,
        BRANCH_PC,
        BRANCH_PC + 4,
        RiscvSystemEvent::WaitForInterrupt { pc: BRANCH_PC },
        Vec::new(),
    )
}

fn memory_record(instruction: RiscvInstruction) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
        instruction,
        4,
        BRANCH_PC,
        BRANCH_PC + 4,
        Vec::new(),
        Vec::new(),
        Some(MemoryAccessKind::Load {
            rd: reg(4),
            address: 0x9000,
            width: MemoryWidth::Word,
            signed: false,
        }),
    )
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn v(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn fp_r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    r_type(funct7, rs2, rs1, funct3, rd, 0x53)
}

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}
