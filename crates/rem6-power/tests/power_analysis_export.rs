use rem6_power::{
    ExternalPowerAnalysisKind, PowerAnalysisExport, PowerAnalysisRecord, PowerDomain,
    PowerDomainConfig, PowerError, PowerEstimate, PowerModel, PowerModelMode, PowerResidency,
    PowerStateKind, PowerStatePower,
};

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.0001,
        "actual {actual} did not match expected {expected}"
    );
}

fn domain(name: &str) -> PowerDomain {
    let config = PowerDomainConfig::new(
        name,
        vec![PowerStateKind::On, PowerStateKind::ClockGated],
        PowerStateKind::On,
    )
    .unwrap();
    let mut domain = PowerDomain::new(config);
    let cpu = domain
        .add_leader(
            format!("{name}.leader"),
            vec![PowerStateKind::On, PowerStateKind::ClockGated],
            PowerStateKind::On,
        )
        .unwrap();
    domain
        .transition_leader(4, cpu, PowerStateKind::ClockGated)
        .unwrap();
    domain
        .transition_leader(10, cpu, PowerStateKind::On)
        .unwrap();
    domain
}

fn model(dynamic_on: f64, static_on: f64, dynamic_gated: f64, static_gated: f64) -> PowerModel {
    let mut model = PowerModel::new(
        PowerModelMode::All,
        25.0,
        vec![
            PowerStatePower::new(PowerStateKind::On, dynamic_on, static_on).unwrap(),
            PowerStatePower::new(PowerStateKind::ClockGated, dynamic_gated, static_gated).unwrap(),
        ],
    )
    .unwrap();
    model.update_temperature(37.5).unwrap();
    model
}

#[test]
fn power_analysis_export_collects_sorted_domain_records_and_totals() {
    let cpu = domain("system.cpu_cluster");
    let gpu = domain("system.gpu");
    let cpu_model = model(12.0, 4.0, 2.0, 1.0);
    let gpu_model = model(8.0, 3.0, 1.0, 0.5);

    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::McPat,
        16,
        vec![
            PowerAnalysisRecord::from_domain_model(16, &gpu, &gpu_model).unwrap(),
            PowerAnalysisRecord::from_domain_model(16, &cpu, &cpu_model).unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(export.kind(), ExternalPowerAnalysisKind::McPat);
    assert_eq!(export.tick(), 16);
    assert_eq!(export.records().len(), 2);
    assert_eq!(export.records()[0].target(), "system.cpu_cluster");
    assert_eq!(export.records()[1].target(), "system.gpu");
    assert_eq!(export.records()[0].current_state(), PowerStateKind::On,);
    assert_eq!(export.records()[0].residency_ticks(PowerStateKind::On), 10,);
    assert_eq!(
        export.records()[0].residency_ticks(PowerStateKind::ClockGated),
        6,
    );
    assert_close(export.records()[0].temperature_c(), 37.5);
    assert_close(export.records()[0].dynamic_watts(), 8.25);
    assert_close(export.records()[0].static_watts(), 2.875);
    assert_close(export.records()[0].total_watts(), 11.125);
    assert_close(export.total_dynamic_watts(), 13.625);
    assert_close(export.total_static_watts(), 4.9375);
    assert_close(export.total_watts(), 18.5625);
}

#[test]
fn power_analysis_export_rejects_ambiguous_or_invalid_records() {
    let record = PowerAnalysisRecord::new(
        "system.cpu_cluster",
        PowerStateKind::On,
        PowerResidency::new(vec![(PowerStateKind::On, 4)]),
        40.0,
        PowerEstimate::new(5.0, 2.0),
    )
    .unwrap();

    assert_eq!(
        PowerAnalysisExport::new(
            ExternalPowerAnalysisKind::Dsent,
            20,
            vec![record.clone(), record],
        )
        .unwrap_err(),
        PowerError::DuplicatePowerAnalysisTarget {
            target: "system.cpu_cluster".to_string(),
        },
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            40.0,
            PowerEstimate::new(1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::EmptyName,
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.bad_temperature",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            f64::NAN,
            PowerEstimate::new(1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::InvalidTemperature,
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.bad_power",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            40.0,
            PowerEstimate::new(f64::INFINITY, 1.0),
        )
        .unwrap_err(),
        PowerError::InvalidPowerValue,
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.negative_power",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            40.0,
            PowerEstimate::new(-1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::InvalidPowerValue,
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.undefined",
            PowerStateKind::Undefined,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            40.0,
            PowerEstimate::new(1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::UndefinedState,
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.zero_residency",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 0)]),
            40.0,
            PowerEstimate::new(1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::NoPowerResidency,
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.missing_current_state",
            PowerStateKind::ClockGated,
            PowerResidency::new(vec![(PowerStateKind::On, 4)]),
            40.0,
            PowerEstimate::new(1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::PowerAnalysisCurrentStateMissingResidency {
            target: "system.missing_current_state".to_string(),
            state: PowerStateKind::ClockGated,
        },
    );
    assert_eq!(
        PowerAnalysisRecord::new(
            "system.undefined_residency",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::Undefined, 4)]),
            40.0,
            PowerEstimate::new(1.0, 1.0),
        )
        .unwrap_err(),
        PowerError::PowerAnalysisUndefinedResidencyState {
            target: "system.undefined_residency".to_string(),
        },
    );
}

#[test]
fn power_analysis_record_from_domain_model_preserves_typed_errors() {
    let cpu = domain("system.cpu_cluster");
    let cpu_model = model(12.0, 4.0, 2.0, 1.0);

    assert_eq!(
        PowerAnalysisRecord::from_domain_model(8, &cpu, &cpu_model).unwrap_err(),
        PowerError::TimeWentBack {
            tick: 8,
            last_tick: 10,
        },
    );
}
