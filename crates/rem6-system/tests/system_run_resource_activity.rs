use rem6_boot::BootImage;
use rem6_coherence::{
    ParallelCoherenceRunHistory, ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs,
};
use rem6_cpu::{
    CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvClusterTurn, RiscvCoreTopologyConfig,
};
use rem6_dram::{
    DramActivityProfile, DramGeometry, DramMemoryTechnology, DramTargetActivity, DramTiming,
    ExternalMemoryProfile,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{
    ClockDomain, ParallelRunProfile, PartitionId, PartitionedScheduler,
    RecordedConservativeRunSummary, WaitForEdgeKind, WaitForGraph, WaitForNode,
    WaitForTargetNodeWindow,
};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, RiscvDataCacheProtocol, RiscvDataCacheRunHistoryRecord,
    RiscvDataCacheRunRecord, RiscvSystemRun, RiscvSystemRunStopReason, RiscvTopologyDramConfig,
    RiscvTopologyHostConfig, RiscvTopologyMemoryConfig, RiscvTopologySystem, StopRequest,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, FabricConnectionConfig, PortDirection,
    PortName, Topology, TopologyBuilder,
};
use rem6_transport::MemoryTrace;

fn component(name: &str) -> ComponentId {
    ComponentId::new(name).unwrap()
}

fn kind(name: &str) -> ComponentKind {
    ComponentKind::new(name).unwrap()
}

fn port(name: &str) -> PortName {
    PortName::new(name).unwrap()
}

fn endpoint(component_name: &str, port_name: &str) -> Endpoint {
    Endpoint::new(component(component_name), port(port_name))
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn fabric(link: &str, bandwidth: u64) -> FabricConnectionConfig {
    FabricConnectionConfig::new(rem6_fabric::FabricLinkId::new(link).unwrap(), bandwidth)
        .with_virtual_networks(
            rem6_fabric::VirtualNetworkId::new(1),
            rem6_fabric::VirtualNetworkId::new(2),
        )
}

fn cpu_fabric_topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu0", "dmem"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn core_config(agent: u32) -> RiscvCoreTopologyConfig {
    core_config_on(0, 0, agent, "cpu0")
}

fn core_config_on(
    cpu: u32,
    partition: u32,
    agent: u32,
    cpu_component: &str,
) -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(0x8000),
        ),
        endpoint(cpu_component, "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint(cpu_component, "dmem"),
        endpoint("mem0", "requests"),
        layout(),
    )
}

fn contended_cpu_fabric_topology() -> Topology {
    TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("cpu1"),
                kind("cpu"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu0", "dmem"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu1", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu1", "dmem"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn ecall_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn load_then_ecall_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x3008),
            vec![0x89, 0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab],
        )
        .unwrap()
}

fn dram_config() -> RiscvTopologyDramConfig {
    RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
}

fn memory_config() -> RiscvTopologyMemoryConfig {
    RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout())
        .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
}

fn wait_node(name: &str) -> WaitForNode {
    WaitForNode::transaction(name).unwrap()
}

fn wait_resource(name: &str) -> WaitForNode {
    WaitForNode::resource(name).unwrap()
}

fn coherence_run_with_waits() -> ParallelCoherenceRunSummary {
    let request = wait_node("data.load.0");
    let line = wait_resource("cache.0.line.3000");
    let mut initial = WaitForGraph::new();
    initial
        .record_wait(request.clone(), line.clone(), WaitForEdgeKind::Queue, 4)
        .unwrap();

    let mut remaining = WaitForGraph::new();
    remaining
        .record_wait(request.clone(), line.clone(), WaitForEdgeKind::Queue, 4)
        .unwrap();
    remaining
        .record_wait(request.clone(), line.clone(), WaitForEdgeKind::Queue, 6)
        .unwrap();
    remaining
        .record_wait(line, request, WaitForEdgeKind::Protocol, 9)
        .unwrap();

    ParallelCoherenceRunSummary::new(
        RecordedConservativeRunSummary::empty(12),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        ParallelCoherenceWaitForGraphs::new(initial, remaining),
    )
}

fn empty_coherence_run(final_tick: u64) -> ParallelCoherenceRunSummary {
    ParallelCoherenceRunSummary::new(
        RecordedConservativeRunSummary::empty(final_tick),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new()),
    )
}

fn coherence_run_with_remote_flow(
    source: PartitionId,
    target: PartitionId,
) -> ParallelCoherenceRunSummary {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(5, 4, 2).unwrap();
    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();

    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new()),
    )
}

