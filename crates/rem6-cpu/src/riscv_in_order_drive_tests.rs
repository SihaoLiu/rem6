use super::*;
use crate::{
    AccessSize, AgentId, CacheLineLayout, CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord,
    CpuId, CpuResetState, InOrderPipelineConfig, InOrderPipelineError, InOrderPipelineInstruction,
    InOrderPipelineSnapshot, InOrderPipelineStageWidth, MemoryRequestId, MemoryRouteId,
    TransportEndpointId,
};
use rem6_isa_riscv::RiscvInstruction;
use rem6_kernel::{PartitionId, PartitionedScheduler};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn uniform_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}

fn core_with_completed_fetch() -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                MemoryRequestId::new(AgentId::new(7), 0),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0013u32.to_le_bytes().to_vec(),
        ));
    core
}

#[test]
fn enqueue_fetch_never_advances_time_and_rejects_full_fetch1() {
    let mut pipeline = crate::InOrderPipelineState::new(uniform_pipeline_config(1));

    pipeline.enqueue_fetch(0).unwrap();
    assert_eq!(pipeline.cycle(), 0);
    assert_eq!(
        pipeline.in_flight(),
        &[InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1
        )]
    );
    assert_eq!(
        pipeline.enqueue_fetch(1),
        Err(InOrderPipelineError::StageAtCapacity {
            stage: InOrderPipelineStage::Fetch1,
            width: 1,
        })
    );
    assert_eq!(pipeline.cycle(), 0);

    pipeline.enqueue_fetch(0).unwrap();
    assert_eq!(pipeline.in_flight().len(), 1);
}

#[test]
fn fetch_admission_distinguishes_available_advance_and_retire_states() {
    let core = core_with_completed_fetch();
    core.reset_in_order_pipeline_config(uniform_pipeline_config(1));

    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::Admitted
    );

    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        uniform_pipeline_config(1),
        0,
        [InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1,
        )],
    ))
    .unwrap();
    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::AdvanceBeforeFetch
    );

    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        uniform_pipeline_config(1),
        0,
        [
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit),
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
        ],
    ))
    .unwrap();
    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::RetireBeforeFetch
    );
}

#[test]
fn pending_pipeline_wake_blocks_fetch_admission() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));

    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::PipelineCyclePending
    );
}

#[test]
fn scheduled_pipeline_wake_is_checkpoint_owned_until_delivery() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    let RiscvInOrderDriveStatus::Scheduled(event) = core
        .schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
        .unwrap()
    else {
        panic!("completed fetch should schedule a pipeline wake");
    };
    let wake = scheduler.pending_event_snapshot(event).unwrap();
    assert_eq!(
        core.checkpoint_owned_in_order_pipeline_wakes(),
        vec![(scheduler.instance_id(), wake)]
    );

    scheduler.run_until_idle();

    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
}

#[test]
fn reset_detaches_pipeline_wake_until_stale_delivery() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    let RiscvInOrderDriveStatus::Scheduled(event) = core
        .schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
        .unwrap()
    else {
        panic!("completed fetch should schedule a pipeline wake");
    };
    let wake = scheduler.pending_event_snapshot(event).unwrap();

    core.reset_instruction_fetch_stream(scheduler.now());

    assert_eq!(
        core.checkpoint_owned_in_order_pipeline_wakes(),
        vec![(scheduler.instance_id(), wake)]
    );
    scheduler.run_until_idle();
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
}

#[test]
fn fetch_stream_reset_preserves_retired_writeback_slot_occupancy() {
    let core = core_with_completed_fetch();
    core.reserve_test_fixed_fu_writeback(40, 20).unwrap();

    core.reset_instruction_fetch_stream(20);
    core.reserve_test_fixed_fu_writeback(41, 20).unwrap();

    let reservations = core.o3_runtime_writeback_reservations();
    assert_eq!(reservations.len(), 2);
    assert_eq!(reservations[0].sequence(), 40);
    assert_eq!(reservations[0].admitted_tick(), 20);
    assert_eq!(reservations[1].sequence(), 41);
    assert_eq!(reservations[1].admitted_tick(), 21);
}

#[test]
fn snapshot_restore_retains_pipeline_wake_ownership_until_stale_delivery() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    let RiscvInOrderDriveStatus::Scheduled(event) = core
        .schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
        .unwrap()
    else {
        panic!("completed fetch should schedule a pipeline wake");
    };
    let wake = scheduler.pending_event_snapshot(event).unwrap();

    core.restore_in_order_pipeline_snapshot(RiscvCore::default_in_order_pipeline_snapshot())
        .unwrap();

    assert_eq!(
        core.checkpoint_owned_in_order_pipeline_wakes(),
        vec![(scheduler.instance_id(), wake)]
    );
    scheduler.run_until_idle();
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
}

