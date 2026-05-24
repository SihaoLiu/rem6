use rem6_kernel::Tick;

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

#[derive(Clone, Debug, PartialEq)]
pub enum ThermalError {
    InvalidTemperature,
    InvalidThermalResistance,
    InvalidThermalCapacitance,
    InvalidThermalStep,
    InvalidPowerInput,
    TimeWentBack {
        tick: Tick,
        last_tick: Tick,
    },
    ThermalDomainMismatch {
        expected: ThermalDomainId,
        actual: ThermalDomainId,
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
        }
    }
}

impl std::error::Error for ThermalError {}

fn validate_temperature(value: f64) -> Result<(), ThermalError> {
    if !value.is_finite() {
        return Err(ThermalError::InvalidTemperature);
    }
    Ok(())
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
