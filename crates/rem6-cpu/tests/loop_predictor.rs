use rem6_cpu::{CpuId, LoopBranchPredictor, LoopBranchPredictorConfig, LoopBranchPredictorError};
use rem6_memory::Address;

fn loop_predictor(use_speculation: bool) -> LoopBranchPredictor {
    LoopBranchPredictor::new(
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

#[test]
fn loop_predictor_allocates_deterministically_and_records_lookup_fields() {
    let mut predictor = loop_predictor(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let miss = predictor.predict(cpu, pc, true, false).unwrap();

    assert_eq!(miss.cpu(), cpu);
    assert_eq!(miss.pc(), pc);
    assert_eq!(miss.loop_index(), 0);
    assert_eq!(miss.loop_index_b(), 0);
    assert_eq!(miss.loop_tag(), 1);
    assert_eq!(miss.loop_hit(), None);
    assert!(!miss.loop_prediction_valid());
    assert!(!miss.loop_prediction_used());
    assert!(!miss.predicted_taken());

    let update = predictor.train(miss.history(), true).unwrap();

    assert_eq!(update.allocated_index(), Some(0));
    assert_eq!(update.loop_use_counter_before(), -1);
    assert_eq!(update.loop_use_counter_after(), -1);
    assert_eq!(update.update_count(), 1);

    let snapshot = predictor.snapshot();
    assert_eq!(snapshot.allocation_cursors()[0], 1);
    assert_eq!(snapshot.entries()[0].tag(), 1);
    assert_eq!(snapshot.entries()[0].age(), 3);
    assert_eq!(snapshot.entries()[0].current_iter(), 1);
    assert_eq!(snapshot.entries()[0].num_iter(), 0);
}

#[test]
fn loop_predictor_learns_trip_count_and_overrides_base_prediction_when_trusted() {
    let mut predictor = loop_predictor(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    train_trip_count_loop(&mut predictor, pc, cpu);

    let first_trusted = predictor.predict(cpu, pc, true, false).unwrap();
    assert!(first_trusted.loop_prediction_valid());
    assert!(!first_trusted.loop_prediction_used());
    assert!(first_trusted.loop_predicted_taken());
    assert!(!first_trusted.predicted_taken());

    let counter_update = predictor.train(first_trusted.history(), true).unwrap();
    assert_eq!(counter_update.loop_use_counter_before(), -1);
    assert_eq!(counter_update.loop_use_counter_after(), 0);

    let body = predictor.predict(cpu, pc, true, false).unwrap();
    assert!(body.loop_prediction_valid());
    assert!(body.loop_prediction_used());
    assert!(body.loop_predicted_taken());
    assert!(body.predicted_taken());
    predictor.train(body.history(), true).unwrap();

    let exit = predictor.predict(cpu, pc, true, true).unwrap();
    assert!(exit.loop_prediction_valid());
    assert!(exit.loop_prediction_used());
    assert!(!exit.loop_predicted_taken());
    assert!(!exit.predicted_taken());

    let exit_update = predictor.train(exit.history(), false).unwrap();
    assert_eq!(exit_update.used_count(), 2);
    assert_eq!(exit_update.correct_count(), 2);
    assert_eq!(exit_update.wrong_count(), 0);
    assert_eq!(predictor.snapshot().entries()[0].current_iter(), 0);
    assert_eq!(predictor.snapshot().entries()[0].confidence(), 3);
}

#[test]
fn loop_predictor_squash_restores_speculative_iteration_without_training() {
    let mut predictor = loop_predictor(true);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    train_trip_count_loop(&mut predictor, pc, cpu);
    let first_trusted = predictor.predict(cpu, pc, true, false).unwrap();
    predictor.train(first_trusted.history(), true).unwrap();
    let body = predictor.predict(cpu, pc, true, false).unwrap();
    predictor.train(body.history(), true).unwrap();

    assert_eq!(predictor.snapshot().entries()[0].current_iter(), 2);
    assert_eq!(predictor.snapshot().entries()[0].current_iter_spec(), 2);

    let exit = predictor.predict(cpu, pc, true, true).unwrap();

    assert_eq!(exit.current_iter_spec_before(), Some(2));
    assert_eq!(predictor.snapshot().entries()[0].current_iter(), 2);
    assert_eq!(predictor.snapshot().entries()[0].current_iter_spec(), 3);

    let squash = predictor.squash(exit.history()).unwrap();

    assert_eq!(squash.restored_current_iter_spec(), Some(2));
    assert_eq!(squash.squash_count(), 1);
    assert_eq!(predictor.snapshot().entries()[0].current_iter(), 2);
    assert_eq!(predictor.snapshot().entries()[0].current_iter_spec(), 2);
    assert_eq!(predictor.update_count(), 14);
}

#[test]
fn loop_predictor_hashes_indexes_and_tags_when_enabled() {
    let mut predictor = LoopBranchPredictor::new(
        LoopBranchPredictorConfig::with_options(
            1, 4, 2, 3, 2, 5, 4, 3, 2, false, false, true, false, 1, 3, true,
        )
        .unwrap(),
    );
    let pc = Address::new(0x1234);

    let prediction = predictor.predict(CpuId::new(0), pc, true, false).unwrap();

    assert_eq!(prediction.loop_index(), 4);
    assert_eq!(prediction.loop_index_b(), 1);
    assert_eq!(prediction.loop_tag(), 9);
    assert_eq!(prediction.loop_hit(), None);

    let update = predictor.train(prediction.history(), true).unwrap();
    assert_eq!(update.allocated_index(), Some(0));

    let hit = predictor.predict(CpuId::new(0), pc, true, false).unwrap();
    assert_eq!(hit.loop_hit(), Some(0));
    assert_eq!(hit.final_index(), Some(0));
}

#[test]
fn loop_predictor_snapshot_restore_preserves_entries_cursors_counts_and_counter() {
    let mut predictor = loop_predictor(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    train_trip_count_loop(&mut predictor, pc, cpu);
    let snapshot = predictor.snapshot();

    let diverged = predictor.predict(cpu, pc, true, false).unwrap();
    predictor.train(diverged.history(), true).unwrap();

    predictor.restore(&snapshot).unwrap();

    assert_eq!(predictor.snapshot().entries(), snapshot.entries());
    assert_eq!(
        predictor.snapshot().allocation_cursors(),
        snapshot.allocation_cursors()
    );
    assert_eq!(predictor.loop_use_counter(), snapshot.loop_use_counter());
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn loop_predictor_rejects_bad_config_thread_and_snapshot_shape() {
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            0, 3, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::ZeroThreads)
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 0, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::LogSizeOutOfRange { bits: 0 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 4, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::LogAssociativityExceedsSize {
            log_size: 3,
            log_assoc: 4,
        })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 0, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::AgeBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 3, 2, 17, 4, 3, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::TagBitsOutOfRange { bits: 17 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 3, 2, 4, 17, 3, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::IterBitsOutOfRange { bits: 17 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 3, 2, 4, 4, 0, 2, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::WithLoopBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 3, 2, 4, 4, 3, 64, false, false, false, false, 1, 3, true
        ),
        Err(LoopBranchPredictorError::InstShiftOutOfRange { bits: 64 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 16, 3, true
        ),
        Err(LoopBranchPredictorError::InitialIterOutOfRange { value: 16, max: 15 })
    );
    assert_eq!(
        LoopBranchPredictorConfig::with_options(
            1, 3, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 8, true
        ),
        Err(LoopBranchPredictorError::InitialAgeOutOfRange { value: 8, max: 7 })
    );

    let mut predictor = loop_predictor(false);
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x10), true, false),
        Err(LoopBranchPredictorError::UnknownThread { cpu: CpuId::new(2) })
    );

    let snapshot = loop_predictor(false).snapshot();
    let mut different_shape = LoopBranchPredictor::new(
        LoopBranchPredictorConfig::with_options(
            2, 4, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true,
        )
        .unwrap(),
    );

    assert_eq!(
        different_shape.restore(&snapshot),
        Err(LoopBranchPredictorError::SnapshotShapeMismatch {
            expected_entries: 16,
            actual_entries: 8,
            expected_sets: 8,
            actual_sets: 4,
        })
    );
}
