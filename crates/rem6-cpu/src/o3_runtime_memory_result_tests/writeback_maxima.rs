use super::*;

#[test]
fn finalized_maxima_survive_equal_live_maxima_replanned_down() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    reserve_three_ready_and_deferred_rows(&mut runtime, 1_000, 10);
    assert_writeback_totals(&runtime, 5, 4, 7, 3, 3);

    for sequence in 1_000..1_005 {
        runtime.finalize_writeback_publication(sequence);
    }
    runtime.prune_writeback_calendar_before(14);
    assert!(runtime.writeback_reservations().is_empty());

    replan_three_live_rows_down_to_two(&mut runtime);

    assert_writeback_totals(&runtime, 9, 7, 11, 3, 3);
}

#[test]
fn live_maxima_without_history_follow_replan_down() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));

    replan_three_live_rows_down_to_two(&mut runtime);

    assert_writeback_totals(&runtime, 4, 3, 4, 2, 2);
}

#[test]
fn drained_checkpoint_restore_seeds_finalized_maxima() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        O3RuntimeState::default().snapshot(),
        O3RuntimeStats {
            writeback_port_max_ready_rows_per_cycle: 3,
            writeback_port_max_deferred_rows: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let mut runtime = O3RuntimeState::default();
    runtime.restore_checkpoint_payload(payload).unwrap();
    assert!(runtime.set_writeback_width(1));

    replan_three_live_rows_down_to_two(&mut runtime);

    assert_writeback_totals(&runtime, 4, 3, 4, 3, 3);
}

#[test]
fn stats_reset_clears_finalized_writeback_maxima() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    reserve_three_ready_and_deferred_rows(&mut runtime, 1_000, 10);
    for sequence in 1_000..1_005 {
        runtime.finalize_writeback_publication(sequence);
    }
    runtime.prune_writeback_calendar_before(14);
    runtime.reset_stats();

    replan_three_live_rows_down_to_two(&mut runtime);

    assert_writeback_totals(&runtime, 4, 3, 4, 2, 2);
}

fn reserve_three_ready_and_deferred_rows(
    runtime: &mut O3RuntimeState,
    first_sequence: u64,
    raw_ready_tick: u64,
) {
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(first_sequence, raw_ready_tick - 1),
            O3LiveWritebackReady::fixed_fu(first_sequence + 1, raw_ready_tick - 1),
        ])
        .unwrap();
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(first_sequence + 2, raw_ready_tick),
            O3LiveWritebackReady::fixed_fu(first_sequence + 3, raw_ready_tick),
            O3LiveWritebackReady::fixed_fu(first_sequence + 4, raw_ready_tick),
        ])
        .unwrap();
}

fn replan_three_live_rows_down_to_two(runtime: &mut O3RuntimeState) {
    let older = load_event(0x8000, 1, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, request(20), 31));
    let producer = fixed_instruction(6);
    let child = fixed_instruction(7);
    let grandchild = fixed_instruction(8);
    let producer_sequence = runtime
        .stage_live_retire_window(
            Address::new(0x8004),
            producer,
            0,
            [
                (Address::new(0x8008), child),
                (Address::new(0x800c), grandchild),
            ],
        )
        .expect("fixed-FU chain stages behind the older memory result");
    let child_sequence = sequence_for_pc(runtime, 0x8008);
    let grandchild_sequence = sequence_for_pc(runtime, 0x800c);
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(2_000, 41),
            O3LiveWritebackReady::fixed_fu(2_001, 41),
        ])
        .unwrap();
    record_fixed_fu_owner(
        runtime,
        producer_sequence,
        producer,
        0x8004,
        request(30),
        42,
    );
    record_speculative_owner(runtime, 0x8008, child, request(31), 42, 7, 7);
    record_speculative_owner(runtime, 0x800c, grandchild, request(32), 42, 8, 8);
    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|issued| issued.sequence == child_sequence)
        .expect("child owner exists")
        .producer_sequences = vec![producer_sequence];
    runtime
        .live_speculative_executions
        .iter_mut()
        .find(|issued| issued.sequence == grandchild_sequence)
        .expect("grandchild owner exists")
        .producer_sequences = vec![child_sequence];
    assert_eq!(runtime.stats().writeback_port_max_ready_rows_per_cycle(), 3);
    assert_eq!(runtime.stats().writeback_port_max_deferred_rows(), 3);

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

    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| issued.sequence != child_sequence && issued.sequence != grandchild_sequence));
    assert!(runtime.writeback_reservation(child_sequence).is_none());
    assert!(runtime.writeback_reservation(grandchild_sequence).is_none());
}

fn assert_writeback_totals(
    runtime: &O3RuntimeState,
    admitted_rows: u64,
    deferred_rows: u64,
    deferred_row_cycles: u64,
    max_ready_rows_per_cycle: u64,
    max_deferred_rows: u64,
) {
    let stats = runtime.stats();
    assert_eq!(stats.writeback_port_admitted_rows(), admitted_rows);
    assert_eq!(stats.writeback_port_deferred_rows(), deferred_rows);
    assert_eq!(
        stats.writeback_port_deferred_row_cycles(),
        deferred_row_cycles
    );
    assert_eq!(
        stats.writeback_port_max_ready_rows_per_cycle(),
        max_ready_rows_per_cycle
    );
    assert_eq!(stats.writeback_port_max_deferred_rows(), max_deferred_rows);
}
