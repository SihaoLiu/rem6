use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_stats::StatsRegistry;
use rem6_workload::{
    HostEventIntent, WorkloadCheckpointComponentSummary, WorkloadCheckpointManifestSummary,
    WorkloadError, WorkloadExecutionMode, WorkloadExpectedCheckpointComponentSummary,
    WorkloadExpectedCheckpointManifestSummary, WorkloadHostEvent, WorkloadId, WorkloadManifest,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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
fn workload_replay_plan_accepts_expected_idle_stop_without_host_event() {
    let manifest = WorkloadManifest::builder(id("expected-idle-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_expected_stop_reason("idle")
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let result = WorkloadResult::new(plan.manifest_identity(), 12).with_stop_reason("idle");

    assert_eq!(manifest.expected_stop_reason(), Some("idle"));
    assert_eq!(plan.planned_stop_reason(), Some("idle"));
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_expected_stop_reason() {
    let idle = WorkloadManifest::builder(id("expected-stop-identity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_expected_stop_reason("idle")
        .build()
        .unwrap();
    let host = WorkloadManifest::builder(id("expected-stop-identity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_expected_stop_reason("host-stop")
        .build()
        .unwrap();

    assert_ne!(idle.identity(), host.identity());
}

#[test]
fn workload_replay_plan_rejects_expected_stop_that_conflicts_with_host_stop() {
    let manifest = WorkloadManifest::builder(id("conflicting-expected-stop"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_expected_stop_reason("idle")
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap();

    let error = WorkloadReplayPlan::from_manifest(&manifest).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::StopReasonMismatch {
            expected: "idle".to_string(),
            actual: Some("host-stop".to_string()),
        }
    );
}

#[test]
fn workload_replay_plan_accepts_expected_stop_that_matches_host_stop() {
    let manifest = WorkloadManifest::builder(id("matching-expected-stop"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_expected_stop_reason("host-stop")
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_eq!(plan.planned_stop_reason(), Some("host-stop"));
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
fn workload_replay_plan_validates_checkpoint_manifest_summary_requirements() {
    let manifest = WorkloadManifest::builder(id("checkpoint-summary-run"), boot_image())
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
        .add_expected_checkpoint_manifest_summary(WorkloadExpectedCheckpointManifestSummary::new(
            "warm", 2, 3, 16,
        ))
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_label("warm");
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointManifestSummary {
            label: "warm".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 1, 3, 12,
        ));
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointManifestSummaryBelowMinimum {
            label: "warm".to_string(),
            minimum_component_count: 2,
            actual_component_count: 1,
            minimum_chunk_count: 3,
            actual_chunk_count: 3,
            minimum_payload_bytes: 16,
            actual_payload_bytes: 12,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 2, 3, 16,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_checkpoint_component_summary_requirements() {
    let manifest = WorkloadManifest::builder(id("checkpoint-component-summary-run"), boot_image())
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
        .add_expected_checkpoint_component_summary(WorkloadExpectedCheckpointComponentSummary::new(
            "warm", "cpu0", 2, 8,
        ))
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("memory0", 1, 16)],
            ),
        );
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointComponentSummary {
            label: "warm".to_string(),
            component: "cpu0".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("cpu0", 1, 4)],
            ),
        );
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointComponentSummaryBelowMinimum {
            label: "warm".to_string(),
            component: "cpu0".to_string(),
            minimum_chunk_count: 2,
            actual_chunk_count: 1,
            minimum_payload_bytes: 8,
            actual_payload_bytes: 4,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("cpu0", 2, 8)],
            ),
        );
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_planned_checkpoint_restores() {
    let manifest = WorkloadManifest::builder(id("checkpoint-restore-run"), boot_image())
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
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_eq!(plan.planned_checkpoint_labels(), &["warm".to_string()]);
    assert_eq!(
        plan.planned_checkpoint_restore_labels(),
        &["warm".to_string()]
    );

    let missing_restore =
        WorkloadResult::new(plan.manifest_identity(), 40).with_checkpoint_label("warm");
    let error = plan.verify_result(&missing_restore).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreLabel {
            label: "warm".to_string(),
        }
    );

    let unexpected_restore = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("cold");
    let error = plan.verify_result(&unexpected_restore).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedCheckpointRestoreLabel {
            label: "cold".to_string(),
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_checkpoint_restore_manifest_summary_requirements() {
    let manifest = WorkloadManifest::builder(id("checkpoint-restore-summary-run"), boot_image())
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
        .add_expected_checkpoint_restore_manifest_summary(
            WorkloadExpectedCheckpointManifestSummary::new("warm", 2, 3, 16),
        )
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreManifestSummary {
            label: "warm".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 2, 2, 8,
        ));
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointRestoreManifestSummaryBelowMinimum {
            label: "warm".to_string(),
            minimum_component_count: 2,
            actual_component_count: 2,
            minimum_chunk_count: 3,
            actual_chunk_count: 2,
            minimum_payload_bytes: 16,
            actual_payload_bytes: 8,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 40, 2, 3, 16,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_checkpoint_restore_component_summary_requirements() {
    let manifest =
        WorkloadManifest::builder(id("checkpoint-restore-component-summary-run"), boot_image())
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
                WorkloadExpectedCheckpointComponentSummary::new("warm", "memory0", 1, 16),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                40,
                [WorkloadCheckpointComponentSummary::new("cpu0", 2, 8)],
            ),
        );
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreComponentSummary {
            label: "warm".to_string(),
            component: "memory0".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                40,
                [WorkloadCheckpointComponentSummary::new("memory0", 1, 8)],
            ),
        );
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointRestoreComponentSummaryBelowMinimum {
            label: "warm".to_string(),
            component: "memory0".to_string(),
            minimum_chunk_count: 1,
            actual_chunk_count: 1,
            minimum_payload_bytes: 16,
            actual_payload_bytes: 8,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                40,
                [WorkloadCheckpointComponentSummary::new("memory0", 1, 16)],
            ),
        );
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_planned_execution_mode_switches() {
    let manifest = WorkloadManifest::builder(id("mode-switch-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::SwitchExecutionMode {
                target: "cpu0".to_string(),
                mode: WorkloadExecutionMode::Functional,
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 40);
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingExecutionModeSwitch {
            tick: 40,
            target: "cpu0".to_string(),
            mode: WorkloadExecutionMode::Functional,
        }
    );

    let unexpected = WorkloadResult::new(plan.manifest_identity(), 40).with_execution_mode_switch(
        40,
        "cpu1",
        WorkloadExecutionMode::Detailed,
    );
    let error = plan.verify_result(&unexpected).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedExecutionModeSwitch {
            tick: 40,
            target: "cpu1".to_string(),
            mode: WorkloadExecutionMode::Detailed,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40).with_execution_mode_switch(
        40,
        "cpu0",
        WorkloadExecutionMode::Functional,
    );
    plan.verify_result(&matched).unwrap();

    let scoped = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_execution_mode_switch_stats_scope(
            40,
            "cpu0",
            WorkloadExecutionMode::Functional,
            1,
            20,
        );
    plan.verify_result(&scoped).unwrap();
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
