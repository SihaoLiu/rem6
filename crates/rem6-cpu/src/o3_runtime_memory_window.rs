use rem6_memory::{AccessSize, AddressRange};

use super::*;

const MAX_LIVE_SCALAR_MEMORIES: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3ScalarMemoryWindowAdmission {
    Independent,
    Forwarded { value: u64, bytes: usize },
}

impl O3RuntimeState {
    pub(super) fn has_scalar_memory_window_capacity(&self) -> bool {
        self.live_scalar_memories.len() < MAX_LIVE_SCALAR_MEMORIES
    }

    pub(crate) fn has_open_scalar_memory_window_slot(&self) -> bool {
        let store_can_extend = self.live_scalar_memories.len() == 1;
        !self.live_scalar_memories.is_empty()
            && self.has_scalar_memory_window_capacity()
            && self.live_scalar_memory_younger_sequences.is_empty()
            && self.live_scalar_memories.iter().all(|live| {
                live.outcome == O3LiveScalarMemoryOutcome::Resident
                    && match live.execution.execution().memory_access() {
                        Some(MemoryAccessKind::Load { .. }) => true,
                        Some(MemoryAccessKind::Store { .. }) => store_can_extend,
                        _ => false,
                    }
            })
    }

    pub(crate) fn can_defer_scalar_memory_instruction(
        &self,
        instruction: RiscvInstruction,
        younger_range: AddressRange,
    ) -> bool {
        self.scalar_memory_window_admission(instruction, younger_range)
            .is_some()
    }

    fn scalar_memory_window_admission(
        &self,
        instruction: RiscvInstruction,
        younger_range: AddressRange,
    ) -> Option<O3ScalarMemoryWindowAdmission> {
        if !self.has_open_scalar_memory_window_slot() {
            return None;
        }
        let RiscvInstruction::Load { rd, rs1, .. } = instruction else {
            return None;
        };
        if rd.is_zero() {
            return None;
        }
        if self.live_scalar_memories.len() > 1 {
            return self
                .live_scalar_memories
                .iter()
                .all(|live| {
                    matches!(
                        live.execution.execution().memory_access(),
                        Some(MemoryAccessKind::Load { rd: older_rd, .. })
                            if rd != *older_rd && rs1 != *older_rd
                    )
                })
                .then_some(O3ScalarMemoryWindowAdmission::Independent);
        }
        let older = self.live_scalar_memories[0]
            .execution
            .execution()
            .memory_access()?;
        match older {
            MemoryAccessKind::Load { rd: older_rd, .. } if rd != *older_rd && rs1 != *older_rd => {
                Some(O3ScalarMemoryWindowAdmission::Independent)
            }
            MemoryAccessKind::Store { value, width, .. } => {
                let older_range = scalar_memory_range(older)?;
                if older_range == younger_range {
                    Some(O3ScalarMemoryWindowAdmission::Forwarded {
                        value: *value,
                        bytes: width.bytes(),
                    })
                } else if !older_range.overlaps(younger_range) {
                    Some(O3ScalarMemoryWindowAdmission::Independent)
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
        let O3ScalarMemoryWindowAdmission::Forwarded { value, bytes } =
            self.scalar_memory_window_admission(instruction, range)?
        else {
            return None;
        };
        Some(value.to_le_bytes()[..bytes].to_vec())
    }

    pub(crate) fn can_stage_scalar_memory(&self, execution: &RiscvCpuExecutionEvent) -> bool {
        let Some(access @ MemoryAccessKind::Load { .. }) = execution.execution().memory_access()
        else {
            return false;
        };
        scalar_memory_range(access).is_some_and(|younger_range| {
            self.can_defer_scalar_memory_instruction(execution.instruction(), younger_range)
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
    fn three_independent_scalar_loads_stage_three_live_rows() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_load_event(0x8000, 10, 12, 10, 0x9000);
        let middle = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let younger = scalar_load_event(0x8008, 12, 14, 10, 0x9080);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert!(runtime.stage_live_scalar_memory_issue(&middle, memory_request(21), 32));
        assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(22), 33));

        assert_eq!(runtime.live_scalar_memories.len(), 3);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 3);
    }

    #[test]
    fn third_scalar_load_waits_for_middle_address_dependency() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_load_event(0x8000, 10, 12, 10, 0x9000);
        let middle = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let dependent = scalar_load_event(0x8008, 12, 14, 13, 0x9080);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert!(runtime.stage_live_scalar_memory_issue(&middle, memory_request(21), 32));
        assert!(!runtime.stage_live_scalar_memory_issue(&dependent, memory_request(22), 33));

        assert_eq!(runtime.live_scalar_memories.len(), 2);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    }

    #[test]
    fn three_scalar_loads_complete_out_of_order_and_retire_oldest_first() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_load_event(0x8000, 10, 12, 10, 0x9000);
        let middle = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let younger = scalar_load_event(0x8008, 12, 14, 10, 0x9080);
        let requests = [memory_request(20), memory_request(21), memory_request(22)];

        for (event, request, issue_tick) in [
            (&older, requests[0], 31),
            (&middle, requests[1], 32),
            (&younger, requests[2], 33),
        ] {
            assert!(runtime.stage_live_scalar_memory_issue(event, request, issue_tick));
        }

        let mut completed = [younger.clone(), middle.clone(), older.clone()];
        for event in &mut completed {
            event.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        }
        for (event, request, response_tick, latency_ticks, data) in [
            (&completed[0], requests[2], 40, 7, [0x77, 0, 0, 0]),
            (&completed[1], requests[1], 42, 10, [0x63, 0, 0, 0]),
        ] {
            assert!(runtime.complete_live_scalar_memory_response(
                event,
                request,
                response_tick,
                latency_ticks,
                Some(&data),
            ));
            assert!(runtime.take_ready_live_scalar_memory_event().is_none());
        }
        assert!(runtime.complete_live_scalar_memory_response(
            &completed[2],
            requests[0],
            45,
            14,
            Some(&[0x2a, 0, 0, 0]),
        ));

        for expected in [&completed[2], &completed[1], &completed[0]] {
            let retired = runtime
                .take_ready_live_scalar_memory_event()
                .expect("completed scalar load should retire in program order");
            assert_eq!(&retired, expected);
            runtime.record_retired_instruction_with_trace(&retired, true);
        }

        assert!(runtime.scalar_memory_lifecycle_is_quiescent());
        assert_eq!(
            runtime
                .trace_records()
                .iter()
                .copied()
                .map(O3RuntimeTraceRecord::lsq_data_response_tick)
                .collect::<Vec<_>>(),
            vec![45, 42, 40]
        );
        assert!(runtime
            .trace_records()
            .windows(2)
            .all(|pair| pair[0].commit_tick() <= pair[1].commit_tick()));
    }

