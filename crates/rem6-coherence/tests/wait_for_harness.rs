use rem6_coherence::{
    HarnessError, MesiHarnessError, PartitionedCacheAgentConfig, PartitionedDirectoryLineHarness,
    PartitionedDramMemoryConfig, PartitionedMesiDirectoryLineHarness,
};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_kernel::{PartitionId, WaitForEdgeKind, WaitForNode};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId,
};
use rem6_protocol_mesi::MesiState;
use rem6_protocol_msi::MsiState;
use rem6_transport::TransportEndpointId;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn dram_target() -> MemoryTargetId {
    MemoryTargetId::new(0)
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn request_node(agent: u32, sequence: u64) -> WaitForNode {
    WaitForNode::transaction(format!("memory.{agent}.{sequence}")).unwrap()
}

fn cache_node(agent: u32, line_address: u64) -> WaitForNode {
    WaitForNode::resource(format!("cache.{agent}.line.{line_address:x}")).unwrap()
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn read(agent: u32, sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent, sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(agent: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        request_id(agent, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn cache_config(
    agent: u32,
    partition: u32,
    endpoint_name: &str,
    request_latency: u64,
    response_latency: u64,
) -> PartitionedCacheAgentConfig {
    PartitionedCacheAgentConfig::new(
        AgentId::new(agent),
        PartitionId::new(partition),
        endpoint(endpoint_name),
        request_latency,
        response_latency,
    )
}

fn dram_memory(line_address: u64, mapped_size: u64) -> DramMemoryController {
    let target = dram_target();
    let mut memory = DramMemoryController::new();
    memory
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            DramGeometry::new(4, 256, 64).unwrap(),
            DramTiming::new(3, 5, 7, 2, 4).unwrap(),
        ))
        .unwrap();
    memory
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(mapped_size).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(target, Address::new(line_address), line_data())
        .unwrap();
    memory
}

fn msi_harness_with_dram_memory() -> PartitionedDirectoryLineHarness {
    PartitionedDirectoryLineHarness::new_with_dram_memory(
        layout(),
        Address::new(0x1000),
        PartitionId::new(2),
        endpoint("msi_dir0"),
        PartitionedDramMemoryConfig::new(
            PartitionId::new(3),
            endpoint("msi_mem0"),
            7,
            11,
            dram_memory(0x1000, 0x4000),
        ),
        [
            cache_config(1, 0, "msi_l1d0", 3, 5),
            cache_config(2, 1, "msi_l1d1", 3, 5),
        ],
    )
    .unwrap()
}

fn mesi_harness_with_dram_memory() -> PartitionedMesiDirectoryLineHarness {
    PartitionedMesiDirectoryLineHarness::new_with_dram_memory(
        layout(),
        Address::new(0x3000),
        PartitionId::new(2),
        endpoint("mesi_dir0"),
        PartitionedDramMemoryConfig::new(
            PartitionId::new(4),
            endpoint("mesi_mem0"),
            7,
            11,
            dram_memory(0x3000, 0x8000),
        ),
        [
            cache_config(1, 0, "mesi_l1d0", 3, 9),
            cache_config(2, 1, "mesi_l1d1", 5, 7),
            cache_config(3, 3, "mesi_l1d2", 2, 4),
        ],
    )
    .unwrap()
}

#[test]
fn msi_parallel_busy_line_records_wait_for_edge_until_fill() {
    let mut harness = msi_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x1000, 4))
        .unwrap();
    let blocked = read(1, 1, 0x1008, 4);
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), blocked)
            .unwrap_err(),
        HarnessError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );

    let graph = harness.wait_for_graph();
    let source = request_node(1, 1);
    let target = cache_node(1, 0x1000);
    let dependencies = graph.dependencies(&source);
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].target(), &target);
    assert_eq!(dependencies[0].kind(), WaitForEdgeKind::Queue);
    assert_eq!(dependencies[0].first_observed_tick(), 0);
    assert_eq!(dependencies[0].last_observed_tick(), 0);
    assert_eq!(dependencies[0].observation_count(), 1);
    assert_eq!(graph.deadlock_diagnostic(), None);

    harness.run_until_idle_parallel_recorded().unwrap();
    assert!(harness.wait_for_graph().is_empty());
}

