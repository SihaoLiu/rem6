use rem6_power::{
    PowerError, PowerExpression, PowerExpressionInputs, PowerExpressionModel,
    PowerExpressionModelSnapshot, PowerMetricBinding, PowerMetricBindings, PowerMetricId,
    PowerModelMode, PowerResidency, PowerStateExpression, PowerStateKind,
};
use rem6_stats::{StatId, StatSample, StatSnapshot, StatsRegistry};

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.0001,
        "actual {actual} did not match expected {expected}"
    );
}

#[test]
fn power_expression_model_evaluates_typed_metrics_and_auto_inputs() {
    let activity = PowerMetricId::new(10);
    let misses = PowerMetricId::new(11);
    let inputs = PowerExpressionInputs::new(40.0, 0.8, 2.0)
        .unwrap()
        .with_metric(activity, 7.0)
        .unwrap()
        .with_metric(misses, 3.0)
        .unwrap();
    let on_dynamic = PowerExpression::add(
        PowerExpression::multiply(
            PowerExpression::multiply(
                PowerExpression::metric(activity),
                PowerExpression::voltage_v(),
            ),
            PowerExpression::constant(0.5).unwrap(),
        ),
        PowerExpression::divide(
            PowerExpression::metric(misses),
            PowerExpression::clock_period_ticks(),
        ),
    );
    let on_static = PowerExpression::add(
        PowerExpression::constant(1.0).unwrap(),
        PowerExpression::multiply(
            PowerExpression::temperature_c(),
            PowerExpression::constant(0.02).unwrap(),
        ),
    );

    let mut model = PowerExpressionModel::new(
        PowerModelMode::All,
        inputs,
        vec![
            PowerStateExpression::new(PowerStateKind::On, on_dynamic, on_static).unwrap(),
            PowerStateExpression::new(
                PowerStateKind::ClockGated,
                PowerExpression::constant(1.0).unwrap(),
                PowerExpression::constant(0.5).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let residency = PowerResidency::new(vec![
        (PowerStateKind::On, 10),
        (PowerStateKind::ClockGated, 5),
    ]);
    let estimate = model.estimate(&residency).unwrap();

    assert_close(estimate.dynamic_watts(), 3.2);
    assert_close(estimate.static_watts(), 1.3666667);
    assert_close(estimate.total_watts(), 4.5666667);

    let snapshot = model.snapshot();
    let mut restored = PowerExpressionModel::new(
        PowerModelMode::All,
        PowerExpressionInputs::new(25.0, 1.0, 1.0).unwrap(),
        Vec::new(),
    )
    .unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.estimate(&residency).unwrap(), estimate);

    let updated_inputs = PowerExpressionInputs::new(40.0, 1.0, 2.0)
        .unwrap()
        .with_metric(activity, 7.0)
        .unwrap()
        .with_metric(misses, 3.0)
        .unwrap();
    model.update_inputs(updated_inputs);
    assert_close(
        model.estimate(&residency).unwrap().dynamic_watts(),
        3.6666667,
    );
}

#[test]
fn power_expression_model_rejects_missing_inputs_and_bad_results() {
    let metric = PowerMetricId::new(7);
    let model = PowerExpressionModel::new(
        PowerModelMode::All,
        PowerExpressionInputs::new(30.0, 0.9, 1.0).unwrap(),
        vec![PowerStateExpression::new(
            PowerStateKind::On,
            PowerExpression::metric(metric),
            PowerExpression::constant(1.0).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();

    assert_eq!(
        model
            .estimate(&PowerResidency::new(vec![(PowerStateKind::On, 1)]))
            .unwrap_err(),
        PowerError::MissingPowerMetric { metric },
    );
    assert_eq!(
        PowerExpression::divide(
            PowerExpression::constant(1.0).unwrap(),
            PowerExpression::constant(0.0).unwrap(),
        )
        .evaluate(&PowerExpressionInputs::new(30.0, 0.9, 1.0).unwrap())
        .unwrap_err(),
        PowerError::InvalidPowerExpressionResult,
    );
    assert_eq!(
        PowerExpressionInputs::new(30.0, 0.9, 0.0).unwrap_err(),
        PowerError::InvalidClockPeriod,
    );
    assert_eq!(
        PowerExpression::constant(f64::NAN).unwrap_err(),
        PowerError::InvalidPowerExpressionInput,
    );
    assert_eq!(
        PowerStateExpression::new(
            PowerStateKind::Undefined,
            PowerExpression::constant(1.0).unwrap(),
            PowerExpression::constant(1.0).unwrap(),
        )
        .unwrap_err(),
        PowerError::UndefinedState,
    );
    assert_eq!(
        PowerExpressionModel::new(
            PowerModelMode::All,
            PowerExpressionInputs::new(30.0, 0.9, 1.0).unwrap(),
            vec![
                PowerStateExpression::new(
                    PowerStateKind::On,
                    PowerExpression::constant(1.0).unwrap(),
                    PowerExpression::constant(1.0).unwrap(),
                )
                .unwrap(),
                PowerStateExpression::new(
                    PowerStateKind::On,
                    PowerExpression::constant(2.0).unwrap(),
                    PowerExpression::constant(1.0).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap_err(),
        PowerError::DuplicatePowerStateExpressionModel {
            state: PowerStateKind::On,
        },
    );
    assert_eq!(
        PowerExpressionModel::new(
            PowerModelMode::DynamicOnly,
            PowerExpressionInputs::new(30.0, 0.9, 1.0).unwrap(),
            Vec::new(),
        )
        .unwrap()
        .restore(&PowerExpressionModelSnapshot::new(
            PowerModelMode::StaticOnly,
            PowerExpressionInputs::new(30.0, 0.9, 1.0).unwrap(),
            Vec::new(),
        ))
        .unwrap_err(),
        PowerError::PowerModelModeMismatch {
            expected: PowerModelMode::DynamicOnly,
            actual: PowerModelMode::StaticOnly,
        },
    );
}

#[test]
fn power_expression_inputs_bind_metrics_from_stats_snapshot() {
    let mut stats = StatsRegistry::new();
    let committed = stats
        .register_counter("system.cpu0.committed_ops", "Count")
        .unwrap();
    let cache_misses = stats
        .register_counter("system.l2.overall_misses", "Count")
        .unwrap();
    stats.increment(committed, 20).unwrap();
    stats.increment(cache_misses, 5).unwrap();
    let snapshot = stats.snapshot(100);

    let ops_metric = PowerMetricId::new(1);
    let miss_metric = PowerMetricId::new(2);
    let bindings = PowerMetricBindings::new(vec![
        PowerMetricBinding::new(ops_metric, committed),
        PowerMetricBinding::new(miss_metric, cache_misses),
    ])
    .unwrap();
    let inputs =
        PowerExpressionInputs::from_stat_snapshot(45.0, 0.7, 2.0, &snapshot, &bindings).unwrap();

    assert_eq!(inputs.metric(ops_metric).unwrap(), 20.0);
    assert_eq!(inputs.metric(miss_metric).unwrap(), 5.0);
    assert_close(
        PowerExpression::add(
            PowerExpression::multiply(
                PowerExpression::metric(ops_metric),
                PowerExpression::constant(0.25).unwrap(),
            ),
            PowerExpression::metric(miss_metric),
        )
        .evaluate(&inputs)
        .unwrap(),
        10.0,
    );
    assert_eq!(bindings.stat_for(ops_metric), Some(committed));
    assert_eq!(bindings.metric_for(committed), Some(ops_metric));

    assert_eq!(
        PowerMetricBindings::new(vec![
            PowerMetricBinding::new(ops_metric, committed),
            PowerMetricBinding::new(ops_metric, cache_misses),
        ])
        .unwrap_err(),
        PowerError::DuplicatePowerMetricBinding { metric: ops_metric },
    );
    assert_eq!(
        PowerMetricBindings::new(vec![
            PowerMetricBinding::new(ops_metric, committed),
            PowerMetricBinding::new(miss_metric, committed),
        ])
        .unwrap_err(),
        PowerError::DuplicateBoundStat { stat: committed },
    );
    assert_eq!(
        PowerExpressionInputs::from_stat_snapshot(
            45.0,
            0.7,
            2.0,
            &snapshot,
            &PowerMetricBindings::new(vec![PowerMetricBinding::new(
                PowerMetricId::new(99),
                StatId::new(999),
            )])
            .unwrap(),
        )
        .unwrap_err(),
        PowerError::MissingBoundStat {
            stat: StatId::new(999),
        },
    );
}

#[test]
fn power_expression_inputs_bind_metric_deltas_from_scoped_stats_snapshots() {
    let mut stats = StatsRegistry::new();
    let committed = stats
        .register_counter("system.cpu0.committed_ops", "Count")
        .unwrap();
    let cache_misses = stats
        .register_counter("system.l2.overall_misses", "Count")
        .unwrap();
    stats.increment(committed, 20).unwrap();
    stats.increment(cache_misses, 5).unwrap();
    let previous = stats.snapshot(100);
    stats.increment(committed, 12).unwrap();
    stats.increment(cache_misses, 2).unwrap();
    let current = stats.snapshot(140);

    let ops_metric = PowerMetricId::new(1);
    let miss_metric = PowerMetricId::new(2);
    let bindings = PowerMetricBindings::new(vec![
        PowerMetricBinding::new(ops_metric, committed),
        PowerMetricBinding::new(miss_metric, cache_misses),
    ])
    .unwrap();
    let inputs = PowerExpressionInputs::from_stat_snapshot_delta(
        45.0, 0.7, 2.0, &previous, &current, &bindings,
    )
    .unwrap();

    assert_eq!(inputs.metric(ops_metric).unwrap(), 12.0);
    assert_eq!(inputs.metric(miss_metric).unwrap(), 2.0);
    assert_close(
        PowerExpression::add(
            PowerExpression::multiply(
                PowerExpression::metric(ops_metric),
                PowerExpression::constant(0.25).unwrap(),
            ),
            PowerExpression::metric(miss_metric),
        )
        .evaluate(&inputs)
        .unwrap(),
        5.0,
    );

    stats.reset(150);
    stats.increment(committed, 3).unwrap();
    let reset_scope = stats.snapshot(170);
    assert_eq!(
        PowerExpressionInputs::from_stat_snapshot_delta(
            45.0,
            0.7,
            2.0,
            &current,
            &reset_scope,
            &bindings,
        )
        .unwrap_err(),
        PowerError::PowerStatSnapshotScopeMismatch {
            previous_epoch: 0,
            current_epoch: 1,
            previous_reset_tick: 0,
            current_reset_tick: 150,
        },
    );
    assert_eq!(
        PowerExpressionInputs::from_stat_snapshot_delta(
            45.0, 0.7, 2.0, &current, &previous, &bindings,
        )
        .unwrap_err(),
        PowerError::PowerStatSnapshotTimeWentBack {
            previous_tick: 140,
            current_tick: 100,
        },
    );
}

#[test]
fn power_expression_inputs_reject_counter_regression_between_stats_snapshots() {
    let stat = StatId::new(7);
    let metric = PowerMetricId::new(3);
    let bindings = PowerMetricBindings::new(vec![PowerMetricBinding::new(metric, stat)]).unwrap();
    let previous = StatSnapshot::new(
        10,
        0,
        0,
        vec![StatSample::new(
            stat,
            "system.cpu0.committed_ops",
            "Count",
            12,
        )],
    );
    let current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::new(
            stat,
            "system.cpu0.committed_ops",
            "Count",
            9,
        )],
    );

    assert_eq!(
        PowerExpressionInputs::from_stat_snapshot_delta(
            45.0, 0.7, 2.0, &previous, &current, &bindings,
        )
        .unwrap_err(),
        PowerError::PowerStatValueWentBack {
            stat,
            previous: 12,
            current: 9,
        },
    );
}

#[test]
fn power_expression_inputs_reject_stats_delta_schema_drift() {
    let committed = StatId::new(7);
    let misses = StatId::new(8);
    let metric = PowerMetricId::new(3);
    let bindings =
        PowerMetricBindings::new(vec![PowerMetricBinding::new(metric, committed)]).unwrap();
    let previous = StatSnapshot::new(
        10,
        0,
        0,
        vec![StatSample::new(
            committed,
            "system.cpu0.committed_ops",
            "Count",
            12,
        )],
    );

    let extra_current = StatSnapshot::new(
        20,
        0,
        0,
        vec![
            StatSample::new(committed, "system.cpu0.committed_ops", "Count", 15),
            StatSample::new(misses, "system.l2.overall_misses", "Count", 1),
        ],
    );
    assert_eq!(
        PowerExpressionInputs::from_stat_snapshot_delta(
            45.0,
            0.7,
            2.0,
            &previous,
            &extra_current,
            &bindings,
        )
        .unwrap_err(),
        PowerError::PowerStatUnexpectedStat { stat: misses },
    );

    let renamed_current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::new(
            committed,
            "system.cpu0.retired_ops",
            "Count",
            15,
        )],
    );
    assert_eq!(
        PowerExpressionInputs::from_stat_snapshot_delta(
            45.0,
            0.7,
            2.0,
            &previous,
            &renamed_current,
            &bindings,
        )
        .unwrap_err(),
        PowerError::PowerStatDescriptorMismatch {
            stat: committed,
            previous_path: "system.cpu0.committed_ops".to_string(),
            current_path: "system.cpu0.retired_ops".to_string(),
            previous_unit: "Count".to_string(),
            current_unit: "Count".to_string(),
        },
    );
}
