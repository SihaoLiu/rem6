use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::{
    GlobalInstTrackerSnapshot, PcCountPair, PcCountTrackerSnapshot, ProbePayload, StatId,
    StatsRegistry,
};
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvInstructionStats, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, RiscvTrapEventPort, SystemHostController, SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn loaded_program_store(instructions: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();

    let mut image = BootImage::new(Address::new(instructions[0].0));
    for (address, instruction) in instructions {
        image = image
            .add_segment(Address::new(*address), word(*instruction))
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn riscv_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    }
}

#[test]
fn system_run_instruction_stats_drive_pc_count_from_retired_fetch_pcs() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(41);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([
        riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000),
        riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000),
    ])
    .unwrap();
    let store = loaded_program_store(&[
        (0x8000, 0x0000_0013),
        (0x8004, 0x0000_0013),
        (0x9000, 0x0000_0013),
        (0x9004, 0x0000_0013),
    ]);
    let mut stats = StatsRegistry::new();
    let cpu0_committed = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    let cpu1_committed = stats
        .register_counter("cpu1.committed_insts", "count")
        .unwrap();
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        stats,
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::with_instruction_stats(
        trap_port,
        RiscvInstructionStats::new([
            (CpuId::new(0), cpu0_committed),
            (CpuId::new(1), cpu1_committed),
        ])
        .with_retired_inst_thresholds([3])
        .with_pc_count_targets([PcCountPair::new(0x8000, 1), PcCountPair::new(0x9000, 1)]),
    );

    let run = driver
        .drive_until_host_stop_or_instruction_limit_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            100,
            3,
            |cpu| GuestEventId::new(100 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::InstructionLimit {
            tick: run.final_tick().unwrap(),
            limit: 3,
            committed: 3,
        }
    );

    let retired = run
        .retired_instruction_probes()
        .expect("run should carry retired instruction probe evidence");
    assert_eq!(
        retired.tracker(),
        &GlobalInstTrackerSnapshot::new(3, Vec::new())
    );
    assert_eq!(
        run.retired_instruction_counts_by_cpu(),
        BTreeMap::from([(CpuId::new(0), 2), (CpuId::new(1), 1)])
    );
    assert_eq!(
        retired.pc_count(),
        Some(&PcCountTrackerSnapshot::new(
            vec![(0x8000, 1), (0x9000, 1)],
            Vec::new(),
            PcCountPair::new(0x9000, 1),
            false,
        ))
    );

    let retired_pc_point = retired
        .retired_pc_point_for_cpu(CpuId::new(0))
        .expect("cpu0 should have a retired PC probe point");
    assert!(retired.retired_pc_point_for_cpu(CpuId::new(1)).is_some());
    let pc_payloads = retired
        .probes()
        .events()
        .iter()
        .filter(|event| event.point() == retired_pc_point)
        .map(|event| event.payload())
        .collect::<Vec<_>>();
    assert_eq!(
        pc_payloads,
        vec![
            &ProbePayload::ProgramCounter { pc: 0x8000 },
            &ProbePayload::ProgramCounter { pc: 0x8004 },
        ]
    );
}

#[test]
fn instruction_stats_clone_uses_independent_pc_count_recorders() {
    let stats = RiscvInstructionStats::new([(CpuId::new(0), StatId::new(0))])
        .with_pc_count_targets([PcCountPair::new(0x8000, 1)]);
    let cloned = stats.clone();

    stats
        .record_retired_instruction_probe(CpuId::new(0), 10, 0x8000)
        .unwrap();

    assert_eq!(
        stats
            .retired_instruction_probe_snapshot()
            .pc_count()
            .unwrap()
            .current_pair(),
        PcCountPair::new(0x8000, 1)
    );
    assert_eq!(
        cloned
            .retired_instruction_probe_snapshot()
            .pc_count()
            .unwrap()
            .current_pair(),
        PcCountPair::new(0, 0)
    );

    cloned
        .record_retired_instruction_probe(CpuId::new(0), 12, 0x8000)
        .unwrap();

    assert_eq!(
        cloned
            .retired_instruction_probe_snapshot()
            .pc_count()
            .unwrap()
            .current_pair(),
        PcCountPair::new(0x8000, 1)
    );
}
