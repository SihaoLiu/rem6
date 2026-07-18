use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord,
    RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::*;
use crate::{CpuFetchEvent, CpuFetchRecord, RiscvCpuExecutionEvent};

#[path = "o3_runtime_control_window_tests/coroutine.rs"]
mod coroutine;
#[path = "o3_runtime_control_window_tests/lifecycle.rs"]
mod lifecycle;
#[path = "o3_runtime_control_window_tests/same_link.rs"]
mod same_link;
#[path = "o3_runtime_control_window_tests/same_link_return.rs"]
mod same_link_return;
#[path = "o3_runtime_control_window_tests/same_link_scalar_return.rs"]
mod same_link_scalar_return;
#[path = "o3_runtime_control_window_tests/same_link_validation.rs"]
mod same_link_validation;

#[test]
fn predicted_control_branch_candidate_has_no_destination_and_keeps_issue_tick() {
    let mut runtime = scalar_load_runtime_with_branch(beq(5, 6));
    let branch = beq(5, 6);
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .expect("independent branch should issue while the load is outstanding");
    assert_eq!(candidate.destination(), None);
    assert_eq!(candidate.issue_tick(11), 11);

    let execution = RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None);
    runtime
        .record_live_speculative_execution(candidate, &[request(11)], 11, execution.clone())
        .unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(0x8004, 11), branch, execution),
        &[request(11)],
        40,
    );

    let retired = runtime
        .take_live_retired_instruction(request(11))
        .expect("retired branch should retain its early issue record");
    assert_eq!(retired.issue_tick, 11);
}

#[test]
fn unresolved_load_source_rejects_predicted_control_branch_candidate() {
    let runtime = scalar_load_runtime_with_branch(beq(4, 0));

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), beq(4, 0))
        .is_none());
}

#[test]
fn scalar_load_stages_predicted_branch_and_two_descendants() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x8008, 0x800c].map(Address::new)
    );
}

