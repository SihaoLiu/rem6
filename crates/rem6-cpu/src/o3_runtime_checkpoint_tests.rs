use super::*;

const BASE_AND_FU_STATS_BYTES: usize = (12 + O3RuntimeFuLatencyClass::COUNT * 2) * U64_BYTES;
const CURRENT_BASE_AND_FU_STATS_BYTES: usize = BASE_AND_FU_STATS_BYTES + U64_BYTES;
const LSQ_OPERATION_STATS_BYTES: usize = O3RuntimeLsqOperation::TRACKED.len() * U64_BYTES;
const LSQ_OPERATION_BYTE_STATS_BYTES: usize = O3RuntimeLsqOperation::TRACKED.len() * 2 * U64_BYTES;
const LSQ_OPERATION_FORWARDING_STATS_BYTES: usize =
    O3RuntimeLsqOperation::TRACKED.len() * 2 * U64_BYTES;
const LSQ_FORWARDING_SUPPRESSION_STATS_BYTES: usize =
    (1 + O3RuntimeLsqOperation::TRACKED.len()) * U64_BYTES;
const LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES: usize =
    (2 + O3RuntimeLsqOperation::TRACKED.len() * 2) * U64_BYTES;
const LSQ_OPERATION_LATENCY_STATS_BYTES: usize =
    O3RuntimeLsqOperation::TRACKED.len() * 4 * U64_BYTES;
const LSQ_DATA_LATENCY_STATS_BYTES: usize = 4 * U64_BYTES;
const LSQ_ORDERING_STATS_BYTES: usize = (O3RuntimeLsqOrdering::TRACKED.len() + 1) * U64_BYTES;
const BRANCH_REPAIR_STATS_BYTES: usize = (3 + crate::BranchTargetKind::COUNT * 3) * U64_BYTES;
const IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES: usize = 2 * U64_BYTES;
const IEW_DEPENDENCY_STATS_BYTES: usize = 2 * U64_BYTES;
const IQ_BRANCH_ISSUED_STATS_BYTES: usize = U64_BYTES;
const MAX_OCCUPANCY_STATS_BYTES: usize = 3 * U64_BYTES;
const BRANCH_EVENT_STATS_BYTES: usize = crate::BranchTargetKind::COUNT * 6 * U64_BYTES;
const BRANCH_EVENT_PREDICTION_STATS_BYTES: usize = crate::BranchTargetKind::COUNT * 4 * U64_BYTES;
const BRANCH_MISMATCH_STATS_BYTES: usize = crate::BranchTargetKind::COUNT * 16 * U64_BYTES;
const CURRENT_STATS_BYTES: usize = (15 + O3RuntimeFuLatencyClass::COUNT * 2) * U64_BYTES
    + LSQ_OPERATION_STATS_BYTES
    + LSQ_OPERATION_BYTE_STATS_BYTES
    + LSQ_OPERATION_FORWARDING_STATS_BYTES
    + LSQ_FORWARDING_SUPPRESSION_STATS_BYTES
    + LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES
    + LSQ_OPERATION_LATENCY_STATS_BYTES
    + LSQ_DATA_LATENCY_STATS_BYTES
    + LSQ_ORDERING_STATS_BYTES
    + BRANCH_REPAIR_STATS_BYTES
    + IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
    + IEW_DEPENDENCY_STATS_BYTES
    + IQ_BRANCH_ISSUED_STATS_BYTES
    + BRANCH_EVENT_STATS_BYTES
    + BRANCH_EVENT_PREDICTION_STATS_BYTES
    + BRANCH_MISMATCH_STATS_BYTES;
const STATS_BYTES_WITHOUT_BRANCH_MISMATCH: usize =
    CURRENT_STATS_BYTES - BRANCH_MISMATCH_STATS_BYTES;
const STATS_BYTES_WITHOUT_BRANCH_EVENT_PREDICTION: usize = STATS_BYTES_WITHOUT_BRANCH_MISMATCH
    - BRANCH_EVENT_PREDICTION_STATS_BYTES
    - LSQ_OPERATION_BYTE_STATS_BYTES;
const STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION_REASON: usize =
    STATS_BYTES_WITHOUT_BRANCH_EVENT_PREDICTION - LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES;
