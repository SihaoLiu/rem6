use super::*;
use crate::riscv_cluster_translation::advance_parallel_data_translation;
use crate::{
    o3_runtime::O3LiveRetireGateCheckpointPayload, AccessSize, AgentId, CacheLineLayout, CpuCore,
    CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuResetState, InOrderPipelineInstruction,
    InOrderPipelineSnapshot, InOrderPipelineStage, MemoryRequestId, MemoryRouteId, RiscvCluster,
    TransportEndpointId,
};
use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{Address, MemoryRequest, TranslationPageMap, TranslationPageSize};
use rem6_mmio::MmioBus;
use rem6_transport::{
    MemoryRoute, MemoryTrace, ParallelMemoryTransaction, TargetOutcome, TransportError,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
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

fn unknown_route_transaction() -> ParallelMemoryTransaction {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(9), 0),
        Address::new(0x9000),
        AccessSize::new(4).unwrap(),
        CacheLineLayout::new(16).unwrap(),
    )
    .unwrap();
    ParallelMemoryTransaction::new(
        MemoryRouteId::new(99),
        request,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
        |_delivery| {},
    )
}

#[test]
fn restored_live_gate_awaiting_rebind_admits_only_the_head_replay() {
    let core = core_with_completed_fetch();
    let request = MemoryRequestId::new(AgentId::new(7), 42);
    let div = RiscvInstruction::Div {
        rd: Register::new(3).unwrap(),
        rs1: Register::new(1).unwrap(),
        rs2: Register::new(2).unwrap(),
    };
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .stage_live_retire_window(Address::new(0x8000), div, 31, None)
            .expect("restored fixed-FU head stages");
        state
            .live_retire_gate
            .restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request, 31)));
    }

    assert!(core.o3_retirement_suppresses_normal_pipeline());
    assert!(fetch_before_pipeline_is_admitted(&core, 0));

    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state
                .live_retire_gate
                .before_retire(request, 0x0220_c1b3, 30, 30)
                .unwrap(),
            crate::riscv_live_retire_gate::RiscvLiveRetireGateDecision::Schedule {
                ready_tick: 31,
                created_wait_ticks: None,
            }
        );
    }

    assert!(!fetch_before_pipeline_is_admitted(&core, 0));
}

#[test]
fn source_local_checkpoint_prepare_is_counted_released_and_expires() {
    let core = core_with_completed_fetch();

    core.prepare_source_local_checkpoint_capture(5);
    core.prepare_source_local_checkpoint_capture(5);
    assert!(!fetch_before_pipeline_is_admitted(&core, 5));

    core.release_source_local_checkpoint_capture(5);
    assert!(!fetch_before_pipeline_is_admitted(&core, 5));
    core.release_source_local_checkpoint_capture(5);
    assert!(fetch_before_pipeline_is_admitted(&core, 5));

    core.prepare_source_local_checkpoint_capture(7);
    assert!(!fetch_before_pipeline_is_admitted(&core, 7));
    assert!(fetch_before_pipeline_is_admitted(&core, 8));
}

#[test]
fn source_local_checkpoint_restore_blocks_until_its_deadline() {
    let core = core_with_completed_fetch();

    core.prepare_source_local_checkpoint_restore(5);
    assert!(core.source_local_checkpoint_restore_blocks_drive(4));
    assert!(core.source_local_checkpoint_restore_blocks_drive(5));
    assert!(!core.source_local_checkpoint_restore_blocks_drive(6));

    core.release_source_local_checkpoint_restore(5);
    assert!(!core.source_local_checkpoint_restore_blocks_drive(5));
}

