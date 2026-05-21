use rem6_boot::BootImage;
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address};
use rem6_stats::StatsRegistry;
use rem6_workload::{
    CheckpointLineage, HostEventIntent, WorkloadError, WorkloadHostEvent, WorkloadHostPlacement,
    WorkloadId, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult, WorkloadRiscvCore,
    WorkloadRouteId, WorkloadTopology,
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
        .add_segment(Address::new(0x8010), vec![0x73, 0x00, 0x00, 0x00])
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

fn disk_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("disk"),
        WorkloadResourceKind::DiskImage,
        "sha256:disk",
        "resources/rootfs.img",
    )
    .unwrap()
}

fn replay_manifest_with_planned_outputs() -> WorkloadManifest {
    WorkloadManifest::builder(id("planned-output-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            80,
            HostEventIntent::Stop {
                reason: "done".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn riscv_topology() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 2, 3)
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
                Address::new(0x9000),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap()
            .with_data("cpu1.dmem", route_id("cpu1.data"))
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn workload_manifest_records_boot_resources_host_events_and_lineage() {
    let manifest = WorkloadManifest::builder(id("riscv-smoke"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_required_resource(resource_id("disk"))
        .add_host_event(WorkloadHostEvent::new(
            25,
            HostEventIntent::StatsReset {
                label: "roi-begin".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .with_checkpoint_lineage(CheckpointLineage::RestoredFrom {
            label: "booted".to_string(),
            manifest_identity: "checkpoint-manifest-1".to_string(),
        })
        .build()
        .unwrap();

    assert_eq!(manifest.id().as_str(), "riscv-smoke");
    assert_eq!(manifest.boot().entry(), Address::new(0x8000));
    assert_eq!(manifest.boot().segments().len(), 2);
    assert_eq!(
        manifest.boot().segments()[0].range().size(),
        AccessSize::new(4).unwrap(),
    );
    assert_eq!(manifest.resources().len(), 2);
    assert_eq!(manifest.resources()[0].id().as_str(), "disk");
    assert_eq!(manifest.resources()[1].id().as_str(), "kernel");
    assert_eq!(manifest.required_resources().len(), 2);
    assert_eq!(manifest.host_events().len(), 2);
    assert_eq!(manifest.host_events()[0].tick(), 25 as Tick);
    assert_eq!(manifest.checkpoint_lineage().unwrap().label(), "booted");
    assert!(!manifest.identity().as_str().is_empty());
}

#[test]
fn workload_manifest_identity_is_stable_for_resource_insertion_order() {
    let first = WorkloadManifest::builder(id("stable-order"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_required_resource(resource_id("disk"))
        .build()
        .unwrap();

    let second = WorkloadManifest::builder(id("stable-order"), boot_image())
        .add_resource(disk_resource())
        .unwrap()
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("disk"))
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_eq!(first.resources(), second.resources());
    assert_eq!(first.required_resources(), second.required_resources());
    assert_eq!(first.identity(), second.identity());
}

#[test]
fn workload_manifest_rejects_missing_and_duplicate_resource_metadata() {
    let missing = WorkloadManifest::builder(id("missing-resource"), boot_image())
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap_err();
    assert_eq!(
        missing,
        WorkloadError::MissingRequiredResource {
            resource: resource_id("kernel"),
        }
    );

    let duplicate = WorkloadManifest::builder(id("duplicate-resource"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(kernel_resource())
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateResource {
            resource: resource_id("kernel"),
        }
    );

    let empty_locator = WorkloadResource::new(
        resource_id("bad"),
        WorkloadResourceKind::Input,
        "sha256:bad",
        "",
    )
    .unwrap_err();
    assert_eq!(
        empty_locator,
        WorkloadError::EmptyResourceLocator {
            resource: resource_id("bad"),
        }
    );
}

#[test]
fn workload_result_links_to_manifest_identity_and_stats_snapshot() {
    let manifest = WorkloadManifest::builder(id("result-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let mut stats = StatsRegistry::new();
    let instructions = stats.register_counter("cpu0.committed", "inst").unwrap();
    stats.increment(instructions, 17).unwrap();
    let snapshot = stats.snapshot(90);

    let result = WorkloadResult::new(manifest.identity(), 96)
        .with_stop_reason("host-stop")
        .with_stats_snapshot(snapshot.clone())
        .with_checkpoint_label("after-roi");

    assert_eq!(result.manifest_identity(), manifest.identity());
    assert_eq!(result.final_tick(), 96);
    assert_eq!(result.stop_reason(), Some("host-stop"));
    assert_eq!(result.stats_snapshot(), Some(&snapshot));
    assert_eq!(result.checkpoint_labels(), &["after-roi".to_string()]);
}

#[test]
fn workload_manifest_reconstructs_boot_image_and_replay_plan() {
    let manifest = WorkloadManifest::builder(id("replay-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .add_required_resource(resource_id("disk"))
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            80,
            HostEventIntent::Stop {
                reason: "done".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::RoiBegin {
                label: "main".to_string(),
            },
        ))
        .with_checkpoint_lineage(CheckpointLineage::CreatedByWorkload {
            label: "cold-boot".to_string(),
        })
        .build()
        .unwrap();

    assert_eq!(manifest.to_boot_image().unwrap(), boot_image());

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(plan.manifest_identity(), manifest.identity());
    assert_eq!(plan.boot(), manifest.boot());
    assert_eq!(plan.to_boot_image().unwrap(), boot_image());
    assert_eq!(plan.required_resources().len(), 2);
    assert_eq!(plan.required_resources()[0].id().as_str(), "disk");
    assert_eq!(
        plan.required_resources()[0].kind(),
        WorkloadResourceKind::DiskImage
    );
    assert_eq!(plan.required_resources()[1].id().as_str(), "kernel");
    assert_eq!(plan.host_events().len(), 2);
    assert_eq!(plan.host_events()[0].tick(), 20);
    assert_eq!(plan.host_events()[1].tick(), 80);
    assert_eq!(
        plan.checkpoint_lineage().unwrap(),
        &CheckpointLineage::CreatedByWorkload {
            label: "cold-boot".to_string(),
        },
    );
}

#[test]
fn workload_manifest_records_topology_for_full_system_replay() {
    let manifest = WorkloadManifest::builder(id("topology-run"), boot_image())
        .with_topology(riscv_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let topology = plan.topology().unwrap();

    assert_eq!(topology.partition_count(), 4);
    assert_eq!(topology.min_remote_delay(), 2);
    assert_eq!(topology.parallel_worker_limit(), 2);
    assert_eq!(topology.host().partition(), 3);
    assert_eq!(topology.host().latency(), 2);
    assert_eq!(topology.host().source(), 41);
    assert_eq!(topology.memory_targets().len(), 1);
    assert_eq!(topology.memory_targets()[0].target(), 0);
    assert_eq!(topology.memory_targets()[0].line_bytes(), 16);
    assert_eq!(topology.memory_routes().len(), 4);
    assert_eq!(topology.memory_routes()[0].id().as_str(), "cpu0.data");
    assert_eq!(topology.riscv_cores().len(), 2);
    assert_eq!(
        topology.riscv_cores()[1].fetch_route().as_str(),
        "cpu1.fetch"
    );
    assert_eq!(topology.riscv_cores()[0].data_endpoint(), Some("cpu0.dmem"));
    assert_eq!(
        topology.riscv_cores()[0]
            .data_route()
            .map(WorkloadRouteId::as_str),
        Some("cpu0.data")
    );

    let different_topology = WorkloadManifest::builder(id("topology-run"), boot_image())
        .with_topology(
            riscv_topology()
                .add_memory_route(
                    WorkloadMemoryRoute::new(
                        route_id("extra.fetch"),
                        "cpu2.ifetch",
                        2,
                        "memory",
                        2,
                        2,
                        3,
                    )
                    .unwrap(),
                )
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    assert_ne!(manifest.identity(), different_topology.identity());
}

#[test]
fn workload_result_validation_rejects_wrong_manifest_and_late_stats() {
    let manifest = WorkloadManifest::builder(id("result-source"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let other = WorkloadManifest::builder(id("different-source"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let valid = WorkloadResult::new(manifest.identity(), 90);
    valid.verify_manifest(&manifest).unwrap();

    let wrong_manifest = WorkloadResult::new(other.identity(), 90)
        .verify_manifest(&manifest)
        .unwrap_err();
    assert_eq!(
        wrong_manifest,
        WorkloadError::ManifestIdentityMismatch {
            expected: manifest.identity(),
            actual: other.identity(),
        }
    );

    let stats = StatsRegistry::new().snapshot(95);
    let late_stats = WorkloadResult::new(manifest.identity(), 90)
        .with_stats_snapshot(stats)
        .verify_manifest(&manifest)
        .unwrap_err();
    assert_eq!(
        late_stats,
        WorkloadError::StatsAfterFinalTick {
            stats_tick: 95,
            final_tick: 90,
        }
    );
}

#[test]
fn workload_replay_plan_validates_matching_result_outputs() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let snapshot = StatsRegistry::new().snapshot(75);
    let result = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_stats_snapshot(snapshot)
        .with_checkpoint_label("warm");

    assert_eq!(plan.planned_checkpoint_labels(), &["warm".to_string()]);
    assert_eq!(plan.planned_stop_reason(), Some("done"));
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_truncated_runs() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let result = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_stop_reason("done")
        .with_checkpoint_label("warm");

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::PlannedHostEventAfterFinalTick {
            event_tick: 80,
            final_tick: 60,
        }
    );
}

#[test]
fn workload_replay_plan_rejects_unplanned_outputs() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let unexpected_checkpoint = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_label("cold");
    let error = plan.verify_result(&unexpected_checkpoint).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedCheckpointLabel {
            label: "cold".to_string(),
        }
    );

    let wrong_stop = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("aborted")
        .with_checkpoint_label("warm");
    let error = plan.verify_result(&wrong_stop).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::StopReasonMismatch {
            expected: "done".to_string(),
            actual: Some("aborted".to_string()),
        }
    );

    let missing_stop =
        WorkloadResult::new(plan.manifest_identity(), 80).with_checkpoint_label("warm");
    let error = plan.verify_result(&missing_stop).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::StopReasonMismatch {
            expected: "done".to_string(),
            actual: None,
        }
    );
}

#[test]
fn workload_replay_plan_rejects_missing_planned_checkpoint() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let result = WorkloadResult::new(plan.manifest_identity(), 80).with_stop_reason("done");

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointLabel {
            label: "warm".to_string(),
        }
    );
}

#[test]
fn workload_replay_plan_rejects_stop_reason_without_planned_stop() {
    let manifest = WorkloadManifest::builder(id("no-stop-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let result = WorkloadResult::new(plan.manifest_identity(), 20)
        .with_stop_reason("host-stop")
        .with_checkpoint_label("warm");

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedStopReason {
            actual: "host-stop".to_string(),
        }
    );
}