#[test]
fn blocked_younger_fu_and_branch_trace_commit_in_program_order_after_load() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let first = addi(5, 0, 1);
    let second = addi(6, 0, 2);
    let branch = beq(0, 0);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), first),
                (Address::new(0x8008), second),
                (Address::new(0x800c), branch),
            ],
        ),
        3
    );

    for (pc, next_pc, instruction, sequence, value) in [
        (0x8004, 0x8008, first, 11, Some((5, 1))),
        (0x8008, 0x800c, second, 12, Some((6, 2))),
        (0x800c, 0x8014, branch, 13, None),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .expect("independent younger row should issue while the load is pending");
        let writes = value
            .map(|(register, value)| vec![RegisterWrite::new(reg(register), value)])
            .unwrap_or_default();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(sequence)],
                20 + sequence,
                RiscvExecutionRecord::new(instruction, pc, next_pc, writes, None),
            )
            .unwrap();
    }
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());

    let mut retired_younger = Vec::new();
    for (pc, next_pc, instruction, sequence, value, retire_tick) in [
        (0x8004, 0x8008, first, 11, Some((5, 1)), 40),
        (0x8008, 0x800c, second, 12, Some((6, 2)), 41),
    ] {
        let writes = value
            .map(|(register, value)| vec![RegisterWrite::new(reg(register), value)])
            .unwrap_or_default();
        let event = RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, next_pc, writes, None),
        );
        assert_eq!(event.execution().pc(), pc);
        runtime.retire_live_staged_instruction(&event, &[request(sequence)], retire_tick);
        assert!(runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .any(|entry| entry.pc() == Address::new(0x8000)));
        retired_younger.push(event);
    }

    let mut completed_load = load.clone();
    completed_load.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert_eq!(runtime.live_data_accesses.len(), 1);
    let load_sequence = runtime.live_data_accesses[0].sequence;
    assert!(runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .any(|entry| entry.sequence() == load_sequence));
    assert!(runtime
        .snapshot()
        .load_store_queue()
        .iter()
        .any(|entry| entry.sequence() == load_sequence));
    assert!(runtime
        .complete_live_data_access_response(
            &completed_load,
            request(20),
            50,
            19,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    let retired_load = runtime
        .take_ready_live_data_access_event(u64::MAX)
        .expect("completed load should retire");
    runtime.record_retired_instruction_with_trace(&retired_load, true);

    let branch_event = RiscvCpuExecutionEvent::new(
        fetch_event(0x800c, 13),
        branch,
        RiscvExecutionRecord::new(branch, 0x800c, 0x8014, Vec::new(), None),
    );
    runtime.retire_live_staged_instruction(&branch_event, &[request(13)], 53);
    for event in &retired_younger {
        runtime.record_retired_instruction_with_trace(event, true);
    }
    runtime.record_retired_instruction_with_trace(&branch_event, true);

    let ordered = [0x8000, 0x8004, 0x8008, 0x800c].map(|pc| {
        runtime
            .trace_records()
            .iter()
            .copied()
            .find(|record| record.pc() == Address::new(pc))
            .unwrap_or_else(|| panic!("missing trace row for {pc:#x}"))
    });
    assert!(ordered
        .iter()
        .all(|record| record.commit_tick() >= record.writeback_tick()));
    assert!(
        ordered
            .windows(2)
            .all(|records| records[0].commit_tick() <= records[1].commit_tick()),
        "commit ticks: {:?}",
        ordered.map(O3RuntimeTraceRecord::commit_tick)
    );
}

#[test]
fn blocked_prefix_dependency_chain_uses_preceding_staged_rename_across_stats_reset() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let committed_x5 = O3PhysicalRegisterId::new(40);
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        5,
        committed_x5,
    ));
    runtime.next_physical_register = 41;

    let load = scalar_load_event();
    let producer = addi(5, 5, 1);
    let consumer = addi(6, 5, 1);
    let younger_writer = addi(5, 0, 2);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), producer),
                (Address::new(0x8008), consumer),
                (Address::new(0x800c), younger_writer),
            ],
        ),
        3
    );

    let staged = runtime.snapshot().reorder_buffer().to_vec();
    let producer_destination = staged_rename_entry(staged[1])
        .expect("producer staged destination")
        .physical();
    let younger_destination = staged_rename_entry(staged[3])
        .expect("younger staged destination")
        .physical();
    assert_ne!(producer_destination, committed_x5);
    assert_ne!(producer_destination, younger_destination);

    for (pc, next_pc, instruction, sequence, write) in [
        (0x8004, 0x8008, producer, 11, RegisterWrite::new(reg(5), 1)),
        (0x8008, 0x800c, consumer, 12, RegisterWrite::new(reg(6), 2)),
        (
            0x800c,
            0x8010,
            younger_writer,
            13,
            RegisterWrite::new(reg(5), 2),
        ),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .expect("staged dependency chain should issue before the load completes");
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(sequence)],
                20 + sequence,
                RiscvExecutionRecord::new(instruction, pc, next_pc, vec![write], None),
            )
            .unwrap();
    }

    let producer_event = RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 11),
        producer,
        RiscvExecutionRecord::new(
            producer,
            0x8004,
            0x8008,
            vec![RegisterWrite::new(reg(5), 1)],
            None,
        ),
    );
    runtime.retire_live_staged_instruction(&producer_event, &[request(11)], 40);
    let retired_producer = runtime
        .live_retired_instructions
        .iter()
        .find(|instruction| instruction.request == request(11))
        .expect("blocked producer retirement record");
    assert_eq!(
        retired_producer.iew_dependency_producer_registers,
        [committed_x5]
    );
    assert_eq!(retired_producer.iew_dependency_producers, 1);
    assert_eq!(retired_producer.iew_dependency_consumers, 1);
    assert_eq!(runtime.stats().iew_producer_insts(), 1);
    assert_eq!(runtime.stats().iew_consumer_insts(), 1);

    runtime.reset_stats();
    assert_eq!(runtime.stats().iew_producer_insts(), 1);
    assert_eq!(runtime.stats().iew_consumer_insts(), 1);

    let consumer_event = RiscvCpuExecutionEvent::new(
        fetch_event(0x8008, 12),
        consumer,
        RiscvExecutionRecord::new(
            consumer,
            0x8008,
            0x800c,
            vec![RegisterWrite::new(reg(6), 2)],
            None,
        ),
    );
    runtime.retire_live_staged_instruction(&consumer_event, &[request(12)], 41);
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
    assert!(runtime.snapshot().reorder_buffer()[1].is_ready());
    assert!(runtime.snapshot().reorder_buffer()[2].is_ready());
    let retired_consumer = runtime
        .live_retired_instructions
        .iter()
        .find(|instruction| instruction.request == request(12))
        .expect("blocked consumer retirement record");
    assert_eq!(
        retired_consumer.iew_dependency_producer_registers,
        [producer_destination]
    );
    assert_eq!(retired_consumer.iew_dependency_producers, 1);
    assert_eq!(retired_consumer.iew_dependency_consumers, 1);
    assert_eq!(runtime.stats().iew_producer_insts(), 2);
    assert_eq!(runtime.stats().iew_consumer_insts(), 2);

    runtime.reset_stats();
    assert_eq!(runtime.stats().iew_producer_insts(), 2);
    assert_eq!(runtime.stats().iew_consumer_insts(), 2);
    assert_eq!(
        runtime.dependency_producers_with_consumers,
        BTreeSet::from([committed_x5, producer_destination])
    );
}

