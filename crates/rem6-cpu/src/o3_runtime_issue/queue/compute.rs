use rem6_isa_riscv::{Register, RiscvExecutionRecord, RiscvInstruction};
use rem6_memory::Address;

use super::super::super::o3_runtime_control_window::execution_writes_rename_destination;
use super::super::super::o3_runtime_live_window::staged_rename_entry;
use super::super::super::{
    o3_live_compute_operands, O3ArchitecturalRegister, O3LiveComputeClass, O3LiveComputeOperands,
    O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeState,
};
use super::super::O3LiveIssueTraceClass;
use crate::o3_dependency::O3RegisterClass;
use crate::o3_pipeline::O3IssueOpClass;
use crate::riscv_fu_latency::riscv_o3_fu_latency_class as o3_fu_latency_class;
use crate::O3RuntimeFuLatencyClass;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveComputeCandidateMetadata {
    destination: O3RenameMapEntry,
    op_class: O3IssueOpClass,
    integer_sources: Vec<Register>,
}

impl O3LiveComputeCandidateMetadata {
    pub(super) fn new(
        destination: O3RenameMapEntry,
        operands: &O3LiveComputeOperands,
        instruction: RiscvInstruction,
    ) -> Option<Self> {
        Some(Self {
            destination,
            op_class: compute_op_class_from_operands(instruction, operands),
            integer_sources: integer_sources(operands)?,
        })
    }

    pub(super) const fn destination(&self) -> O3RenameMapEntry {
        self.destination
    }

    pub(super) const fn op_class(&self) -> O3IssueOpClass {
        self.op_class
    }

    pub(super) fn integer_sources(&self) -> &[Register] {
        &self.integer_sources
    }
}

pub(super) fn compute_candidate_metadata(
    runtime: &O3RuntimeState,
    consumer_index: usize,
    entry: O3ReorderBufferEntry,
    instruction: RiscvInstruction,
) -> Option<O3LiveComputeCandidateMetadata> {
    let operands = o3_live_compute_operands(instruction)?;
    let staged_rename_entry = staged_rename_entry(entry)?;
    if !typed_destination_matches_rename_entry(operands.destination(), staged_rename_entry) {
        return None;
    }
    let metadata =
        O3LiveComputeCandidateMetadata::new(staged_rename_entry, &operands, instruction)?;
    if has_unforwardable_live_source(runtime, consumer_index, operands.sources()) {
        return None;
    }
    Some(metadata)
}

pub(super) fn staged_compute_destination(
    entry: O3ReorderBufferEntry,
    instruction: RiscvInstruction,
) -> Option<O3RenameMapEntry> {
    let operands = o3_live_compute_operands(instruction)?;
    let destination = operands.destination();
    staged_rename_entry(entry)
        .filter(|rename| typed_destination_matches_rename_entry(destination, *rename))
}

pub(super) fn staged_integer_destination(
    entry: O3ReorderBufferEntry,
    destination: Register,
) -> Option<O3RenameMapEntry> {
    staged_rename_entry(entry)
        .filter(|rename| rename_matches_integer_register(*rename, destination))
}

pub(super) fn compute_op_class(instruction: RiscvInstruction) -> Option<O3IssueOpClass> {
    let operands = o3_live_compute_operands(instruction)?;
    Some(compute_op_class_from_operands(instruction, &operands))
}

pub(super) fn compute_trace_class(instruction: RiscvInstruction) -> Option<O3LiveIssueTraceClass> {
    let operands = o3_live_compute_operands(instruction)?;
    Some(match operands.class() {
        O3LiveComputeClass::ScalarInteger => {
            match compute_op_class_from_operands(instruction, &operands) {
                O3IssueOpClass::IntMult => O3LiveIssueTraceClass::IntegerMulDiv,
                _ => O3LiveIssueTraceClass::ScalarInteger,
            }
        }
        O3LiveComputeClass::ScalarFloat => O3LiveIssueTraceClass::ScalarFloat,
        O3LiveComputeClass::VectorToScalar => O3LiveIssueTraceClass::VectorToScalar,
    })
}