fn split_dram_config() -> RiscvTopologyDramConfig {
    dram_config()
        .add_target(
            MemoryTargetId::new(1),
            layout(),
            DramGeometry::new(4, 64, 16).unwrap(),
            DramTiming::new(3, 5, 9, 2, 2).unwrap(),
        )
        .unwrap()
        .add_region_for_target(
            MemoryTargetId::new(1),
            Address::new(0x3000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap()
}

#[test]
fn system_run_starts_without_resource_activity() {
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    );

    assert!(!run.has_resource_activity());
    assert!(!run.has_fabric_activity());
    assert!(!run.has_dram_activity());
    assert_eq!(run.resource_activity_count(), 0);
    assert_eq!(run.fabric_transfer_count(), 0);
    assert_eq!(run.dram_access_count(), 0);
    assert_eq!(run.fabric_activities().len(), 0);
    assert_eq!(run.dram_target_activities().len(), 0);
    assert_eq!(run.data_cache_run_count(), 0);
    assert!(run.data_cache_runs().is_empty());
    assert!(run.data_cache_run_records().is_empty());
    assert!(run.data_cache_protocols().is_empty());
    assert!(run.data_cache_protocol_counts().is_empty());
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        0
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Mesi),
        0
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Moesi),
        0
    );
    assert!(run
        .data_cache_runs_for_protocol(RiscvDataCacheProtocol::Msi)
        .is_empty());
    assert!(!run.has_data_cache_protocol(RiscvDataCacheProtocol::Msi));
    assert_eq!(run.unattributed_data_cache_run_count(), 0);
    assert!(run.data_cache_parallel_scheduler_epochs().is_empty());
    assert!(run.data_cache_parallel_scheduler_dispatches().is_empty());
    assert!(run.data_cache_parallel_scheduler_batches().is_empty());
    assert!(run
        .data_cache_parallel_scheduler_worker_partitions()
        .is_empty());
    assert!(run
        .data_cache_parallel_scheduler_partition_activities()
        .is_empty());
    assert!(run.data_cache_parallel_scheduler_remote_flows().is_empty());
    assert_eq!(
        run.data_cache_parallel_scheduler_remote_flow_count(
            PartitionId::new(0),
            PartitionId::new(1)
        ),
        0
    );
    assert_eq!(
        run.active_data_cache_parallel_scheduler_partition_count(),
        0
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_profile(),
        ParallelRunProfile::default()
    );
    assert_eq!(
        run.data_cache_parallel_run_history(),
        ParallelCoherenceRunHistory::default()
    );
    assert!(run
        .data_cache_parallel_run_histories_by_protocol()
        .is_empty());
    assert!(run.data_cache_parallel_run_history_records().is_empty());
    assert_eq!(
        run.attributed_data_cache_parallel_run_history(),
        ParallelCoherenceRunHistory::default()
    );
    assert_eq!(
        run.unattributed_data_cache_parallel_run_history(),
        ParallelCoherenceRunHistory::default()
    );
    assert_eq!(run.attributed_data_cache_parallel_run_count(), 0);
    assert_eq!(run.unattributed_data_cache_parallel_run_count(), 0);
    assert_eq!(
        run.data_cache_parallel_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        0,
    );
    assert!(!run.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi));
    assert_eq!(
        run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi),
        ParallelCoherenceRunHistory::default()
    );
    assert_eq!(run.data_cache_parallel_scheduler_epoch_count(), 0);
    assert_eq!(run.data_cache_parallel_scheduler_dispatch_count(), 0);
    assert_eq!(run.data_cache_parallel_scheduler_batch_count(), 0);
    assert_eq!(run.data_cache_parallel_scheduler_max_workers(), 0);
    assert_eq!(
        run.full_system_parallel_scheduler_profile(),
        ParallelRunProfile::default()
    );
    assert_eq!(run.full_system_parallel_scheduler_dispatch_count(), 0);
    assert_eq!(run.full_system_parallel_scheduler_batch_count(), 0);
    assert_eq!(run.full_system_parallel_scheduler_max_workers(), 0);
    assert!(run.full_system_parallel_scheduler_dispatches().is_empty());
    assert!(run.full_system_parallel_scheduler_batches().is_empty());
    assert!(run
        .full_system_parallel_scheduler_worker_partitions()
        .is_empty());
    assert!(run
        .full_system_parallel_scheduler_partition_activities()
        .is_empty());
    assert!(run.full_system_parallel_scheduler_remote_flows().is_empty());
    assert_eq!(
        run.full_system_parallel_scheduler_remote_flow_count(
            PartitionId::new(0),
            PartitionId::new(1)
        ),
        0
    );
    assert_eq!(
        run.active_full_system_parallel_scheduler_partition_count(),
        0
    );
    assert!(!run.has_full_system_parallel_scheduler_work());
    assert_eq!(run.initial_data_cache_wait_for_edge_count(), 0);
    assert_eq!(run.remaining_data_cache_wait_for_edge_count(), 0);
    assert!(run.initial_data_cache_wait_for_edges().is_empty());
    assert!(run.remaining_data_cache_wait_for_edges().is_empty());
    assert_eq!(run.data_cache_wait_for_edge_count(), 0);
    assert!(run.data_cache_wait_for_edges().is_empty());
    assert!(!run.has_data_cache_wait_for_edges());
    assert_eq!(run.fabric_wait_for_edge_count(), 0);
    assert!(run.fabric_wait_for_edges().is_empty());
    assert!(!run.has_fabric_wait_for_edges());
    assert_eq!(run.dram_wait_for_edge_count(), 0);
    assert!(run.dram_wait_for_edges().is_empty());
    assert!(!run.has_dram_wait_for_edges());
    assert_eq!(run.initial_data_cache_deadlock_diagnostic_count(), 0);
    assert_eq!(run.remaining_data_cache_deadlock_diagnostic_count(), 0);
    assert_eq!(run.data_cache_deadlock_diagnostic_count(), 0);
}

