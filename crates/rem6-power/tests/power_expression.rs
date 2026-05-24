use rem6_power::{
    PowerError, PowerExpression, PowerExpressionInputs, PowerExpressionModel,
    PowerExpressionModelSnapshot, PowerMetricId, PowerModelMode, PowerResidency,
    PowerStateExpression, PowerStateKind,
};

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
