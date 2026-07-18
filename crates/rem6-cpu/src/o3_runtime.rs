use std::collections::{BTreeMap, BTreeSet};

use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvInstruction, RiscvVectorMemoryInstruction,
    RISCV_VECTOR_REGISTER_BYTES,
};
use rem6_memory::{Address, MemoryRequestId};

use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};
use crate::o3_pipeline::O3PendingStateSnapshot;
use crate::o3_runtime_trace::{O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeTraceRecord};
use crate::riscv_branch_kind::is_riscv_link_register;
use crate::riscv_data_completion::apply_data_completion;
use crate::riscv_defaults::{
    DEFAULT_RISCV_O3_ISSUE_WIDTH, MAX_RISCV_O3_ISSUE_WIDTH, MAX_RISCV_O3_WRITEBACK_WIDTH,
    MIN_RISCV_O3_ISSUE_WIDTH, MIN_RISCV_O3_WRITEBACK_WIDTH,
};
use crate::riscv_execution_event::RiscvCpuExecutionEvent;
use crate::riscv_fu_latency::riscv_o3_fu_latency_class as o3_fu_latency_class;
use crate::{branch_predictor::BranchTargetKind, RiscvDataAccessEventKind};

#[path = "o3_runtime_authority.rs"]
mod o3_runtime_authority;
#[path = "o3_runtime_checkpoint.rs"]
mod o3_runtime_checkpoint;
#[path = "o3_runtime_checkpoint_branch_mismatch.rs"]
mod o3_runtime_checkpoint_branch_mismatch;
#[path = "o3_runtime_control_window.rs"]
mod o3_runtime_control_window;
#[cfg(test)]
#[path = "o3_runtime_control_window_tests.rs"]
mod o3_runtime_control_window_tests;
#[path = "o3_runtime_error.rs"]
mod o3_runtime_error;
#[path = "o3_runtime_handoff.rs"]
mod o3_runtime_handoff;
#[path = "o3_runtime_helpers.rs"]
mod o3_runtime_helpers;
#[path = "o3_runtime_issue.rs"]
mod o3_runtime_issue;
#[cfg(test)]
#[path = "o3_runtime_issue_tests.rs"]
mod o3_runtime_issue_tests;
#[path = "o3_runtime_live_window.rs"]
mod o3_runtime_live_window;
#[path = "o3_runtime_memory.rs"]
mod o3_runtime_memory;
#[cfg(test)]
#[path = "o3_runtime_memory_result_tests.rs"]
mod o3_runtime_memory_result_tests;
#[cfg(test)]
#[path = "o3_runtime_memory_tests.rs"]
mod o3_runtime_memory_tests;
#[path = "o3_runtime_memory_window.rs"]
mod o3_runtime_memory_window;
#[path = "o3_runtime_retire.rs"]
mod o3_runtime_retire;
#[path = "o3_runtime_snapshot_entries.rs"]
mod o3_runtime_snapshot_entries;
#[path = "o3_runtime_stats.rs"]
mod o3_runtime_stats;
#[path = "o3_runtime_writeback.rs"]
mod o3_runtime_writeback;
#[cfg(test)]
#[path = "o3_runtime_writeback_tests.rs"]
mod o3_runtime_writeback_tests;
#[path = "o3_source_operands.rs"]
mod o3_source_operands;
#[path = "o3_store_forwarding.rs"]
mod o3_store_forwarding;

pub(crate) use o3_runtime_checkpoint::O3LiveRetireGateCheckpointPayload;
pub use o3_runtime_checkpoint::O3RuntimeCheckpointPayload;
use o3_runtime_control_window::{
    execution_writes_rename_destination, valid_live_speculative_fetch_identity,
    O3LiveSpeculativeExecution, O3LiveSpeculativeIssueCandidate,
};
pub use o3_runtime_error::O3RuntimeError;
use o3_runtime_helpers::{
    default_o3_runtime_snapshot, encode_register_class, encode_u32, rob_commit_boundary,
    rob_commit_tick, validate_live_staged_rob_metadata, validate_runtime_snapshot, validate_unique,
    O3RuntimeUniqueKey,
};
pub(crate) use o3_runtime_issue::{O3LiveIssueHeadReservation, O3LiveIssueRequest};
use o3_runtime_live_window::{
    staged_rename_entry, O3LiveRetiredInstruction, O3LiveStagedFetchIdentity,
};
pub(crate) use o3_runtime_memory::{
    is_deferred_o3_data_access, o3_memory_result_destination, O3DataAccessWindowPolicy,
};
use o3_runtime_memory::{
    is_deferred_o3_data_instruction, is_terminal_o3_data_access_event,
    o3_instruction_sequence_span, O3LiveDataAccess, O3LiveDataAccessOutcome,
};
pub use o3_runtime_snapshot_entries::{
    O3LoadStoreQueueEntry, O3LoadStoreQueueKind, O3RenameMapEntry, O3ReorderBufferEntry,
};
pub use o3_runtime_stats::O3RuntimeStats;
use o3_runtime_writeback::O3FinalizedWritebackPortStats;
pub(crate) use o3_runtime_writeback::O3WritebackReservationCalendar;
#[cfg(test)]
pub(crate) use o3_runtime_writeback::{O3LiveWritebackReady, O3WritebackReservation};
pub use o3_runtime_writeback::{O3RuntimeWritebackReservation, RiscvO3WritebackDebugState};
pub(crate) use o3_source_operands::{
    o3_live_control_operands, o3_predicted_scalar_descendant_operands,
    o3_scalar_integer_destination, o3_scalar_integer_source_registers,
    o3_speculative_scalar_alu_operands,
};
pub(crate) use o3_store_forwarding::O3StoreLoadForwardingPlan;
use o3_store_forwarding::{
    o3_load_forwarding_access, o3_store_forwarding_entry, O3StoreForwardingEntry,
};
const O3_RUNTIME_U32_MAX: usize = u32::MAX as usize;
const DEFAULT_O3_SCALAR_MEMORY_DEPTH: usize = 2;
const MAX_O3_SCALAR_MEMORY_DEPTH: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeSnapshot {
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    load_store_queue: Vec<O3LoadStoreQueueEntry>,
    rename_map: Vec<O3RenameMapEntry>,
    committed_rename_map: Option<Vec<O3RenameMapEntry>>,
    pending_state: O3PendingStateSnapshot,
}