#[test]
fn system_run_reports_parallel_scheduler_remote_flows() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let directory = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let turn = RiscvClusterTurn::parallel_scheduler(plan, recorded);
    let data_cache_run = coherence_run_with_remote_flow(cache, directory);
    assert_eq!(data_cache_run.remote_flow_count(cache, directory), 2);

    let run = RiscvSystemRun::new(
        vec![turn],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run]);

    assert_eq!(run.parallel_scheduler_remote_flow_count(core, memory), 1);
    assert_eq!(
        run.data_cache_parallel_scheduler_remote_flow_count(cache, directory),
        2
    );
    assert_eq!(
        run.full_system_parallel_scheduler_remote_flow_count(core, memory),
        1
    );
    assert_eq!(
        run.full_system_parallel_scheduler_remote_flow_count(cache, directory),
        2
    );
    assert_eq!(
        run.full_system_parallel_scheduler_remote_flow_count(memory, core),
        0
    );

    let core_flows = run.parallel_scheduler_remote_flows();
    assert_eq!(core_flows.len(), 1);
    assert_eq!(core_flows[0].source(), core);
    assert_eq!(core_flows[0].target(), memory);
    assert_eq!(core_flows[0].send_count(), 1);
    let core_sends = run.parallel_scheduler_remote_sends();
    assert_eq!(core_sends.len(), 1);
    assert_eq!(run.parallel_scheduler_total_remote_send_count(), 1);
    assert_eq!(run.parallel_scheduler_remote_send_count(core, memory), 1);
    assert_eq!(run.parallel_scheduler_remote_send_count(memory, core), 0);
    assert_eq!(
        run.parallel_scheduler_remote_source_partitions(),
        vec![core]
    );
    assert_eq!(
        run.parallel_scheduler_remote_target_partitions(),
        vec![memory]
    );
    assert_eq!(core_sends[0].source(), core);
    assert_eq!(core_sends[0].target(), memory);
    assert_eq!(core_sends[0].source_tick(), 0);
    assert_eq!(core_sends[0].delivery_tick(), 4);
    assert_eq!(core_sends[0].delay(), 4);
    assert_eq!(core_sends[0].order(), 0);

    let data_cache_flows = run.data_cache_parallel_scheduler_remote_flows();
    assert_eq!(data_cache_flows.len(), 1);
    assert_eq!(data_cache_flows[0].source(), cache);
    assert_eq!(data_cache_flows[0].target(), directory);
    assert_eq!(data_cache_flows[0].send_count(), 2);
    let data_cache_sends = run.data_cache_parallel_scheduler_remote_sends();
    assert_eq!(data_cache_sends.len(), 2);
    assert_eq!(data_cache_sends[0].source(), cache);
    assert_eq!(data_cache_sends[0].target(), directory);
    assert_eq!(data_cache_sends[0].source_tick(), 0);
    assert_eq!(data_cache_sends[0].delivery_tick(), 4);
    assert_eq!(data_cache_sends[0].delay(), 4);
    assert_eq!(data_cache_sends[0].order(), 0);
    assert_eq!(data_cache_sends[1].order(), 1);

    let full_flows = run.full_system_parallel_scheduler_remote_flows();
    assert_eq!(full_flows.len(), 2);
    assert_eq!(full_flows[0], core_flows[0]);
    assert_eq!(full_flows[1], data_cache_flows[0]);
    let full_sends = run.full_system_parallel_scheduler_remote_sends();
    assert_eq!(full_sends.len(), 3);
    assert_eq!(full_sends[0], core_sends[0]);
    assert_eq!(full_sends[1], data_cache_sends[0]);
    assert_eq!(full_sends[2], data_cache_sends[1]);
}

