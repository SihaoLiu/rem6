use rem6_boot::BootImage;
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address};
use rem6_stats::StatsRegistry;
use rem6_workload::{
    CheckpointLineage, HostEventIntent, WorkloadError, WorkloadHostEvent, WorkloadId,
    WorkloadManifest, WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
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
