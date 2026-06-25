use rem6_cpu::{
    CpuId, LTageBranchPredictorConfig, LTageBranchPredictorError, LTageProvider,
    LoopBranchPredictorConfig, StatisticalCorrectorBranchKind, StatisticalCorrectorConfig,
    StatisticalCorrectorError, TageBranchPredictorConfig, TageBranchPredictorError, TageProvider,
    TageScLBranchPredictor, TageScLBranchPredictorCheckpointPayload, TageScLBranchPredictorConfig,
    TageScLBranchPredictorError, TageScLProvider,
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
        2,
        3,
        1,
        3,
        2,
        4,
        4,
        3,
        2,
        false,
        use_speculation,
        false,
        false,
        1,
        3,
        true,
    )
    .unwrap()
}

fn predictor(
    speculative_tage: bool,
    speculative_loop: bool,
    speculative_sc: bool,
) -> TageScLBranchPredictor {
    TageScLBranchPredictor::new(
        TageScLBranchPredictorConfig::new(
            LTageBranchPredictorConfig::new(
                tage_config(speculative_tage),
                loop_config(speculative_loop),
            )
            .unwrap(),
            StatisticalCorrectorConfig::tage_sc_l_8kb(2, 2, speculative_sc).unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn tage_sc_l_uses_ltage_when_statistical_corrector_does_not_override() {
    let mut predictor = predictor(false, false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    predictor
        .ltage_mut()
        .tage_mut()
        .write_tagged_entry(2, 5, 17, 2, 1)
        .unwrap();

    let prediction = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(prediction.cpu(), cpu);
    assert_eq!(prediction.pc(), pc);
    assert_eq!(
        prediction.provider(),
        TageScLProvider::LTage(LTageProvider::Tage(TageProvider::TageLongestMatch))
    );
    assert!(prediction.ltage_prediction().predicted_taken());
    assert!(!prediction
        .statistical_corrector_prediction()
        .used_sc_prediction());
    assert!(prediction.predicted_taken());
}

#[test]
fn tage_sc_l_statistical_corrector_overrides_ltage_prediction() {
    let mut predictor = predictor(false, false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x10);

    let probe = predictor.predict(cpu, pc, true).unwrap();
    let sc_probe = probe.statistical_corrector_prediction();
    predictor
        .statistical_corrector_mut()
        .write_bias_entries(
            sc_probe.bias_index(),
            sc_probe.bias_sk_index(),
            sc_probe.bias_bank_index(),
            31,
            31,
            31,
        )
        .unwrap();

    let prediction = predictor.predict(cpu, pc, true).unwrap();

    assert_eq!(prediction.provider(), TageScLProvider::StatisticalCorrector);
    assert!(!prediction.ltage_prediction().predicted_taken());
    assert!(prediction
        .statistical_corrector_prediction()
        .used_sc_prediction());
    assert!(prediction
        .statistical_corrector_prediction()
        .sc_predicted_taken());
    assert!(prediction.predicted_taken());
}

#[test]
fn tage_sc_l_train_updates_sc_ltage_and_histories_in_reference_order() {
    let mut predictor = predictor(false, false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    let update = predictor
        .train(
            prediction.history(),
            true,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0),
        )
        .unwrap();

    assert_eq!(update.statistical_corrector_update().update_count(), 1);
    assert_eq!(
        update.ltage_update().loop_update().allocated_index(),
        Some(2)
    );
    assert_eq!(
        update.ltage_update().tage_update().allocated_entries(),
        &[(1, 3)]
    );
    assert_eq!(
        update
            .statistical_corrector_history_update()
            .new_thread()
            .path_history(),
        1
    );
    assert_eq!(predictor.statistical_corrector().update_count(), 1);
    assert_eq!(predictor.ltage().update_count(), 1);
    assert_eq!(predictor.statistical_corrector().history_update_count(), 1);
}

#[test]
fn tage_sc_l_stale_train_rejects_without_partial_inner_mutation() {
    let mut predictor = predictor(false, false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(
            prediction.history(),
            true,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0),
        )
        .unwrap();
    let snapshot = predictor.snapshot();

    assert_eq!(
        predictor
            .train(
                prediction.history(),
                false,
                StatisticalCorrectorBranchKind::DirectConditional,
                Address::new(0),
            )
            .unwrap_err(),
        TageScLBranchPredictorError::LTage(LTageBranchPredictorError::Tage(
            TageBranchPredictorError::HistoryUpdateOutOfOrder {
                cpu,
                expected_path_history: 1,
                actual_path_history: 0,
                expected_global_history: 1,
                actual_global_history: 0,
            },
        )),
    );
    assert_eq!(predictor.snapshot(), snapshot);
}

#[test]
fn tage_sc_l_repair_restores_ltage_and_statistical_corrector_histories() {
    let mut predictor = predictor(true, true, true);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    let speculative = predictor
        .update_history(
            prediction.history(),
            true,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0),
        )
        .unwrap();

    assert_eq!(speculative.tage_history_update().new_global_history(), 1);
    assert_eq!(
        speculative
            .statistical_corrector_history_update()
            .new_thread()
            .global_history(),
        1
    );
    assert_eq!(
        speculative
            .statistical_corrector_history_update()
            .new_thread()
            .path_history(),
        1
    );

    let repair = predictor
        .repair(
            prediction.history(),
            false,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0),
        )
        .unwrap();

    assert_eq!(
        repair.ltage_repair().history_update().new_global_history(),
        0
    );
    assert_eq!(
        repair
            .statistical_corrector_repair()
            .new_thread()
            .global_history(),
        0
    );
    assert_eq!(
        repair
            .statistical_corrector_repair()
            .new_thread()
            .path_history(),
        1
    );
    assert_eq!(predictor.repair_count(), 1);
}

#[test]
fn tage_sc_l_snapshot_restore_preserves_inner_predictors_and_counts() {
    let mut predictor = predictor(false, false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(
            prediction.history(),
            true,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0),
        )
        .unwrap();
    let snapshot = predictor.snapshot();

    let diverged = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(
            diverged.history(),
            false,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0x80),
        )
        .unwrap();

    predictor.restore(&snapshot).unwrap();

    assert_eq!(predictor.snapshot().ltage(), snapshot.ltage());
    assert_eq!(
        predictor.snapshot().statistical_corrector(),
        snapshot.statistical_corrector()
    );
    assert_eq!(predictor.lookup_count(), snapshot.lookup_count());
    assert_eq!(predictor.update_count(), snapshot.update_count());
}

#[test]
fn tage_sc_l_checkpoint_payload_round_trips_snapshot() {
    let mut predictor = predictor(false, false, false);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x44);

    let prediction = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(
            prediction.history(),
            true,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0),
        )
        .unwrap();
    let snapshot = predictor.snapshot();

    let encoded = TageScLBranchPredictorCheckpointPayload::from_snapshot(snapshot.clone())
        .unwrap()
        .encode();
    let decoded = TageScLBranchPredictorCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);

    let diverged = predictor.predict(cpu, pc, true).unwrap();
    predictor
        .train(
            diverged.history(),
            false,
            StatisticalCorrectorBranchKind::DirectConditional,
            Address::new(0x80),
        )
        .unwrap();
    assert_ne!(predictor.snapshot(), snapshot);

    predictor.restore(&decoded.into_snapshot()).unwrap();

    assert_eq!(predictor.snapshot(), snapshot);
}

