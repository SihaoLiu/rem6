use rem6_isa_riscv::{
    FloatRegisterWrite, MemoryAccessKind, RegisterWrite, RiscvExecutionRecord, RiscvInstruction,
};

use crate::o3_runtime::{o3_live_compute_operands, O3LiveComputeClass};
use crate::{
    CpuFetchEvent, CpuFetchEventKind, RiscvCpuExecutionEvent, RiscvDataAccessEventKind,
    RiscvO3LiveCheckpointError as Error,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointEvent {
    pub fetch: CpuFetchEvent,
    pub execution_pc: u64,
    pub next_pc: u64,
    pub instruction_bytes: u8,
    pub register_writes: Vec<RegisterWrite>,
    pub float_register_writes: Vec<FloatRegisterWrite>,
    pub memory_access: Option<MemoryAccessKind>,
    pub data_access_event_kind: Option<RiscvDataAccessEventKind>,
    pub counts_as_retired_instruction: bool,
}

impl RiscvO3LiveCheckpointEvent {
    pub fn rebuild(&self) -> Result<RiscvCpuExecutionEvent, Error> {
        if self.fetch.kind() != CpuFetchEventKind::Completed {
            return Err(unsupported("fetch is not completed"));
        }
        if self.fetch.pc().get() != self.execution_pc {
            return Err(Error::InstructionMismatch);
        }
        let raw_bytes = self
            .fetch
            .data()
            .ok_or(unsupported("completed fetch has no bytes"))?;
        if !matches!(raw_bytes.len(), 2 | 4) || self.fetch.size().bytes() != raw_bytes.len() as u64
        {
            return Err(unsupported("fetch is not one bounded instruction request"));
        }
        let mut padded = [0_u8; 4];
        padded[..raw_bytes.len()].copy_from_slice(raw_bytes);
        let raw = u32::from_le_bytes(padded);
        let decoded = RiscvInstruction::decode_with_length(raw)
            .map_err(|_| Error::InstructionDecode { raw })?;
        if decoded.bytes() != self.instruction_bytes
            || usize::from(decoded.bytes()) != raw_bytes.len()
        {
            return Err(Error::InstructionWidth {
                encoded: self.instruction_bytes,
                decoded: decoded.bytes(),
            });
        }
        let instruction = decoded.instruction();
        validate_instruction_shape(
            instruction,
            self.memory_access.as_ref(),
            &self.register_writes,
            &self.float_register_writes,
        )?;
        match (&self.memory_access, self.data_access_event_kind) {
            (None, None)
            | (
                Some(MemoryAccessKind::FloatLoad { .. }),
                Some(RiscvDataAccessEventKind::Completed),
            ) => {}
            (Some(MemoryAccessKind::FloatLoad { .. }), None) => {
                return Err(unsupported("FP load has no completed data event"));
            }
            _ => return Err(unsupported("data access is not one completed FP load")),
        }
        let expected_next_pc = self
            .execution_pc
            .checked_add(u64::from(self.instruction_bytes))
            .ok_or(Error::InstructionMismatch)?;
        if self.next_pc != expected_next_pc {
            return Err(unsupported("execution is not non-control sequential flow"));
        }
        let execution = RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
            instruction,
            self.instruction_bytes,
            self.execution_pc,
            self.next_pc,
            self.register_writes.clone(),
            self.float_register_writes.clone(),
            self.memory_access.clone(),
        );
        let mut event = RiscvCpuExecutionEvent::with_retired_instruction_counting(
            self.fetch.clone(),
            instruction,
            execution,
            None,
            self.counts_as_retired_instruction,
        );
        if let Some(kind) = self.data_access_event_kind {
            event.set_data_access_event_kind(kind);
        }
        Ok(event)
    }
}

fn validate_instruction_shape(
    instruction: RiscvInstruction,
    memory_access: Option<&MemoryAccessKind>,
    integer_writes: &[RegisterWrite],
    float_writes: &[FloatRegisterWrite],
) -> Result<(), Error> {
    if let (
        RiscvInstruction::FloatLoad { rd, width, .. },
        Some(MemoryAccessKind::FloatLoad {
            rd: access_rd,
            width: access_width,
            ..
        }),
    ) = (instruction, memory_access)
    {
        return (rd == *access_rd
            && width == *access_width
            && integer_writes.is_empty()
            && float_writes.is_empty())
        .then_some(())
        .ok_or(Error::InstructionMismatch);
    }
    let Some(operands) = o3_live_compute_operands(instruction).filter(|_| memory_access.is_none())
    else {
        return Err(unsupported(
            "instruction is control, system, vector, or unsupported memory",
        ));
    };
    let destination = operands.destination();
    let writes_match = match operands.class() {
        O3LiveComputeClass::ScalarInteger => {
            float_writes.is_empty()
                && matches!(integer_writes, [write] if Some(write.register()) == destination.integer_register())
        }
        O3LiveComputeClass::ScalarFloat => {
            integer_writes.is_empty()
                && matches!(float_writes, [write] if Some(write.register()) == destination.float_register())
        }
        O3LiveComputeClass::VectorToScalar => false,
    };
    writes_match.then_some(()).ok_or(Error::InstructionMismatch)
}

fn unsupported(reason: &'static str) -> Error {
    Error::UnsupportedEvent { reason }
}