    #[test]
    fn middle_scalar_load_failure_discards_only_the_younger_suffix() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_load_event(0x8000, 10, 12, 10, 0x9000);
        let middle = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let younger = scalar_load_event(0x8008, 12, 14, 10, 0x9080);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert!(runtime.stage_live_scalar_memory_issue(&middle, memory_request(21), 32));
        assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(22), 33));
        let mut failed = middle.clone();
        failed.set_data_access_event_kind(RiscvDataAccessEventKind::Failed);

        assert!(runtime.complete_live_scalar_memory_response(
            &failed,
            memory_request(21),
            40,
            8,
            None,
        ));

        assert_eq!(runtime.live_scalar_memories.len(), 2);
        assert_eq!(
            runtime.live_scalar_memories[0].fetch_request,
            older.fetch().request_id()
        );
        assert_eq!(
            runtime.live_scalar_memories[1].fetch_request,
            middle.fetch().request_id()
        );
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    }

    #[test]
    fn store_load_pair_does_not_expand_to_a_third_memory_row() {
        let mut runtime = O3RuntimeState::default();
        let store = scalar_store_event(0x8000, 10, 0x9000);
        let load = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let third = scalar_load_event(0x8008, 12, 14, 10, 0x9080);

        assert!(runtime.stage_live_scalar_memory_issue(&store, memory_request(20), 31));
        assert!(runtime.stage_live_scalar_memory_issue(&load, memory_request(21), 32));
        assert!(!runtime.has_open_scalar_memory_window_slot());
        assert!(!runtime.stage_live_scalar_memory_issue(&third, memory_request(22), 33));
        assert_eq!(runtime.live_scalar_memories.len(), 2);
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
