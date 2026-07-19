use super::*;

#[test]
fn live_no_link_and_split_link_controls_expose_exact_producer_forwarded_targets() {
    for destination in [0, 5] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        let producer = addi(11, 10, 0);
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
                    (Address::new(0x8008), control),
                ],
            ),
            2
        );

        let producer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), producer)
            .expect("JALR target producer candidate");
        let producer_sequence = producer_candidate.sequence();
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
        let control_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), control)
            .expect("producer-forwarded JALR candidate");
        let consumer_sequence = control_candidate.sequence();
        assert_eq!(control_candidate.producer_sequences(), &[producer_sequence]);
        let writes = if destination != 0 {
            vec![RegisterWrite::new(reg(destination), 0x800c)]
        } else {
            Vec::new()
        };
        assert!(runtime
            .record_live_speculative_execution(
                control_candidate,
                &[request(12)],
                21,
                RiscvExecutionRecord::new(control, 0x8008, 0x9000, writes, None),
            )
            .unwrap());

        let forwarded = runtime
            .producer_forwarded_control_target()
            .unwrap_or_else(|| panic!("missing destination x{destination} target authority"));
        assert_eq!(forwarded.target_source(), reg(11));
        assert_eq!(forwarded.target(), Address::new(0x9000));
        assert_eq!(
            forwarded.link_destination(),
            (destination != 0).then(|| reg(destination))
        );

        runtime
            .live_speculative_executions
            .iter_mut()
            .find(|execution| execution.sequence == producer_sequence)
            .unwrap()
            .consumed_requests = vec![request(99)];
        assert!(runtime.producer_forwarded_control_target().is_none());
        runtime
            .live_speculative_executions
            .iter_mut()
            .find(|execution| execution.sequence == producer_sequence)
            .unwrap()
            .consumed_requests = vec![request(11)];
        runtime
            .live_speculative_executions
            .iter_mut()
            .find(|execution| execution.sequence == consumer_sequence)
            .unwrap()
            .consumed_requests = vec![request(99)];
        assert!(runtime.producer_forwarded_control_target().is_none());
    }
}

#[test]
fn live_same_link_control_exposes_exact_producer_forwarded_target() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let producer = addi(1, 11, 0);
    let call = jalr_link(1, 1);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), producer),
                (Address::new(0x8008), call),
            ],
        ),
        2
    );

    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .expect("same-link target producer candidate");
    let producer_sequence = producer_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(1), 0x9000)],
                None,
            ),
        )
        .unwrap());
    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), call)
        .expect("same-link control candidate");
    let consumer_sequence = call_candidate.sequence();
    assert_eq!(call_candidate.producer_sequences(), &[producer_sequence]);
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(12)],
            21,
            RiscvExecutionRecord::new(
                call,
                0x8008,
                0x9000,
                vec![RegisterWrite::new(reg(1), 0x800c)],
                None,
            ),
        )
        .unwrap());

    let forwarded = runtime
        .producer_forwarded_control_target()
        .expect("same-link forwarded target authority");
    assert_eq!(forwarded.data_access_fetch_request(), request(10));
    assert_eq!(forwarded.fetch_request(), request(12));
    assert_eq!(forwarded.pc(), Address::new(0x8008));
    assert_eq!(forwarded.sequential_pc(), Address::new(0x800c));
    assert_eq!(forwarded.consumer_sequence(), consumer_sequence);
    assert_eq!(forwarded.producer_sequence(), producer_sequence);
    assert_eq!(forwarded.target_source(), reg(1));
    assert_eq!(forwarded.target(), Address::new(0x9000));
    assert_eq!(forwarded.link_destination(), Some(reg(1)));
    assert!(
        runtime.record_producer_forwarded_control_target(forwarded, BranchSpeculationId::new(1),)
    );
    assert!(runtime.has_recorded_producer_forwarded_control_target(consumer_sequence));

    let original_ready_tick = forwarded.ready_tick();
    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|execution| execution.sequence == forwarded.producer_sequence())
        .expect("same-link producer execution")
        .admitted_writeback_tick += 2;
    let forwarded = runtime
        .retained_producer_forwarded_control_target()
        .expect("scheduling-only replan must retain same-link target authority");
    assert_eq!(forwarded.ready_tick(), original_ready_tick + 2);

    let descendant = addi(13, 1, 0);
    let descendant_sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            descendant,
            &[request(13)],
        )
        .expect("same-link target descendant append");
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant_sequence),
        Some(&consumer_sequence)
    );
    assert_eq!(runtime.live_data_access_younger_sequences.len(), 3);
    assert!(runtime.has_recorded_producer_forwarded_control_target(consumer_sequence));
    assert!(runtime.producer_forwarded_control_target().is_none());
    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9000), descendant)
        .expect("same-link target descendant candidate");
    assert_eq!(
        descendant_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x800c)]
    );
    assert_eq!(
        descendant_candidate.control_dependency(),
        Some(consumer_sequence)
    );
}
