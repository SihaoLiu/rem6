use rem6_memory::{AccessSize, AddressRange};

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3ScalarMemoryPairAdmission {
    Independent,
    Forwarded { value: u64, bytes: usize },
}

impl O3RuntimeState {
    pub(crate) fn has_open_scalar_memory_overlap_slot(&self) -> bool {
        self.live_scalar_memories.len() == 1
            && self.live_scalar_memory_younger_sequences.is_empty()
            && self.live_scalar_memories[0].outcome == O3LiveScalarMemoryOutcome::Resident
            && matches!(
                self.live_scalar_memories[0]
                    .execution
                    .execution()
                    .memory_access(),
                Some(MemoryAccessKind::Load { .. } | MemoryAccessKind::Store { .. })
            )
    }

    pub(crate) fn can_defer_second_scalar_memory_instruction(
        &self,
        instruction: RiscvInstruction,
        younger_range: AddressRange,
    ) -> bool {
        self.scalar_memory_pair_admission(instruction, younger_range)
            .is_some()
    }

    fn scalar_memory_pair_admission(
        &self,
        instruction: RiscvInstruction,
        younger_range: AddressRange,
    ) -> Option<O3ScalarMemoryPairAdmission> {
        if !self.has_open_scalar_memory_overlap_slot() {
            return None;
        }
        let RiscvInstruction::Load { rd, rs1, .. } = instruction else {
            return None;
        };
        if rd.is_zero() {
            return None;
        }
        let older = self.live_scalar_memories[0]
            .execution
            .execution()
            .memory_access()?;
        match older {
            MemoryAccessKind::Load { rd: older_rd, .. } if rd != *older_rd && rs1 != *older_rd => {
                Some(O3ScalarMemoryPairAdmission::Independent)
            }
            MemoryAccessKind::Store { value, width, .. } => {
                let older_range = scalar_memory_range(older)?;
                if older_range == younger_range {
                    Some(O3ScalarMemoryPairAdmission::Forwarded {
                        value: *value,
                        bytes: width.bytes(),
                    })
                } else if !older_range.overlaps(younger_range) {
                    Some(O3ScalarMemoryPairAdmission::Independent)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(crate) fn forwarded_scalar_load_data(
        &self,
        instruction: RiscvInstruction,
        access: &MemoryAccessKind,
    ) -> Option<Vec<u8>> {
        if !matches!(access, MemoryAccessKind::Load { .. }) {
            return None;
        }
        let range = scalar_memory_range(access)?;
        let O3ScalarMemoryPairAdmission::Forwarded { value, bytes } =
            self.scalar_memory_pair_admission(instruction, range)?
        else {
            return None;
        };
        Some(value.to_le_bytes()[..bytes].to_vec())
    }

    pub(crate) fn can_stage_second_scalar_memory(
        &self,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        let Some(access @ MemoryAccessKind::Load { .. }) = execution.execution().memory_access()
        else {
            return false;
        };
        scalar_memory_range(access).is_some_and(|younger_range| {
            self.can_defer_second_scalar_memory_instruction(execution.instruction(), younger_range)
        })
    }
}

fn scalar_memory_range(access: &MemoryAccessKind) -> Option<AddressRange> {
    let (address, width) = match access {
        MemoryAccessKind::Load { address, width, .. }
        | MemoryAccessKind::Store { address, width, .. } => (*address, *width),
        _ => return None,
    };
    let size = AccessSize::new(width.bytes() as u64).ok()?;
    AddressRange::new(Address::new(address), size).ok()
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryWidth, Register, RiscvExecutionRecord, RiscvInstruction,
    };
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{CpuFetchEvent, CpuFetchRecord};

    #[test]
    fn disjoint_store_then_load_stages_two_live_scalar_memory_rows() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_store_event(0x8000, 10, 0x9000);
        let younger = scalar_load_event(0x8004, 11, 13, 10, 0x9004);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert_eq!(
            runtime.forwarded_scalar_load_data(
                younger.instruction(),
                younger.execution().memory_access().unwrap(),
            ),
            None
        );
        assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));

        assert_eq!(runtime.live_scalar_memories.len(), 2);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
        assert_eq!(
            runtime.snapshot().load_store_queue()[0].kind(),
            O3LoadStoreQueueKind::Store
        );
        assert_eq!(
            runtime.snapshot().load_store_queue()[1].kind(),
            O3LoadStoreQueueKind::Load
        );
    }

    #[test]
    fn exact_store_then_load_stages_forwarding_pair() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_store_event(0x8000, 10, 0x9000);
        let younger = scalar_load_event(0x8004, 11, 13, 10, 0x9000);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert_eq!(
            runtime.forwarded_scalar_load_data(
                younger.instruction(),
                younger.execution().memory_access().unwrap(),
            ),
            Some(vec![0x2a, 0, 0, 0])
        );
        assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));

        assert_eq!(runtime.live_scalar_memories.len(), 2);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    }

    #[test]
    fn store_then_partially_overlapping_load_does_not_stage_second_live_row() {
        for younger_address in [0x8fff, 0x9002, 0x9003] {
            let mut runtime = O3RuntimeState::default();
            let older = scalar_store_event(0x8000, 10, 0x9000);
            let younger = scalar_load_event(0x8004, 11, 13, 10, younger_address);

            assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
            assert!(
                !runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32),
                "younger load at {younger_address:#x} overlaps the older word store"
            );
            assert_eq!(runtime.live_scalar_memories.len(), 1);
        }
    }

    #[test]
    fn store_then_same_address_width_mismatch_does_not_stage_second_live_row() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_store_event(0x8000, 10, 0x9000);
        let younger = scalar_load_event_with_width(0x8004, 11, 13, 10, 0x9000, MemoryWidth::Byte);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert!(!runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));
        assert_eq!(runtime.live_scalar_memories.len(), 1);
    }

    #[test]
    fn load_then_store_remains_serialized() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_load_event(0x8000, 10, 12, 10, 0x9000);
        let younger = scalar_store_event(0x8004, 11, 0x9040);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert!(!runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));
        assert_eq!(runtime.live_scalar_memories.len(), 1);
    }

    fn scalar_load_event(
        pc: u64,
        sequence: u64,
        rd: u8,
        rs1: u8,
        address: u64,
    ) -> RiscvCpuExecutionEvent {
        scalar_load_event_with_width(pc, sequence, rd, rs1, address, MemoryWidth::Word)
    }

    fn scalar_load_event_with_width(
        pc: u64,
        sequence: u64,
        rd: u8,
        rs1: u8,
        address: u64,
        width: MemoryWidth,
    ) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Load {
            rd: reg(rd),
            rs1: reg(rs1),
            offset: Immediate::new(0),
            width,
            signed: false,
        };
        let access = MemoryAccessKind::Load {
            rd: reg(rd),
            address,
            width,
            signed: false,
        };
        execution_event(pc, sequence, instruction, access)
    }

    fn scalar_store_event(pc: u64, sequence: u64, address: u64) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Store {
            rs1: reg(10),
            rs2: reg(11),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
        };
        let access = MemoryAccessKind::Store {
            address,
            width: MemoryWidth::Word,
            value: 0x2a,
        };
        execution_event(pc, sequence, instruction, access)
    }

    fn execution_event(
        pc: u64,
        sequence: u64,
        instruction: RiscvInstruction,
        access: MemoryAccessKind,
    ) -> RiscvCpuExecutionEvent {
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
                memory_request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0073_u32.to_le_bytes().to_vec(),
        )
    }

    fn memory_request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    fn reg(index: u8) -> Register {
        Register::new(index).unwrap()
    }
}
