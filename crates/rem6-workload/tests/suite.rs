use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadId, WorkloadManifest, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult, WorkloadSuite, WorkloadSuiteDispatchPlan,
    WorkloadSuiteExecutionEfficiency, WorkloadSuiteExecutionExpectation,
    WorkloadSuiteExecutionSummary, WorkloadSuiteId, WorkloadSuiteReplayPlan, WorkloadSuiteResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn suite_id(value: &str) -> WorkloadSuiteId {
    WorkloadSuiteId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
}

fn kernel_resource(digest: &str) -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        digest,
        "resources/kernel.elf",
    )
    .unwrap()
}

fn manifest(workload: &str, digest: &str) -> WorkloadManifest {
    WorkloadManifest::builder(id(workload), boot_image())
        .add_resource(kernel_resource(digest))
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap()
}

#[test]
fn workload_suite_orders_manifests_and_preserves_identity() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("riscv-mix"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let reordered = WorkloadSuite::builder(suite_id("riscv-mix"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(suite.identity(), reordered.identity());
    assert_eq!(suite.entries()[0].workload_id(), alpha.id());
    assert_eq!(suite.entries()[1].workload_id(), beta.id());

    let plan = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    assert_eq!(plan.suite_identity(), suite.identity());
    assert_eq!(plan.plans().len(), 2);
    assert_eq!(plan.plans()[0].manifest_identity(), alpha.identity());
    assert_eq!(plan.plans()[1].manifest_identity(), beta.identity());
}

#[test]
fn workload_suite_rejects_duplicate_workload_ids() {
    let first = manifest("dup", "sha256:first");
    let second = manifest("dup", "sha256:second");
    let error = WorkloadSuite::builder(suite_id("dups"))
        .add_manifest(first)
        .unwrap()
        .add_manifest(second)
        .unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::DuplicateSuiteWorkload { workload } if workload == id("dup")
    ));
}

#[test]
fn workload_suite_result_verifies_manifest_identities() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("riscv-mix"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();

    let result = WorkloadSuiteResult::new(suite.identity())
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 20))
        .unwrap()
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 10),
        )
        .unwrap();
    assert_eq!(result.results()[0].workload_id(), alpha.id());
    result.verify_against(&suite).unwrap();

    let missing = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 10),
        )
        .unwrap()
        .verify_against(&suite)
        .unwrap_err();
    assert!(matches!(
        missing,
        WorkloadError::MissingSuiteWorkloadResult { workload } if workload == *beta.id()
    ));

    let unexpected = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 10),
        )
        .unwrap()
        .add_result(
            gamma.id().clone(),
            WorkloadResult::new(gamma.identity(), 30),
        )
        .unwrap()
        .verify_against(&suite)
        .unwrap_err();
    assert!(matches!(
        unexpected,
        WorkloadError::UnexpectedSuiteWorkloadResult { workload } if workload == *gamma.id()
    ));

    let drifted = WorkloadSuiteResult::new(suite.identity())
        .add_result(alpha.id().clone(), WorkloadResult::new(beta.identity(), 10))
        .unwrap()
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 20))
        .unwrap()
        .verify_against(&suite)
        .unwrap_err();
    assert!(matches!(
        drifted,
        WorkloadError::SuiteWorkloadResultManifestMismatch { workload, expected, actual }
            if workload == *alpha.id()
                && expected == alpha.identity()
                && actual == beta.identity()
    ));
}

#[test]
fn workload_suite_dispatch_plan_assigns_sorted_manifests_to_workers() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("dispatch"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let reordered = WorkloadSuite::builder(suite_id("dispatch"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(gamma.clone())
        .unwrap()
        .build()
        .unwrap();

    let plan = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let reordered_plan = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&reordered).unwrap(),
        2,
    )
    .unwrap();

    assert_eq!(plan, reordered_plan);
    assert_eq!(plan.suite_identity(), suite.identity());
    assert_eq!(plan.worker_count(), 2);
    assert_eq!(plan.active_worker_count(), 2);
    assert_eq!(plan.records().len(), 3);
    assert_eq!(plan.records()[0].workload_id(), alpha.id());
    assert_eq!(plan.records()[0].worker_index(), 0);
    assert_eq!(plan.records()[0].dispatch_order(), 0);
    assert_eq!(plan.records()[0].manifest_identity(), alpha.identity());
    assert_eq!(plan.records()[1].workload_id(), beta.id());
    assert_eq!(plan.records()[1].worker_index(), 1);
    assert_eq!(plan.records()[1].dispatch_order(), 1);
    assert_eq!(plan.records()[2].workload_id(), gamma.id());
    assert_eq!(plan.records()[2].worker_index(), 0);
    assert_eq!(plan.records()[2].dispatch_order(), 2);
}

