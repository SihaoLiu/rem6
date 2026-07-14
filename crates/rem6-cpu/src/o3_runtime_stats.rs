use rem6_isa_riscv::{MemoryAccessKind, RegisterWrite};
use rem6_memory::MemoryRequestId;

use crate::branch_predictor::BranchTargetKind;
use crate::o3_runtime_trace::{
    O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeTraceRecord,
};
use crate::riscv_execution_event::RiscvCpuExecutionEvent;
use crate::riscv_fu_latency::riscv_o3_fu_latency_class as o3_fu_latency_class;

use super::o3_store_forwarding::{
    o3_load_forwarding_access, o3_load_register_value, o3_store_load_composition,
    O3StoreForwardingEntry, O3StoreLoadForwardingPlan, O3StoreLoadRelation,
    O3StoreLoadSuppressionReason,
};
use super::{
    o3_lsq_access_bytes, o3_lsq_access_counts, o3_lsq_operation, o3_lsq_ordering,
    o3_rename_write_count, O3PendingLoadForwardingMatch, O3StoreForwardingObservation,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct O3RuntimeStats {
    pub(crate) instructions: u64,
    pub(crate) rob_allocations: u64,
    pub(crate) rob_commits: u64,
    pub(crate) rename_writes: u64,
    pub(crate) lsq_loads: u64,
    pub(crate) lsq_stores: u64,
    pub(crate) lsq_load_bytes: u64,
    pub(crate) lsq_store_bytes: u64,
    pub(crate) lsq_store_to_load_forwarding_candidates: u64,
    pub(crate) lsq_store_to_load_forwarding_matches: u64,
    pub(crate) lsq_store_to_load_forwarding_suppressed: u64,
    pub(crate) lsq_store_to_load_forwarding_address_mismatches: u64,
    pub(crate) lsq_store_to_load_forwarding_byte_mismatches: u64,
    pub(crate) lsq_operation_counts: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_load_bytes: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_store_bytes: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_forwarding_candidates: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_forwarding_matches: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_forwarding_suppressed: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_forwarding_address_mismatches: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_forwarding_byte_mismatches: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_data_latency_samples: u64,
    pub(crate) lsq_data_latency_ticks: u64,
    pub(crate) lsq_data_latency_max_ticks: u64,
    pub(crate) lsq_data_latency_min_ticks: u64,
    pub(crate) lsq_operation_latency_samples: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_latency_ticks: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_latency_max_ticks: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_operation_latency_min_ticks: [u64; O3RuntimeLsqOperation::COUNT],
    pub(crate) lsq_ordering_counts: [u64; O3RuntimeLsqOrdering::COUNT],
    pub(crate) lsq_store_conditional_failures: u64,
    pub(crate) branch_repair_targetless_mismatches: u64,
    pub(crate) branch_repair_wrong_targets: u64,
    pub(crate) branch_repair_direction_only_mismatches: u64,
    pub(crate) branch_repair_targetless_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_repair_wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_repair_direction_only_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_direction_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_direction_mismatch_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_direction_mismatch_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_direction_mismatch_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_direction_mismatch_squashed_target_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_direction_mismatch_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_targetless_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_targetless_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_targetless_squashed_target_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_targetless_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_wrong_target_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_wrong_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_wrong_target_squashed_target_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_wrong_target_squashed_target_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_taken_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_predicted_taken_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_predicted_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_predicted_target_match_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_predicted_target_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_resolved_target_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_link_write_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_squash_kinds: [u64; BranchTargetKind::COUNT],
    pub(crate) branch_event_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    pub(crate) iew_predicted_taken_incorrect: u64,
    pub(crate) iew_predicted_not_taken_incorrect: u64,
    pub(crate) iew_producer_insts: u64,
    pub(crate) iew_consumer_insts: u64,
    pub(crate) fu_latency_instructions: u64,
    pub(crate) fu_latency_cycles: u64,
    pub(crate) fu_latency_class_instructions: [u64; O3RuntimeFuLatencyClass::COUNT],
    pub(crate) fu_latency_class_cycles: [u64; O3RuntimeFuLatencyClass::COUNT],
    pub(crate) fu_latency_class_max_cycles: [u64; O3RuntimeFuLatencyClass::COUNT],
    pub(crate) fu_latency_class_min_cycles: [u64; O3RuntimeFuLatencyClass::COUNT],
    pub(crate) iq_branch_insts_issued: u64,
    pub(crate) issue_cycles: u64,
    pub(crate) issued_rows: u64,
    pub(crate) resource_blocked_row_cycles: u64,
    pub(crate) dependency_blocked_row_cycles: u64,
    pub(crate) max_rows_per_cycle: u64,
    pub(crate) live_retire_gate_scheduled_waits: u64,
    pub(crate) live_retire_gate_wait_ticks: u64,
    pub(crate) live_retire_gate_max_wait_ticks: u64,
    pub(crate) max_rob_occupancy: u64,
    pub(crate) max_lsq_occupancy: u64,
    pub(crate) rename_map_entries: u64,
}

fn sum_branch_kind_counts(counts: [u64; BranchTargetKind::COUNT]) -> u64 {
    counts.into_iter().fold(0_u64, u64::saturating_add)
}

impl O3RuntimeStats {
    pub const fn instructions(self) -> u64 {
        self.instructions
    }

    pub const fn rob_allocations(self) -> u64 {
        self.rob_allocations
    }

    pub const fn rob_commits(self) -> u64 {
        self.rob_commits
    }

    pub const fn rename_writes(self) -> u64 {
        self.rename_writes
    }

    pub const fn lsq_loads(self) -> u64 {
        self.lsq_loads
    }

    pub const fn lsq_stores(self) -> u64 {
        self.lsq_stores
    }

    pub const fn lsq_load_bytes(self) -> u64 {
        self.lsq_load_bytes
    }

    pub const fn lsq_store_bytes(self) -> u64 {
        self.lsq_store_bytes
    }

    pub const fn lsq_store_to_load_forwarding_candidates(self) -> u64 {
        self.lsq_store_to_load_forwarding_candidates
    }

    pub const fn lsq_store_to_load_forwarding_matches(self) -> u64 {
        self.lsq_store_to_load_forwarding_matches
    }

    pub const fn lsq_store_to_load_forwarding_suppressed(self) -> u64 {
        self.lsq_store_to_load_forwarding_suppressed
    }

    pub const fn lsq_store_to_load_forwarding_address_mismatches(self) -> u64 {
        self.lsq_store_to_load_forwarding_address_mismatches
    }

    pub const fn lsq_store_to_load_forwarding_byte_mismatches(self) -> u64 {
        self.lsq_store_to_load_forwarding_byte_mismatches
    }

    pub fn lsq_operation_count(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_counts[operation.index()]
    }

    pub fn lsq_operation_load_bytes(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_load_bytes[operation.index()]
    }

    pub fn lsq_operation_store_bytes(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_store_bytes[operation.index()]
    }

    pub fn lsq_operation_forwarding_candidates(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_forwarding_candidates[operation.index()]
    }

    pub fn lsq_operation_forwarding_matches(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_forwarding_matches[operation.index()]
    }

    pub fn lsq_operation_forwarding_suppressed(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_forwarding_suppressed[operation.index()]
    }

    pub fn lsq_operation_forwarding_address_mismatches(
        self,
        operation: O3RuntimeLsqOperation,
    ) -> u64 {
        self.lsq_operation_forwarding_address_mismatches[operation.index()]
    }

    pub fn lsq_operation_forwarding_byte_mismatches(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_forwarding_byte_mismatches[operation.index()]
    }

    pub const fn lsq_data_latency_samples(self) -> u64 {
        self.lsq_data_latency_samples
    }

    pub const fn lsq_data_latency_ticks(self) -> u64 {
        self.lsq_data_latency_ticks
    }

    pub const fn lsq_data_latency_max_ticks(self) -> u64 {
        self.lsq_data_latency_max_ticks
    }

    pub const fn lsq_data_latency_min_ticks(self) -> u64 {
        if self.lsq_data_latency_samples == 0 {
            0
        } else {
            self.lsq_data_latency_min_ticks
        }
    }

    pub const fn lsq_data_latency_avg_ticks(self) -> u64 {
        if self.lsq_data_latency_samples == 0 {
            0
        } else {
            self.lsq_data_latency_ticks / self.lsq_data_latency_samples
        }
    }

    pub fn lsq_operation_latency_ticks(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_latency_ticks[operation.index()]
    }

    pub fn lsq_operation_latency_samples(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_latency_samples[operation.index()]
    }

    pub fn lsq_operation_latency_max_ticks(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_latency_max_ticks[operation.index()]
    }

    pub fn lsq_operation_latency_min_ticks(self, operation: O3RuntimeLsqOperation) -> u64 {
        if self.lsq_operation_latency_samples(operation) == 0 {
            0
        } else {
            self.lsq_operation_latency_min_ticks[operation.index()]
        }
    }

    pub fn lsq_operation_latency_avg_ticks(self, operation: O3RuntimeLsqOperation) -> u64 {
        let samples = self.lsq_operation_latency_samples(operation);
        if samples == 0 {
            0
        } else {
            self.lsq_operation_latency_ticks(operation) / samples
        }
    }

    pub fn lsq_ordering_count(self, ordering: O3RuntimeLsqOrdering) -> u64 {
        self.lsq_ordering_counts[ordering.index()]
    }

    pub const fn lsq_store_conditional_failures(self) -> u64 {
        self.lsq_store_conditional_failures
    }

    pub fn lsq_operation_store_conditional_failures(self, operation: O3RuntimeLsqOperation) -> u64 {
        if operation == O3RuntimeLsqOperation::StoreConditional {
            self.lsq_store_conditional_failures()
        } else {
            0
        }
    }

    pub const fn branch_repair_targetless_mismatches(self) -> u64 {
        self.branch_repair_targetless_mismatches
    }

    pub const fn branch_repair_wrong_targets(self) -> u64 {
        self.branch_repair_wrong_targets
    }

    pub const fn branch_repair_direction_only_mismatches(self) -> u64 {
        self.branch_repair_direction_only_mismatches
    }

    pub const fn branch_repair_mispredicts(self) -> u64 {
        self.branch_repair_targetless_mismatches
            .saturating_add(self.branch_repair_wrong_targets)
            .saturating_add(self.branch_repair_direction_only_mismatches)
    }

    pub fn branch_repair_targetless_mismatch_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_repair_targetless_mismatch_kinds[kind.index()]
    }

    pub fn branch_repair_wrong_target_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_repair_wrong_target_kinds[kind.index()]
    }

    pub fn branch_repair_direction_only_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_repair_direction_only_kinds[kind.index()]
    }

    pub fn branch_direction_mismatches(self) -> u64 {
        sum_branch_kind_counts(self.branch_direction_mismatch_kinds)
    }

    pub fn branch_direction_mismatch_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_direction_mismatch_kinds[kind.index()]
    }

    pub fn branch_direction_mismatch_link_writes(self) -> u64 {
        sum_branch_kind_counts(self.branch_direction_mismatch_link_write_kinds)
    }

    pub fn branch_direction_mismatch_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_direction_mismatch_link_write_kinds[kind.index()]
    }

    pub fn branch_direction_mismatch_without_link_writes(self) -> u64 {
        sum_branch_kind_counts(self.branch_direction_mismatch_without_link_write_kinds)
    }

    pub fn branch_direction_mismatch_without_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_direction_mismatch_without_link_write_kinds[kind.index()]
    }

    pub fn branch_direction_mismatch_squashed_targets(self) -> u64 {
        sum_branch_kind_counts(self.branch_direction_mismatch_squashed_target_kinds)
    }

    pub fn branch_direction_mismatch_squashed_target_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_direction_mismatch_squashed_target_kinds[kind.index()]
    }

    pub fn branch_direction_mismatch_squashed_target_link_writes(self) -> u64 {
        sum_branch_kind_counts(self.branch_direction_mismatch_squashed_target_link_write_kinds)
    }

    pub fn branch_direction_mismatch_squashed_target_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_direction_mismatch_squashed_target_link_write_kinds[kind.index()]
    }

    pub fn branch_direction_mismatch_squashed_target_without_link_writes(self) -> u64 {
        sum_branch_kind_counts(
            self.branch_direction_mismatch_squashed_target_without_link_write_kinds,
        )
    }

    pub fn branch_direction_mismatch_squashed_target_without_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_direction_mismatch_squashed_target_without_link_write_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_targetless_mismatches(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_targetless_kinds)
    }

    pub fn branch_target_mismatch_targetless_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_target_mismatch_targetless_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_targetless_without_link_writes(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_targetless_without_link_write_kinds)
    }

    pub fn branch_target_mismatch_targetless_without_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_targetless_without_link_write_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_targetless_squashed_targets(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_targetless_squashed_target_kinds)
    }

    pub fn branch_target_mismatch_targetless_squashed_target_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_targetless_squashed_target_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_targetless_squashed_target_without_link_writes(self) -> u64 {
        sum_branch_kind_counts(
            self.branch_target_mismatch_targetless_squashed_target_without_link_write_kinds,
        )
    }

    pub fn branch_target_mismatch_targetless_squashed_target_without_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_targetless_squashed_target_without_link_write_kinds
            [kind.index()]
    }

    pub fn branch_target_mismatch_wrong_targets(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_wrong_target_kinds)
    }

    pub fn branch_target_mismatch_wrong_target_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_target_mismatch_wrong_target_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_wrong_target_link_writes(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_wrong_target_link_write_kinds)
    }

    pub fn branch_target_mismatch_wrong_target_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_wrong_target_link_write_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_wrong_target_without_link_writes(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_wrong_target_without_link_write_kinds)
    }

    pub fn branch_target_mismatch_wrong_target_without_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_wrong_target_without_link_write_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_wrong_target_squashed_targets(self) -> u64 {
        sum_branch_kind_counts(self.branch_target_mismatch_wrong_target_squashed_target_kinds)
    }

    pub fn branch_target_mismatch_wrong_target_squashed_target_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_wrong_target_squashed_target_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_wrong_target_squashed_target_link_writes(self) -> u64 {
        sum_branch_kind_counts(
            self.branch_target_mismatch_wrong_target_squashed_target_link_write_kinds,
        )
    }

    pub fn branch_target_mismatch_wrong_target_squashed_target_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_wrong_target_squashed_target_link_write_kinds[kind.index()]
    }

    pub fn branch_target_mismatch_wrong_target_squashed_target_without_link_writes(self) -> u64 {
        sum_branch_kind_counts(
            self.branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds,
        )
    }

    pub fn branch_target_mismatch_wrong_target_squashed_target_without_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds
            [kind.index()]
    }

    pub fn branch_event_misprediction_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_repair_targetless_mismatch_kind(kind)
            .saturating_add(self.branch_repair_wrong_target_kind(kind))
            .saturating_add(self.branch_repair_direction_only_kind(kind))
    }

    pub const fn branch_event_mispredictions(self) -> u64 {
        self.branch_repair_mispredicts()
    }

    pub fn branch_event_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_kinds[kind.index()]
    }

    pub fn branch_events(self) -> u64 {
        self.branch_event_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_taken(self) -> u64 {
        self.branch_event_taken_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_not_taken(self) -> u64 {
        self.branch_events()
            .saturating_sub(self.branch_event_taken())
    }

    pub fn branch_event_taken_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_taken_kinds[kind.index()]
    }

    pub fn branch_event_not_taken_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_kind(kind)
            .saturating_sub(self.branch_event_taken_kind(kind))
    }

    pub fn branch_event_predicted_taken(self) -> u64 {
        self.branch_event_predicted_taken_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_predicted_not_taken(self) -> u64 {
        self.branch_events()
            .saturating_sub(self.branch_event_predicted_taken())
    }

    pub fn branch_event_predicted_taken_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_predicted_taken_kinds[kind.index()]
    }

    pub fn branch_event_predicted_not_taken_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_kind(kind)
            .saturating_sub(self.branch_event_predicted_taken_kind(kind))
    }

    pub fn branch_event_predicted_targets(self) -> u64 {
        self.branch_event_predicted_target_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_predicted_target_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_predicted_target_kinds[kind.index()]
    }

    pub fn branch_event_predicted_target_matches(self) -> u64 {
        self.branch_event_predicted_target_match_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_predicted_target_match_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_predicted_target_match_kinds[kind.index()]
    }

    pub fn branch_event_predicted_target_mismatches(self) -> u64 {
        self.branch_event_predicted_target_mismatch_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_predicted_target_mismatch_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_predicted_target_mismatch_kinds[kind.index()]
    }

    pub fn branch_event_resolved_targets(self) -> u64 {
        self.branch_event_resolved_target_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_resolved_target_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_resolved_target_kinds[kind.index()]
    }

    pub fn branch_event_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_link_write_kinds[kind.index()]
    }

    pub fn branch_event_without_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_kind(kind)
            .saturating_sub(self.branch_event_link_write_kind(kind))
    }

    pub fn branch_event_link_writes(self) -> u64 {
        self.branch_event_link_write_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_without_link_writes(self) -> u64 {
        self.branch_events()
            .saturating_sub(self.branch_event_link_writes())
    }

    pub fn branch_event_squash_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_squash_kinds[kind.index()]
    }

    pub fn branch_event_squashes(self) -> u64 {
        self.branch_event_squash_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_squashed_targets(self) -> u64 {
        self.branch_event_squashes()
    }

    pub fn branch_event_squashed_target_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_squash_kind(kind)
    }

    pub fn branch_event_squashed_targets_without_link_writes(self) -> u64 {
        self.branch_event_squashed_target_without_link_write_kinds
            .into_iter()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn branch_event_squashed_targets_with_link_writes(self) -> u64 {
        self.branch_event_squashed_targets()
            .saturating_sub(self.branch_event_squashed_targets_without_link_writes())
    }

    pub fn branch_event_squashed_target_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.branch_event_squash_kind(kind)
            .saturating_sub(self.branch_event_squashed_target_without_link_write_kind(kind))
    }

    pub fn branch_event_squashed_target_without_link_write_kind(
        self,
        kind: BranchTargetKind,
    ) -> u64 {
        self.branch_event_squashed_target_without_link_write_kinds[kind.index()]
    }

    pub const fn iew_predicted_taken_incorrect(self) -> u64 {
        self.iew_predicted_taken_incorrect
    }

    pub const fn iew_predicted_not_taken_incorrect(self) -> u64 {
        self.iew_predicted_not_taken_incorrect
    }

    pub const fn iew_producer_insts(self) -> u64 {
        self.iew_producer_insts
    }

    pub const fn iew_consumer_insts(self) -> u64 {
        self.iew_consumer_insts
    }

    pub const fn fu_latency_instructions(self) -> u64 {
        self.fu_latency_instructions
    }

    pub const fn fu_latency_cycles(self) -> u64 {
        self.fu_latency_cycles
    }

    pub fn fu_latency_class_instructions(self, class: O3RuntimeFuLatencyClass) -> u64 {
        self.fu_latency_class_instructions[class.index()]
    }

    pub fn fu_latency_class_cycles(self, class: O3RuntimeFuLatencyClass) -> u64 {
        self.fu_latency_class_cycles[class.index()]
    }

    pub fn fu_latency_class_max_cycles(self, class: O3RuntimeFuLatencyClass) -> u64 {
        self.fu_latency_class_max_cycles[class.index()]
    }

    pub fn fu_latency_class_min_cycles(self, class: O3RuntimeFuLatencyClass) -> u64 {
        self.fu_latency_class_min_cycles[class.index()]
    }

    pub fn fu_latency_class_avg_cycles(self, class: O3RuntimeFuLatencyClass) -> u64 {
        let instructions = self.fu_latency_class_instructions(class);
        if instructions == 0 {
            0
        } else {
            self.fu_latency_class_cycles(class) / instructions
        }
    }

    pub fn fu_integer_mul_instructions(self) -> u64 {
        self.fu_latency_class_instructions(O3RuntimeFuLatencyClass::ScalarIntegerMul)
    }

    pub fn fu_integer_mul_latency_cycles(self) -> u64 {
        self.fu_latency_class_cycles(O3RuntimeFuLatencyClass::ScalarIntegerMul)
    }

    pub fn fu_integer_div_instructions(self) -> u64 {
        self.fu_latency_class_instructions(O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    }

    pub fn fu_integer_div_latency_cycles(self) -> u64 {
        self.fu_latency_class_cycles(O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    }

    pub const fn iq_branch_insts_issued(self) -> u64 {
        self.iq_branch_insts_issued
    }

    pub const fn issue_cycles(self) -> u64 {
        self.issue_cycles
    }

    pub const fn issued_rows(self) -> u64 {
        self.issued_rows
    }

    pub const fn resource_blocked_row_cycles(self) -> u64 {
        self.resource_blocked_row_cycles
    }

    pub const fn dependency_blocked_row_cycles(self) -> u64 {
        self.dependency_blocked_row_cycles
    }

    pub const fn max_rows_per_cycle(self) -> u64 {
        self.max_rows_per_cycle
    }

    pub const fn live_retire_gate_scheduled_waits(self) -> u64 {
        self.live_retire_gate_scheduled_waits
    }

    pub const fn live_retire_gate_wait_ticks(self) -> u64 {
        self.live_retire_gate_wait_ticks
    }

    pub const fn live_retire_gate_max_wait_ticks(self) -> u64 {
        self.live_retire_gate_max_wait_ticks
    }

    pub const fn max_rob_occupancy(self) -> u64 {
        self.max_rob_occupancy
    }

    pub const fn max_lsq_occupancy(self) -> u64 {
        self.max_lsq_occupancy
    }

    pub const fn rename_map_entries(self) -> u64 {
        self.rename_map_entries
    }

    pub const fn has_activity(self) -> bool {
        self.instructions != 0
            || self.rob_allocations != 0
            || self.rob_commits != 0
            || self.rename_writes != 0
            || self.lsq_loads != 0
            || self.lsq_stores != 0
            || self.lsq_load_bytes != 0
            || self.lsq_store_bytes != 0
            || self.lsq_store_to_load_forwarding_candidates != 0
            || self.lsq_store_to_load_forwarding_matches != 0
            || self.lsq_store_to_load_forwarding_suppressed != 0
            || self.lsq_store_to_load_forwarding_address_mismatches != 0
            || self.lsq_store_to_load_forwarding_byte_mismatches != 0
            || self.lsq_data_latency_samples != 0
            || self.lsq_data_latency_ticks != 0
            || self.lsq_data_latency_max_ticks != 0
            || self.lsq_data_latency_min_ticks != 0
            || self.lsq_store_conditional_failures != 0
            || self.branch_repair_targetless_mismatches != 0
            || self.branch_repair_wrong_targets != 0
            || self.branch_repair_direction_only_mismatches != 0
            || self.iew_predicted_taken_incorrect != 0
            || self.iew_predicted_not_taken_incorrect != 0
            || self.iew_producer_insts != 0
            || self.iew_consumer_insts != 0
            || self.fu_latency_instructions != 0
            || self.fu_latency_cycles != 0
            || self.iq_branch_insts_issued != 0
            || self.issue_cycles != 0
            || self.issued_rows != 0
            || self.resource_blocked_row_cycles != 0
            || self.dependency_blocked_row_cycles != 0
            || self.max_rows_per_cycle != 0
            || self.live_retire_gate_scheduled_waits != 0
            || self.live_retire_gate_wait_ticks != 0
            || self.live_retire_gate_max_wait_ticks != 0
            || self.max_rob_occupancy != 0
            || self.max_lsq_occupancy != 0
            || self.rename_map_entries != 0
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{MemoryWidth, Register};
    use rem6_memory::AgentId;

    use super::*;
    use crate::o3_runtime::o3_store_forwarding_entry;

    fn forwarding_plan(
        stores: &[O3StoreForwardingEntry],
        load: &MemoryAccessKind,
    ) -> O3StoreLoadForwardingPlan {
        let load_range = o3_load_forwarding_access(load).unwrap().range();
        match o3_store_load_composition(stores.iter().copied(), load_range).unwrap() {
            O3StoreLoadRelation::Forwarded(plan) | O3StoreLoadRelation::Overlay(plan) => plan,
            O3StoreLoadRelation::Independent(reason) => {
                panic!("stores should contribute to the load: {reason:?}")
            }
        }
    }

    #[test]
    fn contained_forwarded_load_is_a_match_not_an_address_mismatch() {
        let store = o3_store_forwarding_entry(&MemoryAccessKind::Store {
            address: 0x9000,
            width: MemoryWidth::Word,
            value: 0x007f_80ff,
        })
        .unwrap();
        let load_register = Register::new(12).unwrap();
        let load = MemoryAccessKind::Load {
            rd: load_register,
            address: 0x9001,
            width: MemoryWidth::Byte,
            signed: true,
        };
        let plan = forwarding_plan(&[store], &load);
        let mut stats = O3RuntimeStats::default();

        let (observation, pending) = stats.record_store_to_load_forwarding(
            &load,
            MemoryRequestId::new(AgentId::new(7), 1),
            &[RegisterWrite::new(load_register, 0xffff_ffff_ffff_ff80)],
            Some(store),
            Some(3),
            Some(plan),
        );

        assert!(observation.candidate);
        assert!(observation.matched);
        assert!(!observation.suppressed);
        assert!(!observation.address_mismatch);
        assert!(!observation.byte_mismatch);
        assert!(pending.is_none());
        assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 0);
        assert_eq!(stats.lsq_store_to_load_forwarding_address_mismatches(), 0);
    }

    #[test]
    fn partial_forwarded_load_is_a_match_not_a_suppressed_byte_mismatch() {
        let store = o3_store_forwarding_entry(&MemoryAccessKind::Store {
            address: 0x9001,
            width: MemoryWidth::Byte,
            value: 0x5a,
        })
        .unwrap();
        let load_register = Register::new(12).unwrap();
        let load = MemoryAccessKind::Load {
            rd: load_register,
            address: 0x9000,
            width: MemoryWidth::Word,
            signed: true,
        };
        let plan = forwarding_plan(&[store], &load);
        let mut stats = O3RuntimeStats::default();

        let (observation, pending) = stats.record_store_to_load_forwarding(
            &load,
            MemoryRequestId::new(AgentId::new(7), 1),
            &[RegisterWrite::new(load_register, 0xffff_ffff_8033_5a11)],
            Some(store),
            Some(3),
            Some(plan),
        );

        assert!(observation.candidate);
        assert!(observation.matched);
        assert!(observation.partial);
        assert_eq!(observation.forwarded_bytes, 1);
        assert!(!observation.suppressed);
        assert!(!observation.address_mismatch);
        assert!(!observation.byte_mismatch);
        assert!(pending.is_none());
        assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 0);
        assert_eq!(stats.lsq_store_to_load_forwarding_address_mismatches(), 0);
        assert_eq!(stats.lsq_store_to_load_forwarding_byte_mismatches(), 0);
    }

    #[test]
    fn multiple_store_forwarding_records_composed_bytes_and_match() {
        let stores = [
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9001,
                width: MemoryWidth::Byte,
                value: 0xaa,
            })
            .unwrap(),
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9002,
                width: MemoryWidth::Halfword,
                value: 0xccbb,
            })
            .unwrap(),
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9002,
                width: MemoryWidth::Byte,
                value: 0xdd,
            })
            .unwrap(),
        ];
        let load_register = Register::new(12).unwrap();
        let load = MemoryAccessKind::Load {
            rd: load_register,
            address: 0x9000,
            width: MemoryWidth::Word,
            signed: false,
        };
        let plan = forwarding_plan(&stores, &load);
        let mut stats = O3RuntimeStats::default();

        let (observation, pending) = stats.record_store_to_load_forwarding(
            &load,
            MemoryRequestId::new(AgentId::new(7), 1),
            &[RegisterWrite::new(load_register, 0xccdd_aa11)],
            stores.last().copied(),
            Some(3),
            Some(plan),
        );

        assert!(observation.candidate);
        assert!(observation.matched);
        assert!(observation.partial);
        assert_eq!(observation.forwarded_bytes, 3);
        assert!(pending.is_none());
        assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 0);
    }

    #[test]
    fn unforwarded_load_preserves_latest_store_suppression() {
        let stores = [
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9001,
                width: MemoryWidth::Byte,
                value: 0xaa,
            })
            .unwrap(),
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9040,
                width: MemoryWidth::Word,
                value: 0x2a,
            })
            .unwrap(),
        ];
        let load = MemoryAccessKind::Load {
            rd: Register::new(12).unwrap(),
            address: 0x9000,
            width: MemoryWidth::Word,
            signed: false,
        };
        let mut stats = O3RuntimeStats::default();

        let (observation, pending) = stats.record_store_to_load_forwarding(
            &load,
            MemoryRequestId::new(AgentId::new(7), 1),
            &[],
            stores.last().copied(),
            Some(3),
            None,
        );

        assert!(observation.suppressed);
        assert!(observation.address_mismatch);
        assert!(!observation.candidate);
        assert!(pending.is_none());
        assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 1);
        assert_eq!(stats.lsq_store_to_load_forwarding_address_mismatches(), 1);
    }

    #[test]
    fn forwarding_stats_do_not_compose_a_stale_retired_store() {
        let stores = [
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9000,
                width: MemoryWidth::Byte,
                value: 0xaa,
            })
            .unwrap(),
            o3_store_forwarding_entry(&MemoryAccessKind::Store {
                address: 0x9001,
                width: MemoryWidth::Byte,
                value: 0xbb,
            })
            .unwrap(),
        ];
        let load_register = Register::new(12).unwrap();
        let load = MemoryAccessKind::Load {
            rd: load_register,
            address: 0x9000,
            width: MemoryWidth::Word,
            signed: false,
        };
        let live_plan = forwarding_plan(&stores[1..], &load);
        let mut stats = O3RuntimeStats::default();

        let (observation, pending) = stats.record_store_to_load_forwarding(
            &load,
            MemoryRequestId::new(AgentId::new(7), 1),
            &[RegisterWrite::new(load_register, 0x4433_bbaa)],
            Some(stores[0]),
            Some(3),
            Some(live_plan),
        );

        assert!(observation.candidate);
        assert!(observation.matched);
        assert!(observation.partial);
        assert_eq!(observation.forwarded_bytes, 1);
        assert!(pending.is_none());
    }
}

