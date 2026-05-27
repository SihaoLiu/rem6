use rem6_cpu::{
    BiModeBranchPredictor, BiModeBranchPredictorConfig, BiModeBranchPredictorError,
    BiModeDirectionArray, CpuId,
};
use rem6_memory::Address;

fn bimode(
    threads: usize,
    choice_entries: usize,
    global_entries: usize,
    counter_bits: u8,
) -> BiModeBranchPredictor {
    BiModeBranchPredictor::new(
        BiModeBranchPredictorConfig::with_options(
            threads,
            choice_entries,
            global_entries,
            counter_bits,
            counter_bits,
            2,
        )
        .unwrap(),
    )
}

#[test]
fn bimode_predictor_uses_pc_choice_and_pc_xor_history_indexes() {
    let mut predictor = bimode(2, 4, 8, 2);
    let cpu0 = CpuId::new(0);
    let cpu1 = CpuId::new(1);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu0, pc).unwrap();

    assert_eq!(first.cpu(), cpu0);
    assert_eq!(first.pc(), pc);
    assert_eq!(first.choice_index(), 0);
    assert_eq!(first.global_index(), 0);
    assert_eq!(first.global_history_before(), 0);
    assert_eq!(first.selected_array(), BiModeDirectionArray::NotTaken);
    assert!(!first.predicted_taken());

    let history_update = predictor.update_history(first.history(), true).unwrap();

    assert_eq!(history_update.old_history(), 0);
    assert_eq!(history_update.new_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);
    assert_eq!(predictor.snapshot().threads()[1].global_history(), 0);

    let second = predictor.predict(cpu0, pc).unwrap();
    assert_eq!(second.global_index(), 1);
    assert_eq!(second.global_history_before(), 1);

    let other_cpu = predictor.predict(cpu1, pc).unwrap();
    assert_eq!(other_cpu.global_index(), 0);
    assert_eq!(other_cpu.global_history_before(), 0);
}

#[test]
fn bimode_predictor_updates_selected_direction_and_choice_counters() {
    let mut predictor = bimode(1, 4, 8, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    for expected_choice in 1..=2 {
        let prediction = predictor.predict(cpu, pc).unwrap();
        let update = predictor.train(prediction.history(), true, false).unwrap();

        assert_eq!(update.choice_index(), 0);
        assert_eq!(update.global_index(), 0);
        assert_eq!(update.selected_array(), BiModeDirectionArray::NotTaken);
        assert_eq!(update.new_choice_counter(), expected_choice);
        assert_eq!(update.new_not_taken_counter(), expected_choice);
        assert_eq!(update.new_taken_counter(), 0);
    }

    let uses_taken_array = predictor.predict(cpu, pc).unwrap();
    assert_eq!(
        uses_taken_array.selected_array(),
        BiModeDirectionArray::Taken
    );
    assert!(!uses_taken_array.predicted_taken());

    let first_taken_update = predictor
        .train(uses_taken_array.history(), true, false)
        .unwrap();
    assert_eq!(first_taken_update.new_choice_counter(), 3);
    assert_eq!(first_taken_update.new_taken_counter(), 1);
    assert_eq!(first_taken_update.new_not_taken_counter(), 2);

    let second_taken = predictor.predict(cpu, pc).unwrap();
    predictor
        .train(second_taken.history(), true, false)
        .unwrap();

    let trained = predictor.predict(cpu, pc).unwrap();
    assert_eq!(trained.selected_array(), BiModeDirectionArray::Taken);
    assert!(trained.predicted_taken());
    assert_eq!(trained.taken_counter(), 2);
}

#[test]
fn bimode_predictor_squash_repairs_history_without_training_counters() {
    let mut predictor = bimode(1, 4, 8, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let prediction = predictor.predict(cpu, pc).unwrap();
    predictor
        .update_history(prediction.history(), true)
        .unwrap();
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);

    let repair = predictor.train(prediction.history(), false, true).unwrap();

    assert!(repair.squashed());
    assert_eq!(repair.repaired_history(), Some(0));
    assert_eq!(repair.old_choice_counter(), 0);
    assert_eq!(repair.new_choice_counter(), 0);
    assert_eq!(repair.old_not_taken_counter(), 0);
    assert_eq!(repair.new_not_taken_counter(), 0);
    assert_eq!(repair.update_count(), 0);
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 0);
}

#[test]
fn bimode_predictor_rejects_stale_history_update_without_mutating_thread() {
    let mut predictor = bimode(1, 4, 8, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu, pc).unwrap();
    predictor.update_history(first.history(), true).unwrap();

    assert_eq!(
        predictor
            .update_history(first.history(), false)
            .unwrap_err(),
        BiModeBranchPredictorError::HistoryUpdateOutOfOrder {
            cpu,
            expected_history: 1,
            actual_history: 0,
        },
    );
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);
    assert_eq!(predictor.history_update_count(), 1);

    let current = predictor.predict(cpu, pc).unwrap();
    let update = predictor.update_history(current.history(), false).unwrap();
    assert_eq!(update.old_history(), 1);
    assert_eq!(update.new_history(), 2);
    assert_eq!(predictor.history_update_count(), 2);
}

#[test]
fn bimode_predictor_snapshot_restore_preserves_threads_counters_and_counts() {
    let mut predictor = bimode(2, 4, 8, 2);
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
        predictor.snapshot().choice_counters(),
        snapshot.choice_counters()
    );
    assert_eq!(
        predictor.snapshot().taken_counters(),
        snapshot.taken_counters()
    );
    assert_eq!(
        predictor.snapshot().not_taken_counters(),
        snapshot.not_taken_counters()
    );
    assert_eq!(predictor.snapshot().threads(), snapshot.threads());
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn bimode_predictor_rejects_bad_config_thread_and_snapshot_shape() {
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(0, 4, 8, 2, 2, 2),
        Err(BiModeBranchPredictorError::ZeroThreads)
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 0, 8, 2, 2, 2),
        Err(BiModeBranchPredictorError::ZeroChoiceEntries)
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 3, 8, 2, 2, 2),
        Err(BiModeBranchPredictorError::ChoiceEntriesNotPowerOfTwo { entries: 3 })
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 4, 0, 2, 2, 2),
        Err(BiModeBranchPredictorError::ZeroGlobalEntries)
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 4, 6, 2, 2, 2),
        Err(BiModeBranchPredictorError::GlobalEntriesNotPowerOfTwo { entries: 6 })
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 4, 8, 0, 2, 2),
        Err(BiModeBranchPredictorError::ChoiceCounterBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 4, 8, 2, 9, 2),
        Err(BiModeBranchPredictorError::GlobalCounterBitsOutOfRange { bits: 9 })
    );
    assert_eq!(
        BiModeBranchPredictorConfig::with_options(1, 4, 8, 2, 2, 64),
        Err(BiModeBranchPredictorError::InstShiftOutOfRange { bits: 64 })
    );

    let mut predictor = bimode(1, 4, 8, 2);
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x1000)),
        Err(BiModeBranchPredictorError::UnknownThread { cpu: CpuId::new(2) })
    );

    let snapshot = bimode(1, 4, 8, 2).snapshot();
    let mut different_shape = bimode(1, 8, 8, 2);

    assert_eq!(
        different_shape.restore(&snapshot),
        Err(BiModeBranchPredictorError::SnapshotShapeMismatch {
            expected_threads: 1,
            actual_threads: 1,
            expected_choice_entries: 8,
            actual_choice_entries: 4,
            expected_global_entries: 8,
            actual_global_entries: 8,
        })
    );
}
