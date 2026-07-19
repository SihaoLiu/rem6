use super::*;

#[test]
fn staged_window_truncation_prunes_control_lineage() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), beq(5, 6)),
            (Address::new(0x8008), mul(7, 1, 2)),
            (Address::new(0x800c), addi(8, 7, 1)),
        ],
    );
    assert_eq!(runtime.live_control_lineages.len(), 3);
    assert_eq!(
        runtime
            .live_control_lineages
            .values()
            .copied()
            .filter_map(O3LiveControlLineage::pending_control_sequence)
            .count(),
        2
    );
    let load_sequence = runtime.snapshot().reorder_buffer()[0].sequence();

    runtime.discard_live_staged_window_from(load_sequence);

    assert!(runtime.live_control_lineages.is_empty());
    assert!(!runtime.has_live_control_window());
}

#[test]
fn validated_committed_control_keeps_scalar_descendant_window_until_drain() {
    let mut runtime = O3RuntimeState::default();
    let branch = beq(5, 6);
    let scalar = addi(7, 0, 1);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        branch,
        0,
        [(Address::new(0x8004), scalar)],
    );
    let branch_sequence = runtime.snapshot().reorder_buffer()[0].sequence();
    let scalar_sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    assert!(runtime.is_live_control_window_sequence(scalar_sequence));

    runtime.validate_live_speculative_producer(branch_sequence);
    runtime.snapshot.reorder_buffer[0].mark_ready();
    runtime.commit_live_rob_prefix(1, 0);

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(
        runtime.live_control_lineage_parent_for_test(scalar_sequence),
        Some(branch_sequence)
    );
    assert_eq!(
        runtime.pending_live_control_lineage_parent_for_test(scalar_sequence),
        None
    );
    assert!(runtime.has_live_control_window());

    runtime.snapshot.reorder_buffer[0].mark_ready();
    runtime.commit_live_rob_prefix(1, 0);

    assert!(!runtime.has_live_control_window());
}

#[test]
fn invalidated_resident_control_and_descendant_do_not_keep_window_live() {
    let branch = beq(5, 6);
    let scalar = addi(7, 0, 1);
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), scalar),
        ],
    );
    let branch_sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    let scalar_sequence = runtime.snapshot().reorder_buffer()[2].sequence();
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .expect("resident branch should issue while the load is pending");
    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(11)],
            11,
            RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap();

    let mismatched = RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 11),
        branch,
        RiscvExecutionRecord::new(branch, 0x8004, 0x8010, Vec::new(), None),
    );
    runtime.retire_live_staged_instruction(&mismatched, &[request(11)], 40);

    assert!(runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .any(|entry| { entry.sequence() == branch_sequence && entry.is_live_staged() }));
    assert!(runtime
        .live_staged_fetch_identities
        .contains_key(&branch_sequence));
    assert!(!runtime.live_control_lineages.contains_key(&branch_sequence));
    assert!(!runtime.live_control_lineages.contains_key(&scalar_sequence));
    assert!(!runtime.is_live_control_window_sequence(scalar_sequence));
    assert!(!runtime.has_live_control_window());
}
