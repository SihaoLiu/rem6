use super::*;

#[test]
fn prune_retains_an_unpublished_late_same_tick_reservation() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    let raw_ready_tick = 42;

    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(1, raw_ready_tick)])
        .unwrap();
    runtime.finalize_writeback_publication(1);
    assert_partial_writeback_ownership(&runtime, 1, 1, 0);

    let before_error = runtime.clone();
    let error = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(1, raw_ready_tick)])
        .unwrap_err();
    assert_eq!(
        error,
        O3RuntimeError::WritebackReservationSourceMismatch {
            sequence: 1,
            existing_source: "FixedFu",
            requested_source: "MemoryResult",
        }
    );
    assert_eq!(runtime, before_error);

    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(2, raw_ready_tick)])
        .unwrap();
    assert_partial_writeback_ownership(&runtime, 1, 1, 0);
    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| (reservation.sequence(), reservation.admitted_tick()))
            .collect::<Vec<_>>(),
        vec![(1, 42), (2, 43)]
    );
    let stats = runtime.stats();
    assert_eq!(
        (
            stats.writeback_port_cycles(),
            stats.writeback_port_admitted_rows(),
            stats.writeback_port_deferred_rows(),
            stats.writeback_port_deferred_row_cycles(),
            stats.writeback_port_max_ready_rows_per_cycle(),
            stats.writeback_port_max_deferred_rows(),
        ),
        (2, 2, 1, 1, 2, 1)
    );

    runtime.prune_writeback_calendar_before(44);
    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| (reservation.sequence(), reservation.admitted_tick()))
            .collect::<Vec<_>>(),
        vec![(2, 43)]
    );
    assert_unpublished_live_writeback_rows(&runtime, &[2]);
    assert_writeback_stats(&runtime, 2, 2, 1, 1, 2, 1);

    runtime.finalize_writeback_publication(2);
    runtime.prune_writeback_calendar_before(44);
    assert_live_writeback_ownership(&runtime, 0);
}

#[test]
fn prune_retains_unpublished_past_fixed_fu_reservation_for_replanning() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(10, 20)])
        .unwrap();

    runtime.prune_writeback_calendar_before(21);

    assert_eq!(
        runtime
            .writeback_reservation(10)
            .map(O3WritebackReservation::admitted_tick),
        Some(20)
    );
    assert!(runtime.live_writeback_counted_sequences.contains(&10));
    let replanned = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(11, 20)])
        .unwrap();
    assert_eq!(replanned[0].admitted_tick(), 21);
    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| (reservation.sequence(), reservation.admitted_tick()))
            .collect::<Vec<_>>(),
        vec![(10, 20), (11, 21)]
    );
}

#[test]
fn prune_removes_published_past_fixed_fu_reservation() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(10, 20)])
        .unwrap();
    runtime.finalize_writeback_publication(10);

    runtime.prune_writeback_calendar_before(21);

    assert!(runtime.writeback_reservation(10).is_none());
    assert!(runtime.published_writeback_sequences.is_empty());
    assert!(runtime.live_writeback_counted_sequences.is_empty());
}