#[test]
fn restored_execute_wait_rebind_resets_latency_for_changed_instruction() {
    let core = core_with_completed_fetch();
    let config = RiscvCore::default_in_order_pipeline_snapshot()
        .config()
        .clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        4,
        [
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Execute)
                .with_execute_wait(19, 18),
        ],
    ))
    .unwrap();
    let replacement = CpuFetchEvent::completed(
        CpuFetchRecord::new(
            5,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            MemoryRequestId::new(AgentId::new(7), 1),
            Address::new(0x8000),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013u32.to_le_bytes().to_vec(),
    );
    core.core.state.lock().expect("cpu core lock").events = vec![replacement];
    core.sync_in_order_fetch_state().unwrap();

    let rebound = core.in_order_pipeline_snapshot();
    assert_eq!(rebound.in_flight()[0].sequence(), 1);
    assert_eq!(
        rebound.in_flight()[0].execute_wait_remaining_cycles(),
        Some(18)
    );

    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));
    scheduler.run_until_idle();

    let advanced = core.in_order_pipeline_snapshot();
    assert_eq!(
        advanced.in_flight()[0].stage(),
        InOrderPipelineStage::Commit
    );
    assert_eq!(
        advanced.in_flight()[0].execute_wait_remaining_cycles(),
        None
    );
    assert_eq!(
        core.in_order_pipeline_cycle_records()
            .last()
            .unwrap()
            .stall_cause(),
        None
    );
}

#[test]
fn restored_completed_execute_wait_rebind_resets_changed_latency_before_commit() {
    let core = core_with_completed_fetch();
    let config = RiscvCore::default_in_order_pipeline_snapshot()
        .config()
        .clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        4,
        [
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit)
                .with_execute_wait(19, 0),
        ],
    ))
    .unwrap();
    let mul: u32 = (1 << 25) | (2 << 20) | (1 << 15) | (3 << 7) | 0x33;
    let replacement = CpuFetchEvent::completed(
        CpuFetchRecord::new(
            5,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            MemoryRequestId::new(AgentId::new(7), 1),
            Address::new(0x8000),
            AccessSize::new(4).unwrap(),
        ),
        mul.to_le_bytes().to_vec(),
    );
    core.core.state.lock().expect("cpu core lock").events = vec![replacement];
    core.sync_in_order_fetch_state().unwrap();

    let rebound = core.in_order_pipeline_snapshot();
    assert_eq!(rebound.in_flight()[0].sequence(), 1);
    assert_eq!(rebound.in_flight()[0].stage(), InOrderPipelineStage::Commit);
    assert_eq!(
        rebound.in_flight()[0].execute_wait_remaining_cycles(),
        Some(0)
    );

    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));
    scheduler.run_until_idle();

    let reset = core.in_order_pipeline_snapshot();
    assert_eq!(reset.in_flight()[0].stage(), InOrderPipelineStage::Execute);
    assert_eq!(reset.in_flight()[0].execute_wait_total_cycles(), Some(2));
    assert_eq!(
        reset.in_flight()[0].execute_wait_remaining_cycles(),
        Some(1)
    );
    assert_eq!(
        core.in_order_pipeline_cycle_records()
            .last()
            .unwrap()
            .stall_cause(),
        Some(crate::InOrderPipelineStallCause::ExecuteWait)
    );
}

#[test]
fn keyed_execute_wait_restarts_equal_latency_changed_instruction() {
    let scalar_div: u32 = (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    let vector_div: u32 =
        (0b100000 << 26) | (1 << 25) | (2 << 20) | (1 << 15) | (0b010 << 12) | (3 << 7) | 0x57;

    for replacement_sequence in [0, 1] {
        let core = core_with_completed_fetch();
        let config = RiscvCore::default_in_order_pipeline_snapshot()
            .config()
            .clone();
        core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
            config,
            4,
            [
                InOrderPipelineInstruction::new(0, InOrderPipelineStage::Execute)
                    .with_execute_wait_key(19, 1, u64::from(scalar_div) + 1),
            ],
        ))
        .unwrap();
        let replacement = CpuFetchEvent::completed(
            CpuFetchRecord::new(
                5,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                MemoryRequestId::new(AgentId::new(7), replacement_sequence),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            ),
            vector_div.to_le_bytes().to_vec(),
        );
        core.core.state.lock().expect("cpu core lock").events = vec![replacement];
        core.sync_in_order_fetch_state().unwrap();

        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        assert!(matches!(
            core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
                .unwrap(),
            RiscvInOrderDriveStatus::Scheduled(_)
        ));
        scheduler.run_until_idle();

        let rebound = core.in_order_pipeline_snapshot();
        assert_eq!(rebound.in_flight()[0].execute_wait_total_cycles(), Some(19));
        assert_eq!(
            rebound.in_flight()[0].execute_wait_remaining_cycles(),
            Some(18)
        );
        assert_eq!(
            rebound.in_flight()[0].execute_wait_key(),
            Some(u64::from(vector_div) + 1)
        );
    }
}

