use super::*;

fn nonadjacent_forwarded_runtime(destination: u8) -> (O3RuntimeState, u64, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let producer = addi(11, 10, 0);
    let spacer = addi(14, 0, 7);
    let control = if destination == 0 {
        jalr(11)
    } else {
        jalr_link(destination, 11)
    };
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), producer),
                (Address::new(0x8008), spacer),
                (Address::new(0x800c), control),
            ],
        ),
        3
    );

    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .expect("non-adjacent target producer candidate");
    let producer_sequence = producer_candidate.sequence();
    bind_o3(&mut runtime, 0x8004, decoded(producer), &[request(11)]);
    assert!(runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(11), 0x9000)],
                None,
            ),
        )
        .unwrap());

    let spacer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), spacer)
        .expect("independent spacer candidate");
    let spacer_sequence = spacer_candidate.sequence();
    bind_o3(&mut runtime, 0x8008, decoded(spacer), &[request(12)]);
    assert!(runtime
        .record_live_speculative_execution(
            spacer_candidate,
            &[request(12)],
            20,
            RiscvExecutionRecord::new(
                spacer,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(14), 7)],
                None,
            ),
        )
        .unwrap());

    let control_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), control)
        .expect("non-adjacent producer-forwarded JALR candidate");
    let consumer_sequence = control_candidate.sequence();
    assert_eq!(control_candidate.producer_sequences(), &[producer_sequence]);
    let writes = if destination == 0 {
        Vec::new()
    } else {
        vec![RegisterWrite::new(reg(destination), 0x8010)]
    };
    bind_o3(&mut runtime, 0x800c, decoded(control), &[request(13)]);
    assert!(runtime
        .record_live_speculative_execution(
            control_candidate,
            &[request(13)],
            21,
            RiscvExecutionRecord::new(control, 0x800c, 0x9000, writes, None),
        )
        .unwrap());

    (
        runtime,
        producer_sequence,
        spacer_sequence,
        consumer_sequence,
    )
}

#[test]
fn nonadjacent_no_link_and_split_link_controls_use_exact_dependency_producer() {
    for destination in [0, 5] {
        let (runtime, producer_sequence, spacer_sequence, consumer_sequence) =
            nonadjacent_forwarded_runtime(destination);
        let forwarded = runtime
            .producer_forwarded_control_target()
            .unwrap_or_else(|| panic!("missing non-adjacent x{destination} target authority"));

        assert_eq!(forwarded.producer_sequence(), producer_sequence);
        assert_ne!(forwarded.producer_sequence(), spacer_sequence);
        assert_eq!(forwarded.consumer_sequence(), consumer_sequence);
        assert_eq!(forwarded.target_source(), reg(11));
        assert_eq!(forwarded.target(), Address::new(0x9000));
        assert_eq!(
            forwarded.link_destination(),
            (destination != 0).then(|| reg(destination))
        );
    }
}

#[test]
fn nonadjacent_control_rejects_missing_or_ambiguous_dependency() {
    for multiple in [false, true] {
        let (mut runtime, producer_sequence, spacer_sequence, consumer_sequence) =
            nonadjacent_forwarded_runtime(0);
        runtime
            .live_speculative_executions
            .iter_mut()
            .find(|execution| execution.sequence == consumer_sequence)
            .expect("consumer execution")
            .producer_sequences = if multiple {
            vec![producer_sequence, spacer_sequence]
        } else {
            Vec::new()
        };

        assert_eq!(runtime.producer_forwarded_control_target(), None);
    }
}

#[test]
fn recorded_nonadjacent_target_revalidates_after_data_head_retire() {
    let (mut runtime, _, _, consumer_sequence) = nonadjacent_forwarded_runtime(5);
    let forwarded = runtime
        .producer_forwarded_control_target()
        .expect("live non-adjacent target");
    assert!(
        runtime.record_producer_forwarded_control_target(forwarded, BranchSpeculationId::new(1),)
    );

    runtime.live_data_accesses.clear();
    runtime.snapshot.reorder_buffer.remove(0);
    let retained = runtime
        .producer_forwarded_control_target_after_head_retire()
        .expect("post-head-retire non-adjacent target");
    assert_eq!(retained.consumer_sequence(), consumer_sequence);

    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|execution| execution.sequence == consumer_sequence)
        .expect("consumer execution")
        .consumed_requests = vec![request(99)];
    assert_eq!(
        runtime.producer_forwarded_control_target_after_head_retire(),
        None
    );
}
