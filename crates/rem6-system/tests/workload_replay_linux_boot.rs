use rem6_boot::BootImage;
use rem6_cpu::{CpuId, RiscvHartRunState};
use rem6_isa_riscv::Register;
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId, PartitionedMemorySnapshot,
};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadReplayError};
use rem6_workload::{
    HostEventIntent, WorkloadError, WorkloadHostEvent, WorkloadHostPlacement, WorkloadId,
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

fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    (((imm as u32) & 0x0fff) << 20) | (rs1 << 15) | (rd << 7) | 0x13
}

fn lui(rd: u32, imm20: u32) -> u32 {
    (imm20 << 12) | (rd << 7) | 0x37
}

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0010_0073))
        .unwrap()
        .add_segment(Address::new(0x9000), word(0x0010_0073))
        .unwrap()
}

fn sbi_boot_image() -> BootImage {
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(0x4442_434e);
    let mut image = BootImage::new(Address::new(0x8000));
    for (address, instruction) in [
        (0x8000, addi(28, 10, 0)),
        (0x8004, addi(29, 11, 0)),
        (0x8008, addi(17, 0, 0x10)),
        (0x800c, addi(16, 0, 0)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(6, 11, 0)),
        (0x801c, lui(17, dbcn_hi)),
        (0x8020, addi(17, 17, dbcn_lo)),
        (0x8024, addi(16, 0, 2)),
        (0x8028, addi(10, 0, i32::from(b'B'))),
        (0x802c, 0x0000_0073),
        (0x8030, addi(7, 10, 0)),
        (0x8034, addi(8, 11, 0)),
        (0x8038, 0x0010_0073),
        (0x9000, addi(31, 0, 9)),
        (0x9004, 0x0010_0073),
    ] {
        image = image
            .add_segment(Address::new(address), word(instruction))
            .unwrap();
    }
    image
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

fn device_tree_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("dtb"),
        WorkloadResourceKind::DeviceTree,
        "sha256:dtb",
        "resources/riscv.dtb",
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

fn replay_manifest_with_linux_boot_handoff() -> WorkloadManifest {
    replay_manifest_with_bootargs("console=ttyS0 root=/dev/vda")
}

fn replay_manifest_with_bootargs(bootargs: &str) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-linux-initrd-replay"), boot_image())
        .with_topology(replay_topology())
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(initrd_resource())
        .unwrap()
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x97c0))
                .with_device_tree_resource(resource_id("dtb"))
                .with_bootargs(bootargs)
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

fn replay_manifest_with_sbi_linux_boot_handoff() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-linux-sbi-replay"), sbi_boot_image())
        .with_topology(replay_topology())
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(initrd_resource())
        .unwrap()
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x97c0))
                .with_device_tree_resource(resource_id("dtb"))
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
        .with_expected_stop_reason("host-stop")
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
fn workload_replay_installs_resolved_linux_boot_payloads() {
    let manifest = replay_manifest_with_linux_boot_handoff();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let dtb_data = vec![0xd0, 0x0d, 0xfe, 0xed, 0x00, 0x01];
    let initrd_data = (0..20).map(|byte| 0xa0 + byte as u8).collect::<Vec<_>>();
    let resources = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", dtb_data.clone())
                .unwrap(),
            WorkloadResourcePayload::new(
                resource_id("initrd"),
                "sha256:initrd",
                initrd_data.clone(),
            )
            .unwrap(),
        ],
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
            Address::new(0x97c0),
            dtb_data.len(),
        ),
        dtb_data
    );
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

#[test]
fn workload_replay_linux_boot_handoff_enters_supervisor_sbi() {
    let manifest = replay_manifest_with_sbi_linux_boot_handoff();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let resources = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0, 0x0d])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:initrd", vec![0; 20])
                .unwrap(),
        ],
    )
    .unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_resolved_resources(resources)
        .with_max_turns(160)
        .run_parallel()
        .unwrap();

    let boot = outcome.cluster().core(CpuId::new(0)).unwrap();
    let secondary = outcome.cluster().core(CpuId::new(1)).unwrap();

    assert_eq!(boot.read_register(Register::new(28).unwrap()), 0);
    assert_eq!(boot.read_register(Register::new(29).unwrap()), 0x97c0);
    assert_eq!(boot.read_register(Register::new(5).unwrap()), 0);
    assert_eq!(boot.read_register(Register::new(6).unwrap()), 2 << 24);
    assert_eq!(boot.read_register(Register::new(7).unwrap()), 0);
    assert_eq!(boot.read_register(Register::new(8).unwrap()), 0);
    assert_eq!(secondary.hart_run_state(), RiscvHartRunState::Stopped);
    assert_eq!(secondary.read_register(Register::new(31).unwrap()), 0);
    assert_eq!(outcome.run().scheduled_traps()[0].trap().pc(), 0x8038);
    assert_eq!(outcome.run().riscv_debug_console_bytes(), b"B");
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_rejects_resolved_payloads_from_different_manifest() {
    let manifest = replay_manifest_with_linux_boot_handoff();
    let other_manifest = replay_manifest_with_bootargs("console=ttyS1 root=/dev/vda");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let resources = WorkloadResolvedResources::from_manifest(
        &other_manifest,
        [
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0]).unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:initrd", vec![0; 20])
                .unwrap(),
        ],
    )
    .unwrap();

    let error = RiscvWorkloadReplay::new(plan.clone())
        .with_resolved_resources(resources)
        .with_max_turns(32)
        .run_parallel()
        .unwrap_err();

    assert_eq!(
        error,
        RiscvWorkloadReplayError::Workload(WorkloadError::ManifestIdentityMismatch {
            expected: plan.manifest_identity(),
            actual: other_manifest.identity(),
        })
    );
}