#[test]
fn system_run_aggregates_fabric_wait_for_diagnostics() {
    let packet = wait_node("fabric.packet.7");
    let credit = wait_resource("fabric.cpu_mem.vn.1.credit");
    let lane = wait_resource("fabric.cpu_mem.vn.1.lane");
    let mut graph = WaitForGraph::new();
    graph
        .record_wait(packet.clone(), credit.clone(), WaitForEdgeKind::Credit, 6)
        .unwrap();
    graph
        .record_wait(packet.clone(), credit.clone(), WaitForEdgeKind::Credit, 8)
        .unwrap();
    graph
        .record_wait(packet, lane.clone(), WaitForEdgeKind::Queue, 10)
        .unwrap();

    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 12 },
    )
    .with_fabric_wait_for(graph);

    assert!(run.has_resource_activity());
    assert!(run.has_fabric_wait_for_edges());
    assert_eq!(run.fabric_wait_for_edge_count(), 2);
    assert_eq!(run.fabric_wait_for_edges().len(), 2);
    assert_eq!(run.fabric_wait_for_blocked_nodes().len(), 1);
    assert_eq!(
        run.fabric_wait_for_target_nodes(),
        vec![credit.clone(), lane.clone()],
    );
    assert_eq!(
        run.fabric_wait_for_target_node_windows(),
        vec![
            WaitForTargetNodeWindow::new(credit, 1, 6, 8),
            WaitForTargetNodeWindow::new(lane, 1, 10, 10),
        ],
    );
    assert_eq!(
        run.fabric_wait_for_edge_count_by_kind(WaitForEdgeKind::Credit),
        1,
    );
    assert_eq!(
        run.fabric_wait_for_edge_kind_counts()
            .get(&WaitForEdgeKind::Queue)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.fabric_oldest_wait_edge().unwrap().first_observed_tick(),
        6,
    );
    assert_eq!(
        run.fabric_newest_observed_wait_edge()
            .unwrap()
            .last_observed_tick(),
        10,
    );
    assert_eq!(run.fabric_total_wait_observation_count(), 3);
    assert_eq!(run.fabric_first_wait_tick(), Some(6));
    assert_eq!(run.fabric_last_wait_tick(), Some(10));
    assert_eq!(run.fabric_longest_observed_wait_span(), Some(2));
    assert!(run.fabric_deadlock_diagnostics().is_empty());
    assert_eq!(
        run.resource_activity_count(),
        run.fabric_transfer_count() + run.dram_operation_count() + run.fabric_wait_for_edge_count(),
    );
}

#[test]
fn system_run_aggregates_dram_wait_for_diagnostics() {
    let request = wait_node("dram.target.0.agent.1.request.7");
    let bank = wait_resource("dram.target.0.port.0.bank.0");
    let bus = wait_resource("dram.target.0.port.0.bus");
    let mut graph = WaitForGraph::new();
    graph
        .record_wait(request.clone(), bank, WaitForEdgeKind::Queue, 4)
        .unwrap();
    graph
        .record_wait(request.clone(), bus, WaitForEdgeKind::Resource, 8)
        .unwrap();
    graph
        .record_wait(
            request,
            wait_resource("dram.target.0.port.1.bus"),
            WaitForEdgeKind::Resource,
            11,
        )
        .unwrap();

    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 12 },
    )
    .with_dram_wait_for(graph);

    assert!(run.has_resource_activity());
    assert!(run.has_dram_wait_for_edges());
    assert_eq!(run.dram_wait_for_edge_count(), 3);
    assert_eq!(
        run.dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Resource),
        2,
    );
    assert_eq!(run.dram_wait_for_blocked_nodes().len(), 1);
    assert_eq!(run.dram_first_wait_tick(), Some(4));
    assert_eq!(run.dram_last_wait_tick(), Some(11));
    assert_eq!(run.dram_longest_observed_wait_span(), Some(0));
    assert_eq!(
        run.resource_activity_count(),
        run.fabric_transfer_count()
            + run.dram_operation_count()
            + run.fabric_wait_for_edge_count()
            + run.dram_wait_for_edge_count(),
    );
}

#[test]
fn system_run_preserves_barrier_wait_kind_in_full_system_diagnostics() {
    let barrier = WaitForNode::checkpoint_barrier("roi-start").unwrap();
    let first_partition = WaitForNode::partition(PartitionId::new(0));
    let second_partition = WaitForNode::partition(PartitionId::new(1));
    let mut graph = WaitForGraph::new();
    graph
        .record_wait(
            first_partition.clone(),
            barrier.clone(),
            WaitForEdgeKind::Barrier,
            40,
        )
        .unwrap();
    graph
        .record_wait(
            second_partition.clone(),
            barrier,
            WaitForEdgeKind::Barrier,
            41,
        )
        .unwrap();

    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 42 },
    )
    .with_fabric_wait_for(graph);

    assert_eq!(
        run.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Barrier),
        2,
    );
    assert_eq!(
        run.full_system_wait_for_edge_kind_counts()
            .get(&WaitForEdgeKind::Barrier)
            .copied(),
        Some(2),
    );
    assert_eq!(
        run.full_system_wait_for_blocked_nodes(),
        vec![first_partition, second_partition],
    );
    assert_eq!(run.full_system_first_wait_tick(), Some(40));
    assert_eq!(run.full_system_last_wait_tick(), Some(41));
}