#[test]
fn nested_control_dependencies_follow_immediate_branch() {
    let (runtime, _, _, _) = nested_control_runtime();
    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    let outer = rob[1].sequence();
    let inner = rob[2].sequence();
    let descendant = rob[3].sequence();

    assert_eq!(runtime.live_control_dependencies.get(&inner), Some(&outer));
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant),
        Some(&inner)
    );
}

#[test]
fn three_deep_control_dependencies_follow_immediate_branch() {
    let (runtime, _, _, _) = three_deep_control_runtime();
    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    let outer = rob[1].sequence();
    let middle = rob[2].sequence();
    let inner = rob[3].sequence();

    assert_eq!(runtime.live_control_dependencies.get(&middle), Some(&outer));
    assert_eq!(runtime.live_control_dependencies.get(&inner), Some(&middle));
    assert_eq!(runtime.live_control_window_sequences.len(), 3);
}

#[test]
fn mixed_control_dependencies_follow_immediate_control() {
    let (runtime, _, _, _) = mixed_control_runtime();
    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    assert_eq!(rob.len(), 4, "mixed controls must occupy the bounded ROB");
    let direct_jump = rob[1].sequence();
    let conditional = rob[2].sequence();
    let indirect_jump = rob[3].sequence();

    assert_eq!(
        runtime.live_control_dependencies.get(&conditional),
        Some(&direct_jump)
    );
    assert_eq!(
        runtime.live_control_dependencies.get(&indirect_jump),
        Some(&conditional)
    );
    assert_eq!(runtime.live_control_window_sequences.len(), 3);
}

#[test]
fn mixed_no_link_control_candidates_have_no_destination() {
    let (runtime, direct_jump, conditional, indirect_jump) = mixed_control_runtime();

    for (pc, instruction) in [
        (0x8004, direct_jump),
        (0x8008, conditional),
        (0x800c, indirect_jump),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap_or_else(|| panic!("missing mixed control candidate at {pc:#x}"));
        assert_eq!(candidate.destination(), None);
    }
}

