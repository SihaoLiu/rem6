use rem6_cpu::{
    CpuId, TageBranchPredictor, TageBranchPredictorConfig, TageBranchPredictorError, TageProvider,
};
use rem6_memory::Address;

fn tage(speculative_history: bool) -> TageBranchPredictor {
    TageBranchPredictor::new(
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
        .unwrap(),
    )
}

#[test]
fn tage_predictor_computes_bimodal_tagged_indexes_and_folded_history() {
    let mut predictor = tage(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let first = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(first.cpu(), cpu);
    assert_eq!(first.pc(), pc);
    assert_eq!(first.bimodal_index(), 1);
    assert_eq!(first.tagged_indices(), &[1, 3, 5]);
    assert_eq!(first.tagged_tags(), &[0, 1, 17]);
    assert_eq!(first.provider(), TageProvider::BimodalOnly);
    assert!(!first.predicted_taken());
    assert!(!first.longest_match_predicted_taken());
    assert!(!first.alternate_predicted_taken());

    let history_update = predictor
        .update_history(first.history(), true, Address::new(0))
        .unwrap();

    assert_eq!(history_update.old_path_history(), 0);
    assert_eq!(history_update.new_path_history(), 1);
    assert_eq!(history_update.old_global_history(), 0);
    assert_eq!(history_update.new_global_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].path_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].global_history_value(), 1);

    let second = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(second.tagged_indices(), &[1, 0, 0]);
    assert_eq!(second.tagged_tags(), &[0, 2, 18]);
}