#[test]
fn workload_suite_dispatch_plan_rejects_zero_workers() {
    let suite = WorkloadSuite::builder(suite_id("zero-workers"))
        .add_manifest(manifest("alpha", "sha256:alpha"))
        .unwrap()
        .build()
        .unwrap();
    let error = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        0,
    )
    .unwrap_err();

    assert!(matches!(error, WorkloadError::ZeroWorkloadSuiteWorkers));
}

#[test]
fn workload_suite_execution_summary_verifies_dispatch_records() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("execution"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();

    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_completion(beta.id().clone(), beta.identity(), 1, 1, 30)
        .unwrap()
        .add_completion(alpha.id().clone(), alpha.identity(), 0, 0, 20)
        .unwrap();

    assert_eq!(summary.records()[0].workload_id(), alpha.id());
    assert_eq!(summary.records()[0].final_tick(), 20);
    assert_eq!(summary.records()[1].workload_id(), beta.id());
    assert_eq!(summary.maximum_final_tick(), Some(30));
    summary.verify_against_dispatch(&dispatch).unwrap();
}

#[test]
fn workload_suite_execution_summary_derives_from_dispatch_results() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("execution-results"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let results = WorkloadSuiteResult::new(suite.identity())
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 30))
        .unwrap()
        .add_result(
            gamma.id().clone(),
            WorkloadResult::new(gamma.identity(), 40),
        )
        .unwrap()
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 20),
        )
        .unwrap();

    let summary =
        WorkloadSuiteExecutionSummary::from_dispatch_results(&dispatch, &results).unwrap();

    assert_eq!(summary.suite_identity(), suite.identity());
    assert_eq!(summary.records()[0].workload_id(), alpha.id());
    assert_eq!(summary.records()[0].worker_index(), 0);
    assert_eq!(summary.records()[0].dispatch_order(), 0);
    assert_eq!(summary.records()[0].final_tick(), 20);
    assert_eq!(summary.records()[1].workload_id(), beta.id());
    assert_eq!(summary.records()[1].worker_index(), 1);
    assert_eq!(summary.records()[1].dispatch_order(), 1);
    assert_eq!(summary.records()[1].final_tick(), 30);
    assert_eq!(summary.records()[2].workload_id(), gamma.id());
    assert_eq!(summary.records()[2].worker_index(), 0);
    assert_eq!(summary.records()[2].dispatch_order(), 2);
    assert_eq!(summary.records()[2].final_tick(), 40);
    assert_eq!(summary.maximum_final_tick(), Some(40));
    summary.verify_against_dispatch(&dispatch).unwrap();

    let workers = summary.worker_summaries();
    assert_eq!(workers.len(), 2);
    assert_eq!(workers[0].worker_index(), 0);
    assert_eq!(workers[0].completion_count(), 2);
    assert_eq!(workers[0].first_dispatch_order(), Some(0));
    assert_eq!(workers[0].last_dispatch_order(), Some(2));
    assert_eq!(workers[0].maximum_final_tick(), Some(40));
    assert_eq!(workers[1].worker_index(), 1);
    assert_eq!(workers[1].completion_count(), 1);
    assert_eq!(workers[1].first_dispatch_order(), Some(1));
    assert_eq!(workers[1].last_dispatch_order(), Some(1));
    assert_eq!(workers[1].maximum_final_tick(), Some(30));
    assert_eq!(
        summary.worker_summary(0).unwrap().maximum_final_tick(),
        Some(40)
    );
    assert!(summary.worker_summary(2).is_none());
}

#[test]
fn workload_suite_execution_summary_derives_windows_from_results() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("execution-result-windows"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let results = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            beta.id().clone(),
            WorkloadResult::new(beta.identity(), 50)
                .with_start_tick(20)
                .unwrap(),
        )
        .unwrap()
        .add_result(
            gamma.id().clone(),
            WorkloadResult::new(gamma.identity(), 60)
                .with_start_tick(45)
                .unwrap(),
        )
        .unwrap()
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 40)
                .with_start_tick(10)
                .unwrap(),
        )
        .unwrap();

    let summary =
        WorkloadSuiteExecutionSummary::from_dispatch_results(&dispatch, &results).unwrap();

    assert_eq!(summary.minimum_start_tick(), Some(10));
    assert_eq!(summary.maximum_final_tick(), Some(60));
    assert_eq!(summary.total_completion_ticks(), 75);
    assert_eq!(summary.maximum_simultaneous_workers(), 2);
    assert_eq!(summary.records()[0].start_tick(), 10);
    assert_eq!(summary.records()[1].start_tick(), 20);
    assert_eq!(summary.records()[2].start_tick(), 45);
    assert_eq!(
        summary.worker_summary(0).unwrap().busy_tick_span(),
        Some(50)
    );
}

