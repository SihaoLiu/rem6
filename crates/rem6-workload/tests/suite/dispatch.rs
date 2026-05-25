use super::*;

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
fn workload_suite_dispatch_plan_assigns_weighted_manifests_to_least_loaded_workers() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("weighted-dispatch"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();

    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(plan.suite_identity(), suite.identity());
    assert_eq!(plan.worker_count(), 2);
    assert_eq!(plan.records()[0].workload_id(), alpha.id());
    assert_eq!(plan.records()[0].worker_index(), 0);
    assert_eq!(plan.records()[1].workload_id(), beta.id());
    assert_eq!(plan.records()[1].worker_index(), 1);
    assert_eq!(plan.records()[2].workload_id(), delta.id());
    assert_eq!(plan.records()[2].worker_index(), 1);
    assert_eq!(plan.records()[3].workload_id(), gamma.id());
    assert_eq!(plan.records()[3].worker_index(), 1);
}

#[test]
fn workload_suite_dispatch_plan_reports_weighted_load_efficiency_before_execution() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("weighted-dispatch-load"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();

    let summary = plan.estimated_load_summary().unwrap();
    let worker_loads = summary.worker_loads();

    assert_eq!(summary.suite_identity(), suite.identity());
    assert_eq!(summary.worker_count(), 2);
    assert_eq!(summary.serial_estimated_ticks(), 17);
    assert_eq!(summary.maximum_worker_estimated_ticks(), 9);
    assert_eq!(summary.worker_capacity_ticks(), 18);
    assert_eq!(summary.idle_worker_ticks(), 1);
    assert_eq!(
        summary.parallel_speedup_ratio().unwrap(),
        WorkloadSuiteExecutionEfficiency::ratio(17, 9).unwrap()
    );
    assert_eq!(
        summary.worker_utilization_ratio().unwrap(),
        WorkloadSuiteExecutionEfficiency::ratio(17, 18).unwrap()
    );
    assert_eq!(worker_loads.len(), 2);
    assert_eq!(worker_loads[0].worker_index(), 0);
    assert_eq!(worker_loads[0].workload_count(), 1);
    assert_eq!(worker_loads[0].estimated_ticks(), 8);
    assert_eq!(worker_loads[1].worker_index(), 1);
    assert_eq!(worker_loads[1].workload_count(), 3);
    assert_eq!(worker_loads[1].estimated_ticks(), 9);
}

#[test]
fn workload_suite_dispatch_plan_requires_estimates_for_load_summary() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("missing-dispatch-load"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();

    let error = plan.estimated_load_summary().unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::MissingSuiteDispatchEstimate { workload } if workload == *alpha.id()
    ));
}

#[test]
fn workload_suite_dispatch_load_expectation_accepts_planned_parallel_efficiency() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("planned-load-contract"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();
    let summary = plan.estimated_load_summary().unwrap();
    let expectation = WorkloadSuiteDispatchLoadExpectation::new(suite.identity(), 2)
        .unwrap()
        .with_minimum_parallel_speedup(WorkloadSuiteExecutionEfficiency::ratio(17, 10).unwrap())
        .with_minimum_worker_utilization(WorkloadSuiteExecutionEfficiency::ratio(17, 18).unwrap());

    assert_eq!(expectation.suite_identity(), suite.identity());
    assert_eq!(expectation.worker_count(), 2);
    assert_eq!(
        expectation.minimum_parallel_speedup(),
        Some(WorkloadSuiteExecutionEfficiency::ratio(17, 10).unwrap())
    );
    assert_eq!(
        expectation.minimum_worker_utilization(),
        Some(WorkloadSuiteExecutionEfficiency::ratio(17, 18).unwrap())
    );
    summary.verify_against_expectation(&expectation).unwrap();
}

