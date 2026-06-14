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
fn power_analysis_smoke_xml_export_serializes_power_analysis_records() {
    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::McPat,
        42,
        vec![PowerAnalysisRecord::new(
            "system.cpu&cluster",
            PowerStateKind::On,
            PowerResidency::new(vec![
                (PowerStateKind::On, 30),
                (PowerStateKind::ClockGated, 12),
            ]),
            41.25,
            PowerEstimate::new(3.5, 1.25),
        )
        .unwrap()],
    )
    .unwrap();

    assert_eq!(
        export.to_power_analysis_smoke_xml(),
        concat!(
            "<power_analysis_smoke kind=\"McPat\" tick=\"42\">\n",
            "  <totals dynamic_watts=\"3.500000\" static_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "  <component name=\"system.cpu&amp;cluster\" state=\"On\" temperature_c=\"41.250000\" dynamic_watts=\"3.500000\" static_watts=\"1.250000\" total_watts=\"4.750000\">\n",
            "    <residency state=\"On\" ticks=\"30\"/>\n",
            "    <residency state=\"ClockGated\" ticks=\"12\"/>\n",
            "  </component>\n",
            "</power_analysis_smoke>\n",
        ),
    );
}

#[test]
fn power_analysis_smoke_xml_export_records_external_analysis_kind() {
    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::Dsent,
        1,
        vec![PowerAnalysisRecord::new(
            "system.link",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            30.0,
            PowerEstimate::new(0.5, 0.25),
        )
        .unwrap()],
    )
    .unwrap();

    assert!(export
        .to_power_analysis_smoke_xml()
        .starts_with("<power_analysis_smoke kind=\"Dsent\" tick=\"1\">\n"));
}

#[test]
fn mcpat_compatible_xml_export_serializes_adapter_records() {
    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::McPat,
        42,
        vec![PowerAnalysisRecord::new(
            "system.cpu&cluster",
            PowerStateKind::On,
            PowerResidency::new(vec![
                (PowerStateKind::On, 30),
                (PowerStateKind::ClockGated, 12),
            ]),
            41.25,
            PowerEstimate::new(3.5, 1.25),
        )
        .unwrap()],
    )
    .unwrap();

    assert_eq!(
        export.to_mcpat_compatible_xml().unwrap(),
        concat!(
            "<mcpat_power tick=\"42\">\n",
            "  <component id=\"system.cpu&amp;cluster\" name=\"system.cpu&amp;cluster\" state=\"On\">\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <thermal temperature_c=\"41.250000\"/>\n",
            "    <residency state=\"On\" ticks=\"30\" ratio=\"0.714286\"/>\n",
            "    <residency state=\"ClockGated\" ticks=\"12\" ratio=\"0.285714\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "</mcpat_power>\n",
        ),
    );
}

#[test]
fn mcpat_compatible_xml_export_rejects_non_mcpat_kind() {
    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::Dsent,
        1,
        vec![PowerAnalysisRecord::new(
            "system.link",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            30.0,
            PowerEstimate::new(0.5, 0.25),
        )
        .unwrap()],
    )
    .unwrap();

    assert_eq!(
        export.to_mcpat_compatible_xml().unwrap_err(),
        PowerError::PowerAnalysisKindMismatch {
            expected: ExternalPowerAnalysisKind::McPat,
            actual: ExternalPowerAnalysisKind::Dsent,
        },
    );
}

#[test]
fn dsent_compatible_csv_export_serializes_adapter_records() {
    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::Dsent,
        42,
        vec![PowerAnalysisRecord::new(
            "system.mesh.link0",
            PowerStateKind::ClockGated,
            PowerResidency::new(vec![
                (PowerStateKind::On, 12),
                (PowerStateKind::ClockGated, 30),
            ]),
            41.25,
            PowerEstimate::new(0.75, 0.125),
        )
        .unwrap()],
    )
    .unwrap();

    assert_eq!(
        export.to_dsent_compatible_csv().unwrap(),
        concat!(
            "record_type,tick,target,state,temperature_c,dynamic_watts,static_watts,total_watts,residency_state,residency_ticks,residency_ratio\n",
            "component,42,system.mesh.link0,ClockGated,41.250000,0.750000,0.125000,0.875000,On,12,0.285714\n",
            "component,42,system.mesh.link0,ClockGated,41.250000,0.750000,0.125000,0.875000,ClockGated,30,0.714286\n",
            "total,42,__total__,All,,0.750000,0.125000,0.875000,,42,1.000000\n",
        ),
    );
}

#[test]
fn dsent_compatible_csv_export_rejects_non_dsent_kind() {
    let export = PowerAnalysisExport::new(
        ExternalPowerAnalysisKind::McPat,
        1,
        vec![PowerAnalysisRecord::new(
            "system.cpu",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, 1)]),
            30.0,
            PowerEstimate::new(0.5, 0.25),
        )
        .unwrap()],
    )
    .unwrap();

    assert_eq!(
        export.to_dsent_compatible_csv().unwrap_err(),
        PowerError::PowerAnalysisKindMismatch {
            expected: ExternalPowerAnalysisKind::Dsent,
            actual: ExternalPowerAnalysisKind::McPat,
        },
    );
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