#[test]
fn workload_result_rejects_start_tick_after_final_tick() {
    let alpha = manifest("alpha", "sha256:alpha");
    let error = WorkloadResult::new(alpha.identity(), 20)
        .with_start_tick(30)
        .unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::ResultStartAfterFinalTick {
            start_tick: 30,
            final_tick: 20
        }
    ));
}

#[test]
fn workload_suite_execution_summary_records_parallel_windows() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("execution-windows"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();

    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 40)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 20, 50)
        .unwrap()
        .add_timed_completion(gamma.id().clone(), gamma.identity(), 2, 0, 45, 60)
        .unwrap();

    assert_eq!(summary.minimum_start_tick(), Some(10));
    assert_eq!(summary.maximum_final_tick(), Some(60));
    assert_eq!(summary.total_completion_ticks(), 75);
    assert_eq!(summary.maximum_simultaneous_workers(), 2);
    assert!(summary.has_parallel_worker_overlap());
    summary.verify_minimum_simultaneous_workers(2).unwrap();
    let under_parallel = summary.verify_minimum_simultaneous_workers(3).unwrap_err();
    assert!(matches!(
        under_parallel,
        WorkloadError::SuiteParallelismBelowMinimum {
            minimum_workers: 3,
            actual_workers: 2
        }
    ));
    assert_eq!(summary.records()[0].start_tick(), 10);
    assert_eq!(summary.records()[0].duration_ticks(), 30);
    summary.verify_against_dispatch(&dispatch).unwrap();

    let worker_zero = summary.worker_summary(0).unwrap();
    assert_eq!(worker_zero.first_start_tick(), Some(10));
    assert_eq!(worker_zero.last_final_tick(), Some(60));
    assert_eq!(worker_zero.total_completion_ticks(), 45);
    assert_eq!(worker_zero.busy_tick_span(), Some(50));

    let worker_one = summary.worker_summary(1).unwrap();
    assert_eq!(worker_one.first_start_tick(), Some(20));
    assert_eq!(worker_one.last_final_tick(), Some(50));
    assert_eq!(worker_one.total_completion_ticks(), 30);
    assert_eq!(worker_one.busy_tick_span(), Some(30));
}

#[test]
fn workload_suite_execution_summary_reports_efficiency_metrics() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("execution-efficiency"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 40)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 20, 50)
        .unwrap()
        .add_timed_completion(gamma.id().clone(), gamma.identity(), 2, 0, 45, 60)
        .unwrap();

    let efficiency = summary.execution_efficiency(2).unwrap();

    assert_eq!(efficiency.suite_identity(), suite.identity());
    assert_eq!(efficiency.worker_count(), 2);
    assert_eq!(efficiency.minimum_start_tick(), Some(10));
    assert_eq!(efficiency.maximum_final_tick(), Some(60));
    assert_eq!(efficiency.wall_clock_ticks(), 50);
    assert_eq!(efficiency.serial_completion_ticks(), 75);
    assert_eq!(efficiency.worker_capacity_ticks(), 100);
    assert_eq!(efficiency.idle_worker_ticks(), 25);
    assert_eq!(
        efficiency.parallel_speedup_ratio().unwrap(),
        WorkloadSuiteExecutionEfficiency::ratio(75, 50).unwrap()
    );
    assert_eq!(
        efficiency.worker_utilization_ratio().unwrap(),
        WorkloadSuiteExecutionEfficiency::ratio(75, 100).unwrap()
    );
}

#[test]
fn workload_suite_execution_summary_rejects_invalid_efficiency_capacity() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("bad-efficiency"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let two_worker_summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 40)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 20, 50)
        .unwrap();
    let too_few_workers = two_worker_summary.execution_efficiency(1).unwrap_err();
    assert!(matches!(
        too_few_workers,
        WorkloadError::SuiteExecutionWorkerCountBelowActiveWorkers {
            worker_count: 1,
            active_workers: 2
        }
    ));

    let overlapping_single_worker = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 40)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 0, 20, 50)
        .unwrap();
    let over_capacity = overlapping_single_worker
        .execution_efficiency(1)
        .unwrap_err();
    assert!(matches!(
        over_capacity,
        WorkloadError::SuiteExecutionCapacityBelowCompletionTicks {
            worker_capacity_ticks: 40,
            serial_completion_ticks: 60
        }
    ));
}

