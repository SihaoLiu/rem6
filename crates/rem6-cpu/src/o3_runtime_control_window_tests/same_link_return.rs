use super::*;

pub(super) fn recorded_x1_same_link_runtime(
) -> (O3RuntimeState, O3ProducerForwardedControlTarget, u64) {
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
        .producer_forwarded_same_link_control_target()
        .expect("same-link forwarded target authority");
    assert!(runtime.record_producer_forwarded_same_link_control_target(forwarded));
    (runtime, forwarded, consumer_sequence)
}

#[test]
fn producer_forwarded_same_link_call_appends_target_return() {
    let (mut runtime, forwarded, consumer_sequence) = recorded_x1_same_link_runtime();
    let return_jump = jalr_return(1);
    let return_sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            return_jump,
            &[request(13)],
        )
        .expect("same-link call target return append");

    assert_eq!(
        runtime.live_control_dependencies.get(&return_sequence),
        Some(&consumer_sequence)
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9000), return_jump)
        .expect("same-link call target return candidate");
    assert_eq!(candidate.destination(), None);
    assert_eq!(candidate.producer_sequences(), &[consumer_sequence]);
    assert_eq!(
        candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x800c)]
    );
    assert_eq!(candidate.control_dependency(), Some(consumer_sequence));
    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(13)],
            22,
            RiscvExecutionRecord::new(return_jump, 0x9000, 0x800c, Vec::new(), None),
        )
        .unwrap());
    let descendant = runtime
        .producer_forwarded_same_link_return_descendant()
        .expect("same-link call target return lineage");
    assert_eq!(descendant.parent(), forwarded);
    assert_eq!(descendant.fetch_request(), request(13));
    assert_eq!(descendant.pc(), Address::new(0x9000));
    assert_eq!(descendant.target(), Address::new(0x800c));

    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|execution| execution.sequence == forwarded.producer_sequence())
        .expect("same-link producer execution")
        .admitted_writeback_tick += 2;
    assert_eq!(
        runtime
            .producer_forwarded_same_link_return_descendant()
            .expect("scheduling-only retime keeps return lineage"),
        descendant
    );
}

#[test]
fn producer_forwarded_same_link_call_rejects_nonordinary_target_controls() {
    for instruction in [
        RiscvInstruction::Jalr {
            rd: reg(0),
            rs1: reg(1),
            offset: Immediate::new(4),
        },
        jalr_return(5),
        jalr_link(5, 1),
    ] {
        let (mut runtime, forwarded, _) = recorded_x1_same_link_runtime();
        assert_eq!(
            runtime.append_producer_forwarded_control_descendant(
                forwarded,
                Address::new(0x9000),
                instruction,
                &[request(13)],
            ),
            None,
            "unexpected producer-forwarded target control admission for {instruction:?}"
        );
    }
}

#[test]
fn producer_forwarded_same_link_call_appends_return_after_data_head_retires() {
    let (mut runtime, forwarded, consumer_sequence) = recorded_x1_same_link_runtime();
    runtime.live_data_accesses.clear();
    runtime.snapshot.reorder_buffer.remove(0);

    let return_jump = jalr_return(1);
    let return_sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            return_jump,
            &[request(13)],
        )
        .expect("recorded same-link call survives successful data-head retirement");
    assert_eq!(
        runtime.live_control_dependencies.get(&return_sequence),
        Some(&consumer_sequence)
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9000), return_jump)
        .expect("post-head same-link return candidate");
    assert_eq!(candidate.producer_sequences(), &[consumer_sequence]);
}
