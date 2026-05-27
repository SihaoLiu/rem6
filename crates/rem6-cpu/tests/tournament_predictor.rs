use rem6_cpu::{
    CpuId, TournamentBranchPredictor, TournamentBranchPredictorConfig,
    TournamentBranchPredictorError, TournamentPredictorSelection,
};
use rem6_memory::Address;

fn tournament(
    threads: usize,
    local_entries: usize,
    local_history_entries: usize,
    global_entries: usize,
    choice_entries: usize,
    counter_bits: u8,
) -> TournamentBranchPredictor {
    TournamentBranchPredictor::new(
        TournamentBranchPredictorConfig::with_options(
            threads,
            local_entries,
            local_history_entries,
            global_entries,
            choice_entries,
            counter_bits,
            counter_bits,
            counter_bits,
            2,
        )
        .unwrap(),
    )
}

#[test]
fn tournament_predictor_uses_shared_local_history_and_per_cpu_global_history() {
    let mut predictor = tournament(2, 8, 4, 8, 4, 2);
    let cpu0 = CpuId::new(0);
    let cpu1 = CpuId::new(1);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu0, pc).unwrap();

    assert_eq!(first.cpu(), cpu0);
    assert_eq!(first.pc(), pc);
    assert_eq!(first.local_history_index(), 0);
    assert_eq!(first.local_predictor_index(), 0);
    assert_eq!(first.global_index(), 0);
    assert_eq!(first.choice_index(), 0);
    assert_eq!(first.local_history_before(), 0);
    assert_eq!(first.global_history_before(), 0);
    assert_eq!(first.selection(), TournamentPredictorSelection::Local);
    assert!(!first.local_predicted_taken());
    assert!(!first.global_predicted_taken());
    assert!(!first.predicted_taken());

    let history_update = predictor.update_history(first.history(), true).unwrap();

    assert_eq!(history_update.old_global_history(), 0);
    assert_eq!(history_update.new_global_history(), 1);
    assert_eq!(history_update.old_local_history(), 0);
    assert_eq!(history_update.new_local_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);
    assert_eq!(predictor.snapshot().threads()[1].global_history(), 0);
    assert_eq!(predictor.snapshot().local_history_table()[0], 1);

    let second = predictor.predict(cpu0, pc).unwrap();
    assert_eq!(second.local_history_index(), 0);
    assert_eq!(second.local_predictor_index(), 1);
    assert_eq!(second.global_index(), 1);
    assert_eq!(second.choice_index(), 1);
    assert_eq!(second.global_history_before(), 1);

    let other_cpu = predictor.predict(cpu1, pc).unwrap();
    assert_eq!(other_cpu.local_history_index(), 0);
    assert_eq!(other_cpu.local_predictor_index(), 1);
    assert_eq!(other_cpu.global_index(), 0);
    assert_eq!(other_cpu.choice_index(), 0);
    assert_eq!(other_cpu.global_history_before(), 0);
}

#[test]
fn tournament_predictor_updates_local_global_and_choice_on_disagreement() {
    let mut predictor = tournament(2, 8, 4, 8, 4, 2);
    let cpu0 = CpuId::new(0);
    let cpu1 = CpuId::new(1);
    let shared_pc = Address::new(0x1000);
    let seed_pc = Address::new(0x1004);

    let seed = predictor.predict(cpu0, seed_pc).unwrap();
    predictor.update_history(seed.history(), true).unwrap();

    for expected_counter in 1..=2 {
        let prediction = predictor.predict(cpu0, shared_pc).unwrap();

        assert_eq!(prediction.local_history_index(), 0);
        assert_eq!(prediction.local_predictor_index(), 0);
        assert_eq!(prediction.global_index(), 1);

        let update = predictor.train(prediction.history(), true, false).unwrap();

        assert_eq!(update.old_choice_counter(), 0);
        assert_eq!(update.new_choice_counter(), 0);
        assert_eq!(update.new_local_counter(), expected_counter);
        assert_eq!(update.new_global_counter(), expected_counter);
    }

    let disagreement = predictor.predict(cpu1, shared_pc).unwrap();
    assert_eq!(disagreement.local_predictor_index(), 0);
    assert_eq!(disagreement.global_index(), 0);
    assert_eq!(disagreement.choice_index(), 0);
    assert_eq!(
        disagreement.selection(),
        TournamentPredictorSelection::Local
    );
    assert!(disagreement.local_predicted_taken());
    assert!(!disagreement.global_predicted_taken());
    assert!(disagreement.predicted_taken());

    let update = predictor
        .train(disagreement.history(), false, false)
        .unwrap();

    assert_eq!(update.local_predictor_index(), 0);
    assert_eq!(update.global_index(), 0);
    assert_eq!(update.choice_index(), 0);
    assert_eq!(update.old_choice_counter(), 0);
    assert_eq!(update.new_choice_counter(), 1);
    assert_eq!(update.old_local_counter(), 2);
    assert_eq!(update.new_local_counter(), 1);
    assert_eq!(update.old_global_counter(), 0);
    assert_eq!(update.new_global_counter(), 0);
    assert_eq!(update.update_count(), 3);
}

