use rem6_boot::BootImage;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::RiscvWorkloadReplay;
use rem6_workload::{
    HostEventIntent, WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadRiscvCore, WorkloadRouteId, WorkloadTopology,
};

fn workload_id(value: &str) -> rem6_workload::WorkloadId {
    rem6_workload::WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
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

fn u_type(imm: u32, rd: u8, opcode: u32) -> u32 {
    (imm & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn boot_image_with_data_load() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_repeated_data_load() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(i_type(8, 2, 0x3, 6, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_adjacent_target_lines() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(i_type(24, 2, 0x3, 6, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
        .add_segment(
            Address::new(0x9018),
            0x0123_4567_89ab_cdef_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_wide_target_line_reuse() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0xa000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(i_type(24, 2, 0x3, 6, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0xa008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
        .add_segment(
            Address::new(0xa018),
            0x0123_4567_89ab_cdef_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn replay_topology_with_data_route() -> WorkloadTopology {
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
            WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 2, 3)
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
            .unwrap()
            .with_data("cpu0.dmem", route_id("cpu0.data"))
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_mixed_memory_line_sizes() -> WorkloadTopology {
    replay_topology_with_data_route()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                1,
                32,
                AddressRange::new(Address::new(0xa000), AccessSize::new(0x1000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
}

fn kernel_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        "sha256:kernel",
        "resources/kernel.elf",
    )
    .unwrap()
}

fn replay_manifest_with_boot_image(
    boot_image: BootImage,
    topology: WorkloadTopology,
) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-data-probe-load"), boot_image)
        .with_topology(topology)
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn replay_manifest_with_data_load() -> WorkloadManifest {
    replay_manifest_with_boot_image(
        boot_image_with_data_load(),
        replay_topology_with_data_route(),
    )
}

fn replay_manifest_with_repeated_data_load() -> WorkloadManifest {
    replay_manifest_with_boot_image(
        boot_image_with_repeated_data_load(),
        replay_topology_with_data_route(),
    )
}

fn replay_manifest_with_adjacent_target_lines() -> WorkloadManifest {
    replay_manifest_with_boot_image(
        boot_image_with_adjacent_target_lines(),
        replay_topology_with_mixed_memory_line_sizes(),
    )
}

fn replay_manifest_with_wide_target_line_reuse() -> WorkloadManifest {
    replay_manifest_with_boot_image(
        boot_image_with_wide_target_line_reuse(),
        replay_topology_with_mixed_memory_line_sizes(),
    )
}

#[test]
fn workload_replay_records_riscv_data_access_probe_evidence() {
    let manifest = replay_manifest_with_data_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(64)
        .run_parallel()
        .unwrap();

    let probes = outcome
        .run()
        .data_access_probes()
        .expect("workload replay should carry data access probe evidence");
    assert_eq!(probes.stack_distance().config().line_size(), 16);
    assert_eq!(probes.stack_distance().config().system_line_size(), 16);
    assert_eq!(probes.stack_distance().infinite_samples(), 1);
    assert_eq!(probes.stack_distance().finite_samples(), 0);
    assert_eq!(probes.stack_distance().stack(), &[0x9000]);
    assert_eq!(probes.probes().events().len(), 1);

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.riscv_data_access_probe_sample_count(), 1);
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_infinite_samples(),
        1
    );
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_finite_samples(),
        0
    );
    assert_eq!(summary.riscv_data_access_probe_stack_depth(), 1);
    assert!(summary.has_riscv_data_access_probe_activity());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_records_riscv_data_access_probe_reuse_samples() {
    let manifest = replay_manifest_with_repeated_data_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(64)
        .run_parallel()
        .unwrap();

    let probes = outcome
        .run()
        .data_access_probes()
        .expect("workload replay should carry data access probe evidence");
    assert_eq!(probes.stack_distance().infinite_samples(), 1);
    assert_eq!(probes.stack_distance().finite_samples(), 1);
    assert_eq!(probes.stack_distance().stack(), &[0x9000]);
    assert_eq!(probes.probes().events().len(), 2);

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.riscv_data_access_probe_sample_count(), 2);
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_infinite_samples(),
        1
    );
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_finite_samples(),
        1
    );
    assert_eq!(summary.riscv_data_access_probe_stack_depth(), 1);
    assert!(summary.has_riscv_data_access_probe_activity());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_records_target_line_boundaries_for_data_access_probes() {
    let manifest = replay_manifest_with_adjacent_target_lines();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(64)
        .run_parallel()
        .unwrap();

    let probes = outcome
        .run()
        .data_access_probes()
        .expect("workload replay should carry data access probe evidence");
    assert_eq!(probes.stack_distance().config().line_size(), 16);
    assert_eq!(probes.stack_distance().config().system_line_size(), 16);
    assert_eq!(probes.stack_distance().infinite_samples(), 2);
    assert_eq!(probes.stack_distance().finite_samples(), 0);
    assert_eq!(probes.stack_distance().stack(), &[0x9010, 0x9000]);
    assert_eq!(probes.probes().events().len(), 2);

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.riscv_data_access_probe_sample_count(), 2);
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_infinite_samples(),
        2
    );
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_finite_samples(),
        0
    );
    assert_eq!(summary.riscv_data_access_probe_stack_depth(), 2);
    assert!(summary.has_riscv_data_access_probe_activity());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_records_wide_target_line_reuse_for_data_access_probes() {
    let manifest = replay_manifest_with_wide_target_line_reuse();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(64)
        .run_parallel()
        .unwrap();

    let probes = outcome
        .run()
        .data_access_probes()
        .expect("workload replay should carry data access probe evidence");
    assert_eq!(probes.stack_distance().config().line_size(), 16);
    assert_eq!(probes.stack_distance().config().system_line_size(), 16);
    assert_eq!(probes.stack_distance().infinite_samples(), 1);
    assert_eq!(probes.stack_distance().finite_samples(), 1);
    assert_eq!(probes.stack_distance().stack(), &[0xa000]);
    assert_eq!(probes.probes().events().len(), 2);

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.riscv_data_access_probe_sample_count(), 2);
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_infinite_samples(),
        1
    );
    assert_eq!(
        summary.riscv_data_access_probe_stack_distance_finite_samples(),
        1
    );
    assert_eq!(summary.riscv_data_access_probe_stack_depth(), 1);
    assert!(summary.has_riscv_data_access_probe_activity());
    plan.verify_result(outcome.result()).unwrap();
}
