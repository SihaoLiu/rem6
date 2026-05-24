use rem6_boot::BootImage;
use rem6_cpu::CpuId;
use rem6_dram::{DramGeometry, DramTiming, ExternalMemoryProfile};
use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_isa_riscv::Register;
use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId};
use rem6_system::RiscvWorkloadReplay;
use rem6_workload::{
    HostEventIntent, WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadQosPolicy, WorkloadQosQueuePolicyKind,
    WorkloadQosTurnaroundPolicyKind, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadRiscvCore, WorkloadRouteFabric, WorkloadRouteHop,
    WorkloadRouteId, WorkloadTopology,
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

fn boot_image_with_same_tick_data_read_write() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(0x7b, 0, 0x0, 3, 0x13)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(s_type(8, 3, 2, 0x3, 0x23)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x8010), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8014), word(i_type(0, 0, 0x0, 0, 0x13)))
        .unwrap()
        .add_segment(Address::new(0x8018), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x801c), word(0x0010_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn dram_geometry() -> DramGeometry {
    DramGeometry::new(2, 64, layout().bytes()).unwrap()
}

fn dram_timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5).unwrap()
}

fn single_channel_ddr_profile(target: u32) -> ExternalMemoryProfile {
    ExternalMemoryProfile::ddr(
        MemoryTargetId::new(target),
        layout(),
        1,
        1,
        dram_geometry(),
        dram_timing(),
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

fn replay_topology_with_qos_fabric_fetches() -> WorkloadTopology {
    let shared_fabric = WorkloadRouteFabric::new("qos_fetch", 4)
        .unwrap()
        .with_virtual_networks(1, 2)
        .with_credit_depth(1)
        .unwrap();
    let policy = WorkloadQosPolicy::new(4, QosPriority::new(1))
        .unwrap()
        .with_queue_policy(WorkloadQosQueuePolicyKind::Lifo)
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(1))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(8), QosPriority::new(1))
        .unwrap();

    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                layout().bytes(),
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap()
                .with_fabric(shared_fabric.clone()),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
                .unwrap()
                .with_fabric(shared_fabric),
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
        .with_qos_policy(policy)
}

fn replay_topology_with_qos_dram_fetches() -> WorkloadTopology {
    let policy = WorkloadQosPolicy::new(4, QosPriority::new(3))
        .unwrap()
        .with_queue_policy(WorkloadQosQueuePolicyKind::Lifo)
        .with_turnaround_policy(WorkloadQosTurnaroundPolicyKind::PreferCurrentDirection)
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(2))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(8), QosPriority::new(0))
        .unwrap();

    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                layout().bytes(),
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap()
            .with_external_memory_profile(single_channel_ddr_profile(0))
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
        .with_qos_policy(policy)
}

fn replay_topology_with_multihop_qos_dram_fetches() -> WorkloadTopology {
    let policy = WorkloadQosPolicy::new(4, QosPriority::new(3))
        .unwrap()
        .with_queue_policy(WorkloadQosQueuePolicyKind::Fifo)
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(2))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(8), QosPriority::new(0))
        .unwrap();

    WorkloadTopology::new(5, 2, 2, WorkloadHostPlacement::new(4, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                layout().bytes(),
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap()
            .with_external_memory_profile(single_channel_ddr_profile(0))
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new_path(
                route_id("cpu0.fetch"),
                "cpu0.ifetch",
                0,
                [
                    WorkloadRouteHop::new("router.cpu0", 3, 2, 2).unwrap(),
                    WorkloadRouteHop::new("memory", 2, 2, 3).unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new_path(
                route_id("cpu1.fetch"),
                "cpu1.ifetch",
                1,
                [
                    WorkloadRouteHop::new("router.cpu1", 3, 2, 2).unwrap(),
                    WorkloadRouteHop::new("memory", 2, 2, 3).unwrap(),
                ],
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
        .with_qos_policy(policy)
}

fn replay_topology_with_qos_dram_data_read_write() -> WorkloadTopology {
    let policy = WorkloadQosPolicy::new(4, QosPriority::new(1))
        .unwrap()
        .with_queue_policy(WorkloadQosQueuePolicyKind::Fifo)
        .with_turnaround_policy(WorkloadQosTurnaroundPolicyKind::PreferCurrentDirection)
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(1))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(8), QosPriority::new(0))
        .unwrap();

    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                layout().bytes(),
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x1000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                1,
                layout().bytes(),
                AddressRange::new(Address::new(0x9000), AccessSize::new(0x1000).unwrap()).unwrap(),
            )
            .unwrap()
            .with_external_memory_profile(single_channel_ddr_profile(1))
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 6, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.data"), "cpu1.dmem", 1, "memory", 2, 2, 3)
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
        .add_riscv_core(
            WorkloadRiscvCore::new(
                1,
                1,
                8,
                Address::new(0x8010),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap()
            .with_data("cpu1.dmem", route_id("cpu1.data"))
            .unwrap(),
        )
        .unwrap()
        .with_qos_policy(policy)
}

fn replay_manifest_with_qos_fabric_fetches() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("qos-fabric-fetches"), boot_image())
        .with_topology(replay_topology_with_qos_fabric_fetches())
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

fn replay_manifest_with_qos_dram_fetches() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("qos-dram-fetches"), boot_image())
        .with_topology(replay_topology_with_qos_dram_fetches())
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