#[test]
fn linked_control_candidates_use_staged_rename_destinations() {
    for (instruction, pc, architectural) in [
        (jal_link(1, 4), 0x8004, 1),
        (jal_link(5, 4), 0x8004, 5),
        (jalr_link(1, 2), 0x8004, 1),
        (jalr_link(5, 2), 0x8004, 5),
    ] {
        let runtime = scalar_load_runtime_with_branch(instruction);
        let call_row = runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .copied()
            .find(|entry| entry.pc() == Address::new(pc))
            .expect("linked control ROB row");
        let staged = staged_rename_entry(call_row).expect("linked control staged destination");

        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap_or_else(|| panic!("missing linked control candidate for {instruction:?}"));

        assert_eq!(staged.register_class(), O3RegisterClass::Integer);
        assert_eq!(staged.architectural(), architectural);
        assert_eq!(staged.physical(), call_row.destination().unwrap());
        assert_eq!(candidate.destination(), Some(staged));
    }
}

#[test]
fn scalar_and_linked_control_candidates_expose_destinations() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let call = jal_link(1, 4);
    let scalar = addi(8, 0, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), call), (Address::new(0x8008), scalar)],
    );

    let call_destination = staged_rename_entry(runtime.snapshot().reorder_buffer()[1])
        .expect("call staged destination");
    let scalar_destination = staged_rename_entry(runtime.snapshot().reorder_buffer()[2])
        .expect("scalar staged destination");

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("linked control candidate");
    let scalar_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), scalar)
        .expect("scalar descendant candidate");

    assert_eq!(call_candidate.destination(), Some(call_destination));
    assert_eq!(scalar_candidate.destination(), Some(scalar_destination));
}

#[test]
fn same_window_return_candidate_uses_link_call_forwarding() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let call = jal_link(1, 8);
    let return_jump = jalr_return(1);
    let descendant = addi(8, 0, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), return_jump),
                (Address::new(0x8008), descendant),
            ],
        ),
        3
    );
    let return_sequence = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(0x800c))
        .expect("same-window return row")
        .sequence();
    assert_eq!(
        runtime.live_serializing_control_sequences,
        BTreeSet::from([return_sequence])
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("linked call candidate");
    let call_sequence = call_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                call,
                0x8004,
                0x800c,
                vec![RegisterWrite::new(reg(1), 0x8008)],
                None,
            ),
        )
        .unwrap());

    let return_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), return_jump)
        .expect("same-window return candidate");
    assert!(return_candidate
        .producer_sequences()
        .contains(&call_sequence));
    assert_eq!(
        return_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x8008)]
    );
}

#[test]
fn linked_control_candidate_rejects_mismatched_staged_destination() {
    let mut runtime = scalar_load_runtime_with_branch(jal_link(1, 4));
    let call_row = runtime.snapshot.reorder_buffer[1];
    runtime.snapshot.reorder_buffer[1] =
        O3ReorderBufferEntry::new(call_row.sequence(), call_row.pc(), call_row.destination())
            .with_live_staged_rename_destination(Some((O3RegisterClass::Integer, 5)));

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), jal_link(1, 4))
        .is_none());
}

#[test]
fn linked_control_execution_requires_exact_link_write_and_reserves_writeback() {
    let mut runtime = scalar_load_runtime_with_branch(jal_link(1, 4));
    assert!(runtime.set_writeback_width(1));
    let sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), jal_link(1, 4))
        .expect("linked control candidate");
    let destination = candidate
        .destination()
        .expect("linked control candidate destination");

    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                jal_link(1, 4),
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(1), 0x8008)],
                None,
            ),
        )
        .unwrap());

    let issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == sequence)
        .expect("recorded linked control execution");
    assert_eq!(destination.architectural(), 1);
    assert_eq!(issued.raw_ready_tick, issued.admitted_writeback_tick);
    assert_eq!(issued.writeback_slot, Some(0));
    assert_eq!(
        runtime
            .writeback_reservation(sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(issued.admitted_writeback_tick)
    );
}

#[test]
fn linked_control_execution_rejects_missing_wrong_or_extra_link_writes() {
    for writes in [
        Vec::new(),
        vec![RegisterWrite::new(reg(5), 0x8008)],
        vec![
            RegisterWrite::new(reg(1), 0x8008),
            RegisterWrite::new(reg(5), 0x8008),
        ],
    ] {
        assert_invalid_linked_control_writes_do_not_leak_state(writes);
    }
}

