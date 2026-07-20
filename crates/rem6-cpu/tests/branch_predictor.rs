#[path = "branch_predictor/legacy_checkpoint_fixtures.rs"]
mod legacy_checkpoint_fixtures;

use legacy_checkpoint_fixtures::{
    LEGACY_V1_DEFAULT_PAYLOAD, LEGACY_V2_ACTIVE_MAPPING_PAYLOAD,
    LEGACY_V3_TARGET_PREDICTION_PAYLOAD, LEGACY_V4_RAS_PAYLOAD, LEGACY_V5_BRANCH_KIND_PAYLOAD,
};
use rem6_cpu::{
    BranchPredictor, BranchPredictorCheckpointPayload, BranchPredictorConfig, BranchPredictorError,
    BranchSpeculationId, BranchTargetBuffer, BranchTargetBufferConfig, BranchTargetBufferError,
    BranchTargetKind, BranchTargetKindCounts, BranchTargetPrediction, BranchTargetSafetyConfig,
    BranchTargetSafetyProfile, ReturnAddressStack, ReturnAddressStackConfig,
    ReturnAddressStackError, ReturnAddressStackOperationId, ReturnAddressStackOperationKind,
    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY, DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
    DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES,
};
use rem6_memory::Address;

const CHECKPOINT_HEADER_BYTES: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4 * 2;
const CHECKPOINT_TARGET_BYTES: usize = 1 + 8;
const PENDING_SPECULATION_BYTES: usize = 49;
const BTB_LEGACY_HEADER_BYTES: usize = 4 * 2 + 8 * 6;
const BTB_KIND_COUNTER_BYTES: usize = 8 * 8 * 4;
const BTB_HEADER_BYTES: usize = BTB_LEGACY_HEADER_BYTES + BTB_KIND_COUNTER_BYTES;
const BTB_ENTRY_BYTES: usize = 1 + 8 + 8 + 1 + 8;
const RAS_HEADER_BYTES: usize = 4 * 3 + 8;

fn predictor(entries: usize) -> BranchPredictor {
    BranchPredictor::new(BranchPredictorConfig::new(entries).unwrap())
}

fn ras(entries: usize) -> ReturnAddressStack {
    ReturnAddressStack::new(ReturnAddressStackConfig::new(entries).unwrap())
}

fn btb(entries: usize, associativity: usize) -> BranchTargetBuffer {
    BranchTargetBuffer::new(BranchTargetBufferConfig::new(entries, associativity).unwrap())
}

fn single_pending_ras_checkpoint_payload() -> (Vec<u8>, ReturnAddressStackOperationId, usize) {
    let mut predictor = predictor(8);
    let speculation = predictor.predict_speculative(Address::new(0x1000));
    let mut return_address_stack = ras(4);
    let operation = return_address_stack.push_speculative(Address::new(0x1004));
    let payload = BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack(
        predictor.snapshot(),
        btb(8, 2).snapshot(),
        [(21, speculation.id())],
        std::iter::empty::<(u64, BranchTargetPrediction)>(),
        return_address_stack.snapshot(),
        [(21, operation.id())],
    )
    .unwrap()
    .encode();
    let operation_start = CHECKPOINT_HEADER_BYTES
        + 8
        + 8 * CHECKPOINT_TARGET_BYTES
        + PENDING_SPECULATION_BYTES
        + BTB_HEADER_BYTES
        + 8 * BTB_ENTRY_BYTES
        + RAS_HEADER_BYTES
        + 8;
    (payload, operation.id(), operation_start)
}

fn assert_legacy_checkpoint_migrates_to_current(
    legacy: &[u8],
    decoded: &BranchPredictorCheckpointPayload,
) {
    let current = decoded.encode();
    assert_eq!(current[4], 6);
    assert_ne!(current.as_slice(), legacy);
    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&current),
        Ok(decoded.clone())
    );
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
fn restore_rejects_snapshot_with_different_history_width() {
    let snapshot =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap()).snapshot();
    let mut wider = BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 4).unwrap());

    let error = wider.restore(&snapshot).unwrap_err();

    assert_eq!(
        error,
        BranchPredictorError::SnapshotHistoryBitsMismatch {
            expected: 4,
            actual: 3,
        }
    );
}

