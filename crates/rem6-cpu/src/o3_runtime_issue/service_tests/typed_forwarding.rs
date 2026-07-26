use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, RegisterWrite, RiscvFloatRoundingMode, RiscvFloatStatus,
    RiscvHartState, RiscvInstruction, RiscvVectorConfig, VectorRegister,
};

use super::*;
use crate::o3_runtime::o3_runtime_issue::O3LiveIssueForwardedValue;

fn boxed_single(value: f32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(value.to_bits())
}

fn fp_chain_fixture() -> (O3RuntimeState, RiscvHartState, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let producer_raw = fp_raw(0, 2, 1, 4);
    let consumer_raw = fp_raw(0b0001000, 3, 4, 5);
    let mut sequences = Vec::new();
    for (pc, request_sequence, raw) in
        [(BRANCH_PC, 11, producer_raw), (SECOND_PC, 12, consumer_raw)]
    {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        sequences.push(
            runtime
                .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
                .unwrap(),
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }
    let mut hart = RiscvHartState::new(BRANCH_PC);
    for (index, value) in [(1, 1.0_f32), (2, 2.0), (3, 3.0)] {
        hart.write_float(f(index), boxed_single(value));
    }
    (runtime, hart, sequences[0], sequences[1])
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x53
}

#[test]
fn typed_live_forwarding_waits_for_fp_writeback_and_computes_nine() {
    let (mut runtime, hart, producer, consumer) = fp_chain_fixture();
    let canonical_before = hart.clone();

    let first = runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    let producer_ready = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == producer)
        .unwrap()
        .admitted_writeback_tick;
    assert_eq!(first.issued_rows(), 1);
    assert_eq!(first.next_service_tick(), Some(producer_ready));
    assert!(runtime.live_issue.resident_sequences().contains(&consumer));
    assert!(runtime.live_issue_trace_records().iter().any(|record| {
        record.sequence() == consumer
            && record.action() == O3LiveIssueTraceAction::RetainedDependency
            && record.next_wake_tick() == Some(producer_ready)
    }));

    let second = runtime
        .service_live_issue_queue_at(&hart, producer_ready)
        .unwrap();
    assert_eq!(second.issued_rows(), 1);
    let execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == consumer)
        .unwrap();
    assert_eq!(
        execution.execution.float_register_writes(),
        &[FloatRegisterWrite::new(f(5), 0xffff_ffff_4110_0000)],
    );
    assert_eq!(hart.pc(), canonical_before.pc());
    assert_eq!(hart.read_float(f(4)), canonical_before.read_float(f(4)));
    assert_eq!(hart.read_float(f(5)), canonical_before.read_float(f(5)));
    assert_eq!(hart.float_status(), canonical_before.float_status());
}

