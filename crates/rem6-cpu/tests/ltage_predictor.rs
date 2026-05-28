use rem6_cpu::{
    CpuId, LTageBranchPredictor, LTageBranchPredictorConfig, LTageBranchPredictorError,
    LTageProvider, LoopBranchPredictor, LoopBranchPredictorConfig, TageBranchPredictorConfig,
    TageBranchPredictorError, TageProvider,
};
use rem6_memory::Address;

fn tage_config(speculative_history: bool) -> TageBranchPredictorConfig {
    TageBranchPredictorConfig::with_options(
        2,
        2,
        2,
        6,
        vec![0, 4, 5],
        vec![4, 3, 3],
        1,
        3,
        2,
        8,
        4,
        1,
        4,
        1,
        2,
        false,
        speculative_history,
    )
    .unwrap()
}

fn loop_config(use_speculation: bool) -> LoopBranchPredictorConfig {
    LoopBranchPredictorConfig::with_options(
        2,     // threads
        3,     // log table entries
        1,     // log associativity
        3,     // age bits
        2,     // confidence bits
        4,     // tag bits
        4,     // iteration bits
        3,     // loop-use counter bits
        2,     // instruction shift
        false, // direction bit
        use_speculation,
        false, // hashing
        false, // restricted allocation
        1,     // initial iteration
        3,     // initial age
        true,  // optional age reset
    )
    .unwrap()
}

fn ltage(speculative_tage_history: bool, speculative_loop: bool) -> LTageBranchPredictor {
    LTageBranchPredictor::new(
        LTageBranchPredictorConfig::new(
            tage_config(speculative_tage_history),
            loop_config(speculative_loop),
        )
        .unwrap(),
    )
}

fn train_trip_count_loop(predictor: &mut LoopBranchPredictor, pc: Address, cpu: CpuId) {
    let first = predictor.predict(cpu, pc, true, false).unwrap();
    predictor.train(first.history(), true).unwrap();

    for taken in [true, false] {
        let prediction = predictor.predict(cpu, pc, true, false).unwrap();
        predictor.train(prediction.history(), taken).unwrap();
    }

    for _ in 0..3 {
        for taken in [true, true, false] {
            let prediction = predictor.predict(cpu, pc, true, false).unwrap();
            predictor.train(prediction.history(), taken).unwrap();
        }
    }
}

fn train_loop_until_exit_override(predictor: &mut LoopBranchPredictor, pc: Address, cpu: CpuId) {
    train_trip_count_loop(predictor, pc, cpu);

    let first_trusted = predictor.predict(cpu, pc, true, false).unwrap();
    assert!(first_trusted.loop_prediction_valid());
    assert!(!first_trusted.loop_prediction_used());
    predictor.train(first_trusted.history(), true).unwrap();

    let body = predictor.predict(cpu, pc, true, false).unwrap();
    assert!(body.loop_prediction_used());
    assert!(body.loop_predicted_taken());
    predictor.train(body.history(), true).unwrap();
}

