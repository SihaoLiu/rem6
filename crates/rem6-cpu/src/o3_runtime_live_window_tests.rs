use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord,
    RiscvTrap, RiscvTrapKind,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, AgentId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::super::o3_runtime_issue_tests::{bind_o3, decoded};
use super::*;
use crate::{CpuFetchEvent, CpuFetchRecord};

#[path = "o3_runtime_live_window_identity_tests.rs"]
mod identity;

fn div_x3() -> RiscvInstruction {
    RiscvInstruction::Div {
        rd: Register::new(3).unwrap(),
        rs1: Register::new(1).unwrap(),
        rs2: Register::new(2).unwrap(),
    }
}

fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: Register::new(rd).unwrap(),
        rs1: Register::new(rs1).unwrap(),
        imm: Immediate::new(1),
    }
}

fn add(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Add {
        rd: Register::new(rd).unwrap(),
        rs1: Register::new(rs1).unwrap(),
        rs2: Register::new(rs2).unwrap(),
    }
}

fn load_x4() -> RiscvInstruction {
    RiscvInstruction::Load {
        rd: Register::new(4).unwrap(),
        rs1: Register::new(10).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    }
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

#[test]
fn scalar_memory_stops_live_retire_window_before_memory_and_younger_rows() {
    let mut runtime = O3RuntimeState::default();

    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        [
            (Address::new(0x8004), load_x4()),
            (Address::new(0x8008), addi(5, 4)),
        ],
    );

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(
        runtime.snapshot().reorder_buffer()[0].pc(),
        Address::new(0x8000)
    );
    assert_eq!(integer_mapping(&runtime, 4), None);
    assert_eq!(integer_mapping(&runtime, 5), None);
}

#[test]
fn scalar_load_window_allows_one_independent_younger_issue_candidate() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    let independent = addi(5, 0);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), independent)],
    );

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), independent)
        .is_some());
}

#[test]
fn scalar_load_window_blocks_younger_load_destination_consumer() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    let dependent = addi(5, 4);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), dependent)],
    );

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .is_none());
}

#[test]
fn scalar_load_head_stages_three_younger_scalar_alus() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let first = addi(5, 0);
    let second = addi(6, 5);
    let third = add(7, 5, 6);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), first),
            (Address::new(0x8008), second),
            (Address::new(0x800c), third),
        ],
    );

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    assert_eq!(runtime.live_data_access_younger_sequences.len(), 3);
    assert_eq!(
        runtime.snapshot().reorder_buffer()[3].pc(),
        Address::new(0x800c)
    );
}

#[test]
fn scalar_load_backedge_stages_only_the_unique_prefix() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let repeated = addi(5, 0);
    let branch = RiscvInstruction::Beq {
        rs1: Register::new(1).unwrap(),
        rs2: Register::new(2).unwrap(),
        offset: Immediate::new(-4),
    };
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    let staged = runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), repeated),
            (Address::new(0x8008), branch),
            (Address::new(0x8004), repeated),
        ],
    );

    assert_eq!(staged, 2);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
}

#[test]
fn scalar_load_head_stages_terminal_branch_without_rename_or_younger_rows() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = RiscvInstruction::decode(0x00b2_0463).unwrap();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), addi(5, 0)),
            (Address::new(0x8008), addi(6, 5)),
            (Address::new(0x800c), branch),
            (Address::new(0x8010), addi(7, 0)),
        ],
    );

    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    assert_eq!(
        rob.iter().map(|entry| entry.pc()).collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x8008, 0x800c].map(Address::new)
    );
    let branch_row = rob[3];
    assert_eq!(
        (branch_row.destination(), branch_row.rename_destination()),
        (None, None)
    );
    assert!(branch_row.is_live_staged() && !branch_row.is_ready());
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x800c), branch)
        .is_none());
}

