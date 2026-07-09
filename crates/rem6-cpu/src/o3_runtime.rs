use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_isa_riscv::{MemoryAccessKind, Register};
use rem6_memory::{Address, MemoryRequestId};

use crate::branch_predictor::{BranchTargetKind, BranchUpdate};
use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};
use crate::o3_fu_latency::o3_fu_latency_class;
use crate::o3_pipeline::{
    O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PipelineError,
};
use crate::o3_runtime_trace::{O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeTraceRecord};
use crate::riscv_branch_kind::{is_riscv_link_register, riscv_branch_target_kind};
use crate::riscv_execution_event::RiscvCpuExecutionEvent;
use crate::RiscvDataAccessEventKind;

#[path = "o3_runtime_checkpoint.rs"]
mod o3_runtime_checkpoint;
#[path = "o3_runtime_checkpoint_branch_mismatch.rs"]
mod o3_runtime_checkpoint_branch_mismatch;
#[path = "o3_runtime_helpers.rs"]
mod o3_runtime_helpers;
#[path = "o3_runtime_stats.rs"]
mod o3_runtime_stats;
#[path = "o3_source_operands.rs"]
mod o3_source_operands;

pub use o3_runtime_checkpoint::O3RuntimeCheckpointPayload;
use o3_runtime_helpers::{
    default_o3_runtime_snapshot, encode_register_class, encode_u32, validate_runtime_snapshot,
    validate_unique, O3RuntimeUniqueKey,
};
pub use o3_runtime_stats::O3RuntimeStats;
use o3_source_operands::o3_scalar_integer_source_registers;

const U64_BYTES: usize = 8;
const O3_RUNTIME_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3ReorderBufferEntry {
    sequence: u64,
    pc: Address,
    destination: Option<O3PhysicalRegisterId>,
    ready: bool,
}

impl O3ReorderBufferEntry {
    pub const fn new(
        sequence: u64,
        pc: Address,
        destination: Option<O3PhysicalRegisterId>,
    ) -> Self {
        Self {
            sequence,
            pc,
            destination,
            ready: false,
        }
    }