#[test]
fn workload_suite_execution_expectation_requires_efficiency_thresholds() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("efficiency-thresholds"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let minimum_speedup = WorkloadSuiteExecutionEfficiency::ratio(3, 2).unwrap();
    let minimum_utilization = WorkloadSuiteExecutionEfficiency::ratio(3, 4).unwrap();
    let expectation = dispatch
        .execution_expectation(2)
        .unwrap()
        .with_minimum_parallel_speedup(minimum_speedup)
        .with_minimum_worker_utilization(minimum_utilization);
    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 40)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 20, 50)
        .unwrap()
        .add_timed_completion(gamma.id().clone(), gamma.identity(), 2, 0, 45, 60)
        .unwrap();
    let efficiency = summary.execution_efficiency(2).unwrap();

    assert_eq!(expectation.worker_count(), 2);
    assert_eq!(
        expectation.minimum_parallel_speedup(),
        Some(minimum_speedup)
    );
    assert_eq!(
        expectation.minimum_worker_utilization(),
        Some(minimum_utilization)
    );
    assert!(efficiency
        .parallel_speedup_ratio()
        .unwrap()
        .meets_or_exceeds(minimum_speedup));
    assert!(efficiency
        .worker_utilization_ratio()
        .unwrap()
        .meets_or_exceeds(minimum_utilization));
    summary.verify_against_expectation(&expectation).unwrap();
}

#[test]
fn workload_suite_execution_expectation_rejects_underperforming_efficiency() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("underperforming-efficiency"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 0, 50)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 50, 100)
        .unwrap();

    let speedup_error = summary
        .verify_against_expectation(
            &dispatch
                .execution_expectation(1)
                .unwrap()
                .with_minimum_parallel_speedup(
                    WorkloadSuiteExecutionEfficiency::ratio(3, 2).unwrap(),
                ),
        )
        .unwrap_err();
    assert!(matches!(
        speedup_error,
        WorkloadError::SuiteParallelSpeedupBelowMinimum {
            minimum_numerator: 3,
            minimum_denominator: 2,
            actual_numerator: 100,
            actual_denominator: 100
        }
    ));

    let utilization_error = summary
        .verify_against_expectation(
            &dispatch
                .execution_expectation(1)
                .unwrap()
                .with_minimum_worker_utilization(
                    WorkloadSuiteExecutionEfficiency::ratio(3, 4).unwrap(),
                ),
        )
        .unwrap_err();
    assert!(matches!(
        utilization_error,
        WorkloadError::SuiteWorkerUtilizationBelowMinimum {
            minimum_numerator: 3,
            minimum_denominator: 4,
            actual_numerator: 100,
            actual_denominator: 200
        }
    ));
}

#[test]
fn workload_suite_dispatch_plan_declares_execution_parallelism_expectation() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("execution-expectation"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let expectation = dispatch.execution_expectation(2).unwrap();
    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 40)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 20, 50)
        .unwrap();

    assert_eq!(expectation.suite_identity(), suite.identity());
    assert_eq!(expectation.minimum_simultaneous_workers(), 2);
    summary.verify_against_expectation(&expectation).unwrap();

    let other_suite = WorkloadSuite::builder(suite_id("other-expectation"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let mismatched = WorkloadSuiteExecutionExpectation::new(other_suite.identity(), 2).unwrap();
    let error = summary.verify_against_expectation(&mismatched).unwrap_err();
    assert!(matches!(
        error,
        WorkloadError::WorkloadSuiteIdentityMismatch { expected, actual }
            if expected == other_suite.identity() && actual == suite.identity()
    ));
}

#[test]
fn workload_suite_dispatch_plan_rejects_unreachable_execution_parallelism() {
    let alpha = manifest("alpha", "sha256:alpha");
    let suite = WorkloadSuite::builder(suite_id("unreachable-expectation"))
        .add_manifest(alpha)
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        4,
    )
    .unwrap();
    let error = dispatch.execution_expectation(2).unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::SuiteParallelismRequirementExceedsActiveWorkers {
            minimum_workers: 2,
            active_workers: 1
        }
    ));
}