#[test]
fn publishing_one_width_two_row_keeps_its_coadmitted_peer_live() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(2));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(10, 20),
            O3LiveWritebackReady::fixed_fu(12, 20),
        ])
        .unwrap();

    runtime.finalize_writeback_publication(10);
    let partition_after_publication = (
        runtime
            .published_writeback_sequences
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        runtime
            .live_writeback_counted_sequences
            .iter()
            .copied()
            .collect::<Vec<_>>(),
    );

    let before_error = runtime.clone();
    let error = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(12, 20)])
        .unwrap_err();
    assert_eq!(
        error,
        O3RuntimeError::WritebackReservationSourceMismatch {
            sequence: 12,
            existing_source: "FixedFu",
            requested_source: "MemoryResult",
        }
    );
    assert_eq!(runtime, before_error);

    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(11, 20)])
        .unwrap();

    let reservation_rows = runtime
        .writeback_reservations()
        .iter()
        .map(|reservation| {
            (
                reservation.sequence(),
                reservation.raw_ready_tick(),
                reservation.admitted_tick(),
                reservation.slot(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(partition_after_publication, (vec![10], vec![12]));
    assert_eq!(
        reservation_rows,
        vec![(10, 20, 20, 0), (11, 20, 20, 1), (12, 20, 21, 0)]
    );
    assert_eq!(
        runtime
            .live_writeback_counted_sequences
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![11, 12]
    );
    assert_writeback_stats(&runtime, 2, 3, 1, 1, 3, 1);

    runtime.finalize_writeback_publication(11);
    assert_eq!(
        runtime
            .published_writeback_sequences
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![10, 11]
    );
    assert_eq!(
        runtime
            .live_writeback_counted_sequences
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![12]
    );

    runtime.discard_live_writeback_reservations();

    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| (reservation.sequence(), reservation.admitted_tick()))
            .collect::<Vec<_>>(),
        vec![(10, 20), (11, 20)]
    );
    assert_partial_writeback_ownership(&runtime, 1, 1, 0);
    assert_writeback_stats(&runtime, 1, 2, 0, 0, 2, 0);

    runtime.prune_writeback_calendar_before(21);
    assert_live_writeback_ownership(&runtime, 0);
    assert_writeback_stats(&runtime, 1, 2, 0, 0, 2, 0);
}

#[test]
fn repeated_publication_does_not_refinalize_the_same_sequence() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(2));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(10, 20),
            O3LiveWritebackReady::fixed_fu(12, 20),
        ])
        .unwrap();
    runtime.finalize_writeback_publication(10);
    let after_first_publication = runtime.clone();

    runtime.finalize_writeback_publication(10);

    assert_eq!(runtime, after_first_publication);
}

#[test]
fn prune_retains_an_unpublished_deferred_row_until_publication() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(10, 42),
            O3LiveWritebackReady::fixed_fu(11, 42),
        ])
        .unwrap();
    runtime.finalize_writeback_publication(10);
    runtime.finalize_writeback_publication(11);
    assert_partial_writeback_ownership(&runtime, 2, 1, 1);

    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(12, 42)])
        .unwrap();

    assert_writeback_stats(&runtime, 3, 3, 2, 3, 3, 2);
    runtime.prune_writeback_calendar_before(45);
    assert_eq!(
        runtime
            .writeback_reservation(12)
            .map(O3WritebackReservation::admitted_tick),
        Some(44)
    );
    assert_unpublished_live_writeback_rows(&runtime, &[12]);
    assert_writeback_stats(&runtime, 3, 3, 2, 3, 3, 2);

    runtime.finalize_writeback_publication(12);
    runtime.prune_writeback_calendar_before(45);
    assert_live_writeback_ownership(&runtime, 0);
}

#[test]
fn staged_lifecycle_discard_preserves_published_same_tick_occupancy() {
    assert_published_same_tick_occupancy_survives(|runtime| {
        runtime.discard_live_staged_instructions();
    });
}

#[test]
fn data_lifecycle_discard_preserves_published_same_tick_occupancy() {
    assert_published_same_tick_occupancy_survives(|runtime| {
        runtime.discard_live_data_access_lifecycle();
    });
}

#[test]
fn speculative_lifecycle_discard_preserves_published_same_tick_occupancy() {
    assert_published_same_tick_occupancy_survives(|runtime| {
        runtime.discard_live_speculative_executions();
    });
}

#[test]
fn staged_suffix_discard_preserves_published_same_tick_occupancy() {
    assert_published_same_tick_occupancy_survives(|runtime| {
        runtime.discard_live_staged_window_from(0);
    });
}

#[test]
fn unpublished_past_rows_remain_replan_open_across_stats_reset() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(20, 10),
            O3LiveWritebackReady::fixed_fu(21, 10),
        ])
        .unwrap();
    runtime.prune_writeback_calendar_before(11);

    let reentry = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(21, 10)])
        .unwrap();
    assert_eq!(reentry[0].admitted_tick(), 11);
    let late = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(22, 10)])
        .expect("an unpublished row keeps its raw-ready tick available for replanning");
    assert_eq!(late[0].admitted_tick(), 12);

    runtime.reset_stats();
    let after_reset = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(23, 10)])
        .expect("statistics reset must not close an unpublished raw-ready tick");
    assert_eq!(
        after_reset[0].admitted_tick(),
        13,
        "the retained calendar remains the collision authority after a statistics reset"
    );
    assert_eq!(runtime.writeback_reservations().len(), 4);
}