#[test]
fn tage_sc_l_checkpoint_payload_rejects_truncated_payload() {
    let payload = TageScLBranchPredictorCheckpointPayload::from_snapshot(
        predictor(false, false, false).snapshot(),
    )
    .unwrap()
    .encode();

    let error = TageScLBranchPredictorCheckpointPayload::decode(&payload[..3]).unwrap_err();

    assert_eq!(
        error,
        TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: 5,
            actual: 3,
        }
    );
}

#[test]
fn tage_sc_l_checkpoint_payload_rejects_oversized_config_vector_length_without_allocating() {
    let mut payload = TageScLBranchPredictorCheckpointPayload::from_snapshot(
        predictor(false, false, false).snapshot(),
    )
    .unwrap()
    .encode();
    let tag_widths_len_offset = checkpoint_tage_tag_widths_len_offset();
    assert_eq!(
        u32::from_le_bytes(
            payload[tag_widths_len_offset..tag_widths_len_offset + 4]
                .try_into()
                .unwrap()
        ),
        3
    );
    payload[tag_widths_len_offset..tag_widths_len_offset + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());

    let error = TageScLBranchPredictorCheckpointPayload::decode(&payload).unwrap_err();

    assert_eq!(
        error,
        TageScLBranchPredictorError::InvalidCheckpointVectorLength {
            name: "tage-tag-widths",
            expected: 3,
            actual: u32::MAX as usize,
        }
    );
}

#[test]
fn tage_sc_l_checkpoint_payload_rejects_oversized_sc_config_vector_length_without_allocating() {
    let mut payload = TageScLBranchPredictorCheckpointPayload::from_snapshot(
        predictor(false, false, false).snapshot(),
    )
    .unwrap()
    .encode();
    let sc_global_lengths_len_offset = checkpoint_sc_global_lengths_len_offset(&payload);
    assert_eq!(
        u32::from_le_bytes(
            payload[sc_global_lengths_len_offset..sc_global_lengths_len_offset + 4]
                .try_into()
                .unwrap()
        ),
        2
    );
    payload[sc_global_lengths_len_offset..sc_global_lengths_len_offset + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());

    let error = TageScLBranchPredictorCheckpointPayload::decode(&payload).unwrap_err();

    assert_eq!(
        error,
        TageScLBranchPredictorError::InvalidCheckpointVectorLength {
            name: "sc-global-lengths",
            expected: 64,
            actual: u32::MAX as usize,
        }
    );
}

