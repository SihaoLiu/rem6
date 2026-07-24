use super::queue::materialized_queue;
use super::*;
use crate::o3_runtime::o3_runtime_pending_address_tests::multiple::ready_two_pending_issue;
use rem6_isa_riscv::RiscvExecutionRecord;

const EARLIER_BLOCKED_TICK: u64 = 10;
const DURABLE_ISSUE_TICK: u64 = 20;
const LATER_BLOCKED_TICK: u64 = 30;

#[test]
fn durable_head_issue_clears_only_current_and_future_blocking() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let decoded = decoded(instruction);
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        DURABLE_ISSUE_TICK,
    ));
    retain_blocked_decisions(&mut runtime, sequence);

    let execution = RiscvHartState::new(BRANCH_PC)
        .execute_decoded(decoded)
        .unwrap();
    let head =
        O3LiveIssueHeadReservation::for_instruction(sequence, DURABLE_ISSUE_TICK, instruction);
    assert!(runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());

    assert_durable_cleanup(&runtime, sequence);
}

#[test]
fn durable_control_issue_clears_only_current_and_future_blocking() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    runtime
        .stage_live_retire_window(
            Address::new(LOAD_PC),
            div(9, 1, 2),
            0,
            [(Address::new(BRANCH_PC), instruction)],
        )
        .unwrap();
    let decoded = decoded(instruction);
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        DURABLE_ISSUE_TICK,
    ));
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), instruction)
        .unwrap();
    let sequence = candidate.sequence();
    retain_blocked_decisions(&mut runtime, sequence);

    let execution = RiscvHartState::new(BRANCH_PC)
        .execute_decoded(decoded)
        .unwrap();
    assert!(
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(11)],
                DURABLE_ISSUE_TICK,
                execution,
            )
            .unwrap()
    );

    assert_durable_cleanup(&runtime, sequence);
}

#[test]
fn durable_pending_address_issue_clears_only_current_and_future_blocking() {
    let (mut runtime, _, _) = ready_two_pending_issue(2, false);
    let sequence = runtime.pending_data_address_sequences_for_test()[0];
    let queue = materialized_queue(&runtime);
    let entry = queue.entry(sequence).expect("pending issue row");
    let candidate = runtime
        .materialize_live_speculative_issue_candidate(entry.scheduling())
        .expect("pending materialization candidate");
    let instruction = candidate.instruction();
    let rd = match instruction {
        RiscvInstruction::Load { rd, .. } => rd,
        _ => panic!("pending address row must be a load"),
    };
    let consumed_requests = entry.packet().consumed_requests().to_vec();
    let pc = entry.scheduling().pc().get();
    let execution = RiscvExecutionRecord::new(
        instruction,
        pc,
        pc + 4,
        Vec::new(),
        Some(MemoryAccessKind::Load {
            rd,
            address: 0xa000,
            width: MemoryWidth::Doubleword,
            signed: false,
        }),
    );
    retain_blocked_decisions(&mut runtime, sequence);

    assert!(runtime
        .record_pending_data_address_materialization(
            candidate,
            &consumed_requests,
            DURABLE_ISSUE_TICK,
            execution,
        )
        .unwrap());

    assert_durable_cleanup(&runtime, sequence);
}

fn retain_blocked_decisions(runtime: &mut O3RuntimeState, sequence: u64) {
    runtime
        .live_issue
        .observe_sequences(DURABLE_ISSUE_TICK, &[], &[sequence], &[], 1);
    runtime.seal_live_issue_decision();
    runtime
        .live_issue
        .observe_sequences(LATER_BLOCKED_TICK, &[], &[sequence], &[], 1);
    runtime.seal_live_issue_decision();
    runtime
        .live_issue
        .observe_sequences(EARLIER_BLOCKED_TICK, &[], &[], &[sequence], 1);

    let before = runtime.stats();
    assert_eq!(before.issue_cycles(), 3);
    assert_eq!(before.resource_blocked_row_cycles(), 2);
    assert_eq!(before.dependency_blocked_row_cycles(), 1);
}

fn assert_durable_cleanup(runtime: &O3RuntimeState, sequence: u64) {
    assert!(!runtime.live_issue.resident_sequences().contains(&sequence));
    let after = runtime.stats();
    assert_eq!(after.issue_cycles(), 3);
    assert_eq!(after.resource_blocked_row_cycles(), 0);
    assert_eq!(after.dependency_blocked_row_cycles(), 1);
    assert_eq!(runtime.stats(), after);
}
