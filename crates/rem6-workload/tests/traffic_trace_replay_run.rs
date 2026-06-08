use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadExpectedTrafficTraceReplaySummary, WorkloadHostPlacement, WorkloadId, WorkloadManifest,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadRouteId, WorkloadTopology,
    WorkloadTrafficTraceReplayRun,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
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

fn trace_resource(name: &str) -> WorkloadResource {
    WorkloadResource::new(
        resource_id(name),
        WorkloadResourceKind::Input,
        format!("sha256:{name}"),
        format!("traces/{name}.pb"),
    )
    .unwrap()
}

fn topology(route: &str) -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                64,
                rem6_memory::AddressRange::new(
                    Address::new(0x8000),
                    rem6_memory::AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id(route), "cpu0.ifetch", 0, "memory", 2, 2, 3).unwrap(),
        )
        .unwrap()
}

fn trace_run(route: &str, resource: &str, retry_delay: u64) -> WorkloadTrafficTraceReplayRun {
    WorkloadTrafficTraceReplayRun::new(route_id(route), resource_id(resource), 1_000, 7, 64, 99, 2)
        .with_retry_delay(retry_delay)
}

fn manifest_with_run(run: WorkloadTrafficTraceReplayRun) -> WorkloadManifest {
    WorkloadManifest::builder(id("manifest-trace-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(trace_resource(run.resource().as_str()))
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(topology(run.route().as_str()))
        .add_traffic_trace_replay(run)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn workload_manifest_records_traffic_trace_replay_declarations() {
    let run_a = trace_run("cpu0.fetch", "trace-a", 3);
    let run_b = trace_run("cpu1.fetch", "trace-b", 1);
    let manifest = WorkloadManifest::builder(id("manifest-trace-runs"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(trace_resource("trace-a"))
        .unwrap()
        .add_resource(trace_resource("trace-b"))
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(topology("cpu0.fetch"))
        .add_traffic_trace_replay(run_b.clone())
        .unwrap()
        .add_traffic_trace_replay(run_a.clone())
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(
        manifest.traffic_trace_replays(),
        &[run_a.clone(), run_b.clone()]
    );
    assert!(manifest.required_resources().contains(run_a.resource()));
    assert!(manifest.required_resources().contains(run_b.resource()));

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.traffic_trace_replays(),
        manifest.traffic_trace_replays()
    );
}

#[test]
fn workload_manifest_identity_changes_with_traffic_trace_replay_declarations() {
    let base = manifest_with_run(trace_run("cpu0.fetch", "trace-main", 0));
    let route_changed = manifest_with_run(trace_run("cpu0.alt", "trace-main", 0));
    let resource_changed = manifest_with_run(trace_run("cpu0.fetch", "trace-alt", 0));
    let retry_changed = manifest_with_run(trace_run("cpu0.fetch", "trace-main", 4));

    assert_ne!(base.identity(), route_changed.identity());
    assert_ne!(base.identity(), resource_changed.identity());
    assert_ne!(base.identity(), retry_changed.identity());
}

#[test]
fn traffic_trace_replay_declarations_share_summary_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.fetch"))
        .with_minimum_response_delivery_count(1)
        .with_minimum_control_ack_count(1);
    let manifest = WorkloadManifest::builder(id("manifest-trace-run-summary"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(trace_resource("trace-summary"))
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(topology("cpu0.fetch"))
        .add_traffic_trace_replay(trace_run("cpu0.fetch", "trace-summary", 0))
        .unwrap()
        .add_expected_traffic_trace_replay_summary(expected.clone())
        .unwrap()
        .build()
        .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(plan.expected_traffic_trace_replay_summaries(), &[expected]);
    assert_eq!(
        plan.traffic_trace_replays(),
        manifest.traffic_trace_replays()
    );
}
