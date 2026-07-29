use super::*;

macro_rules! checkpoint_pair {
    ($core:expr) => {
        (
            $core.core.checkpoint_state(),
            $core.state.lock().expect("riscv core lock").clone(),
        )
    };
}

#[test]
fn pending_load_graph_rejects_materialized_transport_and_bad_stable_owner() {
    assert_materialized_runtime_graph_rejected_atomically();
    assert_graph_prepare_rejected_atomically(|_stable, live| {
        live.events
            .push(transport_pending_event(&live.pending_addresses[0]));
    });
    assert_graph_prepare_rejected_atomically(|_stable, live| {
        for row in &mut live.pending_addresses {
            row.root_atomic = true;
        }
    });
    assert_graph_prepare_rejected_atomically(|stable, live| {
        let rows = live.pending_addresses.as_slice();
        let lsq = stable
            .snapshot()
            .load_store_queue()
            .iter()
            .map(|entry| {
                if entry.sequence() == rows[1].sequence {
                    O3LoadStoreQueueEntry::load(
                        entry.sequence(),
                        Some(Address::new(ROOT_VALUE)),
                        entry.bytes(),
                    )
                } else {
                    *entry
                }
            })
            .collect();
        *stable = super::super::compute::rebuilt_stable(stable, None, Some(lsq), None).unwrap();
    });
    assert_graph_prepare_rejected_atomically(|stable, live| {
        let bad_sequence = live.pending_addresses[2].sequence;
        let rob = stable
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| {
                if entry.sequence() == bad_sequence {
                    O3ReorderBufferEntry::new(entry.sequence(), entry.pc(), entry.destination())
                        .with_ready(entry.is_ready())
                        .with_ready_tick(entry.ready_tick())
                        .with_live_staged_rename_for_checkpoint(O3RegisterClass::Integer, 9)
                } else {
                    *entry
                }
            })
            .collect();
        *stable = super::super::compute::rebuilt_stable(stable, Some(rob), None, None).unwrap();
    });
}

fn assert_materialized_runtime_graph_rejected_atomically() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let projection = fixture.capture();
    let live = captured_graph(&projection).clone();
    let materialized_stable = materialized_stable_payload(&fixture, &live.pending_addresses[0]);

    let before = checkpoint_pair!(fixture.core);
    assert!(matches!(
        fixture.capture().live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
    assert_eq!(checkpoint_pair!(fixture.core), before);

    let destination = graph_core(fixture.issue_width);
    let before = checkpoint_pair!(destination);
    assert!(destination
        .prepare_checkpoint_restore(checkpoint_input_with_replay_hart(
            &projection,
            &fixture.core,
            materialized_stable,
            live,
        ))
        .is_err());
    assert_eq!(checkpoint_pair!(destination), before);
}

fn materialized_stable_payload(
    fixture: &PendingLoadGraphCheckpointFixture,
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> O3RuntimeCheckpointPayload {
    let mut state = fixture.core.state.lock().expect("riscv core lock");
    state
        .o3_runtime
        .set_pending_data_address_materialized_for_fetch_for_test(
            pending.fetch.request_id(),
            CAPTURED_TICK,
            materialized_pending_execution(pending, ROOT_VALUE),
        );
    state.o3_runtime.bind_oldest_pending_data_address_for_test(
        request(30),
        Address::new(ROOT_VALUE),
        CAPTURED_TICK,
    );
    assert_eq!(state.o3_runtime.pending_data_address_count(), 2);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 1);
    state.o3_runtime.checkpoint_payload()
}

fn materialized_pending_execution(
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let destination = pending.destination.unwrap();
    let raw: [u8; 4] = pending.fetch.data().unwrap().try_into().unwrap();
    let decoded = RiscvInstruction::decode_with_length(u32::from_le_bytes(raw)).unwrap();
    RiscvCpuExecutionEvent::new(
        pending.fetch.clone(),
        decoded.instruction(),
        RiscvExecutionRecord::new(
            decoded.instruction(),
            pending.fetch.pc().get(),
            pending.fetch.pc().get() + 4,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(destination.architectural() as u8),
                address,
                width: MemoryWidth::Doubleword,
                signed: false,
            }),
        ),
    )
}

fn transport_pending_event(
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> RiscvO3LiveCheckpointEvent {
    let materialized = materialized_pending_execution(pending, ROOT_VALUE);
    RiscvO3LiveCheckpointEvent {
        fetch: materialized.fetch().clone(),
        execution_pc: materialized.execution().pc(),
        next_pc: materialized.execution().next_pc(),
        instruction_bytes: 4,
        register_writes: Vec::new(),
        float_register_writes: Vec::new(),
        memory_access: materialized.execution().memory_access().cloned(),
        data_access_event_kind: None,
        counts_as_retired_instruction: true,
    }
}

fn assert_graph_prepare_rejected_atomically(
    mutation: impl FnOnce(&mut O3RuntimeCheckpointPayload, &mut RiscvO3LiveCheckpointPayload),
) {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let projection = fixture.capture();
    let mut stable = projection.stable().clone();
    let mut live = captured_graph(&projection).clone();
    mutation(&mut stable, &mut live);
    let destination = graph_core(fixture.issue_width);
    destination.write_register(reg(9), 0xfeed_face);
    destination.inner().set_pc(Address::new(0xb000));
    let before = checkpoint_pair!(destination);

    assert!(destination
        .prepare_checkpoint_restore(checkpoint_input_with_replay_hart(
            &projection,
            &fixture.core,
            stable,
            live,
        ))
        .is_err());

    assert_eq!(checkpoint_pair!(destination), before);
}
