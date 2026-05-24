use rem6_power::{
    PowerComponentId, PowerDomain, PowerDomainConfig, PowerDomainSnapshot, PowerError, PowerModel,
    PowerModelMode, PowerModelSnapshot, PowerResidency, PowerStateKind, PowerStatePower,
};

#[test]
fn power_domain_tracks_residency_and_matches_followers() {
    let config = PowerDomainConfig::new(
        "cluster0",
        vec![
            PowerStateKind::On,
            PowerStateKind::ClockGated,
            PowerStateKind::Off,
        ],
        PowerStateKind::On,
    )
    .unwrap();
    let mut domain = PowerDomain::new(config.clone());
    let cpu = domain
        .add_leader(
            "cpu0",
            vec![
                PowerStateKind::On,
                PowerStateKind::ClockGated,
                PowerStateKind::Off,
            ],
            PowerStateKind::On,
        )
        .unwrap();
    let cache = domain
        .add_follower(
            "l2",
            vec![PowerStateKind::On, PowerStateKind::ClockGated],
            PowerStateKind::On,
        )
        .unwrap();

    assert_eq!(cpu, PowerComponentId::new(0));
    assert_eq!(cache, PowerComponentId::new(1));

    domain
        .transition_leader(5, cpu, PowerStateKind::Off)
        .unwrap();
    assert_eq!(domain.current_state(), PowerStateKind::ClockGated);
    assert_eq!(
        domain.component_state(cache).unwrap(),
        PowerStateKind::ClockGated
    );
    assert_eq!(domain.leader_calls(), 1);
    assert_eq!(domain.leader_calls_changing_state(), 1);

    domain
        .transition_leader(13, cpu, PowerStateKind::On)
        .unwrap();
    assert_eq!(domain.current_state(), PowerStateKind::On);
    assert_eq!(domain.component_state(cache).unwrap(), PowerStateKind::On);

    let residency = domain.residency_at(20).unwrap();
    assert_eq!(residency.ticks(PowerStateKind::On), 12);
    assert_eq!(residency.ticks(PowerStateKind::ClockGated), 8);
    assert_eq!(residency.ticks(PowerStateKind::Off), 0);
    assert_eq!(domain.domain_transitions(), 2);
    assert_eq!(domain.follower_match_transitions(), 2);

    let snapshot = domain.snapshot();
    let mut restored = PowerDomain::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.residency_at(20).unwrap(), residency);

    assert_eq!(
        restored
            .transition_leader(12, cpu, PowerStateKind::Off)
            .unwrap_err(),
        PowerError::TimeWentBack {
            tick: 12,
            last_tick: 13,
        }
    );
    assert_eq!(
        restored
            .transition_leader(21, cache, PowerStateKind::Off)
            .unwrap_err(),
        PowerError::ComponentIsNotLeader { component: cache }
    );
}

#[test]
fn power_domain_rejects_invalid_configs_and_transitions() {
    assert_eq!(
        PowerDomainConfig::new("bad", vec![PowerStateKind::On], PowerStateKind::Off).unwrap_err(),
        PowerError::StateNotAllowed {
            component: "bad".to_string(),
            state: PowerStateKind::Off,
        }
    );
    assert_eq!(
        PowerDomainConfig::new("", vec![PowerStateKind::On], PowerStateKind::On).unwrap_err(),
        PowerError::EmptyName
    );
    assert_eq!(
        PowerDomainConfig::new(
            "bad",
            vec![PowerStateKind::Undefined],
            PowerStateKind::Undefined
        )
        .unwrap_err(),
        PowerError::UndefinedState
    );

    let config =
        PowerDomainConfig::new("cluster0", vec![PowerStateKind::On], PowerStateKind::On).unwrap();
    let mut domain = PowerDomain::new(config);
    let cpu = domain
        .add_leader("cpu0", vec![PowerStateKind::On], PowerStateKind::On)
        .unwrap();

    assert_eq!(
        domain
            .transition_leader(1, cpu, PowerStateKind::ClockGated)
            .unwrap_err(),
        PowerError::StateNotAllowed {
            component: "cpu0".to_string(),
            state: PowerStateKind::ClockGated,
        }
    );
    assert_eq!(
        domain
            .restore(&PowerDomainSnapshot::new(
                PowerDomainConfig::new("other", vec![PowerStateKind::On], PowerStateKind::On,)
                    .unwrap(),
                Vec::new(),
                PowerStateKind::On,
                PowerStateKind::On,
                0,
                0,
                0,
                0,
                0,
                Vec::new(),
            ))
            .unwrap_err(),
        PowerError::SnapshotConfigMismatch {
            expected: PowerDomainConfig::new(
                "cluster0",
                vec![PowerStateKind::On],
                PowerStateKind::On,
            )
            .unwrap(),
            actual: PowerDomainConfig::new("other", vec![PowerStateKind::On], PowerStateKind::On)
                .unwrap(),
        }
    );
}