#[test]
fn workload_suite_execution_summary_rejects_zero_parallelism_requirement() {
    let alpha = manifest("alpha", "sha256:alpha");
    let suite = WorkloadSuite::builder(suite_id("zero-parallelism"))
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let summary = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 10, 20)
        .unwrap();
    let error = summary.verify_minimum_simultaneous_workers(0).unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::ZeroSuiteParallelismRequirement
    ));
}

#[test]
fn workload_suite_execution_summary_rejects_invalid_windows() {
    let alpha = manifest("alpha", "sha256:alpha");
    let suite = WorkloadSuite::builder(suite_id("bad-window"))
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let error = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 30, 20)
        .unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::SuiteDispatchCompletionWindowInvalid {
            workload,
            start_tick: 30,
            final_tick: 20
        } if workload == *alpha.id()
    ));
}

#[test]
fn workload_suite_execution_summary_rejects_result_dispatch_drift() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("execution-result-drift"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let other_suite = WorkloadSuite::builder(suite_id("other-execution-results"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();

    let mismatched_suite = WorkloadSuiteResult::new(other_suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 20),
        )
        .unwrap()
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 30))
        .unwrap();
    let error = WorkloadSuiteExecutionSummary::from_dispatch_results(&dispatch, &mismatched_suite)
        .unwrap_err();
    assert!(matches!(
        error,
        WorkloadError::WorkloadSuiteIdentityMismatch { expected, actual }
            if expected == suite.identity() && actual == other_suite.identity()
    ));

    let missing = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 20),
        )
        .unwrap();
    let error =
        WorkloadSuiteExecutionSummary::from_dispatch_results(&dispatch, &missing).unwrap_err();
    assert!(matches!(
        error,
        WorkloadError::MissingSuiteWorkloadResult { workload } if workload == *beta.id()
    ));

    let unexpected = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 20),
        )
        .unwrap()
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 30))
        .unwrap()
        .add_result(
            gamma.id().clone(),
            WorkloadResult::new(gamma.identity(), 40),
        )
        .unwrap();
    let error =
        WorkloadSuiteExecutionSummary::from_dispatch_results(&dispatch, &unexpected).unwrap_err();
    assert!(matches!(
        error,
        WorkloadError::UnexpectedSuiteWorkloadResult { workload } if workload == *gamma.id()
    ));

    let drifted = WorkloadSuiteResult::new(suite.identity())
        .add_result(alpha.id().clone(), WorkloadResult::new(beta.identity(), 20))
        .unwrap()
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 30))
        .unwrap();
    let error =
        WorkloadSuiteExecutionSummary::from_dispatch_results(&dispatch, &drifted).unwrap_err();
    assert!(matches!(
        error,
        WorkloadError::SuiteWorkloadResultManifestMismatch { workload, expected, actual }
            if workload == *alpha.id()
                && expected == alpha.identity()
                && actual == beta.identity()
    ));
}

#[test]
fn workload_suite_execution_summary_rejects_dispatch_drift() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("execution-drift"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let dispatch = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();

    let missing = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_completion(alpha.id().clone(), alpha.identity(), 0, 0, 20)
        .unwrap()
        .verify_against_dispatch(&dispatch)
        .unwrap_err();
    assert!(matches!(
        missing,
        WorkloadError::MissingSuiteDispatchCompletion { workload } if workload == *beta.id()
    ));

    let unexpected = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_completion(alpha.id().clone(), alpha.identity(), 0, 0, 20)
        .unwrap()
        .add_completion(gamma.id().clone(), gamma.identity(), 2, 0, 40)
        .unwrap()
        .verify_against_dispatch(&dispatch)
        .unwrap_err();
    assert!(matches!(
        unexpected,
        WorkloadError::UnexpectedSuiteDispatchCompletion { workload } if workload == *gamma.id()
    ));

    let wrong_worker = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_completion(alpha.id().clone(), alpha.identity(), 0, 1, 20)
        .unwrap()
        .add_completion(beta.id().clone(), beta.identity(), 1, 1, 30)
        .unwrap()
        .verify_against_dispatch(&dispatch)
        .unwrap_err();
    assert!(matches!(
        wrong_worker,
        WorkloadError::SuiteDispatchWorkerMismatch { workload, expected, actual }
            if workload == *alpha.id() && expected == 0 && actual == 1
    ));

    let duplicate = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_completion(alpha.id().clone(), alpha.identity(), 0, 0, 20)
        .unwrap()
        .add_completion(alpha.id().clone(), alpha.identity(), 0, 0, 22)
        .unwrap_err();
    assert!(matches!(
        duplicate,
        WorkloadError::DuplicateSuiteDispatchCompletion { workload } if workload == *alpha.id()
    ));
}