const PRE_BRANCH_EVENT_STATS_BYTES: usize = STATS_BYTES_WITHOUT_BRANCH_MISMATCH
    - LSQ_OPERATION_BYTE_STATS_BYTES
    - BRANCH_EVENT_PREDICTION_STATS_BYTES
    - BRANCH_EVENT_STATS_BYTES
    - LSQ_FORWARDING_SUPPRESSION_STATS_BYTES
    - LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES;

fn encoded_without_lsq_operation_byte_stats(
    encoded: &[u8],
    stats_bytes_with_lsq_operation_byte_stats: usize,
) -> Vec<u8> {
    let stats_offset = encoded
        .len()
        .checked_sub(stats_bytes_with_lsq_operation_byte_stats)
        .unwrap();
    let operation_byte_offset =
        stats_offset + CURRENT_BASE_AND_FU_STATS_BYTES + LSQ_OPERATION_STATS_BYTES;
    [
        &encoded[..operation_byte_offset],
        &encoded[operation_byte_offset + LSQ_OPERATION_BYTE_STATS_BYTES..],
    ]
    .concat()
}

fn encoded_without_branch_mismatch_stats(encoded: &[u8]) -> Vec<u8> {
    let mismatch_offset = encoded
        .len()
        .checked_sub(BRANCH_MISMATCH_STATS_BYTES)
        .unwrap();
    encoded[..mismatch_offset].to_vec()
}

fn encoded_without_branch_event_prediction_stats(encoded: &[u8]) -> Vec<u8> {
    let encoded = encoded_without_branch_mismatch_stats(encoded);
    let prediction_offset = encoded
        .len()
        .checked_sub(BRANCH_EVENT_PREDICTION_STATS_BYTES)
        .unwrap();
    encoded_without_lsq_operation_byte_stats(
        &encoded[..prediction_offset],
        STATS_BYTES_WITHOUT_BRANCH_MISMATCH - BRANCH_EVENT_PREDICTION_STATS_BYTES,
    )
}

fn encoded_without_branch_event_stats(encoded: &[u8]) -> Vec<u8> {
    let branch_event_offset = encoded.len().checked_sub(BRANCH_EVENT_STATS_BYTES).unwrap();
    encoded[..branch_event_offset].to_vec()
}

fn encoded_without_lsq_forwarding_suppression_stats(encoded: &[u8]) -> Vec<u8> {
    let encoded = encoded_without_lsq_forwarding_suppression_reason_stats(encoded);
    let stats_offset = encoded
        .len()
        .checked_sub(STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION_REASON)
        .unwrap();
    let operation_suppression_offset = stats_offset
        + CURRENT_BASE_AND_FU_STATS_BYTES
        + LSQ_OPERATION_STATS_BYTES
        + LSQ_OPERATION_FORWARDING_STATS_BYTES;
    let operation_suppression_bytes = LSQ_FORWARDING_SUPPRESSION_STATS_BYTES - U64_BYTES;
    let aggregate_suppression_offset = stats_offset + 10 * U64_BYTES;
    let without_operation_suppression = [
        &encoded[..operation_suppression_offset],
        &encoded[operation_suppression_offset + operation_suppression_bytes..],
    ]
    .concat();
    [
        &without_operation_suppression[..aggregate_suppression_offset],
        &without_operation_suppression[aggregate_suppression_offset + U64_BYTES..],
    ]
    .concat()
}

fn encoded_without_lsq_forwarding_suppression_reason_stats(encoded: &[u8]) -> Vec<u8> {
    let encoded = encoded_without_branch_event_prediction_stats(encoded);
    let stats_offset = encoded
        .len()
        .checked_sub(STATS_BYTES_WITHOUT_BRANCH_EVENT_PREDICTION)
        .unwrap();
    let operation_suppression_bytes = LSQ_FORWARDING_SUPPRESSION_STATS_BYTES - U64_BYTES;
    let reason_offset = stats_offset
        + CURRENT_BASE_AND_FU_STATS_BYTES
        + LSQ_OPERATION_STATS_BYTES
        + LSQ_OPERATION_FORWARDING_STATS_BYTES
        + operation_suppression_bytes;
    [
        &encoded[..reason_offset],
        &encoded[reason_offset + LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES..],
    ]
    .concat()
}