impl O3RuntimeStats {
    pub(super) fn observe_rob_occupancy(&mut self, occupancy: usize) {
        self.max_rob_occupancy = self
            .max_rob_occupancy
            .max(u64::try_from(occupancy).unwrap_or(u64::MAX));
    }

    pub(super) fn observe_lsq_occupancy(&mut self, occupancy: usize) {
        self.max_lsq_occupancy = self
            .max_lsq_occupancy
            .max(u64::try_from(occupancy).unwrap_or(u64::MAX));
    }

    pub(super) fn set_rename_map_entries(&mut self, entries: usize) {
        self.rename_map_entries = u64::try_from(entries).unwrap_or(u64::MAX);
    }

    pub(super) fn record_iew_dependency_producer(&mut self) {
        self.iew_producer_insts = self.iew_producer_insts.saturating_add(1);
    }

    pub(super) fn record_iew_dependency_consumer(&mut self) {
        self.iew_consumer_insts = self.iew_consumer_insts.saturating_add(1);
    }

    pub(super) fn record_issue_cycle(
        &mut self,
        new_cycle: bool,
        issued_rows: usize,
        resource_blocked_rows: usize,
        dependency_blocked_rows: usize,
        total_rows_at_tick: usize,
    ) {
        self.issue_cycles = self
            .issue_cycles
            .saturating_add(if new_cycle { 1 } else { 0 });
        self.issued_rows = self.issued_rows.saturating_add(issued_rows as u64);
        self.resource_blocked_row_cycles = self
            .resource_blocked_row_cycles
            .saturating_add(resource_blocked_rows as u64);
        self.dependency_blocked_row_cycles = self
            .dependency_blocked_row_cycles
            .saturating_add(dependency_blocked_rows as u64);
        self.max_rows_per_cycle = self.max_rows_per_cycle.max(total_rows_at_tick as u64);
    }

