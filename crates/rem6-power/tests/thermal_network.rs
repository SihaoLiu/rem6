use rem6_power::{
    PowerEstimate, PowerExpressionInputs, ThermalDomainId, ThermalError, ThermalNetwork,
    ThermalNetworkSnapshot, ThermalNodeId,
};

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.0001,
        "actual {actual} did not match expected {expected}"
    );
}

#[test]
fn thermal_network_solves_coupled_domain_temperatures() {
    let cpu_domain = ThermalDomainId::new(1);
    let gpu_domain = ThermalDomainId::new(2);
    let cpu = ThermalNodeId::new(10);
    let gpu = ThermalNodeId::new(20);
    let air = ThermalNodeId::new(99);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_domain(gpu, gpu_domain, 25.0, 10.0).unwrap();
    network.add_reference(air, 25.0).unwrap();
    network.add_resistor(cpu, air, 2.0).unwrap();
    network.add_resistor(gpu, air, 2.0).unwrap();
    network.add_resistor(cpu, gpu, 1.0).unwrap();

    let updates = network
        .advance(10, vec![(cpu_domain, PowerEstimate::new(8.0, 2.0))])
        .unwrap();

    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].domain(), cpu_domain);
    assert_eq!(updates[1].domain(), gpu_domain);
    assert_close(
        network.temperature_for_domain(cpu_domain).unwrap(),
        25.8761905,
    );
    assert_close(
        network.temperature_for_domain(gpu_domain).unwrap(),
        25.0761905,
    );
    assert_close(updates[0].total_power_watts(), 10.0);
    assert_close(updates[1].total_power_watts(), 0.0);

    let inputs = PowerExpressionInputs::new(25.0, 0.8, 2.0)
        .unwrap()
        .with_thermal_network_domain(&network, cpu_domain)
        .unwrap();
    assert_close(inputs.temperature_c(), 25.8761905);

    let snapshot = network.snapshot();
    let mut restored = ThermalNetwork::new(1.0).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.updates(), network.updates());
}

#[test]
fn thermal_network_rejects_invalid_topology_and_runtime_state() {
    let cpu_domain = ThermalDomainId::new(1);
    let cpu = ThermalNodeId::new(10);
    let air = ThermalNodeId::new(99);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_reference(air, 25.0).unwrap();
    network.add_resistor(cpu, air, 2.0).unwrap();
    network
        .advance(10, vec![(cpu_domain, PowerEstimate::new(1.0, 0.0))])
        .unwrap();

    assert_eq!(
        ThermalNetwork::new(0.0).unwrap_err(),
        ThermalError::InvalidThermalStep,
    );
    assert_eq!(
        network
            .add_domain(ThermalNodeId::new(11), cpu_domain, 25.0, 10.0)
            .unwrap_err(),
        ThermalError::DuplicateThermalDomain { domain: cpu_domain },
    );
    assert_eq!(
        network.add_reference(cpu, 25.0).unwrap_err(),
        ThermalError::DuplicateThermalNode { node: cpu },
    );
    assert_eq!(
        network
            .add_resistor(cpu, ThermalNodeId::new(77), 1.0)
            .unwrap_err(),
        ThermalError::UnknownThermalNode {
            node: ThermalNodeId::new(77),
        },
    );
    assert_eq!(
        network
            .advance(
                11,
                vec![(ThermalDomainId::new(77), PowerEstimate::new(1.0, 0.0))],
            )
            .unwrap_err(),
        ThermalError::UnknownThermalDomain {
            domain: ThermalDomainId::new(77),
        },
    );
    assert_eq!(
        network
            .advance(9, vec![(cpu_domain, PowerEstimate::new(1.0, 0.0))])
            .unwrap_err(),
        ThermalError::TimeWentBack {
            tick: 9,
            last_tick: 10,
        },
    );
    assert_eq!(
        network
            .restore(&ThermalNetworkSnapshot::new(
                1.0,
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ))
            .unwrap_err(),
        ThermalError::NoThermalDomains,
    );
}