#[test]
fn system_run_aggregates_data_cache_wait_for_diagnostics() {
    let cache_run = coherence_run_with_waits();
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 12 },
    )
    .with_data_cache_runs(vec![cache_run.clone()]);

    assert_eq!(run.data_cache_runs(), &[cache_run]);
    assert_eq!(run.data_cache_protocols(), vec![None]);
    assert!(run.data_cache_protocol_counts().is_empty());
    assert_eq!(run.unattributed_data_cache_run_count(), 1);
    assert!(run
        .data_cache_runs_for_protocol(RiscvDataCacheProtocol::Msi)
        .is_empty());
    assert_eq!(run.data_cache_run_count(), 1);
    assert_eq!(
        run.data_cache_parallel_scheduler_profile(),
        ParallelRunProfile::default()
    );
    assert_eq!(run.data_cache_parallel_run_history().run_count(), 1);
    assert!(run.data_cache_parallel_run_history().has_wait_for_edges());
    assert_eq!(
        run.unattributed_data_cache_parallel_run_history(),
        run.data_cache_parallel_run_history()
    );
    assert_eq!(
        run.attributed_data_cache_parallel_run_history(),
        ParallelCoherenceRunHistory::default()
    );
    assert_eq!(run.unattributed_data_cache_parallel_run_count(), 1);
    assert_eq!(run.attributed_data_cache_parallel_run_count(), 0);
    assert!(!run.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi));
    assert_eq!(
        run.full_system_parallel_scheduler_profile(),
        run.parallel_scheduler_profile()
    );
    assert_eq!(run.initial_data_cache_wait_for_edge_count(), 1);
    assert_eq!(run.remaining_data_cache_wait_for_edge_count(), 2);
    assert_eq!(run.data_cache_wait_for_edge_count(), 2);
    assert_eq!(run.initial_data_cache_wait_for_edges().len(), 1);
    assert_eq!(run.remaining_data_cache_wait_for_edges().len(), 2);
    assert_eq!(run.data_cache_wait_for_edges().len(), 2);
    assert_eq!(run.initial_data_cache_wait_for_blocked_nodes().len(), 1);
    assert_eq!(run.remaining_data_cache_wait_for_blocked_nodes().len(), 2);
    assert_eq!(
        run.initial_data_cache_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(
        run.initial_data_cache_wait_for_edge_kind_counts()
            .get(&WaitForEdgeKind::Queue)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.remaining_data_cache_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(
        run.remaining_data_cache_wait_for_edge_count_by_kind(WaitForEdgeKind::Protocol),
        1,
    );
    assert_eq!(
        run.remaining_data_cache_wait_for_edge_kind_counts()
            .get(&WaitForEdgeKind::Protocol)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.data_cache_wait_for_edge_kind_counts()
            .get(&WaitForEdgeKind::Protocol)
            .copied(),
        Some(1),
    );
    assert_eq!(run.data_cache_wait_for_blocked_nodes().len(), 2);
    assert_eq!(
        run.remaining_data_cache_oldest_wait_edge()
            .unwrap()
            .first_observed_tick(),
        4,
    );
    assert_eq!(
        run.data_cache_oldest_wait_edge()
            .unwrap()
            .first_observed_tick(),
        4,
    );
    assert_eq!(
        run.initial_data_cache_oldest_wait_edge()
            .unwrap()
            .first_observed_tick(),
        4,
    );
    assert_eq!(
        run.remaining_data_cache_newest_observed_wait_edge()
            .unwrap()
            .last_observed_tick(),
        9,
    );
    assert_eq!(
        run.data_cache_newest_observed_wait_edge()
            .unwrap()
            .last_observed_tick(),
        9,
    );
    assert_eq!(
        run.initial_data_cache_newest_observed_wait_edge()
            .unwrap()
            .last_observed_tick(),
        4,
    );
    assert_eq!(run.initial_data_cache_total_wait_observation_count(), 1);
    assert_eq!(run.remaining_data_cache_total_wait_observation_count(), 3);
    assert_eq!(run.data_cache_total_wait_observation_count(), 3);
    assert_eq!(run.initial_data_cache_first_wait_tick(), Some(4));
    assert_eq!(run.remaining_data_cache_first_wait_tick(), Some(4));
    assert_eq!(run.data_cache_first_wait_tick(), Some(4));
    assert_eq!(run.initial_data_cache_last_wait_tick(), Some(4));
    assert_eq!(run.remaining_data_cache_last_wait_tick(), Some(9));
    assert_eq!(run.data_cache_last_wait_tick(), Some(9));
    assert_eq!(run.initial_data_cache_longest_observed_wait_span(), Some(0));
    assert_eq!(
        run.remaining_data_cache_longest_observed_wait_span(),
        Some(2)
    );
    assert_eq!(run.data_cache_longest_observed_wait_span(), Some(2));
    assert!(run.initial_data_cache_deadlock_diagnostics().is_empty());
    assert_eq!(run.initial_data_cache_deadlock_diagnostic_count(), 0);
    assert_eq!(
        run.remaining_data_cache_deadlock_diagnostics()[0].edge_count(),
        2,
    );
    assert_eq!(run.data_cache_deadlock_diagnostics()[0].edge_count(), 2);
    assert_eq!(run.remaining_data_cache_deadlock_diagnostic_count(), 1);
    assert_eq!(run.data_cache_deadlock_diagnostic_count(), 1);
    assert!(run.has_data_cache_wait_for_edges());
}

#[test]
fn system_run_reports_cross_subsystem_wait_for_deadlocks() {
    let packet = wait_node("fabric.packet.42");
    let line = wait_resource("cache.0.line.4000");

    let mut fabric_graph = WaitForGraph::new();
    fabric_graph
        .record_wait(packet.clone(), line.clone(), WaitForEdgeKind::Queue, 5)
        .unwrap();

    let mut data_cache_remaining = WaitForGraph::new();
    data_cache_remaining
        .record_wait(line.clone(), packet.clone(), WaitForEdgeKind::Protocol, 7)
        .unwrap();
    let data_cache_run = ParallelCoherenceRunSummary::new(
        RecordedConservativeRunSummary::empty(9),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), data_cache_remaining),
    );

    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 9 },
    )
    .with_fabric_wait_for(fabric_graph)
    .with_data_cache_runs(vec![data_cache_run]);

    assert_eq!(run.fabric_deadlock_diagnostic_count(), 0);
    assert_eq!(run.data_cache_deadlock_diagnostic_count(), 0);
    assert_eq!(run.resource_wait_for_edge_count(), 1);
    assert_eq!(run.full_system_wait_for_edge_count(), 2);
    assert!(run.has_full_system_wait_for_edges());
    assert_eq!(run.resource_wait_for_target_nodes(), vec![line.clone()]);
    assert_eq!(
        run.full_system_wait_for_target_nodes(),
        vec![line.clone(), packet.clone()],
    );
    assert_eq!(
        run.full_system_wait_for_target_node_windows(),
        vec![
            WaitForTargetNodeWindow::new(line.clone(), 1, 5, 5),
            WaitForTargetNodeWindow::new(packet.clone(), 1, 7, 7),
        ],
    );
    assert_eq!(
        run.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(
        run.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Protocol),
        1,
    );
    assert_eq!(run.full_system_deadlock_diagnostic_count(), 1);
    assert!(run.has_full_system_deadlock_diagnostics());
    let diagnostic = run.full_system_deadlock_diagnostics().remove(0);
    assert_eq!(diagnostic.edge_count(), 2);
    assert_eq!(diagnostic.first_observed_tick(), 5);
    assert_eq!(diagnostic.last_observed_tick(), 7);
}