#[test]
fn workload_suite_dispatch_load_expectation_rejects_underplanned_efficiency() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("underplanned-load-contract"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 10).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 1).unwrap(),
        ],
    )
    .unwrap();
    let summary = plan.estimated_load_summary().unwrap();

    let speedup_error = summary
        .verify_against_expectation(
            &WorkloadSuiteDispatchLoadExpectation::new(suite.identity(), 2)
                .unwrap()
                .with_minimum_parallel_speedup(
                    WorkloadSuiteExecutionEfficiency::ratio(3, 2).unwrap(),
                ),
        )
        .unwrap_err();
    assert!(matches!(
        speedup_error,
        WorkloadError::SuitePlannedParallelSpeedupBelowMinimum {
            minimum_numerator: 3,
            minimum_denominator: 2,
            actual_numerator: 12,
            actual_denominator: 10
        }
    ));

    let utilization_error = summary
        .verify_against_expectation(
            &WorkloadSuiteDispatchLoadExpectation::new(suite.identity(), 2)
                .unwrap()
                .with_minimum_worker_utilization(
                    WorkloadSuiteExecutionEfficiency::ratio(3, 4).unwrap(),
                ),
        )
        .unwrap_err();
    assert!(matches!(
        utilization_error,
        WorkloadError::SuitePlannedWorkerUtilizationBelowMinimum {
            minimum_numerator: 3,
            minimum_denominator: 4,
            actual_numerator: 12,
            actual_denominator: 20
        }
    ));
}

#[test]
fn workload_suite_dispatch_load_expectation_rejects_plan_drift() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("dispatch-load-drift"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let other_suite = WorkloadSuite::builder(suite_id("other-dispatch-load"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 2).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 2).unwrap(),
        ],
    )
    .unwrap();
    let summary = plan.estimated_load_summary().unwrap();

    let identity_error = summary
        .verify_against_expectation(
            &WorkloadSuiteDispatchLoadExpectation::new(other_suite.identity(), 2).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        identity_error,
        WorkloadError::WorkloadSuiteIdentityMismatch { expected, actual }
            if expected == other_suite.identity() && actual == suite.identity()
    ));

    let worker_error = summary
        .verify_against_expectation(
            &WorkloadSuiteDispatchLoadExpectation::new(suite.identity(), 3).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        worker_error,
        WorkloadError::SuiteDispatchWorkerCountMismatch {
            expected: 3,
            actual: 2
        }
    ));
}

#[test]
fn workload_suite_dispatch_plan_builds_planned_timeline_from_weighted_estimates() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("planned-dispatch-timeline"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();

    let timeline = plan.planned_execution_timeline().unwrap();

    assert_eq!(timeline.suite_identity(), suite.identity());
    assert_eq!(timeline.worker_count(), 2);
    assert_eq!(timeline.entries().len(), 4);
    assert_eq!(timeline.minimum_start_tick(), Some(0));
    assert_eq!(timeline.maximum_final_tick(), Some(9));
    assert_eq!(timeline.total_estimated_ticks(), 17);
    assert_eq!(timeline.maximum_simultaneous_workers(), 2);
    assert_eq!(timeline.entries()[0].workload_id(), alpha.id());
    assert_eq!(timeline.entries()[0].worker_index(), 0);
    assert_eq!(timeline.entries()[0].planned_start_tick(), 0);
    assert_eq!(timeline.entries()[0].planned_final_tick(), 8);
    assert_eq!(timeline.entries()[1].workload_id(), beta.id());
    assert_eq!(timeline.entries()[1].worker_index(), 1);
    assert_eq!(timeline.entries()[1].planned_start_tick(), 0);
    assert_eq!(timeline.entries()[1].planned_final_tick(), 1);
    assert_eq!(timeline.entries()[2].workload_id(), delta.id());
    assert_eq!(timeline.entries()[2].worker_index(), 1);
    assert_eq!(timeline.entries()[2].planned_start_tick(), 1);
    assert_eq!(timeline.entries()[2].planned_final_tick(), 2);
    assert_eq!(timeline.entries()[3].workload_id(), gamma.id());
    assert_eq!(timeline.entries()[3].worker_index(), 1);
    assert_eq!(timeline.entries()[3].planned_start_tick(), 2);
    assert_eq!(timeline.entries()[3].planned_final_tick(), 9);
}