#[test]
fn ltage_uses_tage_when_loop_prediction_is_not_trusted() {
    let mut predictor = ltage(false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    predictor
        .tage_mut()
        .write_tagged_entry(2, 5, 17, 2, 1)
        .unwrap();
    predictor
        .tage_mut()
        .write_bimodal_entry(1, false, true)
        .unwrap();

    let prediction = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(prediction.cpu(), cpu);
    assert_eq!(prediction.pc(), pc);
    assert_eq!(
        prediction.provider(),
        LTageProvider::Tage(TageProvider::TageLongestMatch)
    );
    assert!(prediction.tage_prediction().predicted_taken());
    assert!(!prediction.loop_prediction().loop_prediction_used());
    assert!(prediction.predicted_taken());
}

#[test]
fn ltage_loop_predictor_overrides_tage_when_trusted() {
    let mut predictor = ltage(false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    predictor
        .tage_mut()
        .write_bimodal_entry(4, true, true)
        .unwrap();
    train_loop_until_exit_override(predictor.loop_predictor_mut(), pc, cpu);

    let prediction = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(prediction.provider(), LTageProvider::Loop);
    assert!(prediction.tage_prediction().predicted_taken());
    assert!(prediction.loop_prediction().loop_prediction_used());
    assert!(!prediction.loop_prediction().loop_predicted_taken());
    assert!(!prediction.predicted_taken());
}

#[test]
fn ltage_train_updates_loop_tage_and_history_in_gem5_order() {
    let mut predictor = ltage(false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    let update = predictor
        .train(prediction.history(), true, Address::new(0x80))
        .unwrap();

    assert_eq!(update.loop_update().allocated_index(), Some(0));
    assert_eq!(update.tage_update().allocated_entries().len(), 1);
    assert_eq!(update.history_update().old_global_history(), 0);
    assert_eq!(update.history_update().new_global_history(), 1);
    assert_eq!(predictor.loop_predictor().update_count(), 1);
    assert_eq!(predictor.tage().update_count(), 1);
    assert_eq!(predictor.tage().history_update_count(), 1);
}

#[test]
fn ltage_stale_train_rejects_without_partial_inner_mutation() {
    let mut predictor = ltage(false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(prediction.history(), true, Address::new(0x80))
        .unwrap();
    let snapshot = predictor.snapshot();

    assert_eq!(
        predictor
            .train(prediction.history(), false, Address::new(0))
            .unwrap_err(),
        LTageBranchPredictorError::Tage(TageBranchPredictorError::HistoryUpdateOutOfOrder {
            cpu,
            expected_path_history: 0,
            actual_path_history: 0,
            expected_global_history: 1,
            actual_global_history: 0,
        }),
    );
    assert_eq!(predictor.snapshot(), snapshot);
}

#[test]
fn ltage_repair_restores_loop_speculation_and_tage_history() {
    let mut predictor = ltage(true, true);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    predictor
        .tage_mut()
        .write_bimodal_entry(4, true, true)
        .unwrap();
    train_loop_until_exit_override(predictor.loop_predictor_mut(), pc, cpu);

    assert_eq!(
        predictor.loop_predictor().snapshot().entries()[0].current_iter(),
        2
    );
    assert_eq!(
        predictor.loop_predictor().snapshot().entries()[0].current_iter_spec(),
        2
    );

    let prediction = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(
        predictor.loop_predictor().snapshot().entries()[0].current_iter_spec(),
        3
    );

    let repair = predictor
        .repair(prediction.history(), false, Address::new(0))
        .unwrap();

    assert_eq!(repair.loop_squash().restored_current_iter_spec(), Some(2));
    assert_eq!(repair.history_update().old_global_history(), 0);
    assert_eq!(repair.history_update().new_global_history(), 0);
    assert_eq!(
        predictor.loop_predictor().snapshot().entries()[0].current_iter_spec(),
        2
    );
}

#[test]
fn ltage_snapshot_restore_preserves_inner_predictors_and_counts() {
    let mut predictor = ltage(false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(prediction.history(), true, Address::new(0x80))
        .unwrap();
    let snapshot = predictor.snapshot();

    let diverged = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(diverged.history(), false, Address::new(0))
        .unwrap();

    predictor.restore(&snapshot).unwrap();

    assert_eq!(predictor.snapshot().tage(), snapshot.tage());
    assert_eq!(
        predictor.snapshot().loop_predictor(),
        snapshot.loop_predictor()
    );
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn ltage_rejects_mismatched_config_and_forwards_inner_errors() {
    let one_thread_loop = LoopBranchPredictorConfig::new(1).unwrap();

    assert_eq!(
        LTageBranchPredictorConfig::new(tage_config(false), one_thread_loop),
        Err(LTageBranchPredictorError::ThreadCountMismatch {
            tage_threads: 2,
            loop_threads: 1,
        })
    );

    let loop_with_different_shift = LoopBranchPredictorConfig::with_options(
        2, 3, 1, 3, 2, 4, 4, 3, 3, false, false, false, false, 1, 3, true,
    )
    .unwrap();

    assert_eq!(
        LTageBranchPredictorConfig::new(tage_config(false), loop_with_different_shift),
        Err(LTageBranchPredictorError::InstShiftMismatch {
            tage_inst_shift: 2,
            loop_inst_shift: 3,
        })
    );

    let mut predictor = ltage(false, false);

    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x10), true),
        Err(LTageBranchPredictorError::Tage(
            TageBranchPredictorError::UnknownThread { cpu: CpuId::new(2) }
        ))
    );
}
