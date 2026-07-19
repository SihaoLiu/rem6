use super::*;

fn executed_direct_return_runtime(
    retire_data_head: bool,
) -> (O3RuntimeState, O3ProducerForwardedControlTarget, u64) {
    let (mut runtime, forwarded, _) =
        super::producer_forwarded_return::recorded_linked_runtime(1, 1);
    if retire_data_head {
        runtime.live_data_accesses.clear();
        runtime.snapshot.reorder_buffer.remove(0);
    }
    let instruction = jalr_return(1);
    let sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            instruction,
            &[request(13)],
        )
        .expect("same-link direct return append");
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9000), instruction)
        .expect("same-link direct return candidate");
    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(13)],
            22,
            RiscvExecutionRecord::new(instruction, 0x9000, 0x800c, Vec::new(), None),
        )
        .unwrap());
    assert!(runtime.producer_forwarded_return_descendant().is_some());
    (runtime, forwarded, sequence)
}

fn executed_scalar_return_runtime() -> (O3RuntimeState, u64) {
    let (mut runtime, _, descendant) =
        super::producer_forwarded_scalar_return::recorded_x1_scalar_runtime();
    super::producer_forwarded_scalar_return::retire_data_head(&mut runtime);
    let instruction = jalr_return(1);
    let sequence = runtime
        .append_producer_forwarded_scalar_return_descendant(
            descendant,
            Address::new(0x9004),
            instruction,
            &[request(14)],
        )
        .expect("linked-call scalar return append");
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9004), instruction)
        .expect("linked-call scalar return candidate");
    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(14)],
            23,
            RiscvExecutionRecord::new(instruction, 0x9004, 0x800c, Vec::new(), None),
        )
        .unwrap());
    assert!(runtime.producer_forwarded_return_descendant().is_some());
    (runtime, sequence)
}

#[test]
fn direct_return_requires_dependency_and_window_membership() {
    let (mut runtime, _, sequence) = executed_direct_return_runtime(false);
    runtime.live_control_dependencies.remove(&sequence);
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);

    let (mut runtime, _, sequence) = executed_direct_return_runtime(false);
    runtime.live_control_window_sequences.remove(&sequence);
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);
}

#[test]
fn direct_return_requires_bound_fetch_identity() {
    let (mut runtime, _, sequence) = executed_direct_return_runtime(false);
    assert!(
        runtime.replace_producer_forwarded_chain_fetch_identity_for_test(sequence, &[request(99)],)
    );
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);
}

#[test]
fn head_retired_direct_return_reconstructs_exact_recorded_parent() {
    let (mut runtime, forwarded, _) = executed_direct_return_runtime(true);
    assert_eq!(
        runtime
            .producer_forwarded_return_descendant()
            .expect("post-head direct return lineage")
            .parent(),
        forwarded
    );

    assert!(
        runtime.replace_producer_forwarded_chain_fetch_identity_for_test(
            forwarded.consumer_sequence(),
            &[request(99)],
        )
    );
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);
}

#[test]
fn scalar_descendant_requires_dependency_and_window_membership() {
    let (mut runtime, _, descendant) =
        super::producer_forwarded_scalar_return::recorded_x1_scalar_runtime();
    runtime
        .live_control_dependencies
        .remove(&descendant.sequence());
    assert_eq!(runtime.producer_forwarded_scalar_descendant(), None);

    let (mut runtime, _, descendant) =
        super::producer_forwarded_scalar_return::recorded_x1_scalar_runtime();
    runtime
        .live_control_window_sequences
        .remove(&descendant.sequence());
    assert_eq!(runtime.producer_forwarded_scalar_descendant(), None);
}

#[test]
fn scalar_return_requires_dependency_window_and_fetch_identity() {
    let (mut runtime, sequence) = executed_scalar_return_runtime();
    runtime.live_control_dependencies.remove(&sequence);
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);

    let (mut runtime, sequence) = executed_scalar_return_runtime();
    runtime.live_control_window_sequences.remove(&sequence);
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);

    let (mut runtime, sequence) = executed_scalar_return_runtime();
    assert!(
        runtime.replace_producer_forwarded_chain_fetch_identity_for_test(sequence, &[request(99)],)
    );
    assert_eq!(runtime.producer_forwarded_return_descendant(), None);
}