#[test]
fn workload_suite_dispatch_timeline_materializes_planned_execution_summary() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("planned-timeline-summary"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();
    let timeline = plan.planned_execution_timeline().unwrap();

    let summary = timeline.to_execution_summary().unwrap();

    assert_eq!(summary.suite_identity(), suite.identity());
    assert_eq!(summary.records().len(), 4);
    assert_eq!(summary.records()[0].workload_id(), alpha.id());
    assert_eq!(summary.records()[0].start_tick(), 0);
    assert_eq!(summary.records()[0].final_tick(), 8);
    assert_eq!(summary.records()[3].workload_id(), gamma.id());
    assert_eq!(summary.records()[3].start_tick(), 2);
    assert_eq!(summary.records()[3].final_tick(), 9);
    assert_eq!(summary.maximum_simultaneous_workers(), 2);
    timeline.verify_execution_summary(&summary).unwrap();
}

#[test]
fn workload_suite_dispatch_timeline_reports_planned_worker_summaries() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("planned-worker-summary"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();
    let timeline = plan.planned_execution_timeline().unwrap();

    let worker_summaries = timeline.worker_summaries().unwrap();

    assert_eq!(worker_summaries.len(), 2);
    assert_eq!(worker_summaries[0].worker_index(), 0);
    assert_eq!(worker_summaries[0].completion_count(), 1);
    assert_eq!(worker_summaries[0].first_start_tick(), Some(0));
    assert_eq!(worker_summaries[0].last_final_tick(), Some(8));
    assert_eq!(worker_summaries[0].total_completion_ticks(), 8);
    assert_eq!(worker_summaries[0].busy_tick_span(), Some(8));
    assert_eq!(worker_summaries[1].worker_index(), 1);
    assert_eq!(worker_summaries[1].completion_count(), 3);
    assert_eq!(worker_summaries[1].first_start_tick(), Some(0));
    assert_eq!(worker_summaries[1].last_final_tick(), Some(9));
    assert_eq!(worker_summaries[1].total_completion_ticks(), 9);
    assert_eq!(worker_summaries[1].busy_tick_span(), Some(9));

    let worker_one = timeline.worker_summary(1).unwrap().unwrap();
    assert_eq!(worker_one.completion_count(), 3);
    assert!(timeline.worker_summary(2).unwrap().is_none());
}

#[test]
fn workload_suite_dispatch_timeline_accepts_planned_execution_expectation() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let delta = manifest("delta", "sha256:delta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("planned-timeline-contract"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(delta.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 8).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(delta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 7).unwrap(),
        ],
    )
    .unwrap();
    let timeline = plan.planned_execution_timeline().unwrap();
    let expectation = WorkloadSuiteExecutionExpectation::new(suite.identity(), 2)
        .unwrap()
        .with_minimum_parallel_speedup(WorkloadSuiteExecutionEfficiency::ratio(17, 9).unwrap())
        .with_minimum_worker_utilization(WorkloadSuiteExecutionEfficiency::ratio(17, 18).unwrap());

    timeline.verify_against_expectation(&expectation).unwrap();
}

