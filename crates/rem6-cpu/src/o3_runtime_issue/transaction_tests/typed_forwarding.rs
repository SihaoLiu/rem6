use rem6_isa_riscv::{FloatRegister, RiscvHartState, RiscvInstruction};

use super::*;
use crate::o3_runtime::o3_runtime_issue::O3LiveIssueForwardedValue;

fn issued_fp_producer_fixture() -> (O3RuntimeState, RiscvHartState, u64, u64) {
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
    for (index, bits) in [
        (1, 0xffff_ffff_3f80_0000),
        (2, 0xffff_ffff_4000_0000),
        (3, 0xffff_ffff_4040_0000),
    ] {
        hart.write_float(FloatRegister::new(index).unwrap(), bits);
    }
    runtime.service_live_issue_queue_at(&hart, 20).unwrap();
    (runtime, hart, sequences[0], sequences[1])
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x53
}

#[test]
fn typed_live_forwarding_transaction_failure_rolls_back_exact_state() {
    let (mut runtime, hart, _, consumer) = issued_fp_producer_fixture();
    let tick = runtime.live_issue_service_tick().unwrap();
    let queue = super::super::queue::materialized_queue(&runtime);
    let dependencies = O3LiveIssueDependencyTable::new(&runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_scoped_at(
            tick,
            dependencies.resolved_scopes_at(tick),
            queue
                .entries()
                .iter()
                .map(|entry| dependencies.scoped_instruction(entry)),
        )
        .unwrap();
    let prepared = match runtime
        .prepare_live_issue_batch(&hart, &queue, plan.issued(), tick)
        .unwrap()
    {
        O3PreparedLiveIssueBatch::Prepared(rows) => rows,
        O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
            panic!("unexpected replay at {sequence}")
        }
    };
    assert_eq!(prepared.len(), 1);
    assert!(matches!(
        prepared[0].candidate.forwarded_values(),
        [O3LiveIssueForwardedValue::FloatingPoint(_)],
    ));
    assert!(runtime.remove_live_staged_issue_identity_for_test(consumer));
    let before = super::touched(&runtime);

    assert!(matches!(
        runtime.record_live_issue_batch(prepared),
        Err(O3LiveIssueTransactionError::Runtime(
            O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence }
        )) if sequence == consumer
    ));
    assert_eq!(super::touched(&runtime), before);
}