#[test]
fn typed_live_forwarding_uses_dynamic_frm_without_mutating_canonical_status() {
    const BOXED_ROUNDING_INPUT: u64 = 0xffff_ffff_3f80_0001;
    const BOXED_ROUND_UP_PRODUCT: u64 = 0xffff_ffff_3f80_0003;
    const FLOAT_FLAG_INEXACT: u64 = 1;

    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let producer_raw = fp_raw(0, 2, 1, 4);
    let consumer_raw = fp_raw(0b0001000, 3, 4, 5) | (0b111 << 12);
    let consumer_instruction = RiscvInstruction::decode(consumer_raw).unwrap();
    assert_eq!(
        consumer_instruction,
        RiscvInstruction::FloatMulS {
            rd: f(5),
            rs1: f(4),
            rs2: f(3),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        }
    );

    let mut sequences = Vec::new();
    for (pc, request_sequence, raw) in
        [(BRANCH_PC, 21, producer_raw), (SECOND_PC, 22, consumer_raw)]
    {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        sequences.push(
            runtime
                .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
                .unwrap(),
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }
    let [producer, consumer] = sequences.as_slice() else {
        unreachable!()
    };
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.write_float(f(1), BOXED_ROUNDING_INPUT);
    hart.write_float(f(2), boxed_single(0.0));
    hart.write_float(f(3), BOXED_ROUNDING_INPUT);
    hart.set_float_status(RiscvFloatStatus::new(0).with_frm(3));
    assert_eq!(
        RiscvFloatRoundingMode::from_frm_bits(hart.float_status().frm() as u8),
        Some(RiscvFloatRoundingMode::RoundUp)
    );
    let canonical_before = hart.clone();

    let mut oracle_hart = hart.clone();
    oracle_hart.write_float(f(4), BOXED_ROUNDING_INPUT);
    oracle_hart.execute(consumer_instruction).unwrap();
    assert_eq!(oracle_hart.read_float(f(5)), BOXED_ROUND_UP_PRODUCT);
    assert_eq!(oracle_hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    let first = runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    assert_eq!(first.issued_rows(), 1);
    let producer_ready = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *producer)
        .unwrap()
        .admitted_writeback_tick;
    assert_eq!(first.next_service_tick(), Some(producer_ready));
    let consumer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(SECOND_PC), consumer_instruction)
        .unwrap();
    assert_eq!(
        consumer_candidate.forwarded_values(),
        &[O3LiveIssueForwardedValue::FloatingPoint(
            FloatRegisterWrite::new(f(4), BOXED_ROUNDING_INPUT),
        )],
    );

    let second = runtime
        .service_live_issue_queue_at(&hart, producer_ready)
        .unwrap();
    assert_eq!(second.issued_rows(), 1);
    let execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *consumer)
        .unwrap();
    assert_eq!(
        execution.execution.float_register_writes(),
        &[FloatRegisterWrite::new(f(5), BOXED_ROUND_UP_PRODUCT)],
    );
    assert_eq!(hart.float_status(), canonical_before.float_status());
}

#[test]
fn typed_live_forwarding_vector_result_feeds_integer() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_writeback_width(1));
    let unrelated_writeback = runtime.allocate_sequence();
    let vector_raw = (0b010000 << 26) | (1 << 25) | (3 << 20) | (0b010 << 12) | (11 << 7) | 0x57;
    let integer_raw = i_type(1, 11, 0, 13, 0x13);
    let mut sequences = Vec::new();
    for (pc, request_sequence, raw) in [(BRANCH_PC, 11, vector_raw), (SECOND_PC, 12, integer_raw)] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        sequences.push(
            runtime
                .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
                .unwrap(),
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }
    let [producer, consumer] = sequences.as_slice() else {
        unreachable!()
    };
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.set_vector_config(RiscvVectorConfig::new(1, 0xd8));
    let vector = VectorRegister::new(3).unwrap();
    let mut lanes = hart.read_vector(vector);
    lanes[..8].copy_from_slice(&9_u64.to_le_bytes());
    hart.write_vector(vector, lanes);

    let producer_raw_ready = 20
        + crate::riscv_fu_latency::riscv_execute_wait_cycles(
            RiscvInstruction::decode(vector_raw).unwrap(),
        );
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
            unrelated_writeback,
            producer_raw_ready,
        )])
        .unwrap();

    let first = runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    let producer_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *producer)
        .unwrap();
    let producer_ready = producer_execution.admitted_writeback_tick;
    assert_eq!(producer_execution.raw_ready_tick, producer_raw_ready);
    assert!(producer_ready > producer_raw_ready);
    assert_eq!(first.next_service_tick(), Some(producer_ready));
    assert!(runtime.live_issue_trace_records().iter().any(|record| {
        record.sequence() == *consumer
            && record.action() == O3LiveIssueTraceAction::RetainedDependency
            && record.next_wake_tick() == Some(producer_ready)
    }));
    let consumer_candidate = runtime
        .live_speculative_issue_candidate(
            Address::new(SECOND_PC),
            RiscvInstruction::decode(integer_raw).unwrap(),
        )
        .unwrap();
    assert_eq!(
        consumer_candidate.forwarded_values(),
        &[O3LiveIssueForwardedValue::Integer(RegisterWrite::new(
            reg(11),
            9,
        ))],
    );
    runtime
        .service_live_issue_queue_at(&hart, producer_ready)
        .unwrap();
    let consumer_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *consumer)
        .unwrap();
    assert_eq!(
        consumer_execution.execution.register_writes(),
        &[RegisterWrite::new(reg(13), 10)],
    );
}