#[test]
fn config_rejects_empty_table() {
    assert_eq!(
        BranchPredictorConfig::new(0),
        Err(BranchPredictorError::ZeroTableEntries)
    );
    assert_eq!(
        BranchPredictorConfig::with_history_bits(8, 0),
        Err(BranchPredictorError::HistoryBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        BranchPredictorConfig::with_history_bits(8, 65),
        Err(BranchPredictorError::HistoryBitsOutOfRange { bits: 65 })
    );
}

#[test]
fn btb_config_rejects_gem5_issue_3188_oversized_shapes() {
    assert_eq!(
        BranchTargetBufferConfig::new(8192, 8),
        Err(BranchTargetBufferError::EntriesExceedLimit {
            entries: 8192,
            max_entries: 4096,
        })
    );
    assert_eq!(
        BranchTargetBufferConfig::new(4096, 16),
        Err(BranchTargetBufferError::AssociativityExceedsLimit {
            associativity: 16,
            max_associativity: 8,
        })
    );

    let explicit = BranchTargetBufferConfig::with_limits(8192, 8, 8192, 8).unwrap();
    assert_eq!(explicit.entries(), 8192);
    assert_eq!(explicit.associativity(), 8);
    assert_eq!(explicit.sets(), 1024);
}

#[test]
fn branch_target_safety_rejects_gem5_issue_2211_riscv_o3_config() {
    assert_eq!(
        BranchTargetSafetyConfig::riscv_o3_full_system(false, false).unwrap_err(),
        BranchPredictorError::ReturnTargetProtectionDisabled {
            profile: BranchTargetSafetyProfile::RiscvO3FullSystem,
            return_address_stack_enabled: false,
            indirect_targets_hashed: false,
        }
    );

    let protected_by_ras = BranchTargetSafetyConfig::riscv_o3_full_system(true, false).unwrap();
    assert_eq!(
        protected_by_ras.profile(),
        BranchTargetSafetyProfile::RiscvO3FullSystem
    );
    assert!(protected_by_ras.return_address_stack_enabled());
    assert!(!protected_by_ras.indirect_targets_hashed());
    assert!(protected_by_ras.return_target_protected());

    let protected_by_indirect_path =
        BranchTargetSafetyConfig::riscv_o3_full_system(false, true).unwrap();
    assert!(!protected_by_indirect_path.return_address_stack_enabled());
    assert!(protected_by_indirect_path.indirect_targets_hashed());
    assert!(protected_by_indirect_path.return_target_protected());
}

#[test]
fn speculative_prediction_records_history_and_unwinds_false_path() {
    let mut predictor = predictor(8);
    let taken_pc = Address::new(0x1000);
    let false_path_pc = Address::new(0x1004);
    let target = Address::new(0x1080);

    predictor.update(taken_pc, true, Some(target));

    let first = predictor.predict_speculative(taken_pc);
    assert_eq!(first.id(), BranchSpeculationId::new(0));
    assert_eq!(first.history_before(), 0);
    assert_eq!(first.history_after(), 1);
    assert!(first.predicted_taken());
    assert!(first.history_taken());
    assert_eq!(predictor.speculative_history(), 1);
    assert_eq!(predictor.committed_history(), 0);

    let second = predictor.predict_speculative(false_path_pc);
    assert_eq!(second.id(), BranchSpeculationId::new(1));
    assert_eq!(second.history_before(), 1);
    assert_eq!(second.history_after(), 2);
    assert!(!second.predicted_taken());
    assert!(!second.history_taken());
    assert_eq!(predictor.pending_speculation_count(), 2);

    let repair = predictor.repair_speculation(first.id(), false).unwrap();

    assert_eq!(repair.repaired().id(), first.id());
    assert_eq!(repair.removed_youngers(), &[second]);
    assert_eq!(repair.history_before(), 0);
    assert_eq!(repair.old_history_after(), 1);
    assert_eq!(repair.new_history_after(), 0);
    assert_eq!(predictor.speculative_history(), 0);
    assert_eq!(predictor.committed_history(), 0);
    assert_eq!(predictor.pending_speculation_count(), 1);
    assert_eq!(predictor.pending_speculations()[0].id(), first.id());
    assert!(!predictor.pending_speculations()[0].history_taken());
}

#[test]
fn committing_speculation_advances_committed_history_in_order() {
    let mut predictor = predictor(8);
    let taken_pc = Address::new(0x1000);
    let not_taken_pc = Address::new(0x1004);
    let target = Address::new(0x1080);

    predictor.update(taken_pc, true, Some(target));

    let first = predictor.predict_speculative(taken_pc);
    let second = predictor.predict_speculative(not_taken_pc);

    assert_eq!(predictor.speculative_history(), 2);
    assert_eq!(predictor.committed_history(), 0);

    let committed_first = predictor.commit_speculation(first.id()).unwrap();
    assert_eq!(committed_first, first);
    assert_eq!(predictor.committed_history(), 1);
    assert_eq!(predictor.speculative_history(), 2);
    assert_eq!(
        predictor.pending_speculations(),
        std::slice::from_ref(&second)
    );

    let committed_second = predictor.commit_speculation(second.id()).unwrap();
    assert_eq!(committed_second, second);
    assert_eq!(predictor.committed_history(), 2);
    assert_eq!(predictor.speculative_history(), 2);
    assert_eq!(predictor.pending_speculation_count(), 0);
}

#[test]
fn discarding_speculation_restores_history_without_committing_branch() {
    let mut predictor = predictor(8);
    let taken_pc = Address::new(0x1000);
    let skipped_pc = Address::new(0x1004);
    let younger_pc = Address::new(0x1008);
    let target = Address::new(0x1080);

    predictor.update(taken_pc, true, Some(target));
    let committed = predictor.predict_speculative(taken_pc);
    predictor.commit_speculation(committed.id()).unwrap();
    assert_eq!(predictor.committed_history(), 1);

    let skipped = predictor.predict_speculative(skipped_pc);
    let younger = predictor.predict_speculative(younger_pc);
    assert_eq!(skipped.history_before(), 1);
    assert_eq!(predictor.speculative_history(), 4);

    let discard = predictor.discard_speculation(skipped.id()).unwrap();

    assert_eq!(discard.discarded(), &skipped);
    assert_eq!(discard.removed_youngers(), &[younger]);
    assert_eq!(discard.restored_history(), 1);
    assert_eq!(predictor.committed_history(), 1);
    assert_eq!(predictor.speculative_history(), 1);
    assert_eq!(predictor.pending_speculations(), &[]);
}

#[test]
fn discarding_all_speculations_restores_committed_history() {
    let mut predictor = predictor(8);
    let taken_pc = Address::new(0x1000);
    let skipped_pc = Address::new(0x1004);
    let younger_pc = Address::new(0x1008);
    let target = Address::new(0x1080);

    predictor.update(taken_pc, true, Some(target));
    let committed = predictor.predict_speculative(taken_pc);
    predictor.commit_speculation(committed.id()).unwrap();
    assert_eq!(predictor.committed_history(), 1);

    let skipped = predictor.predict_speculative(skipped_pc);
    let younger = predictor.predict_speculative(younger_pc);

    let discarded = predictor.discard_all_speculations();

    assert_eq!(discarded, vec![skipped, younger]);
    assert_eq!(predictor.committed_history(), 1);
    assert_eq!(predictor.speculative_history(), 1);
    assert_eq!(predictor.pending_speculations(), &[]);
}

#[test]
fn speculation_commit_rejects_out_of_order_and_unknown_records() {
    let mut predictor = predictor(8);
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));

    assert_eq!(
        predictor.commit_speculation(second.id()),
        Err(BranchPredictorError::OutOfOrderSpeculationCommit {
            expected: first.id(),
            actual: second.id(),
        })
    );
    assert_eq!(
        predictor.repair_speculation(BranchSpeculationId::new(99), true),
        Err(BranchPredictorError::UnknownSpeculation {
            id: BranchSpeculationId::new(99),
        })
    );
    assert_eq!(
        predictor.discard_speculation(BranchSpeculationId::new(99)),
        Err(BranchPredictorError::UnknownSpeculation {
            id: BranchSpeculationId::new(99),
        })
    );
}