    pub const fn with_ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    pub const fn pc(self) -> Address {
        self.pc
    }
    pub const fn destination(self) -> Option<O3PhysicalRegisterId> {
        self.destination
    }
    pub const fn is_ready(self) -> bool {
        self.ready
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LoadStoreQueueKind {
    Load,
    Store,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LoadStoreQueueEntry {
    sequence: u64,
    address: Option<Address>,
    bytes: u32,
    kind: O3LoadStoreQueueKind,
    completed: bool,
}

impl O3LoadStoreQueueEntry {
    pub const fn load(sequence: u64, address: Option<Address>, bytes: u32) -> Self {
        Self {
            sequence,
            address,
            bytes,
            kind: O3LoadStoreQueueKind::Load,
            completed: false,
        }
    }

    pub const fn store(sequence: u64, address: Option<Address>, bytes: u32) -> Self {
        Self {
            sequence,
            address,
            bytes,
            kind: O3LoadStoreQueueKind::Store,
            completed: false,
        }
    }

    pub const fn with_completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    pub const fn address(self) -> Option<Address> {
        self.address
    }
    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub const fn kind(self) -> O3LoadStoreQueueKind {
        self.kind
    }

    pub const fn is_completed(self) -> bool {
        self.completed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RenameMapEntry {
    register_class: O3RegisterClass,
    architectural: u32,
    physical: O3PhysicalRegisterId,
}

impl O3RenameMapEntry {
    pub const fn new(
        register_class: O3RegisterClass,
        architectural: u32,
        physical: O3PhysicalRegisterId,
    ) -> Self {
        Self {
            register_class,
            architectural,
            physical,
        }
    }

    pub const fn register_class(self) -> O3RegisterClass {
        self.register_class
    }

    pub const fn architectural(self) -> u32 {
        self.architectural
    }

    pub const fn physical(self) -> O3PhysicalRegisterId {
        self.physical
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeSnapshot {
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    load_store_queue: Vec<O3LoadStoreQueueEntry>,
    rename_map: Vec<O3RenameMapEntry>,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeState {
    snapshot: O3RuntimeSnapshot,
    stats: O3RuntimeStats,
    trace_records: Vec<O3RuntimeTraceRecord>,
    dirty_trace_record_indices: BTreeSet<usize>,
    data_access_sequences: BTreeMap<MemoryRequestId, u64>,
    trace_data_access_sequences: BTreeMap<MemoryRequestId, u64>,
    store_forwarding_window: O3StoreForwardingWindow,
    dependency_producers_with_consumers: BTreeSet<O3PhysicalRegisterId>,
    next_sequence: u64,
    next_physical_register: u32,
}

impl O3RuntimeState {
    pub fn restore(&mut self, snapshot: O3RuntimeSnapshot) -> Result<(), O3RuntimeError> {
        validate_runtime_snapshot(&snapshot)?;
        self.next_sequence = next_runtime_sequence(&snapshot);
        self.next_physical_register = next_runtime_physical_register(&snapshot);
        self.snapshot = snapshot;
        self.trace_records.clear();
        self.dirty_trace_record_indices.clear();
        self.data_access_sequences.clear();
        self.trace_data_access_sequences.clear();
        self.store_forwarding_window = O3StoreForwardingWindow::default();
        self.dependency_producers_with_consumers.clear();
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
        self.dependency_producers_with_consumers = dependency_producers_with_consumers;
        Ok(())
    }

    pub fn snapshot(&self) -> O3RuntimeSnapshot {
        self.snapshot.clone()
    }

    pub const fn stats(&self) -> O3RuntimeStats {
        self.stats
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
        self.trace_data_access_sequences.len()
    }

    pub(crate) fn record_data_access_outcome(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        response_tick: u64,
        latency_ticks: u64,
    ) {
        let runtime_sequence = self
            .data_access_sequences
            .remove(&execution.fetch().request_id());
        let trace_sequence = self
            .trace_data_access_sequences
            .remove(&execution.fetch().request_id());
        if let Some(sequence) = trace_sequence {
            self.mark_trace_data_access_outcome(sequence, response_tick, latency_ticks);
        }
        if runtime_sequence.is_some() {
            if let Some(access) = execution.execution().memory_access() {
                self.stats
                    .record_lsq_operation_latency(o3_lsq_operation(access), latency_ticks);
            }
            if o3_store_conditional_failed(execution) {
                self.stats.record_store_conditional_failure();
                if let Some(sequence) = trace_sequence {
                    self.mark_trace_store_conditional_failed(sequence);
                }
            }
        }
    }

    pub(crate) fn discard_data_access_outcome(&mut self, fetch_request: MemoryRequestId) {
        self.data_access_sequences.remove(&fetch_request);
        self.trace_data_access_sequences.remove(&fetch_request);
    }

    pub fn reset_stats(&mut self) {
        self.stats = O3RuntimeStats::default();
        self.trace_records.clear();
        self.dirty_trace_record_indices.clear();
        self.data_access_sequences.clear();
        self.trace_data_access_sequences.clear();
        self.store_forwarding_window = O3StoreForwardingWindow::default();
        self.dependency_producers_with_consumers.clear();
    }

    pub fn record_retired_instruction(&mut self, execution: &RiscvCpuExecutionEvent) {
        self.record_retired_instruction_with_trace(execution, false);
    }

    pub fn record_retired_instruction_with_trace(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_enabled: bool,
    ) {
        let mut trace_record = self.record_runtime_state(execution);
        self.stats
            .record_retired_instruction(execution, trace_record);
        let observation = self.record_store_forwarding_window(
            execution,
            trace_enabled.then_some(trace_record.sequence()),
        );
        trace_record.set_store_load_forwarding(
            observation.candidate,
            observation.matched,
            observation.suppressed,
            observation.address_mismatch,
            observation.byte_mismatch,
        );
        self.record_data_access_sequence(execution, trace_record.sequence());
        if trace_enabled {
            self.record_trace_data_access_sequence(execution, trace_record.sequence());
            self.trace_records.push(trace_record);
        }
    }

    fn record_runtime_state(&mut self, execution: &RiscvCpuExecutionEvent) -> O3RuntimeTraceRecord {
        let sequence = self.allocate_sequence();
        let record = execution.execution();
        let (iew_dependency_producers, iew_dependency_consumers) =
            self.record_scalar_integer_dependencies(&record.instruction());
        let destination = self.record_rename_map_entries(record);
        let (lsq_loads, lsq_stores) = record
            .memory_access()
            .map(o3_lsq_access_counts)
            .unwrap_or((0, 0));
        let lsq_operation = record
            .memory_access()
            .map(o3_lsq_operation)
            .unwrap_or(O3RuntimeLsqOperation::None);
        let lsq_ordering = record
            .memory_access()
            .map(o3_lsq_ordering)
            .unwrap_or(O3RuntimeLsqOrdering::None);
        let (lsq_load_bytes, lsq_store_bytes) = record
            .memory_access()
            .map(o3_lsq_access_bytes)
            .unwrap_or((0, 0));
        let (lsq_load_address, lsq_store_address) = record
            .memory_access()
            .map(o3_lsq_access_addresses)
            .unwrap_or((None, None));
        let branch_update = execution.branch_update();
        let branch_kind = branch_update
            .map(|_| riscv_branch_target_kind(record.instruction()))
            .unwrap_or(BranchTargetKind::NoBranch);
        let branch_predicted_taken = branch_update.is_some_and(|update| update.predicted_taken());
        let branch_resolved_taken = branch_update.is_some_and(|update| update.actual_taken());
        let branch_link_register_write =
            branch_update.is_some() && o3_branch_link_register_write(record);
        let branch_predicted_target = branch_update.and_then(|update| update.predicted_target());
        let branch_resolved_target = branch_update.and_then(|update| update.actual_target());
        let branch_fallthrough_target = Address::new(
            record
                .pc()
                .saturating_add(u64::from(record.instruction_bytes())),
        );
        let branch_squashed_target = branch_update.and_then(|update| {
            o3_branch_squashed_target(branch_kind, update, branch_fallthrough_target)
        });
        for entry in &mut self.snapshot.reorder_buffer {
            entry.ready = true;
        }
        let fu_latency_cycles =
            crate::riscv_execute::in_order_execute_wait_cycles(execution.instruction());
        self.snapshot.reorder_buffer.push(
            O3ReorderBufferEntry::new(sequence, Address::new(record.pc()), destination)
                .with_ready(fu_latency_cycles == 0),
        );
        let rob_occupancy = self.snapshot.reorder_buffer.len();
        self.stats.observe_rob_occupancy(rob_occupancy);

        let lsq_start = self.snapshot.load_store_queue.len();
        if let Some(access) = record.memory_access() {
            for entry in o3_lsq_entries(sequence, access) {
                self.snapshot.load_store_queue.push(entry);
            }
            self.stats
                .observe_lsq_occupancy(self.snapshot.load_store_queue.len());
        }
        let lsq_occupancy = self.snapshot.load_store_queue.len();
        let rename_map_entries = self.snapshot.rename_map.len();
        let trace_record = O3RuntimeTraceRecord::new(
            sequence,
            execution.fetch().tick(),
            Address::new(record.pc()),
            rob_occupancy,
            o3_rename_write_count(record),
            lsq_loads,
            lsq_stores,
            lsq_occupancy,
            lsq_operation,
            lsq_ordering,
            lsq_load_address,
            lsq_store_address,
            lsq_load_bytes,
            lsq_store_bytes,
            o3_store_conditional_failed(execution),
            0,
            0,
            rename_map_entries,
            iew_dependency_producers,
            iew_dependency_consumers,
            branch_kind,
            branch_predicted_taken,
            branch_resolved_taken,
            branch_link_register_write,
            branch_predicted_target,
            branch_resolved_target,
            branch_squashed_target,
            o3_fu_latency_class(execution.instruction()),
            fu_latency_cycles,
            record.system_event().is_some(),
        );

        let rob_commits = self
            .snapshot
            .reorder_buffer
            .partition_point(|entry| entry.is_ready());
        self.snapshot.reorder_buffer.drain(0..rob_commits);
        self.snapshot.load_store_queue.truncate(lsq_start);
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
        trace_record
    }

    fn record_rename_map_entries(
        &mut self,
        record: &rem6_isa_riscv::RiscvExecutionRecord,
    ) -> Option<O3PhysicalRegisterId> {
        if record.system_event().is_some() {
            return None;
        }

        let mut first_destination = None;
        for write in record.register_writes() {
            if !write.register().is_zero() {
                let physical = self.install_rename_map_entry(
                    O3RegisterClass::Integer,
                    u32::from(write.register().index()),
                );
                first_destination.get_or_insert(physical);
            }
        }
        for write in record.float_register_writes() {
            let physical = self.install_rename_map_entry(
                O3RegisterClass::FloatingPoint,
                u32::from(write.register().index()),
            );
            first_destination.get_or_insert(physical);
        }
        if let Some(access) = record.memory_access() {
            for (register_class, architectural) in o3_memory_destination_registers(access) {
                let physical = self.install_rename_map_entry(register_class, architectural);
                first_destination.get_or_insert(physical);
            }
        }

        first_destination
    }

    fn record_scalar_integer_dependencies(
        &mut self,
        instruction: &rem6_isa_riscv::RiscvInstruction,
    ) -> (u64, u64) {
        let mut producers = 0_u64;
        let mut consumers = 0_u64;
        for register in o3_scalar_integer_source_registers(instruction) {
            if register.is_zero() {
                continue;
            }
            let Some(source) = self.snapshot.rename_map.iter().find(|entry| {
                entry.register_class() == O3RegisterClass::Integer
                    && entry.architectural() == u32::from(register.index())
            }) else {
                continue;
            };
            if self
                .dependency_producers_with_consumers
                .insert(source.physical())
            {
                producers = producers.saturating_add(1);
                self.stats.record_iew_dependency_producer();
            }
            consumers = consumers.saturating_add(1);
            self.stats.record_iew_dependency_consumer();
        }
        (producers, consumers)
    }

    fn install_rename_map_entry(
        &mut self,
        register_class: O3RegisterClass,
        architectural: u32,
    ) -> O3PhysicalRegisterId {
        let physical = self.allocate_physical_register();
        let entry = O3RenameMapEntry::new(register_class, architectural, physical);
        if let Some(existing) = self.snapshot.rename_map.iter_mut().find(|existing| {
            existing.register_class() == register_class && existing.architectural() == architectural
        }) {
            *existing = entry;
        } else {
            self.snapshot.rename_map.push(entry);
        }
        self.snapshot.rename_map.sort_by_key(|entry| {
            (
                encode_register_class(entry.register_class()),
                entry.architectural(),
            )
        });
        physical
    }

    fn allocate_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        sequence
    }

    fn allocate_physical_register(&mut self) -> O3PhysicalRegisterId {
        let physical = O3PhysicalRegisterId::new(self.next_physical_register);
        self.next_physical_register = self.next_physical_register.saturating_add(1);
        physical
    }

    fn record_store_forwarding_window(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_sequence: Option<u64>,
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
        if load.address != pending.address || load.bytes != pending.bytes {
            return;
        }

        if o3_load_data_value(data, pending.bytes)
            .is_some_and(|value| o3_low_bytes_equal(value, pending.value, pending.bytes))
        {
            self.stats
                .record_store_to_load_forwarding_match(pending.operation);
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

    fn record_data_access_sequence(&mut self, execution: &RiscvCpuExecutionEvent, sequence: u64) {
        if execution.execution().memory_access().is_some() {
            self.data_access_sequences
                .insert(execution.fetch().request_id(), sequence);
        }
    }

    fn record_trace_data_access_sequence(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        trace_sequence: u64,
    ) {
        if execution.execution().memory_access().is_some() {
            self.trace_data_access_sequences
                .insert(execution.fetch().request_id(), trace_sequence);
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

    pub fn pending_state_checkpoint_payload(&self) -> O3PendingStateCheckpointPayload {
        O3PendingStateCheckpointPayload::from_snapshot(self.snapshot.pending_state.clone())
            .expect("O3 runtime pending-state snapshot is valid")
    }

    pub fn restore_pending_state_checkpoint_payload(
        &mut self,
        payload: O3PendingStateCheckpointPayload,
    ) -> Result<(), O3PipelineError> {
        let pending_state =
            O3PendingStateCheckpointPayload::from_snapshot(payload.snapshot().clone())?
                .into_snapshot();
        self.snapshot = O3RuntimeSnapshot::new(
            self.snapshot.reorder_buffer.clone(),
            self.snapshot.load_store_queue.clone(),
            self.snapshot.rename_map.clone(),
            pending_state,
        )
        .expect("existing O3 runtime snapshot is valid");
        Ok(())
    }
}

impl Default for O3RuntimeState {
    fn default() -> Self {
        Self {
            snapshot: default_o3_runtime_snapshot(),
            stats: O3RuntimeStats::default(),
            trace_records: Vec::new(),
            dirty_trace_record_indices: BTreeSet::new(),
            data_access_sequences: BTreeMap::new(),
            trace_data_access_sequences: BTreeMap::new(),
            store_forwarding_window: O3StoreForwardingWindow::default(),
            dependency_producers_with_consumers: BTreeSet::new(),
            next_sequence: 0,
            next_physical_register: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3StoreForwardingWindow {
    store: Option<O3StoreForwardingEntry>,
    pending_load_match: Option<O3PendingLoadForwardingMatch>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3StoreForwardingObservation {
    candidate: bool,
    matched: bool,
    suppressed: bool,
    address_mismatch: bool,
    byte_mismatch: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3StoreForwardingEntry {
    address: Address,
    bytes: u32,
    value: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3PendingLoadForwardingMatch {
    fetch_request: MemoryRequestId,
    address: Address,
    bytes: u32,
    value: u64,
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
        .filter(|physical| *physical != u32::MAX)
        .max()
        .map_or(1, |physical| physical.saturating_add(1))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3LoadForwardingAccess {
    register: Register,
    address: Address,
    bytes: u32,
}

fn o3_branch_link_register_write(record: &rem6_isa_riscv::RiscvExecutionRecord) -> bool {
    if !matches!(
        riscv_branch_target_kind(record.instruction()),
        BranchTargetKind::CallDirect | BranchTargetKind::CallIndirect
    ) {
        return false;
    }
    record
        .register_writes()
        .iter()
        .any(|write| is_riscv_link_register(write.register()))
}

fn o3_branch_squashed_target(
    branch_kind: BranchTargetKind,
    update: &BranchUpdate,
    fallthrough_target: Address,
) -> Option<Address> {
    if matches!(branch_kind, BranchTargetKind::NoBranch) {
        return None;
    }
    let predicted_taken = update.predicted_taken();
    let resolved_taken = update.actual_taken();
    let predicted_target = update.predicted_target();
    let resolved_target = update.actual_target();
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

fn o3_store_forwarding_entry(access: &MemoryAccessKind) -> Option<O3StoreForwardingEntry> {
    match access {
        MemoryAccessKind::Store {
            address,
            width,
            value,
        } => Some(O3StoreForwardingEntry {
            address: Address::new(*address),
            bytes: o3_lsq_bytes(width.bytes()),
            value: *value,
        }),
        _ => None,
    }
}

fn o3_load_forwarding_access(access: &MemoryAccessKind) -> Option<O3LoadForwardingAccess> {
    match access {
        MemoryAccessKind::Load {
            rd, address, width, ..
        } => Some(O3LoadForwardingAccess {
            register: *rd,
            address: Address::new(*address),
            bytes: o3_lsq_bytes(width.bytes()),
        }),
        _ => None,
    }
}

fn o3_load_register_value(
    register_writes: &[rem6_isa_riscv::RegisterWrite],
    register: Register,
) -> Option<u64> {
    register_writes
        .iter()
        .find(|write| write.register() == register)
        .map(rem6_isa_riscv::RegisterWrite::value)
}

fn o3_load_data_value(data: &[u8], bytes: u32) -> Option<u64> {
    let width = usize::try_from(bytes).ok()?;
    if data.len() < width || width > U64_BYTES {
        return None;
    }
    let mut value = 0_u64;
    for (index, byte) in data.iter().take(width).copied().enumerate() {
        value |= u64::from(byte) << (index * 8);
    }
    Some(value)
}

fn o3_low_bytes_equal(lhs: u64, rhs: u64, bytes: u32) -> bool {
    let bits = bytes.saturating_mul(8);
    let mask = if bits >= u64::BITS {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    (lhs & mask) == (rhs & mask)
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
        .with_completed(true)
}

fn o3_lsq_store(sequence: u64, address: u64, bytes: usize) -> O3LoadStoreQueueEntry {
    O3LoadStoreQueueEntry::store(sequence, Some(Address::new(address)), o3_lsq_bytes(bytes))
        .with_completed(true)
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
mod tests {
    use rem6_isa_riscv::{MemoryWidth, RiscvExecutionRecord, RiscvInstruction};
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{CpuFetchEvent, CpuFetchRecord};

    #[test]
    fn failed_store_conditional_trace_mark_uses_dynamic_sequence_identity() {
        let mut runtime = O3RuntimeState::default();
        let mut first = store_conditional_event(0x8000, 10);
        let second = store_conditional_event(0x8000, 11);

        runtime.record_retired_instruction_with_trace(&first, true);
        runtime.record_retired_instruction_with_trace(&second, true);
        first.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
        runtime.record_data_access_outcome(&first, 41, 7);

        let trace = runtime.trace_records();
        assert_eq!(trace.len(), 2);
        assert!(trace[0].lsq_store_conditional_failed());
        assert_eq!(trace[0].lsq_data_response_tick(), 41);
        assert_eq!(trace[0].lsq_data_latency_ticks(), 7);
        assert!(!trace[1].lsq_store_conditional_failed());
        assert_eq!(trace[1].lsq_data_response_tick(), 0);
        assert_eq!(trace[1].lsq_data_latency_ticks(), 0);
    }

    #[test]
    fn failed_store_conditional_pending_trace_identity_can_be_discarded() {
        let mut runtime = O3RuntimeState::default();
        let mut event = store_conditional_event(0x8000, 10);

        runtime.record_retired_instruction_with_trace(&event, true);
        assert_eq!(runtime.trace_data_access_sequences.len(), 1);

        runtime.discard_data_access_outcome(event.fetch().request_id());
        assert!(runtime.trace_data_access_sequences.is_empty());

        event.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
        runtime.record_data_access_outcome(&event, 41, 7);
        assert!(!runtime.trace_records()[0].lsq_store_conditional_failed());
        assert_eq!(runtime.trace_records()[0].lsq_data_response_tick(), 0);
        assert_eq!(runtime.trace_records()[0].lsq_data_latency_ticks(), 0);
    }

    #[test]
    fn failed_store_conditional_stats_count_failed_operation() {
        let mut runtime = O3RuntimeState::default();
        let mut event = store_conditional_event(0x8000, 10);

        runtime.record_retired_instruction(&event);
        event.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
        runtime.record_data_access_outcome(&event, 41, 7);

        let stats = runtime.stats();

        assert_eq!(
            stats.lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
            1
        );
        assert_eq!(
            stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(stats.lsq_data_latency_samples(), 1);
        assert_eq!(stats.lsq_data_latency_ticks(), 7);
        assert_eq!(stats.lsq_data_latency_min_ticks(), 7);
        assert_eq!(stats.lsq_data_latency_max_ticks(), 7);
        assert_eq!(stats.lsq_data_latency_avg_ticks(), 7);
        assert_eq!(
            stats.lsq_operation_latency_min_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(
            stats.lsq_operation_latency_max_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(
            stats.lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(stats.lsq_store_conditional_failures(), 1);
    }

    #[test]
    fn lsq_operation_latency_min_keeps_zero_tick_sample() {
        let mut runtime = O3RuntimeState::default();
        let first = store_conditional_event(0x8000, 10);
        let second = store_conditional_event(0x8004, 11);

        runtime.record_retired_instruction(&first);
        runtime.record_retired_instruction(&second);
        runtime.record_data_access_outcome(&first, 41, 0);
        runtime.record_data_access_outcome(&second, 46, 5);

        let stats = runtime.stats();

        assert_eq!(
            stats.lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
            2
        );
        assert_eq!(
            stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::StoreConditional),
            5
        );
        assert_eq!(stats.lsq_data_latency_samples(), 2);
        assert_eq!(stats.lsq_data_latency_ticks(), 5);
        assert_eq!(stats.lsq_data_latency_min_ticks(), 0);
        assert_eq!(stats.lsq_data_latency_max_ticks(), 5);
        assert_eq!(stats.lsq_data_latency_avg_ticks(), 2);
        assert_eq!(
            stats.lsq_operation_latency_min_ticks(O3RuntimeLsqOperation::StoreConditional),
            0
        );
        assert_eq!(
            stats.lsq_operation_latency_max_ticks(O3RuntimeLsqOperation::StoreConditional),
            5
        );
        assert_eq!(
            stats.lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
            2
        );
    }

    #[test]
    fn failed_store_conditional_checkpoint_round_trips_failure_count() {
        let mut runtime = O3RuntimeState::default();
        let mut event = store_conditional_event(0x8000, 10);

        runtime.record_retired_instruction(&event);
        event.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
        runtime.record_data_access_outcome(&event, 41, 7);

        let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
            runtime.snapshot(),
            runtime.stats(),
        )
        .unwrap();
        let encoded = payload.encode();
        let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

        assert_eq!(decoded.encode(), encoded);
        assert_eq!(decoded.stats().lsq_store_conditional_failures(), 1);
        assert_eq!(
            decoded
                .stats()
                .lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .lsq_operation_latency_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(decoded.stats().lsq_data_latency_samples(), 1);
        assert_eq!(decoded.stats().lsq_data_latency_ticks(), 7);
        assert_eq!(decoded.stats().lsq_data_latency_min_ticks(), 7);
        assert_eq!(decoded.stats().lsq_data_latency_max_ticks(), 7);
        assert_eq!(decoded.stats().lsq_data_latency_avg_ticks(), 7);
        assert_eq!(
            decoded
                .stats()
                .lsq_operation_latency_min_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(
            decoded
                .stats()
                .lsq_operation_latency_max_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
        assert_eq!(
            decoded
                .stats()
                .lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
            7
        );
    }

    #[test]
    fn branch_repair_stats_checkpoint_round_trips_current_payload() {
        let mut targetless_kinds = [0; BranchTargetKind::COUNT];
        let mut wrong_target_kinds = [0; BranchTargetKind::COUNT];
        let mut direction_only_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_taken_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_predicted_taken_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_predicted_target_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_predicted_target_match_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_predicted_target_mismatch_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_resolved_target_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_link_write_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_squash_kinds = [0; BranchTargetKind::COUNT];
        let mut branch_event_squashed_target_without_link_write_kinds =
            [0; BranchTargetKind::COUNT];
        targetless_kinds[BranchTargetKind::DirectConditional.index()] = 2;
        wrong_target_kinds[BranchTargetKind::CallIndirect.index()] = 3;
        direction_only_kinds[BranchTargetKind::DirectUnconditional.index()] = 4;
        branch_event_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_taken_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_predicted_taken_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_predicted_target_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_predicted_target_match_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_predicted_target_mismatch_kinds[BranchTargetKind::CallIndirect.index()] = 2;
        branch_event_resolved_target_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_link_write_kinds[BranchTargetKind::Return.index()] = 0;
        branch_event_squash_kinds[BranchTargetKind::Return.index()] = 1;
        branch_event_squashed_target_without_link_write_kinds[BranchTargetKind::Return.index()] = 1;
        let stats = O3RuntimeStats {
            branch_repair_targetless_mismatches: 2,
            branch_repair_wrong_targets: 3,
            branch_repair_direction_only_mismatches: 4,
            branch_repair_targetless_mismatch_kinds: targetless_kinds,
            branch_repair_wrong_target_kinds: wrong_target_kinds,
            branch_repair_direction_only_kinds: direction_only_kinds,
            branch_event_kinds,
            branch_event_taken_kinds,
            branch_event_predicted_taken_kinds,
            branch_event_predicted_target_kinds,
            branch_event_predicted_target_match_kinds,
            branch_event_predicted_target_mismatch_kinds,
            branch_event_resolved_target_kinds,
            branch_event_link_write_kinds,
            branch_event_squash_kinds,
            branch_event_squashed_target_without_link_write_kinds,
            iew_predicted_taken_incorrect: 5,
            iew_predicted_not_taken_incorrect: 6,
            iq_branch_insts_issued: 8,
            ..O3RuntimeStats::default()
        };
        let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
            default_o3_runtime_snapshot(),
            stats,
        )
        .unwrap();
        let encoded = payload.encode();
        let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

        assert_eq!(decoded.encode(), encoded);
        assert_eq!(decoded.stats().branch_repair_targetless_mismatches(), 2);
        assert_eq!(decoded.stats().branch_repair_wrong_targets(), 3);
        assert_eq!(decoded.stats().branch_repair_direction_only_mismatches(), 4);
        assert_eq!(
            decoded
                .stats()
                .branch_repair_targetless_mismatch_kind(BranchTargetKind::DirectConditional),
            2
        );
        assert_eq!(
            decoded
                .stats()
                .branch_repair_wrong_target_kind(BranchTargetKind::CallIndirect),
            3
        );
        assert_eq!(
            decoded
                .stats()
                .branch_repair_direction_only_kind(BranchTargetKind::DirectUnconditional),
            4
        );
        assert_eq!(
            decoded.stats().branch_event_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_taken_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_predicted_taken_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_predicted_not_taken_kind(BranchTargetKind::Return),
            0
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_predicted_target_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_predicted_target_match_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_predicted_target_mismatch_kind(BranchTargetKind::CallIndirect),
            2
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_resolved_target_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_link_write_kind(BranchTargetKind::Return),
            0
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_squash_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            decoded
                .stats()
                .branch_event_squashed_target_without_link_write_kind(BranchTargetKind::Return),
            1
        );
        assert_eq!(decoded.stats().iew_predicted_taken_incorrect(), 5);
        assert_eq!(decoded.stats().iew_predicted_not_taken_incorrect(), 6);
        assert_eq!(decoded.stats().iq_branch_insts_issued(), 8);
    }

    fn store_conditional_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::StoreConditional {
            rd: Register::new(7).unwrap(),
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            width: MemoryWidth::Doubleword,
            acquire: false,
            release: false,
        };
        let access = MemoryAccessKind::StoreConditional {
            rd: Register::new(7).unwrap(),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            value: 0x2a,
            acquire: false,
            release: false,
        };
        RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        )
    }

    fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                10 + sequence,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRequestId::new(AgentId::new(7), sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0073u32.to_le_bytes().to_vec(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum O3RuntimeError {
    DuplicateReorderBufferSequence {
        sequence: u64,
    },
    DuplicateLoadStoreQueueSequence {
        sequence: u64,
    },
    DuplicateRenameMapEntry {
        register_class: O3RegisterClass,
        architectural: u32,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    InvalidRegisterClassCode {
        code: u8,
    },
    InvalidLoadStoreKindCode {
        code: u8,
    },
    InvalidCheckpointBool {
        field: &'static str,
        value: u8,
    },
    InvalidPendingState {
        error: O3PipelineError,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for O3RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateReorderBufferSequence { sequence } => {
                write!(formatter, "O3 runtime ROB repeats sequence {sequence}")
            }
            Self::DuplicateLoadStoreQueueSequence { sequence } => {
                write!(formatter, "O3 runtime LSQ repeats sequence {sequence}")
            }
            Self::DuplicateRenameMapEntry {
                register_class,
                architectural,
            } => write!(
                formatter,
                "O3 runtime rename map repeats {register_class:?} architectural register {architectural}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "O3 runtime checkpoint payload has {actual} bytes but expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "O3 runtime checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "O3 runtime checkpoint payload version {version} is not supported"
            ),
            Self::InvalidRegisterClassCode { code } => write!(
                formatter,
                "O3 runtime checkpoint payload has invalid register-class code {code}"
            ),
            Self::InvalidLoadStoreKindCode { code } => write!(
                formatter,
                "O3 runtime checkpoint payload has invalid LSQ kind code {code}"
            ),
            Self::InvalidCheckpointBool { field, value } => write!(
                formatter,
                "O3 runtime checkpoint field {field} boolean has invalid value {value}"
            ),
            Self::InvalidPendingState { error } => {
                write!(formatter, "O3 runtime checkpoint has invalid pending state: {error}")
            }
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "O3 runtime checkpoint field {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for O3RuntimeError {}

impl crate::RiscvCore {
    fn with_o3_runtime<T>(&self, f: impl FnOnce(&mut O3RuntimeState) -> T) -> T {
        let mut state = self.state.lock().expect("riscv core lock");
        f(&mut state.o3_runtime)
    }

    pub fn o3_runtime_stats(&self) -> O3RuntimeStats {
        self.with_o3_runtime(|runtime| runtime.stats())
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

    pub fn record_o3_completed_load_data(
        &self,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
        data: &[u8],
    ) {
        self.with_o3_runtime(|runtime| {
            runtime.record_completed_load_data(fetch_request, access, data);
        });
    }

    pub fn default_o3_runtime_checkpoint_payload() -> O3RuntimeCheckpointPayload {
        O3RuntimeCheckpointPayload::from_snapshot(default_o3_runtime_snapshot())
            .expect("default O3 runtime checkpoint payload is valid")
    }

    pub fn o3_runtime_checkpoint_payload(&self) -> O3RuntimeCheckpointPayload {
        self.with_o3_runtime(|runtime| {
            O3RuntimeCheckpointPayload::from_snapshot_with_stats_and_dependency_producers(
                runtime.snapshot(),
                runtime.stats(),
                runtime.dependency_producers_with_consumers.clone(),
            )
            .expect("captured RISC-V O3 runtime checkpoint is internally consistent")
        })
    }

    pub fn restore_o3_runtime_checkpoint_payload(
        &self,
        payload: O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        self.validate_o3_runtime_checkpoint_payload(&payload)?;
        self.with_o3_runtime(|runtime| runtime.restore_checkpoint_payload(payload))
    }

    pub fn validate_o3_runtime_checkpoint_payload(
        &self,
        payload: &O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        O3RuntimeCheckpointPayload::from_snapshot(payload.snapshot().clone()).map(|_| ())
    }

    pub fn default_o3_pending_state_checkpoint_payload() -> O3PendingStateCheckpointPayload {
        O3RuntimeState::default().pending_state_checkpoint_payload()
    }

    pub fn o3_pending_state_checkpoint_payload(&self) -> O3PendingStateCheckpointPayload {
        self.with_o3_runtime(|runtime| runtime.pending_state_checkpoint_payload())
    }

    pub fn restore_o3_pending_state_checkpoint_payload(
        &self,
        payload: O3PendingStateCheckpointPayload,
    ) -> Result<(), O3PipelineError> {
        self.validate_o3_pending_state_checkpoint_payload(&payload)?;
        self.with_o3_runtime(|runtime| runtime.restore_pending_state_checkpoint_payload(payload))
    }

    pub fn validate_o3_pending_state_checkpoint_payload(
        &self,
        payload: &O3PendingStateCheckpointPayload,
    ) -> Result<(), O3PipelineError> {
        O3PendingStateCheckpointPayload::from_snapshot(payload.snapshot().clone()).map(|_| ())
    }
}