#[test]
fn no_link_control_execution_rejects_integer_writes_and_uses_no_writeback_slot() {
    let mut runtime = scalar_load_runtime_with_branch(jal(4));
    let sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), jal(4))
        .expect("no-link control candidate");

    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(jal(4), 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap());
    let issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == sequence)
        .expect("recorded no-link control execution");
    assert_eq!(issued.writeback_slot, None);
    assert!(runtime.writeback_reservation(sequence).is_none());

    let mut runtime = scalar_load_runtime_with_branch(jal(4));
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), jal(4))
        .expect("no-link control candidate");
    assert!(!runtime
        .record_live_speculative_execution(
            candidate,
            &[request(12)],
            20,
            RiscvExecutionRecord::new(
                jal(4),
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(1), 0x8008)],
                None,
            ),
        )
        .unwrap());
}

#[test]
fn predicted_mul_wakes_dependent_add_candidate() {
    let mut runtime = O3RuntimeState::default();
    let head = addi(3, 0, 1);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        head,
        0,
        [
            (Address::new(0x8004), multiply),
            (Address::new(0x8008), dependent),
        ],
    );

    let multiply_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), multiply)
        .expect("predicted MUL should be an issue candidate");
    let multiply_execution = RiscvExecutionRecord::new(
        multiply,
        0x8004,
        0x8008,
        vec![RegisterWrite::new(reg(7), 42)],
        None,
    );
    runtime
        .record_live_speculative_execution(
            multiply_candidate,
            &[request(11)],
            12,
            multiply_execution.clone(),
        )
        .unwrap();
    assert_eq!(
        runtime.live_speculative_execution_ready_tick(&[request(11)], &multiply_execution),
        Some(14)
    );

    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), dependent)
        .expect("MUL result should wake the dependent scalar descendant");
    assert_eq!(
        dependent_candidate.issue_tick(12),
        12 + crate::riscv_fu_latency::riscv_execute_wait_cycles(multiply)
    );
    assert_eq!(
        dependent_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(7), 42)]
    );
}

#[test]
fn discarding_control_descendants_removes_younger_rename_state() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );
    let branch_sequence = runtime.snapshot().reorder_buffer()[1].sequence();

    let branch_execution = RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None);
    let branch_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            branch_candidate,
            &[request(11)],
            11,
            branch_execution.clone(),
        )
        .unwrap();
    let multiply_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), multiply)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            multiply_candidate,
            &[request(12)],
            12,
            RiscvExecutionRecord::new(
                multiply,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(7), 42)],
                None,
            ),
        )
        .unwrap();
    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), dependent)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            dependent_candidate,
            &[request(13)],
            14,
            RiscvExecutionRecord::new(
                dependent,
                0x800c,
                0x8010,
                vec![RegisterWrite::new(reg(8), 43)],
                None,
            ),
        )
        .unwrap();
    assert_eq!(runtime.live_speculative_executions.len(), 3);

    runtime.discard_live_control_descendants_from_at(branch_sequence, 0);

    let snapshot = runtime.snapshot();
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [Address::new(0x8000), Address::new(0x8004)]
    );
    assert!(snapshot
        .rename_map()
        .iter()
        .all(|entry| !matches!(entry.architectural(), 7 | 8)));
    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(
        runtime.live_speculative_executions[0].sequence,
        branch_sequence
    );
}

