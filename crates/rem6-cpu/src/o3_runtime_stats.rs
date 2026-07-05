use crate::branch_predictor::BranchTargetKind;
use crate::o3_runtime_trace::{
    O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
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
    pub(crate) lsq_operation_counts: [u64; O3RuntimeLsqOperation::COUNT],
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
    pub(crate) fu_latency_instructions: u64,
    pub(crate) fu_latency_cycles: u64,
    pub(crate) fu_latency_class_instructions: [u64; O3RuntimeFuLatencyClass::COUNT],
    pub(crate) fu_latency_class_cycles: [u64; O3RuntimeFuLatencyClass::COUNT],
    pub(crate) max_rob_occupancy: u64,
    pub(crate) max_lsq_occupancy: u64,
    pub(crate) rename_map_entries: u64,
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

    pub fn lsq_operation_count(self, operation: O3RuntimeLsqOperation) -> u64 {
        self.lsq_operation_counts[operation.index()]
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

    pub const fn branch_repair_targetless_mismatches(self) -> u64 {
        self.branch_repair_targetless_mismatches
    }

    pub const fn branch_repair_wrong_targets(self) -> u64 {
        self.branch_repair_wrong_targets
    }

    pub const fn branch_repair_direction_only_mismatches(self) -> u64 {
        self.branch_repair_direction_only_mismatches
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
            || self.lsq_store_conditional_failures != 0
            || self.branch_repair_targetless_mismatches != 0
            || self.branch_repair_wrong_targets != 0
            || self.branch_repair_direction_only_mismatches != 0
            || self.fu_latency_instructions != 0
            || self.fu_latency_cycles != 0
            || self.max_rob_occupancy != 0
            || self.max_lsq_occupancy != 0
            || self.rename_map_entries != 0
    }
}