#[test]
fn scheduled_source_local_restore_blocks_new_work_without_freezing_drain() {
    let core = core_with_completed_fetch();

    core.prepare_source_local_checkpoint_restore_after(5, 7);
    assert!(core.source_local_checkpoint_restore_blocks_new_work(5));
    assert!(!core.source_local_checkpoint_restore_blocks_drive(5));
    assert!(core.source_local_checkpoint_restore_blocks_new_work(6));
    assert!(!core.source_local_checkpoint_restore_blocks_drive(6));
    assert!(core.source_local_checkpoint_restore_blocks_new_work(7));
    assert!(!core.source_local_checkpoint_restore_blocks_drive(7));
    assert!(!core.source_local_checkpoint_restore_blocks_new_work(8));
    assert!(!core.source_local_checkpoint_restore_blocks_drive(8));

    core.release_source_local_checkpoint_restore_after(5, 7);
    assert!(!core.source_local_checkpoint_restore_blocks_drive(7));
}

#[test]
fn scheduled_source_local_restore_stops_parallel_data_translation_driver() {
    let core = core_with_completed_fetch();
    let scheduler = PartitionedScheduler::new(1).unwrap();
    let page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    core.prepare_source_local_checkpoint_restore_after(0, 1);

    assert!(advance_parallel_data_translation(core.id(), &core, &scheduler, &page_map,).unwrap());
}

#[test]
fn parallel_driver_reconciles_orphaned_restored_fetch_before_admission() {
    let core = core_with_completed_fetch();
    let config = core.in_order_pipeline_snapshot().config().clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        7,
        [InOrderPipelineInstruction::new(
            1,
            InOrderPipelineStage::Fetch1,
        )],
    ))
    .unwrap();
    core.core.reset_fetch_stream_to_pc(Address::new(0x8000));

    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap();

    assert!(matches!(
        actions.as_slice(),
        [event]
            if event.cpu() == CpuId::new(0)
                && matches!(event.action(), RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight(),
        &[InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1,
        )]
    );
}

#[test]
fn exhausted_parallel_budgets_do_not_reconcile_orphaned_restored_fetches() {
    let core = core_with_completed_fetch();
    let config = core.in_order_pipeline_snapshot().config().clone();
    let orphaned = InOrderPipelineSnapshot::with_cycle(
        config,
        7,
        [InOrderPipelineInstruction::new(
            1,
            InOrderPipelineStage::Fetch1,
        )],
    );
    core.restore_in_order_pipeline_snapshot(orphaned.clone())
        .unwrap();
    core.core.reset_fetch_stream_to_pc(Address::new(0x8000));

    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    let actions = cluster
        .drive_ready_cores_parallel_with_instruction_budget(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            0,
        )
        .unwrap();
    assert!(actions.is_empty());
    assert_eq!(core.in_order_pipeline_snapshot(), orphaned);

    let actions = cluster
        .drive_ready_cores_parallel_with_mmio_and_instruction_budget(
            &mut scheduler,
            &transport,
            &MmioBus::new(),
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            0,
        )
        .unwrap();
    assert!(actions.is_empty());
    assert_eq!(core.in_order_pipeline_snapshot(), orphaned);
}

#[test]
fn failed_fetch_ahead_preparation_and_batch_preserve_fetch_pc() {
    let cpu = CpuId::new(0);
    let core = core_with_completed_fetch();
    let original_pc = core.inner().pc();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap();

    let mut prepared_actions = PreparedParallelActions::new();
    let mut transaction_cpus = Vec::new();
    let mut transactions = Vec::new();
    let error = push_prepared_parallel_fetch_action(
        cpu,
        &core,
        scheduler.now(),
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
        &mut prepared_actions,
        &mut transaction_cpus,
        &mut transactions,
        None,
        Some(Address::new(0x800f)),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RiscvClusterError::Core {
            error: RiscvCpuError::Cpu(crate::CpuError::FetchCrossesLine { .. }),
            ..
        }
    ));
    assert_eq!(core.inner().pc(), original_pc);

    push_prepared_parallel_fetch_action(
        cpu,
        &core,
        scheduler.now(),
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
        &mut prepared_actions,
        &mut transaction_cpus,
        &mut transactions,
        None,
        Some(Address::new(0x9000)),
    )
    .unwrap();
    transaction_cpus.push(cpu);
    transactions.push(unknown_route_transaction());

    let result = finish_prepared_parallel_actions(
        &mut scheduler,
        &transport,
        prepared_actions,
        transaction_cpus,
        transactions,
    );
    assert!(result.is_err());
    assert_eq!(core.inner().pc(), original_pc);
}