    pub(crate) fn record_live_retire_gate_wait(&mut self, wait_ticks: u64) {
        self.live_retire_gate_scheduled_waits =
            self.live_retire_gate_scheduled_waits.saturating_add(1);
        self.live_retire_gate_wait_ticks =
            self.live_retire_gate_wait_ticks.saturating_add(wait_ticks);
        self.live_retire_gate_max_wait_ticks = self.live_retire_gate_max_wait_ticks.max(wait_ticks);
    }

    pub(super) fn record_retired_instruction(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_record: O3RuntimeTraceRecord,
    ) {
        self.instructions = self.instructions.saturating_add(1);
        self.rob_allocations = self.rob_allocations.saturating_add(1);
        self.rob_commits = self.rob_commits.saturating_add(1);
        self.record_branch_event(trace_record);
        self.record_branch_mismatch(trace_record);
        self.record_branch_repair(trace_record);
        if trace_record.branch_event() {
            self.iq_branch_insts_issued = self.iq_branch_insts_issued.saturating_add(1);
        }

        let record = execution.execution();
        let fu_latency_cycles =
            crate::riscv_fu_latency::riscv_execute_wait_cycles(record.instruction());
        if fu_latency_cycles > 0 {
            self.fu_latency_instructions = self.fu_latency_instructions.saturating_add(1);
            self.fu_latency_cycles = self.fu_latency_cycles.saturating_add(fu_latency_cycles);
            if let Some(class) = o3_fu_latency_class(record.instruction()) {
                let index = class.index();
                self.fu_latency_class_instructions[index] =
                    self.fu_latency_class_instructions[index].saturating_add(1);
                self.fu_latency_class_cycles[index] =
                    self.fu_latency_class_cycles[index].saturating_add(fu_latency_cycles);
                self.fu_latency_class_max_cycles[index] =
                    self.fu_latency_class_max_cycles[index].max(fu_latency_cycles);
                if self.fu_latency_class_min_cycles[index] == 0
                    || fu_latency_cycles < self.fu_latency_class_min_cycles[index]
                {
                    self.fu_latency_class_min_cycles[index] = fu_latency_cycles;
                }
            }
        }

        self.rename_writes = self
            .rename_writes
            .saturating_add(o3_rename_write_count(record));

        if let Some(access) = record.memory_access() {
            let (loads, stores) = o3_lsq_access_counts(access);
            self.lsq_loads = self.lsq_loads.saturating_add(loads);
            self.lsq_stores = self.lsq_stores.saturating_add(stores);
            let (load_bytes, store_bytes) = o3_lsq_access_bytes(access);
            self.lsq_load_bytes = self.lsq_load_bytes.saturating_add(load_bytes);
            self.lsq_store_bytes = self.lsq_store_bytes.saturating_add(store_bytes);

            let operation = o3_lsq_operation(access);
            let operation_index = operation.index();
            self.lsq_operation_counts[operation_index] =
                self.lsq_operation_counts[operation_index].saturating_add(1);
            self.lsq_operation_load_bytes[operation_index] =
                self.lsq_operation_load_bytes[operation_index].saturating_add(load_bytes);
            self.lsq_operation_store_bytes[operation_index] =
                self.lsq_operation_store_bytes[operation_index].saturating_add(store_bytes);

            let ordering = o3_lsq_ordering(access);
            if ordering != O3RuntimeLsqOrdering::None {
                let ordering_index = ordering.index();
                self.lsq_ordering_counts[ordering_index] =
                    self.lsq_ordering_counts[ordering_index].saturating_add(1);
            }
        }
    }