#[test]
fn scalar_load_head_younger_alus_wake_transitively_with_fan_in() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let first = addi(5, 0);
    let second = addi(6, 5);
    let third = add(7, 5, 6);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), first),
            (Address::new(0x8008), second),
            (Address::new(0x800c), third),
        ],
    );
    let first_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), first)
        .unwrap();
    bind_o3(&mut runtime, 0x8004, decoded(first), &[request(11)]);
    runtime
        .record_live_speculative_execution(
            first_candidate,
            &[request(11)],
            10,
            RiscvExecutionRecord::new(
                first,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(5).unwrap(), 5)],
                None,
            ),
        )
        .unwrap();
    let second_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), second)
        .expect("the first younger ALU should wake the second");
    assert_eq!(second_candidate.issue_tick(10), 10);
    bind_o3(&mut runtime, 0x8008, decoded(second), &[request(12)]);
    runtime
        .record_live_speculative_execution(
            second_candidate,
            &[request(12)],
            10,
            RiscvExecutionRecord::new(
                second,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(Register::new(6).unwrap(), 16)],
                None,
            ),
        )
        .unwrap();
    let third_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), third)
        .expect("both younger ALU producers should wake the fan-in row");
    assert_eq!(
        third_candidate.forwarded_register_writes(),
        &[
            RegisterWrite::new(Register::new(5).unwrap(), 5),
            RegisterWrite::new(Register::new(6).unwrap(), 16),
        ]
    );
    assert_eq!(third_candidate.issue_tick(10), 11);
}

#[test]
fn scalar_load_head_dependency_row_remains_blocked_before_load_writeback() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let independent = addi(5, 0);
    let dependent = addi(6, 4);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), independent),
            (Address::new(0x8008), dependent),
        ],
    );

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), independent)
        .is_some());
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8008), dependent)
        .is_none());
}

#[test]
fn completed_live_load_forwards_into_dependent_alu_candidate() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    let dependent = addi(5, 4);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), dependent)],
    );
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .is_none());

    let mut completed = load.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            41,
            10,
            Some(&0x2a_u32.to_le_bytes()),
        )
        .unwrap());

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .is_none());
    assert!(runtime.take_ready_live_data_access_event(41).is_none());
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .is_none());
    assert!(runtime.take_ready_live_data_access_event(42).is_some());

    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .expect("admitted load should wake its dependent scalar ALU");
    assert_eq!(
        candidate.forwarded_register_writes(),
        &[RegisterWrite::new(Register::new(4).unwrap(), 0x2a)]
    );
    assert_eq!(candidate.issue_tick(10), 42);
}

#[test]
fn scalar_memory_prefix_stages_load_dependent_terminal_alu() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(3);
    let store = scalar_store_event();
    let load = scalar_load_event();
    let dependent = addi(5, 4);
    assert!(runtime.stage_live_data_access_issue_for_test(&store, request(19), 30));
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));

    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), dependent)],
    );

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    assert_eq!(runtime.live_data_access_younger_sequences.len(), 1);
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .is_none());
}

#[test]
fn scalar_load_head_discard_removes_every_younger_row() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), addi(5, 0)),
            (Address::new(0x8008), addi(6, 5)),
            (Address::new(0x800c), add(7, 5, 6)),
        ],
    );

    runtime.discard_live_staged_instructions();

    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.live_data_access_younger_sequences.is_empty());
    assert!(runtime.live_speculative_executions.is_empty());
}

fn retire_live(runtime: &mut O3RuntimeState, execution: &RiscvCpuExecutionEvent, retire_tick: u64) {
    let consumed_requests = [execution.fetch().request_id()];
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(execution.execution().pc()),
        decoded(execution.instruction()),
        &consumed_requests,
    ));
    runtime.retire_live_staged_instruction(execution, &consumed_requests, retire_tick);
}

#[test]
fn live_rename_overlay_rolls_back_to_committed_mapping_on_discard() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        3,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;

    runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);

    assert_eq!(
        runtime.snapshot().rename_map()[0].physical(),
        O3PhysicalRegisterId::new(41)
    );
    runtime.discard_live_retire_window();
    assert_eq!(
        runtime.snapshot().rename_map()[0].physical(),
        O3PhysicalRegisterId::new(40)
    );
}

#[test]
fn live_staged_rob_checkpoint_reconstructs_rename_overlay() {
    let mut runtime = O3RuntimeState::default();
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        [
            (Address::new(0x8004), addi(4, 0)),
            (Address::new(0x8008), addi(5, 4)),
            (Address::new(0x800c), add(6, 4, 5)),
        ],
    );
    let live_snapshot = runtime.snapshot();
    let encoded = runtime.checkpoint_payload().encode();

    let mut restored = O3RuntimeState::default();
    restored
        .restore_checkpoint_payload(O3RuntimeCheckpointPayload::decode(&encoded).unwrap())
        .unwrap();

    assert_eq!(restored.snapshot(), live_snapshot);
    assert!(restored.snapshot().reorder_buffer()[0].is_live_staged());
    assert!(restored.snapshot().reorder_buffer()[1].is_live_staged());
    assert!(restored.snapshot().reorder_buffer()[2].is_live_staged());
    assert!(restored.snapshot().reorder_buffer()[3].is_live_staged());

    let restored_destinations = restored
        .snapshot()
        .reorder_buffer()
        .iter()
        .filter_map(|entry| entry.destination())
        .collect::<BTreeSet<_>>();
    restored.stage_live_retire_window(Address::new(0x8010), addi(7, 0), 0, None);
    let next_destination = restored
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(0x8010))
        .and_then(|entry| entry.destination())
        .unwrap();
    assert!(
        !restored_destinations.contains(&next_destination),
        "post-restore allocation must not reuse a live staged physical register"
    );
}

