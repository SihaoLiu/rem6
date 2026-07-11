use rem6_memory::AddressRange;

use super::o3_store_forwarding::{
    o3_load_forwarding_access, o3_store_load_relation, O3StoreLoadForwardingPlan,
    O3StoreLoadRelation,
};
use super::*;
use crate::riscv_scalar_memory_window::independent_scalar_load_destination;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3ScalarMemoryWindowAdmission {
    Independent,
    Forwarded(O3StoreLoadForwardingPlan),
    Overlay(O3StoreLoadForwardingPlan),
}

impl O3RuntimeState {
    pub(crate) fn set_scalar_memory_window_limit(&mut self, limit: usize) {
        self.scalar_memory_window_limit = limit.clamp(1, MAX_O3_SCALAR_MEMORY_DEPTH);
        self.scalar_memory_window_limit_explicit = true;
    }

    pub(crate) fn set_branch_derived_scalar_memory_window_limit(&mut self, limit: usize) {
        if !self.scalar_memory_window_limit_explicit {
            self.scalar_memory_window_limit = limit.clamp(1, MAX_O3_SCALAR_MEMORY_DEPTH);
        }
    }

    pub(crate) const fn scalar_memory_window_limit(&self) -> usize {
        self.scalar_memory_window_limit
    }

    pub(crate) fn scalar_load_window_destinations(&self) -> Option<Vec<Register>> {
        if self.deferred_scalar_memory_execution.is_some()
            || !self.live_scalar_memory_younger_sequences.is_empty()
            || self
                .live_scalar_memories
                .iter()
                .any(|live| live.outcome != O3LiveScalarMemoryOutcome::Resident)
        {
            return None;
        }
        self.live_scalar_memories
            .iter()
            .map(|live| {
                independent_scalar_load_destination(
                    live.execution.instruction(),
                    std::iter::empty(),
                )
            })
            .collect()
    }

    pub(super) fn has_scalar_memory_window_capacity(&self) -> bool {
        self.live_scalar_memories.len() < self.scalar_memory_window_limit
    }