#[test]
fn keyed_head_execute_wait_rebind_preserves_progress_with_orphaned_younger_row() {
    let core = core_with_completed_fetch();
    let scalar_div: u32 = (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    let config = RiscvCore::default_in_order_pipeline_snapshot()
        .config()
        .clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        4,
        [
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Execute)
                .with_execute_wait_key(19, 5, u64::from(scalar_div) + 1),
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Decode),
        ],
    ))
    .unwrap();
    let replacement = CpuFetchEvent::completed(
        CpuFetchRecord::new(
            5,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            MemoryRequestId::new(AgentId::new(7), 1),
            Address::new(0x8000),
            AccessSize::new(4).unwrap(),
        ),
        scalar_div.to_le_bytes().to_vec(),
    );
    core.core.state.lock().expect("cpu core lock").events = vec![replacement];
    core.sync_in_order_fetch_state().unwrap();

    let rebound = core.in_order_pipeline_snapshot();
    assert_eq!(
        rebound
            .in_flight()
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        rebound.in_flight()[0].execute_wait_remaining_cycles(),
        Some(5)
    );

    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));
    scheduler.run_until_idle();
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight()[0].execute_wait_remaining_cycles(),
        Some(4)
    );
}

#[test]
fn detailed_rebound_completed_scalar_wait_defers_vector_and_float_latency_to_live_gate() {
    let float_div: u32 = (0x0c << 25) | (2 << 20) | (1 << 15) | (3 << 7) | 0x53;
    let vector_div: u32 =
        (0b100000 << 26) | (1 << 25) | (2 << 20) | (1 << 15) | (0b010 << 12) | (3 << 7) | 0x57;

    for replacement_raw in [float_div, vector_div] {
        let core = core_with_completed_fetch();
        let config = RiscvCore::default_in_order_pipeline_snapshot()
            .config()
            .clone();
        core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
            config,
            4,
            [
                InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit)
                    .with_execute_wait(19, 0),
            ],
        ))
        .unwrap();
        let replacement = CpuFetchEvent::completed(
            CpuFetchRecord::new(
                5,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                MemoryRequestId::new(AgentId::new(7), 1),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            ),
            replacement_raw.to_le_bytes().to_vec(),
        );
        core.core.state.lock().expect("cpu core lock").events = vec![replacement];
        core.set_detailed_live_retire_gate_enabled(true);
        core.sync_in_order_fetch_state().unwrap();

        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        assert_eq!(
            core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
                .unwrap(),
            RiscvInOrderDriveStatus::Ready
        );
        let rebound = core.in_order_pipeline_snapshot();
        assert_eq!(rebound.in_flight()[0].stage(), InOrderPipelineStage::Commit);
        assert_eq!(rebound.in_flight()[0].execute_wait_total_cycles(), None);

        assert_eq!(
            core.drive_next_completed_fetch_serial_action(&mut scheduler)
                .unwrap(),
            None
        );
        assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
        assert_eq!(core.checkpoint_owned_live_retire_gate_wakes().len(), 1);
    }
}

