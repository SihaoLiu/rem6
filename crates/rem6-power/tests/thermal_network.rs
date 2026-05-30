use rem6_power::{
    PowerEstimate, PowerExpressionInputs, ThermalDomainId, ThermalError, ThermalNetwork,
    ThermalNetworkNodeInitialization, ThermalNetworkNodeKind, ThermalNetworkSnapshot,
    ThermalNodeId,
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
fn thermal_network_capacitor_edges_couple_domain_temperatures() {
    let cpu_domain = ThermalDomainId::new(1);
    let gpu_domain = ThermalDomainId::new(2);
    let cpu = ThermalNodeId::new(10);
    let gpu = ThermalNodeId::new(20);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_domain(gpu, gpu_domain, 25.0, 10.0).unwrap();
    network.add_capacitor(cpu, gpu, 5.0).unwrap();

    network
        .advance(10, vec![(cpu_domain, PowerEstimate::new(10.0, 0.0))])
        .unwrap();

    assert_close(network.temperature_for_domain(cpu_domain).unwrap(), 25.75);
    assert_close(network.temperature_for_domain(gpu_domain).unwrap(), 25.25);

    let snapshot = network.snapshot();
    assert_eq!(snapshot.capacitors().len(), 1);
    let mut restored = ThermalNetwork::new(1.0).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn thermal_network_capacitor_reference_edges_add_thermal_inertia() {
    let cpu_domain = ThermalDomainId::new(1);
    let cpu = ThermalNodeId::new(10);
    let air = ThermalNodeId::new(99);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_reference(air, 10.0).unwrap();
    network.add_capacitor(cpu, air, 5.0).unwrap();

    network
        .advance(10, vec![(cpu_domain, PowerEstimate::new(10.0, 0.0))])
        .unwrap();

    assert_close(
        network.temperature_for_domain(cpu_domain).unwrap(),
        25.6666667,
    );
}

#[test]
fn thermal_network_junction_nodes_require_explicit_initial_temperature() {
    let cpu_domain = ThermalDomainId::new(1);
    let cpu = ThermalNodeId::new(10);
    let spreader = ThermalNodeId::new(20);
    let air = ThermalNodeId::new(99);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_junction(spreader, 25.0, 5.0).unwrap();
    network.add_reference(air, 25.0).unwrap();
    network.add_resistor(cpu, spreader, 1.0).unwrap();
    network.add_resistor(spreader, air, 1.0).unwrap();

    network
        .advance(10, vec![(cpu_domain, PowerEstimate::new(10.0, 0.0))])
        .unwrap();

    assert_close(
        network.temperature_for_domain(cpu_domain).unwrap(),
        25.9210526,
    );
    assert_close(network.temperature_for_node(spreader).unwrap(), 25.1315789);
    let snapshot = network.snapshot();
    assert_eq!(snapshot.nodes()[1].node(), spreader);
    assert_eq!(snapshot.nodes()[1].domain(), None);
    assert_close(snapshot.nodes()[1].temperature_c(), 25.1315789);
    assert_eq!(snapshot.nodes()[1].capacitance_j_per_c(), Some(5.0));

    let mut restored = ThermalNetwork::new(1.0).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_close(restored.temperature_for_node(spreader).unwrap(), 25.1315789);
}

#[test]
fn thermal_network_records_initial_node_evidence() {
    let cpu_domain = ThermalDomainId::new(1);
    let cpu = ThermalNodeId::new(10);
    let spreader = ThermalNodeId::new(20);
    let air = ThermalNodeId::new(99);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 31.0, 10.0).unwrap();
    network.add_junction(spreader, 30.0, 5.0).unwrap();
    network.add_reference(air, 28.0).unwrap();
    network.add_resistor(cpu, spreader, 1.0).unwrap();
    network.add_resistor(spreader, air, 1.0).unwrap();

    network
        .advance(10, vec![(cpu_domain, PowerEstimate::new(10.0, 0.0))])
        .unwrap();

    let records = network.initial_nodes();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].kind(), ThermalNetworkNodeKind::Domain);
    assert_eq!(records[0].node(), cpu);
    assert_eq!(records[0].domain(), Some(cpu_domain));
    assert_close(records[0].initial_temperature_c(), 31.0);
    assert_eq!(records[0].capacitance_j_per_c(), Some(10.0));
    assert_eq!(records[1].kind(), ThermalNetworkNodeKind::Junction);
    assert_eq!(records[1].node(), spreader);
    assert_eq!(records[1].domain(), None);
    assert_close(records[1].initial_temperature_c(), 30.0);
    assert_eq!(records[1].capacitance_j_per_c(), Some(5.0));
    assert_eq!(records[2].kind(), ThermalNetworkNodeKind::Reference);
    assert_eq!(records[2].node(), air);
    assert_eq!(records[2].domain(), None);
    assert_close(records[2].initial_temperature_c(), 28.0);
    assert_eq!(records[2].capacitance_j_per_c(), None);

    assert_ne!(
        network.temperature_for_node(spreader).unwrap(),
        records[1].initial_temperature_c()
    );

    let snapshot = network.snapshot();
    assert_eq!(snapshot.initial_nodes(), records);
    let mut restored = ThermalNetwork::new(1.0).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.initial_nodes(), records);
}

