use crate::branch_predictor::BranchTargetKind;

use super::{O3RuntimeError, O3RuntimeStats};

const U64_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct O3RuntimeBranchMismatchCheckpointStats {
    pub(super) branch_direction_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_direction_mismatch_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_direction_mismatch_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_direction_mismatch_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_direction_mismatch_squashed_target_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_direction_mismatch_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_targetless_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_targetless_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_targetless_squashed_target_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_targetless_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_wrong_target_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_wrong_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_wrong_target_squashed_target_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_wrong_target_squashed_target_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(super) branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
}

pub(super) fn write_o3_runtime_branch_mismatch_stats(payload: &mut Vec<u8>, stats: O3RuntimeStats) {
    for count in [
        O3RuntimeStats::branch_direction_mismatch_kind,
        O3RuntimeStats::branch_direction_mismatch_link_write_kind,
        O3RuntimeStats::branch_direction_mismatch_without_link_write_kind,
        O3RuntimeStats::branch_direction_mismatch_squashed_target_kind,
        O3RuntimeStats::branch_direction_mismatch_squashed_target_link_write_kind,
        O3RuntimeStats::branch_direction_mismatch_squashed_target_without_link_write_kind,
        O3RuntimeStats::branch_target_mismatch_targetless_kind,
        O3RuntimeStats::branch_target_mismatch_targetless_without_link_write_kind,
        O3RuntimeStats::branch_target_mismatch_targetless_squashed_target_kind,
        O3RuntimeStats::branch_target_mismatch_targetless_squashed_target_without_link_write_kind,
        O3RuntimeStats::branch_target_mismatch_wrong_target_kind,
        O3RuntimeStats::branch_target_mismatch_wrong_target_link_write_kind,
        O3RuntimeStats::branch_target_mismatch_wrong_target_without_link_write_kind,
        O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_kind,
        O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_link_write_kind,
        O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_without_link_write_kind,
    ] {
        write_branch_kind_counts(payload, stats, count);
    }
}

pub(super) fn read_o3_runtime_branch_mismatch_stats(
    payload: &[u8],
    offset: &mut usize,
) -> Result<O3RuntimeBranchMismatchCheckpointStats, O3RuntimeError> {
    let mut stats = O3RuntimeBranchMismatchCheckpointStats::default();
    read_branch_kind_counts(payload, offset, &mut stats.branch_direction_mismatch_kinds)?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_direction_mismatch_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_direction_mismatch_without_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_direction_mismatch_squashed_target_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_direction_mismatch_squashed_target_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_direction_mismatch_squashed_target_without_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_targetless_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_targetless_without_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_targetless_squashed_target_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_targetless_squashed_target_without_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_wrong_target_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_wrong_target_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_wrong_target_without_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_wrong_target_squashed_target_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_wrong_target_squashed_target_link_write_kinds,
    )?;
    read_branch_kind_counts(
        payload,
        offset,
        &mut stats.branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds,
    )?;
    Ok(stats)
}

fn write_branch_kind_counts<F>(payload: &mut Vec<u8>, stats: O3RuntimeStats, count: F)
where
    F: Fn(O3RuntimeStats, BranchTargetKind) -> u64,
{
    for kind in BranchTargetKind::ALL {
        payload.extend_from_slice(&count(stats, kind).to_le_bytes());
    }
}

fn read_branch_kind_counts(
    payload: &[u8],
    offset: &mut usize,
    counts: &mut [u64; BranchTargetKind::COUNT],
) -> Result<(), O3RuntimeError> {
    for kind in BranchTargetKind::ALL {
        counts[kind.index()] = read_u64(payload, offset)?;
    }
    Ok(())
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, O3RuntimeError> {
    let end = offset.saturating_add(U64_BYTES);
    if end > payload.len() {
        return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        });
    }
    let bytes = payload[*offset..end]
        .try_into()
        .expect("O3 runtime checkpoint u64 slice width is fixed");
    *offset = end;
    Ok(u64::from_le_bytes(bytes))
}