fn replay_manifest_with_multihop_qos_dram_fetches() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("multihop-qos-dram-fetches"), boot_image())
        .with_topology(replay_topology_with_multihop_qos_dram_fetches())
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

fn replay_manifest_with_qos_dram_data_read_write() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("qos-dram-data-read-write"),
        boot_image_with_same_tick_data_read_write(),
    )
    .with_topology(replay_topology_with_qos_dram_data_read_write())
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

#[test]
fn workload_replay_applies_declared_qos_policy_to_fabric_fetch_order() {
    let manifest = replay_manifest_with_qos_fabric_fetches();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.run().scheduled_traps()[0].cpu(), CpuId::new(1));
    assert!(outcome
        .result()
        .parallel_execution_summary()
        .unwrap()
        .has_fabric_contention());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_applies_declared_qos_policy_to_dram_accesses() {
    let manifest = replay_manifest_with_qos_dram_fetches();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.run().dram_qos_access_count(), 2);
    assert_eq!(outcome.run().dram_qos_byte_count(), 8);
    assert_eq!(
        outcome
            .run()
            .dram_qos_priority_access_count(QosPriority::new(0)),
        1
    );
    assert_eq!(
        outcome
            .run()
            .dram_qos_priority_access_count(QosPriority::new(2)),
        1
    );
    assert_eq!(
        outcome
            .run()
            .dram_qos_requestor_access_count(QosRequestorId::new(7)),
        1
    );
    assert_eq!(
        outcome
            .run()
            .dram_qos_requestor_access_count(QosRequestorId::new(8)),
        1
    );

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.dram_qos_access_count(), 2);
    assert_eq!(summary.dram_qos_byte_count(), 8);
    assert_eq!(
        summary.dram_qos_priority_access_count(QosPriority::new(0)),
        1
    );
    assert_eq!(
        summary.dram_qos_priority_access_count(QosPriority::new(2)),
        1
    );
    assert_eq!(
        summary.dram_qos_requestor_access_count(QosRequestorId::new(7)),
        1
    );
    assert_eq!(
        summary.dram_qos_requestor_access_count(QosRequestorId::new(8)),
        1
    );
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_batches_same_tick_dram_accesses_before_qos_arbitration() {
    let manifest = replay_manifest_with_qos_dram_fetches();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.run().scheduled_traps()[0].cpu(), CpuId::new(1));
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_orders_multihop_same_tick_dram_accesses_before_target_delivery() {
    let manifest = replay_manifest_with_multihop_qos_dram_fetches();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.run().scheduled_traps()[0].cpu(), CpuId::new(1));
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_coalesces_same_tick_dram_target_deliveries_into_controller_batch() {
    let manifest = replay_manifest_with_qos_dram_data_read_write();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(256)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    plan.verify_result(outcome.result()).unwrap();
}
