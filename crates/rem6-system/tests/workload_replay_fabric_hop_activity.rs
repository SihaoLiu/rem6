use rem6_boot::BootImage;
use rem6_fabric::VirtualNetworkId;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::RiscvWorkloadReplay;
use rem6_workload::{
    HostEventIntent, WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadResource, WorkloadResourceId,
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

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
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

fn multihop_fabric_topology() -> WorkloadTopology {
    let cpu_to_router = WorkloadRouteFabric::new("cpu_router", 4)
        .unwrap()
        .with_virtual_networks(1, 2)
        .with_credit_depth(2)
        .unwrap()
        .with_router_stage("router0", 0, 1, 0, 3)
        .unwrap();
    let router_to_memory = WorkloadRouteFabric::new("router_memory", 8)
        .unwrap()
        .with_virtual_networks(3, 4)
        .with_credit_depth(2)
        .unwrap();

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
            WorkloadMemoryRoute::new_path(
                route_id("cpu0.fetch"),
                "cpu0.ifetch",
                0,
                [
                    WorkloadRouteHop::new("router0.cpu", 1, 2, 2)
                        .unwrap()
                        .with_fabric(cpu_to_router),
                    WorkloadRouteHop::new("memory", 2, 2, 3)
                        .unwrap()
                        .with_fabric(router_to_memory),
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
}

fn multihop_fabric_manifest() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("fabric-hop-activity"), boot_image())
        .with_topology(multihop_fabric_topology())
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
fn workload_replay_summary_exposes_multihop_fabric_hop_activity() {
    let plan =
        rem6_workload::WorkloadReplayPlan::from_manifest(&multihop_fabric_manifest()).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    let hop_lanes = summary
        .fabric_hop_activities()
        .iter()
        .map(|activity| (activity.link().as_str(), activity.virtual_network()))
        .collect::<Vec<_>>();

    assert!(summary.fabric_hop_activities().len() >= 4);
    assert!(hop_lanes.contains(&("cpu_router", VirtualNetworkId::new(1))));
    assert!(hop_lanes.contains(&("router_memory", VirtualNetworkId::new(3))));
    assert!(hop_lanes.contains(&("router_memory", VirtualNetworkId::new(4))));
    assert!(hop_lanes.contains(&("cpu_router", VirtualNetworkId::new(2))));
}

#[test]
fn workload_replay_summary_exposes_router_stage_fabric_hop_activity() {
    let plan =
        rem6_workload::WorkloadReplayPlan::from_manifest(&multihop_fabric_manifest()).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    let activity = summary
        .fabric_hop_activities()
        .iter()
        .find(|activity| {
            activity.link().as_str() == "cpu_router"
                && activity.virtual_network() == VirtualNetworkId::new(1)
                && activity.router().is_some()
        })
        .expect("cpu_router request hop should carry router-stage timing");
    let router = activity.router().unwrap();

    assert_eq!(router.router().as_str(), "router0");
    assert_eq!(router.input_port(), 0);
    assert_eq!(router.output_port(), 1);
    assert_eq!(router.virtual_channel(), 0);
    assert_eq!(router.latency_ticks(), 3);
    assert!(activity.start_tick() >= router.depart_tick());
}
