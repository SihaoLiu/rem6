use rem6_power::{
    PowerEstimate, PowerExpression, PowerExpressionInputs, PowerExpressionModel, PowerModelMode,
    PowerResidency, PowerStateExpression, PowerStateKind, ThermalDomainId, ThermalError,
    ThermalRcModel, ThermalRcSnapshot,
};

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.0001,
        "actual {actual} did not match expected {expected}"
    );
}

#[test]
fn thermal_rc_model_updates_power_expression_temperature_inputs() {
    let mut thermal = ThermalRcModel::new(ThermalDomainId::new(7), 25.0, 2.0, 10.0, 1.0).unwrap();
    let update = thermal.advance(10, PowerEstimate::new(4.0, 1.0)).unwrap();

    assert_eq!(update.domain(), ThermalDomainId::new(7));
    assert_eq!(update.tick(), 10);
    assert_close(update.previous_temperature_c(), 25.0);
    assert_close(update.temperature_c(), 25.5);
    assert_close(update.total_power_watts(), 5.0);
    assert_close(thermal.current_temperature_c(), 25.5);

    let inputs = PowerExpressionInputs::new(25.0, 0.8, 2.0)
        .unwrap()
        .with_thermal_domain(&thermal)
        .unwrap();
    let model = PowerExpressionModel::new(
        PowerModelMode::All,
        inputs,
        vec![PowerStateExpression::new(
            PowerStateKind::On,
            PowerExpression::constant(1.0).unwrap(),
            PowerExpression::multiply(
                PowerExpression::temperature_c(),
                PowerExpression::constant(0.1).unwrap(),
            ),
        )
        .unwrap()],
    )
    .unwrap();
    let estimate = model
        .estimate(&PowerResidency::new(vec![(PowerStateKind::On, 10)]))
        .unwrap();

    assert_close(estimate.dynamic_watts(), 1.0);
    assert_close(estimate.static_watts(), 2.55);
    assert_close(estimate.total_watts(), 3.55);
}

#[test]
fn thermal_rc_model_restores_updates_and_rejects_invalid_state() {
    let mut thermal = ThermalRcModel::new(ThermalDomainId::new(3), 30.0, 1.5, 5.0, 0.5).unwrap();
    thermal.advance(4, PowerEstimate::new(2.0, 1.0)).unwrap();
    thermal.advance(8, PowerEstimate::new(1.0, 1.0)).unwrap();
    let snapshot = thermal.snapshot();

    let mut restored = ThermalRcModel::new(ThermalDomainId::new(3), 30.0, 1.5, 5.0, 0.5).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.updates(), thermal.updates());

    assert_eq!(
        restored
            .advance(7, PowerEstimate::new(1.0, 0.0))
            .unwrap_err(),
        ThermalError::TimeWentBack {
            tick: 7,
            last_tick: 8,
        },
    );
    assert_eq!(
        ThermalRcModel::new(ThermalDomainId::new(1), 25.0, 0.0, 1.0, 1.0).unwrap_err(),
        ThermalError::InvalidThermalResistance,
    );
    assert_eq!(
        ThermalRcModel::new(ThermalDomainId::new(1), 25.0, 1.0, 0.0, 1.0).unwrap_err(),
        ThermalError::InvalidThermalCapacitance,
    );
    assert_eq!(
        ThermalRcModel::new(ThermalDomainId::new(1), 25.0, 1.0, 1.0, 0.0).unwrap_err(),
        ThermalError::InvalidThermalStep,
    );
    assert_eq!(
        restored
            .restore(&ThermalRcSnapshot::new(
                ThermalDomainId::new(4),
                30.0,
                30.0,
                1.5,
                5.0,
                0.5,
                0,
                Vec::new(),
            ))
            .unwrap_err(),
        ThermalError::ThermalDomainMismatch {
            expected: ThermalDomainId::new(3),
            actual: ThermalDomainId::new(4),
        },
    );
}
