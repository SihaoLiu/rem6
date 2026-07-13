use super::*;
use crate::{
    AccessSize, AgentId, CacheLineLayout, CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord,
    CpuId, CpuResetState, MemoryRequestId, MemoryRouteId, TransportEndpointId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};

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

    core.reset_instruction_fetch_stream();

    assert_eq!(
        core.checkpoint_owned_in_order_pipeline_wakes(),
        vec![(scheduler.instance_id(), wake)]
    );
    scheduler.run_until_idle();
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
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