#[test]
fn snapshot_restore_preserves_pending_speculation_history() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let first_pc = Address::new(0x1000);
    let second_pc = Address::new(0x1004);

    predictor.update(first_pc, true, Some(Address::new(0x1080)));
    let first = predictor.predict_speculative(first_pc);
    let second = predictor.predict_speculative(second_pc);
    let snapshot = predictor.snapshot();

    predictor.repair_speculation(first.id(), false).unwrap();
    predictor.update(second_pc, true, Some(Address::new(0x1100)));

    predictor.restore(&snapshot).unwrap();

    assert_eq!(predictor.committed_history(), 0);
    assert_eq!(predictor.speculative_history(), 2);
    assert_eq!(predictor.pending_speculations(), &[first, second]);
    assert_eq!(predictor.predict(second_pc).counter(), 1);
}

#[test]
fn checkpoint_payload_round_trips_snapshot_and_active_mapping() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let taken_pc = Address::new(0x1000);
    let skipped_pc = Address::new(0x1004);

    predictor.update(taken_pc, true, Some(Address::new(0x1080)));
    let first = predictor.predict_speculative(taken_pc);
    let second = predictor.predict_speculative(skipped_pc);
    let snapshot = predictor.snapshot();
    let mut branch_target_buffer = btb(8, 2);
    branch_target_buffer.lookup(taken_pc, BranchTargetKind::DirectConditional);
    branch_target_buffer.update(
        taken_pc,
        Address::new(0x1080),
        BranchTargetKind::DirectConditional,
    );
    branch_target_buffer.lookup(taken_pc, BranchTargetKind::DirectConditional);
    let branch_target_buffer_snapshot = branch_target_buffer.snapshot();
    let mut return_address_stack = ras(4);
    let call_operation = return_address_stack.push_speculative(Address::new(0x1004));
    let return_operation = return_address_stack.pop_speculative();
    let return_address_stack_snapshot = return_address_stack.snapshot();

    let payload = BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack_and_branch_kinds(
        snapshot.clone(),
        branch_target_buffer_snapshot.clone(),
        [(22, second.id()), (21, first.id())],
        [
            (22, BranchTargetPrediction::new(false, None)),
            (
                21,
                BranchTargetPrediction::new(true, Some(Address::new(0x1080))),
            ),
        ],
        return_address_stack_snapshot.clone(),
        [
            (22, return_operation.id()),
            (21, call_operation.id()),
        ],
        [
            (22, BranchTargetKind::DirectUnconditional),
            (21, BranchTargetKind::DirectConditional),
        ],
    )
    .unwrap();

    assert_eq!(
        payload.active_speculations(),
        &[(21, first.id()), (22, second.id())]
    );
    assert_eq!(
        payload.active_branch_target_predictions(),
        &[
            (
                21,
                BranchTargetPrediction::new(true, Some(Address::new(0x1080)))
            ),
            (22, BranchTargetPrediction::new(false, None)),
        ]
    );
    assert_eq!(
        payload.active_return_address_stack_operations(),
        &[(21, call_operation.id()), (22, return_operation.id()),]
    );
    assert_eq!(
        payload.active_branch_kinds(),
        &[
            (21, BranchTargetKind::DirectConditional),
            (22, BranchTargetKind::DirectUnconditional),
        ]
    );

    let decoded = BranchPredictorCheckpointPayload::decode(&payload.encode()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(
        decoded.branch_target_buffer_snapshot(),
        &branch_target_buffer_snapshot
    );
    assert_eq!(decoded.active_speculations(), payload.active_speculations());
    assert_eq!(
        decoded.active_branch_target_predictions(),
        payload.active_branch_target_predictions()
    );
    assert_eq!(
        decoded.return_address_stack_snapshot(),
        &return_address_stack_snapshot
    );
    assert_eq!(
        decoded.active_return_address_stack_operations(),
        payload.active_return_address_stack_operations()
    );
    assert_eq!(decoded.active_branch_kinds(), payload.active_branch_kinds());
}

#[test]
fn checkpoint_payload_round_trips_return_address_stack_operation_id_gaps_after_squash() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let mut return_address_stack = ras(4);
    let first_ras = return_address_stack.push_speculative(Address::new(0x1004));
    let squashed = return_address_stack.push_speculative(Address::new(0x2004));
    let _removed_younger = return_address_stack.push_speculative(Address::new(0x3004));
    return_address_stack.squash_from(squashed.id()).unwrap();
    let second_ras = return_address_stack.push_speculative(Address::new(0x4004));
    assert_eq!(first_ras.id(), ReturnAddressStackOperationId::new(0));
    assert_eq!(second_ras.id(), ReturnAddressStackOperationId::new(3));

    let payload = BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack(
        predictor.snapshot(),
        btb(8, 2).snapshot(),
        [(21, first.id()), (22, second.id())],
        std::iter::empty::<(u64, BranchTargetPrediction)>(),
        return_address_stack.snapshot(),
        [(21, first_ras.id()), (22, second_ras.id())],
    )
    .unwrap();

    let decoded = BranchPredictorCheckpointPayload::decode(&payload.encode()).unwrap();

    assert_eq!(
        decoded
            .return_address_stack_snapshot()
            .pending_operations()
            .iter()
            .map(|operation| operation.id())
            .collect::<Vec<_>>(),
        vec![
            ReturnAddressStackOperationId::new(0),
            ReturnAddressStackOperationId::new(3)
        ]
    );
    assert_eq!(
        decoded.active_return_address_stack_operations(),
        &[(21, first_ras.id()), (22, second_ras.id())]
    );
}

