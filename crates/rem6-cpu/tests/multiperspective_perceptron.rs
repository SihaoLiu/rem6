use rem6_cpu::{
    CpuId, MultiperspectivePerceptron, MultiperspectivePerceptronConfig,
    MultiperspectivePerceptronError, MultiperspectivePerceptronFeature,
    MultiperspectivePerceptronFeatureKind,
};
use rem6_memory::Address;

fn compact_config(filter_entries: usize) -> MultiperspectivePerceptronConfig {
    MultiperspectivePerceptronConfig::with_options(
        2,
        filter_entries,
        8,
        4,
        16,
        -4,
        1,
        -5,
        5,
        -1,
        1,
        4,
        8,
        -2,
        0,
        0,
        0,
        64,
        2,
        2,
        0,
        0xff,
        false,
        true,
        0,
        4,
        3,
        4096,
        1,
        false,
        vec![
            MultiperspectivePerceptronFeature::bias(64, 8, 6),
            MultiperspectivePerceptronFeature::global_history(0, 7, 64, 0, 6),
            MultiperspectivePerceptronFeature::local(64, 0, 6),
        ],
    )
    .unwrap()
}

#[test]
fn mpp_8kb_profile_matches_gem5_shape() {
    let config = MultiperspectivePerceptronConfig::eight_kb(2).unwrap();
    let predictor = MultiperspectivePerceptron::new(config.clone()).unwrap();

    assert_eq!(config.threads(), 2);
    assert_eq!(config.budget_bits(), 8192 * 8 + 2048);
    assert_eq!(config.num_local_histories(), 48);
    assert_eq!(config.num_filter_entries(), 0);
    assert_eq!(config.local_history_length(), 11);
    assert_eq!(config.pc_shift(), -10);
    assert_eq!(config.threshold(), 1);
    assert_eq!(config.features().len(), 16);
    assert_eq!(
        config.features()[0].kind(),
        MultiperspectivePerceptronFeatureKind::Bias
    );
    assert_eq!(config.features()[0].coefficient_q6(), 154);
    assert_eq!(config.features()[9].table_entries(), 600);
    assert_eq!(config.features()[10].table_entries(), 375);
    assert_eq!(config.features()[11].table_entries(), 512);
    assert_eq!(predictor.table_entries().len(), 16);
    assert!(predictor.table_entries().iter().all(|entries| *entries > 0));
}

#[test]
fn mpp_trains_perceptron_weights_and_private_thread_histories() {
    let mut predictor = MultiperspectivePerceptron::new(compact_config(0)).unwrap();
    let cpu0 = CpuId::new(0);
    let cpu1 = CpuId::new(1);
    let pc = Address::new(0x40);

    let initial = predictor.predict(cpu0, pc, true).unwrap();
    assert!(!initial.predicted_taken());
    assert_eq!(initial.linear_sum(), -2);

    let first_update = predictor
        .train(initial.history(), true, Address::new(0x20))
        .unwrap();
    assert!(first_update.trained());
    assert_eq!(first_update.feature_updates()[0].magnitude_before(), 0);
    assert_eq!(first_update.feature_updates()[0].magnitude_after(), 1);
    assert!(!first_update.feature_updates()[0].sign_after());
    assert_eq!(first_update.update_count(), 1);

    for _ in 0..5 {
        let prediction = predictor.predict(cpu0, pc, true).unwrap();
        predictor
            .train(prediction.history(), true, Address::new(0x20))
            .unwrap();
    }

    let trained = predictor.predict(cpu0, pc, true).unwrap();
    assert!(trained.predicted_taken());
    assert!(trained.linear_sum() > 0);

    let cpu0_thread = predictor.thread_snapshot(cpu0).unwrap();
    let cpu1_thread = predictor.thread_snapshot(cpu1).unwrap();
    assert_eq!(cpu0_thread.global_history_prefix(6), vec![true; 6]);
    assert_eq!(cpu0_thread.local_history_for(pc) & 0b1111, 0b1111);
    assert_eq!(cpu0_thread.path_history()[0], (pc.get() >> 2) as u16);
    assert_eq!(cpu0_thread.imli_counters()[0], 6);
    assert_eq!(cpu1_thread.global_history_prefix(6), vec![false; 6]);
    assert_eq!(cpu1_thread.local_history_for(pc), 0);
}

#[test]
fn mpp_filter_records_stable_branches_without_hidden_shared_state() {
    let mut predictor = MultiperspectivePerceptron::new(compact_config(8)).unwrap();
    let cpu = CpuId::new(0);
    let pc = Address::new(0x80);

    let first = predictor.predict(cpu, pc, true).unwrap();
    assert!(first.used_static_prediction());
    assert!(!first.filtered());
    assert!(!first.predicted_taken());

    let first_update = predictor
        .train(first.history(), false, Address::new(0))
        .unwrap();
    assert!(first_update.trained());
    assert!(first_update
        .filter_after()
        .unwrap()
        .always_not_taken_so_far());

    let second = predictor.predict(cpu, pc, true).unwrap();
    assert!(second.filtered());
    assert!(!second.predicted_taken());

    let transition = predictor
        .train(second.history(), true, Address::new(0x40))
        .unwrap();
    assert!(!transition.trained());
    assert!(transition.filter_after().unwrap().seen_taken());
    assert!(transition.filter_after().unwrap().seen_untaken());

    let third = predictor.predict(cpu, pc, true).unwrap();
    assert!(!third.filtered());
    assert!(!third.used_static_prediction());
}

#[test]
fn mpp_snapshot_restore_preserves_tables_histories_and_counters() {
    let mut predictor = MultiperspectivePerceptron::new(compact_config(0)).unwrap();
    let cpu = CpuId::new(0);
    let pc = Address::new(0xc0);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(prediction.history(), true, Address::new(0x40))
        .unwrap();
    let snapshot = predictor.snapshot();

    let diverged = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(diverged.history(), false, Address::new(0))
        .unwrap();
    predictor.restore(&snapshot).unwrap();

    assert_eq!(predictor.snapshot(), snapshot);
}

#[test]
fn mpp_rejects_invalid_config_and_unknown_threads() {
    assert_eq!(
        MultiperspectivePerceptronConfig::with_options(
            0,
            0,
            8,
            4,
            16,
            -4,
            1,
            -5,
            5,
            -1,
            1,
            4,
            8,
            -2,
            0,
            0,
            0,
            64,
            2,
            2,
            0,
            0xff,
            false,
            true,
            0,
            4,
            3,
            4096,
            1,
            false,
            vec![MultiperspectivePerceptronFeature::bias(64, 8, 6)],
        ),
        Err(MultiperspectivePerceptronError::ZeroThreads)
    );

    assert_eq!(
        MultiperspectivePerceptronConfig::with_options(
            1,
            0,
            0,
            4,
            16,
            -4,
            1,
            -5,
            5,
            -1,
            1,
            4,
            8,
            -2,
            0,
            0,
            0,
            64,
            2,
            2,
            0,
            0xff,
            false,
            true,
            0,
            4,
            3,
            4096,
            1,
            false,
            vec![MultiperspectivePerceptronFeature::bias(64, 8, 6)],
        ),
        Err(MultiperspectivePerceptronError::ZeroLocalHistories)
    );

    let mut predictor = MultiperspectivePerceptron::new(compact_config(0)).unwrap();
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x10), true),
        Err(MultiperspectivePerceptronError::UnknownThread { cpu: CpuId::new(2) })
    );
}
