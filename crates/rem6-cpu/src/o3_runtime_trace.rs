use rem6_memory::Address;

use crate::branch_predictor::BranchTargetKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RuntimeTraceRecord {
    sequence: u64,
    tick: u64,
    pc: Address,
    rob_allocated: bool,
    rob_committed: bool,
    rob_occupancy: u64,
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScalarIntegerMul => "scalar_integer_mul",
            Self::ScalarIntegerDiv => "scalar_integer_div",
        }
    }
}

impl O3RuntimeLsqOperation {
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
        pc: Address,
        rob_occupancy: usize,
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
            pc,
            rob_allocated: true,
            rob_committed: true,
            rob_occupancy: u64::try_from(rob_occupancy).unwrap_or(u64::MAX),
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
    }

    pub const fn rename_map_entries(self) -> u64 {
        self.rename_map_entries
    }

    pub const fn store_load_forwarding_candidate(self) -> bool {
        self.store_load_forwarding_candidate
    }

    pub const fn store_load_forwarding_match(self) -> bool {
        self.store_load_forwarding_match
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

    pub(crate) fn set_store_load_forwarding(&mut self, candidate: bool, matched: bool) {
        self.store_load_forwarding_candidate = candidate;
        self.store_load_forwarding_match = matched;
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