#[test]
fn system_run_tracks_protocol_tagged_data_cache_runs() {
    let msi_run = empty_coherence_run(8);
    let mesi_run = empty_coherence_run(13);
    let moesi_run = coherence_run_with_waits();
    let chi_run = empty_coherence_run(21);
    let records = vec![
        RiscvDataCacheRunRecord::new(RiscvDataCacheProtocol::Msi, msi_run.clone()),
        RiscvDataCacheRunRecord::new(RiscvDataCacheProtocol::Mesi, mesi_run.clone()),
        RiscvDataCacheRunRecord::new(RiscvDataCacheProtocol::Moesi, moesi_run.clone()),
        RiscvDataCacheRunRecord::new(RiscvDataCacheProtocol::Chi, chi_run.clone()),
    ];

    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 13 },
    )
    .with_data_cache_run_records(records.clone());

    assert_eq!(run.data_cache_run_count(), 4);
    assert_eq!(
        run.data_cache_runs(),
        &[
            msi_run.clone(),
            mesi_run.clone(),
            moesi_run.clone(),
            chi_run.clone()
        ]
    );
    assert_eq!(run.data_cache_run_records(), records);
    assert_eq!(
        run.data_cache_protocols(),
        vec![
            Some(RiscvDataCacheProtocol::Msi),
            Some(RiscvDataCacheProtocol::Mesi),
            Some(RiscvDataCacheProtocol::Moesi),
            Some(RiscvDataCacheProtocol::Chi),
        ],
    );
    assert_eq!(run.unattributed_data_cache_run_count(), 0);
    assert_eq!(
        run.data_cache_protocol_counts()
            .get(&RiscvDataCacheProtocol::Msi)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.data_cache_protocol_counts()
            .get(&RiscvDataCacheProtocol::Mesi)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.data_cache_protocol_counts()
            .get(&RiscvDataCacheProtocol::Moesi)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.data_cache_protocol_counts()
            .get(&RiscvDataCacheProtocol::Chi)
            .copied(),
        Some(1),
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        1,
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Mesi),
        1,
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Moesi),
        1,
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Chi),
        1,
    );
    assert!(run.has_data_cache_protocol(RiscvDataCacheProtocol::Msi));
    assert!(run.has_data_cache_protocol(RiscvDataCacheProtocol::Mesi));
    assert!(run.has_data_cache_protocol(RiscvDataCacheProtocol::Moesi));
    assert!(run.has_data_cache_protocol(RiscvDataCacheProtocol::Chi));
    assert_eq!(
        run.data_cache_runs_for_protocol(RiscvDataCacheProtocol::Msi),
        vec![msi_run],
    );
    assert_eq!(
        run.data_cache_runs_for_protocol(RiscvDataCacheProtocol::Mesi),
        vec![mesi_run],
    );
    assert_eq!(
        run.data_cache_runs_for_protocol(RiscvDataCacheProtocol::Moesi),
        vec![moesi_run],
    );
    assert_eq!(
        run.data_cache_runs_for_protocol(RiscvDataCacheProtocol::Chi),
        vec![chi_run],
    );
    assert_eq!(run.data_cache_parallel_run_history().run_count(), 4);
    assert_eq!(
        run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi)
            .run_count(),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Mesi)
            .run_count(),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Moesi)
            .run_count(),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Chi)
            .run_count(),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_count_for_protocol(RiscvDataCacheProtocol::Mesi),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_count_for_protocol(RiscvDataCacheProtocol::Moesi),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_run_count_for_protocol(RiscvDataCacheProtocol::Chi),
        1,
    );
    assert!(run.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi));
    assert!(run.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Mesi));
    assert!(run.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Moesi));
    assert!(run.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Chi));
    assert!(run
        .data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Moesi)
        .has_wait_for_edges());
    assert_eq!(
        run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi)
            .total_cpu_responses(),
        0,
    );
    let histories = run.data_cache_parallel_run_histories_by_protocol();
    assert_eq!(
        histories
            .get(&RiscvDataCacheProtocol::Msi)
            .unwrap()
            .run_count(),
        1,
    );
    assert_eq!(
        histories
            .get(&RiscvDataCacheProtocol::Mesi)
            .unwrap()
            .run_count(),
        1,
    );
    assert_eq!(
        histories
            .get(&RiscvDataCacheProtocol::Moesi)
            .unwrap()
            .run_count(),
        1,
    );
    assert_eq!(
        histories
            .get(&RiscvDataCacheProtocol::Chi)
            .unwrap()
            .run_count(),
        1,
    );
    assert_eq!(histories.len(), 4);
    let history_records = run.data_cache_parallel_run_history_records();
    assert_eq!(
        history_records,
        vec![
            RiscvDataCacheRunHistoryRecord::new(
                RiscvDataCacheProtocol::Msi,
                run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi),
            ),
            RiscvDataCacheRunHistoryRecord::new(
                RiscvDataCacheProtocol::Mesi,
                run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Mesi),
            ),
            RiscvDataCacheRunHistoryRecord::new(
                RiscvDataCacheProtocol::Moesi,
                run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Moesi),
            ),
            RiscvDataCacheRunHistoryRecord::new(
                RiscvDataCacheProtocol::Chi,
                run.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Chi),
            ),
        ],
    );
    assert_eq!(
        run.data_cache_parallel_run_history_record(RiscvDataCacheProtocol::Msi),
        Some(history_records[0].clone()),
    );
    assert_eq!(
        run.attributed_data_cache_parallel_run_history(),
        run.data_cache_parallel_run_history()
    );
    assert_eq!(
        run.unattributed_data_cache_parallel_run_history(),
        ParallelCoherenceRunHistory::default()
    );
    assert_eq!(run.attributed_data_cache_parallel_run_count(), 4);
    assert_eq!(run.unattributed_data_cache_parallel_run_count(), 0);
}

