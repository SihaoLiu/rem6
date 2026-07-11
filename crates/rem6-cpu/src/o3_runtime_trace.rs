use rem6_memory::Address;

use crate::branch_predictor::BranchTargetKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RuntimeTraceRecord {
    sequence: u64,
    tick: u64,
    commit_tick: u64,
    pc: Address,
    rob_allocated: bool,
    rob_committed: bool,
    rob_occupancy: u64,
    rob_commits_at_tick: u64,
    rob_commit_blocked: bool,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    lsq_occupancy: u64,
    lsq_operation: O3RuntimeLsqOperation,
    lsq_ordering: O3RuntimeLsqOrdering,
    lsq_load_address: Option<Address>,
    lsq_store_address: Option<Address>,
    lsq_load_bytes: u64,
    lsq_store_bytes: u64,
    lsq_store_conditional_failed: bool,
    lsq_data_response_tick: u64,
    lsq_data_latency_ticks: u64,
    rename_map_entries: u64,
    store_load_forwarding_candidate: bool,
    store_load_forwarding_match: bool,
    store_load_forwarding_partial: bool,
    store_load_forwarding_bytes: u64,
    store_load_forwarding_suppressed: bool,
    store_load_forwarding_address_mismatch: bool,
    store_load_forwarding_byte_mismatch: bool,
    iew_dependency_producers: u64,
    iew_dependency_consumers: u64,
    branch_kind: BranchTargetKind,
    branch_predicted_taken: bool,
    branch_resolved_taken: bool,
    branch_mispredicted: bool,
    branch_link_register_write: bool,
    branch_predicted_target: Option<Address>,
    branch_resolved_target: Option<Address>,
    branch_squashed_target: Option<Address>,
    fu_latency_class: Option<O3RuntimeFuLatencyClass>,
    fu_latency_cycles: u64,
    system_event: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3RuntimeFuLatencyClass {
    ScalarIntegerMul,
    ScalarIntegerDiv,
    ScalarFloatAdd,
    ScalarFloatCompare,
    ScalarFloatMisc,
    ScalarFloatMul,
    ScalarFloatFma,
    ScalarFloatDiv,
    ScalarFloatSqrt,
    VectorIntegerMul,
    VectorIntegerDiv,
    VectorFloatAdd,
    VectorFloatCompare,
    VectorFloatMisc,
    VectorFloatMul,
    VectorFloatFma,
    VectorFloatDiv,
    VectorFloatSqrt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3RuntimeLsqOperation {
    None,
    Load,
    Store,
    LoadReserved,
    StoreConditional,
    Atomic,
    FloatLoad,
    FloatStore,
    VectorLoad,
    VectorStore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3RuntimeLsqOrdering {
    None,
    Acquire,
    Release,
    AcquireRelease,
}

impl O3RuntimeFuLatencyClass {
    pub const COUNT: usize = 18;

    pub const ALL: [Self; Self::COUNT] = [
        Self::ScalarIntegerMul,
        Self::ScalarIntegerDiv,
        Self::ScalarFloatAdd,
        Self::ScalarFloatCompare,
        Self::ScalarFloatMisc,
        Self::ScalarFloatMul,
        Self::ScalarFloatFma,
        Self::ScalarFloatDiv,
        Self::ScalarFloatSqrt,
        Self::VectorIntegerMul,
        Self::VectorIntegerDiv,
        Self::VectorFloatAdd,
        Self::VectorFloatCompare,
        Self::VectorFloatMisc,
        Self::VectorFloatMul,
        Self::VectorFloatFma,
        Self::VectorFloatDiv,
        Self::VectorFloatSqrt,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::ScalarIntegerMul => 0,
            Self::ScalarIntegerDiv => 1,
            Self::ScalarFloatAdd => 2,
            Self::ScalarFloatCompare => 3,
            Self::ScalarFloatMisc => 4,
            Self::ScalarFloatMul => 5,
            Self::ScalarFloatFma => 6,
            Self::ScalarFloatDiv => 7,
            Self::ScalarFloatSqrt => 8,
            Self::VectorIntegerMul => 9,
            Self::VectorIntegerDiv => 10,
            Self::VectorFloatAdd => 11,
            Self::VectorFloatCompare => 12,
            Self::VectorFloatMisc => 13,
            Self::VectorFloatMul => 14,
            Self::VectorFloatFma => 15,
            Self::VectorFloatDiv => 16,
            Self::VectorFloatSqrt => 17,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScalarIntegerMul => "scalar_integer_mul",
            Self::ScalarIntegerDiv => "scalar_integer_div",
            Self::ScalarFloatAdd => "scalar_float_add",
            Self::ScalarFloatCompare => "scalar_float_compare",
            Self::ScalarFloatMisc => "scalar_float_misc",
            Self::ScalarFloatMul => "scalar_float_mul",
            Self::ScalarFloatFma => "scalar_float_fma",
            Self::ScalarFloatDiv => "scalar_float_div",
            Self::ScalarFloatSqrt => "scalar_float_sqrt",
            Self::VectorIntegerMul => "vector_integer_mul",
            Self::VectorIntegerDiv => "vector_integer_div",
            Self::VectorFloatAdd => "vector_float_add",
            Self::VectorFloatCompare => "vector_float_compare",
            Self::VectorFloatMisc => "vector_float_misc",
            Self::VectorFloatMul => "vector_float_mul",
            Self::VectorFloatFma => "vector_float_fma",
            Self::VectorFloatDiv => "vector_float_div",
            Self::VectorFloatSqrt => "vector_float_sqrt",
        }
    }

    pub const fn stat_stem(self) -> &'static str {
        match self {
            Self::ScalarIntegerMul => "integer_mul",
            Self::ScalarIntegerDiv => "integer_div",
            Self::ScalarFloatAdd => "float_add",
            Self::ScalarFloatCompare => "float_compare",
            Self::ScalarFloatMisc => "float_misc",
            Self::ScalarFloatMul => "float_mul",
            Self::ScalarFloatFma => "float_fma",
            Self::ScalarFloatDiv => "float_div",
            Self::ScalarFloatSqrt => "float_sqrt",
            Self::VectorIntegerMul => "vector_integer_mul",
            Self::VectorIntegerDiv => "vector_integer_div",
            Self::VectorFloatAdd => "vector_float_add",
            Self::VectorFloatCompare => "vector_float_compare",
            Self::VectorFloatMisc => "vector_float_misc",
            Self::VectorFloatMul => "vector_float_mul",
            Self::VectorFloatFma => "vector_float_fma",
            Self::VectorFloatDiv => "vector_float_div",
            Self::VectorFloatSqrt => "vector_float_sqrt",
        }
    }
}

impl O3RuntimeLsqOperation {
    pub const COUNT: usize = 10;
    pub const TRACKED: [Self; 9] = [
        Self::Load,
        Self::Store,
        Self::LoadReserved,
        Self::StoreConditional,
        Self::Atomic,
        Self::FloatLoad,
        Self::FloatStore,
        Self::VectorLoad,
        Self::VectorStore,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::None => 0,
            Self::Load => 1,
            Self::Store => 2,
            Self::LoadReserved => 3,
            Self::StoreConditional => 4,
            Self::Atomic => 5,
            Self::FloatLoad => 6,
            Self::FloatStore => 7,
            Self::VectorLoad => 8,
            Self::VectorStore => 9,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Load => "load",
            Self::Store => "store",
            Self::LoadReserved => "load_reserved",
            Self::StoreConditional => "store_conditional",
            Self::Atomic => "atomic",
            Self::FloatLoad => "float_load",
            Self::FloatStore => "float_store",
            Self::VectorLoad => "vector_load",
            Self::VectorStore => "vector_store",
        }
    }
}