#[test]
fn msi_parallel_repeated_busy_line_updates_wait_observation() {
    let mut harness = msi_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 10, 0x1000, 4))
        .unwrap();
    let blocked = read(1, 11, 0x1008, 4);
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), blocked.clone())
            .unwrap_err(),
        HarnessError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), blocked)
            .unwrap_err(),
        HarnessError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );

    let graph = harness.wait_for_graph();
    let dependencies = graph.dependencies(&request_node(1, 11));
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].target(), &cache_node(1, 0x1000));
    assert_eq!(dependencies[0].first_observed_tick(), 0);
    assert_eq!(dependencies[0].last_observed_tick(), 0);
    assert_eq!(dependencies[0].observation_count(), 2);

    harness.run_until_idle_parallel_recorded().unwrap();
    assert!(harness.wait_for_graph().is_empty());
}

#[test]
fn msi_parallel_fill_clears_all_waiters_for_busy_line() {
    let mut harness = msi_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 20, 0x1000, 4))
        .unwrap();
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), read(1, 21, 0x1008, 4))
            .unwrap_err(),
        HarnessError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), write(1, 22, 0x1004, vec![0xaa]))
            .unwrap_err(),
        HarnessError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );

    let graph = harness.wait_for_graph();
    let target = cache_node(1, 0x1000);
    assert_eq!(graph.dependencies(&request_node(1, 21)).len(), 1);
    assert_eq!(graph.dependencies(&request_node(1, 22)).len(), 1);
    assert_eq!(graph.dependents(&target).len(), 2);

    harness.run_until_idle_parallel_recorded().unwrap();
    assert!(harness.wait_for_graph().is_empty());
}

#[test]
fn mesi_parallel_busy_line_records_wait_for_edge_until_fill() {
    let mut harness = mesi_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 20, 0x3000, 4))
        .unwrap();
    let blocked = read(1, 21, 0x3008, 4);
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), blocked)
            .unwrap_err(),
        MesiHarnessError::LineBusy {
            state: MesiState::InvalidToExclusive
        }
    );

    let graph = harness.wait_for_graph();
    let source = request_node(1, 21);
    let target = cache_node(1, 0x3000);
    let dependencies = graph.dependencies(&source);
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].target(), &target);
    assert_eq!(dependencies[0].kind(), WaitForEdgeKind::Queue);
    assert_eq!(dependencies[0].first_observed_tick(), 0);
    assert_eq!(dependencies[0].last_observed_tick(), 0);
    assert_eq!(dependencies[0].observation_count(), 1);
    assert_eq!(graph.deadlock_diagnostic(), None);

    harness.run_until_idle_parallel_recorded().unwrap();
    assert!(harness.wait_for_graph().is_empty());
}

#[test]
fn mesi_parallel_repeated_busy_line_updates_wait_observation() {
    let mut harness = mesi_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 30, 0x3000, 4))
        .unwrap();
    let blocked = read(1, 31, 0x3008, 4);
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), blocked.clone())
            .unwrap_err(),
        MesiHarnessError::LineBusy {
            state: MesiState::InvalidToExclusive
        }
    );
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), blocked)
            .unwrap_err(),
        MesiHarnessError::LineBusy {
            state: MesiState::InvalidToExclusive
        }
    );

    let graph = harness.wait_for_graph();
    let dependencies = graph.dependencies(&request_node(1, 31));
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].target(), &cache_node(1, 0x3000));
    assert_eq!(dependencies[0].first_observed_tick(), 0);
    assert_eq!(dependencies[0].last_observed_tick(), 0);
    assert_eq!(dependencies[0].observation_count(), 2);

    harness.run_until_idle_parallel_recorded().unwrap();
    assert!(harness.wait_for_graph().is_empty());
}

#[test]
fn mesi_parallel_fill_clears_all_waiters_for_busy_line() {
    let mut harness = mesi_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 40, 0x3000, 4))
        .unwrap();
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), read(1, 41, 0x3008, 4))
            .unwrap_err(),
        MesiHarnessError::LineBusy {
            state: MesiState::InvalidToExclusive
        }
    );
    assert_eq!(
        harness
            .submit_cpu_request_parallel(agent(1), write(1, 42, 0x3004, vec![0xaa]))
            .unwrap_err(),
        MesiHarnessError::LineBusy {
            state: MesiState::InvalidToExclusive
        }
    );

    let graph = harness.wait_for_graph();
    let target = cache_node(1, 0x3000);
    assert_eq!(graph.dependencies(&request_node(1, 41)).len(), 1);
    assert_eq!(graph.dependencies(&request_node(1, 42)).len(), 1);
    assert_eq!(graph.dependents(&target).len(), 2);

    harness.run_until_idle_parallel_recorded().unwrap();
    assert!(harness.wait_for_graph().is_empty());
}