#[test]
fn topology_run_reports_fabric_and_dram_activity_for_fetch_window() {
    let source = GuestSourceId::new(91);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(91)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            30,
            |cpu| GuestEventId::new(910 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(910),
            source,
            0,
        )),
    );
    assert!(run.has_fabric_activity());
    assert!(run.active_fabric_lane_count() >= 1);
    assert_eq!(
        run.fabric_transfer_count(),
        run.fabric_profile().transfer_count(),
    );
    assert!(run
        .fabric_activity(
            &rem6_fabric::FabricLinkId::new("cpu_mem").unwrap(),
            rem6_fabric::VirtualNetworkId::new(1),
        )
        .is_some());
    assert!(run.has_dram_activity());
    assert_eq!(run.active_dram_target_count(), 1);
    assert_eq!(run.dram_profile().access_count(), 1);
    assert_eq!(run.dram_profile().read_count(), 1);
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );
    assert!(run.has_resource_activity());
    assert_eq!(
        run.resource_activity_count(),
        run.fabric_transfer_count() + run.dram_operation_count() + run.fabric_wait_for_edge_count(),
    );
}

#[test]
fn system_run_preserves_external_memory_profile_when_merging_dram_activity() {
    let target = MemoryTargetId::new(3);
    let profile = ExternalMemoryProfile::nvm(
        target,
        layout(),
        2,
        8,
        DramGeometry::new(4, 64, layout().bytes()).unwrap(),
        DramTiming::new(4, 8, 12, 3, 5).unwrap(),
    )
    .unwrap();
    let activity = DramTargetActivity::new(target, DramActivityProfile::default())
        .with_memory_profile(profile);
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    )
    .with_dram_activity(vec![activity.clone(), activity]);

    let merged = run.dram_target_activity(target).unwrap();

    assert_eq!(merged.memory_profile(), Some(&profile));
    assert_eq!(
        merged.memory_profile().unwrap().technology(),
        DramMemoryTechnology::Nvm,
    );
}