#[test]
fn checkpoint_v6_payloads_decode_without_aggregate_lsq_data_latency_stats() {
    let operation = O3RuntimeLsqOperation::StoreConditional;
    let mut operation_samples = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_ticks = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_max_ticks = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_min_ticks = [0; O3RuntimeLsqOperation::COUNT];
    operation_samples[operation.index()] = 2;
    operation_ticks[operation.index()] = 11;
    operation_max_ticks[operation.index()] = 6;
    operation_min_ticks[operation.index()] = 5;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            lsq_data_latency_samples: 2,
            lsq_data_latency_ticks: 11,
            lsq_data_latency_max_ticks: 6,
            lsq_data_latency_min_ticks: 5,
            lsq_operation_latency_samples: operation_samples,
            lsq_operation_latency_ticks: operation_ticks,
            lsq_operation_latency_max_ticks: operation_max_ticks,
            lsq_operation_latency_min_ticks: operation_min_ticks,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let encoded = encoded_without_branch_event_stats(
        &encoded_without_lsq_forwarding_suppression_stats(&payload.encode()),
    );
    let stats_offset = encoded
        .len()
        .checked_sub(PRE_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let forwarding_offset = stats_offset + BASE_AND_FU_STATS_BYTES + LSQ_OPERATION_STATS_BYTES;
    let data_latency_offset = stats_offset
        + BASE_AND_FU_STATS_BYTES
        + LSQ_OPERATION_STATS_BYTES
        + LSQ_OPERATION_FORWARDING_STATS_BYTES
        + LSQ_OPERATION_LATENCY_STATS_BYTES;
    let split_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - IEW_DEPENDENCY_STATS_BYTES
        - IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let dependency_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - IEW_DEPENDENCY_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let iq_branch_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let mut v6_encoded = [
        &encoded[..forwarding_offset],
        &encoded[forwarding_offset + LSQ_OPERATION_FORWARDING_STATS_BYTES..data_latency_offset],
        &encoded[data_latency_offset + LSQ_DATA_LATENCY_STATS_BYTES..split_offset],
        &encoded[split_offset + IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES..dependency_offset],
        &encoded[dependency_offset + IEW_DEPENDENCY_STATS_BYTES..iq_branch_offset],
        &encoded[iq_branch_offset + IQ_BRANCH_ISSUED_STATS_BYTES..],
    ]
    .concat();
    v6_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_LATENCY_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v6_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.lsq_operation_latency_samples(operation), 2);
    assert_eq!(stats.lsq_operation_latency_ticks(operation), 11);
    assert_eq!(stats.lsq_operation_latency_max_ticks(operation), 6);
    assert_eq!(stats.lsq_operation_latency_min_ticks(operation), 5);
    assert_eq!(stats.lsq_operation_latency_avg_ticks(operation), 5);
    assert_eq!(stats.lsq_data_latency_samples(), 0);
    assert_eq!(stats.lsq_data_latency_ticks(), 0);
    assert_eq!(stats.lsq_data_latency_max_ticks(), 0);
    assert_eq!(stats.lsq_data_latency_min_ticks(), 0);
    assert_eq!(stats.lsq_data_latency_avg_ticks(), 0);
}

#[test]
fn checkpoint_v7_payloads_decode_without_iew_branch_mispredict_split_stats() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            branch_repair_targetless_mismatches: 2,
            branch_repair_wrong_targets: 3,
            branch_repair_direction_only_mismatches: 4,
            iew_predicted_taken_incorrect: 3,
            iew_predicted_not_taken_incorrect: 5,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let encoded = encoded_without_branch_event_stats(
        &encoded_without_lsq_forwarding_suppression_stats(&payload.encode()),
    );
    let stats_offset = encoded
        .len()
        .checked_sub(PRE_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let forwarding_offset = stats_offset + BASE_AND_FU_STATS_BYTES + LSQ_OPERATION_STATS_BYTES;
    let split_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - IEW_DEPENDENCY_STATS_BYTES
        - IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let dependency_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - IEW_DEPENDENCY_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let iq_branch_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let mut v7_encoded = [
        &encoded[..forwarding_offset],
        &encoded[forwarding_offset + LSQ_OPERATION_FORWARDING_STATS_BYTES..split_offset],
        &encoded[split_offset + IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES..dependency_offset],
        &encoded[dependency_offset + IEW_DEPENDENCY_STATS_BYTES..iq_branch_offset],
        &encoded[iq_branch_offset + IQ_BRANCH_ISSUED_STATS_BYTES..],
    ]
    .concat();
    v7_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_DATA_LATENCY_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v7_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.branch_repair_targetless_mismatches(), 2);
    assert_eq!(stats.branch_repair_wrong_targets(), 3);
    assert_eq!(stats.branch_repair_direction_only_mismatches(), 4);
    assert_eq!(stats.branch_repair_mispredicts(), 9);
    assert_eq!(stats.iew_predicted_taken_incorrect(), 0);
    assert_eq!(stats.iew_predicted_not_taken_incorrect(), 0);
}