#[test]
fn workload_suite_dispatch_timeline_rejects_underplanned_expectation() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("underplanned-timeline-contract"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let other_suite = WorkloadSuite::builder(suite_id("other-timeline-contract"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(gamma.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 10).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 1).unwrap(),
        ],
    )
    .unwrap();
    let timeline = plan.planned_execution_timeline().unwrap();

    let identity_error = timeline
        .verify_against_expectation(
            &WorkloadSuiteExecutionExpectation::new(other_suite.identity(), 2).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        identity_error,
        WorkloadError::WorkloadSuiteIdentityMismatch { expected, actual }
            if expected == other_suite.identity() && actual == suite.identity()
    ));

    let worker_error = timeline
        .verify_against_expectation(
            &WorkloadSuiteExecutionExpectation::new(suite.identity(), 3).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        worker_error,
        WorkloadError::SuiteDispatchWorkerCountMismatch {
            expected: 3,
            actual: 2
        }
    ));

    let serial_suite = WorkloadSuite::builder(suite_id("serial-planned-timeline"))
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let serial_timeline = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &WorkloadSuiteReplayPlan::from_suite(&serial_suite).unwrap(),
        2,
        &[WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 10).unwrap()],
    )
    .unwrap()
    .planned_execution_timeline()
    .unwrap();
    let parallelism_error = serial_timeline
        .verify_against_expectation(
            &WorkloadSuiteExecutionExpectation::new(serial_suite.identity(), 2).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        parallelism_error,
        WorkloadError::SuiteParallelismBelowMinimum {
            minimum_workers: 2,
            actual_workers: 1
        }
    ));

    let speedup_error = timeline
        .verify_against_expectation(
            &WorkloadSuiteExecutionExpectation::new(suite.identity(), 2)
                .unwrap()
                .with_minimum_parallel_speedup(
                    WorkloadSuiteExecutionEfficiency::ratio(3, 2).unwrap(),
                ),
        )
        .unwrap_err();
    assert!(matches!(
        speedup_error,
        WorkloadError::SuitePlannedParallelSpeedupBelowMinimum {
            minimum_numerator: 3,
            minimum_denominator: 2,
            actual_numerator: 12,
            actual_denominator: 10
        }
    ));

    let utilization_error = timeline
        .verify_against_expectation(
            &WorkloadSuiteExecutionExpectation::new(suite.identity(), 2)
                .unwrap()
                .with_minimum_worker_utilization(
                    WorkloadSuiteExecutionEfficiency::ratio(3, 4).unwrap(),
                ),
        )
        .unwrap_err();
    assert!(matches!(
        utilization_error,
        WorkloadError::SuitePlannedWorkerUtilizationBelowMinimum {
            minimum_numerator: 3,
            minimum_denominator: 4,
            actual_numerator: 12,
            actual_denominator: 20
        }
    ));
}

#[test]
fn workload_suite_dispatch_plan_requires_estimates_for_planned_timeline() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("missing-timeline-estimates"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();

    let error = plan.planned_execution_timeline().unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::MissingSuiteDispatchEstimate { workload } if workload == *alpha.id()
    ));
}

#[test]
fn workload_suite_dispatch_timeline_rejects_execution_window_drift() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("timeline-window-drift"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let plan = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 4).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 5).unwrap(),
        ],
    )
    .unwrap();
    let timeline = plan.planned_execution_timeline().unwrap();
    let matching = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 0, 4)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 0, 5)
        .unwrap();

    timeline.verify_execution_summary(&matching).unwrap();

    let drifted = WorkloadSuiteExecutionSummary::new(suite.identity())
        .add_timed_completion(alpha.id().clone(), alpha.identity(), 0, 0, 1, 5)
        .unwrap()
        .add_timed_completion(beta.id().clone(), beta.identity(), 1, 1, 0, 5)
        .unwrap();
    let error = timeline.verify_execution_summary(&drifted).unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::SuiteDispatchTimelineWindowMismatch {
            workload,
            expected_start_tick: 0,
            expected_final_tick: 4,
            actual_start_tick: 1,
            actual_final_tick: 5,
        } if workload == *alpha.id()
    ));
}

#[test]
fn workload_suite_dispatch_plan_rejects_invalid_weighted_dispatch_inputs() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("bad-weighted-dispatch"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let replay = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();

    let zero = WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 0).unwrap_err();
    assert!(matches!(
        zero,
        WorkloadError::ZeroSuiteDispatchWeight { workload } if workload == *alpha.id()
    ));

    let missing = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 1).unwrap()],
    )
    .unwrap_err();
    assert!(matches!(
        missing,
        WorkloadError::MissingSuiteDispatchWeight { workload } if workload == *beta.id()
    ));

    let duplicate = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 2).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        duplicate,
        WorkloadError::DuplicateSuiteDispatchWeight { workload } if workload == *alpha.id()
    ));

    let unexpected = WorkloadSuiteDispatchPlan::from_replay_plan_weighted(
        &replay,
        2,
        &[
            WorkloadSuiteDispatchWeight::new(alpha.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(beta.id().clone(), 1).unwrap(),
            WorkloadSuiteDispatchWeight::new(gamma.id().clone(), 1).unwrap(),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        unexpected,
        WorkloadError::UnexpectedSuiteDispatchWeight { workload } if workload == *gamma.id()
    ));
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