#[test]
fn independent_live_staged_scalar_execution_keeps_early_issue_timing() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();
    let packet = decoded(younger_instruction);
    bind_o3(&mut runtime, 0x8004, packet, &[request(2)]);
    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    retire_live(&mut runtime, &divide, 29);
    runtime.record_retired_instruction_with_trace(&divide, true);
    let younger = execution_event(younger_instruction, 0x8004, 2, 4);
    retire_live(&mut runtime, &younger, 30);
    runtime.record_retired_instruction_with_trace(&younger, true);
    let trace = runtime.trace_records().last().copied().unwrap();
    assert_eq!(trace.issue_tick(), 10);
    assert_eq!(trace.writeback_tick(), 10);
    assert_eq!(trace.admitted_writeback_tick(), Some(10));
    assert_eq!(trace.fu_latency_cycles(), 0);
    assert_eq!(trace.commit_tick(), 29);
}

#[test]
fn matching_split_fetch_identity_keeps_early_issue_timing() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();
    let requests = [request(2), request(3)];
    let packet = decoded(younger_instruction);
    bind_o3(&mut runtime, 0x8004, packet, &requests);
    runtime
        .record_live_speculative_execution(
            candidate,
            &requests,
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    retire_live(&mut runtime, &divide, 29);
    runtime.record_retired_instruction_with_trace(&divide, true);
    let younger = execution_event(younger_instruction, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&younger, &requests, 30);
    runtime.record_retired_instruction_with_trace(&younger, true);
    let trace = runtime.trace_records().last().copied().unwrap();
    assert_eq!(trace.issue_tick(), 10);
    assert_eq!(trace.writeback_tick(), 10);
    assert_eq!(trace.admitted_writeback_tick(), Some(10));
    assert_eq!(trace.fu_latency_cycles(), 0);
    assert_eq!(trace.commit_tick(), 29);
}

#[test]
fn dependent_live_staged_scalar_execution_cannot_issue_early() {
    let mut runtime = O3RuntimeState::default();
    let dependent = addi(4, 3);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), dependent)),
    );

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), dependent)
        .is_none());
    runtime.snapshot.reorder_buffer[0].mark_ready_at(20);
    assert!(
        runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none(),
        "a ready-but-uncommitted producer is still absent from the architectural hart clone"
    );
}

#[test]
fn repeated_live_window_staging_extends_past_existing_rows() {
    let mut runtime = O3RuntimeState::default();
    let head = div_x3();
    let first = addi(4, 0);
    let second = addi(5, 4);
    let third = addi(6, 5);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        head,
        29,
        Some((Address::new(0x8004), first)),
    );

    runtime.stage_live_retire_window(
        Address::new(0x8000),
        head,
        29,
        [
            (Address::new(0x8004), first),
            (Address::new(0x8008), second),
            (Address::new(0x800c), third),
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
fn speculative_predecessor_wakes_and_forwards_to_dependent_younger() {
    let mut runtime = O3RuntimeState::default();
    let producer = addi(4, 0);
    let consumer = addi(5, 4);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), producer)),
    );
    runtime.stage_live_retire_window(Address::new(0x8008), consumer, 0, None);
    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .unwrap();
    bind_o3(&mut runtime, 0x8004, decoded(producer), &[request(2)]);
    runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    let consumer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), consumer)
        .expect("speculative x4 write should wake the x5 consumer");

    assert_eq!(
        consumer_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(Register::new(4).unwrap(), 1)]
    );
    assert_eq!(consumer_candidate.issue_tick(10), 10);
}