#[test]
fn checkpoint_payload_decodes_v4_active_mapping_with_ras_without_branch_kinds() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let mut return_address_stack = ras(4);
    let first_ras = return_address_stack.push_speculative(Address::new(0x1004));
    let second_ras = return_address_stack.push_speculative(Address::new(0x1008));
    let payload = BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack(
        predictor.snapshot(),
        btb(8, 2).snapshot(),
        [(21, first.id()), (22, second.id())],
        [
            (
                21,
                BranchTargetPrediction::new(true, Some(Address::new(0x1080))),
            ),
            (22, BranchTargetPrediction::new(false, None)),
        ],
        return_address_stack.snapshot(),
        [(21, first_ras.id()), (22, second_ras.id())],
    )
    .unwrap();

    let decoded = BranchPredictorCheckpointPayload::decode(LEGACY_V4_RAS_PAYLOAD).unwrap();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(
        decoded.branch_target_buffer_snapshot(),
        payload.branch_target_buffer_snapshot()
    );
    assert_eq!(decoded.active_speculations(), payload.active_speculations());
    assert_eq!(
        decoded.active_branch_target_predictions(),
        payload.active_branch_target_predictions()
    );
    assert_eq!(
        decoded.return_address_stack_snapshot(),
        payload.return_address_stack_snapshot()
    );
    assert_eq!(
        decoded.active_return_address_stack_operations(),
        payload.active_return_address_stack_operations()
    );
    assert_eq!(decoded.active_branch_kinds(), &[]);
    assert_legacy_checkpoint_migrates_to_current(LEGACY_V4_RAS_PAYLOAD, &decoded);
}

#[test]
fn checkpoint_payload_decodes_v2_active_mapping_without_branch_target_predictions() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let payload = BranchPredictorCheckpointPayload::from_snapshots(
        predictor.snapshot(),
        btb(8, 2).snapshot(),
        [(21, first.id()), (22, second.id())],
    )
    .unwrap();

    let decoded =
        BranchPredictorCheckpointPayload::decode(LEGACY_V2_ACTIVE_MAPPING_PAYLOAD).unwrap();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(
        decoded.branch_target_buffer_snapshot(),
        payload.branch_target_buffer_snapshot()
    );
    assert_eq!(decoded.active_speculations(), payload.active_speculations());
    assert_eq!(decoded.active_branch_target_predictions(), &[]);
    assert_eq!(
        decoded.return_address_stack_snapshot(),
        &ras(DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES).snapshot()
    );
    assert_eq!(decoded.active_return_address_stack_operations(), &[]);
    assert_eq!(decoded.active_branch_kinds(), &[]);
    assert_legacy_checkpoint_migrates_to_current(LEGACY_V2_ACTIVE_MAPPING_PAYLOAD, &decoded);
}

#[test]
fn checkpoint_payload_decodes_v3_active_mapping_with_branch_target_predictions_without_ras() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let payload = BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions(
        predictor.snapshot(),
        btb(8, 2).snapshot(),
        [(21, first.id()), (22, second.id())],
        [
            (
                21,
                BranchTargetPrediction::new(true, Some(Address::new(0x1080))),
            ),
            (22, BranchTargetPrediction::new(false, None)),
        ],
    )
    .unwrap();

    let decoded =
        BranchPredictorCheckpointPayload::decode(LEGACY_V3_TARGET_PREDICTION_PAYLOAD).unwrap();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(
        decoded.branch_target_buffer_snapshot(),
        payload.branch_target_buffer_snapshot()
    );
    assert_eq!(decoded.active_speculations(), payload.active_speculations());
    assert_eq!(
        decoded.active_branch_target_predictions(),
        payload.active_branch_target_predictions()
    );
    assert_eq!(
        decoded.return_address_stack_snapshot(),
        &ras(DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES).snapshot()
    );
    assert_eq!(decoded.active_return_address_stack_operations(), &[]);
    assert_eq!(decoded.active_branch_kinds(), &[]);
    assert_legacy_checkpoint_migrates_to_current(LEGACY_V3_TARGET_PREDICTION_PAYLOAD, &decoded);
}

#[test]
fn checkpoint_payload_decodes_legacy_v1_with_default_btb_snapshot() {
    let snapshot = predictor(8).snapshot();

    let decoded = BranchPredictorCheckpointPayload::decode(LEGACY_V1_DEFAULT_PAYLOAD).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(decoded.active_speculations(), &[]);
    assert_eq!(decoded.active_branch_target_predictions(), &[]);
    assert_eq!(
        decoded.branch_target_buffer_snapshot(),
        &btb(
            DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
            DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY,
        )
        .snapshot()
    );
    assert_eq!(
        decoded.return_address_stack_snapshot(),
        &ras(DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES).snapshot()
    );
    assert_eq!(decoded.active_return_address_stack_operations(), &[]);
    assert_eq!(decoded.active_branch_kinds(), &[]);
    assert_legacy_checkpoint_migrates_to_current(LEGACY_V1_DEFAULT_PAYLOAD, &decoded);
}

