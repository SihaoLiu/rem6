use super::*;

#[test]
fn committed_return_retains_exact_recorded_fetch_identity() {
    let (mut runtime, forwarded, _) = recorded_linked_runtime(1, 1);
    let call = jalr_link(1, 1);
    let return_jump = jalr_return(1);
    let return_sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            decoded(return_jump),
            &[request(13)],
            0,
        )
        .expect("linked call target return append");
    let staged_descendant = runtime
        .producer_forwarded_return_descendant()
        .expect("staged linked call target return lineage");
    assert_eq!(staged_descendant.parent(), forwarded);
    assert_eq!(staged_descendant.fetch_request(), request(13));
    assert!(runtime.has_recorded_producer_forwarded_return_descendant(return_sequence));

    let commits = runtime.snapshot.reorder_buffer.len();
    runtime.commit_live_rob_prefix(commits, 22);

    assert_eq!(runtime.producer_forwarded_return_descendant(), None);
    assert_eq!(
        runtime.recorded_producer_forwarded_control_sequence_for_fetch_identity(
            Address::new(0x8008),
            call,
            &[request(12)],
        ),
        Some(forwarded.consumer_sequence())
    );
    assert!(
        runtime.recorded_producer_forwarded_return_matches_fetch_identity(
            Address::new(0x9000),
            return_jump,
            &[request(13)],
        )
    );
    assert!(
        !runtime.recorded_producer_forwarded_return_matches_fetch_identity(
            Address::new(0x9000),
            return_jump,
            &[request(99)],
        )
    );
    assert!(runtime.consume_committed_live_staged_fetch_identity(
        Address::new(0x8008),
        call,
        &[request(12)],
    ));
    assert!(runtime.consume_committed_live_staged_fetch_identity(
        Address::new(0x9000),
        return_jump,
        &[request(13)],
    ));
    assert!(
        !runtime.recorded_producer_forwarded_return_matches_fetch_identity(
            Address::new(0x9000),
            return_jump,
            &[request(13)],
        )
    );
}