#[test]
fn normal_pipeline_scheduler_owns_float_and_vector_execute_waits() {
    let float_div: u32 = (0x0c << 25) | (2 << 20) | (1 << 15) | (3 << 7) | 0x53;
    let vector_div: u32 =
        (0b100000 << 26) | (1 << 25) | (2 << 20) | (1 << 15) | (0b010 << 12) | (3 << 7) | 0x57;

    for (replacement_raw, expected_wait_cycles) in [(float_div, 11), (vector_div, 19)] {
        let core = core_with_completed_fetch();
        let replacement = CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                MemoryRequestId::new(AgentId::new(7), 0),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            ),
            replacement_raw.to_le_bytes().to_vec(),
        );
        core.core.state.lock().expect("cpu core lock").events = vec![replacement];
        let config = RiscvCore::default_in_order_pipeline_snapshot()
            .config()
            .clone();
        core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
            config,
            4,
            [InOrderPipelineInstruction::new(
                0,
                InOrderPipelineStage::Execute,
            )],
        ))
        .unwrap();

        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        assert!(matches!(
            core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
                .unwrap(),
            RiscvInOrderDriveStatus::Scheduled(_)
        ));
        scheduler.run_until_idle();

        let waiting = core.in_order_pipeline_snapshot();
        assert_eq!(
            waiting.in_flight()[0].stage(),
            InOrderPipelineStage::Execute
        );
        assert_eq!(
            waiting.in_flight()[0].execute_wait_total_cycles(),
            Some(expected_wait_cycles)
        );
        assert_eq!(
            waiting.in_flight()[0].execute_wait_remaining_cycles(),
            Some(expected_wait_cycles - 1)
        );
        assert_eq!(
            core.in_order_pipeline_cycle_records()
                .last()
                .unwrap()
                .stall_cause(),
            Some(crate::InOrderPipelineStallCause::ExecuteWait)
        );
        let checkpoint = crate::InOrderPipelineCheckpointPayload::from_snapshot(waiting.clone())
            .unwrap()
            .encode();
        assert_eq!(checkpoint[4], 3);
        let restored = crate::InOrderPipelineCheckpointPayload::decode(&checkpoint)
            .unwrap()
            .into_snapshot();
        assert_eq!(
            restored.in_flight()[0].execute_wait_key(),
            waiting.in_flight()[0].execute_wait_key()
        );
        let mut malformed = checkpoint;
        let wait_offset = malformed.len() - (1 + 8 + 8 + 8);
        malformed[wait_offset] = 0;
        malformed[wait_offset + 1..wait_offset + 17].fill(0);
        assert!(matches!(
            crate::InOrderPipelineCheckpointPayload::decode(&malformed),
            Err(crate::InOrderPipelineError::InvalidCheckpointExecuteWait {
                code: 0,
                total_cycles: 0,
                remaining_cycles: 0,
            })
        ));
    }
}

#[test]
fn detailed_mode_switch_drains_active_normal_execute_wait_once() {
    let core = core_with_completed_fetch();
    let div: u32 = (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    let replacement = CpuFetchEvent::completed(
        CpuFetchRecord::new(
            0,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            MemoryRequestId::new(AgentId::new(7), 0),
            Address::new(0x8000),
            AccessSize::new(4).unwrap(),
        ),
        div.to_le_bytes().to_vec(),
    );
    core.core.state.lock().expect("cpu core lock").events = vec![replacement];
    let config = RiscvCore::default_in_order_pipeline_snapshot()
        .config()
        .clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        4,
        [
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Execute)
                .with_execute_wait(19, 1),
        ],
    ))
    .unwrap();

    core.set_detailed_live_retire_gate_enabled(true);
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight()[0].execute_wait_remaining_cycles(),
        Some(1)
    );

    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));
    scheduler.run_until_idle();
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight()[0].execute_wait_remaining_cycles(),
        Some(0)
    );

    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));
    scheduler.run_until_idle();
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight()[0].stage(),
        InOrderPipelineStage::Commit
    );

    assert!(matches!(
        core.drive_next_completed_fetch_serial_action(&mut scheduler)
            .unwrap(),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(core.checkpoint_owned_live_retire_gate_wakes().is_empty());
}

#[test]
fn live_detailed_gate_bypasses_normal_pipeline_scheduler() {
    let core = core_with_completed_fetch();
    let request = MemoryRequestId::new(AgentId::new(7), 0);
    let div = (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    core.set_detailed_live_retire_gate_enabled(true);
    assert!(matches!(
        core.state
            .lock()
            .expect("riscv core lock")
            .live_retire_gate
            .before_retire(request, div, 0, 0)
            .unwrap(),
        crate::riscv_live_retire_gate::RiscvLiveRetireGateDecision::Schedule { .. }
    ));
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    let action = core
        .drive_next_completed_fetch_serial_action(&mut scheduler)
        .unwrap();

    assert_eq!(action, None);
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
    assert_eq!(core.checkpoint_owned_live_retire_gate_wakes().len(), 1);
}

#[test]
fn inherited_o3_window_suppresses_normal_pipeline_after_mode_disable() {
    let core = core_with_completed_fetch();
    let instruction = RiscvInstruction::decode(0x0000_0013).unwrap();
    core.state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .stage_live_retire_window(Address::new(0x8000), instruction, 0, []);

    assert!(core.o3_retirement_suppresses_normal_pipeline());
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.drive_next_completed_fetch_serial_action(&mut scheduler)
            .unwrap(),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
}