#[test]
fn discarding_older_branch_removes_linked_call_descendant_state() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let call = jal_link(1, 4);
    let descendant = addi(8, 1, 1);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), call),
            (Address::new(0x800c), descendant),
        ],
    );
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let branch_sequence = rob[1].sequence();
    let call_sequence = rob[2].sequence();
    let descendant_sequence = rob[3].sequence();

    let branch_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .expect("older branch candidate");
    assert!(runtime
        .record_live_speculative_execution(
            branch_candidate,
            &[request(11)],
            11,
            RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap());
    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), call)
        .expect("linked call descendant candidate");
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(12)],
            12,
            RiscvExecutionRecord::new(
                call,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(1), 0x800c)],
                None,
            ),
        )
        .unwrap());
    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), descendant)
        .expect("scalar descendant of linked call");
    assert!(runtime
        .record_live_speculative_execution(
            descendant_candidate,
            &[request(13)],
            13,
            RiscvExecutionRecord::new(
                descendant,
                0x800c,
                0x8010,
                vec![RegisterWrite::new(reg(8), 0x800d)],
                None,
            ),
        )
        .unwrap());
    assert!(runtime.writeback_reservation(call_sequence).is_some());
    assert!(runtime
        .snapshot()
        .rename_map()
        .iter()
        .any(|entry| entry.architectural() == 1));

    runtime.discard_live_control_descendants_from_at(branch_sequence, 0);

    let snapshot = runtime.snapshot();
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .map(|entry| entry.sequence())
            .collect::<Vec<_>>(),
        [rob[0].sequence(), branch_sequence]
    );
    assert!(snapshot
        .reorder_buffer()
        .iter()
        .all(|entry| ![call_sequence, descendant_sequence].contains(&entry.sequence())));
    assert!(runtime.writeback_reservation(call_sequence).is_none());
    assert!(runtime.live_speculative_executions.iter().all(|issued| ![
        call_sequence,
        descendant_sequence
    ]
    .contains(&issued.sequence)));
    assert!(snapshot
        .rename_map()
        .iter()
        .all(|entry| !matches!(entry.architectural(), 1 | 8)));
}

#[test]
fn linked_call_rollback_restores_prior_committed_rename() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let committed_x1 = O3PhysicalRegisterId::new(44);
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        1,
        committed_x1,
    ));
    runtime.next_physical_register = 45;
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let call = jal_link(1, 4);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), branch), (Address::new(0x8008), call)],
    );
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let branch_sequence = rob[1].sequence();
    let call_sequence = rob[2].sequence();
    let call_destination = staged_rename_entry(rob[2]).expect("linked call staged destination");
    assert_ne!(call_destination.physical(), committed_x1);
    assert_eq!(
        runtime
            .snapshot()
            .rename_map()
            .iter()
            .find(|entry| entry.architectural() == 1)
            .map(|entry| entry.physical()),
        Some(call_destination.physical())
    );

    let branch_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .expect("older branch candidate");
    assert!(runtime
        .record_live_speculative_execution(
            branch_candidate,
            &[request(11)],
            11,
            RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap());
    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), call)
        .expect("linked call descendant candidate");
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(12)],
            12,
            RiscvExecutionRecord::new(
                call,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(reg(1), 0x800c)],
                None,
            ),
        )
        .unwrap());
    assert!(runtime.writeback_reservation(call_sequence).is_some());

    runtime.discard_live_control_descendants_from_at(branch_sequence, 0);

    assert!(runtime.writeback_reservation(call_sequence).is_none());
    assert_eq!(
        runtime
            .snapshot()
            .rename_map()
            .iter()
            .find(|entry| entry.architectural() == 1)
            .map(|entry| entry.physical()),
        Some(committed_x1)
    );
}

#[test]
fn staged_window_truncation_prunes_control_dependencies() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), beq(5, 6)),
            (Address::new(0x8008), mul(7, 1, 2)),
            (Address::new(0x800c), addi(8, 7, 1)),
        ],
    );
    assert_eq!(runtime.live_control_dependencies.len(), 2);
    let load_sequence = runtime.snapshot().reorder_buffer()[0].sequence();

    runtime.discard_live_staged_window_from(load_sequence);

    assert!(runtime.live_control_dependencies.is_empty());
    assert!(!runtime.has_live_control_window());
}

fn scalar_load_runtime_with_branch(branch: RiscvInstruction) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), branch)],
    );
    runtime
}

fn nested_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = beq(5, 6);
    let inner = beq(7, 8);
    let descendant = mul(9, 1, 2);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), outer),
            (Address::new(0x8008), inner),
            (Address::new(0x800c), descendant),
        ],
    );
    (runtime, outer, inner, descendant)
}