#[test]
fn tage_sc_l_checkpoint_payload_rejects_oversized_vector_length_without_allocating() {
    let mut payload = TageScLBranchPredictorCheckpointPayload::from_snapshot(
        predictor(false, false, false).snapshot(),
    )
    .unwrap()
    .encode();
    let bimodal_prediction_len_offset = checkpoint_tage_bimodal_prediction_len_offset(&payload);
    assert_eq!(
        u32::from_le_bytes(
            payload[bimodal_prediction_len_offset..bimodal_prediction_len_offset + 4]
                .try_into()
                .unwrap()
        ),
        16
    );
    payload[bimodal_prediction_len_offset..bimodal_prediction_len_offset + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());

    let error = TageScLBranchPredictorCheckpointPayload::decode(&payload).unwrap_err();

    assert_eq!(
        error,
        TageScLBranchPredictorError::InvalidCheckpointVectorLength {
            name: "tage-bimodal-prediction",
            expected: 16,
            actual: u32::MAX as usize,
        }
    );
}

fn checkpoint_tage_tag_widths_len_offset() -> usize {
    5 + 4 * 4
}

fn checkpoint_sc_global_lengths_len_offset(payload: &[u8]) -> usize {
    let mut offset = 5;

    offset += 4 * 4;
    skip_u8_vec(payload, &mut offset);
    skip_u8_vec(payload, &mut offset);
    offset += 5;
    offset += 4;
    offset += 1;
    offset += 4;
    offset += 1;
    offset += 2;

    offset += 4;
    offset += 8;
    offset += 4;
    offset += 2;
    offset += 1;
    offset += 1;

    offset += 4;
    offset += 1;
    offset += 1;
    offset += 4;

    offset
}

fn checkpoint_tage_bimodal_prediction_len_offset(payload: &[u8]) -> usize {
    let mut offset = 5;

    offset += 4 * 4;
    skip_u8_vec(payload, &mut offset);
    skip_u8_vec(payload, &mut offset);
    offset += 5;
    offset += 4;
    offset += 1;
    offset += 4;
    offset += 1;
    offset += 2;

    offset += 4;
    offset += 8;
    offset += 4;
    offset += 2;
    offset += 1;
    offset += 1;

    offset += 4;
    offset += 1;
    offset += 1;
    offset += 4;
    skip_u8_vec(payload, &mut offset);
    skip_u8_vec(payload, &mut offset);
    skip_u8_vec(payload, &mut offset);
    skip_u8_vec(payload, &mut offset);
    offset += 4;
    offset += 4;
    offset += 5;
    offset += 2;
    offset += 1;
    offset += 1;

    offset
}

fn skip_u8_vec(payload: &[u8], offset: &mut usize) {
    let len = u32::from_le_bytes(payload[*offset..*offset + 4].try_into().unwrap()) as usize;
    *offset += 4 + len;
}

#[test]
fn tage_sc_l_rejects_mismatched_config_and_forwards_inner_errors() {
    let one_thread_sc = StatisticalCorrectorConfig::tage_sc_l_8kb(1, 2, false).unwrap();

    assert_eq!(
        TageScLBranchPredictorConfig::new(
            LTageBranchPredictorConfig::new(tage_config(false), loop_config(false)).unwrap(),
            one_thread_sc,
        ),
        Err(TageScLBranchPredictorError::ThreadCountMismatch {
            ltage_threads: 2,
            statistical_corrector_threads: 1,
        })
    );

    let different_shift_sc = StatisticalCorrectorConfig::tage_sc_l_8kb(2, 0, false).unwrap();

    assert_eq!(
        TageScLBranchPredictorConfig::new(
            LTageBranchPredictorConfig::new(tage_config(false), loop_config(false)).unwrap(),
            different_shift_sc,
        ),
        Err(TageScLBranchPredictorError::InstShiftMismatch {
            ltage_inst_shift: 2,
            statistical_corrector_inst_shift: 0,
        })
    );

    let mut predictor = predictor(false, false, false);
    assert_eq!(
        predictor.predict(CpuId::new(2), Address::new(0x10), true),
        Err(TageScLBranchPredictorError::LTage(
            LTageBranchPredictorError::Tage(rem6_cpu::TageBranchPredictorError::UnknownThread {
                cpu: CpuId::new(2),
            })
        ))
    );

    let sc_snapshot = StatisticalCorrectorConfig::tage_sc_l_8kb(2, 2, false).unwrap();
    let wrong_sc = StatisticalCorrectorError::SnapshotConfigMismatch {
        expected: Box::new(sc_snapshot.clone()),
        actual: Box::new(sc_snapshot),
    };
    assert!(matches!(
        TageScLBranchPredictorError::StatisticalCorrector(wrong_sc),
        TageScLBranchPredictorError::StatisticalCorrector(_)
    ));
}