#[test]
fn checkpoint_payload_decodes_v5_active_mapping_with_branch_kinds_without_btb_kind_counters() {
    let mut predictor =
        BranchPredictor::new(BranchPredictorConfig::with_history_bits(8, 3).unwrap());
    let taken_pc = Address::new(0x1000);
    let skipped_pc = Address::new(0x1004);

    predictor.update(taken_pc, true, Some(Address::new(0x1080)));
    let first = predictor.predict_speculative(taken_pc);
    let second = predictor.predict_speculative(skipped_pc);
    let snapshot = predictor.snapshot();
    let mut branch_target_buffer = btb(8, 2);
    branch_target_buffer.lookup(taken_pc, BranchTargetKind::DirectConditional);
    branch_target_buffer.update(
        taken_pc,
        Address::new(0x1080),
        BranchTargetKind::DirectConditional,
    );
    branch_target_buffer.lookup(taken_pc, BranchTargetKind::DirectConditional);
    let branch_target_buffer_snapshot = branch_target_buffer.snapshot();
    let mut return_address_stack = ras(4);
    let call_operation = return_address_stack.push_speculative(Address::new(0x1004));
    let return_operation = return_address_stack.pop_speculative();
    let return_address_stack_snapshot = return_address_stack.snapshot();

    let payload = BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack_and_branch_kinds(
        snapshot,
        branch_target_buffer_snapshot,
        [(22, second.id()), (21, first.id())],
        [
            (22, BranchTargetPrediction::new(false, None)),
            (
                21,
                BranchTargetPrediction::new(true, Some(Address::new(0x1080))),
            ),
        ],
        return_address_stack_snapshot,
        [
            (22, return_operation.id()),
            (21, call_operation.id()),
        ],
        [
            (22, BranchTargetKind::DirectUnconditional),
            (21, BranchTargetKind::DirectConditional),
        ],
    )
    .unwrap();

    let decoded = BranchPredictorCheckpointPayload::decode(LEGACY_V5_BRANCH_KIND_PAYLOAD).unwrap();
    assert_eq!(decoded.snapshot(), payload.snapshot());
    let decoded_btb = decoded.branch_target_buffer_snapshot();
    let expected_btb = payload.branch_target_buffer_snapshot();
    assert_eq!(decoded_btb.config(), expected_btb.config());
    assert_eq!(decoded_btb.entries(), expected_btb.entries());
    assert_eq!(
        decoded_btb.access_sequence(),
        expected_btb.access_sequence()
    );
    assert_eq!(decoded_btb.lookup_count(), expected_btb.lookup_count());
    assert_eq!(decoded_btb.hit_count(), expected_btb.hit_count());
    assert_eq!(decoded_btb.miss_count(), expected_btb.miss_count());
    assert_eq!(decoded_btb.update_count(), expected_btb.update_count());
    assert_eq!(decoded_btb.eviction_count(), expected_btb.eviction_count());
    let zero_kind_counts = BranchTargetKindCounts::default();
    assert_eq!(decoded_btb.lookup_kind_counts(), zero_kind_counts);
    assert_eq!(decoded_btb.hit_kind_counts(), zero_kind_counts);
    assert_eq!(decoded_btb.miss_kind_counts(), zero_kind_counts);
    assert_eq!(decoded_btb.update_kind_counts(), zero_kind_counts);
    assert_eq!(decoded.active_speculations(), payload.active_speculations());
    assert_eq!(
        decoded.active_branch_target_predictions(),
        payload.active_branch_target_predictions()
    );
    assert_eq!(
        decoded.return_address_stack_snapshot(),
        payload.return_address_stack_snapshot()
    );
    assert_eq!(
        decoded.active_return_address_stack_operations(),
        payload.active_return_address_stack_operations()
    );
    assert_eq!(decoded.active_branch_kinds(), payload.active_branch_kinds());
    assert_legacy_checkpoint_migrates_to_current(LEGACY_V5_BRANCH_KIND_PAYLOAD, &decoded);
}

#[test]
fn checkpoint_payload_rejects_btb_config_outside_decode_limits() {
    let oversized_config = BranchTargetBufferConfig::with_limits(8192, 4, 8192, 4).unwrap();
    let oversized_snapshot = BranchTargetBuffer::new(oversized_config).snapshot();

    assert!(matches!(
        BranchPredictorCheckpointPayload::from_snapshots(
            predictor(8).snapshot(),
            oversized_snapshot,
            std::iter::empty::<(u64, BranchSpeculationId)>(),
        ),
        Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint { .. })
    ));
}

#[test]
fn checkpoint_payload_rejects_btb_entry_in_wrong_serialized_set() {
    const BTB_ENTRY_VALID_BYTES: usize = 1;
    let mut branch_target_buffer = btb(8, 2);
    branch_target_buffer.update(
        Address::new(0x1000),
        Address::new(0x1080),
        BranchTargetKind::DirectConditional,
    );
    let payload = BranchPredictorCheckpointPayload::from_snapshots(
        predictor(8).snapshot(),
        branch_target_buffer.snapshot(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();
    let first_btb_entry_pc = CHECKPOINT_HEADER_BYTES
        + 8
        + 8 * CHECKPOINT_TARGET_BYTES
        + BTB_HEADER_BYTES
        + BTB_ENTRY_VALID_BYTES;
    encoded[first_btb_entry_pc..first_btb_entry_pc + 8].copy_from_slice(&0x1004_u64.to_le_bytes());

    assert!(matches!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint { .. })
    ));
}

#[test]
fn checkpoint_payload_rejects_duplicate_btb_entry_pc() {
    let mut branch_target_buffer = btb(8, 2);
    branch_target_buffer.update(
        Address::new(0x1000),
        Address::new(0x1080),
        BranchTargetKind::DirectConditional,
    );
    let payload = BranchPredictorCheckpointPayload::from_snapshots(
        predictor(8).snapshot(),
        branch_target_buffer.snapshot(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();
    let first_btb_entry =
        CHECKPOINT_HEADER_BYTES + 8 + 8 * CHECKPOINT_TARGET_BYTES + BTB_HEADER_BYTES;
    let second_btb_entry = first_btb_entry + BTB_ENTRY_BYTES;
    encoded[second_btb_entry] = 1;
    encoded[second_btb_entry + 1..second_btb_entry + 9].copy_from_slice(&0x1000_u64.to_le_bytes());
    encoded[second_btb_entry + 9..second_btb_entry + 17].copy_from_slice(&0x1090_u64.to_le_bytes());
    encoded[second_btb_entry + 17] = 1;
    encoded[second_btb_entry + 18..second_btb_entry + 26].copy_from_slice(&2_u64.to_le_bytes());

    assert!(matches!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint { .. })
    ));
}

