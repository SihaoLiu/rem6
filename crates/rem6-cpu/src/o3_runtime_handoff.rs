use super::{O3LiveScalarMemoryOutcome, O3RuntimeState};
use rem6_isa_riscv::MemoryAccessKind;

use crate::riscv_execution_mode_handoff::{
    RiscvCompletedPartialScalarLoadHandoff, RiscvForwardedScalarLoadHandoff,
    RiscvO3LiveDataHandoffOperation, RiscvResidentScalarMemoryHandoff,
};

impl O3RuntimeState {
    pub(crate) fn live_scalar_memory_handoff(
        &self,
    ) -> Option<(
        Vec<RiscvResidentScalarMemoryHandoff>,
        Vec<RiscvForwardedScalarLoadHandoff>,
        Vec<RiscvCompletedPartialScalarLoadHandoff>,
        usize,
    )> {
        if self.deferred_scalar_memory_execution.is_some() || self.live_scalar_memories.is_empty() {
            return None;
        }
        let mut resident_rows = Vec::new();
        let mut forwarded_rows = Vec::new();
        let mut completed_partial_rows = Vec::new();
        for live in &self.live_scalar_memories {
            let trace_sequence = self
                .pending_data_accesses
                .get(&live.fetch_request)
                .and_then(|pending| pending.trace_sequence);
            if live.outcome == O3LiveScalarMemoryOutcome::Resident
                && !live.event_taken
                && live.response_tick.is_none()
                && live.latency_ticks.is_none()
                && live.commit_tick.is_none()
                && live.load_data.is_none()
                && live.forwarding_plan.is_none()
            {
                let operation = match live.execution.execution().memory_access()? {
                    MemoryAccessKind::Load { .. } => RiscvO3LiveDataHandoffOperation::Load,
                    MemoryAccessKind::Store { .. } => RiscvO3LiveDataHandoffOperation::Store,
                    _ => return None,
                };
                resident_rows.push(RiscvResidentScalarMemoryHandoff {
                    fetch_request: live.fetch_request,
                    data_request: live.data_request,
                    issue_tick: live.issue_tick,
                    operation,
                    o3_sequence: live.sequence,
                    trace_sequence,
                });
                continue;
            }
            let forwarding_plan = live.forwarding_plan?;
            if live.outcome != O3LiveScalarMemoryOutcome::Completed
                || live.event_taken
                || live.commit_tick.is_some()
                || !matches!(
                    live.execution.execution().memory_access(),
                    Some(MemoryAccessKind::Load { .. })
                )
            {
                return None;
            }
            let response_tick = live.response_tick?;
            live.latency_ticks?;
            let data = live.load_data.as_deref()?;
            let bytes = forwarding_plan.bytes();
            if data.len() != bytes as usize || !forwarding_plan.matches_data(data) {
                return None;
            }
            let mut fixed_data = [0; 8];
            fixed_data[..data.len()].copy_from_slice(data);
            if forwarding_plan.is_partial() {
                completed_partial_rows.push(RiscvCompletedPartialScalarLoadHandoff {
                    fetch_request: live.fetch_request,
                    data_request: live.data_request,
                    issue_tick: live.issue_tick,
                    response_tick,
                    address: forwarding_plan.load_range().start(),
                    bytes,
                    original_forwarded_mask: forwarding_plan.forwarded_mask(),
                    data: fixed_data,
                    o3_sequence: live.sequence,
                    trace_sequence,
                });
                continue;
            }
            forwarded_rows.push(RiscvForwardedScalarLoadHandoff {
                fetch_request: live.fetch_request,
                data_request: live.data_request,
                issue_tick: live.issue_tick,
                response_tick,
                address: forwarding_plan.load_range().start(),
                bytes,
                data: fixed_data,
                o3_sequence: live.sequence,
                trace_sequence,
            });
        }
        Some((
            resident_rows,
            forwarded_rows,
            completed_partial_rows,
            self.live_scalar_memory_younger_sequences.len(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryWidth, Register, RiscvExecutionRecord, RiscvInstruction,
    };
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{CpuFetchEvent, CpuFetchRecord, RiscvCpuExecutionEvent, RiscvDataAccessEventKind};

    #[test]
    fn completed_partial_overlay_becomes_live_handoff_authority() {
        let mut runtime = O3RuntimeState::default();
        let store = scalar_store_event(0x8000, 10, 0x9001);
        let load = scalar_load_event(0x8004, 11, 0x9000);
        assert!(runtime.stage_live_scalar_memory_issue(&store, memory_request(20), 31));
        let plan = runtime
            .scalar_load_forwarding_plan(
                load.instruction(),
                load.execution().memory_access().unwrap(),
            )
            .expect("byte store should partially overlay the word load");
        assert!(plan.is_partial());
        assert!(runtime.stage_live_scalar_memory_issue(&load, memory_request(21), 32));
        let mut completed = load.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime
            .complete_live_scalar_memory_forwarding(
                &completed,
                memory_request(21),
                40,
                8,
                &[0x11, 0x5a, 0x33, 0x80],
                plan,
            )
            .unwrap());

        let (resident, forwarded, completed_partial, younger) = runtime
            .live_scalar_memory_handoff()
            .expect("completed partial row should be transferable");
        assert_eq!(resident.len(), 1);
        assert!(forwarded.is_empty());
        assert_eq!(completed_partial.len(), 1);
        assert_eq!(younger, 0);
        assert_eq!(
            completed_partial[0].fetch_request,
            load.fetch().request_id()
        );
        assert_eq!(completed_partial[0].data_request, memory_request(21));
        assert_eq!(completed_partial[0].issue_tick, 32);
        assert_eq!(completed_partial[0].response_tick, 40);
        assert_eq!(completed_partial[0].address, Address::new(0x9000));
        assert_eq!(completed_partial[0].bytes, 4);
        assert_eq!(completed_partial[0].original_forwarded_mask, 0b0010);
        assert_eq!(&completed_partial[0].data[..4], &[0x11, 0x5a, 0x33, 0x80]);
        assert_eq!(completed_partial[0].o3_sequence, 1);
    }

    fn scalar_load_event(pc: u64, sequence: u64, address: u64) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Load {
            rd: reg(12),
            rs1: reg(10),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        };
        execution_event(
            pc,
            sequence,
            instruction,
            MemoryAccessKind::Load {
                rd: reg(12),
                address,
                width: MemoryWidth::Word,
                signed: false,
            },
        )
    }

    fn scalar_store_event(pc: u64, sequence: u64, address: u64) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Store {
            rs1: reg(10),
            rs2: reg(11),
            offset: Immediate::new(0),
            width: MemoryWidth::Byte,
        };
        execution_event(
            pc,
            sequence,
            instruction,
            MemoryAccessKind::Store {
                address,
                width: MemoryWidth::Byte,
                value: 0x5a,
            },
        )
    }

    fn execution_event(
        pc: u64,
        sequence: u64,
        instruction: RiscvInstruction,
        access: MemoryAccessKind,
    ) -> RiscvCpuExecutionEvent {
        RiscvCpuExecutionEvent::new(
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
            ),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        )
    }

    fn memory_request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    fn reg(index: u8) -> Register {
        Register::new(index).unwrap()
    }
}