fn issued_nested_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let (mut runtime, outer, inner, descendant) = nested_control_runtime();
    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            outer_candidate,
            &[request(11)],
            11,
            RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap();

    let inner_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            inner_candidate,
            &[request(12)],
            12,
            RiscvExecutionRecord::new(inner, 0x8008, 0x800c, Vec::new(), None),
        )
        .unwrap();

    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), descendant)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            descendant_candidate,
            &[request(13)],
            13,
            RiscvExecutionRecord::new(
                descendant,
                0x800c,
                0x8010,
                vec![RegisterWrite::new(reg(9), 42)],
                None,
            ),
        )
        .unwrap();
    (runtime, outer, inner, descendant)
}

fn three_deep_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = bne(5, 6);
    let middle = blt(7, 8);
    let inner = bgeu(9, 10);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), outer),
            (Address::new(0x8008), middle),
            (Address::new(0x800c), inner),
        ],
    );
    (runtime, outer, middle, inner)
}

fn issued_three_deep_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let (mut runtime, outer, middle, inner) = three_deep_control_runtime();
    for (pc, next_pc, instruction, sequence) in [
        (0x8004, 0x8008, outer, 11),
        (0x8008, 0x800c, middle, 12),
        (0x800c, 0x8010, inner, 13),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(sequence)],
                sequence,
                RiscvExecutionRecord::new(instruction, pc, next_pc, Vec::new(), None),
            )
            .unwrap();
    }
    (runtime, outer, middle, inner)
}

fn mixed_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let direct_jump = jal(4);
    let conditional = beq(5, 6);
    let indirect_jump = jalr(9);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), direct_jump),
            (Address::new(0x8008), conditional),
            (Address::new(0x800c), indirect_jump),
        ],
    );
    (runtime, direct_jump, conditional, indirect_jump)
}

fn issued_mixed_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let (mut runtime, direct_jump, conditional, indirect_jump) = mixed_control_runtime();
    for (pc, next_pc, instruction, sequence) in [
        (0x8004, 0x8008, direct_jump, 11),
        (0x8008, 0x800c, conditional, 12),
        (0x800c, 0x8010, indirect_jump, 13),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(sequence)],
                sequence,
                RiscvExecutionRecord::new(instruction, pc, next_pc, Vec::new(), None),
            )
            .unwrap();
    }
    (runtime, direct_jump, conditional, indirect_jump)
}

fn scalar_load_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(4),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8000,
            0x8004,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(4),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            }),
        ),
    )
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn addi(rd: u8, rs1: u8, immediate: i64) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(immediate),
    }
}

fn beq(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Beq {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn jal(offset: i64) -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: reg(0),
        offset: Immediate::new(offset),
    }
}

fn jal_link(rd: u8, offset: i64) -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: reg(rd),
        offset: Immediate::new(offset),
    }
}

fn jalr(rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(0),
        rs1: reg(rs1),
        offset: Immediate::new(0),
    }
}

fn jalr_return(rs1: u8) -> RiscvInstruction {
    jalr(rs1)
}

fn jalr_link(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
    }
}

fn bne(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Bne {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn blt(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Blt {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn bgeu(rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Bgeu {
        rs1: reg(rs1),
        rs2: reg(rs2),
        offset: Immediate::new(8),
    }
}

fn mul(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Mul {
        rd: reg(rd),
        rs1: reg(rs1),
        rs2: reg(rs2),
    }
}

fn assert_invalid_linked_control_writes_do_not_leak_state(writes: Vec<RegisterWrite>) {
    let mut runtime = scalar_load_runtime_with_branch(jal_link(1, 4));
    assert!(runtime.set_writeback_width(1));
    let call_sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), jal_link(1, 4))
        .expect("linked control candidate");
    assert!(!runtime
        .record_live_speculative_execution(
            candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(jal_link(1, 4), 0x8004, 0x8008, writes, None),
        )
        .unwrap());
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.writeback_reservation(call_sequence).is_none());
}