#[test]
fn quiescent_finalize_seals_retained_ticks_before_clearing_the_calendar() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(30, 20)])
        .unwrap();
    runtime.finalize_writeback_publication(30);

    runtime.finalize_all_writeback_reservations().unwrap();

    assert!(runtime.writeback_reservations().is_empty());
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
    let error = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(31, 20)])
        .expect_err("checkpoint finalization must seal every cleared calendar tick");
    assert_eq!(
        error,
        O3RuntimeError::WritebackReservationTickClosed {
            sequence: 31,
            raw_ready_tick: 20,
            closed_before_tick: 21,
        }
    );
}

#[test]
fn quiescent_finalize_at_max_tick_fails_without_mutating_state() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(40, u64::MAX)])
        .unwrap();
    let before = runtime.clone();

    let error = runtime.finalize_all_writeback_reservations().unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackClosureTickOverflow { tick: u64::MAX }
    );
    assert_eq!(runtime, before);
}

#[test]
fn writeback_live_ownership_is_bounded_across_prune_and_discard_cycles() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));

    for cycle in 0..128_u64 {
        let sequence = 10_000 + cycle;
        let raw_ready_tick = 100 + cycle * 2;
        runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
                sequence,
                raw_ready_tick,
            )])
            .unwrap();
        assert_live_writeback_ownership(&runtime, 1);
        runtime.finalize_writeback_publication(sequence);
        assert_eq!(runtime.writeback_reservations().len(), 1);
        assert!(runtime.live_writeback_counted_sequences.is_empty());
        assert_eq!(runtime.published_writeback_sequences.len(), 1);
        assert_partial_writeback_ownership(&runtime, 1, 1, 0);

        runtime.prune_writeback_calendar_before(raw_ready_tick + 1);
        assert_live_writeback_ownership(&runtime, 0);
    }

    let finalized = runtime.stats();
    assert_eq!(finalized.writeback_port_cycles(), 128);
    assert_eq!(finalized.writeback_port_admitted_rows(), 128);

    for cycle in 0..128_u64 {
        let sequence = 20_000 + cycle;
        let raw_ready_tick = 1_000 + cycle * 2;
        runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
                sequence,
                raw_ready_tick,
            )])
            .unwrap();
        assert_live_writeback_ownership(&runtime, 1);

        runtime.discard_future_writeback_sequence(sequence, raw_ready_tick - 1);
        assert_live_writeback_ownership(&runtime, 0);
        assert_eq!(runtime.stats(), finalized);
    }
}

#[test]
fn stats_reset_does_not_make_an_unpublished_calendar_row_prunable() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(3, 42)])
        .unwrap();
    runtime.finalize_writeback_publication(3);
    let reservations = runtime.writeback_reservations();
    assert_partial_writeback_ownership(&runtime, 1, 1, 0);

    runtime.reset_stats();

    assert_eq!(runtime.writeback_reservations(), reservations);
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
    assert_eq!(runtime.stats(), O3RuntimeStats::default());
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 42)])
        .unwrap();
    assert_writeback_stats(&runtime, 2, 1, 1, 1, 1, 1);
    runtime.prune_writeback_calendar_before(44);
    assert_eq!(
        runtime
            .writeback_reservation(4)
            .map(O3WritebackReservation::admitted_tick),
        Some(43)
    );
    assert_eq!(
        runtime.live_writeback_counted_sequences,
        BTreeSet::from([4])
    );
    assert_writeback_stats(&runtime, 2, 1, 1, 1, 1, 1);
}

#[test]
fn partial_prune_retains_the_full_unpublished_shared_live_ownership() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(10, 10),
            O3LiveWritebackReady::fixed_fu(11, 10),
            O3LiveWritebackReady::fixed_fu(12, 10),
        ])
        .unwrap();

    assert_writeback_stats(&runtime, 3, 3, 2, 3, 3, 2);
    runtime.prune_writeback_calendar_before(11);
    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| reservation.sequence())
            .collect::<Vec<_>>(),
        vec![10, 11, 12]
    );
    assert_unpublished_live_writeback_rows(&runtime, &[10, 11, 12]);
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
    assert_writeback_stats(&runtime, 3, 3, 2, 3, 3, 2);

    runtime.discard_future_writeback_from_sequence(11, 10);

    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| reservation.sequence())
            .collect::<Vec<_>>(),
        vec![10]
    );
    assert_unpublished_live_writeback_rows(&runtime, &[10]);
    assert_writeback_stats(&runtime, 1, 1, 0, 0, 1, 0);
}

