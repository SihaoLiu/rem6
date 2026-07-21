use super::*;

#[test]
fn memory_result_replanning_invalidates_fu_conflict_chain_for_authoritative_reissue() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_writeback_width(2));
    let older = load_event(0x8000, 1, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, request(20), 31));
    let other = multiply_instruction(6, 0);
    let producer = fixed_instruction(7);
    let child = multiply_instruction(8, 7);
    let grandchild = dependent_instruction(9, 8);
    let unaffected = multiply_instruction(10, 0);
    let other_sequence = runtime
        .stage_live_retire_window(
            Address::new(0x8004),
            other,
            0,
            [
                (Address::new(0x8008), producer),
                (Address::new(0x800c), child),
                (Address::new(0x8010), grandchild),
                (Address::new(0x8014), unaffected),
            ],
        )
        .expect("fixed-FU window stages behind older memory result");
    let producer_sequence = sequence_for_pc(&runtime, 0x8008);
    let child_sequence = sequence_for_pc(&runtime, 0x800c);
    let grandchild_sequence = sequence_for_pc(&runtime, 0x8010);
    let unaffected_sequence = sequence_for_pc(&runtime, 0x8014);
    record_fixed_fu_owner(&mut runtime, other_sequence, other, 0x8004, request(30), 40);
    let producer_execution =
        record_speculative_owner(&mut runtime, 0x8008, producer, request(31), 42, 7, 7);
    let child_execution =
        record_speculative_owner(&mut runtime, 0x800c, child, request(32), 42, 8, 0);
    let grandchild_execution =
        record_speculative_owner(&mut runtime, 0x8010, grandchild, request(33), 42, 9, 1);
    let unaffected_execution =
        record_speculative_owner(&mut runtime, 0x8014, unaffected, request(34), 43, 10, 0);
    assert_eq!(
        issue_rows_by_tick(&runtime),
        vec![
            (40, vec![other_sequence]),
            (42, vec![producer_sequence, child_sequence]),
            (43, vec![unaffected_sequence]),
            (44, vec![grandchild_sequence]),
        ]
    );

    let mut completed_older = older.clone();
    completed_older.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed_older,
            request(20),
            41,
            10,
            Some(&0x1111_1111u32.to_le_bytes()),
        )
        .unwrap());

    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| issued.sequence != child_sequence && issued.sequence != grandchild_sequence));
    assert!(runtime.writeback_reservation(child_sequence).is_none());
    assert!(runtime.writeback_reservation(grandchild_sequence).is_none());
    for sequence in [child_sequence, grandchild_sequence] {
        let rob = runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == sequence)
            .copied()
            .expect("invalidated descendant keeps its staged ROB row");
        assert!(!rob.is_ready());
    }
    assert_speculative_owner(
        &runtime,
        unaffected_sequence,
        &[request(34)],
        &unaffected_execution,
        43,
        45,
        45,
        Some(0),
        &[],
    );

    let head = runtime
        .live_data_access_head_reservation(older.fetch().request_id())
        .expect("memory head remains available for authoritative reissue");
    runtime
        .schedule_live_speculative_issues(
            &RiscvHartState::new(0x8000),
            head,
            43,
            &[
                issue_request(0x800c, request(32), child),
                issue_request(0x8010, request(33), grandchild),
            ],
        )
        .unwrap();

    assert_speculative_owner(
        &runtime,
        producer_sequence,
        &[request(31)],
        &producer_execution,
        42,
        42,
        43,
        Some(0),
        &[],
    );
    assert_speculative_owner(
        &runtime,
        child_sequence,
        &[request(32)],
        &child_execution,
        44,
        46,
        46,
        Some(0),
        &[producer_sequence],
    );
    assert_speculative_owner(
        &runtime,
        grandchild_sequence,
        &[request(33)],
        &grandchild_execution,
        46,
        46,
        46,
        Some(1),
        &[child_sequence],
    );
    assert_speculative_owner(
        &runtime,
        unaffected_sequence,
        &[request(34)],
        &unaffected_execution,
        43,
        45,
        45,
        Some(0),
        &[],
    );
    assert_eq!(
        calendar_rows_with_raw(&runtime),
        vec![
            (0, 42, 42, 0),
            (other_sequence, 42, 42, 1),
            (producer_sequence, 42, 43, 0),
            (child_sequence, 46, 46, 0),
            (grandchild_sequence, 46, 46, 1),
            (unaffected_sequence, 45, 45, 0),
        ]
    );
    assert_issue_capacity(&runtime, 2);
    assert!(runtime.live_data_access_publication_is_admitted(42));
    assert_eq!(runtime.stats().issue_cycles(), 3);
    assert_eq!(runtime.stats().issued_rows(), 2);
    assert_eq!(runtime.stats().resource_blocked_row_cycles(), 1);
    assert_eq!(runtime.stats().dependency_blocked_row_cycles(), 2);
    assert_eq!(runtime.stats().max_rows_per_cycle(), 1);
    assert_writeback_stats(&runtime, 4, 6, 1, 1, 3, 1);
}

#[test]
fn invalidated_descendant_reissue_counts_additional_authoritative_planner_activity() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(1));
    assert!(runtime.set_writeback_width(1));
    let older = load_event(0x8000, 1, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, request(20), 31));
    let producer = fixed_instruction(6);
    let child = dependent_instruction(7, 6);
    let producer_sequence = runtime
        .stage_live_retire_window(
            Address::new(0x8004),
            producer,
            0,
            [(Address::new(0x8008), child)],
        )
        .expect("producer and dependent stage behind older memory result");
    let child_sequence = sequence_for_pc(&runtime, 0x8008);
    record_fixed_fu_owner(
        &mut runtime,
        producer_sequence,
        producer,
        0x8004,
        request(30),
        42,
    );
    let head = runtime
        .live_data_access_head_reservation(older.fetch().request_id())
        .expect("memory head remains live");
    let child_request = issue_request(0x8008, request(31), child);
    runtime
        .schedule_live_speculative_issues(
            &RiscvHartState::new(0x8000),
            head,
            42,
            std::slice::from_ref(&child_request),
        )
        .unwrap();
    assert_eq!(runtime.stats().issue_cycles(), 2);
    assert_eq!(runtime.stats().issued_rows(), 1);
    assert_eq!(runtime.stats().resource_blocked_row_cycles(), 1);

    let mut completed_older = older;
    completed_older.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed_older,
            request(20),
            41,
            10,
            Some(&0x1111_1111u32.to_le_bytes()),
        )
        .unwrap());
    assert!(runtime.writeback_reservation(child_sequence).is_none());

    runtime
        .schedule_live_speculative_issues(&RiscvHartState::new(0x8000), head, 43, &[child_request])
        .unwrap();

    assert_eq!(runtime.stats().issue_cycles(), 2);
    assert_eq!(runtime.stats().issued_rows(), 2);
    assert_eq!(runtime.stats().resource_blocked_row_cycles(), 1);
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == child_sequence)
            .map(|issued| issued.issue_tick),
        Some(43)
    );
}
