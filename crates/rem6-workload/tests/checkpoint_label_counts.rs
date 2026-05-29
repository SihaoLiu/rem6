use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    HostEventIntent, WorkloadError, WorkloadHostEvent, WorkloadId, WorkloadManifest,
    WorkloadReplayPlan, WorkloadResult,
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