#[test]
fn checkpoint_payload_rejects_current_pending_count_before_allocation() {
    const PENDING_COUNT_OFFSET: usize = 4 + 1 + 4 + 1 + 8 * 4;
    const COUNTER_BYTES: usize = 8;
    const TARGET_BYTES: usize = 8 * (1 + 8);
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        predictor(8).snapshot(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();
    let pending_count = 100_u32;
    let expected = CHECKPOINT_HEADER_BYTES
        + COUNTER_BYTES
        + TARGET_BYTES
        + pending_count as usize * PENDING_SPECULATION_BYTES
        + BTB_HEADER_BYTES;
    encoded[PENDING_COUNT_OFFSET..PENDING_COUNT_OFFSET + 4]
        .copy_from_slice(&pending_count.to_le_bytes());

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected,
            actual: encoded.len(),
        })
    );
}

#[test]
fn checkpoint_payload_rejects_current_active_count_before_allocation() {
    const ACTIVE_COUNT_OFFSET: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4;
    const ACTIVE_SPECULATION_BYTES: usize = 38;
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        predictor(8).snapshot(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();
    let active_count = 300_u32;
    let expected = encoded.len() + active_count as usize * ACTIVE_SPECULATION_BYTES;
    encoded[ACTIVE_COUNT_OFFSET..ACTIVE_COUNT_OFFSET + 4]
        .copy_from_slice(&active_count.to_le_bytes());

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected,
            actual: encoded.len(),
        })
    );
}

#[test]
fn checkpoint_payload_rejects_invalid_counter_encoding() {
    const FIRST_COUNTER_OFFSET: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4 * 2;
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        predictor(8).snapshot(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();

    encoded[FIRST_COUNTER_OFFSET] = 4;

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidCheckpointCounter { value: 4 })
    );
}

#[test]
fn checkpoint_payload_rejects_unmapped_pending_speculation() {
    let mut predictor = predictor(8);
    let pending = predictor.predict_speculative(Address::new(0x1000));

    assert_eq!(
        BranchPredictorCheckpointPayload::from_snapshot(
            predictor.snapshot(),
            std::iter::empty::<(u64, BranchSpeculationId)>(),
        ),
        Err(BranchPredictorError::UnmappedCheckpointSpeculation { id: pending.id() })
    );
}

#[test]
fn checkpoint_payload_rejects_active_mapping_order_that_cannot_commit() {
    const ACTIVE_SPECULATION_BYTES: usize = 38;
    const ACTIVE_SPECULATION_ID_OFFSET: usize = 8;
    let mut predictor = predictor(8);
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        predictor.snapshot(),
        [(21, first.id()), (22, second.id())],
    )
    .unwrap();
    let mut encoded = payload.encode();
    let active_start = encoded.len() - ACTIVE_SPECULATION_BYTES * 2;

    encoded[active_start + ACTIVE_SPECULATION_ID_OFFSET
        ..active_start + ACTIVE_SPECULATION_ID_OFFSET + 8]
        .copy_from_slice(&second.id().get().to_le_bytes());
    let second_entry_id = active_start + ACTIVE_SPECULATION_BYTES + ACTIVE_SPECULATION_ID_OFFSET;
    encoded[second_entry_id..second_entry_id + 8].copy_from_slice(&first.id().get().to_le_bytes());

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidCheckpointSpeculationOrder {
            sequence: 21,
            id: second.id(),
            expected: first.id(),
        })
    );
}

#[test]
fn checkpoint_payload_rejects_return_address_stack_mapping_order_that_cannot_commit() {
    let mut predictor = predictor(8);
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let mut return_address_stack = ras(4);
    let first_ras = return_address_stack.push_speculative(Address::new(0x1004));
    let second_ras = return_address_stack.push_speculative(Address::new(0x2004));

    assert_eq!(
        BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack(
            predictor.snapshot(),
            btb(8, 2).snapshot(),
            [(21, first.id()), (22, second.id())],
            std::iter::empty::<(u64, BranchTargetPrediction)>(),
            return_address_stack.snapshot(),
            [(21, second_ras.id()), (22, first_ras.id())],
        ),
        Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackOperationOrder {
                id: second_ras.id(),
                expected: first_ras.id(),
            }
        )
    );
}

#[test]
fn checkpoint_payload_rejects_push_ras_operation_without_pushed_address() {
    let (mut encoded, operation_id, operation_start) = single_pending_ras_checkpoint_payload();
    const OPERATION_ID_BYTES: usize = 8;
    const OPERATION_KIND_BYTES: usize = 1;
    let pushed_address_flag = operation_start + OPERATION_ID_BYTES + OPERATION_KIND_BYTES;
    encoded[pushed_address_flag] = 0;

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackOperation { id: operation_id }
        )
    );
}

#[test]
fn checkpoint_payload_rejects_ras_operation_final_stack_mismatch() {
    let (mut encoded, operation_id, operation_start) = single_pending_ras_checkpoint_payload();
    const OPERATION_FIXED_BYTES: usize = 8 + 1 + (1 + 8) * 2 + 4 * 2;
    let stack_after_start = operation_start + OPERATION_FIXED_BYTES;
    encoded[stack_after_start..stack_after_start + 8].copy_from_slice(&0x2004_u64.to_le_bytes());

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(
            BranchPredictorError::InvalidCheckpointReturnAddressStackOperation { id: operation_id }
        )
    );
}