pub(super) fn valid_recorded_compute_execution(
    execution: &RiscvExecutionRecord,
    pc: Address,
    instruction: RiscvInstruction,
    destination: O3RenameMapEntry,
) -> bool {
    if Address::new(execution.pc()) != pc
        || execution.instruction() != instruction
        || execution.trap().is_some()
        || execution.system_event().is_some()
        || execution.memory_access().is_some()
    {
        return false;
    }
    execution.next_pc()
        == execution
            .pc()
            .wrapping_add(u64::from(execution.instruction_bytes()))
        && execution_exactly_writes_compute_destination(execution, destination)
}

pub(super) fn execution_exactly_writes_compute_destination(
    execution: &RiscvExecutionRecord,
    destination: O3RenameMapEntry,
) -> bool {
    match destination.register_class() {
        O3RegisterClass::Integer => {
            execution.register_writes().len() == 1
                && execution.float_register_writes().is_empty()
                && execution_writes_rename_destination(execution, destination)
        }
        O3RegisterClass::FloatingPoint => {
            execution.register_writes().is_empty()
                && execution.float_register_writes().len() == 1
                && execution_writes_rename_destination(execution, destination)
        }
        O3RegisterClass::Vector | O3RegisterClass::ConditionCode | O3RegisterClass::Misc => false,
    }
}

fn compute_op_class_from_operands(
    instruction: RiscvInstruction,
    operands: &O3LiveComputeOperands,
) -> O3IssueOpClass {
    match operands.class() {
        O3LiveComputeClass::ScalarInteger => {
            if matches!(
                o3_fu_latency_class(instruction),
                Some(
                    O3RuntimeFuLatencyClass::ScalarIntegerMul
                        | O3RuntimeFuLatencyClass::ScalarIntegerDiv
                )
            ) {
                O3IssueOpClass::IntMult
            } else {
                O3IssueOpClass::IntAlu
            }
        }
        O3LiveComputeClass::ScalarFloat => O3IssueOpClass::Float,
        O3LiveComputeClass::VectorToScalar => O3IssueOpClass::Vector,
    }
}

fn integer_sources(operands: &O3LiveComputeOperands) -> Option<Vec<Register>> {
    operands
        .sources()
        .iter()
        .copied()
        .filter(|source| source.register_class() == O3RegisterClass::Integer)
        .map(|source| {
            u8::try_from(source.architectural())
                .ok()
                .and_then(|index| Register::new(index).ok())
        })
        .collect()
}

pub(super) fn rename_matches_integer_register(
    rename: O3RenameMapEntry,
    register: Register,
) -> bool {
    rename.register_class() == O3RegisterClass::Integer
        && rename.architectural() == u32::from(register.index())
}

fn typed_destination_matches_rename_entry(
    destination: O3ArchitecturalRegister,
    rename: O3RenameMapEntry,
) -> bool {
    rename.register_class() == destination.register_class()
        && rename.architectural() == destination.architectural()
}

fn has_unforwardable_live_source(
    runtime: &O3RuntimeState,
    consumer_index: usize,
    sources: &[O3ArchitecturalRegister],
) -> bool {
    sources.iter().copied().any(|source| {
        source.register_class() != O3RegisterClass::Integer
            && older_live_source_producer(runtime, consumer_index, source)
    })
}

fn older_live_source_producer(
    runtime: &O3RuntimeState,
    consumer_index: usize,
    source: O3ArchitecturalRegister,
) -> bool {
    runtime.snapshot.reorder_buffer[..consumer_index]
        .iter()
        .rev()
        .any(|producer| {
            producer.is_live_staged()
                && producer.rename_destination()
                    == Some((source.register_class(), source.architectural()))
        })
}
