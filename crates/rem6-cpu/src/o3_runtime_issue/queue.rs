use rem6_isa_riscv::{
    Register, RegisterWrite, RiscvDecodedInstruction, RiscvExecutionRecord, RiscvInstruction,
};
use rem6_memory::{Address, MemoryRequestId};

use super::super::o3_runtime_control_window::execution_writes_rename_destination;
use super::super::o3_runtime_live_window::staged_rename_entry;
use super::*;
use crate::branch_predictor::BranchTargetKind;
use crate::o3_dependency::O3RegisterClass;
use crate::o3_pipeline::O3IssueOpClass;
use crate::riscv_fu_latency::riscv_o3_fu_latency_class as o3_fu_latency_class;
use crate::O3RuntimeFuLatencyClass;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssuePacket {
    decoded: RiscvDecodedInstruction,
    consumed_requests: Vec<MemoryRequestId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueQueueEntry {
    packet: O3LiveIssuePacket,
    scheduling: O3LiveIssueSchedulingCandidate,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueQueue {
    entries: Vec<O3LiveIssueQueueEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) enum O3LiveIssueQueueCapture {
    Ready(O3LiveIssueQueue),
    ReplayPending(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSchedulingCandidate {
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    kind: O3LiveSpeculativeIssueKind,
    op_class: O3IssueOpClass,
    control_dependency: Option<u64>,
    data_producers: Vec<O3LiveIssueSourceProducer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSourceProducer {
    sequence: u64,
    source: Register,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveSpeculativeIssueCandidate {
    scheduling: O3LiveIssueSchedulingCandidate,
    producer_sequences: Vec<u64>,
    forwarded_register_writes: Vec<RegisterWrite>,
    forwarded_ready_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveSpeculativeIssueKind {
    PendingDataAddress(O3RenameMapEntry),
    Scalar(O3RenameMapEntry),
    Control {
        kind: BranchTargetKind,
        destination: Option<O3RenameMapEntry>,
    },
}

impl O3LiveIssuePacket {
    pub(in crate::o3_runtime) fn new(
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Self {
        Self {
            decoded,
            consumed_requests: consumed_requests.to_vec(),
        }
    }

    pub(in crate::o3_runtime) const fn decoded(&self) -> RiscvDecodedInstruction {
        self.decoded
    }

    pub(in crate::o3_runtime) const fn instruction(&self) -> RiscvInstruction {
        self.decoded.instruction()
    }

    pub(in crate::o3_runtime) fn consumed_requests(&self) -> &[MemoryRequestId] {
        &self.consumed_requests
    }

    pub(in crate::o3_runtime) fn matches_execution(
        &self,
        execution: &RiscvExecutionRecord,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.instruction() == execution.instruction()
            && self.decoded.bytes() == execution.instruction_bytes()
            && self.consumed_requests() == consumed_requests
    }
}

impl O3LiveIssueQueue {
    pub(in crate::o3_runtime) fn capture(
        runtime: &O3RuntimeState,
        head: O3LiveIssueHeadReservation,
    ) -> Result<O3LiveIssueQueueCapture, O3RuntimeError> {
        let mut entries = Vec::new();
        for (index, rob) in runtime.snapshot.reorder_buffer.iter().copied().enumerate() {
            let sequence = rob.sequence();
            if !rob.is_live_staged()
                || sequence == head.sequence()
                || runtime
                    .live_speculative_executions
                    .iter()
                    .any(|issued| issued.sequence == sequence)
            {
                continue;
            }
            let pending = runtime.pending_data_addresses.find_sequence(sequence);
            if pending.is_some_and(|pending| pending.materialized.is_some()) {
                continue;
            }
            let pending = pending.is_some();
            let Some(packet) = runtime.live_staged_issue_packet(sequence).cloned() else {
                if pending {
                    return Ok(O3LiveIssueQueueCapture::ReplayPending(sequence));
                }
                continue;
            };
            let Some(scheduling) =
                runtime.live_issue_scheduling_candidate_from_metadata(index, rob, &packet)
            else {
                if pending {
                    return Ok(O3LiveIssueQueueCapture::ReplayPending(sequence));
                }
                if !live_issue_instruction_is_supported(packet.instruction()) {
                    continue;
                }
                return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
            };
            entries.push(O3LiveIssueQueueEntry { packet, scheduling });
        }
        Self::try_from_entries(entries).map(O3LiveIssueQueueCapture::Ready)
    }

    fn try_from_entries(entries: Vec<O3LiveIssueQueueEntry>) -> Result<Self, O3RuntimeError> {
        if let Some(entries) = entries
            .windows(2)
            .find(|entries| entries[0].sequence() >= entries[1].sequence())
        {
            let sequence = entries[1].sequence();
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        Ok(Self { entries })
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn from_entries_for_test(
        entries: Vec<O3LiveIssueQueueEntry>,
    ) -> Result<Self, O3RuntimeError> {
        Self::try_from_entries(entries)
    }

    pub(in crate::o3_runtime) fn entries(&self) -> &[O3LiveIssueQueueEntry] {
        &self.entries
    }

    pub(in crate::o3_runtime) fn entry(&self, sequence: u64) -> Option<&O3LiveIssueQueueEntry> {
        self.entries
            .binary_search_by_key(&sequence, O3LiveIssueQueueEntry::sequence)
            .ok()
            .map(|index| &self.entries[index])
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.iter().map(O3LiveIssueQueueEntry::sequence)
    }
}

impl O3LiveIssueQueueEntry {
    pub(in crate::o3_runtime) const fn sequence(&self) -> u64 {
        self.scheduling.sequence()
    }

    pub(in crate::o3_runtime) const fn packet(&self) -> &O3LiveIssuePacket {
        &self.packet
    }

    pub(in crate::o3_runtime) const fn scheduling(&self) -> &O3LiveIssueSchedulingCandidate {
        &self.scheduling
    }
}

impl O3LiveIssueSourceProducer {
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn source(self) -> Register {
        self.source
    }
}

impl O3LiveIssueSchedulingCandidate {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(in crate::o3_runtime) const fn pc(&self) -> Address {
        self.pc
    }

    pub(crate) const fn op_class(&self) -> O3IssueOpClass {
        self.op_class
    }

    pub(crate) const fn control_dependency(&self) -> Option<u64> {
        self.control_dependency
    }

    pub(crate) fn data_producers(&self) -> &[O3LiveIssueSourceProducer] {
        &self.data_producers
    }

    pub(crate) const fn is_pending_data_address(&self) -> bool {
        matches!(self.kind, O3LiveSpeculativeIssueKind::PendingDataAddress(_))
    }
}

impl O3LiveSpeculativeIssueCandidate {
    #[cfg(test)]
    pub(crate) const fn destination(&self) -> Option<O3RenameMapEntry> {
        match self.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress(destination)
            | O3LiveSpeculativeIssueKind::Scalar(destination) => Some(destination),
            O3LiveSpeculativeIssueKind::Control { destination, .. } => destination,
        }
    }

    pub(crate) fn forwarded_register_writes(&self) -> &[RegisterWrite] {
        &self.forwarded_register_writes
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.scheduling.sequence
    }

    pub(crate) fn producer_sequences(&self) -> &[u64] {
        &self.producer_sequences
    }

    pub(crate) const fn is_pending_data_address(&self) -> bool {
        matches!(
            self.scheduling.kind,
            O3LiveSpeculativeIssueKind::PendingDataAddress(_)
        )
    }

    pub(crate) const fn pending_data_address_destination(&self) -> Option<O3RenameMapEntry> {
        match self.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress(destination) => Some(destination),
            _ => None,
        }
    }

    pub(crate) fn data_producers(&self) -> &[O3LiveIssueSourceProducer] {
        &self.scheduling.data_producers
    }

    pub(crate) const fn instruction(&self) -> RiscvInstruction {
        self.scheduling.instruction
    }

    #[cfg(test)]
    pub(crate) const fn control_dependency(&self) -> Option<u64> {
        self.scheduling.control_dependency
    }

    pub(crate) fn issue_tick(&self, earliest_tick: u64) -> u64 {
        earliest_tick.max(self.forwarded_ready_tick)
    }

    pub(in crate::o3_runtime) fn valid_recorded_execution(
        &self,
        execution: &RiscvExecutionRecord,
    ) -> bool {
        if Address::new(execution.pc()) != self.scheduling.pc()
            || execution.instruction() != self.instruction()
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
        {
            return false;
        }
        match self.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress(_) => false,
            O3LiveSpeculativeIssueKind::Scalar(destination) => {
                execution.next_pc()
                    == execution
                        .pc()
                        .wrapping_add(u64::from(execution.instruction_bytes()))
                    && execution.register_writes().len() == 1
                    && execution_writes_rename_destination(execution, destination)
            }
            O3LiveSpeculativeIssueKind::Control { kind, destination } => {
                o3_live_control_operands(execution.instruction()).is_some_and(|control| {
                    control.kind() == kind
                        && control_destination_matches_rename_entry(
                            control.destination(),
                            destination,
                        )
                }) && match destination {
                    Some(destination) => {
                        execution.register_writes().len() == 1
                            && execution_writes_rename_destination(execution, destination)
                    }
                    None => execution.register_writes().is_empty(),
                }
            }
        }
    }

    pub(in crate::o3_runtime) const fn consumes_writeback_slot(&self) -> bool {
        matches!(
            self.scheduling.kind,
            O3LiveSpeculativeIssueKind::Scalar(_)
                | O3LiveSpeculativeIssueKind::Control {
                    destination: Some(_),
                    ..
                }
        )
    }
}

impl O3RuntimeState {
    #[cfg(test)]
    pub(crate) fn live_speculative_issue_candidate(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let index = self
            .snapshot
            .reorder_buffer
            .iter()
            .position(|entry| entry.is_live_staged() && entry.pc() == pc)?;
        let entry = self.snapshot.reorder_buffer[index];
        let scheduling =
            self.live_issue_scheduling_candidate_from_instruction(index, entry, instruction, &[])?;
        self.materialize_live_speculative_issue_candidate(&scheduling)
    }

    fn live_issue_scheduling_candidate_from_metadata(
        &self,
        index: usize,
        entry: O3ReorderBufferEntry,
        packet: &O3LiveIssuePacket,
    ) -> Option<O3LiveIssueSchedulingCandidate> {
        self.live_issue_scheduling_candidate_from_instruction(
            index,
            entry,
            packet.instruction(),
            packet.consumed_requests(),
        )
    }

    fn live_issue_scheduling_candidate_from_instruction(
        &self,
        index: usize,
        entry: O3ReorderBufferEntry,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<O3LiveIssueSchedulingCandidate> {
        let sequence = entry.sequence();
        if self
            .live_speculative_executions
            .iter()
            .any(|issued| issued.sequence == sequence)
        {
            return None;
        }
        let pc = entry.pc();
        if let Some((destination, producer_register, producer_sequence)) = self
            .pending_data_address_candidate_metadata(sequence, pc, instruction, consumed_requests)
        {
            let expected_producer = O3LiveIssueSourceProducer {
                sequence: producer_sequence,
                source: producer_register,
            };
            let mut data_producers = self.live_issue_source_producers(index, &[producer_register]);
            if data_producers.is_empty()
                && self
                    .pending_data_address_committed_producer_ready_tick(
                        producer_sequence,
                        producer_register,
                    )
                    .is_some()
            {
                data_producers.push(expected_producer);
            }
            if data_producers.as_slice() != [expected_producer] {
                return None;
            }
            return Some(O3LiveIssueSchedulingCandidate {
                sequence,
                pc,
                instruction,
                kind: O3LiveSpeculativeIssueKind::PendingDataAddress(destination),
                op_class: O3IssueOpClass::Memory,
                control_dependency: None,
                data_producers,
            });
        }
        if index == 0 || !self.live_staged_instruction_matches(entry.sequence(), instruction) {
            return None;
        }
        let (scalar_destination, control, sources) = if let Some((destination, sources)) =
            o3_predicted_scalar_descendant_operands(instruction)
        {
            (Some(destination), None, sources)
        } else {
            let control = o3_live_control_operands(instruction)?;
            let sources = control.sources().to_vec();
            (None, Some(control), sources)
        };
        let kind = if let Some(destination) = scalar_destination {
            O3LiveSpeculativeIssueKind::Scalar(staged_integer_destination(entry, destination)?)
        } else {
            let control = control.expect("control candidate has control operands");
            let destination = match control.destination() {
                Some(destination) => Some(staged_integer_destination(entry, destination)?),
                None if entry.destination().is_none() && entry.rename_destination().is_none() => {
                    None
                }
                None => return None,
            };
            O3LiveSpeculativeIssueKind::Control {
                kind: control.kind(),
                destination,
            }
        };
        let control_sequence = self.pending_control_sequence_for(sequence);
        Some(O3LiveIssueSchedulingCandidate {
            sequence,
            pc,
            instruction,
            kind,
            op_class: live_issue_op_class(instruction),
            control_dependency: control_sequence,
            data_producers: self.live_issue_source_producers(index, &sources),
        })
    }

    pub(crate) fn materialize_live_speculative_issue_candidate(
        &self,
        scheduling: &O3LiveIssueSchedulingCandidate,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let mut producer_sequences = Vec::new();
        let mut forwarded_register_writes = Vec::new();
        let mut forwarded_ready_tick = 0;
        for producer in scheduling.data_producers.iter().copied() {
            let (write, ready_tick) =
                match self.live_issue_source_value(producer.sequence(), producer.source()) {
                    Some((write, ready_tick)) => (Some(write), ready_tick),
                    None if scheduling.is_pending_data_address() => (
                        None,
                        self.pending_data_address_committed_producer_ready_tick(
                            producer.sequence(),
                            producer.source(),
                        )?,
                    ),
                    None => return None,
                };
            if !producer_sequences.contains(&producer.sequence()) {
                producer_sequences.push(producer.sequence());
            }
            if let Some(write) = write {
                if !forwarded_register_writes
                    .iter()
                    .any(|forwarded: &RegisterWrite| forwarded.register() == producer.source())
                {
                    forwarded_register_writes.push(write);
                }
            }
            forwarded_ready_tick = forwarded_ready_tick.max(ready_tick);
        }
        if let Some(control_sequence) = scheduling.control_dependency {
            if !producer_sequences.contains(&control_sequence) {
                producer_sequences.push(control_sequence);
            }
        }
        Some(O3LiveSpeculativeIssueCandidate {
            scheduling: scheduling.clone(),
            producer_sequences,
            forwarded_register_writes,
            forwarded_ready_tick,
        })
    }

    fn live_issue_source_producers(
        &self,
        consumer_index: usize,
        sources: &[Register],
    ) -> Vec<O3LiveIssueSourceProducer> {
        let mut producers = Vec::new();
        for source in sources.iter().copied().filter(|source| !source.is_zero()) {
            let producer = self.snapshot.reorder_buffer[..consumer_index]
                .iter()
                .rev()
                .copied()
                .find(|producer| {
                    producer.is_live_staged()
                        && producer.rename_destination()
                            == Some((O3RegisterClass::Integer, u32::from(source.index())))
                });
            if let Some(producer) = producer {
                let source_producer = O3LiveIssueSourceProducer {
                    sequence: producer.sequence(),
                    source,
                };
                if !producers.contains(&source_producer) {
                    producers.push(source_producer);
                }
            }
        }
        producers
    }

    #[cfg(test)]
    pub(crate) fn remove_live_staged_issue_identity_for_test(&mut self, sequence: u64) -> bool {
        self.live_staged_fetch_identities
            .remove(&sequence)
            .is_some()
    }
}

pub(in crate::o3_runtime) fn live_issue_op_class(instruction: RiscvInstruction) -> O3IssueOpClass {
    if o3_live_control_operands(instruction).is_some() {
        return O3IssueOpClass::Branch;
    }
    if matches!(
        o3_fu_latency_class(instruction),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    ) {
        O3IssueOpClass::IntMult
    } else {
        O3IssueOpClass::IntAlu
    }
}

fn live_issue_instruction_is_supported(instruction: RiscvInstruction) -> bool {
    o3_predicted_scalar_descendant_operands(instruction).is_some()
        || o3_live_control_operands(instruction).is_some()
}

fn control_destination_matches_rename_entry(
    destination: Option<Register>,
    rename: Option<O3RenameMapEntry>,
) -> bool {
    match (destination, rename) {
        (Some(destination), Some(rename)) => rename_matches_integer_register(rename, destination),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn staged_integer_destination(
    entry: O3ReorderBufferEntry,
    destination: Register,
) -> Option<O3RenameMapEntry> {
    staged_rename_entry(entry)
        .filter(|rename| rename_matches_integer_register(*rename, destination))
}

fn rename_matches_integer_register(rename: O3RenameMapEntry, register: Register) -> bool {
    rename.register_class() == O3RegisterClass::Integer
        && rename.architectural() == u32::from(register.index())
}