#[test]
fn sequential_prune_keeps_ready_rows_from_the_same_raw_tick_live() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(40, 10),
            O3LiveWritebackReady::fixed_fu(41, 10),
            O3LiveWritebackReady::fixed_fu(42, 10),
        ])
        .unwrap();

    runtime.prune_writeback_calendar_before(11);
    assert_unpublished_live_writeback_rows(&runtime, &[40, 41, 42]);
    runtime.prune_writeback_calendar_before(13);

    assert_unpublished_live_writeback_rows(&runtime, &[40, 41, 42]);
    assert_writeback_stats(&runtime, 3, 3, 2, 3, 3, 2);
}

#[test]
fn sequential_prune_keeps_deferred_depth_from_the_same_cycle_live() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(50, 9),
            O3LiveWritebackReady::fixed_fu(51, 9),
            O3LiveWritebackReady::fixed_fu(52, 9),
        ])
        .unwrap();

    runtime.prune_writeback_calendar_before(10);
    assert_unpublished_live_writeback_rows(&runtime, &[50, 51, 52]);
    runtime.prune_writeback_calendar_before(11);
    assert_unpublished_live_writeback_rows(&runtime, &[50, 51, 52]);
    runtime.prune_writeback_calendar_before(12);

    assert_unpublished_live_writeback_rows(&runtime, &[50, 51, 52]);
    assert_eq!(runtime.stats().writeback_port_max_deferred_rows(), 2);
    assert_writeback_stats(&runtime, 3, 3, 2, 3, 3, 2);
}

#[test]
fn partial_prune_retains_all_unpublished_cycles_for_live_planning() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(20, 9),
            O3LiveWritebackReady::fixed_fu(21, 9),
            O3LiveWritebackReady::fixed_fu(22, 10),
        ])
        .unwrap();

    assert_writeback_stats(&runtime, 3, 3, 2, 2, 2, 1);
    runtime.prune_writeback_calendar_before(11);
    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| reservation.sequence())
            .collect::<Vec<_>>(),
        vec![20, 21, 22]
    );
    assert_unpublished_live_writeback_rows(&runtime, &[20, 21, 22]);
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
    assert_writeback_stats(&runtime, 3, 3, 2, 2, 2, 1);

    runtime.discard_future_writeback_sequence(22, 10);

    assert_unpublished_live_writeback_rows(&runtime, &[20, 21]);
    assert_writeback_stats(&runtime, 2, 2, 1, 1, 2, 1);
}

#[test]
fn drained_restore_seeds_finalized_additive_stats_across_live_discard() {
    let restored = O3RuntimeStats {
        writeback_port_cycles: 7,
        writeback_port_admitted_rows: 9,
        writeback_port_deferred_rows: 3,
        writeback_port_deferred_row_cycles: 5,
        writeback_port_max_ready_rows_per_cycle: 4,
        writeback_port_max_deferred_rows: 2,
        ..O3RuntimeStats::default()
    };
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        O3RuntimeState::default().snapshot(),
        restored,
    )
    .unwrap();
    let mut runtime = O3RuntimeState::default();
    runtime.restore_checkpoint_payload(payload).unwrap();
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
    assert!(runtime.set_writeback_width(1));

    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(30, 100)])
        .unwrap();
    assert_writeback_stats(&runtime, 8, 10, 3, 5, 4, 2);
    runtime.discard_future_writeback_sequence(30, 99);

    assert_eq!(runtime.stats(), restored);
    assert_live_writeback_ownership(&runtime, 0);
}

#[test]
fn drained_restore_clears_a_prior_writeback_closure_watermark() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(50, 10)])
        .unwrap();
    runtime.prune_writeback_calendar_before(11);
    let payload = O3RuntimeState::default().checkpoint_payload();

    runtime.restore_checkpoint_payload(payload).unwrap();

    let reservation = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(51, 10)])
        .unwrap();
    assert_eq!(reservation[0].admitted_tick(), 10);
}

