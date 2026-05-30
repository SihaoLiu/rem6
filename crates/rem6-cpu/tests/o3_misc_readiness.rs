use rem6_cpu::{
    O3DependencyProducerKind, O3DependencyReleasePlan, O3DependencyReleaseReason,
    O3DependencyReleaseStage, O3DestinationRegister, O3RegisterClass,
};

#[test]
fn o3_misc_register_dependencies_release_at_commit_after_architectural_update() {
    let misc_tpidr = O3DestinationRegister::commit_visible_misc();

    let writeback =
        O3DependencyReleasePlan::for_stage(O3DependencyReleaseStage::Writeback, [misc_tpidr]);
    assert_eq!(writeback.stage(), O3DependencyReleaseStage::Writeback);
    assert!(!writeback.complete_memory_dependencies());
    assert!(!writeback.wakes_any_dependents());
    assert!(!writeback.marks_any_scoreboard_ready());
    assert_eq!(writeback.destination_releases().len(), 1);
    assert_eq!(
        writeback.destination_releases()[0].reason(),
        O3DependencyReleaseReason::CommitVisibleDestinationDeferred
    );

    let commit = O3DependencyReleasePlan::for_stage(O3DependencyReleaseStage::Commit, [misc_tpidr]);
    assert_eq!(commit.stage(), O3DependencyReleaseStage::Commit);
    assert!(!commit.complete_memory_dependencies());
    assert!(commit.wakes_any_dependents());
    assert!(commit.marks_any_scoreboard_ready());
    assert_eq!(commit.destination_releases().len(), 1);
    assert!(commit.destination_releases()[0].wake_dependents());
    assert!(commit.destination_releases()[0].mark_scoreboard_ready());
    assert_eq!(
        commit.destination_releases()[0].reason(),
        O3DependencyReleaseReason::CommitVisibleDestinationPublished
    );
}

#[test]
fn o3_dependency_release_keeps_memory_completion_at_writeback_only() {
    let load_result = O3DestinationRegister::writeback_visible(O3RegisterClass::Integer);

    let writeback = O3DependencyReleasePlan::for_stage_with_producer(
        O3DependencyReleaseStage::Writeback,
        O3DependencyProducerKind::Memory,
        [load_result],
    );
    assert!(writeback.complete_memory_dependencies());
    assert!(writeback.destination_releases()[0].wake_dependents());
    assert!(writeback.destination_releases()[0].mark_scoreboard_ready());
    assert_eq!(
        writeback.destination_releases()[0].reason(),
        O3DependencyReleaseReason::WritebackVisibleDestinationPublished
    );

    let commit = O3DependencyReleasePlan::for_stage_with_producer(
        O3DependencyReleaseStage::Commit,
        O3DependencyProducerKind::Memory,
        [load_result],
    );
    assert!(!commit.complete_memory_dependencies());
    assert!(!commit.wakes_any_dependents());
    assert!(!commit.marks_any_scoreboard_ready());
    assert_eq!(
        commit.destination_releases()[0].reason(),
        O3DependencyReleaseReason::DestinationAlreadyPublished
    );
}

#[test]
fn o3_dependency_release_skips_always_ready_fixed_mappings() {
    let fixed_mapping = O3DestinationRegister::always_ready_misc();

    for stage in [
        O3DependencyReleaseStage::Writeback,
        O3DependencyReleaseStage::Commit,
    ] {
        let plan = O3DependencyReleasePlan::for_stage(stage, [fixed_mapping]);
        assert!(!plan.wakes_any_dependents());
        assert!(!plan.marks_any_scoreboard_ready());
        assert_eq!(
            plan.destination_releases()[0].reason(),
            O3DependencyReleaseReason::AlwaysReadyFixedMapping
        );
    }
}