#[test]
fn tournament_predictor_squash_repairs_histories_without_training_counters() {
    let mut predictor = tournament(1, 8, 4, 8, 4, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let prediction = predictor.predict(cpu, pc).unwrap();
    predictor
        .update_history(prediction.history(), true)
        .unwrap();
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);
    assert_eq!(predictor.snapshot().local_history_table()[0], 1);

    let repair = predictor.train(prediction.history(), false, true).unwrap();

    assert!(repair.squashed());
    assert_eq!(repair.repaired_global_history(), Some(0));
    assert_eq!(repair.repaired_local_history(), Some(0));
    assert_eq!(repair.old_choice_counter(), 0);
    assert_eq!(repair.new_choice_counter(), 0);
    assert_eq!(repair.old_local_counter(), 0);
    assert_eq!(repair.new_local_counter(), 0);
    assert_eq!(repair.old_global_counter(), 0);
    assert_eq!(repair.new_global_counter(), 0);
    assert_eq!(repair.update_count(), 0);
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 0);
    assert_eq!(predictor.snapshot().local_history_table()[0], 0);
}

#[test]
fn tournament_predictor_rejects_stale_history_update_without_mutating_histories() {
    let mut predictor = tournament(1, 8, 4, 8, 4, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu, pc).unwrap();
    predictor.update_history(first.history(), true).unwrap();

    assert_eq!(
        predictor
            .update_history(first.history(), false)
            .unwrap_err(),
        TournamentBranchPredictorError::HistoryUpdateOutOfOrder {
            cpu,
            expected_global_history: 1,
            actual_global_history: 0,
            expected_local_history: Some(1),
            actual_local_history: Some(0),
        },
    );
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);
    assert_eq!(predictor.snapshot().local_history_table()[0], 1);
    assert_eq!(predictor.history_update_count(), 1);

    let current = predictor.predict(cpu, pc).unwrap();
    let update = predictor.update_history(current.history(), false).unwrap();
    assert_eq!(update.old_global_history(), 1);
    assert_eq!(update.new_global_history(), 2);
    assert_eq!(update.old_local_history(), 1);
    assert_eq!(update.new_local_history(), 2);
    assert_eq!(predictor.history_update_count(), 2);
}

#[test]
fn tournament_predictor_snapshot_restore_preserves_tables_histories_and_counts() {
    let mut predictor = tournament(2, 8, 4, 8, 4, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let prediction = predictor.predict(cpu, pc).unwrap();
    predictor
        .update_history(prediction.history(), true)
        .unwrap();
    predictor.train(prediction.history(), true, false).unwrap();
    let snapshot = predictor.snapshot();

    let diverged = predictor.predict(cpu, pc).unwrap();
    predictor.update_history(diverged.history(), false).unwrap();
    predictor.train(diverged.history(), false, false).unwrap();

    predictor.restore(&snapshot).unwrap();

    assert_eq!(
        predictor.snapshot().local_counters(),
        snapshot.local_counters()
    );
    assert_eq!(
        predictor.snapshot().local_history_table(),
        snapshot.local_history_table()
    );
    assert_eq!(
        predictor.snapshot().global_counters(),
        snapshot.global_counters()
    );
    assert_eq!(
        predictor.snapshot().choice_counters(),
        snapshot.choice_counters()
    );
    assert_eq!(predictor.snapshot().threads(), snapshot.threads());
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(
        predictor.history_update_count(),
        snapshot.history_update_count()
    );
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn tournament_predictor_rejects_bad_config_thread_and_snapshot_shape() {
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(0, 8, 4, 8, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::ZeroThreads)
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 0, 4, 8, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::ZeroLocalEntries)
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 7, 4, 8, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::LocalEntriesNotPowerOfTwo { entries: 7 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 0, 8, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::ZeroLocalHistoryEntries)
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 6, 8, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::LocalHistoryEntriesNotPowerOfTwo { entries: 6 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 0, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::ZeroGlobalEntries)
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 6, 4, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::GlobalEntriesNotPowerOfTwo { entries: 6 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 0, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::ZeroChoiceEntries)
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 6, 2, 2, 2, 2),
        Err(TournamentBranchPredictorError::ChoiceEntriesNotPowerOfTwo { entries: 6 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 4, 0, 2, 2, 2),
        Err(TournamentBranchPredictorError::LocalCounterBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 4, 2, 9, 2, 2),
        Err(TournamentBranchPredictorError::GlobalCounterBitsOutOfRange { bits: 9 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 4, 2, 2, 0, 2),
        Err(TournamentBranchPredictorError::ChoiceCounterBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 4, 2, 2, 2, 64),
        Err(TournamentBranchPredictorError::InstShiftOutOfRange { bits: 64 })
    );

    let mut predictor = tournament(1, 8, 4, 8, 4, 2);
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x1000)),
        Err(TournamentBranchPredictorError::UnknownThread { cpu: CpuId::new(2) })
    );

    let snapshot = tournament(1, 8, 4, 8, 4, 2).snapshot();
    let mut different_shape = tournament(1, 16, 4, 8, 4, 2);

    assert_eq!(
        different_shape.restore(&snapshot),
        Err(TournamentBranchPredictorError::SnapshotShapeMismatch {
            expected_threads: 1,
            actual_threads: 1,
            expected_local_entries: 16,
            actual_local_entries: 8,
            expected_local_history_entries: 4,
            actual_local_history_entries: 4,
            expected_global_entries: 8,
            actual_global_entries: 8,
            expected_choice_entries: 4,
            actual_choice_entries: 4,
        })
    );

    let expected_config =
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 4, 3, 2, 2, 2).unwrap();
    let actual_config =
        TournamentBranchPredictorConfig::with_options(1, 8, 4, 8, 4, 2, 2, 2, 2).unwrap();
    let snapshot = TournamentBranchPredictor::new(actual_config.clone()).snapshot();
    let mut different_config = TournamentBranchPredictor::new(expected_config.clone());

    assert_eq!(
        different_config.restore(&snapshot),
        Err(TournamentBranchPredictorError::SnapshotConfigMismatch {
            expected: expected_config,
            actual: actual_config,
        })
    );
}
