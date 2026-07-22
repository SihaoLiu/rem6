use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvExecutionRecord, RiscvInstruction};
use rem6_memory::MemoryRequestId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssuePacket {
    decoded: RiscvDecodedInstruction,
    consumed_requests: Vec<MemoryRequestId>,
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
