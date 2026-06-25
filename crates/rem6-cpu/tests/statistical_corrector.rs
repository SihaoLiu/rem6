use rem6_cpu::{
    CpuId, StatisticalCorrector, StatisticalCorrectorBranchKind, StatisticalCorrectorConfig,
    StatisticalCorrectorError, StatisticalCorrectorInput,
};
use rem6_memory::Address;

fn sc(speculative_history: bool) -> StatisticalCorrector {
    StatisticalCorrector::new(
        StatisticalCorrectorConfig::tage_sc_l_8kb(2, 2, speculative_history).unwrap(),
    )
}

fn low_conf_input(previous_prediction: bool) -> StatisticalCorrectorInput {
    StatisticalCorrectorInput::new(previous_prediction)
        .with_bias_bit(true)
        .with_tage_counter(0, 3)
        .with_banks(0, 0)
}

#[test]
fn statistical_corrector_computes_8kb_bias_gehl_sum_and_threshold() {
    let mut corrector = sc(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let prediction = corrector
        .predict(cpu, pc, true, low_conf_input(false))
        .unwrap();

    assert_eq!(prediction.cpu(), cpu);
    assert_eq!(prediction.pc(), pc);
    assert_eq!(prediction.bias_index(), 22);
    assert_eq!(prediction.bias_sk_index(), 16);
    assert_eq!(prediction.bias_bank_index(), 4);
    assert_eq!(prediction.update_index(), 5);
    assert_eq!(prediction.update_weight_index(), 5);
    assert_eq!(prediction.global_indices(), &[4, 4]);
    assert_eq!(prediction.backward_indices(), &[4, 4]);
    assert_eq!(prediction.local_indices(), &[4, 4]);
    assert_eq!(prediction.imli_indices(), &[4]);
    assert_eq!(prediction.local_history(), 0);
    assert_eq!(prediction.linear_sum(), -172);
    assert_eq!(prediction.threshold(), 35);
    assert!(!prediction.sc_predicted_taken());
    assert!(!prediction.used_sc_prediction());
    assert!(!prediction.predicted_taken());
}

#[test]
fn statistical_corrector_overrides_weak_base_prediction_when_sum_disagrees() {
    let mut corrector = sc(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let probe = corrector
        .predict(cpu, pc, true, low_conf_input(false))
        .unwrap();
    corrector
        .write_bias_entries(
            probe.bias_index(),
            probe.bias_sk_index(),
            probe.bias_bank_index(),
            31,
            31,
            31,
        )
        .unwrap();

    let prediction = corrector
        .predict(cpu, pc, true, low_conf_input(false))
        .unwrap();

    assert!(prediction.linear_sum() > prediction.threshold());
    assert!(prediction.sc_predicted_taken());
    assert!(prediction.used_sc_prediction());
    assert!(prediction.predicted_taken());
}

#[test]
fn statistical_corrector_trains_bias_gehl_and_threshold_counters() {
    let mut corrector = sc(false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let prediction = corrector
        .predict(cpu, pc, true, low_conf_input(false))
        .unwrap();
    let update = corrector.train(prediction.history(), true).unwrap();

    assert_eq!(update.update_threshold_before(), 280);
    assert_eq!(update.update_threshold_after(), 281);
    assert_eq!(update.per_pc_threshold_before(), 0);
    assert_eq!(update.per_pc_threshold_after(), 1);
    assert_eq!(update.bias_before(), (-1, -8, -32));
    assert_eq!(update.bias_after(), (0, -7, -31));
    assert_eq!(update.global_counter_after(), Some(0));
    assert_eq!(update.backward_counter_after(), Some(0));
    assert_eq!(update.local_counter_after(), Some(0));
    assert_eq!(update.imli_counter_after(), Some(0));
    assert_eq!(update.update_count(), 1);
    assert_eq!(corrector.snapshot().wrong_count(), 1);
}

#[test]
fn statistical_corrector_keeps_repairable_per_cpu_histories() {
    let mut corrector = sc(true);
    let cpu0 = CpuId::new(0);
    let cpu1 = CpuId::new(1);
    let pc = Address::new(0x10);

    let prediction = corrector
        .predict(cpu0, pc, true, low_conf_input(false))
        .unwrap();
    let update = corrector
        .update_history(
            prediction.history(),
            StatisticalCorrectorBranchKind::DirectConditional,
            true,
            Address::new(0),
            9,
        )
        .unwrap();

    assert_eq!(update.old_thread().global_history(), 0);
    assert_eq!(update.new_thread().global_history(), 1);
    assert_eq!(update.new_thread().backward_history(), 1);
    assert_eq!(update.new_thread().imli_count(), 1);
    assert_eq!(update.new_thread().path_history(), 9);
    assert_eq!(corrector.snapshot().threads()[1].global_history(), 0);

    let repair = corrector
        .repair_history(
            prediction.history(),
            StatisticalCorrectorBranchKind::DirectConditional,
            false,
            Address::new(0),
            3,
        )
        .unwrap();

    assert_eq!(repair.old_thread().global_history(), 1);
    assert_eq!(repair.new_thread().global_history(), 0);
    assert_eq!(repair.new_thread().backward_history(), 0);
    assert_eq!(repair.new_thread().imli_count(), 0);
    assert_eq!(repair.new_thread().path_history(), 3);
    assert_eq!(corrector.snapshot().threads()[0].global_history(), 0);
    assert_eq!(corrector.snapshot().threads()[1].global_history(), 0);
    assert_eq!(cpu1, CpuId::new(1));
}

#[test]
fn statistical_corrector_snapshot_restore_preserves_tables_histories_and_counts() {
    let mut corrector = sc(true);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let prediction = corrector
        .predict(cpu, pc, true, low_conf_input(false))
        .unwrap();
    corrector.train(prediction.history(), true).unwrap();
    corrector
        .update_history(
            prediction.history(),
            StatisticalCorrectorBranchKind::DirectConditional,
            true,
            Address::new(0),
            5,
        )
        .unwrap();
    let snapshot = corrector.snapshot();

    let diverged = corrector
        .predict(cpu, pc, true, low_conf_input(true))
        .unwrap();
    corrector.train(diverged.history(), false).unwrap();
    corrector
        .update_history(
            diverged.history(),
            StatisticalCorrectorBranchKind::DirectConditional,
            false,
            Address::new(0x80),
            1,
        )
        .unwrap();

    corrector.restore(&snapshot).unwrap();

    assert_eq!(corrector.snapshot().bias(), snapshot.bias());
    assert_eq!(corrector.snapshot().global_gehl(), snapshot.global_gehl());
    assert_eq!(corrector.snapshot().threads(), snapshot.threads());
    assert_eq!(corrector.lookup_count(), snapshot.lookup_count());
    assert_eq!(corrector.update_count(), snapshot.update_count());
    assert_eq!(
        corrector.history_update_count(),
        snapshot.history_update_count()
    );
}

#[test]
fn statistical_corrector_rejects_bad_config_thread_and_snapshot_config() {
    assert_eq!(
        StatisticalCorrectorConfig::tage_sc_l_8kb(0, 2, false),
        Err(StatisticalCorrectorError::ZeroThreads)
    );
    assert_eq!(
        StatisticalCorrectorConfig::with_options(
            1,
            1,
            1,
            64,
            vec![6, 3],
            vec![16, 8],
            vec![6, 3],
            vec![8],
            7,
            7,
            7,
            7,
            7,
            7,
            7,
            6,
            7,
            12,
            8,
            6,
            6,
            0,
            2,
            false,
        ),
        Err(StatisticalCorrectorError::LogSizeUpTooSmall { bits: 1 })
    );
    assert_eq!(
        StatisticalCorrectorConfig::with_options(
            1,
            1,
            21,
            64,
            vec![6, 3],
            vec![16, 8],
            vec![6, 3],
            vec![8],
            7,
            7,
            7,
            7,
            7,
            7,
            7,
            6,
            7,
            12,
            8,
            6,
            6,
            0,
            2,
            false,
        ),
        Err(StatisticalCorrectorError::LogSizeOutOfRange {
            field: "update",
            bits: 21,
        })
    );

    let mut corrector = sc(false);
    assert_eq!(
        corrector.predict(
            CpuId::new(2),
            Address::new(0x10),
            true,
            low_conf_input(false)
        ),
        Err(StatisticalCorrectorError::UnknownThread { cpu: CpuId::new(2) })
    );

    let snapshot = sc(false).snapshot();
    let mut different_config =
        StatisticalCorrector::new(StatisticalCorrectorConfig::tage_sc_l_8kb(2, 0, false).unwrap());

    assert_eq!(
        different_config.restore(&snapshot),
        Err(StatisticalCorrectorError::SnapshotConfigMismatch {
            expected: Box::new(different_config.config().clone()),
            actual: Box::new(snapshot.config().clone()),
        })
    );
}
