use rem6_boot::BootImage;
use rem6_cpu::CpuId;
use rem6_isa_riscv::Register;
use rem6_memory::{AccessSize, Address, AddressRange, MemoryTargetId};
use rem6_system::{RiscvSystemRunStopReason, RiscvWorkloadReplay, SystemActionOutcome};
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

fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9000), word(0x0010_0073))
        .unwrap()
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

fn boot_image_with_data_store() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(0x7b, 0, 0x0, 3, 0x13)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(s_type(8, 3, 2, 0x3, 0x23)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9008), vec![0; 8])
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

fn replay_manifest() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay"), boot_image())
        .with_topology(replay_topology())
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
    WorkloadManifest::builder(
        workload_id("riscv-replay-data-load"),
        boot_image_with_data_load(),
    )
    .with_topology(replay_topology_with_data_route())
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

fn replay_manifest_with_data_store() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-data-store"),
        boot_image_with_data_store(),
    )
    .with_topology(replay_topology_with_data_route())
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

fn replay_manifest_with_planned_host_actions() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-host-actions"), boot_image())
        .with_topology(replay_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::RoiBegin {
                label: "roi".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::Checkpoint {
                label: "after-boot".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            2,
            HostEventIntent::RoiEnd {
                label: "roi".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

#[test]
fn workload_replay_plan_reconstructs_parallel_riscv_system_run() {
    let manifest = replay_manifest();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(20)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.result().manifest_identity(), manifest.identity());
    assert_eq!(
        outcome.result().final_tick(),
        outcome.run().final_tick().unwrap()
    );
    assert_eq!(outcome.result().stop_reason(), Some("host-stop"));
    assert_eq!(outcome.result().checkpoint_labels(), &[] as &[String]);
    plan.verify_result(outcome.result()).unwrap();

    assert!(matches!(
        outcome.run().stop_reason(),
        RiscvSystemRunStopReason::HostStop(_)
    ));
    assert_eq!(outcome.run().scheduled_traps().len(), 2);
    assert!(outcome.run().active_partition_count() >= 2);
    assert!(outcome.run().max_parallel_scheduler_workers() >= 1);
}

#[test]
fn workload_replay_executes_planned_host_actions() {
    let manifest = replay_manifest_with_planned_host_actions();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(20)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome.result().checkpoint_labels(),
        &["after-boot".to_string()]
    );
    let stats = outcome.result().stats_snapshot().unwrap();
    assert_eq!(stats.reset_tick(), 1);
    assert_eq!(stats.epoch(), 1);

    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::StatsReset(record)
            if record.tick() == 1 && record.epoch() == 1
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::StatsSnapshot(snapshot)
            if snapshot.tick() == 2 && snapshot.reset_tick() == 1
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::Checkpoint { tick, event, source, manifest }
            if *tick == 1
                && event.get() == 10_002
                && source.get() == 51
                && manifest.label() == "after-boot"
    )));
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_reconstructs_riscv_data_route() {
    let manifest = replay_manifest_with_data_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let cpu0 = CpuId::new(0);
    let activity = outcome.run().cpu_activity(cpu0).unwrap();
    assert_eq!(activity.data_access_issue_count(), 1);
    assert_eq!(
        outcome
            .cluster()
            .core(cpu0)
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    let data_events = outcome.cluster().core(cpu0).unwrap().data_access_events();
    assert_eq!(data_events.len(), 2);
    assert_eq!(data_events[0].endpoint().as_str(), "cpu0.dmem");
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_preserves_riscv_data_store_in_memory_snapshot() {
    let manifest = replay_manifest_with_data_store();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let cpu0 = CpuId::new(0);
    let activity = outcome.run().cpu_activity(cpu0).unwrap();
    assert_eq!(activity.data_access_issue_count(), 1);
    let partition = outcome
        .memory_snapshot()
        .partitions()
        .iter()
        .find(|partition| partition.target() == MemoryTargetId::new(0))
        .unwrap();
    let line = partition
        .lines()
        .iter()
        .find(|line| line.line() == Address::new(0x9000))
        .unwrap();
    assert_eq!(&line.data()[8..16], &0x7b_u64.to_le_bytes());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_rejects_manifest_without_topology() {
    let manifest = WorkloadManifest::builder(workload_id("missing-topology"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let error = RiscvWorkloadReplay::new(plan).run_parallel().unwrap_err();
    assert!(matches!(
        error,
        rem6_system::RiscvWorkloadReplayError::MissingTopology
    ));
}
