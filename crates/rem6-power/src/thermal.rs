use rem6_kernel::Tick;
use std::collections::{BTreeMap, BTreeSet};

use crate::PowerEstimate;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThermalDomainId(u64);

impl ThermalDomainId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThermalNodeId(u64);

impl ThermalNodeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalUpdate {
    tick: Tick,
    domain: ThermalDomainId,
    previous_temperature_c: f64,
    temperature_c: f64,
    total_power_watts: f64,
}

impl ThermalUpdate {
    pub const fn new(
        tick: Tick,
        domain: ThermalDomainId,
        previous_temperature_c: f64,
        temperature_c: f64,
        total_power_watts: f64,
    ) -> Self {
        Self {
            tick,
            domain,
            previous_temperature_c,
            temperature_c,
            total_power_watts,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn domain(&self) -> ThermalDomainId {
        self.domain
    }

    pub const fn previous_temperature_c(&self) -> f64 {
        self.previous_temperature_c
    }

    pub const fn temperature_c(&self) -> f64 {
        self.temperature_c
    }

    pub const fn total_power_watts(&self) -> f64 {
        self.total_power_watts
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalResistor {
    left: ThermalNodeId,
    right: ThermalNodeId,
    resistance_c_per_w: f64,
}

impl ThermalResistor {
    pub fn new(
        left: ThermalNodeId,
        right: ThermalNodeId,
        resistance_c_per_w: f64,
    ) -> Result<Self, ThermalError> {
        if left == right {
            return Err(ThermalError::ThermalSelfConnection { node: left });
        }
        validate_positive(resistance_c_per_w, ThermalError::InvalidThermalResistance)?;
        Ok(Self {
            left,
            right,
            resistance_c_per_w,
        })
    }

    pub const fn left(&self) -> ThermalNodeId {
        self.left
    }

    pub const fn right(&self) -> ThermalNodeId {
        self.right
    }

    pub const fn resistance_c_per_w(&self) -> f64 {
        self.resistance_c_per_w
    }

    fn conductance(&self) -> f64 {
        1.0 / self.resistance_c_per_w
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalCapacitor {
    left: ThermalNodeId,
    right: ThermalNodeId,
    capacitance_j_per_c: f64,
}

impl ThermalCapacitor {
    pub fn new(
        left: ThermalNodeId,
        right: ThermalNodeId,
        capacitance_j_per_c: f64,
    ) -> Result<Self, ThermalError> {
        if left == right {
            return Err(ThermalError::ThermalSelfConnection { node: left });
        }
        validate_positive(capacitance_j_per_c, ThermalError::InvalidThermalCapacitance)?;
        Ok(Self {
            left,
            right,
            capacitance_j_per_c,
        })
    }

    pub const fn left(&self) -> ThermalNodeId {
        self.left
    }

    pub const fn right(&self) -> ThermalNodeId {
        self.right
    }

    pub const fn capacitance_j_per_c(&self) -> f64 {
        self.capacitance_j_per_c
    }

    fn capacitance_per_step(&self, step_seconds: f64) -> f64 {
        self.capacitance_j_per_c / step_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ThermalNetworkNodeKind {
    Domain,
    Junction,
    Reference,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalNetworkNodeInitialization {
    node: ThermalNodeId,
    kind: ThermalNetworkNodeKind,
    domain: Option<ThermalDomainId>,
    initial_temperature_c: f64,
    capacitance_j_per_c: Option<f64>,
}

impl ThermalNetworkNodeInitialization {
    pub const fn new(
        node: ThermalNodeId,
        kind: ThermalNetworkNodeKind,
        domain: Option<ThermalDomainId>,
        initial_temperature_c: f64,
        capacitance_j_per_c: Option<f64>,
    ) -> Self {
        Self {
            node,
            kind,
            domain,
            initial_temperature_c,
            capacitance_j_per_c,
        }
    }

    pub const fn node(&self) -> ThermalNodeId {
        self.node
    }

    pub const fn kind(&self) -> ThermalNetworkNodeKind {
        self.kind
    }

    pub const fn domain(&self) -> Option<ThermalDomainId> {
        self.domain
    }

    pub const fn initial_temperature_c(&self) -> f64 {
        self.initial_temperature_c
    }

    pub const fn capacitance_j_per_c(&self) -> Option<f64> {
        self.capacitance_j_per_c
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalNetworkNodeSnapshot {
    node: ThermalNodeId,
    domain: Option<ThermalDomainId>,
    temperature_c: f64,
    capacitance_j_per_c: Option<f64>,
}

impl ThermalNetworkNodeSnapshot {
    pub const fn new(
        node: ThermalNodeId,
        domain: Option<ThermalDomainId>,
        temperature_c: f64,
        capacitance_j_per_c: Option<f64>,
    ) -> Self {
        Self {
            node,
            domain,
            temperature_c,
            capacitance_j_per_c,
        }
    }

    pub const fn node(&self) -> ThermalNodeId {
        self.node
    }

    pub const fn domain(&self) -> Option<ThermalDomainId> {
        self.domain
    }

    pub const fn temperature_c(&self) -> f64 {
        self.temperature_c
    }

    pub const fn capacitance_j_per_c(&self) -> Option<f64> {
        self.capacitance_j_per_c
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalNetworkSnapshot {
    step_seconds: f64,
    last_tick: Tick,
    nodes: Vec<ThermalNetworkNodeSnapshot>,
    initial_nodes: Vec<ThermalNetworkNodeInitialization>,
    resistors: Vec<ThermalResistor>,
    capacitors: Vec<ThermalCapacitor>,
    updates: Vec<ThermalUpdate>,
}

impl ThermalNetworkSnapshot {
    pub const fn new(
        step_seconds: f64,
        last_tick: Tick,
        nodes: Vec<ThermalNetworkNodeSnapshot>,
        resistors: Vec<ThermalResistor>,
        capacitors: Vec<ThermalCapacitor>,
        updates: Vec<ThermalUpdate>,
    ) -> Self {
        Self {
            step_seconds,
            last_tick,
            nodes,
            initial_nodes: Vec::new(),
            resistors,
            capacitors,
            updates,
        }
    }

    pub const fn with_initial_nodes(
        step_seconds: f64,
        last_tick: Tick,
        nodes: Vec<ThermalNetworkNodeSnapshot>,
        initial_nodes: Vec<ThermalNetworkNodeInitialization>,
        resistors: Vec<ThermalResistor>,
        capacitors: Vec<ThermalCapacitor>,
        updates: Vec<ThermalUpdate>,
    ) -> Self {
        Self {
            step_seconds,
            last_tick,
            nodes,
            initial_nodes,
            resistors,
            capacitors,
            updates,
        }
    }

    pub const fn step_seconds(&self) -> f64 {
        self.step_seconds
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub fn nodes(&self) -> &[ThermalNetworkNodeSnapshot] {
        &self.nodes
    }

    pub fn initial_nodes(&self) -> &[ThermalNetworkNodeInitialization] {
        &self.initial_nodes
    }

    pub fn resistors(&self) -> &[ThermalResistor] {
        &self.resistors
    }

    pub fn capacitors(&self) -> &[ThermalCapacitor] {
        &self.capacitors
    }

    pub fn updates(&self) -> &[ThermalUpdate] {
        &self.updates
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalNetwork {
    step_seconds: f64,
    last_tick: Tick,
    nodes: BTreeMap<ThermalNodeId, ThermalNetworkNode>,
    initial_nodes: BTreeMap<ThermalNodeId, ThermalNetworkNodeInitialization>,
    domains: BTreeMap<ThermalDomainId, ThermalNodeId>,
    resistors: Vec<ThermalResistor>,
    capacitors: Vec<ThermalCapacitor>,
    updates: Vec<ThermalUpdate>,
}

impl ThermalNetwork {
    pub fn new(step_seconds: f64) -> Result<Self, ThermalError> {
        validate_positive(step_seconds, ThermalError::InvalidThermalStep)?;
        Ok(Self {
            step_seconds,
            last_tick: 0,
            nodes: BTreeMap::new(),
            initial_nodes: BTreeMap::new(),
            domains: BTreeMap::new(),
            resistors: Vec::new(),
            capacitors: Vec::new(),
            updates: Vec::new(),
        })
    }

    pub const fn step_seconds(&self) -> f64 {
        self.step_seconds
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub fn updates(&self) -> &[ThermalUpdate] {
        &self.updates
    }

    pub fn initial_nodes(&self) -> Vec<ThermalNetworkNodeInitialization> {
        self.initial_nodes.values().cloned().collect()
    }

    pub fn add_domain(
        &mut self,
        node: ThermalNodeId,
        domain: ThermalDomainId,
        initial_temperature_c: f64,
        capacitance_j_per_c: f64,
    ) -> Result<(), ThermalError> {
        if self.nodes.contains_key(&node) {
            return Err(ThermalError::DuplicateThermalNode { node });
        }
        if self.domains.contains_key(&domain) {
            return Err(ThermalError::DuplicateThermalDomain { domain });
        }
        validate_temperature(initial_temperature_c)?;
        validate_positive(capacitance_j_per_c, ThermalError::InvalidThermalCapacitance)?;
        self.nodes.insert(
            node,
            ThermalNetworkNode::Domain {
                domain,
                temperature_c: initial_temperature_c,
                capacitance_j_per_c,
            },
        );
        self.initial_nodes.insert(
            node,
            ThermalNetworkNodeInitialization::new(
                node,
                ThermalNetworkNodeKind::Domain,
                Some(domain),
                initial_temperature_c,
                Some(capacitance_j_per_c),
            ),
        );
        self.domains.insert(domain, node);
        Ok(())
    }

    pub fn add_reference(
        &mut self,
        node: ThermalNodeId,
        temperature_c: f64,
    ) -> Result<(), ThermalError> {
        if self.nodes.contains_key(&node) {
            return Err(ThermalError::DuplicateThermalNode { node });
        }
        validate_temperature(temperature_c)?;
        self.nodes
            .insert(node, ThermalNetworkNode::Reference { temperature_c });
        self.initial_nodes.insert(
            node,
            ThermalNetworkNodeInitialization::new(
                node,
                ThermalNetworkNodeKind::Reference,
                None,
                temperature_c,
                None,
            ),
        );
        Ok(())
    }

    pub fn add_junction(
        &mut self,
        node: ThermalNodeId,
        initial_temperature_c: f64,
        capacitance_j_per_c: f64,
    ) -> Result<(), ThermalError> {
        if self.nodes.contains_key(&node) {
            return Err(ThermalError::DuplicateThermalNode { node });
        }
        validate_temperature(initial_temperature_c)?;
        validate_positive(capacitance_j_per_c, ThermalError::InvalidThermalCapacitance)?;
        self.nodes.insert(
            node,
            ThermalNetworkNode::Junction {
                temperature_c: initial_temperature_c,
                capacitance_j_per_c,
            },
        );
        self.initial_nodes.insert(
            node,
            ThermalNetworkNodeInitialization::new(
                node,
                ThermalNetworkNodeKind::Junction,
                None,
                initial_temperature_c,
                Some(capacitance_j_per_c),
            ),
        );
        Ok(())
    }

    pub fn add_resistor(
        &mut self,
        left: ThermalNodeId,
        right: ThermalNodeId,
        resistance_c_per_w: f64,
    ) -> Result<(), ThermalError> {
        self.require_node(left)?;
        self.require_node(right)?;
        self.resistors
            .push(ThermalResistor::new(left, right, resistance_c_per_w)?);
        Ok(())
    }

    pub fn add_capacitor(
        &mut self,
        left: ThermalNodeId,
        right: ThermalNodeId,
        capacitance_j_per_c: f64,
    ) -> Result<(), ThermalError> {
        self.require_node(left)?;
        self.require_node(right)?;
        self.capacitors
            .push(ThermalCapacitor::new(left, right, capacitance_j_per_c)?);
        Ok(())
    }

    pub fn temperature_for_domain(&self, domain: ThermalDomainId) -> Result<f64, ThermalError> {
        let node = self
            .domains
            .get(&domain)
            .copied()
            .ok_or(ThermalError::UnknownThermalDomain { domain })?;
        self.nodes
            .get(&node)
            .and_then(ThermalNetworkNode::domain_temperature_c)
            .ok_or(ThermalError::UnknownThermalDomain { domain })
    }

    pub fn temperature_for_node(&self, node: ThermalNodeId) -> Result<f64, ThermalError> {
        self.nodes
            .get(&node)
            .map(ThermalNetworkNode::temperature_c)
            .ok_or(ThermalError::UnknownThermalNode { node })
    }

    pub fn advance(
        &mut self,
        tick: Tick,
        powers: Vec<(ThermalDomainId, PowerEstimate)>,
    ) -> Result<Vec<ThermalUpdate>, ThermalError> {
        if tick < self.last_tick {
            return Err(ThermalError::TimeWentBack {
                tick,
                last_tick: self.last_tick,
            });
        }
        if self.domains.is_empty() {
            return Err(ThermalError::NoThermalDomains);
        }
        let entries = self.solve_entries();
        if entries.is_empty() {
            return Err(ThermalError::NoThermalDomains);
        }
        let power_map = self.power_map(powers)?;
        let mut index_by_node = BTreeMap::new();
        for (index, entry) in entries.iter().enumerate() {
            index_by_node.insert(entry.node, index);
        }

        let n = entries.len();
        let mut matrix = vec![vec![0.0; n]; n];
        let mut rhs = vec![0.0; n];
        for (index, entry) in entries.iter().enumerate() {
            let c_over_step = entry.capacitance_j_per_c / self.step_seconds;
            matrix[index][index] += c_over_step;
            rhs[index] += c_over_step * entry.temperature_c
                + entry
                    .domain
                    .and_then(|domain| power_map.get(&domain).copied())
                    .unwrap_or_default();
        }
        for resistor in &self.resistors {
            self.apply_resistor(resistor, &index_by_node, &mut matrix, &mut rhs)?;
        }
        for capacitor in &self.capacitors {
            self.apply_capacitor(capacitor, &index_by_node, &mut matrix, &mut rhs)?;
        }

        let temperatures = solve_linear_system(matrix, rhs)?;
        let mut updates = Vec::new();
        for (entry, temperature_c) in entries.iter().zip(temperatures) {
            validate_temperature(temperature_c)?;
            let Some(node) = self.nodes.get_mut(&entry.node) else {
                return Err(ThermalError::UnknownThermalNode { node: entry.node });
            };
            node.set_temperature_c(temperature_c);
            if let Some(domain) = entry.domain {
                let total_power_watts = power_map.get(&domain).copied().unwrap_or_default();
                updates.push(ThermalUpdate::new(
                    tick,
                    domain,
                    entry.temperature_c,
                    temperature_c,
                    total_power_watts,
                ));
            }
        }
        self.last_tick = tick;
        self.updates.extend(updates.iter().copied());
        Ok(updates)
    }

    pub fn snapshot(&self) -> ThermalNetworkSnapshot {
        let nodes = self
            .nodes
            .iter()
            .map(|(node, record)| match record {
                ThermalNetworkNode::Domain {
                    domain,
                    temperature_c,
                    capacitance_j_per_c,
                } => ThermalNetworkNodeSnapshot::new(
                    *node,
                    Some(*domain),
                    *temperature_c,
                    Some(*capacitance_j_per_c),
                ),
                ThermalNetworkNode::Reference { temperature_c } => {
                    ThermalNetworkNodeSnapshot::new(*node, None, *temperature_c, None)
                }
                ThermalNetworkNode::Junction {
                    temperature_c,
                    capacitance_j_per_c,
                } => ThermalNetworkNodeSnapshot::new(
                    *node,
                    None,
                    *temperature_c,
                    Some(*capacitance_j_per_c),
                ),
            })
            .collect();
        ThermalNetworkSnapshot::with_initial_nodes(
            self.step_seconds,
            self.last_tick,
            nodes,
            self.initial_nodes(),
            self.resistors.clone(),
            self.capacitors.clone(),
            self.updates.clone(),
        )
    }

    pub fn restore(&mut self, snapshot: &ThermalNetworkSnapshot) -> Result<(), ThermalError> {
        validate_positive(snapshot.step_seconds(), ThermalError::InvalidThermalStep)?;
        let mut nodes = BTreeMap::new();
        let mut initial_nodes = BTreeMap::new();
        let mut domains = BTreeMap::new();
        for node_snapshot in snapshot.nodes() {
            if nodes.contains_key(&node_snapshot.node()) {
                return Err(ThermalError::DuplicateThermalNode {
                    node: node_snapshot.node(),
                });
            }
            validate_temperature(node_snapshot.temperature_c())?;
            let record = if let Some(domain) = node_snapshot.domain() {
                let capacitance_j_per_c = node_snapshot
                    .capacitance_j_per_c()
                    .ok_or(ThermalError::InvalidThermalCapacitance)?;
                validate_positive(capacitance_j_per_c, ThermalError::InvalidThermalCapacitance)?;
                if domains.insert(domain, node_snapshot.node()).is_some() {
                    return Err(ThermalError::DuplicateThermalDomain { domain });
                }
                ThermalNetworkNode::Domain {
                    domain,
                    temperature_c: node_snapshot.temperature_c(),
                    capacitance_j_per_c,
                }
            } else if let Some(capacitance_j_per_c) = node_snapshot.capacitance_j_per_c() {
                validate_positive(capacitance_j_per_c, ThermalError::InvalidThermalCapacitance)?;
                ThermalNetworkNode::Junction {
                    temperature_c: node_snapshot.temperature_c(),
                    capacitance_j_per_c,
                }
            } else {
                ThermalNetworkNode::Reference {
                    temperature_c: node_snapshot.temperature_c(),
                }
            };
            nodes.insert(node_snapshot.node(), record);
        }
        for initial in snapshot.initial_nodes() {
            validate_node_initialization(initial, &nodes, &domains)?;
            if initial_nodes
                .insert(initial.node(), initial.clone())
                .is_some()
            {
                return Err(ThermalError::DuplicateThermalNode {
                    node: initial.node(),
                });
            }
        }
        if initial_nodes.is_empty() {
            initial_nodes = derive_initial_nodes(&nodes);
        } else {
            for node in nodes.keys() {
                if !initial_nodes.contains_key(node) {
                    return Err(ThermalError::MissingThermalNodeInitialization { node: *node });
                }
            }
        }
        if domains.is_empty() {
            return Err(ThermalError::NoThermalDomains);
        }
        for resistor in snapshot.resistors() {
            if !nodes.contains_key(&resistor.left()) {
                return Err(ThermalError::UnknownThermalNode {
                    node: resistor.left(),
                });
            }
            if !nodes.contains_key(&resistor.right()) {
                return Err(ThermalError::UnknownThermalNode {
                    node: resistor.right(),
                });
            }
            ThermalResistor::new(
                resistor.left(),
                resistor.right(),
                resistor.resistance_c_per_w(),
            )?;
        }
        for capacitor in snapshot.capacitors() {
            if !nodes.contains_key(&capacitor.left()) {
                return Err(ThermalError::UnknownThermalNode {
                    node: capacitor.left(),
                });
            }
            if !nodes.contains_key(&capacitor.right()) {
                return Err(ThermalError::UnknownThermalNode {
                    node: capacitor.right(),
                });
            }
            ThermalCapacitor::new(
                capacitor.left(),
                capacitor.right(),
                capacitor.capacitance_j_per_c(),
            )?;
        }

        self.step_seconds = snapshot.step_seconds();
        self.last_tick = snapshot.last_tick();
        self.nodes = nodes;
        self.initial_nodes = initial_nodes;
        self.domains = domains;
        self.resistors = snapshot.resistors().to_vec();
        self.capacitors = snapshot.capacitors().to_vec();
        self.updates = snapshot.updates().to_vec();
        Ok(())
    }

    fn require_node(&self, node: ThermalNodeId) -> Result<(), ThermalError> {
        if !self.nodes.contains_key(&node) {
            return Err(ThermalError::UnknownThermalNode { node });
        }
        Ok(())
    }

    fn power_map(
        &self,
        powers: Vec<(ThermalDomainId, PowerEstimate)>,
    ) -> Result<BTreeMap<ThermalDomainId, f64>, ThermalError> {
        let mut map = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for (domain, estimate) in powers {
            if !self.domains.contains_key(&domain) {
                return Err(ThermalError::UnknownThermalDomain { domain });
            }
            if !seen.insert(domain) {
                return Err(ThermalError::DuplicatePowerInput { domain });
            }
            let power = estimate.total_watts();
            validate_nonnegative(power, ThermalError::InvalidPowerInput)?;
            map.insert(domain, power);
        }
        Ok(map)
    }

    fn solve_entries(&self) -> Vec<ThermalSolveEntry> {
        self.nodes
            .iter()
            .filter_map(|(node, record)| match record {
                ThermalNetworkNode::Domain {
                    domain,
                    temperature_c,
                    capacitance_j_per_c,
                } => Some(ThermalSolveEntry {
                    node: *node,
                    domain: Some(*domain),
                    temperature_c: *temperature_c,
                    capacitance_j_per_c: *capacitance_j_per_c,
                }),
                ThermalNetworkNode::Junction {
                    temperature_c,
                    capacitance_j_per_c,
                } => Some(ThermalSolveEntry {
                    node: *node,
                    domain: None,
                    temperature_c: *temperature_c,
                    capacitance_j_per_c: *capacitance_j_per_c,
                }),
                ThermalNetworkNode::Reference { .. } => None,
            })
            .collect()
    }

    fn apply_resistor(
        &self,
        resistor: &ThermalResistor,
        index_by_node: &BTreeMap<ThermalNodeId, usize>,
        matrix: &mut [Vec<f64>],
        rhs: &mut [f64],
    ) -> Result<(), ThermalError> {
        let conductance = resistor.conductance();
        let left = self
            .nodes
            .get(&resistor.left())
            .ok_or(ThermalError::UnknownThermalNode {
                node: resistor.left(),
            })?;
        let right = self
            .nodes
            .get(&resistor.right())
            .ok_or(ThermalError::UnknownThermalNode {
                node: resistor.right(),
            })?;
        match (
            index_by_node.get(&resistor.left()).copied(),
            index_by_node.get(&resistor.right()).copied(),
        ) {
            (Some(left_index), Some(right_index)) => {
                matrix[left_index][left_index] += conductance;
                matrix[right_index][right_index] += conductance;
                matrix[left_index][right_index] -= conductance;
                matrix[right_index][left_index] -= conductance;
            }
            (Some(left_index), None) => {
                matrix[left_index][left_index] += conductance;
                rhs[left_index] += conductance * right.reference_temperature_c()?;
            }
            (None, Some(right_index)) => {
                matrix[right_index][right_index] += conductance;
                rhs[right_index] += conductance * left.reference_temperature_c()?;
            }
            (None, None) => {}
        }
        Ok(())
    }

    fn apply_capacitor(
        &self,
        capacitor: &ThermalCapacitor,
        index_by_node: &BTreeMap<ThermalNodeId, usize>,
        matrix: &mut [Vec<f64>],
        rhs: &mut [f64],
    ) -> Result<(), ThermalError> {
        let capacitance_per_step = capacitor.capacitance_per_step(self.step_seconds);
        let left = self
            .nodes
            .get(&capacitor.left())
            .ok_or(ThermalError::UnknownThermalNode {
                node: capacitor.left(),
            })?;
        let right = self
            .nodes
            .get(&capacitor.right())
            .ok_or(ThermalError::UnknownThermalNode {
                node: capacitor.right(),
            })?;
        let left_temperature_c = left.temperature_c();
        let right_temperature_c = right.temperature_c();
        match (
            index_by_node.get(&capacitor.left()).copied(),
            index_by_node.get(&capacitor.right()).copied(),
        ) {
            (Some(left_index), Some(right_index)) => {
                matrix[left_index][left_index] += capacitance_per_step;
                matrix[right_index][right_index] += capacitance_per_step;
                matrix[left_index][right_index] -= capacitance_per_step;
                matrix[right_index][left_index] -= capacitance_per_step;
                rhs[left_index] +=
                    capacitance_per_step * (left_temperature_c - right_temperature_c);
                rhs[right_index] +=
                    capacitance_per_step * (right_temperature_c - left_temperature_c);
            }
            (Some(left_index), None) => {
                matrix[left_index][left_index] += capacitance_per_step;
                rhs[left_index] += capacitance_per_step * left_temperature_c;
            }
            (None, Some(right_index)) => {
                matrix[right_index][right_index] += capacitance_per_step;
                rhs[right_index] += capacitance_per_step * right_temperature_c;
            }
            (None, None) => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalRcSnapshot {
    domain: ThermalDomainId,
    ambient_temperature_c: f64,
    current_temperature_c: f64,
    thermal_resistance_c_per_w: f64,
    thermal_capacitance_j_per_c: f64,
    step_seconds: f64,
    last_tick: Tick,
    updates: Vec<ThermalUpdate>,
}

impl ThermalRcSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        domain: ThermalDomainId,
        ambient_temperature_c: f64,
        current_temperature_c: f64,
        thermal_resistance_c_per_w: f64,
        thermal_capacitance_j_per_c: f64,
        step_seconds: f64,
        last_tick: Tick,
        updates: Vec<ThermalUpdate>,
    ) -> Self {
        Self {
            domain,
            ambient_temperature_c,
            current_temperature_c,
            thermal_resistance_c_per_w,
            thermal_capacitance_j_per_c,
            step_seconds,
            last_tick,
            updates,
        }
    }

    pub const fn domain(&self) -> ThermalDomainId {
        self.domain
    }

    pub const fn ambient_temperature_c(&self) -> f64 {
        self.ambient_temperature_c
    }

    pub const fn current_temperature_c(&self) -> f64 {
        self.current_temperature_c
    }

    pub const fn thermal_resistance_c_per_w(&self) -> f64 {
        self.thermal_resistance_c_per_w
    }

    pub const fn thermal_capacitance_j_per_c(&self) -> f64 {
        self.thermal_capacitance_j_per_c
    }

    pub const fn step_seconds(&self) -> f64 {
        self.step_seconds
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub fn updates(&self) -> &[ThermalUpdate] {
        &self.updates
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalRcModel {
    domain: ThermalDomainId,
    ambient_temperature_c: f64,
    current_temperature_c: f64,
    thermal_resistance_c_per_w: f64,
    thermal_capacitance_j_per_c: f64,
    step_seconds: f64,
    last_tick: Tick,
    updates: Vec<ThermalUpdate>,
}

impl ThermalRcModel {
    pub fn new(
        domain: ThermalDomainId,
        ambient_temperature_c: f64,
        thermal_resistance_c_per_w: f64,
        thermal_capacitance_j_per_c: f64,
        step_seconds: f64,
    ) -> Result<Self, ThermalError> {
        validate_temperature(ambient_temperature_c)?;
        validate_positive(
            thermal_resistance_c_per_w,
            ThermalError::InvalidThermalResistance,
        )?;
        validate_positive(
            thermal_capacitance_j_per_c,
            ThermalError::InvalidThermalCapacitance,
        )?;
        validate_positive(step_seconds, ThermalError::InvalidThermalStep)?;
        Ok(Self {
            domain,
            ambient_temperature_c,
            current_temperature_c: ambient_temperature_c,
            thermal_resistance_c_per_w,
            thermal_capacitance_j_per_c,
            step_seconds,
            last_tick: 0,
            updates: Vec::new(),
        })
    }

    pub const fn domain(&self) -> ThermalDomainId {
        self.domain
    }

    pub const fn ambient_temperature_c(&self) -> f64 {
        self.ambient_temperature_c
    }

    pub const fn current_temperature_c(&self) -> f64 {
        self.current_temperature_c
    }

    pub const fn thermal_resistance_c_per_w(&self) -> f64 {
        self.thermal_resistance_c_per_w
    }

    pub const fn thermal_capacitance_j_per_c(&self) -> f64 {
        self.thermal_capacitance_j_per_c
    }

    pub const fn step_seconds(&self) -> f64 {
        self.step_seconds
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub fn updates(&self) -> &[ThermalUpdate] {
        &self.updates
    }

    pub fn advance(
        &mut self,
        tick: Tick,
        estimate: PowerEstimate,
    ) -> Result<ThermalUpdate, ThermalError> {
        if tick < self.last_tick {
            return Err(ThermalError::TimeWentBack {
                tick,
                last_tick: self.last_tick,
            });
        }
        let total_power_watts = estimate.total_watts();
        validate_nonnegative(total_power_watts, ThermalError::InvalidPowerInput)?;
        let previous_temperature_c = self.current_temperature_c;
        let cooling_watts =
            (previous_temperature_c - self.ambient_temperature_c) / self.thermal_resistance_c_per_w;
        let delta_c = (total_power_watts - cooling_watts) * self.step_seconds
            / self.thermal_capacitance_j_per_c;
        let temperature_c = previous_temperature_c + delta_c;
        validate_temperature(temperature_c)?;

        self.current_temperature_c = temperature_c;
        self.last_tick = tick;
        let update = ThermalUpdate::new(
            tick,
            self.domain,
            previous_temperature_c,
            temperature_c,
            total_power_watts,
        );
        self.updates.push(update);
        Ok(update)
    }

    pub fn snapshot(&self) -> ThermalRcSnapshot {
        ThermalRcSnapshot::new(
            self.domain,
            self.ambient_temperature_c,
            self.current_temperature_c,
            self.thermal_resistance_c_per_w,
            self.thermal_capacitance_j_per_c,
            self.step_seconds,
            self.last_tick,
            self.updates.clone(),
        )
    }

    pub fn restore(&mut self, snapshot: &ThermalRcSnapshot) -> Result<(), ThermalError> {
        if snapshot.domain() != self.domain {
            return Err(ThermalError::ThermalDomainMismatch {
                expected: self.domain,
                actual: snapshot.domain(),
            });
        }
        validate_temperature(snapshot.ambient_temperature_c())?;
        validate_temperature(snapshot.current_temperature_c())?;
        validate_positive(
            snapshot.thermal_resistance_c_per_w(),
            ThermalError::InvalidThermalResistance,
        )?;
        validate_positive(
            snapshot.thermal_capacitance_j_per_c(),
            ThermalError::InvalidThermalCapacitance,
        )?;
        validate_positive(snapshot.step_seconds(), ThermalError::InvalidThermalStep)?;
        self.ambient_temperature_c = snapshot.ambient_temperature_c();
        self.current_temperature_c = snapshot.current_temperature_c();
        self.thermal_resistance_c_per_w = snapshot.thermal_resistance_c_per_w();
        self.thermal_capacitance_j_per_c = snapshot.thermal_capacitance_j_per_c();
        self.step_seconds = snapshot.step_seconds();
        self.last_tick = snapshot.last_tick();
        self.updates = snapshot.updates().to_vec();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ThermalSolveEntry {
    node: ThermalNodeId,
    domain: Option<ThermalDomainId>,
    temperature_c: f64,
    capacitance_j_per_c: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum ThermalNetworkNode {
    Domain {
        domain: ThermalDomainId,
        temperature_c: f64,
        capacitance_j_per_c: f64,
    },
    Junction {
        temperature_c: f64,
        capacitance_j_per_c: f64,
    },
    Reference {
        temperature_c: f64,
    },
}

impl ThermalNetworkNode {
    fn temperature_c(&self) -> f64 {
        match self {
            Self::Domain { temperature_c, .. }
            | Self::Junction { temperature_c, .. }
            | Self::Reference { temperature_c } => *temperature_c,
        }
    }

    fn set_temperature_c(&mut self, next_temperature_c: f64) {
        match self {
            Self::Domain { temperature_c, .. } | Self::Junction { temperature_c, .. } => {
                *temperature_c = next_temperature_c;
            }
            Self::Reference { .. } => {}
        }
    }

    fn domain_temperature_c(&self) -> Option<f64> {
        match self {
            Self::Domain { temperature_c, .. } => Some(*temperature_c),
            Self::Junction { .. } | Self::Reference { .. } => None,
        }
    }

    fn reference_temperature_c(&self) -> Result<f64, ThermalError> {
        match self {
            Self::Reference { temperature_c } => Ok(*temperature_c),
            Self::Domain { .. } | Self::Junction { .. } => {
                Err(ThermalError::SingularThermalNetwork)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThermalError {
    InvalidTemperature,
    InvalidThermalResistance,
    InvalidThermalCapacitance,
    InvalidThermalStep,
    InvalidPowerInput,
    DuplicateThermalNode {
        node: ThermalNodeId,
    },
    DuplicateThermalDomain {
        domain: ThermalDomainId,
    },
    DuplicatePowerInput {
        domain: ThermalDomainId,
    },
    UnknownThermalNode {
        node: ThermalNodeId,
    },
    UnknownThermalDomain {
        domain: ThermalDomainId,
    },
    ThermalSelfConnection {
        node: ThermalNodeId,
    },
    NoThermalDomains,
    SingularThermalNetwork,
    TimeWentBack {
        tick: Tick,
        last_tick: Tick,
    },
    ThermalDomainMismatch {
        expected: ThermalDomainId,
        actual: ThermalDomainId,
    },
    ThermalNodeInitializationMismatch {
        node: ThermalNodeId,
    },
    MissingThermalNodeInitialization {
        node: ThermalNodeId,
    },
}

impl std::fmt::Display for ThermalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTemperature => write!(formatter, "thermal temperature is not valid"),
            Self::InvalidThermalResistance => {
                write!(formatter, "thermal resistance must be finite and positive")
            }
            Self::InvalidThermalCapacitance => {
                write!(formatter, "thermal capacitance must be finite and positive")
            }
            Self::InvalidThermalStep => {
                write!(formatter, "thermal step must be finite and positive")
            }
            Self::InvalidPowerInput => write!(formatter, "thermal power input is not valid"),
            Self::DuplicateThermalNode { node } => {
                write!(formatter, "duplicate thermal node {}", node.get())
            }
            Self::DuplicateThermalDomain { domain } => {
                write!(formatter, "duplicate thermal domain {}", domain.get())
            }
            Self::DuplicatePowerInput { domain } => {
                write!(
                    formatter,
                    "duplicate thermal power input for domain {}",
                    domain.get()
                )
            }
            Self::UnknownThermalNode { node } => {
                write!(formatter, "unknown thermal node {}", node.get())
            }
            Self::UnknownThermalDomain { domain } => {
                write!(formatter, "unknown thermal domain {}", domain.get())
            }
            Self::ThermalSelfConnection { node } => {
                write!(
                    formatter,
                    "thermal node {} cannot connect to itself",
                    node.get()
                )
            }
            Self::NoThermalDomains => write!(formatter, "thermal network has no domains"),
            Self::SingularThermalNetwork => {
                write!(formatter, "thermal network linear system is singular")
            }
            Self::TimeWentBack { tick, last_tick } => write!(
                formatter,
                "thermal update tick {tick} is before last tick {last_tick}"
            ),
            Self::ThermalDomainMismatch { expected, actual } => write!(
                formatter,
                "thermal snapshot domain {} does not match {}",
                actual.get(),
                expected.get()
            ),
            Self::ThermalNodeInitializationMismatch { node } => write!(
                formatter,
                "thermal node {} initialization does not match restored node",
                node.get()
            ),
            Self::MissingThermalNodeInitialization { node } => write!(
                formatter,
                "thermal node {} is missing initialization evidence",
                node.get()
            ),
        }
    }
}

impl std::error::Error for ThermalError {}

const ABSOLUTE_ZERO_C: f64 = -273.15;

fn validate_temperature(value: f64) -> Result<(), ThermalError> {
    if !value.is_finite() || value <= ABSOLUTE_ZERO_C {
        return Err(ThermalError::InvalidTemperature);
    }
    Ok(())
}

fn validate_node_initialization(
    initial: &ThermalNetworkNodeInitialization,
    nodes: &BTreeMap<ThermalNodeId, ThermalNetworkNode>,
    domains: &BTreeMap<ThermalDomainId, ThermalNodeId>,
) -> Result<(), ThermalError> {
    validate_temperature(initial.initial_temperature_c())?;
    if let Some(capacitance_j_per_c) = initial.capacitance_j_per_c() {
        validate_positive(capacitance_j_per_c, ThermalError::InvalidThermalCapacitance)?;
    }
    match nodes.get(&initial.node()) {
        Some(ThermalNetworkNode::Domain {
            domain,
            capacitance_j_per_c,
            ..
        }) if initial.kind() == ThermalNetworkNodeKind::Domain
            && initial.domain() == Some(*domain)
            && initial.capacitance_j_per_c() == Some(*capacitance_j_per_c)
            && domains.get(domain).copied() == Some(initial.node()) =>
        {
            Ok(())
        }
        Some(ThermalNetworkNode::Junction {
            capacitance_j_per_c,
            ..
        }) if initial.kind() == ThermalNetworkNodeKind::Junction
            && initial.domain().is_none()
            && initial.capacitance_j_per_c() == Some(*capacitance_j_per_c) =>
        {
            Ok(())
        }
        Some(ThermalNetworkNode::Reference { .. })
            if initial.kind() == ThermalNetworkNodeKind::Reference
                && initial.domain().is_none()
                && initial.capacitance_j_per_c().is_none() =>
        {
            Ok(())
        }
        Some(_) => Err(ThermalError::ThermalNodeInitializationMismatch {
            node: initial.node(),
        }),
        None => Err(ThermalError::UnknownThermalNode {
            node: initial.node(),
        }),
    }
}

fn derive_initial_nodes(
    nodes: &BTreeMap<ThermalNodeId, ThermalNetworkNode>,
) -> BTreeMap<ThermalNodeId, ThermalNetworkNodeInitialization> {
    nodes
        .iter()
        .map(|(node, record)| {
            let initial = match record {
                ThermalNetworkNode::Domain {
                    domain,
                    temperature_c,
                    capacitance_j_per_c,
                } => ThermalNetworkNodeInitialization::new(
                    *node,
                    ThermalNetworkNodeKind::Domain,
                    Some(*domain),
                    *temperature_c,
                    Some(*capacitance_j_per_c),
                ),
                ThermalNetworkNode::Junction {
                    temperature_c,
                    capacitance_j_per_c,
                } => ThermalNetworkNodeInitialization::new(
                    *node,
                    ThermalNetworkNodeKind::Junction,
                    None,
                    *temperature_c,
                    Some(*capacitance_j_per_c),
                ),
                ThermalNetworkNode::Reference { temperature_c } => {
                    ThermalNetworkNodeInitialization::new(
                        *node,
                        ThermalNetworkNodeKind::Reference,
                        None,
                        *temperature_c,
                        None,
                    )
                }
            };
            (*node, initial)
        })
        .collect()
}

fn validate_positive(value: f64, error: ThermalError) -> Result<(), ThermalError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(error);
    }
    Ok(())
}

fn validate_nonnegative(value: f64, error: ThermalError) -> Result<(), ThermalError> {
    if !value.is_finite() || value < 0.0 {
        return Err(error);
    }
    Ok(())
}

fn solve_linear_system(
    mut matrix: Vec<Vec<f64>>,
    mut rhs: Vec<f64>,
) -> Result<Vec<f64>, ThermalError> {
    let n = rhs.len();
    for row in &matrix {
        if row.len() != n {
            return Err(ThermalError::SingularThermalNetwork);
        }
    }
    for pivot in 0..n {
        let mut best = pivot;
        let mut best_abs = matrix[pivot][pivot].abs();
        for (row, values) in matrix.iter().enumerate().skip(pivot + 1) {
            let value = values[pivot].abs();
            if value > best_abs {
                best = row;
                best_abs = value;
            }
        }
        if best_abs <= f64::EPSILON {
            return Err(ThermalError::SingularThermalNetwork);
        }
        if best != pivot {
            matrix.swap(best, pivot);
            rhs.swap(best, pivot);
        }

        let pivot_value = matrix[pivot][pivot];
        for value in matrix[pivot].iter_mut().skip(pivot) {
            *value /= pivot_value;
        }
        rhs[pivot] /= pivot_value;

        let pivot_tail = matrix[pivot][pivot..].to_vec();
        let pivot_rhs = rhs[pivot];
        for (row_index, row_values) in matrix.iter_mut().enumerate() {
            if row_index == pivot {
                continue;
            }
            let factor = row_values[pivot];
            if factor == 0.0 {
                continue;
            }
            for (value, pivot_value) in row_values.iter_mut().skip(pivot).zip(pivot_tail.iter()) {
                *value -= factor * *pivot_value;
            }
            rhs[row_index] -= factor * pivot_rhs;
        }
    }
    if rhs.iter().any(|value| !value.is_finite()) {
        return Err(ThermalError::SingularThermalNetwork);
    }
    Ok(rhs)
}
