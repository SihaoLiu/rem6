use super::*;

#[test]
fn same_window_coroutine_uses_call_forwarding_and_link_destination() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let call = jal_link(1, 8);
    let coroutine = jalr_link(5, 1);
    let descendant = addi(8, 5, 0);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), coroutine),
                (Address::new(0x8008), descendant),
            ],
        ),
        3
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("linked call candidate");
    let call_sequence = call_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                call,
                0x8004,
                0x800c,
                vec![RegisterWrite::new(reg(1), 0x8008)],
                None,
            ),
        )
        .unwrap());

    let coroutine_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), coroutine)
        .expect("same-window coroutine candidate");
    let coroutine_sequence = coroutine_candidate.sequence();
    assert_eq!(
        coroutine_candidate.destination().unwrap().architectural(),
        5
    );
    assert!(coroutine_candidate
        .producer_sequences()
        .contains(&call_sequence));
    assert_eq!(
        coroutine_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x8008)]
    );
    assert_eq!(coroutine_candidate.issue_tick(1), 20);
    assert!(runtime
        .record_live_speculative_execution(
            coroutine_candidate,
            &[request(12)],
            1,
            RiscvExecutionRecord::new(
                coroutine,
                0x800c,
                0x8008,
                vec![RegisterWrite::new(reg(5), 0x8010)],
                None,
            ),
        )
        .unwrap());
    let issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == coroutine_sequence)
        .expect("recorded coroutine execution");
    let coroutine_admitted_writeback_tick = issued.admitted_writeback_tick;
    assert_eq!(issued.writeback_slot, Some(0));
    assert!(coroutine_admitted_writeback_tick >= 20);
    assert_eq!(
        runtime
            .writeback_reservation(coroutine_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(coroutine_admitted_writeback_tick)
    );

    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), descendant)
        .expect("coroutine result should wake the staged descendant");
    let descendant_sequence = descendant_candidate.sequence();
    let descendant_producer_sequences = descendant_candidate.producer_sequences().to_vec();
    assert!(descendant_producer_sequences.contains(&coroutine_sequence));
    assert!(descendant_producer_sequences
        .iter()
        .all(|sequence| *sequence == coroutine_sequence));
    assert_eq!(
        descendant_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(5), 0x8010)]
    );
    assert_eq!(
        descendant_candidate.issue_tick(1),
        coroutine_admitted_writeback_tick
    );
    assert!(runtime
        .record_live_speculative_execution(
            descendant_candidate,
            &[request(13)],
            1,
            RiscvExecutionRecord::new(
                descendant,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(8), 0x8010)],
                None,
            ),
        )
        .unwrap());
    let descendant_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == descendant_sequence)
        .expect("recorded descendant execution");
    assert_eq!(
        descendant_issued.producer_sequences,
        descendant_producer_sequences
    );
    assert_eq!(
        descendant_issued.issue_tick,
        coroutine_admitted_writeback_tick
    );
    assert_eq!(
        descendant_issued.raw_ready_tick,
        coroutine_admitted_writeback_tick
    );
    assert_eq!(
        descendant_issued.admitted_writeback_tick,
        coroutine_admitted_writeback_tick + 1
    );
    assert_eq!(descendant_issued.writeback_slot, Some(0));
    assert_eq!(
        runtime
            .writeback_reservation(descendant_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(descendant_issued.admitted_writeback_tick)
    );
    assert!(runtime.has_live_control_descendants(coroutine_sequence));
    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [
            Address::new(0x8000),
            Address::new(0x8004),
            Address::new(0x800c),
            Address::new(0x8008),
        ]
    );

    runtime.discard_live_control_descendants_from_at(
        coroutine_sequence,
        coroutine_admitted_writeback_tick,
    );

    assert!(!runtime.has_live_control_descendants(coroutine_sequence));
    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [
            Address::new(0x8000),
            Address::new(0x8004),
            Address::new(0x800c),
        ]
    );
    assert!(runtime.writeback_reservation(descendant_sequence).is_none());
}

