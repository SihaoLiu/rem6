use rem6_isa_riscv::{FloatRegister, RiscvVectorConfig, VectorRegister};

use super::*;

#[test]
fn mixed_compute_service_coissues_fp_vector_and_retries_second_fp() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let raws = [
        r_type(0, 2, 1, 0, 4, 0x53),
        r_type(0b0001000, 7, 6, 0, 5, 0x53),
        (0b010000 << 26) | (1 << 25) | (3 << 20) | (0b010 << 12) | (11 << 7) | 0x57,
    ];
    let mut sequences = Vec::new();
    for ((pc, request_sequence), raw) in [(BRANCH_PC, 11), (SECOND_PC, 12), (THIRD_PC, 13)]
        .into_iter()
        .zip(raws)
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
    let [first_fp, second_fp, vector] = sequences.as_slice() else {
        unreachable!()
    };
    let mut hart = RiscvHartState::new(BRANCH_PC);
    for (register, value) in [(1, 1.0_f32), (2, 2.0), (6, 3.0), (7, 4.0)] {
        hart.write_float(
            FloatRegister::new(register).unwrap(),
            0xffff_ffff_0000_0000 | u64::from(value.to_bits()),
        );
    }
    hart.set_vector_config(RiscvVectorConfig::new(1, 0xd8));
    let vector_register = VectorRegister::new(3).unwrap();
    let mut vector_value = hart.read_vector(vector_register);
    vector_value[..8].copy_from_slice(&9_u64.to_le_bytes());
    hart.write_vector(vector_register, vector_value);

    let first = runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    assert_eq!(first.issued_rows(), 2);
    assert_eq!(first.next_service_tick(), Some(21));
    assert_eq!(runtime.live_issue_service_tick(), Some(21));
    assert_eq!(runtime.live_issue.resident_sequences(), &[*second_fp]);
    let turn = runtime
        .live_issue_trace_records()
        .iter()
        .filter(|record| record.service_tick() == 20)
        .map(|record| (record.sequence(), record.action()))
        .collect::<Vec<_>>();
    assert!(turn.contains(&(*first_fp, O3LiveIssueTraceAction::Selected)));
    assert!(turn.contains(&(*vector, O3LiveIssueTraceAction::Selected)));
    assert!(turn.contains(&(*second_fp, O3LiveIssueTraceAction::RetainedResource)));

    let second = runtime.service_live_issue_queue_at(&hart, 21).unwrap();
    assert_eq!(second.issued_rows(), 1);
    assert_eq!(second.next_service_tick(), None);
    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .find(|row| row.sequence == *second_fp)
            .unwrap()
            .issue_tick,
        21
    );
}