#[test]
fn power_model_weights_state_power_by_residency_and_temperature() {
    let mut model = PowerModel::new(
        PowerModelMode::All,
        25.0,
        vec![
            PowerStatePower::new(PowerStateKind::On, 10.0, 2.0)
                .unwrap()
                .with_static_temperature_coefficient(0.10)
                .unwrap(),
            PowerStatePower::new(PowerStateKind::ClockGated, 1.0, 1.0)
                .unwrap()
                .with_static_temperature_coefficient(0.05)
                .unwrap(),
            PowerStatePower::new(PowerStateKind::Off, 0.0, 0.0).unwrap(),
        ],
    )
    .unwrap();
    model.update_temperature(35.0).unwrap();

    let residency = PowerResidency::new(vec![
        (PowerStateKind::On, 12),
        (PowerStateKind::ClockGated, 8),
        (PowerStateKind::Off, 0),
    ]);
    let estimate = model.estimate(&residency).unwrap();

    assert!((estimate.dynamic_watts() - 6.4).abs() < 0.0001);
    assert!((estimate.static_watts() - 2.4).abs() < 0.0001);
    assert!((estimate.total_watts() - 8.8).abs() < 0.0001);

    let snapshot = model.snapshot();
    let mut restored = PowerModel::new(PowerModelMode::All, 25.0, Vec::new()).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.estimate(&residency).unwrap(), estimate);
}

#[test]
fn power_model_modes_and_errors_are_explicit() {
    let dynamic_only = PowerModel::new(
        PowerModelMode::DynamicOnly,
        25.0,
        vec![PowerStatePower::new(PowerStateKind::On, 5.0, 7.0).unwrap()],
    )
    .unwrap();
    let static_only = PowerModel::new(
        PowerModelMode::StaticOnly,
        25.0,
        vec![PowerStatePower::new(PowerStateKind::On, 5.0, 7.0).unwrap()],
    )
    .unwrap();
    let residency = PowerResidency::new(vec![(PowerStateKind::On, 10)]);

    assert_eq!(
        dynamic_only.estimate(&residency).unwrap().static_watts(),
        0.0
    );
    assert_eq!(
        static_only.estimate(&residency).unwrap().dynamic_watts(),
        0.0
    );
    assert_eq!(
        dynamic_only
            .estimate(&PowerResidency::new(vec![(PowerStateKind::ClockGated, 1)]))
            .unwrap_err(),
        PowerError::MissingPowerStateModel {
            state: PowerStateKind::ClockGated,
        }
    );
    assert_eq!(
        PowerStatePower::new(PowerStateKind::Undefined, 1.0, 1.0).unwrap_err(),
        PowerError::UndefinedState,
    );
    assert_eq!(
        PowerModel::new(PowerModelMode::All, f64::NAN, Vec::new()).unwrap_err(),
        PowerError::InvalidTemperature,
    );
    assert_eq!(
        PowerModel::new(
            PowerModelMode::All,
            25.0,
            vec![PowerStatePower::new(PowerStateKind::On, 1.0, 1.0).unwrap()],
        )
        .unwrap()
        .restore(&PowerModelSnapshot::new(
            PowerModelMode::StaticOnly,
            25.0,
            Vec::new(),
        ))
        .unwrap_err(),
        PowerError::PowerModelModeMismatch {
            expected: PowerModelMode::All,
            actual: PowerModelMode::StaticOnly,
        }
    );
}