#[test]
fn same_window_coroutine_round_trip_serializes_three_controls() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let call = jal_link(1, 8);
    let coroutine = jalr_link(5, 1);
    let return_jump = jalr_return(5);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), coroutine),
                (Address::new(0x8008), return_jump),
            ],
        ),
        3
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("linked call candidate");
    let call_sequence = call_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                call,
                0x8004,
                0x800c,
                vec![RegisterWrite::new(reg(1), 0x8008)],
                None,
            ),
        )
        .unwrap());
    let call_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == call_sequence)
        .expect("recorded linked call execution");
    let call_admitted_writeback_tick = call_issued.admitted_writeback_tick;

    let coroutine_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), coroutine)
        .expect("same-window coroutine candidate");
    let coroutine_sequence = coroutine_candidate.sequence();
    assert_eq!(coroutine_candidate.producer_sequences(), &[call_sequence]);
    assert_eq!(
        coroutine_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x8008)]
    );
    assert_eq!(
        coroutine_candidate.issue_tick(1),
        call_admitted_writeback_tick
    );
    assert!(runtime
        .record_live_speculative_execution(
            coroutine_candidate,
            &[request(12)],
            1,
            RiscvExecutionRecord::new(
                coroutine,
                0x800c,
                0x8008,
                vec![RegisterWrite::new(reg(5), 0x8010)],
                None,
            ),
        )
        .unwrap());
    let coroutine_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == coroutine_sequence)
        .expect("recorded coroutine execution");
    let coroutine_admitted_writeback_tick = coroutine_issued.admitted_writeback_tick;

    let return_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), return_jump)
        .expect("same-window ordinary return candidate");
    let return_sequence = return_candidate.sequence();
    assert_eq!(return_candidate.destination(), None);
    assert_eq!(return_candidate.producer_sequences(), &[coroutine_sequence]);
    assert_eq!(
        return_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(5), 0x8010)]
    );
    assert_eq!(
        return_candidate.issue_tick(1),
        coroutine_admitted_writeback_tick
    );
    assert!(runtime
        .record_live_speculative_execution(
            return_candidate,
            &[request(13)],
            1,
            RiscvExecutionRecord::new(return_jump, 0x8008, 0x8010, Vec::new(), None),
        )
        .unwrap());
    let return_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == return_sequence)
        .expect("recorded ordinary return execution");
    assert_eq!(return_issued.producer_sequences, vec![coroutine_sequence]);
    assert_eq!(return_issued.issue_tick, coroutine_admitted_writeback_tick);
    assert_eq!(
        return_issued.raw_ready_tick,
        coroutine_admitted_writeback_tick
    );
    assert_eq!(return_issued.writeback_slot, None);
    assert!(runtime.writeback_reservation(return_sequence).is_none());
    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x800c, 0x8008].map(Address::new)
    );
    assert!(runtime.has_live_control_descendants(coroutine_sequence));

    runtime.discard_live_control_descendants_from_at(
        coroutine_sequence,
        coroutine_admitted_writeback_tick,
    );

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x800c].map(Address::new)
    );
    assert!(!runtime.has_live_control_descendants(coroutine_sequence));
}