#[test]
fn checkpoint_v8_payloads_decode_without_iew_dependency_stats() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            iew_producer_insts: 3,
            iew_consumer_insts: 4,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let encoded = encoded_without_branch_event_stats(
        &encoded_without_lsq_forwarding_suppression_stats(&payload.encode()),
    );
    let stats_offset = encoded
        .len()
        .checked_sub(PRE_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let forwarding_offset = stats_offset + BASE_AND_FU_STATS_BYTES + LSQ_OPERATION_STATS_BYTES;
    let dependency_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - IEW_DEPENDENCY_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let iq_branch_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let mut v8_encoded = [
        &encoded[..forwarding_offset],
        &encoded[forwarding_offset + LSQ_OPERATION_FORWARDING_STATS_BYTES..dependency_offset],
        &encoded[dependency_offset + IEW_DEPENDENCY_STATS_BYTES..iq_branch_offset],
        &encoded[iq_branch_offset + IQ_BRANCH_ISSUED_STATS_BYTES..],
    ]
    .concat();
    v8_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_BRANCH_MISPREDICT_SPLIT_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v8_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iew_producer_insts(), 0);
    assert_eq!(stats.iew_consumer_insts(), 0);
}

#[test]
fn checkpoint_v9_payloads_decode_without_iq_branch_issued_stats() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let encoded = encoded_without_branch_event_stats(
        &encoded_without_lsq_forwarding_suppression_stats(&payload.encode()),
    );
    let stats_offset = encoded
        .len()
        .checked_sub(PRE_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let forwarding_offset = stats_offset + BASE_AND_FU_STATS_BYTES + LSQ_OPERATION_STATS_BYTES;
    let iq_branch_offset = stats_offset + PRE_BRANCH_EVENT_STATS_BYTES
        - IQ_BRANCH_ISSUED_STATS_BYTES
        - MAX_OCCUPANCY_STATS_BYTES;
    let mut v9_encoded = [
        &encoded[..forwarding_offset],
        &encoded[forwarding_offset + LSQ_OPERATION_FORWARDING_STATS_BYTES..iq_branch_offset],
        &encoded[iq_branch_offset + IQ_BRANCH_ISSUED_STATS_BYTES..],
    ]
    .concat();
    v9_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_IEW_DEPENDENCY_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v9_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 0);
}

#[test]
fn checkpoint_v10_payloads_decode_without_lsq_operation_forwarding_stats() {
    let operation = O3RuntimeLsqOperation::Load;
    let mut operation_counts = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_forwarding_candidates = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_forwarding_matches = [0; O3RuntimeLsqOperation::COUNT];
    operation_counts[operation.index()] = 2;
    operation_forwarding_candidates[operation.index()] = 1;
    operation_forwarding_matches[operation.index()] = 1;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            lsq_store_to_load_forwarding_candidates: 1,
            lsq_store_to_load_forwarding_matches: 1,
            lsq_operation_counts: operation_counts,
            lsq_operation_forwarding_candidates: operation_forwarding_candidates,
            lsq_operation_forwarding_matches: operation_forwarding_matches,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let encoded = encoded_without_branch_event_stats(
        &encoded_without_lsq_forwarding_suppression_stats(&payload.encode()),
    );
    let stats_offset = encoded
        .len()
        .checked_sub(PRE_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let forwarding_offset = stats_offset + BASE_AND_FU_STATS_BYTES + LSQ_OPERATION_STATS_BYTES;
    let mut v10_encoded = [
        &encoded[..forwarding_offset],
        &encoded[forwarding_offset + LSQ_OPERATION_FORWARDING_STATS_BYTES..],
    ]
    .concat();
    v10_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_IQ_BRANCH_ISSUED_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v10_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.lsq_operation_count(operation), 2);
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 1);
    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.lsq_operation_forwarding_candidates(operation), 0);
    assert_eq!(stats.lsq_operation_forwarding_matches(operation), 0);
}

