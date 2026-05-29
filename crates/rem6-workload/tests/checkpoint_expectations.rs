use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    HostEventIntent, WorkloadCheckpointChunkSummary, WorkloadCheckpointComponentSummary,
    WorkloadCheckpointManifestSummary, WorkloadError, WorkloadExpectedCheckpointComponentSummary,
    WorkloadHostEvent, WorkloadId, WorkloadManifest, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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

#[test]
fn checkpoint_component_expectation_requires_named_chunks() {
    let manifest = WorkloadManifest::builder(id("checkpoint-required-chunks"), boot_image())
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
        .add_expected_checkpoint_component_summary(
            WorkloadExpectedCheckpointComponentSummary::new("warm", "cpu0", 1, 4)
                .with_required_chunks(["regs"]),
        )
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_chunk = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::with_chunk_summaries(
                    "cpu0",
                    [WorkloadCheckpointChunkSummary::new("pc", 8)],
                )],
            ),
        );
    let error = plan.verify_result(&missing_chunk).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointComponentChunkSummary {
            label: "warm".to_string(),
            component: "cpu0".to_string(),
            chunk: "regs".to_string(),
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::with_chunk_summaries(
                    "cpu0",
                    [
                        WorkloadCheckpointChunkSummary::new("pc", 8),
                        WorkloadCheckpointChunkSummary::new("regs", 4),
                    ],
                )],
            ),
        );
    plan.verify_result(&matched).unwrap();
}

#[test]
fn restore_checkpoint_component_expectation_requires_named_chunks() {
    let manifest =
        WorkloadManifest::builder(id("restore-checkpoint-required-chunks"), boot_image())
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
                40,
                HostEventIntent::RestoreCheckpoint {
                    label: "warm".to_string(),
                },
            ))
            .add_expected_checkpoint_restore_component_summary(
                WorkloadExpectedCheckpointComponentSummary::new("warm", "memory0", 1, 4)
                    .with_required_chunks(["pages"]),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_chunk = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::with_chunk_summaries(
                    "memory0",
                    [WorkloadCheckpointChunkSummary::new("metadata", 8)],
                )],
            ),
        );
    let error = plan.verify_result(&missing_chunk).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreComponentChunkSummary {
            label: "warm".to_string(),
            component: "memory0".to_string(),
            chunk: "pages".to_string(),
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::with_chunk_summaries(
                    "memory0",
                    [
                        WorkloadCheckpointChunkSummary::new("metadata", 8),
                        WorkloadCheckpointChunkSummary::new("pages", 4),
                    ],
                )],
            ),
        );
    plan.verify_result(&matched).unwrap();
}

#[test]
fn checkpoint_required_chunks_contribute_to_manifest_identity() {
    let manifest_with_chunks = |chunk_names: &[&str]| {
        WorkloadManifest::builder(id("checkpoint-required-chunk-identity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_host_event(WorkloadHostEvent::new(
                20,
                HostEventIntent::Checkpoint {
                    label: "warm".to_string(),
                },
            ))
            .add_expected_checkpoint_component_summary(
                WorkloadExpectedCheckpointComponentSummary::new("warm", "cpu0", 1, 4)
                    .with_required_chunks(chunk_names.iter().copied()),
            )
            .unwrap()
            .build()
            .unwrap()
    };

    assert_ne!(
        manifest_with_chunks(&["pc"]).identity(),
        manifest_with_chunks(&["regs"]).identity()
    );
    assert_eq!(
        manifest_with_chunks(&["regs", "pc"]).identity(),
        manifest_with_chunks(&["pc", "regs"]).identity()
    );
}
