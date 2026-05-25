use rem6_cpu::{
    CpuFetchEvent, CpuFetchRecord, CpuId, RiscvClusterDriveEvent, RiscvClusterRun,
    RiscvClusterStopReason, RiscvClusterTurn, RiscvCoreDriveAction, RiscvCoreDriveActivity,
    RiscvCpuExecutionEvent,
};
use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};
use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

fn scheduler_event(partition: u32, local: u64) -> PartitionEventId {
    PartitionEventId::new(PartitionId::new(partition), local)
}

fn fetch_event(cpu: u32, sequence: u64, pc: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(cpu),
            MemoryRouteId::new(u64::from(cpu)),
            TransportEndpointId::new(format!("cpu{cpu}.ifetch")).unwrap(),
            MemoryRequestId::new(AgentId::new(cpu), sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0073u32.to_le_bytes().to_vec(),
    )
}

fn executed(cpu: u32, sequence: u64, pc: u64) -> RiscvClusterDriveEvent {
    let instruction = RiscvInstruction::Ecall;
    RiscvClusterDriveEvent::new(
        CpuId::new(cpu),
        RiscvCoreDriveAction::InstructionExecuted(Box::new(RiscvCpuExecutionEvent::new(
            fetch_event(cpu, sequence, pc),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), None),
        ))),
    )
}

#[test]
fn cluster_run_reports_core_drive_activity_by_cpu() {
    let run = RiscvClusterRun::new(
        vec![
            RiscvClusterTurn::core(vec![
                RiscvClusterDriveEvent::new(
                    CpuId::new(0),
                    RiscvCoreDriveAction::FetchIssued {
                        event: scheduler_event(0, 1),
                    },
                ),
                RiscvClusterDriveEvent::new(
                    CpuId::new(1),
                    RiscvCoreDriveAction::FetchIssued {
                        event: scheduler_event(1, 1),
                    },
                ),
            ]),
            RiscvClusterTurn::core(vec![
                executed(0, 2, 0x8000),
                RiscvClusterDriveEvent::new(
                    CpuId::new(0),
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: scheduler_event(0, 3),
                    },
                ),
                executed(1, 4, 0x9000),
            ]),
        ],
        RiscvClusterStopReason::StopCondition,
    );

    assert_eq!(
        run.cpu_activity(CpuId::new(0)).unwrap(),
        RiscvCoreDriveActivity::new(1, 1, 1)
    );
    assert_eq!(
        run.cpu_activity(CpuId::new(1)).unwrap(),
        RiscvCoreDriveActivity::new(1, 1, 0)
    );
    assert_eq!(run.cpu_activities().len(), 2);
    assert_eq!(run.active_cpu_count(), 2);
    assert!(run.has_cpu_activity(CpuId::new(0)));
    assert!(!run.has_cpu_activity(CpuId::new(2)));
    assert_eq!(
        run.turns()[1].cpu_activity(CpuId::new(0)).unwrap(),
        RiscvCoreDriveActivity::new(0, 1, 1)
    );
    assert_eq!(run.turns()[1].active_cpu_count(), 2);
    assert_eq!(
        run.cpu_activity(CpuId::new(0))
            .unwrap()
            .total_drive_action_count(),
        3
    );
    assert!(run.cpu_activity(CpuId::new(0)).unwrap().has_activity());
    assert!(run.cpu_activity(CpuId::new(2)).is_none());
    assert_eq!(
        run.partition_activity(PartitionId::new(0)).unwrap(),
        RiscvCoreDriveActivity::new(1, 1, 1)
    );
    assert_eq!(
        run.partition_activity(PartitionId::new(1)).unwrap(),
        RiscvCoreDriveActivity::new(1, 1, 0)
    );
    assert_eq!(run.partition_activities().len(), 2);
    assert_eq!(run.active_partition_count(), 2);
    assert!(run.has_partition_activity(PartitionId::new(0)));
    assert!(!run.has_partition_activity(PartitionId::new(2)));
    assert_eq!(
        run.turns()[1]
            .partition_activity(PartitionId::new(0))
            .unwrap(),
        RiscvCoreDriveActivity::new(0, 1, 1)
    );
    assert_eq!(run.turns()[1].active_partition_count(), 2);
    assert!(run.partition_activity(PartitionId::new(2)).is_none());
}

