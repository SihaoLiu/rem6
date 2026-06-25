use rem6_cpu::{
    BranchPredictor, BranchPredictorCheckpointPayload, BranchPredictorConfig, BranchPredictorError,
    BranchSpeculationId, BranchTargetBuffer, BranchTargetBufferConfig, BranchTargetBufferError,
    BranchTargetKind, BranchTargetSafetyConfig, BranchTargetSafetyProfile, ReturnAddressStack,
    ReturnAddressStackConfig, ReturnAddressStackError, ReturnAddressStackOperationId,
    ReturnAddressStackOperationKind, DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY,
    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
};
use rem6_memory::Address;

fn predictor(entries: usize) -> BranchPredictor {
    BranchPredictor::new(BranchPredictorConfig::new(entries).unwrap())
}

fn ras(entries: usize) -> ReturnAddressStack {
    ReturnAddressStack::new(ReturnAddressStackConfig::new(entries).unwrap())
}

fn btb(entries: usize, associativity: usize) -> BranchTargetBuffer {
    BranchTargetBuffer::new(BranchTargetBufferConfig::new(entries, associativity).unwrap())
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

    let payload = BranchPredictorCheckpointPayload::from_snapshots(
        snapshot.clone(),
        branch_target_buffer_snapshot.clone(),
        [(22, second.id()), (21, first.id())],
    )
    .unwrap();

    assert_eq!(
        payload.active_speculations(),
        &[(21, first.id()), (22, second.id())]
    );

    let decoded = BranchPredictorCheckpointPayload::decode(&payload.encode()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(
        decoded.branch_target_buffer_snapshot(),
        &branch_target_buffer_snapshot
    );
    assert_eq!(decoded.active_speculations(), payload.active_speculations());
}

#[test]
fn checkpoint_payload_decodes_legacy_v1_with_default_btb_snapshot() {
    const VERSION_OFFSET: usize = 4;
    const HEADER_BYTES: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4 * 2;
    const LEGACY_COUNTER_BYTES: usize = 8;
    const LEGACY_TARGET_BYTES: usize = 8 * (1 + 8);
    let snapshot = predictor(8).snapshot();
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        snapshot.clone(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();
    encoded[VERSION_OFFSET] = 1;
    encoded.truncate(HEADER_BYTES + LEGACY_COUNTER_BYTES + LEGACY_TARGET_BYTES);

    let decoded = BranchPredictorCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(decoded.active_speculations(), &[]);
    assert_eq!(
        decoded.branch_target_buffer_snapshot().config().entries(),
        DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES
    );
    assert_eq!(
        decoded
            .branch_target_buffer_snapshot()
            .config()
            .associativity(),
        DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY
    );
    assert_eq!(decoded.branch_target_buffer_snapshot().lookup_count(), 0);
    assert_eq!(decoded.branch_target_buffer_snapshot().hit_count(), 0);
    assert!(decoded
        .branch_target_buffer_snapshot()
        .entries()
        .iter()
        .all(Option::is_none));
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
    const HEADER_BYTES: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4 * 2;
    const CHECKPOINT_TARGET_BYTES: usize = 1 + 8;
    const BTB_HEADER_BYTES: usize = 4 * 2 + 8 * 6;
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
    let first_btb_entry_pc =
        HEADER_BYTES + 8 + 8 * CHECKPOINT_TARGET_BYTES + BTB_HEADER_BYTES + BTB_ENTRY_VALID_BYTES;
    encoded[first_btb_entry_pc..first_btb_entry_pc + 8].copy_from_slice(&0x1004_u64.to_le_bytes());

    assert!(matches!(
        BranchPredictorCheckpointPayload::decode(&encoded),
        Err(BranchPredictorError::InvalidBranchTargetBufferCheckpoint { .. })
    ));
}

#[test]
fn checkpoint_payload_rejects_duplicate_btb_entry_pc() {
    const HEADER_BYTES: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4 * 2;
    const CHECKPOINT_TARGET_BYTES: usize = 1 + 8;
    const BTB_HEADER_BYTES: usize = 4 * 2 + 8 * 6;
    const BTB_ENTRY_BYTES: usize = 1 + 8 + 8 + 1 + 8;
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
    let first_btb_entry = HEADER_BYTES + 8 + 8 * CHECKPOINT_TARGET_BYTES + BTB_HEADER_BYTES;
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
fn checkpoint_payload_rejects_v2_pending_count_before_allocation() {
    const HEADER_BYTES: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4 * 2;
    const PENDING_COUNT_OFFSET: usize = 4 + 1 + 4 + 1 + 8 * 4;
    const COUNTER_BYTES: usize = 8;
    const TARGET_BYTES: usize = 8 * (1 + 8);
    const PENDING_SPECULATION_BYTES: usize = 49;
    const BTB_HEADER_BYTES: usize = 4 * 2 + 8 * 6;
    let payload = BranchPredictorCheckpointPayload::from_snapshot(
        predictor(8).snapshot(),
        std::iter::empty::<(u64, BranchSpeculationId)>(),
    )
    .unwrap();
    let mut encoded = payload.encode();
    let pending_count = 100_u32;
    let expected = HEADER_BYTES
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
fn checkpoint_payload_rejects_v2_active_count_before_allocation() {
    const ACTIVE_COUNT_OFFSET: usize = 4 + 1 + 4 + 1 + 8 * 4 + 4;
    const ACTIVE_SPECULATION_BYTES: usize = 16;
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
    const ACTIVE_SPECULATION_BYTES: usize = 16;
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

    encoded[active_start + 8..active_start + 16].copy_from_slice(&second.id().get().to_le_bytes());
    encoded[active_start + 24..active_start + 32].copy_from_slice(&first.id().get().to_le_bytes());

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
    assert_eq!(btb.miss_count(), 1);

    let update = btb.update(pc, target, BranchTargetKind::DirectConditional);

    assert_eq!(update.pc(), pc);
    assert_eq!(update.target(), target);
    assert_eq!(update.kind(), BranchTargetKind::DirectConditional);
    assert_eq!(update.replaced(), None);
    assert_eq!(update.update_count(), 1);
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
    assert_eq!(btb.miss_count(), 1);
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
