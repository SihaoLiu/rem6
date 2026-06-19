use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_coherence::{
    HarnessError, PartitionedCacheAgentConfig, PartitionedChiDirectoryLineHarness,
    PartitionedDirectoryLineHarness, PartitionedMesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarness,
};
use rem6_memory::MemoryOperation;
use rem6_workload::{
    WorkloadAcceleratorDevice, WorkloadAcceleratorDmaCopy, WorkloadDataCacheProtocol,
    WorkloadGpuDevice, WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryTarget,
    WorkloadRiscvCore, WorkloadRiscvDataCache,
};

use super::*;

fn assert_response_harness<H: cache_response::WorkloadDataCacheResponseHarness>() {}

fn test_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn test_route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn test_workload_id(value: &str) -> rem6_workload::WorkloadId {
    rem6_workload::WorkloadId::new(value).unwrap()
}

fn test_boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), 0x0000_0073_u32.to_le_bytes().to_vec())
        .unwrap()
        .add_segment(Address::new(0x9024), vec![0x3a, 0x4b, 0x5c, 0x6d])
        .unwrap()
        .add_segment(Address::new(0x9048), vec![0; 4])
        .unwrap()
}

fn test_manifest(id: &str, topology: WorkloadTopology) -> WorkloadManifest {
    WorkloadManifest::builder(test_workload_id(id), test_boot_image())
        .with_topology(topology)
        .build()
        .unwrap()
}

fn common_dma_topology() -> WorkloadTopology {
    WorkloadTopology::new(5, 2, 4, WorkloadHostPlacement::new(4, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                rem6_memory::AddressRange::new(
                    Address::new(0x8000),
                    AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                test_route_id("cpu0.fetch"),
                "cpu0.ifetch",
                0,
                "memory",
                2,
                3,
                3,
            )
            .unwrap(),
        )
        .unwrap()
        .add_riscv_core(
            WorkloadRiscvCore::new(
                0,
                0,
                7,
                Address::new(0x8000),
                "cpu0.ifetch",
                test_route_id("cpu0.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn gpu_dma_topology() -> WorkloadTopology {
    common_dma_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                test_route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(test_route_id("gpu0.dma"), "gpu0.dma", 3, "memory", 2, 3, 5)
                .unwrap(),
        )
        .unwrap()
        .add_gpu_device(
            WorkloadGpuDevice::new(
                12,
                3,
                2,
                1,
                "gpu0.control",
                "gpu0.dma",
                test_route_id("gpu0.command"),
            )
            .unwrap(),
        )
        .unwrap()
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                12,
                200,
                test_route_id("gpu0.dma"),
                77,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap()
}

fn accelerator_dma_topology() -> WorkloadTopology {
    common_dma_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                test_route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                test_route_id("accelerator0.dma"),
                "accelerator0.dma",
                3,
                "memory",
                2,
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                "accelerator0.dma",
                test_route_id("accelerator0.command"),
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_dma_copy(
            WorkloadAcceleratorDmaCopy::new(
                22,
                300,
                test_route_id("accelerator0.dma"),
                88,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap()
}

fn mismatched_data_cache() -> Option<Arc<Mutex<WorkloadDataCacheBackend>>> {
    let config = WorkloadRiscvDataCache::new(
        WorkloadDataCacheProtocol::Msi,
        0,
        Address::new(0x9020),
        2,
        "dcache.dir",
        test_route_id("dcache.backing"),
    )
    .unwrap();
    let agent = PartitionedCacheAgentConfig::new(
        AgentId::new(1),
        PartitionId::new(1),
        TransportEndpointId::new("agent1.dma").unwrap(),
        2,
        3,
    );
    let line = WorkloadDataCacheLineBackend::new(
        &config,
        test_layout(),
        Address::new(0x9020),
        WorkloadDataCacheLineMemory::Line((0..16).collect()),
        vec![agent],
    )
    .unwrap();

    Some(Arc::new(Mutex::new(WorkloadDataCacheBackend::new([line]))))
}

fn assert_unknown_cache_controller_error(
    error: RiscvWorkloadReplayError,
    expected_agent: u32,
    expected_sequence: u64,
) {
    let RiscvWorkloadReplayError::DataCacheController { record } = error else {
        panic!("expected data-cache controller error");
    };
    assert_eq!(
        record.request_id(),
        Some(MemoryRequestId::new(
            AgentId::new(expected_agent),
            expected_sequence
        ))
    );
    assert_eq!(record.protocol(), crate::RiscvDataCacheProtocol::Msi);
    assert_eq!(record.target(), MemoryTargetId::new(0));
    assert_eq!(record.address(), Address::new(0x9024));
    assert_eq!(record.line(), Address::new(0x9020));
    assert_eq!(record.operation(), MemoryOperation::ReadShared);
    assert!(matches!(
        record.error(),
        crate::RiscvDataCacheControllerError::Msi(HarnessError::UnknownCache { agent })
            if *agent == AgentId::new(expected_agent)
    ));
}

#[test]
fn all_data_cache_protocol_harnesses_share_response_adapter() {
    assert_response_harness::<PartitionedDirectoryLineHarness>();
    assert_response_harness::<PartitionedMesiDirectoryLineHarness>();
    assert_response_harness::<PartitionedMoesiDirectoryLineHarness>();
    assert_response_harness::<PartitionedChiDirectoryLineHarness>();
}

#[test]
fn gpu_dma_copy_returns_data_cache_controller_error_before_missing_write() {
    let topology = gpu_dma_topology();
    let manifest = test_manifest("gpu-dma-controller-error-priority", topology.clone());
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let replay = RiscvWorkloadReplay::new(plan);
    let route_map = replay.build_route_map().unwrap();
    let transport = replay.build_transport().unwrap();
    let memory = replay.load_memory_backend().unwrap();
    let devices = build_gpu_devices(&topology).unwrap();
    let data_cache = mismatched_data_cache();

    let error = replay
        .run_gpu_dma_copies(
            &topology,
            &route_map,
            &devices,
            &transport,
            &memory,
            &data_cache,
        )
        .unwrap_err();

    assert_unknown_cache_controller_error(error, 77, 400);
}

#[test]
fn accelerator_dma_copy_returns_data_cache_controller_error_before_missing_write() {
    let topology = accelerator_dma_topology();
    let manifest = test_manifest(
        "accelerator-dma-controller-error-priority",
        topology.clone(),
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let replay = RiscvWorkloadReplay::new(plan);
    let route_map = replay.build_route_map().unwrap();
    let transport = replay.build_transport().unwrap();
    let memory = replay.load_memory_backend().unwrap();
    let devices = build_accelerator_devices(&topology).unwrap();
    let data_cache = mismatched_data_cache();

    let error = run_accelerator_dma_copies(
        &topology,
        &route_map,
        &devices,
        &transport,
        &memory,
        &data_cache,
    )
    .unwrap_err();

    assert_unknown_cache_controller_error(error, 88, 600);
}