#[test]
fn thermal_network_restore_rejects_mismatched_initial_node_evidence() {
    let cpu_domain = ThermalDomainId::new(1);
    let cpu = ThermalNodeId::new(10);
    let spreader = ThermalNodeId::new(20);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_junction(spreader, 25.0, 5.0).unwrap();
    let snapshot = network.snapshot();
    let mut initial_nodes = snapshot.initial_nodes().to_vec();
    initial_nodes[1] = ThermalNetworkNodeInitialization::new(
        spreader,
        ThermalNetworkNodeKind::Reference,
        None,
        25.0,
        None,
    );

    let mismatched = ThermalNetworkSnapshot::with_initial_nodes(
        snapshot.step_seconds(),
        snapshot.last_tick(),
        snapshot.nodes().to_vec(),
        initial_nodes,
        snapshot.resistors().to_vec(),
        snapshot.capacitors().to_vec(),
        snapshot.updates().to_vec(),
    );

    let mut restored = ThermalNetwork::new(1.0).unwrap();
    assert_eq!(
        restored.restore(&mismatched).unwrap_err(),
        ThermalError::ThermalNodeInitializationMismatch { node: spreader },
    );
}

#[test]
fn thermal_network_restore_rejects_partial_initial_node_evidence() {
    let cpu_domain = ThermalDomainId::new(1);
    let cpu = ThermalNodeId::new(10);
    let spreader = ThermalNodeId::new(20);
    let mut network = ThermalNetwork::new(1.0).unwrap();
    network.add_domain(cpu, cpu_domain, 25.0, 10.0).unwrap();
    network.add_junction(spreader, 25.0, 5.0).unwrap();
    let snapshot = network.snapshot();
    let partial_initial_nodes = vec![snapshot.initial_nodes()[0].clone()];

    let partial = ThermalNetworkSnapshot::with_initial_nodes(
        snapshot.step_seconds(),
        snapshot.last_tick(),
        snapshot.nodes().to_vec(),
        partial_initial_nodes,
        snapshot.resistors().to_vec(),
        snapshot.capacitors().to_vec(),
        snapshot.updates().to_vec(),
    );

    let mut restored = ThermalNetwork::new(1.0).unwrap();
    assert_eq!(
        restored.restore(&partial).unwrap_err(),
        ThermalError::MissingThermalNodeInitialization { node: spreader },
    );
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
    let mut passive_only = ThermalNetwork::new(1.0).unwrap();
    passive_only
        .add_junction(ThermalNodeId::new(20), 25.0, 1.0)
        .unwrap();
    assert_eq!(
        passive_only.advance(1, Vec::new()).unwrap_err(),
        ThermalError::NoThermalDomains,
    );
    assert_eq!(
        network
            .add_junction(ThermalNodeId::new(12), -273.15, 1.0)
            .unwrap_err(),
        ThermalError::InvalidTemperature,
    );
    assert_eq!(
        network
            .add_reference(ThermalNodeId::new(13), -274.0)
            .unwrap_err(),
        ThermalError::InvalidTemperature,
    );
    assert_eq!(
        network
            .add_domain(
                ThermalNodeId::new(14),
                ThermalDomainId::new(14),
                -274.0,
                1.0
            )
            .unwrap_err(),
        ThermalError::InvalidTemperature,
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
        network.add_capacitor(cpu, cpu, 1.0).unwrap_err(),
        ThermalError::ThermalSelfConnection { node: cpu },
    );
    assert_eq!(
        network.add_capacitor(cpu, air, 0.0).unwrap_err(),
        ThermalError::InvalidThermalCapacitance,
    );
    assert_eq!(
        network
            .add_capacitor(cpu, ThermalNodeId::new(77), 1.0)
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
                Vec::new(),
            ))
            .unwrap_err(),
        ThermalError::NoThermalDomains,
    );
}
