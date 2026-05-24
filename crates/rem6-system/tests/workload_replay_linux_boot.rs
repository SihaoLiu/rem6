use rem6_boot::BootImage;
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId, PartitionedMemorySnapshot,
};
use rem6_system::RiscvWorkloadReplay;
use rem6_workload::{
    HostEventIntent, WorkloadHostEvent, WorkloadHostPlacement, WorkloadId,
    WorkloadLinuxBootHandoff, WorkloadLinuxInitrd, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResolvedResources, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadResourcePayload, WorkloadRiscvCore,
    WorkloadRouteId, WorkloadTopology,
};

fn workload_id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9000), word(0x0010_0073))
        .unwrap()
}

fn initrd_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("initrd"),
        WorkloadResourceKind::Initrd,
        "sha256:initrd",
        "resources/initrd.cpio",
    )
    .unwrap()
}

fn replay_topology() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
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
                route_id("cpu0.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
        .add_riscv_core(
            WorkloadRiscvCore::new(
                1,
                1,
                8,
                Address::new(0x9000),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_manifest_with_linux_initrd_handoff() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-linux-initrd-replay"), boot_image())
        .with_topology(replay_topology())
        .add_resource(initrd_resource())
        .unwrap()
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x97c0))
                .with_bootargs("console=ttyS0 root=/dev/vda")
                .with_initrd(
                    WorkloadLinuxInitrd::new(
                        resource_id("initrd"),
                        Address::new(0x9804),
                        AccessSize::new(20).unwrap(),
                    )
                    .unwrap(),
                ),
        )
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn snapshot_blob(
    snapshot: &PartitionedMemorySnapshot,
    target: MemoryTargetId,
    start: Address,
    len: usize,
) -> Vec<u8> {
    let partition = snapshot
        .partitions()
        .iter()
        .find(|partition| partition.target() == target)
        .unwrap();
    let mut cursor = start.get();
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        let address = Address::new(cursor);
        let line_address = layout().line_address(address);
        let offset = layout().line_offset(address) as usize;
        let line = partition
            .lines()
            .iter()
            .find(|line| line.line() == line_address)
            .unwrap();
        let take = (line.data().len() - offset).min(len - bytes.len());
        bytes.extend_from_slice(&line.data()[offset..offset + take]);
        cursor += take as u64;
    }
    bytes
}

#[test]
fn workload_replay_installs_resolved_linux_initrd_payload() {
    let manifest = replay_manifest_with_linux_initrd_handoff();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let initrd_data = (0..20).map(|byte| 0xa0 + byte as u8).collect::<Vec<_>>();
    let resources =
        WorkloadResolvedResources::from_manifest(
            &manifest,
            [WorkloadResourcePayload::new(
                resource_id("initrd"),
                "sha256:initrd",
                initrd_data.clone(),
            )
            .unwrap()],
        )
        .unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_resolved_resources(resources)
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(
        snapshot_blob(
            outcome.memory_snapshot(),
            MemoryTargetId::new(0),
            Address::new(0x9804),
            initrd_data.len(),
        ),
        initrd_data
    );
    plan.verify_result(outcome.result()).unwrap();
}
