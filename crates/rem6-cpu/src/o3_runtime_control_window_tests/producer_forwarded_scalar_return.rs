use super::producer_forwarded_return::decoded;
use super::*;

pub(super) fn recorded_linked_scalar_runtime(
    target_source: u8,
    link: u8,
) -> (
    O3RuntimeState,
    O3ProducerForwardedControlTarget,
    O3ProducerForwardedScalarChain,
) {
    let (mut runtime, forwarded, consumer_sequence) =
        super::producer_forwarded_return::recorded_linked_runtime(target_source, link);
    let scalar = addi(13, link, 0);
    let scalar_sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            decoded(scalar),
            &[request(13)],
            0,
        )
        .expect("linked-call scalar descendant append");
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9000), scalar)
        .expect("linked-call scalar descendant candidate");
    assert_eq!(candidate.sequence(), scalar_sequence);
    assert_eq!(candidate.producer_sequences(), &[consumer_sequence]);
    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(13)],
            22,
            RiscvExecutionRecord::new(
                scalar,
                0x9000,
                0x9004,
                vec![RegisterWrite::new(reg(13), 0x800c)],
                None,
            ),
        )
        .unwrap());
    let scalar_chain = runtime
        .producer_forwarded_scalar_chain()
        .expect("typed producer-forwarded scalar lineage");
    let descendant = scalar_chain.last().expect("one scalar descendant");
    assert_eq!(descendant.parent(), forwarded);
    assert_eq!(descendant.fetch_request(), request(13));
    assert_eq!(descendant.pc(), Address::new(0x9000));
    assert_eq!(descendant.sequential_pc(), Address::new(0x9004));
    assert_eq!(descendant.sequence(), scalar_sequence);
    (runtime, forwarded, scalar_chain)
}

pub(super) fn recorded_x1_scalar_runtime() -> (
    O3RuntimeState,
    O3ProducerForwardedControlTarget,
    O3ProducerForwardedScalarChain,
) {
    recorded_linked_scalar_runtime(1, 1)
}

pub(super) fn retire_data_head(runtime: &mut O3RuntimeState) {
    runtime.live_data_accesses.clear();
    runtime.snapshot.reorder_buffer.remove(0);
    runtime.last_live_commit_tick = Some(30);
}

#[test]
fn producer_forwarded_scalar_lineage_survives_successful_data_head_retirement() {
    let (mut runtime, _, scalar_chain) = recorded_x1_scalar_runtime();

    retire_data_head(&mut runtime);

    assert_eq!(
        runtime.producer_forwarded_scalar_chain(),
        Some(scalar_chain)
    );
}

#[test]
fn producer_forwarded_scalar_return_waits_for_data_head_retirement() {
    let (mut runtime, _, scalar_chain) = recorded_x1_scalar_runtime();
    let descendant = scalar_chain.last().expect("one scalar descendant");
    let return_jump = jalr_return(1);

    assert_eq!(
        runtime.append_producer_forwarded_scalar_return_descendant(
            &scalar_chain,
            Address::new(0x9004),
            decoded(return_jump),
            &[request(14)],
            0,
        ),
        None
    );
    assert_eq!(runtime.snapshot.reorder_buffer.len(), 4);

    retire_data_head(&mut runtime);
    let return_sequence = runtime
        .append_producer_forwarded_scalar_return_descendant(
            &scalar_chain,
            Address::new(0x9004),
            decoded(return_jump),
            &[request(14)],
            0,
        )
        .expect("post-head scalar-sequential return append");
    assert_eq!(runtime.snapshot.reorder_buffer.len(), 4);
    assert_eq!(runtime.live_data_access_younger_sequences.len(), 4);
    assert_eq!(
        runtime.pending_live_control_lineage_parent_for_test(return_sequence),
        Some(descendant.parent().consumer_sequence())
    );

    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9004), return_jump)
        .expect("scalar-sequential return candidate");
    assert_eq!(
        candidate.producer_sequences(),
        &[descendant.parent().consumer_sequence()]
    );
    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(14)],
            23,
            RiscvExecutionRecord::new(return_jump, 0x9004, 0x800c, Vec::new(), None),
        )
        .unwrap());
    let returned = runtime
        .producer_forwarded_return_descendant()
        .expect("scalar-sequential return lineage");
    assert_eq!(returned.parent(), descendant.parent());
    assert_eq!(returned.scalar_chain(), &scalar_chain);
    assert_eq!(returned.pc(), Address::new(0x9004));
    assert_eq!(returned.target(), Address::new(0x800c));
}

#[test]
fn producer_forwarded_split_link_scalar_return_uses_link_destination() {
    let (mut runtime, _, scalar_chain) = recorded_linked_scalar_runtime(11, 5);
    retire_data_head(&mut runtime);

    let return_jump = jalr_return(5);
    assert!(runtime
        .append_producer_forwarded_scalar_return_descendant(
            &scalar_chain,
            Address::new(0x9004),
            decoded(return_jump),
            &[request(14)],
            0,
        )
        .is_some());
}

#[test]
fn producer_forwarded_split_link_scalar_requires_link_dependency() {
    let (_, forwarded, _) = super::producer_forwarded_return::recorded_linked_runtime(11, 5);

    for scalar in [addi(13, 11, 0), addi(5, 5, 0)] {
        assert_eq!(
            forwarded.fetched_scalar_chain(scalar, 4, &[request(13)]),
            None,
            "unexpected split-link scalar lineage for {scalar:?}"
        );
    }
}

#[test]
fn producer_forwarded_scalar_return_rejects_nonordinary_shapes() {
    for (pc, instruction) in [
        (Address::new(0x9000), jalr_return(1)),
        (
            Address::new(0x9004),
            RiscvInstruction::Jalr {
                rd: reg(0),
                rs1: reg(1),
                offset: Immediate::new(4),
            },
        ),
        (Address::new(0x9004), jalr_return(5)),
        (Address::new(0x9004), jalr_link(1, 1)),
    ] {
        let (mut runtime, _, scalar_chain) = recorded_x1_scalar_runtime();
        retire_data_head(&mut runtime);
        assert_eq!(
            runtime.append_producer_forwarded_scalar_return_descendant(
                &scalar_chain,
                pc,
                decoded(instruction),
                &[request(14)],
                0,
            ),
            None,
            "unexpected scalar-return admission for {instruction:?} at {pc:?}"
        );
    }
}

#[test]
fn producer_forwarded_scalar_lineage_fails_closed_after_identity_change() {
    let (mut runtime, _, scalar_chain) = recorded_x1_scalar_runtime();
    let descendant = scalar_chain.last().expect("one scalar descendant");
    retire_data_head(&mut runtime);
    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|execution| execution.sequence == descendant.sequence())
        .expect("scalar execution")
        .consumed_requests = vec![request(99)];

    assert_eq!(runtime.producer_forwarded_scalar_chain(), None);
    assert_eq!(
        runtime.append_producer_forwarded_scalar_return_descendant(
            &scalar_chain,
            Address::new(0x9004),
            decoded(jalr_return(1)),
            &[request(14)],
            0,
        ),
        None
    );
}