#[test]
fn same_window_reverse_coroutine_forwards_both_links_and_discards_descendant() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let call = jalr_link(5, 11);
    let coroutine = jalr_link(1, 5);
    let descendant = addi(8, 1, 0);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), coroutine),
                (Address::new(0x8008), descendant),
            ],
        ),
        3
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("committed-source indirect linked call candidate");
    let call_sequence = call_candidate.sequence();
    assert_eq!(call_candidate.destination().unwrap().architectural(), 5);
    assert!(call_candidate.producer_sequences().is_empty());
    assert!(call_candidate.forwarded_register_writes().is_empty());
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                call,
                0x8004,
                0x800c,
                vec![RegisterWrite::new(reg(5), 0x8008)],
                None,
            ),
        )
        .unwrap());
    let call_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == call_sequence)
        .expect("recorded committed-source indirect call execution");
    let call_admitted_writeback_tick = call_issued.admitted_writeback_tick;
    assert_eq!(call_issued.issue_tick, 20);
    assert_eq!(call_issued.raw_ready_tick, 20);
    assert_eq!(call_admitted_writeback_tick, 20);
    assert_eq!(call_issued.writeback_slot, Some(0));

    let coroutine_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), coroutine)
        .expect("reverse same-window coroutine candidate");
    let coroutine_sequence = coroutine_candidate.sequence();
    assert_eq!(
        coroutine_candidate.destination().unwrap().architectural(),
        1
    );
    assert!(coroutine_candidate
        .producer_sequences()
        .contains(&call_sequence));
    assert!(coroutine_candidate
        .producer_sequences()
        .iter()
        .all(|sequence| *sequence == call_sequence));
    assert_eq!(
        coroutine_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(5), 0x8008)]
    );
    assert_eq!(
        coroutine_candidate.issue_tick(1),
        call_admitted_writeback_tick
    );
    assert!(runtime
        .record_live_speculative_execution(
            coroutine_candidate,
            &[request(12)],
            1,
            RiscvExecutionRecord::new(
                coroutine,
                0x800c,
                0x8008,
                vec![RegisterWrite::new(reg(1), 0x8010)],
                None,
            ),
        )
        .unwrap());
    let coroutine_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == coroutine_sequence)
        .expect("recorded reverse coroutine execution");
    let coroutine_admitted_writeback_tick = coroutine_issued.admitted_writeback_tick;
    assert_eq!(coroutine_issued.issue_tick, call_admitted_writeback_tick);
    assert_eq!(
        coroutine_issued.raw_ready_tick,
        call_admitted_writeback_tick
    );
    assert_eq!(
        coroutine_admitted_writeback_tick,
        call_admitted_writeback_tick + 1
    );
    assert_eq!(coroutine_issued.writeback_slot, Some(0));
    assert_eq!(
        runtime
            .writeback_reservation(coroutine_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(coroutine_admitted_writeback_tick)
    );

    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), descendant)
        .expect("reverse coroutine result should wake the staged descendant");
    let descendant_sequence = descendant_candidate.sequence();
    assert!(descendant_candidate
        .producer_sequences()
        .contains(&coroutine_sequence));
    assert!(descendant_candidate
        .producer_sequences()
        .iter()
        .all(|sequence| *sequence == coroutine_sequence));
    assert_eq!(
        descendant_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x8010)]
    );
    assert_eq!(
        descendant_candidate.issue_tick(1),
        coroutine_admitted_writeback_tick
    );
    assert!(runtime
        .record_live_speculative_execution(
            descendant_candidate,
            &[request(13)],
            1,
            RiscvExecutionRecord::new(
                descendant,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(8), 0x8010)],
                None,
            ),
        )
        .unwrap());
    let descendant_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == descendant_sequence)
        .expect("recorded reverse coroutine descendant execution");
    assert_eq!(
        descendant_issued.issue_tick,
        coroutine_admitted_writeback_tick
    );
    assert_eq!(
        descendant_issued.raw_ready_tick,
        coroutine_admitted_writeback_tick
    );
    assert_eq!(
        descendant_issued.admitted_writeback_tick,
        coroutine_admitted_writeback_tick + 1
    );
    assert_eq!(descendant_issued.writeback_slot, Some(0));
    assert_eq!(
        runtime
            .writeback_reservation(descendant_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(descendant_issued.admitted_writeback_tick)
    );
    assert!(runtime.has_live_control_descendants(coroutine_sequence));
    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x800c, 0x8008].map(Address::new)
    );

    runtime.discard_live_control_descendants_from_at(
        coroutine_sequence,
        coroutine_admitted_writeback_tick,
    );

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x800c].map(Address::new)
    );
    assert!(!runtime.has_live_control_descendants(coroutine_sequence));
    assert!(runtime.writeback_reservation(descendant_sequence).is_none());
}