#[test]
fn checkpoint_payload_rejects_next_speculation_that_reuses_pending_id() {
    const NEXT_SPECULATION_OFFSET: usize = 4 + 1 + 4 + 1 + 8 * 3;
    let mut predictor = predictor(8);
    let first = predictor.predict_speculative(Address::new(0x1000));
    let second = predictor.predict_speculative(Address::new(0x1004));
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        predictor.snapshot(),
        [(21, first.id()), (22, second.id())],
    )
    .unwrap();
    let mut encoded = payload.encode();

    encoded[NEXT_SPECULATION_OFFSET..NEXT_SPECULATION_OFFSET + 8]
        .copy_from_slice(&second.id().get().to_le_bytes());

    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidCheckpointNextSpeculation {
            next: second.id(),
            pending: second.id(),
        })
    );
}

#[test]
fn return_address_stack_predicts_returns_and_commits_in_order() {
    let mut ras = ras(2);
    let first_return = Address::new(0x1004);
    let second_return = Address::new(0x2004);

    let first_call = ras.push_speculative(first_return);
    assert_eq!(first_call.id(), ReturnAddressStackOperationId::new(0));
    assert_eq!(first_call.kind(), ReturnAddressStackOperationKind::Push);
    assert_eq!(first_call.pushed_address(), Some(first_return));
    assert_eq!(first_call.depth_before(), 0);
    assert_eq!(first_call.depth_after(), 1);
    assert_eq!(ras.top(), Some(first_return));

    let second_call = ras.push_speculative(second_return);
    let predicted_return = ras.pop_speculative();

    assert_eq!(
        predicted_return.kind(),
        ReturnAddressStackOperationKind::Pop
    );
    assert_eq!(predicted_return.predicted_return(), Some(second_return));
    assert_eq!(predicted_return.depth_before(), 2);
    assert_eq!(predicted_return.depth_after(), 1);
    assert_eq!(ras.top(), Some(first_return));
    assert_eq!(ras.pending_operation_count(), 3);

    assert_eq!(ras.commit_operation(first_call.id()).unwrap(), first_call);
    assert_eq!(ras.commit_operation(second_call.id()).unwrap(), second_call);
    assert_eq!(
        ras.commit_operation(predicted_return.id()).unwrap(),
        predicted_return
    );
    assert_eq!(ras.pending_operation_count(), 0);
    assert_eq!(ras.stack_entries(), &[first_return]);
}

#[test]
fn return_address_stack_squash_restores_selected_and_younger_operations() {
    let mut ras = ras(4);
    let call_return = Address::new(0x1004);
    let false_path_return = Address::new(0x2004);

    let call = ras.push_speculative(call_return);
    let mispredicted_return = ras.pop_speculative();
    let false_path_call = ras.push_speculative(false_path_return);

    assert_eq!(ras.stack_entries(), &[false_path_return]);

    let repair = ras.squash_from(mispredicted_return.id()).unwrap();

    assert_eq!(repair.reverted().id(), mispredicted_return.id());
    assert_eq!(repair.removed_youngers(), &[false_path_call]);
    assert_eq!(repair.restored_stack(), &[call_return]);
    assert_eq!(ras.stack_entries(), &[call_return]);
    assert_eq!(ras.pending_operations(), &[call]);
}

#[test]
fn return_address_stack_pop_then_push_predicts_old_top_and_leaves_new_return() {
    let mut ras = ras(4);
    let old_return = Address::new(0x1004);
    let new_return = Address::new(0x2004);
    let seed = ras.push_speculative(old_return);
    ras.commit_operation(seed.id()).unwrap();

    let operation = ras.pop_then_push_speculative(new_return);

    assert_eq!(
        operation.kind(),
        ReturnAddressStackOperationKind::PopThenPush
    );
    assert_eq!(operation.predicted_return(), Some(old_return));
    assert_eq!(operation.pushed_address(), Some(new_return));
    assert_eq!(operation.stack_before(), &[old_return]);
    assert_eq!(operation.stack_after(), &[new_return]);
    assert_eq!(ras.stack_entries(), &[new_return]);
}

#[test]
fn return_address_stack_capacity_overwrites_oldest_entry() {
    let mut ras = ras(2);
    let first = Address::new(0x1004);
    let second = Address::new(0x2004);
    let third = Address::new(0x3004);

    ras.push_speculative(first);
    ras.push_speculative(second);
    let overflow = ras.push_speculative(third);

    assert_eq!(overflow.stack_before(), &[first, second]);
    assert_eq!(overflow.stack_after(), &[second, third]);
    assert_eq!(ras.stack_entries(), &[second, third]);
    assert_eq!(ras.top(), Some(third));
}

#[test]
fn return_address_stack_commit_rejects_out_of_order_and_unknown_records() {
    let mut ras = ras(4);
    let first = ras.push_speculative(Address::new(0x1004));
    let second = ras.push_speculative(Address::new(0x2004));

    assert_eq!(
        ras.commit_operation(second.id()),
        Err(ReturnAddressStackError::OutOfOrderOperationCommit {
            expected: first.id(),
            actual: second.id(),
        })
    );
    assert_eq!(
        ras.squash_from(ReturnAddressStackOperationId::new(99)),
        Err(ReturnAddressStackError::UnknownOperation {
            id: ReturnAddressStackOperationId::new(99),
        })
    );
}

#[test]
fn return_address_stack_snapshot_restore_preserves_stack_and_pending_operations() {
    let mut ras = ras(2);
    let first = ras.push_speculative(Address::new(0x1004));
    let second = ras.push_speculative(Address::new(0x2004));
    let snapshot = ras.snapshot();

    ras.pop_speculative();
    ras.commit_operation(first.id()).unwrap();

    ras.restore(&snapshot).unwrap();

    assert_eq!(
        ras.stack_entries(),
        &[Address::new(0x1004), Address::new(0x2004)]
    );
    assert_eq!(ras.pending_operations(), &[first, second]);
    assert_eq!(ras.next_operation(), ReturnAddressStackOperationId::new(2));
}