#[test]
fn speculative_scalar_chain_wakes_transitively_with_fan_in() {
    let mut runtime = O3RuntimeState::default();
    let first = addi(4, 0);
    let second = addi(5, 4);
    let third = add(6, 4, 5);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        [
            (Address::new(0x8004), first),
            (Address::new(0x8008), second),
            (Address::new(0x800c), third),
        ],
    );

    let first_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), first)
        .unwrap();
    bind_o3(&mut runtime, 0x8004, decoded(first), &[request(2)]);
    runtime
        .record_live_speculative_execution(
            first_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                first,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 5)],
                None,
            ),
        )
        .unwrap();
    let second_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), second)
        .unwrap();
    assert_eq!(second_candidate.issue_tick(10), 10);
    bind_o3(&mut runtime, 0x8008, decoded(second), &[request(3)]);
    runtime
        .record_live_speculative_execution(
            second_candidate,
            &[request(3)],
            10,
            RiscvExecutionRecord::new(
                second,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(Register::new(5).unwrap(), 16)],
                None,
            ),
        )
        .unwrap();

    let third_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), third)
        .expect("both speculative producers should wake the fan-in consumer");

    assert_eq!(
        third_candidate.forwarded_register_writes(),
        &[
            RegisterWrite::new(Register::new(4).unwrap(), 5),
            RegisterWrite::new(Register::new(5).unwrap(), 16),
        ]
    );
    assert_eq!(third_candidate.issue_tick(10), 11);
}

#[test]
fn invalidated_first_speculative_row_discards_two_downstream_rows() {
    let mut runtime = O3RuntimeState::default();
    let first = addi(4, 0);
    let second = addi(5, 4);
    let third = add(6, 4, 5);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        [
            (Address::new(0x8004), first),
            (Address::new(0x8008), second),
            (Address::new(0x800c), third),
        ],
    );
    for (pc, instruction, request_id, destination, value) in [
        (0x8004, first, 2, 4, 5),
        (0x8008, second, 3, 5, 16),
        (0x800c, third, 4, 6, 21),
    ] {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap();
        let fetch_request = request(request_id);
        bind_o3(&mut runtime, pc, decoded(instruction), &[fetch_request]);
        runtime
            .record_live_speculative_execution(
                candidate,
                &[fetch_request],
                10,
                RiscvExecutionRecord::new(
                    instruction,
                    pc,
                    pc + 4,
                    vec![RegisterWrite::new(
                        Register::new(destination).unwrap(),
                        value,
                    )],
                    None,
                ),
            )
            .unwrap();
    }
    assert_eq!(runtime.live_speculative_executions.len(), 3);
    let mismatched = execution_event(first, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&mismatched, &[request(2)], 20);

    assert!(runtime.live_speculative_executions.is_empty());
}

#[test]
fn speculative_consumer_uses_nearest_live_producer() {
    let mut runtime = O3RuntimeState::default();
    let producer = addi(3, 0);
    let consumer = addi(5, 3);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), producer)),
    );
    runtime.stage_live_retire_window(Address::new(0x8008), consumer, 0, None);
    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .unwrap();
    bind_o3(&mut runtime, 0x8004, decoded(producer), &[request(2)]);
    runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(3).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();

    let consumer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), consumer)
        .expect("the nearer x3 writer should hide the older pending divide destination");

    assert_eq!(
        consumer_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(Register::new(3).unwrap(), 1)]
    );
    assert_eq!(consumer_candidate.issue_tick(10), 10);
}

#[test]
fn invalidated_speculative_producer_revokes_dependent_issue_timing() {
    let mut runtime = O3RuntimeState::default();
    let producer = addi(4, 0);
    let consumer = addi(5, 4);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        [
            (Address::new(0x8004), producer),
            (Address::new(0x8008), consumer),
        ],
    );
    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .unwrap();
    bind_o3(&mut runtime, 0x8004, decoded(producer), &[request(2)]);
    runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    let consumer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), consumer)
        .unwrap();
    bind_o3(&mut runtime, 0x8008, decoded(consumer), &[request(3)]);
    runtime
        .record_live_speculative_execution(
            consumer_candidate,
            &[request(3)],
            10,
            RiscvExecutionRecord::new(
                consumer,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(Register::new(5).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();

    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    retire_live(&mut runtime, &divide, 29);
    runtime.record_retired_instruction_with_trace(&divide, true);
    let producer = execution_event(producer, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&producer, &[request(4)], 30);
    runtime.record_retired_instruction_with_trace(&producer, true);
    let consumer = execution_event(consumer, 0x8008, 3, 5);
    runtime.retire_live_staged_instruction(&consumer, &[request(3)], 31);
    runtime.record_retired_instruction_with_trace(&consumer, true);

    let trace = runtime.trace_records().last().copied().unwrap();
    assert_eq!(trace.issue_tick(), 31);
    assert_eq!(trace.commit_tick(), 31);
}

#[test]
fn public_snapshot_checkpoint_preserves_committed_rename_rollback() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        3,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;
    runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);

    let payload = O3RuntimeCheckpointPayload::from_snapshot(runtime.snapshot()).unwrap();
    let mut restored = O3RuntimeState::default();
    restored.restore_checkpoint_payload(payload).unwrap();
    assert_eq!(
        integer_mapping(&restored, 3),
        Some(O3PhysicalRegisterId::new(41))
    );

    restored.discard_live_retire_window();
    assert_eq!(
        integer_mapping(&restored, 3),
        Some(O3PhysicalRegisterId::new(40))
    );
}

