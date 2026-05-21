use rem6_cpu::{BranchPredictor, BranchPredictorConfig, BranchPredictorError};
use rem6_memory::Address;

fn predictor(entries: usize) -> BranchPredictor {
    BranchPredictor::new(BranchPredictorConfig::new(entries).unwrap())
}

#[test]
fn two_bit_predictor_learns_taken_target() {
    let mut predictor = predictor(8);
    let pc = Address::new(0x1000);
    let target = Address::new(0x1080);

    let first = predictor.predict(pc);

    assert_eq!(first.pc(), pc);
    assert_eq!(first.index(), 0);
    assert!(!first.predicted_taken());
    assert_eq!(first.target(), None);
    assert_eq!(first.counter(), 1);

    let update = predictor.update(pc, true, Some(target));

    assert_eq!(update.pc(), pc);
    assert_eq!(update.index(), 0);
    assert!(!update.predicted_taken());
    assert!(update.actual_taken());
    assert_eq!(update.actual_target(), Some(target));
    assert_eq!(update.old_counter(), 1);
    assert_eq!(update.new_counter(), 2);
    assert_eq!(update.update_count(), 1);

    let second = predictor.predict(pc);

    assert!(second.predicted_taken());
    assert_eq!(second.target(), Some(target));
    assert_eq!(second.counter(), 2);

    let redirected = Address::new(0x1090);
    let second_update = predictor.update(pc, true, Some(redirected));

    assert!(second_update.predicted_taken());
    assert_eq!(second_update.new_counter(), 3);
    assert_eq!(predictor.predict(pc).target(), Some(redirected));
}

#[test]
fn snapshot_restore_preserves_counters_targets_and_update_count() {
    let mut predictor = predictor(8);
    let loop_pc = Address::new(0x1000);
    let call_pc = Address::new(0x1004);
    let loop_target = Address::new(0x0ff0);
    let call_target = Address::new(0x2000);

    predictor.update(loop_pc, true, Some(loop_target));
    predictor.update(call_pc, true, Some(call_target));
    predictor.update(call_pc, true, Some(call_target));

    let snapshot = predictor.snapshot();
    assert_eq!(snapshot.update_count(), 3);

    predictor.update(loop_pc, false, None);
    predictor.update(loop_pc, false, None);
    predictor.update(call_pc, false, None);
    assert!(!predictor.predict(loop_pc).predicted_taken());

    predictor.restore(&snapshot).unwrap();

    let loop_prediction = predictor.predict(loop_pc);
    assert!(loop_prediction.predicted_taken());
    assert_eq!(loop_prediction.target(), Some(loop_target));
    assert_eq!(loop_prediction.counter(), 2);

    let call_prediction = predictor.predict(call_pc);
    assert!(call_prediction.predicted_taken());
    assert_eq!(call_prediction.target(), Some(call_target));
    assert_eq!(call_prediction.counter(), 3);
    assert_eq!(predictor.update_count(), 3);
}

#[test]
fn restore_rejects_snapshot_with_different_table_size() {
    let snapshot = predictor(8).snapshot();
    let mut smaller = predictor(4);

    let error = smaller.restore(&snapshot).unwrap_err();

    assert_eq!(
        error,
        BranchPredictorError::SnapshotTableEntriesMismatch {
            expected: 4,
            actual: 8,
        }
    );
}

#[test]
fn config_rejects_empty_table() {
    assert_eq!(
        BranchPredictorConfig::new(0),
        Err(BranchPredictorError::ZeroTableEntries)
    );
}