#[test]
fn checkpoint_v11_payloads_decode_without_branch_event_stats() {
    let mut branch_event_kinds = [0; crate::BranchTargetKind::COUNT];
    branch_event_kinds[crate::BranchTargetKind::Return.index()] = 1;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            branch_event_kinds,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let mut v11_encoded = encoded_without_branch_event_stats(
        &encoded_without_lsq_forwarding_suppression_stats(&payload.encode()),
    );
    v11_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_MATRIX_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v11_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.branch_event_kind(crate::BranchTargetKind::Return), 0);
}

#[test]
fn checkpoint_v12_payloads_decode_without_lsq_forwarding_suppression_stats() {
    let operation = O3RuntimeLsqOperation::Load;
    let mut branch_event_kinds = [0; crate::BranchTargetKind::COUNT];
    branch_event_kinds[crate::BranchTargetKind::Return.index()] = 1;
    let mut operation_suppressed = [0; O3RuntimeLsqOperation::COUNT];
    operation_suppressed[operation.index()] = 1;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            branch_event_kinds,
            lsq_store_to_load_forwarding_suppressed: 1,
            lsq_operation_forwarding_suppressed: operation_suppressed,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let mut v12_encoded = encoded_without_lsq_forwarding_suppression_stats(&payload.encode());
    v12_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v12_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.branch_event_kind(crate::BranchTargetKind::Return), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 0);
    assert_eq!(stats.lsq_operation_forwarding_suppressed(operation), 0);
}

#[test]
fn checkpoint_v13_payloads_decode_without_lsq_forwarding_suppression_reason_stats() {
    let operation = O3RuntimeLsqOperation::Load;
    let mut branch_event_kinds = [0; crate::BranchTargetKind::COUNT];
    branch_event_kinds[crate::BranchTargetKind::Return.index()] = 1;
    let mut operation_suppressed = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_address_mismatches = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_byte_mismatches = [0; O3RuntimeLsqOperation::COUNT];
    operation_suppressed[operation.index()] = 1;
    operation_address_mismatches[operation.index()] = 1;
    operation_byte_mismatches[operation.index()] = 2;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            branch_event_kinds,
            lsq_store_to_load_forwarding_suppressed: 1,
            lsq_store_to_load_forwarding_address_mismatches: 1,
            lsq_store_to_load_forwarding_byte_mismatches: 2,
            lsq_operation_forwarding_suppressed: operation_suppressed,
            lsq_operation_forwarding_address_mismatches: operation_address_mismatches,
            lsq_operation_forwarding_byte_mismatches: operation_byte_mismatches,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let mut v13_encoded =
        encoded_without_lsq_forwarding_suppression_reason_stats(&payload.encode());
    v13_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v13_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.branch_event_kind(crate::BranchTargetKind::Return), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 1);
    assert_eq!(stats.lsq_operation_forwarding_suppressed(operation), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_address_mismatches(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_byte_mismatches(), 0);
    assert_eq!(
        stats.lsq_operation_forwarding_address_mismatches(operation),
        0
    );
    assert_eq!(stats.lsq_operation_forwarding_byte_mismatches(operation), 0);
}