impl O3RuntimeLsqOrdering {
    pub const COUNT: usize = 4;
    pub const TRACKED: [Self; 3] = [Self::Acquire, Self::Release, Self::AcquireRelease];

    pub const fn index(self) -> usize {
        match self {
            Self::None => 0,
            Self::Acquire => 1,
            Self::Release => 2,
            Self::AcquireRelease => 3,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Acquire => "acquire",
            Self::Release => "release",
            Self::AcquireRelease => "acquire_release",
        }
    }

    pub const fn acquire(self) -> bool {
        match self {
            Self::Acquire | Self::AcquireRelease => true,
            Self::None | Self::Release => false,
        }
    }

    pub const fn release(self) -> bool {
        match self {
            Self::Release | Self::AcquireRelease => true,
            Self::None | Self::Acquire => false,
        }
    }
}

impl O3RuntimeTraceRecord {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        sequence: u64,
        tick: u64,
        commit_tick: u64,
        pc: Address,
        rob_occupancy: usize,
        rob_commits_at_tick: usize,
        rob_commit_blocked: bool,
        rename_writes: u64,
        lsq_loads: u64,
        lsq_stores: u64,
        lsq_occupancy: usize,
        lsq_operation: O3RuntimeLsqOperation,
        lsq_ordering: O3RuntimeLsqOrdering,
        lsq_load_address: Option<Address>,
        lsq_store_address: Option<Address>,
        lsq_load_bytes: u64,
        lsq_store_bytes: u64,
        lsq_store_conditional_failed: bool,
        lsq_data_response_tick: u64,
        lsq_data_latency_ticks: u64,
        rename_map_entries: usize,
        iew_dependency_producers: u64,
        iew_dependency_consumers: u64,
        branch_kind: BranchTargetKind,
        branch_predicted_taken: bool,
        branch_resolved_taken: bool,
        branch_link_register_write: bool,
        branch_predicted_target: Option<Address>,
        branch_resolved_target: Option<Address>,
        branch_squashed_target: Option<Address>,
        fu_latency_class: Option<O3RuntimeFuLatencyClass>,
        fu_latency_cycles: u64,
        system_event: bool,
    ) -> Self {
        Self {
            sequence,
            tick,
            commit_tick,
            pc,
            rob_allocated: true,
            rob_committed: true,
            rob_occupancy: u64::try_from(rob_occupancy).unwrap_or(u64::MAX),
            rob_commits_at_tick: u64::try_from(rob_commits_at_tick).unwrap_or(u64::MAX),
            rob_commit_blocked,
            rename_writes,
            lsq_loads,
            lsq_stores,
            lsq_occupancy: u64::try_from(lsq_occupancy).unwrap_or(u64::MAX),
            lsq_operation,
            lsq_ordering,
            lsq_load_address,
            lsq_store_address,
            lsq_load_bytes,
            lsq_store_bytes,
            lsq_store_conditional_failed,
            lsq_data_response_tick,
            lsq_data_latency_ticks,
            rename_map_entries: u64::try_from(rename_map_entries).unwrap_or(u64::MAX),
            store_load_forwarding_candidate: false,
            store_load_forwarding_match: false,
            store_load_forwarding_partial: false,
            store_load_forwarding_bytes: 0,
            store_load_forwarding_suppressed: false,
            store_load_forwarding_address_mismatch: false,
            store_load_forwarding_byte_mismatch: false,
            iew_dependency_producers,
            iew_dependency_consumers,
            branch_kind,
            branch_predicted_taken,
            branch_resolved_taken,
            branch_mispredicted: branch_mispredicted(
                branch_kind,
                branch_predicted_taken,
                branch_resolved_taken,
                branch_predicted_target,
                branch_resolved_target,
            ),
            branch_link_register_write,
            branch_predicted_target,
            branch_resolved_target,
            branch_squashed_target,
            fu_latency_class,
            fu_latency_cycles,
            system_event,
        }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn issue_tick(self) -> u64 {
        self.tick
    }

    pub fn writeback_tick(self) -> u64 {
        self.tick
            .saturating_add(self.fu_latency_cycles)
            .max(self.tick.saturating_add(self.lsq_data_latency_ticks))
            .max(self.lsq_data_response_tick)
    }

    pub const fn commit_tick(self) -> u64 {
        self.commit_tick
    }

    pub fn issue_to_writeback_ticks(self) -> u64 {
        self.writeback_tick().saturating_sub(self.issue_tick())
    }

    pub fn writeback_to_commit_ticks(self) -> u64 {
        self.commit_tick().saturating_sub(self.writeback_tick())
    }

    pub fn issue_to_commit_ticks(self) -> u64 {
        self.commit_tick().saturating_sub(self.issue_tick())
    }

    pub const fn pc(self) -> Address {
        self.pc
    }

    pub const fn rob_allocated(self) -> bool {
        self.rob_allocated
    }

    pub const fn rob_committed(self) -> bool {
        self.rob_committed
    }

    pub const fn rob_occupancy(self) -> u64 {
        self.rob_occupancy
    }

    pub const fn rob_commits_at_tick(self) -> u64 {
        self.rob_commits_at_tick
    }

    pub const fn rob_commit_blocked(self) -> bool {
        self.rob_commit_blocked
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

    pub const fn lsq_occupancy(self) -> u64 {
        self.lsq_occupancy
    }

    pub const fn lsq_operation(self) -> O3RuntimeLsqOperation {
        self.lsq_operation
    }

    pub const fn lsq_ordering(self) -> O3RuntimeLsqOrdering {
        self.lsq_ordering
    }

    pub const fn lsq_load_address(self) -> Option<Address> {
        self.lsq_load_address
    }

    pub const fn lsq_store_address(self) -> Option<Address> {
        self.lsq_store_address
    }

    pub const fn lsq_load_bytes(self) -> u64 {
        self.lsq_load_bytes
    }

    pub const fn lsq_store_bytes(self) -> u64 {
        self.lsq_store_bytes
    }

    pub const fn lsq_store_conditional_failed(self) -> bool {
        self.lsq_store_conditional_failed
    }

    pub(crate) fn set_lsq_store_conditional_failed(&mut self, failed: bool) {
        self.lsq_store_conditional_failed = failed;
    }

    pub const fn lsq_data_response_tick(self) -> u64 {
        self.lsq_data_response_tick
    }

    pub const fn lsq_data_latency_ticks(self) -> u64 {
        self.lsq_data_latency_ticks
    }

    pub(crate) fn set_lsq_data_response(&mut self, response_tick: u64, latency_ticks: u64) {
        self.lsq_data_response_tick = response_tick;
        self.lsq_data_latency_ticks = latency_ticks;
        if self.current_instruction_committed() {
            self.commit_tick = self.commit_tick.max(self.writeback_tick());
        }
    }

    pub const fn rename_map_entries(self) -> u64 {
        self.rename_map_entries
    }

    const fn current_instruction_committed(self) -> bool {
        self.rob_allocated && self.rob_committed && self.rob_commits_at_tick >= self.rob_occupancy
    }

    pub fn structural_pressure_key(self) -> (u64, u64, u64, u64, u64, u64) {
        let active_structures = u64::from(self.rob_occupancy != 0)
            + u64::from(self.lsq_occupancy != 0)
            + u64::from(self.rename_map_entries != 0);
        (
            active_structures,
            self.rob_occupancy
                .saturating_add(self.lsq_occupancy)
                .saturating_add(self.rename_map_entries),
            self.rob_occupancy,
            self.lsq_occupancy,
            self.rename_map_entries,
            self.sequence,
        )
    }

    pub const fn store_load_forwarding_candidate(self) -> bool {
        self.store_load_forwarding_candidate
    }

    pub const fn store_load_forwarding_match(self) -> bool {
        self.store_load_forwarding_match
    }

    pub const fn store_load_forwarding_partial(self) -> bool {
        self.store_load_forwarding_partial
    }

    pub const fn store_load_forwarding_bytes(self) -> u64 {
        self.store_load_forwarding_bytes
    }

    pub const fn store_load_forwarding_suppressed(self) -> bool {
        self.store_load_forwarding_suppressed
    }

    pub const fn store_load_forwarding_address_mismatch(self) -> bool {
        self.store_load_forwarding_address_mismatch
    }

    pub const fn store_load_forwarding_byte_mismatch(self) -> bool {
        self.store_load_forwarding_byte_mismatch
    }

    pub const fn iew_dependency_producers(self) -> u64 {
        self.iew_dependency_producers
    }

    pub const fn iew_dependency_consumers(self) -> u64 {
        self.iew_dependency_consumers
    }

    pub const fn branch_event(self) -> bool {
        !matches!(self.branch_kind, BranchTargetKind::NoBranch)
    }

    pub const fn branch_kind(self) -> BranchTargetKind {
        self.branch_kind
    }

    pub const fn branch_predicted_taken(self) -> bool {
        self.branch_predicted_taken
    }

    pub const fn branch_resolved_taken(self) -> bool {
        self.branch_resolved_taken
    }

    pub const fn branch_mispredicted(self) -> bool {
        self.branch_mispredicted
    }

    pub const fn branch_link_register_write(self) -> bool {
        self.branch_link_register_write
    }

    pub const fn branch_predicted_target(self) -> Option<Address> {
        self.branch_predicted_target
    }

    pub const fn branch_resolved_target(self) -> Option<Address> {
        self.branch_resolved_target
    }

    pub const fn branch_squash(self) -> bool {
        self.branch_squashed_target.is_some()
    }

    pub const fn branch_squashed_target(self) -> Option<Address> {
        self.branch_squashed_target
    }

    pub const fn fu_latency_class(self) -> Option<O3RuntimeFuLatencyClass> {
        self.fu_latency_class
    }

    pub const fn fu_latency_cycles(self) -> u64 {
        self.fu_latency_cycles
    }

    pub const fn system_event(self) -> bool {
        self.system_event
    }

    pub(crate) fn set_store_load_forwarding(
        &mut self,
        candidate: bool,
        matched: bool,
        suppressed: bool,
        address_mismatch: bool,
        byte_mismatch: bool,
    ) {
        self.store_load_forwarding_candidate = candidate;
        self.store_load_forwarding_match = matched;
        self.store_load_forwarding_suppressed = suppressed;
        self.store_load_forwarding_address_mismatch = address_mismatch;
        self.store_load_forwarding_byte_mismatch = byte_mismatch;
    }

    pub(crate) fn set_store_load_forwarding_contribution(
        &mut self,
        partial: bool,
        forwarded_bytes: u64,
    ) {
        self.store_load_forwarding_partial = partial;
        self.store_load_forwarding_bytes = forwarded_bytes;
    }

    pub(crate) fn mark_store_load_forwarding_match(&mut self) {
        self.store_load_forwarding_match = true;
    }
}

fn branch_mispredicted(
    branch_kind: BranchTargetKind,
    predicted_taken: bool,
    resolved_taken: bool,
    predicted_target: Option<Address>,
    resolved_target: Option<Address>,
) -> bool {
    if matches!(branch_kind, BranchTargetKind::NoBranch) {
        return false;
    }
    predicted_taken != resolved_taken || (predicted_taken && predicted_target != resolved_target)
}