    pub(crate) fn can_consider_scalar_memory_younger(&self) -> bool {
        !self.live_scalar_memories.is_empty()
            && self.has_scalar_memory_window_capacity()
            && self.live_scalar_memory_younger_sequences.is_empty()
            && self
                .live_scalar_memories
                .iter()
                .all(|live| live.outcome == O3LiveScalarMemoryOutcome::Resident)
            && (self.live_scalar_memories.iter().all(|live| {
                independent_scalar_load_destination(
                    live.execution.instruction(),
                    std::iter::empty(),
                )
                .is_some()
            }) || (self.live_scalar_memories.len() == 1
                && matches!(
                    self.live_scalar_memories[0]
                        .execution
                        .execution()
                        .memory_access(),
                    Some(MemoryAccessKind::Store { .. })
                )))
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
        if !self.can_consider_scalar_memory_younger() {
            return None;
        }
        let older_load_destinations = self
            .live_scalar_memories
            .iter()
            .map(|live| {
                independent_scalar_load_destination(
                    live.execution.instruction(),
                    std::iter::empty(),
                )
            })
            .collect::<Option<Vec<_>>>();
        if let Some(older_load_destinations) = older_load_destinations {
            return independent_scalar_load_destination(instruction, older_load_destinations)
                .map(|_| O3ScalarMemoryWindowAdmission::Independent);
        }
        if self.live_scalar_memories.len() != 1 {
            return None;
        }
        independent_scalar_load_destination(instruction, std::iter::empty())?;
        let older = self.live_scalar_memories[0]
            .execution
            .execution()
            .memory_access()?;
        match older {
            MemoryAccessKind::Store { .. } => match o3_store_load_relation(older, younger_range)? {
                O3StoreLoadRelation::Forwarded(plan) => {
                    Some(O3ScalarMemoryWindowAdmission::Forwarded(plan))
                }
                O3StoreLoadRelation::Overlay(plan) => {
                    Some(O3ScalarMemoryWindowAdmission::Overlay(plan))
                }
                O3StoreLoadRelation::Independent(_) => {
                    Some(O3ScalarMemoryWindowAdmission::Independent)
                }
            },
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn forwarded_scalar_load_data(
        &self,
        instruction: RiscvInstruction,
        access: &MemoryAccessKind,
    ) -> Option<Vec<u8>> {
        if !matches!(access, MemoryAccessKind::Load { .. }) {
            return None;
        }
        let range = o3_load_forwarding_access(access)?.range();
        let O3ScalarMemoryWindowAdmission::Forwarded(plan) =
            self.scalar_memory_window_admission(instruction, range)?
        else {
            return None;
        };
        Some(plan.data())
    }

    pub(crate) fn scalar_load_forwarding_plan(
        &self,
        instruction: RiscvInstruction,
        access: &MemoryAccessKind,
    ) -> Option<O3StoreLoadForwardingPlan> {
        if !matches!(access, MemoryAccessKind::Load { .. }) {
            return None;
        }
        let range = o3_load_forwarding_access(access)?.range();
        match self.scalar_memory_window_admission(instruction, range)? {
            O3ScalarMemoryWindowAdmission::Forwarded(plan)
            | O3ScalarMemoryWindowAdmission::Overlay(plan) => Some(plan),
            O3ScalarMemoryWindowAdmission::Independent => None,
        }
    }

    pub(crate) fn can_stage_scalar_memory(&self, execution: &RiscvCpuExecutionEvent) -> bool {
        let Some(access @ MemoryAccessKind::Load { .. }) = execution.execution().memory_access()
        else {
            return false;
        };
        o3_load_forwarding_access(access).is_some_and(|load| {
            self.can_defer_scalar_memory_instruction(execution.instruction(), load.range())
        })
    }
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
        runtime.set_scalar_memory_window_limit(3);
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
    fn configured_four_load_window_stages_four_rows_and_rejects_a_fifth() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let loads = [
            scalar_load_event(0x8000, 10, 12, 10, 0x9000),
            scalar_load_event(0x8004, 11, 13, 10, 0x9040),
            scalar_load_event(0x8008, 12, 14, 10, 0x9080),
            scalar_load_event(0x800c, 13, 15, 10, 0x90c0),
            scalar_load_event(0x8010, 14, 16, 10, 0x9100),
        ];

        for (index, load) in loads[..4].iter().enumerate() {
            assert!(runtime.stage_live_scalar_memory_issue(
                load,
                memory_request(20 + index as u64),
                31 + index as u64,
            ));
        }
        assert!(!runtime.stage_live_scalar_memory_issue(&loads[4], memory_request(24), 35,));

        assert_eq!(runtime.live_scalar_memories.len(), 4);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 4);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 4);
    }

    #[test]
    fn third_scalar_load_waits_for_middle_address_dependency() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(3);
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
    fn fourth_scalar_load_waits_for_any_older_address_dependency() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let older = scalar_load_event(0x8000, 10, 12, 10, 0x9000);
        let middle = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let third = scalar_load_event(0x8008, 12, 14, 10, 0x9080);
        let dependent = scalar_load_event(0x800c, 13, 15, 13, 0x90c0);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        assert!(runtime.stage_live_scalar_memory_issue(&middle, memory_request(21), 32));
        assert!(runtime.stage_live_scalar_memory_issue(&third, memory_request(22), 33));
        assert!(!runtime.stage_live_scalar_memory_issue(&dependent, memory_request(23), 34));

        assert_eq!(runtime.live_scalar_memories.len(), 3);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 3);
    }

    #[test]
    fn three_scalar_loads_complete_out_of_order_and_retire_oldest_first() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(3);
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
    fn four_scalar_loads_complete_in_reverse_and_retire_oldest_first() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let issued = [
            scalar_load_event(0x8000, 10, 12, 10, 0x9000),
            scalar_load_event(0x8004, 11, 13, 10, 0x9040),
            scalar_load_event(0x8008, 12, 14, 10, 0x9080),
            scalar_load_event(0x800c, 13, 15, 10, 0x90c0),
        ];
        let requests = [
            memory_request(20),
            memory_request(21),
            memory_request(22),
            memory_request(23),
        ];
        for (index, event) in issued.iter().enumerate() {
            assert!(runtime.stage_live_scalar_memory_issue(
                event,
                requests[index],
                31 + index as u64,
            ));
        }

        let mut completed = [
            issued[3].clone(),
            issued[2].clone(),
            issued[1].clone(),
            issued[0].clone(),
        ];
        for event in &mut completed {
            event.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        }
        for (event, request, response_tick, data) in [
            (&completed[0], requests[3], 40, [0x88, 0, 0, 0]),
            (&completed[1], requests[2], 41, [0x77, 0, 0, 0]),
            (&completed[2], requests[1], 42, [0x63, 0, 0, 0]),
        ] {
            assert!(runtime.complete_live_scalar_memory_response(
                event,
                request,
                response_tick,
                response_tick - 30,
                Some(&data),
            ));
            assert!(runtime.take_ready_live_scalar_memory_event().is_none());
        }
        assert!(runtime.complete_live_scalar_memory_response(
            &completed[3],
            requests[0],
            45,
            14,
            Some(&[0x2a, 0, 0, 0]),
        ));

        for expected in [&completed[3], &completed[2], &completed[1], &completed[0]] {
            let retired = runtime
                .take_ready_live_scalar_memory_event()
                .expect("completed scalar load should retire in program order");
            assert_eq!(&retired, expected);
            runtime.record_retired_instruction_with_trace(&retired, true);
        }

        assert!(runtime.scalar_memory_lifecycle_is_quiescent());
        assert!(runtime
            .trace_records()
            .windows(2)
            .all(|pair| pair[0].commit_tick() <= pair[1].commit_tick()));
    }

    #[test]
    fn middle_scalar_load_failure_discards_only_the_younger_suffix() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(3);
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
    fn third_of_four_scalar_loads_failure_discards_only_the_fourth_suffix() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let loads = [
            scalar_load_event(0x8000, 10, 12, 10, 0x9000),
            scalar_load_event(0x8004, 11, 13, 10, 0x9040),
            scalar_load_event(0x8008, 12, 14, 10, 0x9080),
            scalar_load_event(0x800c, 13, 15, 10, 0x90c0),
        ];
        for (index, load) in loads.iter().enumerate() {
            assert!(runtime.stage_live_scalar_memory_issue(
                load,
                memory_request(20 + index as u64),
                31 + index as u64,
            ));
        }
        let mut failed = loads[2].clone();
        failed.set_data_access_event_kind(RiscvDataAccessEventKind::Failed);

        assert!(runtime.complete_live_scalar_memory_response(
            &failed,
            memory_request(22),
            40,
            7,
            None,
        ));

        assert_eq!(runtime.live_scalar_memories.len(), 3);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    }

    #[test]
    fn third_load_failure_discards_already_completed_fourth_response() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let loads = [
            scalar_load_event(0x8000, 10, 12, 10, 0x9000),
            scalar_load_event(0x8004, 11, 13, 10, 0x9040),
            scalar_load_event(0x8008, 12, 14, 10, 0x9080),
            scalar_load_event(0x800c, 13, 15, 10, 0x90c0),
        ];
        for (index, load) in loads.iter().enumerate() {
            assert!(runtime.stage_live_scalar_memory_issue(
                load,
                memory_request(20 + index as u64),
                31 + index as u64,
            ));
        }
        let mut completed_fourth = loads[3].clone();
        completed_fourth.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &completed_fourth,
            memory_request(23),
            40,
            7,
            Some(&[0x88, 0, 0, 0]),
        ));
        let mut failed_third = loads[2].clone();
        failed_third.set_data_access_event_kind(RiscvDataAccessEventKind::Failed);

        assert!(runtime.complete_live_scalar_memory_response(
            &failed_third,
            memory_request(22),
            41,
            8,
            None,
        ));

        assert_eq!(runtime.live_scalar_memories.len(), 3);
        assert!(runtime
            .live_scalar_memories
            .iter()
            .all(|live| live.fetch_request != loads[3].fetch().request_id()));
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    }

    #[test]
    fn store_load_pair_does_not_expand_to_a_third_memory_row() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let store = scalar_store_event(0x8000, 10, 0x9000);
        let load = scalar_load_event(0x8004, 11, 13, 10, 0x9040);
        let third = scalar_load_event(0x8008, 12, 14, 10, 0x9080);

        assert!(runtime.stage_live_scalar_memory_issue(&store, memory_request(20), 31));
        assert!(runtime.stage_live_scalar_memory_issue(&load, memory_request(21), 32));
        assert!(!runtime.can_consider_scalar_memory_younger());
        assert!(!runtime.stage_live_scalar_memory_issue(&third, memory_request(22), 33));
        assert_eq!(runtime.live_scalar_memories.len(), 2);
    }

    #[test]
    fn configured_depth_one_serializes_store_load_pair() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(1);
        let store = scalar_store_event(0x8000, 10, 0x9000);
        let load = scalar_load_event(0x8004, 11, 13, 10, 0x9040);

        assert!(runtime.stage_live_scalar_memory_issue(&store, memory_request(20), 31));
        assert!(!runtime.stage_live_scalar_memory_issue(&load, memory_request(21), 32));
        assert_eq!(runtime.live_scalar_memories.len(), 1);
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
    fn word_store_forwards_fully_contained_byte_and_half_loads() {
        for (address, width, expected) in [
            (0x9001, MemoryWidth::Byte, vec![0x80]),
            (0x9000, MemoryWidth::Halfword, vec![0xff, 0x80]),
            (0x9002, MemoryWidth::Halfword, vec![0x7f, 0x00]),
        ] {
            let mut runtime = O3RuntimeState::default();
            let older = scalar_store_event_with_width_and_value(
                0x8000,
                10,
                0x9000,
                MemoryWidth::Word,
                0x007f_80ff,
            );
            let younger = scalar_load_event_with_width(0x8004, 11, 13, 10, address, width);

            assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
            assert_eq!(
                runtime.forwarded_scalar_load_data(
                    younger.instruction(),
                    younger.execution().memory_access().unwrap(),
                ),
                Some(expected),
                "contained load at {address:#x} with {width:?} should receive selected store bytes"
            );
            assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));
            assert_eq!(runtime.live_scalar_memories.len(), 2);
        }
    }

    #[test]
    fn store_then_partially_overlapping_load_stages_transport_backed_second_row() {
        for (younger_address, forwarded_bytes) in [(0x8fff, 3), (0x9002, 2), (0x9003, 1)] {
            let mut runtime = O3RuntimeState::default();
            let older = scalar_store_event(0x8000, 10, 0x9000);
            let younger = scalar_load_event(0x8004, 11, 13, 10, younger_address);

            assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
            let access = younger.execution().memory_access().unwrap();
            assert_eq!(
                runtime
                    .scalar_load_forwarding_plan(younger.instruction(), access)
                    .map(O3StoreLoadForwardingPlan::forwarded_bytes),
                Some(forwarded_bytes),
                "younger load at {younger_address:#x} should retain the overlapping store bytes"
            );
            assert_eq!(
                runtime.forwarded_scalar_load_data(younger.instruction(), access),
                None,
                "partial forwarding still requires a transport response"
            );
            assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));
            assert_eq!(runtime.live_scalar_memories.len(), 2);
        }
    }

    #[test]
    fn byte_store_then_same_address_word_load_stages_partial_forwarding_pair() {
        let mut runtime = O3RuntimeState::default();
        let older =
            scalar_store_event_with_width_and_value(0x8000, 10, 0x9000, MemoryWidth::Byte, 0x2a);
        let younger = scalar_load_event_with_width(0x8004, 11, 13, 10, 0x9000, MemoryWidth::Word);

        assert!(runtime.stage_live_scalar_memory_issue(&older, memory_request(20), 31));
        let access = younger.execution().memory_access().unwrap();
        assert_eq!(
            runtime
                .scalar_load_forwarding_plan(younger.instruction(), access)
                .map(O3StoreLoadForwardingPlan::forwarded_bytes),
            Some(1)
        );
        assert_eq!(
            runtime.forwarded_scalar_load_data(younger.instruction(), access),
            None
        );
        assert!(runtime.stage_live_scalar_memory_issue(&younger, memory_request(21), 32));
        assert_eq!(runtime.live_scalar_memories.len(), 2);
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
        scalar_store_event_with_width_and_value(pc, sequence, address, MemoryWidth::Word, 0x2a)
    }

    fn scalar_store_event_with_width_and_value(
        pc: u64,
        sequence: u64,
        address: u64,
        width: MemoryWidth,
        value: u64,
    ) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Store {
            rs1: reg(10),
            rs2: reg(11),
            offset: Immediate::new(0),
            width,
        };
        let access = MemoryAccessKind::Store {
            address,
            width,
            value,
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