#[test]
fn cluster_run_reports_parallel_scheduler_partition_activity() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 2).unwrap();
    for index in 0..3 {
        scheduler
            .schedule_parallel_at(PartitionId::new(index), 0, |_| {})
            .unwrap();
    }
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvClusterRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        RiscvClusterStopReason::StopCondition,
    );

    let epoch = run.turns()[0].parallel_scheduler_epoch().unwrap();
    let partition0_activity = epoch.partition_activity(PartitionId::new(0)).unwrap();
    assert_eq!(partition0_activity.worker_count(), 1);
    assert_eq!(partition0_activity.dispatch_count(), 1);
    assert_eq!(partition0_activity.max_pending_events(), 1);
    assert_eq!(epoch.active_partition_count(), 3);
    assert!(epoch.has_partition_activity(PartitionId::new(1)));

    let run_partition2 = run
        .parallel_scheduler_partition_activity(PartitionId::new(2))
        .unwrap();
    assert_eq!(run_partition2.worker_count(), 1);
    assert_eq!(run_partition2.dispatch_count(), 1);
    assert_eq!(run_partition2.max_pending_events(), 1);
    assert_eq!(run.active_parallel_scheduler_partition_count(), 3);
    assert!(run.has_parallel_scheduler_partition_activity(PartitionId::new(0)));
    assert!(!run.has_parallel_scheduler_partition_activity(PartitionId::new(3)));
}

#[test]
fn cluster_run_reports_parallel_scheduler_remote_flows() {
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let memory = PartitionId::new(2);
    let io = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(core0, 0, move |context| {
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
            context.schedule_remote_after(io, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(core1, 0, move |context| {
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvClusterRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        RiscvClusterStopReason::StopCondition,
    );

    let epoch = run.turns()[0].parallel_scheduler_epoch().unwrap();
    let flows = epoch.remote_flows();
    assert_eq!(flows.len(), 3);
    assert_eq!(flows[0].source(), core0);
    assert_eq!(flows[0].target(), memory);
    assert_eq!(flows[0].send_count(), 2);
    assert_eq!(flows[0].first_tick(), 4);
    assert_eq!(flows[0].last_tick(), 4);
    assert_eq!(flows[1].source(), core0);
    assert_eq!(flows[1].target(), io);
    assert_eq!(flows[1].send_count(), 1);
    assert_eq!(flows[2].source(), core1);
    assert_eq!(flows[2].target(), memory);
    assert_eq!(flows[2].send_count(), 1);
    assert_eq!(epoch.remote_flow_count(core0, memory), 2);
    assert_eq!(epoch.remote_flow_count(core1, memory), 1);
    assert_eq!(epoch.remote_flow_count(memory, core0), 0);

    assert_eq!(run.parallel_scheduler_remote_flows(), flows);
    assert_eq!(run.parallel_scheduler_remote_flow_count(core0, memory), 2);
    assert_eq!(run.parallel_scheduler_remote_flow_count(core1, memory), 1);
    assert_eq!(run.parallel_scheduler_remote_flow_count(memory, core0), 0);
}

#[test]
fn cluster_run_preserves_remote_flow_delay_bounds_across_epochs() {
    let source = PartitionId::new(0);
    let target = PartitionId::new(1);
    let mut first_scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();
    first_scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    let first_plan = first_scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let first_recorded = first_scheduler.run_next_epoch_parallel_recorded().unwrap();

    let mut second_scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 8, 1).unwrap();
    second_scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 8, |_| {}).unwrap();
        })
        .unwrap();
    let second_plan = second_scheduler
        .plan_next_parallel_epoch()
        .unwrap()
        .unwrap();
    let second_recorded = second_scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvClusterRun::new(
        vec![
            RiscvClusterTurn::parallel_scheduler(first_plan, first_recorded),
            RiscvClusterTurn::parallel_scheduler(second_plan, second_recorded),
        ],
        RiscvClusterStopReason::StopCondition,
    );

    let flows = run.parallel_scheduler_remote_flows();
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].source(), source);
    assert_eq!(flows[0].target(), target);
    assert_eq!(flows[0].send_count(), 2);
    assert_eq!(flows[0].first_tick(), 4);
    assert_eq!(flows[0].last_tick(), 8);
    assert_eq!(flows[0].delay_bounds(), Some((4, 8)));
}
