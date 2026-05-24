use std::collections::BTreeMap;

use crate::{PowerError, PowerEstimate, PowerModelMode, PowerResidency, PowerStateKind};
use rem6_stats::{StatId, StatSnapshot};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PowerMetricId(u64);

impl PowerMetricId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerMetricBinding {
    metric: PowerMetricId,
    stat: StatId,
}

impl PowerMetricBinding {
    pub const fn new(metric: PowerMetricId, stat: StatId) -> Self {
        Self { metric, stat }
    }

    pub const fn metric(&self) -> PowerMetricId {
        self.metric
    }

    pub const fn stat(&self) -> StatId {
        self.stat
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerMetricBindings {
    by_metric: BTreeMap<PowerMetricId, StatId>,
    by_stat: BTreeMap<StatId, PowerMetricId>,
}

impl PowerMetricBindings {
    pub fn new(bindings: Vec<PowerMetricBinding>) -> Result<Self, PowerError> {
        let mut by_metric = BTreeMap::new();
        let mut by_stat = BTreeMap::new();
        for binding in bindings {
            if by_metric.insert(binding.metric(), binding.stat()).is_some() {
                return Err(PowerError::DuplicatePowerMetricBinding {
                    metric: binding.metric(),
                });
            }
            if by_stat.insert(binding.stat(), binding.metric()).is_some() {
                return Err(PowerError::DuplicateBoundStat {
                    stat: binding.stat(),
                });
            }
        }
        Ok(Self { by_metric, by_stat })
    }

    pub fn stat_for(&self, metric: PowerMetricId) -> Option<StatId> {
        self.by_metric.get(&metric).copied()
    }

    pub fn metric_for(&self, stat: StatId) -> Option<PowerMetricId> {
        self.by_stat.get(&stat).copied()
    }

    pub fn entries(&self) -> &BTreeMap<PowerMetricId, StatId> {
        &self.by_metric
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerExpressionInputs {
    temperature_c: f64,
    voltage_v: f64,
    clock_period_ticks: f64,
    metrics: BTreeMap<PowerMetricId, f64>,
}

impl PowerExpressionInputs {
    pub fn new(
        temperature_c: f64,
        voltage_v: f64,
        clock_period_ticks: f64,
    ) -> Result<Self, PowerError> {
        validate_expression_input(temperature_c)?;
        validate_expression_input(voltage_v)?;
        if clock_period_ticks <= 0.0 || !clock_period_ticks.is_finite() {
            return Err(PowerError::InvalidClockPeriod);
        }
        Ok(Self {
            temperature_c,
            voltage_v,
            clock_period_ticks,
            metrics: BTreeMap::new(),
        })
    }

    pub fn with_metric(mut self, metric: PowerMetricId, value: f64) -> Result<Self, PowerError> {
        validate_expression_input(value)?;
        self.metrics.insert(metric, value);
        Ok(self)
    }

    pub fn from_stat_snapshot(
        temperature_c: f64,
        voltage_v: f64,
        clock_period_ticks: f64,
        snapshot: &StatSnapshot,
        bindings: &PowerMetricBindings,
    ) -> Result<Self, PowerError> {
        let stat_values = snapshot
            .samples()
            .iter()
            .map(|sample| (sample.id(), sample.value() as f64))
            .collect::<BTreeMap<_, _>>();
        let mut inputs = Self::new(temperature_c, voltage_v, clock_period_ticks)?;
        for (metric, stat) in bindings.entries() {
            let Some(value) = stat_values.get(stat) else {
                return Err(PowerError::MissingBoundStat { stat: *stat });
            };
            inputs = inputs.with_metric(*metric, *value)?;
        }
        Ok(inputs)
    }

    pub const fn temperature_c(&self) -> f64 {
        self.temperature_c
    }

    pub const fn voltage_v(&self) -> f64 {
        self.voltage_v
    }

    pub const fn clock_period_ticks(&self) -> f64 {
        self.clock_period_ticks
    }

    pub fn metric(&self, metric: PowerMetricId) -> Result<f64, PowerError> {
        self.metrics
            .get(&metric)
            .copied()
            .ok_or(PowerError::MissingPowerMetric { metric })
    }

    pub fn metrics(&self) -> &BTreeMap<PowerMetricId, f64> {
        &self.metrics
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PowerExpression {
    Constant(f64),
    Metric(PowerMetricId),
    TemperatureC,
    VoltageV,
    ClockPeriodTicks,
    Add(Box<PowerExpression>, Box<PowerExpression>),
    Subtract(Box<PowerExpression>, Box<PowerExpression>),
    Multiply(Box<PowerExpression>, Box<PowerExpression>),
    Divide(Box<PowerExpression>, Box<PowerExpression>),
}

impl PowerExpression {
    pub fn constant(value: f64) -> Result<Self, PowerError> {
        validate_expression_input(value)?;
        Ok(Self::Constant(value))
    }

    pub const fn metric(metric: PowerMetricId) -> Self {
        Self::Metric(metric)
    }

    pub const fn temperature_c() -> Self {
        Self::TemperatureC
    }

    pub const fn voltage_v() -> Self {
        Self::VoltageV
    }

    pub const fn clock_period_ticks() -> Self {
        Self::ClockPeriodTicks
    }

    pub fn add(left: Self, right: Self) -> Self {
        Self::Add(Box::new(left), Box::new(right))
    }

    pub fn subtract(left: Self, right: Self) -> Self {
        Self::Subtract(Box::new(left), Box::new(right))
    }

    pub fn multiply(left: Self, right: Self) -> Self {
        Self::Multiply(Box::new(left), Box::new(right))
    }

    pub fn divide(left: Self, right: Self) -> Self {
        Self::Divide(Box::new(left), Box::new(right))
    }

    pub fn evaluate(&self, inputs: &PowerExpressionInputs) -> Result<f64, PowerError> {
        let value = match self {
            Self::Constant(value) => *value,
            Self::Metric(metric) => inputs.metric(*metric)?,
            Self::TemperatureC => inputs.temperature_c(),
            Self::VoltageV => inputs.voltage_v(),
            Self::ClockPeriodTicks => inputs.clock_period_ticks(),
            Self::Add(left, right) => left.evaluate(inputs)? + right.evaluate(inputs)?,
            Self::Subtract(left, right) => left.evaluate(inputs)? - right.evaluate(inputs)?,
            Self::Multiply(left, right) => left.evaluate(inputs)? * right.evaluate(inputs)?,
            Self::Divide(left, right) => left.evaluate(inputs)? / right.evaluate(inputs)?,
        };
        validate_expression_result(value)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerStateExpression {
    state: PowerStateKind,
    dynamic_expression: PowerExpression,
    static_expression: PowerExpression,
}

impl PowerStateExpression {
    pub fn new(
        state: PowerStateKind,
        dynamic_expression: PowerExpression,
        static_expression: PowerExpression,
    ) -> Result<Self, PowerError> {
        if state == PowerStateKind::Undefined {
            return Err(PowerError::UndefinedState);
        }
        Ok(Self {
            state,
            dynamic_expression,
            static_expression,
        })
    }

    pub const fn state(&self) -> PowerStateKind {
        self.state
    }

    pub const fn dynamic_expression(&self) -> &PowerExpression {
        &self.dynamic_expression
    }

    pub const fn static_expression(&self) -> &PowerExpression {
        &self.static_expression
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerExpressionModelSnapshot {
    mode: PowerModelMode,
    inputs: PowerExpressionInputs,
    states: Vec<PowerStateExpression>,
}

impl PowerExpressionModelSnapshot {
    pub const fn new(
        mode: PowerModelMode,
        inputs: PowerExpressionInputs,
        states: Vec<PowerStateExpression>,
    ) -> Self {
        Self {
            mode,
            inputs,
            states,
        }
    }

    pub const fn mode(&self) -> PowerModelMode {
        self.mode
    }

    pub const fn inputs(&self) -> &PowerExpressionInputs {
        &self.inputs
    }

    pub fn states(&self) -> &[PowerStateExpression] {
        &self.states
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerExpressionModel {
    mode: PowerModelMode,
    inputs: PowerExpressionInputs,
    states: BTreeMap<PowerStateKind, PowerStateExpression>,
}

impl PowerExpressionModel {
    pub fn new(
        mode: PowerModelMode,
        inputs: PowerExpressionInputs,
        states: Vec<PowerStateExpression>,
    ) -> Result<Self, PowerError> {
        let states = state_expression_map(states)?;
        Ok(Self {
            mode,
            inputs,
            states,
        })
    }

    pub const fn mode(&self) -> PowerModelMode {
        self.mode
    }

    pub const fn inputs(&self) -> &PowerExpressionInputs {
        &self.inputs
    }

    pub fn update_inputs(&mut self, inputs: PowerExpressionInputs) {
        self.inputs = inputs;
    }

    pub fn estimate(&self, residency: &PowerResidency) -> Result<PowerEstimate, PowerError> {
        let total_ticks = residency.total_ticks();
        if total_ticks == 0 {
            return Err(PowerError::NoPowerResidency);
        }

        let mut dynamic_watts = 0.0;
        let mut static_watts = 0.0;
        for (state, ticks) in residency.entries() {
            if *ticks == 0 {
                continue;
            }
            let Some(state_model) = self.states.get(state) else {
                return Err(PowerError::MissingPowerStateExpressionModel { state: *state });
            };
            let weight = *ticks as f64 / total_ticks as f64;
            if self.mode != PowerModelMode::StaticOnly {
                let value = state_model.dynamic_expression().evaluate(&self.inputs)?;
                validate_power_output(value)?;
                dynamic_watts += value * weight;
            }
            if self.mode != PowerModelMode::DynamicOnly {
                let value = state_model.static_expression().evaluate(&self.inputs)?;
                validate_power_output(value)?;
                static_watts += value * weight;
            }
        }

        Ok(PowerEstimate::new(dynamic_watts, static_watts))
    }

    pub fn snapshot(&self) -> PowerExpressionModelSnapshot {
        PowerExpressionModelSnapshot::new(
            self.mode,
            self.inputs.clone(),
            self.states.values().cloned().collect(),
        )
    }

    pub fn restore(&mut self, snapshot: &PowerExpressionModelSnapshot) -> Result<(), PowerError> {
        if snapshot.mode() != self.mode {
            return Err(PowerError::PowerModelModeMismatch {
                expected: self.mode,
                actual: snapshot.mode(),
            });
        }
        self.inputs = snapshot.inputs().clone();
        self.states = state_expression_map(snapshot.states().to_vec())?;
        Ok(())
    }
}

fn state_expression_map(
    states: Vec<PowerStateExpression>,
) -> Result<BTreeMap<PowerStateKind, PowerStateExpression>, PowerError> {
    let mut map = BTreeMap::new();
    for state in states {
        let state_kind = state.state();
        if map.insert(state_kind, state).is_some() {
            return Err(PowerError::DuplicatePowerStateExpressionModel { state: state_kind });
        }
    }
    Ok(map)
}

fn validate_expression_input(value: f64) -> Result<(), PowerError> {
    if !value.is_finite() {
        return Err(PowerError::InvalidPowerExpressionInput);
    }
    Ok(())
}

fn validate_expression_result(value: f64) -> Result<(), PowerError> {
    if !value.is_finite() {
        return Err(PowerError::InvalidPowerExpressionResult);
    }
    Ok(())
}

fn validate_power_output(value: f64) -> Result<(), PowerError> {
    if !value.is_finite() || value < 0.0 {
        return Err(PowerError::InvalidPowerExpressionResult);
    }
    Ok(())
}