#[test]
fn trapping_live_staged_instruction_does_not_publish_rename() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        3,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;
    runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);
    let event = RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        div_x3(),
        RiscvExecutionRecord::with_trap(
            div_x3(),
            0x8000,
            0x9000,
            RiscvTrap::new(RiscvTrapKind::Interrupt { code: 1 }, 0x8000),
        ),
    );

    retire_live(&mut runtime, &event, 29);
    runtime.discard_live_staged_instructions();
    runtime.record_retired_instruction(&event);

    assert_eq!(
        integer_mapping(&runtime, 3),
        Some(O3PhysicalRegisterId::new(40))
    );
}

#[test]
fn stats_reset_preserves_pending_live_dependency_producer_identity() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        1,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;
    let first_instruction = addi(2, 1);
    let first = execution_event(first_instruction, 0x8000, 1, 2);
    runtime.stage_live_retire_window(Address::new(0x8000), first_instruction, 0, None);
    retire_live(&mut runtime, &first, 10);

    runtime.reset_stats();
    runtime.record_retired_instruction(&first);
    let second_instruction = addi(3, 1);
    runtime.record_retired_instruction(&execution_event(second_instruction, 0x8004, 2, 3));

    assert_eq!(runtime.stats().iew_producer_insts(), 1);
    assert_eq!(runtime.stats().iew_consumer_insts(), 2);
}

#[test]
fn stats_reset_rebases_pending_consumer_onto_previously_seen_producer() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        1,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;
    let prior_instruction = addi(2, 1);
    runtime.record_retired_instruction(&execution_event(prior_instruction, 0x7ffc, 0, 2));
    let pending_instruction = addi(3, 1);
    let pending = execution_event(pending_instruction, 0x8000, 1, 3);
    runtime.stage_live_retire_window(Address::new(0x8000), pending_instruction, 0, None);
    retire_live(&mut runtime, &pending, 10);

    runtime.reset_stats();

    assert_eq!(runtime.stats().iew_producer_insts(), 1);
    assert_eq!(runtime.stats().iew_consumer_insts(), 1);
}

#[test]
fn snapshot_without_live_rename_overlay_equals_public_reconstruction() {
    let runtime = O3RuntimeState::default();

    assert_eq!(runtime.snapshot(), default_o3_runtime_snapshot());
}

#[test]
fn live_rename_overlay_preserves_canonical_register_order() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        10,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;
    runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);

    assert_eq!(
        runtime
            .snapshot()
            .rename_map()
            .iter()
            .map(|entry| entry.architectural())
            .collect::<Vec<_>>(),
        vec![3, 10]
    );
}

fn execution_event(
    instruction: RiscvInstruction,
    pc: u64,
    sequence: u64,
    destination: u8,
) -> RiscvCpuExecutionEvent {
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            vec![RegisterWrite::new(Register::new(destination).unwrap(), 1)],
            None,
        ),
    )
}

fn scalar_load_event() -> RiscvCpuExecutionEvent {
    let instruction = load_x4();
    RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8000,
            0x8004,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: Register::new(4).unwrap(),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            }),
        ),
    )
}

fn scalar_store_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Store {
        rs1: Register::new(10).unwrap(),
        rs2: Register::new(11).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(0x7ffc, 9),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x7ffc,
            0x8000,
            Vec::new(),
            Some(MemoryAccessKind::Store {
                address: 0x9000,
                width: MemoryWidth::Word,
                value: 0x2a,
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
            MemoryRequestId::new(AgentId::new(7), sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn integer_mapping(runtime: &O3RuntimeState, architectural: u32) -> Option<O3PhysicalRegisterId> {
    runtime
        .snapshot()
        .rename_map()
        .iter()
        .find(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == architectural
        })
        .map(|entry| entry.physical())
}