#[test]
fn due_restored_o3_writeback_wake_precedes_fetch_admission() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.o3_writeback_wake.restore_desired_unscheduled(5);
        state
            .o3_writeback_wake
            .set_desired_tick_with_fetch_bypass(Some(5), true, 0);
    }
    let event = scheduler
        .schedule_at(PartitionId::new(0), 5, |_| {})
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );

    assert!(fetch_before_pipeline_is_admitted(&core, 4));
    assert!(!fetch_before_pipeline_is_admitted(&core, 5));

    core.state
        .lock()
        .expect("riscv core lock")
        .o3_writeback_wake
        .mark_fired(5);
    assert!(fetch_before_pipeline_is_admitted(&core, 5));
}

#[test]
fn due_runtime_o3_writeback_wake_precedes_fetch_admission() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.o3_writeback_wake.set_desired_tick(Some(5), 0);
    }
    let event = scheduler
        .schedule_at(PartitionId::new(0), 5, |_| {})
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );

    assert!(!fetch_before_pipeline_is_admitted(&core, 5));

    core.mark_o3_writeback_wake_fired(5);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.o3_writeback_wake.set_desired_tick(Some(5), 5);
    }
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let event = scheduler
        .schedule_at(PartitionId::new(0), 5, |_| {})
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );

    assert!(!fetch_before_pipeline_is_admitted(&core, 5));
}

#[test]
fn exclusive_pipeline_due_wake_can_bypass_fetch_admission_until_mixed() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .o3_writeback_wake
            .set_desired_tick_with_fetch_bypass(Some(5), true, 0);
    }
    let event = scheduler
        .schedule_at(PartitionId::new(0), 5, |_| {})
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );

    assert!(fetch_before_pipeline_is_admitted(&core, 5));

    core.state
        .lock()
        .expect("riscv core lock")
        .o3_writeback_wake
        .set_desired_tick_with_fetch_bypass(Some(5), false, 5);
    assert!(!fetch_before_pipeline_is_admitted(&core, 5));

    core.state
        .lock()
        .expect("riscv core lock")
        .o3_writeback_wake
        .set_desired_tick_with_fetch_bypass(Some(5), true, 5);
    assert!(!fetch_before_pipeline_is_admitted(&core, 5));
}

#[test]
fn failed_parallel_batch_cancels_prepared_pipeline_wake() {
    let cpu = CpuId::new(0);
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let transport = MemoryTransport::new();
    let mut prepared_actions = PreparedParallelActions::new();

    assert!(push_prepared_pipeline_cycle_drive_event(
        cpu,
        &core,
        &mut scheduler,
        &mut prepared_actions,
    )
    .unwrap());
    let wake = core
        .checkpoint_owned_in_order_pipeline_wakes()
        .into_iter()
        .next()
        .expect("prepared pipeline cycle should own its scheduler wake");

    let result = finish_prepared_parallel_actions(
        &mut scheduler,
        &transport,
        prepared_actions,
        vec![cpu],
        vec![unknown_route_transaction()],
    );

    assert!(matches!(
        result,
        Err(RiscvClusterError::Core {
            cpu: failed_cpu,
            error: RiscvCpuError::Transport(TransportError::UnknownRoute { route }),
        }) if failed_cpu == cpu && route == MemoryRouteId::new(99)
    ));
    assert_eq!(scheduler.pending_event_snapshot(wake.1.id()), None);
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
}