#[test]
fn topology_run_reports_fabric_wait_for_for_contended_fetches() {
    let source = GuestSourceId::new(94);
    let system = RiscvTopologySystem::with_min_remote_delay(
        contended_cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([
            core_config_on(0, 0, 94, "cpu0"),
            core_config_on(1, 1, 95, "cpu1"),
        ]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            30,
            |cpu| GuestEventId::new(940 + u64::from(cpu.get())),
        )
        .unwrap();

    assert!(run.has_fabric_activity());
    assert!(run.has_fabric_wait_for_edges());
    assert!(run.fabric_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue) >= 1);
    assert!(!run.fabric_wait_for_blocked_nodes().is_empty());
    assert!(run.fabric_first_wait_tick().unwrap() <= run.fabric_last_wait_tick().unwrap());
    assert!(run.has_dram_wait_for_edges());
    assert!(run.dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue) >= 1);
    assert!(!run.dram_wait_for_blocked_nodes().is_empty());
    assert!(run.dram_first_wait_tick().unwrap() <= run.dram_last_wait_tick().unwrap());
    assert_eq!(
        run.resource_activity_count(),
        run.fabric_transfer_count()
            + run.dram_operation_count()
            + run.fabric_wait_for_edge_count()
            + run.dram_wait_for_edge_count(),
    );
}

#[test]
fn topology_run_keeps_code_and_data_dram_targets_separate() {
    let source = GuestSourceId::new(92);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(92)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(split_dram_config(), &load_then_ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            40,
            |cpu| GuestEventId::new(920 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(run.active_dram_target_count(), 2);
    assert_eq!(run.dram_profile().access_count(), 3);
    assert_eq!(run.dram_profile().read_count(), 3);
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        2,
    );
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(1))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789,
    );
    assert_eq!(
        system.dram_activity_profile().unwrap().access_count(),
        run.dram_profile().access_count(),
    );
}

#[test]
fn topology_run_reports_fabric_without_dram_for_store_memory() {
    let source = GuestSourceId::new(93);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(93)]),
        2,
    )
    .unwrap()
    .with_boot_image_memory(memory_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            30,
            |cpu| GuestEventId::new(930 + u64::from(cpu.get())),
        )
        .unwrap();

    assert!(run.has_resource_activity());
    assert!(run.has_fabric_activity());
    assert!(!run.has_dram_activity());
    assert!(run.fabric_transfer_count() > 0);
    assert_eq!(run.dram_access_count(), 0);
    assert_eq!(run.dram_target_activities().len(), 0);
    assert_eq!(
        run.resource_activity_count(),
        run.fabric_transfer_count() + run.fabric_wait_for_edge_count(),
    );
    assert!(system.memory_store().is_some());
    assert!(system.dram_memory_controller().is_none());
}