#[test]
fn checkpoint_v14_payloads_decode_without_branch_event_prediction_stats() {
    let mut branch_event_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_taken_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_match_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_mismatch_kinds = [0; crate::BranchTargetKind::COUNT];
    branch_event_kinds[crate::BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_taken_kinds[crate::BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_target_kinds[crate::BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_target_match_kinds[crate::BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_target_mismatch_kinds[crate::BranchTargetKind::CallIndirect.index()] = 2;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            branch_event_kinds,
            branch_event_predicted_taken_kinds,
            branch_event_predicted_target_kinds,
            branch_event_predicted_target_match_kinds,
            branch_event_predicted_target_mismatch_kinds,
            lsq_store_to_load_forwarding_address_mismatches: 1,
            lsq_store_to_load_forwarding_byte_mismatches: 2,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let mut v14_encoded = encoded_without_branch_event_prediction_stats(&payload.encode());
    v14_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_FORWARDING_SUPPRESSION_REASON_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v14_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.branch_event_kind(crate::BranchTargetKind::Return), 1);
    assert_eq!(
        stats.branch_event_predicted_taken_kind(crate::BranchTargetKind::Return),
        0
    );
    assert_eq!(
        stats.branch_event_predicted_not_taken_kind(crate::BranchTargetKind::Return),
        1
    );
    assert_eq!(
        stats.branch_event_predicted_target_kind(crate::BranchTargetKind::Return),
        0
    );
    assert_eq!(
        stats.branch_event_predicted_target_match_kind(crate::BranchTargetKind::Return),
        0
    );
    assert_eq!(
        stats.branch_event_predicted_target_mismatch_kind(crate::BranchTargetKind::CallIndirect),
        0
    );
    assert_eq!(stats.lsq_store_to_load_forwarding_address_mismatches(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_byte_mismatches(), 2);
}

#[test]
fn checkpoint_v17_payloads_round_trip_branch_mismatch_stats() {
    let mut direction_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut direction_without_link = [0; crate::BranchTargetKind::COUNT];
    let mut direction_squashed = [0; crate::BranchTargetKind::COUNT];
    let mut direction_squashed_without_link = [0; crate::BranchTargetKind::COUNT];
    let mut targetless_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut targetless_without_link = [0; crate::BranchTargetKind::COUNT];
    let mut targetless_squashed = [0; crate::BranchTargetKind::COUNT];
    let mut targetless_squashed_without_link = [0; crate::BranchTargetKind::COUNT];
    let mut wrong_target_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut wrong_target_link = [0; crate::BranchTargetKind::COUNT];
    let mut wrong_target_squashed = [0; crate::BranchTargetKind::COUNT];
    let mut wrong_target_squashed_link = [0; crate::BranchTargetKind::COUNT];
    direction_kinds[crate::BranchTargetKind::DirectUnconditional.index()] = 2;
    direction_without_link[crate::BranchTargetKind::DirectUnconditional.index()] = 2;
    direction_squashed[crate::BranchTargetKind::DirectUnconditional.index()] = 2;
    direction_squashed_without_link[crate::BranchTargetKind::DirectUnconditional.index()] = 2;
    targetless_kinds[crate::BranchTargetKind::DirectConditional.index()] = 3;
    targetless_without_link[crate::BranchTargetKind::DirectConditional.index()] = 3;
    targetless_squashed[crate::BranchTargetKind::DirectConditional.index()] = 3;
    targetless_squashed_without_link[crate::BranchTargetKind::DirectConditional.index()] = 3;
    wrong_target_kinds[crate::BranchTargetKind::CallIndirect.index()] = 4;
    wrong_target_link[crate::BranchTargetKind::CallIndirect.index()] = 4;
    wrong_target_squashed[crate::BranchTargetKind::CallIndirect.index()] = 4;
    wrong_target_squashed_link[crate::BranchTargetKind::CallIndirect.index()] = 4;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            branch_direction_mismatch_kinds: direction_kinds,
            branch_direction_mismatch_without_link_write_kinds: direction_without_link,
            branch_direction_mismatch_squashed_target_kinds: direction_squashed,
            branch_direction_mismatch_squashed_target_without_link_write_kinds:
                direction_squashed_without_link,
            branch_target_mismatch_targetless_kinds: targetless_kinds,
            branch_target_mismatch_targetless_without_link_write_kinds: targetless_without_link,
            branch_target_mismatch_targetless_squashed_target_kinds: targetless_squashed,
            branch_target_mismatch_targetless_squashed_target_without_link_write_kinds:
                targetless_squashed_without_link,
            branch_target_mismatch_wrong_target_kinds: wrong_target_kinds,
            branch_target_mismatch_wrong_target_link_write_kinds: wrong_target_link,
            branch_target_mismatch_wrong_target_squashed_target_kinds: wrong_target_squashed,
            branch_target_mismatch_wrong_target_squashed_target_link_write_kinds:
                wrong_target_squashed_link,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();

    let decoded = O3RuntimeCheckpointPayload::decode(&payload.encode()).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.branch_direction_mismatches(), 2);
    assert_eq!(
        stats.branch_direction_mismatch_squashed_target_without_link_write_kind(
            crate::BranchTargetKind::DirectUnconditional
        ),
        2
    );
    assert_eq!(stats.branch_target_mismatch_targetless_mismatches(), 3);
    assert_eq!(
        stats.branch_target_mismatch_targetless_squashed_target_without_link_write_kind(
            crate::BranchTargetKind::DirectConditional
        ),
        3
    );
    assert_eq!(stats.branch_target_mismatch_wrong_targets(), 4);
    assert_eq!(stats.branch_target_mismatch_wrong_target_link_writes(), 4);
    assert_eq!(
        stats.branch_target_mismatch_wrong_target_squashed_target_link_write_kind(
            crate::BranchTargetKind::CallIndirect
        ),
        4
    );
}

#[test]
fn checkpoint_v16_payloads_decode_without_branch_mismatch_stats() {
    let operation = O3RuntimeLsqOperation::Atomic;
    let mut operation_load_bytes = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_store_bytes = [0; O3RuntimeLsqOperation::COUNT];
    let mut branch_event_predicted_target_mismatch_kinds = [0; crate::BranchTargetKind::COUNT];
    let mut branch_direction_mismatch_kinds = [0; crate::BranchTargetKind::COUNT];
    operation_load_bytes[operation.index()] = 8;
    operation_store_bytes[operation.index()] = 8;
    branch_event_predicted_target_mismatch_kinds[crate::BranchTargetKind::CallIndirect.index()] = 2;
    branch_direction_mismatch_kinds[crate::BranchTargetKind::DirectUnconditional.index()] = 9;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            lsq_load_bytes: 8,
            lsq_store_bytes: 8,
            lsq_operation_load_bytes: operation_load_bytes,
            lsq_operation_store_bytes: operation_store_bytes,
            branch_event_predicted_target_mismatch_kinds,
            branch_direction_mismatch_kinds,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let mut v16_encoded = encoded_without_branch_mismatch_stats(&payload.encode());
    v16_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_LSQ_OPERATION_BYTE_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v16_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.lsq_operation_load_bytes(operation), 8);
    assert_eq!(stats.lsq_operation_store_bytes(operation), 8);
    assert_eq!(
        stats.branch_event_predicted_target_mismatch_kind(crate::BranchTargetKind::CallIndirect),
        2
    );
    assert_eq!(stats.branch_direction_mismatches(), 0);
    assert_eq!(stats.branch_target_mismatch_targetless_mismatches(), 0);
    assert_eq!(stats.branch_target_mismatch_wrong_targets(), 0);
}

#[test]
fn checkpoint_v15_payloads_decode_without_lsq_operation_byte_stats() {
    let operation = O3RuntimeLsqOperation::Atomic;
    let mut operation_counts = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_load_bytes = [0; O3RuntimeLsqOperation::COUNT];
    let mut operation_store_bytes = [0; O3RuntimeLsqOperation::COUNT];
    let mut branch_event_predicted_taken_kinds = [0; crate::BranchTargetKind::COUNT];
    operation_counts[operation.index()] = 2;
    operation_load_bytes[operation.index()] = 8;
    operation_store_bytes[operation.index()] = 8;
    branch_event_predicted_taken_kinds[crate::BranchTargetKind::Return.index()] = 1;
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            lsq_load_bytes: 8,
            lsq_store_bytes: 8,
            lsq_operation_counts: operation_counts,
            lsq_operation_load_bytes: operation_load_bytes,
            lsq_operation_store_bytes: operation_store_bytes,
            branch_event_predicted_taken_kinds,
            iq_branch_insts_issued: 3,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let v16_encoded = encoded_without_branch_mismatch_stats(&payload.encode());
    let mut v15_encoded =
        encoded_without_lsq_operation_byte_stats(&v16_encoded, STATS_BYTES_WITHOUT_BRANCH_MISMATCH);
    v15_encoded[O3_RUNTIME_CHECKPOINT_MAGIC.len()] =
        O3_RUNTIME_CHECKPOINT_VERSION_WITH_BRANCH_EVENT_PREDICTION_STATS;

    let decoded = O3RuntimeCheckpointPayload::decode(&v15_encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.iq_branch_insts_issued(), 3);
    assert_eq!(stats.lsq_load_bytes(), 8);
    assert_eq!(stats.lsq_store_bytes(), 8);
    assert_eq!(stats.lsq_operation_count(operation), 2);
    assert_eq!(stats.lsq_operation_load_bytes(operation), 0);
    assert_eq!(stats.lsq_operation_store_bytes(operation), 0);
    assert_eq!(
        stats.branch_event_predicted_taken_kind(crate::BranchTargetKind::Return),
        1
    );
}