#[test]
fn return_address_stack_rejects_bad_config_and_snapshot_shape() {
    assert_eq!(
        ReturnAddressStackConfig::new(0),
        Err(ReturnAddressStackError::ZeroEntries)
    );

    let snapshot = ras(2).snapshot();
    let mut wider = ras(3);

    assert_eq!(
        wider.restore(&snapshot),
        Err(ReturnAddressStackError::SnapshotEntriesMismatch {
            expected: 3,
            actual: 2,
        })
    );
}

#[test]
fn branch_target_buffer_records_miss_update_hit_and_counters() {
    let mut btb = btb(4, 2);
    let pc = Address::new(0x1000);
    let target = Address::new(0x1080);

    let miss = btb.lookup(pc, BranchTargetKind::DirectConditional);

    assert!(!miss.hit());
    assert_eq!(miss.pc(), pc);
    assert_eq!(miss.target(), None);
    assert_eq!(miss.kind(), BranchTargetKind::DirectConditional);
    assert_eq!(miss.lookup_count(), 1);
    assert_eq!(
        btb.lookup_kind_counts()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(btb.miss_count(), 1);
    assert_eq!(
        btb.miss_kind_counts()
            .value(BranchTargetKind::DirectConditional),
        1
    );

    let update = btb.update(pc, target, BranchTargetKind::DirectConditional);

    assert_eq!(update.pc(), pc);
    assert_eq!(update.target(), target);
    assert_eq!(update.kind(), BranchTargetKind::DirectConditional);
    assert_eq!(update.replaced(), None);
    assert_eq!(update.update_count(), 1);
    assert_eq!(
        btb.update_kind_counts()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert!(btb.valid(pc));

    let hit = btb.lookup(pc, BranchTargetKind::DirectConditional);

    assert!(hit.hit());
    assert_eq!(hit.target(), Some(target));
    assert_eq!(hit.entry().unwrap().pc(), pc);
    assert_eq!(
        hit.entry().unwrap().kind(),
        BranchTargetKind::DirectConditional
    );
    assert_eq!(btb.lookup_count(), 2);
    assert_eq!(btb.hit_count(), 1);
    assert_eq!(
        btb.hit_kind_counts()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(btb.miss_count(), 1);
    assert_eq!(btb.lookup_kind_counts().total(), btb.lookup_count());
    assert_eq!(btb.hit_kind_counts().total(), btb.hit_count());
    assert_eq!(btb.miss_kind_counts().total(), btb.miss_count());
    assert_eq!(btb.update_kind_counts().total(), btb.update_count());
}

#[test]
fn branch_target_buffer_uses_set_associative_lru_replacement() {
    let mut btb = btb(4, 2);
    let first = Address::new(0x1000);
    let second = Address::new(0x1010);
    let third = Address::new(0x1020);

    btb.update(
        first,
        Address::new(0x1100),
        BranchTargetKind::DirectUnconditional,
    );
    btb.update(second, Address::new(0x1200), BranchTargetKind::CallDirect);
    btb.lookup(first, BranchTargetKind::DirectUnconditional);

    let replacement = btb.update(third, Address::new(0x1300), BranchTargetKind::Return);

    assert_eq!(replacement.replaced().unwrap().pc(), second);
    assert!(btb.valid(first));
    assert!(!btb.valid(second));
    assert!(btb.valid(third));
    assert_eq!(btb.eviction_count(), 1);
}

#[test]
fn branch_target_buffer_snapshot_restore_preserves_entries_and_counters() {
    let mut btb = btb(8, 2);
    let pc = Address::new(0x1000);
    let target = Address::new(0x1080);

    btb.lookup(pc, BranchTargetKind::IndirectConditional);
    btb.update(pc, target, BranchTargetKind::IndirectConditional);
    let snapshot = btb.snapshot();

    btb.invalidate();
    btb.update(
        Address::new(0x2000),
        Address::new(0x2080),
        BranchTargetKind::CallIndirect,
    );
    assert!(!btb.valid(pc));

    btb.restore(&snapshot).unwrap();

    assert!(btb.valid(pc));
    assert_eq!(btb.snapshot().entries(), snapshot.entries());
    assert_eq!(
        btb.lookup(pc, BranchTargetKind::IndirectConditional)
            .target(),
        Some(target)
    );
    assert_eq!(btb.miss_count(), 1);
    assert_eq!(btb.update_count(), 1);
    assert_eq!(
        btb.lookup_kind_counts()
            .value(BranchTargetKind::IndirectConditional),
        2
    );
    assert_eq!(
        btb.miss_kind_counts()
            .value(BranchTargetKind::IndirectConditional),
        1
    );
    assert_eq!(
        btb.update_kind_counts()
            .value(BranchTargetKind::IndirectConditional),
        1
    );
}

#[test]
fn branch_target_buffer_rejects_bad_config_and_snapshot_shape() {
    assert_eq!(
        BranchTargetBufferConfig::new(0, 1),
        Err(BranchTargetBufferError::ZeroEntries)
    );
    assert_eq!(
        BranchTargetBufferConfig::new(4, 0),
        Err(BranchTargetBufferError::ZeroAssociativity)
    );
    assert_eq!(
        BranchTargetBufferConfig::new(4, 8),
        Err(BranchTargetBufferError::AssociativityExceedsEntries {
            entries: 4,
            associativity: 8,
        })
    );
    assert_eq!(
        BranchTargetBufferConfig::new(6, 4),
        Err(
            BranchTargetBufferError::EntriesNotDivisibleByAssociativity {
                entries: 6,
                associativity: 4,
            }
        )
    );
    assert_eq!(
        BranchTargetBufferConfig::new(6, 2),
        Err(BranchTargetBufferError::SetCountNotPowerOfTwo { sets: 3 })
    );

    let snapshot = btb(4, 2).snapshot();
    let mut larger = btb(8, 2);

    assert_eq!(
        larger.restore(&snapshot),
        Err(BranchTargetBufferError::SnapshotShapeMismatch {
            expected_entries: 8,
            expected_associativity: 2,
            actual_entries: 4,
            actual_associativity: 2,
        })
    );
}
