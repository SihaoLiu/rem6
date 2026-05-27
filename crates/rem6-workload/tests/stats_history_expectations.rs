use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_stats::{
    StatDumpId, StatDumpRecord, StatHistoryRecord, StatResetId, StatSnapshot, StatsResetRecord,
};
use rem6_workload::{
    WorkloadError, WorkloadExpectedStatsHistory, WorkloadId, WorkloadManifest, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
    WorkloadStatsHistoryExpectationError,
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

fn expected_stats_history(
    minimum_reset_count: usize,
    minimum_dump_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> WorkloadExpectedStatsHistory {
    WorkloadExpectedStatsHistory::new(minimum_reset_count, minimum_dump_count)
        .unwrap()
        .with_tick_window(first_tick, last_tick)
        .unwrap()
}

fn stats_reset(tick: u64, epoch: u64) -> StatHistoryRecord {
    StatHistoryRecord::Reset(StatsResetRecord::with_id(
        StatResetId::new(epoch),
        tick,
        epoch,
        Vec::new(),
    ))
}

fn stats_dump(id: u64, tick: u64, epoch: u64, reset_tick: u64) -> StatHistoryRecord {
    StatHistoryRecord::Dump(StatDumpRecord::new(
        StatDumpId::new(id),
        StatSnapshot::new(tick, epoch, reset_tick, Vec::new()),
    ))
}

fn result_with_history(
    plan: &WorkloadReplayPlan,
    history: Vec<StatHistoryRecord>,
) -> WorkloadResult {
    WorkloadResult::new(plan.manifest_identity(), 32).with_stats_history_records(history)
}

#[test]
fn workload_manifest_records_stats_history_expectations() {
    let expected = expected_stats_history(1, 1, 4, 9);
    let manifest = WorkloadManifest::builder(id("manifest-stats-history"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_stats_history(expected)
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(manifest.expected_stats_history(), Some(&expected));
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_stats_history(),
        manifest.expected_stats_history()
    );

    let result = result_with_history(&plan, vec![stats_reset(4, 1), stats_dump(1, 9, 1, 4)]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_stats_history_expectations() {
    let base = WorkloadManifest::builder(id("identity-stats-history"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let counted = WorkloadManifest::builder(id("identity-stats-history"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_stats_history(WorkloadExpectedStatsHistory::new(1, 1).unwrap())
        .unwrap()
        .build()
        .unwrap();
    let windowed = WorkloadManifest::builder(id("identity-stats-history"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_stats_history(expected_stats_history(1, 1, 4, 9))
        .unwrap()
        .build()
        .unwrap();

    assert_ne!(base.identity(), counted.identity());
    assert_ne!(counted.identity(), windowed.identity());
}

#[test]
fn workload_replay_plan_rejects_bad_stats_history_evidence() {
    let expected = expected_stats_history(1, 1, 4, 9);
    let plan = WorkloadReplayPlan::from_manifest(
        &WorkloadManifest::builder(id("verify-stats-history"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_stats_history(expected)
            .unwrap()
            .build()
            .unwrap(),
    )
    .unwrap();

    let underactive = result_with_history(&plan, vec![stats_reset(4, 1)]);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::StatsHistoryExpectation(
            WorkloadStatsHistoryExpectationError::BelowMinimum {
                minimum_reset_count: 1,
                actual_reset_count: 1,
                minimum_dump_count: 1,
                actual_dump_count: 0,
            },
        ),
    );

    let wrong_window = result_with_history(&plan, vec![stats_reset(5, 1), stats_dump(1, 11, 1, 5)]);
    assert_eq!(
        plan.verify_result(&wrong_window).unwrap_err(),
        WorkloadError::StatsHistoryExpectation(
            WorkloadStatsHistoryExpectationError::TickWindowMismatch {
                expected_first_tick: 4,
                actual_first_tick: Some(5),
                expected_last_tick: 9,
                actual_last_tick: Some(11),
            },
        ),
    );
}

#[test]
fn workload_rejects_duplicate_or_empty_stats_history_expectations() {
    assert_eq!(
        WorkloadExpectedStatsHistory::new(0, 0).unwrap_err(),
        WorkloadError::StatsHistoryExpectation(
            WorkloadStatsHistoryExpectationError::EmptyExpectation,
        ),
    );

    let expected = WorkloadExpectedStatsHistory::new(1, 0).unwrap();
    let duplicate = WorkloadManifest::builder(id("duplicate-stats-history"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_stats_history(expected)
        .unwrap()
        .add_expected_stats_history(expected)
        .unwrap_err();

    assert_eq!(
        duplicate,
        WorkloadError::StatsHistoryExpectation(
            WorkloadStatsHistoryExpectationError::DuplicateExpectation,
        ),
    );
}