#[test]
fn tage_predictor_uses_alt_on_new_counter_for_pseudo_new_longest_match() {
    let mut predictor = tage(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    predictor
        .write_tagged_entry(1, 3, 1, -2, 1)
        .expect("bank 1 entry");
    predictor
        .write_tagged_entry(2, 5, 17, 0, 0)
        .expect("bank 2 entry");
    predictor
        .write_bimodal_entry(1, false, false)
        .expect("bimodal entry");

    let alt_provider = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(alt_provider.hit_bank(), Some(2));
    assert_eq!(alt_provider.alternate_bank(), Some(1));
    assert_eq!(alt_provider.provider(), TageProvider::TageAlternateMatch);
    assert!(alt_provider.longest_match_predicted_taken());
    assert!(!alt_provider.alternate_predicted_taken());
    assert!(!alt_provider.predicted_taken());
    assert!(alt_provider.pseudo_new_allocation());

    let update = predictor.train(alt_provider.history(), true).unwrap();

    assert_eq!(update.use_alt_counter_before(), 0);
    assert_eq!(update.use_alt_counter_after(), -1);
    assert_eq!(update.updated_bank(), Some(2));
    assert_eq!(update.updated_alt_bank(), Some(1));
    assert_eq!(predictor.snapshot().use_alt_on_new_counters(), &[-1]);

    let longest_provider = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(longest_provider.provider(), TageProvider::TageLongestMatch);
    assert!(longest_provider.predicted_taken());
}

#[test]
fn tage_predictor_allocates_entries_deterministically_on_mispredict() {
    let mut predictor = tage(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    let update = predictor.train(prediction.history(), true).unwrap();

    assert_eq!(update.allocated_entries(), &[(1, 3)]);
    assert_eq!(update.t_counter_after(), 1);
    assert_eq!(update.update_count(), 1);

    let snapshot = predictor.snapshot();
    assert_eq!(snapshot.tagged_tables()[1][3].tag(), 1);
    assert_eq!(snapshot.tagged_tables()[1][3].counter(), 0);
    assert_eq!(snapshot.tagged_tables()[1][3].useful(), 0);
    assert!(snapshot.bimodal_prediction()[1]);
    assert!(!snapshot.bimodal_hysteresis()[0]);
}

#[test]
fn tage_predictor_repairs_speculative_history_with_actual_outcome() {
    let mut predictor = tage(true);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .update_history(prediction.history(), true, Address::new(0))
        .unwrap();

    assert_eq!(predictor.snapshot().threads()[0].path_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].global_history_value(), 1);

    let repair = predictor
        .repair_history(prediction.history(), false, Address::new(0))
        .unwrap();

    assert_eq!(repair.old_path_history(), 1);
    assert_eq!(repair.new_path_history(), 1);
    assert_eq!(repair.old_global_history(), 1);
    assert_eq!(repair.new_global_history(), 0);
    assert_eq!(repair.history_update_count(), 2);

    let repaired = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(repaired.tagged_indices(), &[1, 1, 1]);
    assert_eq!(repaired.tagged_tags(), &[0, 1, 17]);
}

#[test]
fn tage_predictor_rejects_stale_history_update_without_mutating_thread() {
    let mut predictor = tage(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let first = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .update_history(first.history(), true, Address::new(0))
        .unwrap();

    assert_eq!(
        predictor
            .update_history(first.history(), false, Address::new(0))
            .unwrap_err(),
        TageBranchPredictorError::HistoryUpdateOutOfOrder {
            cpu,
            expected_path_history: 1,
            actual_path_history: 0,
            expected_global_history: 1,
            actual_global_history: 0,
        },
    );
    assert_eq!(predictor.snapshot().threads()[0].path_history(), 1);
    assert_eq!(predictor.snapshot().threads()[0].global_history_value(), 1);
    assert_eq!(predictor.history_update_count(), 1);

    let current = predictor.predict(cpu, pc, true).unwrap();
    let update = predictor
        .update_history(current.history(), false, Address::new(0))
        .unwrap();
    assert_eq!(update.old_path_history(), 1);
    assert_eq!(update.new_path_history(), 3);
    assert_eq!(update.old_global_history(), 1);
    assert_eq!(update.new_global_history(), 2);
    assert_eq!(predictor.history_update_count(), 2);
}

#[test]
fn tage_predictor_snapshot_restore_preserves_tables_histories_and_counts() {
    let mut predictor = tage(true);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor.train(prediction.history(), true).unwrap();
    predictor
        .update_history(prediction.history(), true, Address::new(0))
        .unwrap();
    let snapshot = predictor.snapshot();

    let diverged = predictor.predict(cpu, pc, true).unwrap();
    predictor.train(diverged.history(), false).unwrap();
    predictor
        .update_history(diverged.history(), false, Address::new(0))
        .unwrap();

    predictor.restore(&snapshot).unwrap();

    assert_eq!(
        predictor.snapshot().tagged_tables(),
        snapshot.tagged_tables()
    );
    assert_eq!(
        predictor.snapshot().bimodal_prediction(),
        snapshot.bimodal_prediction()
    );
    assert_eq!(
        predictor.snapshot().bimodal_hysteresis(),
        snapshot.bimodal_hysteresis()
    );
    assert_eq!(predictor.snapshot().threads(), snapshot.threads());
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn tage_predictor_rejects_bad_config_thread_and_snapshot_shape() {
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            0,
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
            false,
        ),
        Err(TageBranchPredictorError::ZeroThreads)
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            0,
            2,
            6,
            vec![0],
            vec![4],
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
            false,
        ),
        Err(TageBranchPredictorError::ZeroHistoryTables)
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            7,
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
            false,
        ),
        Err(TageBranchPredictorError::HistoryRangeInvalid {
            min_history: 7,
            max_history: 6,
        })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![0, 4],
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
            false,
        ),
        Err(TageBranchPredictorError::TableVectorLengthMismatch {
            expected: 3,
            actual_tag_widths: 2,
            actual_log_sizes: 3,
        })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![1, 4, 5],
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
            false,
        ),
        Err(TageBranchPredictorError::BimodalTagWidthNonZero { width: 1 })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![0, 1, 5],
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
            false,
        ),
        Err(TageBranchPredictorError::TagWidthOutOfRange { bank: 1, bits: 1 })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![0, 4, 5],
            vec![4, 3, 3],
            1,
            1,
            2,
            8,
            4,
            1,
            4,
            1,
            2,
            false,
            false,
        ),
        Err(TageBranchPredictorError::CounterBitsOutOfRange { bits: 1 })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![0, 4, 5],
            vec![4, 3, 3],
            1,
            3,
            2,
            0,
            4,
            1,
            4,
            1,
            2,
            false,
            false,
        ),
        Err(TageBranchPredictorError::PathHistoryBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![0, 4, 5],
            vec![4, 3, 3],
            5,
            3,
            2,
            8,
            4,
            1,
            4,
            1,
            2,
            false,
            false,
        ),
        Err(TageBranchPredictorError::BimodalHysteresisRatioOutOfRange { bits: 5, max: 4 })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
            2,
            2,
            6,
            vec![0, 4, 5],
            vec![4, 3, 3],
            1,
            3,
            2,
            8,
            64,
            1,
            4,
            1,
            2,
            false,
            false,
        ),
        Err(TageBranchPredictorError::UsefulResetPeriodOutOfRange { bits: 64 })
    );
    assert_eq!(
        TageBranchPredictorConfig::with_options(
            1,
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
            0,
            4,
            1,
            2,
            false,
            false,
        ),
        Err(TageBranchPredictorError::ZeroUseAltOnNewCounters)
    );

    let mut predictor = tage(false);
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x44), true),
        Err(TageBranchPredictorError::UnknownThread { cpu: CpuId::new(2) })
    );

    let snapshot = tage(false).snapshot();
    let mut different_shape = TageBranchPredictor::new(
        TageBranchPredictorConfig::with_options(
            2,
            3,
            2,
            8,
            vec![0, 4, 5, 6],
            vec![4, 3, 3, 3],
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
            false,
        )
        .unwrap(),
    );

    assert_eq!(
        different_shape.restore(&snapshot),
        Err(TageBranchPredictorError::SnapshotShapeMismatch {
            expected_history_tables: 3,
            actual_history_tables: 2,
            expected_bimodal_entries: 16,
            actual_bimodal_entries: 16,
        })
    );
}
