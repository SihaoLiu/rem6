use rem6_cpu::{
    CpuId, GShareBranchPredictor, GShareBranchPredictorConfig, GShareBranchPredictorError,
};
use rem6_memory::Address;

fn gshare(threads: usize, entries: usize, counter_bits: u8) -> GShareBranchPredictor {
    GShareBranchPredictor::new(
        GShareBranchPredictorConfig::with_options(threads, entries, counter_bits, 2).unwrap(),
    )
}

#[test]
fn gshare_predictor_hashes_pc_with_per_cpu_global_history() {
    let mut predictor = gshare(2, 8, 2);
    let cpu0 = CpuId::new(0);
    let cpu1 = CpuId::new(1);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu0, pc).unwrap();

    assert_eq!(first.cpu(), cpu0);
    assert_eq!(first.pc(), pc);
    assert_eq!(first.index(), 0);
    assert_eq!(first.global_history_before(), 0);
    assert_eq!(first.counter(), 0);
    assert!(!first.predicted_taken());

    let history_update = predictor.update_history(first.history(), true).unwrap();

    assert_eq!(history_update.old_history(), 0);
    assert_eq!(history_update.new_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);
    assert_eq!(predictor.snapshot().threads()[1].global_history(), 0);

    let second = predictor.predict(cpu0, pc).unwrap();
    assert_eq!(second.index(), 1);
    assert_eq!(second.global_history_before(), 1);

    let other_cpu = predictor.predict(cpu1, pc).unwrap();
    assert_eq!(other_cpu.index(), 0);
    assert_eq!(other_cpu.global_history_before(), 0);
}

#[test]
fn gshare_predictor_trains_saturating_counter_using_recorded_history_index() {
    let mut predictor = gshare(1, 8, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu, pc).unwrap();
    let first_update = predictor.train(first.history(), true, false).unwrap();

    assert_eq!(first_update.index(), 0);
    assert_eq!(first_update.old_counter(), 0);
    assert_eq!(first_update.new_counter(), 1);
    assert_eq!(first_update.update_count(), 1);
    assert!(!first_update.squashed());

    let second = predictor.predict(cpu, pc).unwrap();
    let second_update = predictor.train(second.history(), true, false).unwrap();

    assert_eq!(second_update.old_counter(), 1);
    assert_eq!(second_update.new_counter(), 2);
    assert!(predictor.predict(cpu, pc).unwrap().predicted_taken());

    let taken_prediction = predictor.predict(cpu, pc).unwrap();
    predictor
        .train(taken_prediction.history(), false, false)
        .unwrap();
    let weak_taken = predictor.predict(cpu, pc).unwrap();
    assert!(!weak_taken.predicted_taken());
    assert_eq!(weak_taken.counter(), 1);

    predictor.train(weak_taken.history(), false, false).unwrap();
    assert_eq!(predictor.predict(cpu, pc).unwrap().counter(), 0);
}

#[test]
fn gshare_predictor_supports_eight_bit_saturating_counters() {
    let mut predictor = gshare(1, 8, 8);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    for expected_counter in 1..=u8::MAX {
        let prediction = predictor.predict(cpu, pc).unwrap();
        let update = predictor.train(prediction.history(), true, false).unwrap();
        assert_eq!(update.new_counter(), expected_counter);
    }

    let saturated = predictor.predict(cpu, pc).unwrap();
    let update = predictor.train(saturated.history(), true, false).unwrap();

    assert_eq!(update.old_counter(), u8::MAX);
    assert_eq!(update.new_counter(), u8::MAX);
    assert!(predictor.predict(cpu, pc).unwrap().predicted_taken());
}

#[test]
fn gshare_predictor_squash_repairs_history_without_training_counter() {
    let mut predictor = gshare(1, 8, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let prediction = predictor.predict(cpu, pc).unwrap();
    predictor
        .update_history(prediction.history(), true)
        .unwrap();
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 1);

    let repair = predictor.train(prediction.history(), false, true).unwrap();

    assert!(repair.squashed());
    assert_eq!(repair.old_counter(), 0);
    assert_eq!(repair.new_counter(), 0);
    assert_eq!(repair.repaired_history(), Some(0));
    assert_eq!(repair.update_count(), 0);
    assert_eq!(predictor.snapshot().threads()[0].global_history(), 0);
    assert_eq!(predictor.predict(cpu, pc).unwrap().counter(), 0);
}

#[test]
fn gshare_predictor_rejects_stale_history_update_without_mutating_thread() {
    let mut predictor = gshare(1, 8, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);

    let first = predictor.predict(cpu, pc).unwrap();
    predictor.update_history(first.history(), true).unwrap();

    assert_eq!(
        predictor
            .update_history(first.history(), false)
            .unwrap_err(),
        GShareBranchPredictorError::HistoryUpdateOutOfOrder {
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
fn gshare_predictor_snapshot_restore_preserves_threads_counters_and_counts() {
    let mut predictor = gshare(2, 8, 2);
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

    assert_eq!(predictor.snapshot().counters(), snapshot.counters());
    assert_eq!(predictor.snapshot().threads(), snapshot.threads());
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn gshare_predictor_rejects_bad_config_thread_and_snapshot_shape() {
    assert_eq!(
        GShareBranchPredictorConfig::with_options(0, 8, 2, 2),
        Err(GShareBranchPredictorError::ZeroThreads)
    );
    assert_eq!(
        GShareBranchPredictorConfig::with_options(1, 0, 2, 2),
        Err(GShareBranchPredictorError::ZeroTableEntries)
    );
    assert_eq!(
        GShareBranchPredictorConfig::with_options(1, 7, 2, 2),
        Err(GShareBranchPredictorError::TableEntriesNotPowerOfTwo { entries: 7 })
    );
    assert_eq!(
        GShareBranchPredictorConfig::with_options(1, 8, 0, 2),
        Err(GShareBranchPredictorError::CounterBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        GShareBranchPredictorConfig::with_options(1, 8, 9, 2),
        Err(GShareBranchPredictorError::CounterBitsOutOfRange { bits: 9 })
    );
    assert_eq!(
        GShareBranchPredictorConfig::with_options(1, 8, 2, 64),
        Err(GShareBranchPredictorError::InstShiftOutOfRange { bits: 64 })
    );

    let mut predictor = gshare(1, 8, 2);
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x1000)),
        Err(GShareBranchPredictorError::UnknownThread { cpu: CpuId::new(2) })
    );

    let snapshot = gshare(1, 8, 2).snapshot();
    let mut different_entries = gshare(1, 16, 2);

    assert_eq!(
        different_entries.restore(&snapshot),
        Err(GShareBranchPredictorError::SnapshotShapeMismatch {
            expected_threads: 1,
            actual_threads: 1,
            expected_entries: 16,
            actual_entries: 8,
        })
    );
}