#[test]
fn live_maxima_ownership_is_unchanged_by_transaction_error_after_prune() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(60, 10),
            O3LiveWritebackReady::fixed_fu(61, 10),
            O3LiveWritebackReady::fixed_fu(62, 10),
        ])
        .unwrap();
    runtime.prune_writeback_calendar_before(11);
    assert_unpublished_live_writeback_rows(&runtime, &[60, 61, 62]);
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
    let before = runtime.clone();

    let error = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(61, 10)])
        .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackReservationSourceMismatch {
            sequence: 61,
            existing_source: "FixedFu",
            requested_source: "MemoryResult",
        }
    );
    assert_eq!(runtime, before);
    assert_unpublished_live_writeback_rows(&runtime, &[60, 61, 62]);
    assert_partial_writeback_ownership(&runtime, 0, 0, 0);
}

fn assert_unpublished_live_writeback_rows(runtime: &O3RuntimeState, sequences: &[u64]) {
    assert_eq!(
        runtime
            .writeback_reservations()
            .iter()
            .map(|reservation| reservation.sequence())
            .collect::<Vec<_>>(),
        sequences
    );
    assert_eq!(
        runtime.live_writeback_counted_sequences,
        sequences.iter().copied().collect()
    );
    assert!(runtime.published_writeback_sequences.is_empty());
}

fn assert_live_writeback_ownership(runtime: &O3RuntimeState, live_rows: usize) {
    assert_eq!(runtime.writeback_reservations().len(), live_rows);
    assert!(runtime.published_writeback_sequences.len() <= runtime.writeback_reservations().len());
    assert_eq!(runtime.live_writeback_counted_sequences.len(), live_rows);
    let (live_cycle_ticks, live_ready_rows_by_tick) = runtime.live_writeback_schedule_debug();
    assert_eq!(live_cycle_ticks.len(), live_rows);
    assert_eq!(live_ready_rows_by_tick.len(), live_rows);
    assert_eq!(
        live_ready_rows_by_tick
            .values()
            .map(BTreeSet::len)
            .sum::<usize>(),
        live_rows
    );
    let (cycle_ticks, ready_ticks, deferred_ticks, bounded) =
        runtime.writeback_partial_ownership_debug();
    assert!(bounded);
    if live_rows == 0 {
        assert!(runtime.published_writeback_sequences.is_empty());
        assert_eq!((cycle_ticks, ready_ticks, deferred_ticks), (0, 0, 0));
    }
}

fn assert_partial_writeback_ownership(
    runtime: &O3RuntimeState,
    cycle_ticks: usize,
    ready_ticks: usize,
    deferred_ticks: usize,
) {
    assert_eq!(
        runtime.writeback_partial_ownership_debug(),
        (cycle_ticks, ready_ticks, deferred_ticks, true)
    );
}

fn assert_published_same_tick_occupancy_survives(cleanup: impl FnOnce(&mut O3RuntimeState)) {
    let (mut runtime, sequence, admitted_tick) = runtime_with_live_speculative_writeback();
    assert_eq!(
        runtime
            .writeback_reservation(sequence)
            .unwrap()
            .raw_ready_tick(),
        admitted_tick
    );
    runtime.finalize_writeback_publication(sequence);

    cleanup(&mut runtime);
    let partial_ownership = runtime.writeback_partial_ownership_debug();
    let retained = runtime
        .writeback_reservation(sequence)
        .map(O3WritebackReservation::admitted_tick);
    let replacement = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(0, admitted_tick)])
        .unwrap();

    assert_eq!(
        (retained, replacement[0].admitted_tick(), partial_ownership),
        (Some(admitted_tick), admitted_tick + 1, (1, 1, 0, true))
    );
}

fn assert_writeback_stats(
    runtime: &O3RuntimeState,
    cycles: u64,
    admitted_rows: u64,
    deferred_rows: u64,
    deferred_row_cycles: u64,
    max_ready_rows_per_cycle: u64,
    max_deferred_rows: u64,
) {
    let stats = runtime.stats();
    assert_eq!(stats.writeback_port_cycles(), cycles);
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