impl O3RuntimeSnapshot {
    pub fn new<R, L, M>(
        reorder_buffer: R,
        load_store_queue: L,
        rename_map: M,
        pending_state: O3PendingStateSnapshot,
    ) -> Result<Self, O3RuntimeError>
    where
        R: IntoIterator<Item = O3ReorderBufferEntry>,
        L: IntoIterator<Item = O3LoadStoreQueueEntry>,
        M: IntoIterator<Item = O3RenameMapEntry>,
    {
        let mut reorder_buffer = reorder_buffer.into_iter().collect::<Vec<_>>();
        reorder_buffer.sort_by_key(|entry| entry.sequence());
        validate_unique(
            "ROB",
            reorder_buffer
                .iter()
                .map(|entry| O3RuntimeUniqueKey::Sequence(entry.sequence())),
        )?;

        let mut load_store_queue = load_store_queue.into_iter().collect::<Vec<_>>();
        load_store_queue.sort_by_key(|entry| entry.sequence());
        validate_unique(
            "LSQ",
            load_store_queue
                .iter()
                .map(|entry| O3RuntimeUniqueKey::Sequence(entry.sequence())),
        )?;

        let mut rename_map = rename_map.into_iter().collect::<Vec<_>>();
        rename_map.sort_by_key(|entry| {
            (
                encode_register_class(entry.register_class()),
                entry.architectural(),
            )
        });
        validate_unique(
            "rename_map",
            rename_map.iter().map(|entry| {
                O3RuntimeUniqueKey::Rename(entry.register_class(), entry.architectural())
            }),
        )?;

        let snapshot = Self {
            reorder_buffer,
            load_store_queue,
            rename_map,
            committed_rename_map: None,
            pending_state,
        };
        validate_runtime_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn reorder_buffer(&self) -> &[O3ReorderBufferEntry] {
        &self.reorder_buffer
    }

    pub fn load_store_queue(&self) -> &[O3LoadStoreQueueEntry] {
        &self.load_store_queue
    }

    pub fn rename_map(&self) -> &[O3RenameMapEntry] {
        &self.rename_map
    }

    pub const fn pending_state(&self) -> &O3PendingStateSnapshot {
        &self.pending_state
    }

    fn with_committed_rename_map(mut self, committed_rename_map: Vec<O3RenameMapEntry>) -> Self {
        if self.rename_map != committed_rename_map {
            self.committed_rename_map = Some(committed_rename_map);
        }
        self
    }

    fn into_checkpoint_snapshot(mut self) -> Self {
        if let Some(committed_rename_map) = self.committed_rename_map.take() {
            self.rename_map = committed_rename_map;
        }
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeState {
    snapshot: O3RuntimeSnapshot,
    stats: O3RuntimeStats,
    trace_records: Vec<O3RuntimeTraceRecord>,
    dirty_trace_record_indices: BTreeSet<usize>,
    pending_data_accesses: BTreeMap<MemoryRequestId, O3PendingDataAccess>,
    store_forwarding_window: O3StoreForwardingWindow,
    dependency_producers_with_consumers: BTreeSet<O3PhysicalRegisterId>,
    live_retired_instructions: Vec<O3LiveRetiredInstruction>,
    live_speculative_executions: Vec<O3LiveSpeculativeExecution>,
    live_issue_cycle_ticks: BTreeSet<u64>,
    writeback_calendar: O3WritebackReservationCalendar,
    published_writeback_sequences: BTreeSet<u64>,
    live_writeback_counted_sequences: BTreeSet<u64>,
    finalized_writeback_port_stats: O3FinalizedWritebackPortStats,
    live_writeback_cycle_ticks: BTreeSet<u64>,
    live_writeback_ready_rows_by_tick: BTreeMap<u64, BTreeSet<u64>>,
    live_control_dependencies: BTreeMap<u64, u64>,
    live_control_window_sequences: BTreeSet<u64>,
    live_serializing_control_sequences: BTreeSet<u64>,
    live_staged_fetch_identities: BTreeMap<u64, O3LiveStagedFetchIdentity>,
    invalidated_live_staged_fetch_identities: BTreeMap<u64, O3LiveStagedFetchIdentity>,
    deferred_live_data_access_execution: Option<MemoryRequestId>,
    live_data_accesses: Vec<O3LiveDataAccess>,
    live_data_access_younger_sequences: BTreeSet<u64>,
    scalar_memory_window_limit: usize,
    scalar_memory_window_limit_explicit: bool,
    issue_width: usize,
    last_live_commit_tick: Option<u64>,
    next_sequence: u64,
    next_physical_register: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3PendingDataAccess {
    trace_sequence: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct O3ScalarIntegerDependencyObservation {
    producer_physical_registers: Vec<O3PhysicalRegisterId>,
    newly_observed_producers: u64,
    consumers: u64,
}

impl O3RuntimeState {
    pub fn restore(&mut self, snapshot: O3RuntimeSnapshot) -> Result<(), O3RuntimeError> {
        let snapshot = snapshot.into_checkpoint_snapshot();
        validate_runtime_snapshot(&snapshot)?;
        self.next_sequence = next_runtime_sequence(&snapshot);
        self.next_physical_register = next_runtime_physical_register(&snapshot);
        self.snapshot = snapshot;
        self.trace_records.clear();
        self.dirty_trace_record_indices.clear();
        self.pending_data_accesses.clear();
        self.store_forwarding_window = O3StoreForwardingWindow::default();
        self.dependency_producers_with_consumers.clear();
        self.live_retired_instructions.clear();
        self.live_speculative_executions.clear();
        self.live_issue_cycle_ticks.clear();
        self.clear_all_writeback_state();
        self.seed_finalized_writeback_stats_from_aggregate();
        self.live_control_dependencies.clear();
        self.live_control_window_sequences.clear();
        self.live_serializing_control_sequences.clear();
        self.live_staged_fetch_identities.clear();
        self.invalidated_live_staged_fetch_identities.clear();
        self.deferred_live_data_access_execution = None;
        self.live_data_accesses.clear();
        self.live_data_access_younger_sequences.clear();
        self.last_live_commit_tick = None;
        Ok(())
    }

    pub fn restore_checkpoint_payload(
        &mut self,
        payload: O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        let stats = payload.stats();
        let dependency_producers_with_consumers =
            payload.dependency_producers_with_consumers().clone();
        self.restore(payload.into_snapshot())?;
        self.stats = stats;
        self.seed_finalized_writeback_stats_from_aggregate();
        self.dependency_producers_with_consumers = dependency_producers_with_consumers;
        Ok(())
    }

    pub fn snapshot(&self) -> O3RuntimeSnapshot {
        self.snapshot_with_live_rename_map()
    }

    pub const fn stats(&self) -> O3RuntimeStats {
        self.stats
    }

    #[cfg(test)]
    pub(crate) const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub(crate) fn set_issue_width(&mut self, issue_width: usize) -> bool {
        if !(MIN_RISCV_O3_ISSUE_WIDTH..=MAX_RISCV_O3_ISSUE_WIDTH).contains(&issue_width) {
            return false;
        }
        self.issue_width = issue_width;
        true
    }

    pub(crate) const fn writeback_width(&self) -> usize {
        self.snapshot
            .pending_state()
            .writeback()
            .policy()
            .writeback_width()
    }

    pub(crate) fn set_writeback_width(&mut self, writeback_width: usize) -> bool {
        if !(MIN_RISCV_O3_WRITEBACK_WIDTH..=MAX_RISCV_O3_WRITEBACK_WIDTH).contains(&writeback_width)
        {
            return false;
        }
        if !self.writeback_calendar.is_empty()
            || !self.live_writeback_ready_rows_by_tick.is_empty()
            || !self.live_writeback_cycle_ticks.is_empty()
        {
            return false;
        }

        self.rebuild_writeback_policy(writeback_width)
            .expect("rebuilt O3 pending-state snapshot is valid");
        debug_assert_eq!(self.writeback_width(), writeback_width);
        true
    }

    pub fn trace_records(&self) -> &[O3RuntimeTraceRecord] {
        &self.trace_records
    }

    pub fn take_trace_updates(&mut self, start: usize) -> (usize, Vec<O3RuntimeTraceRecord>) {
        let start = start.min(self.trace_records.len());
        let mut records = self.trace_records[start..].to_vec();
        let dirty = std::mem::take(&mut self.dirty_trace_record_indices);
        records.extend(
            dirty
                .into_iter()
                .filter(|index| *index < start)
                .filter_map(|index| self.trace_records.get(index).copied()),
        );
        (self.trace_records.len(), records)
    }

    #[cfg(test)]
    pub(crate) fn pending_trace_data_access_outcomes(&self) -> usize {
        self.pending_data_accesses
            .values()
            .filter(|pending| pending.trace_sequence.is_some())
            .count()
    }

    pub(crate) fn record_data_access_outcome(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        response_tick: u64,
        latency_ticks: u64,
    ) {
        let Some(pending) = self
            .pending_data_accesses
            .remove(&execution.fetch().request_id())
        else {
            return;
        };
        if let Some(sequence) = pending.trace_sequence {
            self.mark_trace_data_access_outcome(sequence, response_tick, latency_ticks);
        }
        if let Some(access) = execution.execution().memory_access() {
            self.stats
                .record_lsq_operation_latency(o3_lsq_operation(access), latency_ticks);
        }
        if o3_store_conditional_failed(execution) {
            self.stats.record_store_conditional_failure();
            if let Some(sequence) = pending.trace_sequence {
                self.mark_trace_store_conditional_failed(sequence);
            }
        }
    }

    pub(crate) fn discard_data_access_outcome(&mut self, fetch_request: MemoryRequestId) {
        self.pending_data_accesses.remove(&fetch_request);
    }

    pub fn reset_stats(&mut self) {
        self.stats = O3RuntimeStats::default();
        self.live_issue_cycle_ticks.clear();
        self.reset_writeback_stats_ownership();
        let live_rob_occupancy = self
            .live_retired_instructions
            .iter()
            .map(|instruction| instruction.rob_occupancy)
            .max()
            .unwrap_or_default()
            .max(self.snapshot.reorder_buffer.len());
        self.stats.observe_rob_occupancy(live_rob_occupancy);
        self.stats
            .observe_lsq_occupancy(self.snapshot.load_store_queue.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        self.dependency_producers_with_consumers.clear();
        for instruction in &self.live_retired_instructions {
            for physical in &instruction.iew_dependency_producer_registers {
                if self.dependency_producers_with_consumers.insert(*physical) {
                    self.stats.record_iew_dependency_producer();
                }
            }
            for _ in 0..instruction.iew_dependency_consumers {
                self.stats.record_iew_dependency_consumer();
            }
        }
        self.trace_records.clear();
        self.dirty_trace_record_indices.clear();
        for pending in self.pending_data_accesses.values_mut() {
            pending.trace_sequence = None;
        }
        self.store_forwarding_window = O3StoreForwardingWindow::default();
    }

    pub(crate) fn record_live_retire_gate_wait(&mut self, wait_ticks: u64) {
        self.stats.record_live_retire_gate_wait(wait_ticks);
    }

    pub fn record_retired_instruction(&mut self, execution: &RiscvCpuExecutionEvent) {
        self.record_retired_instruction_with_trace(execution, false);
    }

    pub fn record_retired_instruction_with_trace(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_enabled: bool,
    ) {
        let live_data_access = self.consume_live_data_access_retirement(execution);
        if live_data_access.is_none() && is_terminal_o3_data_access_event(execution) {
            return;
        }
        if live_data_access.as_ref().is_some_and(|live| {
            matches!(
                live.outcome,
                O3LiveDataAccessOutcome::Retried | O3LiveDataAccessOutcome::Failed
            )
        }) {
            return;
        }
        let completed_live_data_access = live_data_access
            .as_ref()
            .filter(|live| live.outcome == O3LiveDataAccessOutcome::Completed);
        let mut trace_record = self.record_runtime_state(execution, completed_live_data_access);
        if let Some(tick) = completed_live_data_access.and_then(|live| live.admitted_writeback_tick)
        {
            trace_record.set_admitted_writeback_tick(tick);
        }
        self.stats
            .record_retired_instruction(execution, trace_record);
        let observation = self
            .take_prepared_store_forwarding_observation(execution)
            .unwrap_or_else(|| {
                self.record_store_forwarding_window(
                    execution,
                    trace_enabled.then_some(trace_record.sequence()),
                    completed_live_data_access.and_then(|live| live.forwarding_plan),
                )
            });
        trace_record.set_store_load_forwarding(
            observation.candidate,
            observation.matched,
            observation.suppressed,
            observation.address_mismatch,
            observation.byte_mismatch,
        );
        trace_record.set_store_load_forwarding_contribution(
            observation.partial,
            observation.forwarded_bytes,
        );
        if completed_live_data_access.is_none() {
            self.record_pending_data_access(
                execution,
                trace_enabled.then_some(trace_record.sequence()),
            );
        }
        if trace_enabled {
            self.trace_records.push(trace_record);
        }
    }

    fn record_store_forwarding_window(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_sequence: Option<u64>,
        forwarding_plan: Option<O3StoreLoadForwardingPlan>,
    ) -> O3StoreForwardingObservation {
        let record = execution.execution();
        match record.memory_access() {
            Some(access @ MemoryAccessKind::Load { .. }) => {
                let (observation, pending_load_match) = self.stats.record_store_to_load_forwarding(
                    access,
                    execution.fetch().request_id(),
                    record.register_writes(),
                    self.store_forwarding_window.store,
                    trace_sequence,
                    forwarding_plan,
                );
                self.store_forwarding_window.pending_load_match = pending_load_match;
                self.store_forwarding_window.store = None;
                observation
            }
            Some(access) => {
                self.store_forwarding_window.store = o3_store_forwarding_entry(access);
                O3StoreForwardingObservation::default()
            }
            None => {
                self.store_forwarding_window.store = None;
                O3StoreForwardingObservation::default()
            }
        }
    }

    fn prepare_ready_live_data_access_forwarding_matcher(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) {
        if !matches!(
            execution.execution().memory_access(),
            Some(MemoryAccessKind::Load { .. })
        ) {
            return;
        }
        let forwarding_plan = self
            .live_data_accesses
            .iter()
            .find(|live| {
                live.fetch_request == execution.fetch().request_id()
                    && live.outcome == O3LiveDataAccessOutcome::Completed
            })
            .and_then(|live| live.forwarding_plan);
        let observation = self.record_store_forwarding_window(execution, None, forwarding_plan);
        self.store_forwarding_window.prepared_load = Some(O3PreparedLoadForwarding {
            fetch_request: execution.fetch().request_id(),
            observation,
        });
    }

    fn take_prepared_store_forwarding_observation(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> Option<O3StoreForwardingObservation> {
        let prepared = self.store_forwarding_window.prepared_load?;
        if prepared.fetch_request != execution.fetch().request_id() {
            return None;
        }
        self.store_forwarding_window.prepared_load = None;
        Some(prepared.observation)
    }

    pub fn record_completed_load_data(
        &mut self,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
        data: &[u8],
    ) {
        let Some(pending) = self.store_forwarding_window.pending_load_match else {
            return;
        };
        if pending.fetch_request != fetch_request {
            return;
        }
        self.store_forwarding_window.pending_load_match = None;

        let Some(load) = o3_load_forwarding_access(access) else {
            return;
        };
        if load.range() != pending.plan.load_range() {
            return;
        }

        if pending.plan.matches_data(data) {
            self.stats
                .record_store_to_load_forwarding_match(pending.operation);
            if let Some(prepared) = self.store_forwarding_window.prepared_load.as_mut() {
                if prepared.fetch_request == fetch_request {
                    prepared.observation.matched = true;
                }
            }
            self.mark_trace_store_forwarding_match(pending.trace_sequence);
        }
    }

    fn mark_trace_store_forwarding_match(&mut self, trace_sequence: Option<u64>) {
        let Some(sequence) = trace_sequence else {
            return;
        };
        if let Some(record) = self
            .trace_records
            .iter_mut()
            .find(|record| record.sequence() == sequence)
        {
            record.mark_store_load_forwarding_match();
        }
    }

    fn record_pending_data_access(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_sequence: Option<u64>,
    ) {
        if execution.execution().memory_access().is_some() {
            self.pending_data_accesses.insert(
                execution.fetch().request_id(),
                O3PendingDataAccess { trace_sequence },
            );
        }
    }

    fn mark_trace_data_access_outcome(
        &mut self,
        sequence: u64,
        response_tick: u64,
        latency_ticks: u64,
    ) {
        if let Some((index, record)) = self
            .trace_records
            .iter_mut()
            .enumerate()
            .find(|(_, record)| record.sequence() == sequence)
        {
            record.set_lsq_data_response(response_tick, latency_ticks);
            self.dirty_trace_record_indices.insert(index);
        }
    }

    fn mark_trace_store_conditional_failed(&mut self, sequence: u64) {
        if let Some((index, record)) = self
            .trace_records
            .iter_mut()
            .enumerate()
            .find(|(_, record)| record.sequence() == sequence)
        {
            record.set_lsq_store_conditional_failed(true);
            self.dirty_trace_record_indices.insert(index);
        }
    }
}

impl Default for O3RuntimeState {
    fn default() -> Self {
        Self {
            snapshot: default_o3_runtime_snapshot(),
            stats: O3RuntimeStats::default(),
            trace_records: Vec::new(),
            dirty_trace_record_indices: BTreeSet::new(),
            pending_data_accesses: BTreeMap::new(),
            store_forwarding_window: O3StoreForwardingWindow::default(),
            dependency_producers_with_consumers: BTreeSet::new(),
            live_retired_instructions: Vec::new(),
            live_speculative_executions: Vec::new(),
            live_issue_cycle_ticks: BTreeSet::new(),
            writeback_calendar: O3WritebackReservationCalendar::default(),
            published_writeback_sequences: BTreeSet::new(),
            live_writeback_counted_sequences: BTreeSet::new(),
            finalized_writeback_port_stats: O3FinalizedWritebackPortStats::default(),
            live_writeback_cycle_ticks: BTreeSet::new(),
            live_writeback_ready_rows_by_tick: BTreeMap::new(),
            live_control_dependencies: BTreeMap::new(),
            live_control_window_sequences: BTreeSet::new(),
            live_serializing_control_sequences: BTreeSet::new(),
            live_staged_fetch_identities: BTreeMap::new(),
            invalidated_live_staged_fetch_identities: BTreeMap::new(),
            deferred_live_data_access_execution: None,
            live_data_accesses: Vec::new(),
            live_data_access_younger_sequences: BTreeSet::new(),
            scalar_memory_window_limit: DEFAULT_O3_SCALAR_MEMORY_DEPTH,
            scalar_memory_window_limit_explicit: false,
            issue_width: DEFAULT_RISCV_O3_ISSUE_WIDTH,
            last_live_commit_tick: None,
            next_sequence: 0,
            next_physical_register: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3StoreForwardingWindow {
    store: Option<O3StoreForwardingEntry>,
    pending_load_match: Option<O3PendingLoadForwardingMatch>,
    prepared_load: Option<O3PreparedLoadForwarding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3PreparedLoadForwarding {
    fetch_request: MemoryRequestId,
    observation: O3StoreForwardingObservation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3StoreForwardingObservation {
    candidate: bool,
    matched: bool,
    partial: bool,
    forwarded_bytes: u64,
    suppressed: bool,
    address_mismatch: bool,
    byte_mismatch: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3PendingLoadForwardingMatch {
    fetch_request: MemoryRequestId,
    plan: O3StoreLoadForwardingPlan,
    operation: O3RuntimeLsqOperation,
    trace_sequence: Option<u64>,
}

fn next_runtime_sequence(snapshot: &O3RuntimeSnapshot) -> u64 {
    snapshot
        .reorder_buffer()
        .iter()
        .map(|entry| entry.sequence())
        .chain(
            snapshot
                .load_store_queue()
                .iter()
                .map(|entry| entry.sequence()),
        )
        .max()
        .map_or(0, |sequence| sequence.saturating_add(1))
}

fn next_runtime_physical_register(snapshot: &O3RuntimeSnapshot) -> u32 {
    snapshot
        .rename_map()
        .iter()
        .map(|entry| entry.physical().get())
        .chain(
            snapshot
                .reorder_buffer()
                .iter()
                .filter_map(|entry| entry.destination())
                .map(O3PhysicalRegisterId::get),
        )
        .filter(|physical| *physical != u32::MAX)
        .max()
        .map_or(1, |physical| physical.saturating_add(1))
}

fn o3_execution_writes_link_register(record: &rem6_isa_riscv::RiscvExecutionRecord) -> bool {
    record
        .register_writes()
        .iter()
        .any(|write| is_riscv_link_register(write.register()))
}

fn o3_branch_squashed_target(
    branch_kind: BranchTargetKind,
    predicted_taken: bool,
    predicted_target: Option<Address>,
    resolved_taken: bool,
    resolved_target: Option<Address>,
    fallthrough_target: Address,
) -> Option<Address> {
    if matches!(branch_kind, BranchTargetKind::NoBranch) {
        return None;
    }
    let mispredicted = predicted_taken != resolved_taken
        || (predicted_taken && predicted_target != resolved_target);
    if !mispredicted {
        return None;
    }
    if predicted_taken {
        predicted_target
    } else {
        Some(fallthrough_target)
    }
}

fn o3_rename_write_count(record: &rem6_isa_riscv::RiscvExecutionRecord) -> u64 {
    if record.system_event().is_some() {
        return 0;
    }

    let immediate_writes =
        (record.register_writes().len() + record.float_register_writes().len()) as u64;
    let memory_writes = record
        .memory_access()
        .map(o3_memory_destination_writes)
        .unwrap_or(0);
    immediate_writes.saturating_add(memory_writes)
}

fn o3_memory_destination_writes(access: &MemoryAccessKind) -> u64 {
    match access {
        MemoryAccessKind::Load { rd, .. }
        | MemoryAccessKind::LoadReserved { rd, .. }
        | MemoryAccessKind::StoreConditional { rd, .. }
        | MemoryAccessKind::AtomicMemory { rd, .. } => integer_destination_count(*rd),
        MemoryAccessKind::FloatLoad { .. } => 1,
        MemoryAccessKind::VectorLoadUnitStride {
            group_registers, ..
        }
        | MemoryAccessKind::VectorLoadStrided {
            group_registers, ..
        }
        | MemoryAccessKind::VectorLoadIndexed {
            group_registers, ..
        } => vector_destination_count(*group_registers),
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            fields,
            group_registers,
            ..
        } => vector_destination_count(*fields)
            .saturating_mul(vector_destination_count(*group_registers)),
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => 0,
    }
}

fn o3_memory_destination_registers(access: &MemoryAccessKind) -> Vec<(O3RegisterClass, u32)> {
    match access {
        MemoryAccessKind::Load { rd, .. }
        | MemoryAccessKind::LoadReserved { rd, .. }
        | MemoryAccessKind::StoreConditional { rd, .. }
        | MemoryAccessKind::AtomicMemory { rd, .. } => {
            if rd.is_zero() {
                Vec::new()
            } else {
                vec![(O3RegisterClass::Integer, u32::from(rd.index()))]
            }
        }
        MemoryAccessKind::FloatLoad { rd, .. } => {
            vec![(O3RegisterClass::FloatingPoint, u32::from(rd.index()))]
        }
        MemoryAccessKind::VectorLoadUnitStride {
            vd,
            group_registers,
            ..
        }
        | MemoryAccessKind::VectorLoadStrided {
            vd,
            group_registers,
            ..
        }
        | MemoryAccessKind::VectorLoadIndexed {
            vd,
            group_registers,
            ..
        } => vector_rename_registers(*vd, *group_registers),
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd,
            fields,
            group_registers,
            ..
        } => vector_rename_registers(*vd, fields.saturating_mul(*group_registers)),
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => Vec::new(),
    }
}

fn vector_rename_registers(
    register: rem6_isa_riscv::VectorRegister,
    count: usize,
) -> Vec<(O3RegisterClass, u32)> {
    (0..count)
        .map(|offset| {
            (
                O3RegisterClass::Vector,
                u32::from(register.index())
                    .saturating_add(u32::try_from(offset).unwrap_or(u32::MAX)),
            )
        })
        .collect()
}

fn o3_lsq_entries(sequence: u64, access: &MemoryAccessKind) -> Vec<O3LoadStoreQueueEntry> {
    match access {
        MemoryAccessKind::Load { address, width, .. }
        | MemoryAccessKind::FloatLoad { address, width, .. }
        | MemoryAccessKind::LoadReserved { address, width, .. } => {
            vec![o3_lsq_load(sequence, *address, width.bytes())]
        }
        MemoryAccessKind::VectorLoadUnitStride {
            address, byte_len, ..
        }
        | MemoryAccessKind::VectorLoadSegmentUnitStride {
            address, byte_len, ..
        } => {
            vec![o3_lsq_load(sequence, *address, *byte_len)]
        }
        MemoryAccessKind::VectorLoadStrided {
            address, span_len, ..
        }
        | MemoryAccessKind::VectorLoadIndexed {
            address, span_len, ..
        } => {
            vec![o3_lsq_load(sequence, *address, *span_len)]
        }
        MemoryAccessKind::Store { address, width, .. }
        | MemoryAccessKind::FloatStore { address, width, .. }
        | MemoryAccessKind::StoreConditional { address, width, .. } => {
            vec![o3_lsq_store(sequence, *address, width.bytes())]
        }
        MemoryAccessKind::VectorStoreUnitStride { address, data, .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { address, data, .. }
        | MemoryAccessKind::VectorStoreStrided { address, data, .. }
        | MemoryAccessKind::VectorStoreIndexed { address, data, .. } => {
            vec![o3_lsq_store(sequence, *address, data.len())]
        }
        MemoryAccessKind::AtomicMemory { address, width, .. } => vec![
            o3_lsq_load(sequence, *address, width.bytes()),
            o3_lsq_store(sequence.saturating_add(1), *address, width.bytes()),
        ],
    }
}

fn o3_lsq_load(sequence: u64, address: u64, bytes: usize) -> O3LoadStoreQueueEntry {
    O3LoadStoreQueueEntry::load(sequence, Some(Address::new(address)), o3_lsq_bytes(bytes))
}

fn o3_lsq_store(sequence: u64, address: u64, bytes: usize) -> O3LoadStoreQueueEntry {
    O3LoadStoreQueueEntry::store(sequence, Some(Address::new(address)), o3_lsq_bytes(bytes))
}

fn o3_lsq_bytes(bytes: usize) -> u32 {
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

const fn integer_destination_count(register: Register) -> u64 {
    if register.is_zero() {
        0
    } else {
        1
    }
}

fn vector_destination_count(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

const fn o3_lsq_access_counts(access: &MemoryAccessKind) -> (u64, u64) {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. }
        | MemoryAccessKind::LoadReserved { .. } => (1, 0),
        MemoryAccessKind::StoreConditional { .. }
        | MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => (0, 1),
        MemoryAccessKind::AtomicMemory { .. } => (1, 1),
    }
}

const fn o3_lsq_operation(access: &MemoryAccessKind) -> O3RuntimeLsqOperation {
    match access {
        MemoryAccessKind::Load { .. } => O3RuntimeLsqOperation::Load,
        MemoryAccessKind::Store { .. } => O3RuntimeLsqOperation::Store,
        MemoryAccessKind::LoadReserved { .. } => O3RuntimeLsqOperation::LoadReserved,
        MemoryAccessKind::StoreConditional { .. } => O3RuntimeLsqOperation::StoreConditional,
        MemoryAccessKind::AtomicMemory { .. } => O3RuntimeLsqOperation::Atomic,
        MemoryAccessKind::FloatLoad { .. } => O3RuntimeLsqOperation::FloatLoad,
        MemoryAccessKind::FloatStore { .. } => O3RuntimeLsqOperation::FloatStore,
        MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. } => O3RuntimeLsqOperation::VectorLoad,
        MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => O3RuntimeLsqOperation::VectorStore,
    }
}

const fn o3_lsq_ordering(access: &MemoryAccessKind) -> O3RuntimeLsqOrdering {
    let (acquire, release) = match access {
        MemoryAccessKind::LoadReserved {
            acquire, release, ..
        }
        | MemoryAccessKind::StoreConditional {
            acquire, release, ..
        }
        | MemoryAccessKind::AtomicMemory {
            acquire, release, ..
        } => (*acquire, *release),
        _ => (false, false),
    };
    match (acquire, release) {
        (false, false) => O3RuntimeLsqOrdering::None,
        (true, false) => O3RuntimeLsqOrdering::Acquire,
        (false, true) => O3RuntimeLsqOrdering::Release,
        (true, true) => O3RuntimeLsqOrdering::AcquireRelease,
    }
}

fn o3_lsq_access_bytes(access: &MemoryAccessKind) -> (u64, u64) {
    match access {
        MemoryAccessKind::Load { width, .. }
        | MemoryAccessKind::FloatLoad { width, .. }
        | MemoryAccessKind::LoadReserved { width, .. } => (usize_to_u64(width.bytes()), 0),
        MemoryAccessKind::VectorLoadUnitStride { byte_len, .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { byte_len, .. } => {
            (usize_to_u64(*byte_len), 0)
        }
        MemoryAccessKind::VectorLoadStrided { span_len, .. }
        | MemoryAccessKind::VectorLoadIndexed { span_len, .. } => (usize_to_u64(*span_len), 0),
        MemoryAccessKind::Store { width, .. }
        | MemoryAccessKind::FloatStore { width, .. }
        | MemoryAccessKind::StoreConditional { width, .. } => (0, usize_to_u64(width.bytes())),
        MemoryAccessKind::VectorStoreUnitStride { data, .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { data, .. }
        | MemoryAccessKind::VectorStoreStrided { data, .. }
        | MemoryAccessKind::VectorStoreIndexed { data, .. } => (0, usize_to_u64(data.len())),
        MemoryAccessKind::AtomicMemory { width, .. } => {
            let bytes = usize_to_u64(width.bytes());
            (bytes, bytes)
        }
    }
}

fn o3_lsq_access_addresses(access: &MemoryAccessKind) -> (Option<Address>, Option<Address>) {
    match access {
        MemoryAccessKind::Load { address, .. }
        | MemoryAccessKind::FloatLoad { address, .. }
        | MemoryAccessKind::LoadReserved { address, .. }
        | MemoryAccessKind::VectorLoadUnitStride { address, .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { address, .. }
        | MemoryAccessKind::VectorLoadStrided { address, .. }
        | MemoryAccessKind::VectorLoadIndexed { address, .. } => {
            (Some(Address::new(*address)), None)
        }
        MemoryAccessKind::Store { address, .. }
        | MemoryAccessKind::FloatStore { address, .. }
        | MemoryAccessKind::StoreConditional { address, .. }
        | MemoryAccessKind::VectorStoreUnitStride { address, .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { address, .. }
        | MemoryAccessKind::VectorStoreStrided { address, .. }
        | MemoryAccessKind::VectorStoreIndexed { address, .. } => {
            (None, Some(Address::new(*address)))
        }
        MemoryAccessKind::AtomicMemory { address, .. } => {
            let address = Address::new(*address);
            (Some(address), Some(address))
        }
    }
}

fn o3_store_conditional_failed(execution: &RiscvCpuExecutionEvent) -> bool {
    matches!(
        execution.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::ConditionalFailed)
    ) && matches!(
        execution.execution().memory_access(),
        Some(MemoryAccessKind::StoreConditional { .. })
    )
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "o3_runtime_tests.rs"]
mod o3_runtime_tests;

impl crate::RiscvCore {
    fn with_o3_runtime<T>(&self, f: impl FnOnce(&mut O3RuntimeState) -> T) -> T {
        let mut state = self.state.lock().expect("riscv core lock");
        f(&mut state.o3_runtime)
    }

    pub fn o3_runtime_stats(&self) -> O3RuntimeStats {
        self.with_o3_runtime(|runtime| runtime.stats())
    }

    pub fn o3_runtime_snapshot(&self) -> O3RuntimeSnapshot {
        self.with_o3_runtime(|runtime| runtime.snapshot())
    }

    pub fn o3_runtime_trace_records(&self) -> Vec<O3RuntimeTraceRecord> {
        self.with_o3_runtime(|runtime| runtime.trace_records().to_vec())
    }

    pub fn take_o3_runtime_trace_updates(
        &self,
        start: usize,
    ) -> (usize, Vec<O3RuntimeTraceRecord>) {
        self.with_o3_runtime(|runtime| runtime.take_trace_updates(start))
    }

    pub fn reset_o3_runtime_stats(&self) {
        self.with_o3_runtime(O3RuntimeState::reset_stats);
    }

    pub fn record_o3_retired_instruction(&self, execution: &RiscvCpuExecutionEvent) {
        self.with_o3_runtime(|runtime| runtime.record_retired_instruction(execution));
    }

    pub fn record_o3_retired_instruction_with_trace(
        &self,
        execution: &RiscvCpuExecutionEvent,
        trace_enabled: bool,
    ) {
        self.with_o3_runtime(|runtime| {
            runtime.record_retired_instruction_with_trace(execution, trace_enabled);
        });
    }

    pub fn record_ready_o3_data_access_event_with_trace(
        &self,
        current_tick: u64,
        trace_enabled: bool,
    ) -> Option<RiscvCpuExecutionEvent> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_terminal_memory_result.is_some()
            || !state
                .o3_runtime
                .live_data_access_publication_is_admitted(current_tick)
        {
            return None;
        }
        let mut wake_tick = current_tick;
        if let Some((fetch_request, issue_tick, publication_tick)) =
            state.o3_runtime.ready_live_data_access_completion_timing()
        {
            wake_tick = publication_tick;
            let execution = crate::riscv_data_issue::record_deferred_o3_data_retire_cycle(
                &mut state,
                fetch_request,
                issue_tick,
                publication_tick,
            )
            .expect("completed O3 data access has a matching execution event");
            assert!(
                state
                    .o3_runtime
                    .replace_ready_live_data_access_execution(&execution),
                "completed O3 data access accepts its ordered pipeline retirement"
            );
        }
        let memory_result = state.o3_runtime.ready_live_memory_result_completion();
        let execution = state
            .o3_runtime
            .take_ready_live_data_access_event(current_tick)?;
        if let Some(completion) = memory_result {
            state
                .o3_runtime
                .prepare_ready_live_data_access_forwarding_matcher(&execution);
            apply_data_completion(
                &mut state,
                self.id(),
                &completion,
                "deferred O3 memory-result data",
            );
            crate::riscv_checker::sync_checker_hart(&mut state);
        }
        state.wake_ready_o3_data_access_younger_window(wake_tick, &fetch_events);
        state
            .o3_runtime
            .record_retired_instruction_with_trace(&execution, trace_enabled);
        state.refresh_o3_writeback_wake(current_tick);
        Some(execution)
    }

    pub fn default_o3_runtime_checkpoint_payload() -> O3RuntimeCheckpointPayload {
        O3RuntimeCheckpointPayload::from_snapshot(default_o3_runtime_snapshot())
            .expect("default O3 runtime checkpoint payload is valid")
    }
}
