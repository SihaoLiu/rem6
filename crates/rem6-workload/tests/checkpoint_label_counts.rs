use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    HostEventIntent, WorkloadCheckpointManifestSummary, WorkloadError, WorkloadHostEvent,
    WorkloadId, WorkloadManifest, WorkloadReplayPlan, WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
}

#[test]
fn checkpoint_labels_verify_planned_occurrence_counts() {
    let manifest = WorkloadManifest::builder(id("checkpoint-label-counts"), boot_image())
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let underrecorded =
        WorkloadResult::new(plan.manifest_identity(), 40).with_checkpoint_label("warm");
    let error = plan.verify_result(&underrecorded).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointLabel {
            label: "warm".to_string()
        }
    );

    let overrecorded = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_checkpoint_label("warm")
        .with_checkpoint_label("warm");
    let error = plan.verify_result(&overrecorded).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedCheckpointLabel {
            label: "warm".to_string()
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_checkpoint_label("warm");
    plan.verify_result(&matched).unwrap();
}

#[test]
fn checkpoint_restore_labels_verify_planned_occurrence_counts() {
    let manifest = WorkloadManifest::builder(id("checkpoint-restore-label-counts"), boot_image())
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
        .add_host_event(WorkloadHostEvent::new(
            60,
            HostEventIntent::RestoreCheckpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let underrecorded = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    let error = plan.verify_result(&underrecorded).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreLabel {
            label: "warm".to_string()
        }
    );

    let overrecorded = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    let error = plan.verify_result(&overrecorded).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedCheckpointRestoreLabel {
            label: "warm".to_string()
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    plan.verify_result(&matched).unwrap();
}

#[test]
fn checkpoint_manifest_summary_tick_must_not_exceed_final_tick() {
    let manifest = WorkloadManifest::builder(id("checkpoint-summary-tick"), boot_image())
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let result = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 80, 1, 1, 8,
        ));

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::checkpoint_manifest_summary_after_final_tick("warm", 80, 40)
    );
}

#[test]
fn checkpoint_manifest_summary_tick_must_match_planned_checkpoint_tick() {
    let manifest = WorkloadManifest::builder(id("checkpoint-summary-planned-tick"), boot_image())
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let mismatched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 30, 1, 1, 8,
        ));

    let error = plan.verify_result(&mismatched).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::checkpoint_manifest_summary_tick_mismatch("warm", 30, [20])
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 1, 1, 8,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn checkpoint_manifest_summary_ticks_cover_repeated_checkpoint_labels() {
    let manifest = WorkloadManifest::builder(id("checkpoint-summary-repeated-ticks"), boot_image())
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let duplicate_tick = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 1, 1, 8,
        ))
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 1, 1, 8,
        ));

    let error = plan.verify_result(&duplicate_tick).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::checkpoint_manifest_summary_tick_mismatch("warm", 20, [40])
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 1, 1, 8,
        ))
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 1, 1, 8,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn checkpoint_restore_manifest_summary_tick_must_not_exceed_final_tick() {
    let manifest = WorkloadManifest::builder(id("checkpoint-restore-summary-tick"), boot_image())
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
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let result = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 80, 1, 1, 8,
        ));

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::checkpoint_restore_manifest_summary_after_final_tick("warm", 80, 40)
    );
}

#[test]
fn checkpoint_restore_manifest_summary_tick_must_match_planned_restore_tick() {
    let manifest =
        WorkloadManifest::builder(id("checkpoint-restore-summary-planned-tick"), boot_image())
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
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let mismatched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 30, 1, 1, 8,
        ));

    let error = plan.verify_result(&mismatched).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::checkpoint_restore_manifest_summary_tick_mismatch("warm", 30, [40])
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 1, 1, 8,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn checkpoint_restore_manifest_summary_ticks_cover_repeated_restore_labels() {
    let manifest = WorkloadManifest::builder(
        id("checkpoint-restore-summary-repeated-ticks"),
        boot_image(),
    )
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
    .add_host_event(WorkloadHostEvent::new(
        60,
        HostEventIntent::RestoreCheckpoint {
            label: "warm".to_string(),
        },
    ))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let duplicate_tick = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 1, 1, 8,
        ))
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 1, 1, 8,
        ));

    let error = plan.verify_result(&duplicate_tick).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::checkpoint_restore_manifest_summary_tick_mismatch("warm", 40, [60])
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 1, 1, 8,
        ))
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 60, 1, 1, 8,
        ));
    plan.verify_result(&matched).unwrap();
}