    pub(super) fn record_store_conditional_failure(&mut self) {
        self.lsq_store_conditional_failures = self.lsq_store_conditional_failures.saturating_add(1);
    }

    fn record_branch_event(&mut self, event: O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }
        let index = event.branch_kind().index();
        self.branch_event_kinds[index] = self.branch_event_kinds[index].saturating_add(1);
        if event.branch_predicted_taken() {
            self.branch_event_predicted_taken_kinds[index] =
                self.branch_event_predicted_taken_kinds[index].saturating_add(1);
        }
        if event.branch_predicted_target().is_some() {
            self.branch_event_predicted_target_kinds[index] =
                self.branch_event_predicted_target_kinds[index].saturating_add(1);
        }
        if event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) == event.branch_resolved_target())
        {
            self.branch_event_predicted_target_match_kinds[index] =
                self.branch_event_predicted_target_match_kinds[index].saturating_add(1);
        }
        if event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) != event.branch_resolved_target())
        {
            self.branch_event_predicted_target_mismatch_kinds[index] =
                self.branch_event_predicted_target_mismatch_kinds[index].saturating_add(1);
        }
        if event.branch_resolved_taken() {
            self.branch_event_taken_kinds[index] =
                self.branch_event_taken_kinds[index].saturating_add(1);
        }
        if event.branch_resolved_target().is_some() {
            self.branch_event_resolved_target_kinds[index] =
                self.branch_event_resolved_target_kinds[index].saturating_add(1);
        }
        if event.branch_link_register_write() {
            self.branch_event_link_write_kinds[index] =
                self.branch_event_link_write_kinds[index].saturating_add(1);
        }
        if event.branch_squash() {
            self.branch_event_squash_kinds[index] =
                self.branch_event_squash_kinds[index].saturating_add(1);
        }
        if event.branch_squashed_target().is_some() && !event.branch_link_register_write() {
            self.branch_event_squashed_target_without_link_write_kinds[index] =
                self.branch_event_squashed_target_without_link_write_kinds[index].saturating_add(1);
        }
    }

    fn record_branch_mismatch(&mut self, event: O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }
        let index = event.branch_kind().index();
        let link_write = event.branch_link_register_write();
        let squashed_target = event.branch_squashed_target().is_some();
        if event.branch_predicted_taken() != event.branch_resolved_taken() {
            self.branch_direction_mismatch_kinds[index] =
                self.branch_direction_mismatch_kinds[index].saturating_add(1);
            if link_write {
                self.branch_direction_mismatch_link_write_kinds[index] =
                    self.branch_direction_mismatch_link_write_kinds[index].saturating_add(1);
            } else {
                self.branch_direction_mismatch_without_link_write_kinds[index] = self
                    .branch_direction_mismatch_without_link_write_kinds[index]
                    .saturating_add(1);
            }
            if squashed_target {
                self.branch_direction_mismatch_squashed_target_kinds[index] =
                    self.branch_direction_mismatch_squashed_target_kinds[index].saturating_add(1);
                if link_write {
                    self.branch_direction_mismatch_squashed_target_link_write_kinds[index] = self
                        .branch_direction_mismatch_squashed_target_link_write_kinds[index]
                        .saturating_add(1);
                } else {
                    self.branch_direction_mismatch_squashed_target_without_link_write_kinds
                        [index] = self
                        .branch_direction_mismatch_squashed_target_without_link_write_kinds[index]
                        .saturating_add(1);
                }
            }
        }
        if event.branch_predicted_target().is_some() && event.branch_resolved_target().is_none() {
            self.branch_target_mismatch_targetless_kinds[index] =
                self.branch_target_mismatch_targetless_kinds[index].saturating_add(1);
            if !link_write {
                self.branch_target_mismatch_targetless_without_link_write_kinds[index] = self
                    .branch_target_mismatch_targetless_without_link_write_kinds[index]
                    .saturating_add(1);
            }
            if squashed_target {
                self.branch_target_mismatch_targetless_squashed_target_kinds[index] = self
                    .branch_target_mismatch_targetless_squashed_target_kinds[index]
                    .saturating_add(1);
                if !link_write {
                    self.branch_target_mismatch_targetless_squashed_target_without_link_write_kinds
                        [index] = self
                        .branch_target_mismatch_targetless_squashed_target_without_link_write_kinds
                        [index]
                        .saturating_add(1);
                }
            }
        }
        if event
            .branch_predicted_target()
            .zip(event.branch_resolved_target())
            .is_some_and(|(predicted, resolved)| predicted != resolved)
        {
            self.branch_target_mismatch_wrong_target_kinds[index] =
                self.branch_target_mismatch_wrong_target_kinds[index].saturating_add(1);
            if link_write {
                self.branch_target_mismatch_wrong_target_link_write_kinds[index] = self
                    .branch_target_mismatch_wrong_target_link_write_kinds[index]
                    .saturating_add(1);
            } else {
                self.branch_target_mismatch_wrong_target_without_link_write_kinds[index] = self
                    .branch_target_mismatch_wrong_target_without_link_write_kinds[index]
                    .saturating_add(1);
            }
            if squashed_target {
                self.branch_target_mismatch_wrong_target_squashed_target_kinds[index] = self
                    .branch_target_mismatch_wrong_target_squashed_target_kinds[index]
                    .saturating_add(1);
                if link_write {
                    self.branch_target_mismatch_wrong_target_squashed_target_link_write_kinds
                        [index] = self
                        .branch_target_mismatch_wrong_target_squashed_target_link_write_kinds
                        [index]
                        .saturating_add(1);
                } else {
                    self.branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds
                        [index] = self
                        .branch_target_mismatch_wrong_target_squashed_target_without_link_write_kinds
                        [index]
                        .saturating_add(1);
                }
            }
        }
    }

    pub(super) fn record_lsq_operation_latency(
        &mut self,
        operation: O3RuntimeLsqOperation,
        latency_ticks: u64,
    ) {
        if operation == O3RuntimeLsqOperation::None {
            return;
        }
        let aggregate_samples = self.lsq_data_latency_samples;
        self.lsq_data_latency_samples = aggregate_samples.saturating_add(1);
        self.lsq_data_latency_ticks = self.lsq_data_latency_ticks.saturating_add(latency_ticks);
        self.lsq_data_latency_max_ticks = self.lsq_data_latency_max_ticks.max(latency_ticks);
        if aggregate_samples == 0 || latency_ticks < self.lsq_data_latency_min_ticks {
            self.lsq_data_latency_min_ticks = latency_ticks;
        }

        let index = operation.index();
        let samples = self.lsq_operation_latency_samples[index];
        self.lsq_operation_latency_samples[index] = samples.saturating_add(1);
        self.lsq_operation_latency_ticks[index] =
            self.lsq_operation_latency_ticks[index].saturating_add(latency_ticks);
        self.lsq_operation_latency_max_ticks[index] =
            self.lsq_operation_latency_max_ticks[index].max(latency_ticks);
        if samples == 0 || latency_ticks < self.lsq_operation_latency_min_ticks[index] {
            self.lsq_operation_latency_min_ticks[index] = latency_ticks;
        }
    }

    fn record_branch_repair(&mut self, event: O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }
        let index = event.branch_kind().index();
        let repaired = if event.branch_predicted_target().is_some()
            && event.branch_resolved_target().is_none()
        {
            self.branch_repair_targetless_mismatches =
                self.branch_repair_targetless_mismatches.saturating_add(1);
            self.branch_repair_targetless_mismatch_kinds[index] =
                self.branch_repair_targetless_mismatch_kinds[index].saturating_add(1);
            true
        } else if event
            .branch_predicted_target()
            .zip(event.branch_resolved_target())
            .is_some_and(|(predicted, resolved)| predicted != resolved)
        {
            self.branch_repair_wrong_targets = self.branch_repair_wrong_targets.saturating_add(1);
            self.branch_repair_wrong_target_kinds[index] =
                self.branch_repair_wrong_target_kinds[index].saturating_add(1);
            true
        } else if event.branch_predicted_taken() != event.branch_resolved_taken() {
            self.branch_repair_direction_only_mismatches = self
                .branch_repair_direction_only_mismatches
                .saturating_add(1);
            self.branch_repair_direction_only_kinds[index] =
                self.branch_repair_direction_only_kinds[index].saturating_add(1);
            true
        } else {
            false
        };
        if repaired {
            self.record_iew_branch_mispredict_split(event);
        }
    }

    fn record_iew_branch_mispredict_split(&mut self, event: O3RuntimeTraceRecord) {
        if event.branch_predicted_taken() {
            self.iew_predicted_taken_incorrect =
                self.iew_predicted_taken_incorrect.saturating_add(1);
        } else {
            self.iew_predicted_not_taken_incorrect =
                self.iew_predicted_not_taken_incorrect.saturating_add(1);
        }
    }

    pub(super) fn record_store_to_load_forwarding(
        &mut self,
        access: &MemoryAccessKind,
        fetch_request: MemoryRequestId,
        register_writes: &[RegisterWrite],
        prior_store: Option<O3StoreForwardingEntry>,
        trace_sequence: Option<u64>,
        forwarding_plan: Option<O3StoreLoadForwardingPlan>,
    ) -> (
        O3StoreForwardingObservation,
        Option<O3PendingLoadForwardingMatch>,
    ) {
        let Some(load) = o3_load_forwarding_access(access) else {
            return (O3StoreForwardingObservation::default(), None);
        };
        let (plan, partial) = match forwarding_plan {
            Some(plan) => (plan, plan.is_partial()),
            None => {
                let Some(relation) =
                    prior_store.and_then(|store| o3_store_load_composition([store], load.range()))
                else {
                    return (O3StoreForwardingObservation::default(), None);
                };
                match relation {
                    O3StoreLoadRelation::Independent(reason) => {
                        self.record_store_to_load_forwarding_suppressed(
                            o3_lsq_operation(access),
                            reason,
                        );
                        return (
                            O3StoreForwardingObservation {
                                suppressed: true,
                                address_mismatch: reason
                                    == O3StoreLoadSuppressionReason::AddressMismatch,
                                byte_mismatch: reason == O3StoreLoadSuppressionReason::ByteMismatch,
                                ..O3StoreForwardingObservation::default()
                            },
                            None,
                        );
                    }
                    O3StoreLoadRelation::Forwarded(_) | O3StoreLoadRelation::Overlay(_) => {
                        return (O3StoreForwardingObservation::default(), None);
                    }
                }
            }
        };

        self.lsq_store_to_load_forwarding_candidates = self
            .lsq_store_to_load_forwarding_candidates
            .saturating_add(1);
        let operation = o3_lsq_operation(access);
        let operation_index = operation.index();
        self.lsq_operation_forwarding_candidates[operation_index] =
            self.lsq_operation_forwarding_candidates[operation_index].saturating_add(1);
        let mut observation = O3StoreForwardingObservation {
            candidate: true,
            matched: false,
            partial,
            forwarded_bytes: u64::from(plan.forwarded_bytes()),
            ..O3StoreForwardingObservation::default()
        };
        match o3_load_register_value(register_writes, load.register()) {
            Some(value) => {
                if plan.matches_value(value) {
                    self.record_store_to_load_forwarding_match(operation);
                    observation.matched = true;
                }
                (observation, None)
            }
            None => (
                observation,
                Some(O3PendingLoadForwardingMatch {
                    fetch_request,
                    plan,
                    operation,
                    trace_sequence,
                }),
            ),
        }
    }

    pub(super) fn record_store_to_load_forwarding_match(
        &mut self,
        operation: O3RuntimeLsqOperation,
    ) {
        self.lsq_store_to_load_forwarding_matches =
            self.lsq_store_to_load_forwarding_matches.saturating_add(1);
        let operation_index = operation.index();
        self.lsq_operation_forwarding_matches[operation_index] =
            self.lsq_operation_forwarding_matches[operation_index].saturating_add(1);
    }

    fn record_store_to_load_forwarding_suppressed(
        &mut self,
        operation: O3RuntimeLsqOperation,
        reason: O3StoreLoadSuppressionReason,
    ) {
        self.lsq_store_to_load_forwarding_suppressed = self
            .lsq_store_to_load_forwarding_suppressed
            .saturating_add(1);
        let operation_index = operation.index();
        self.lsq_operation_forwarding_suppressed[operation_index] =
            self.lsq_operation_forwarding_suppressed[operation_index].saturating_add(1);
        match reason {
            O3StoreLoadSuppressionReason::AddressMismatch => {
                self.lsq_store_to_load_forwarding_address_mismatches = self
                    .lsq_store_to_load_forwarding_address_mismatches
                    .saturating_add(1);
                self.lsq_operation_forwarding_address_mismatches[operation_index] = self
                    .lsq_operation_forwarding_address_mismatches[operation_index]
                    .saturating_add(1);
            }
            O3StoreLoadSuppressionReason::ByteMismatch => {
                self.lsq_store_to_load_forwarding_byte_mismatches = self
                    .lsq_store_to_load_forwarding_byte_mismatches
                    .saturating_add(1);
                self.lsq_operation_forwarding_byte_mismatches[operation_index] = self
                    .lsq_operation_forwarding_byte_mismatches[operation_index]
                    .saturating_add(1);
            }
        }
    }
}
