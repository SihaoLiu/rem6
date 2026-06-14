use std::fmt::Write as _;

use rem6_kernel::Tick;

use crate::{
    validate_power_value, validate_temperature, PowerDomain, PowerError, PowerEstimate, PowerModel,
    PowerResidency, PowerStateKind,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExternalPowerAnalysisKind {
    McPat,
    Dsent,
    DramPower,
    Generic,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerAnalysisRecord {
    target: String,
    current_state: PowerStateKind,
    residency_ticks: Vec<(PowerStateKind, Tick)>,
    temperature_c: f64,
    estimate: PowerEstimate,
}

impl PowerAnalysisRecord {
    pub fn new(
        target: impl Into<String>,
        current_state: PowerStateKind,
        residency: PowerResidency,
        temperature_c: f64,
        estimate: PowerEstimate,
    ) -> Result<Self, PowerError> {
        let target = target.into();
        if target.is_empty() {
            return Err(PowerError::EmptyName);
        }
        if current_state == PowerStateKind::Undefined {
            return Err(PowerError::UndefinedState);
        }
        if residency.total_ticks() == 0 {
            return Err(PowerError::NoPowerResidency);
        }
        for state in residency.entries().keys() {
            if *state == PowerStateKind::Undefined {
                return Err(PowerError::PowerAnalysisUndefinedResidencyState {
                    target: target.clone(),
                });
            }
        }
        if residency.ticks(current_state) == 0 {
            return Err(PowerError::PowerAnalysisCurrentStateMissingResidency {
                target: target.clone(),
                state: current_state,
            });
        }
        validate_temperature(temperature_c)?;
        validate_power_value(estimate.dynamic_watts())?;
        validate_power_value(estimate.static_watts())?;
        Ok(Self {
            target,
            current_state,
            residency_ticks: residency.entries().iter().map(|(k, v)| (*k, *v)).collect(),
            temperature_c,
            estimate,
        })
    }

    pub fn from_domain_model(
        tick: Tick,
        domain: &PowerDomain,
        model: &PowerModel,
    ) -> Result<Self, PowerError> {
        let residency = domain.residency_at(tick)?;
        let estimate = model.estimate(&residency)?;
        Self::new(
            domain.config().name(),
            domain.current_state(),
            residency,
            model.current_temperature_c(),
            estimate,
        )
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn current_state(&self) -> PowerStateKind {
        self.current_state
    }

    pub fn residency_ticks(&self, state: PowerStateKind) -> Tick {
        self.residency_ticks
            .iter()
            .find_map(|(candidate, ticks)| (*candidate == state).then_some(*ticks))
            .unwrap_or_default()
    }

    pub fn residency_entries(&self) -> &[(PowerStateKind, Tick)] {
        &self.residency_ticks
    }

    pub const fn temperature_c(&self) -> f64 {
        self.temperature_c
    }

    pub const fn dynamic_watts(&self) -> f64 {
        self.estimate.dynamic_watts()
    }

    pub const fn static_watts(&self) -> f64 {
        self.estimate.static_watts()
    }

    pub const fn total_watts(&self) -> f64 {
        self.estimate.total_watts()
    }

    pub const fn estimate(&self) -> PowerEstimate {
        self.estimate
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerAnalysisExport {
    kind: ExternalPowerAnalysisKind,
    tick: Tick,
    records: Vec<PowerAnalysisRecord>,
    total_dynamic_watts: f64,
    total_static_watts: f64,
}

impl PowerAnalysisExport {
    pub fn new(
        kind: ExternalPowerAnalysisKind,
        tick: Tick,
        mut records: Vec<PowerAnalysisRecord>,
    ) -> Result<Self, PowerError> {
        records.sort_by(|left, right| left.target().cmp(right.target()));
        for window in records.windows(2) {
            if window[0].target() == window[1].target() {
                return Err(PowerError::DuplicatePowerAnalysisTarget {
                    target: window[0].target().to_string(),
                });
            }
        }
        let total_dynamic_watts = records.iter().map(PowerAnalysisRecord::dynamic_watts).sum();
        let total_static_watts = records.iter().map(PowerAnalysisRecord::static_watts).sum();
        validate_power_value(total_dynamic_watts)?;
        validate_power_value(total_static_watts)?;
        Ok(Self {
            kind,
            tick,
            records,
            total_dynamic_watts,
            total_static_watts,
        })
    }

    pub const fn kind(&self) -> ExternalPowerAnalysisKind {
        self.kind
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn records(&self) -> &[PowerAnalysisRecord] {
        &self.records
    }

    pub const fn total_dynamic_watts(&self) -> f64 {
        self.total_dynamic_watts
    }

    pub const fn total_static_watts(&self) -> f64 {
        self.total_static_watts
    }

    pub const fn total_watts(&self) -> f64 {
        self.total_dynamic_watts + self.total_static_watts
    }

    pub fn to_mcpat_compatible_xml(&self) -> Result<String, PowerError> {
        self.require_kind(ExternalPowerAnalysisKind::McPat)?;

        let mut output = String::new();
        writeln!(&mut output, "<mcpat_power tick=\"{}\">", self.tick)
            .expect("writing to a string cannot fail");
        for record in &self.records {
            write!(&mut output, "  <component id=\"").expect("writing to a string cannot fail");
            push_xml_attribute_value(&mut output, record.target());
            write!(&mut output, "\" name=\"").expect("writing to a string cannot fail");
            push_xml_attribute_value(&mut output, record.target());
            writeln!(&mut output, "\" state=\"{:?}\">", record.current_state(),)
                .expect("writing to a string cannot fail");
            writeln!(
                &mut output,
                "    <power dynamic_watts=\"{:.6}\" leakage_watts=\"{:.6}\" total_watts=\"{:.6}\"/>",
                record.dynamic_watts(),
                record.static_watts(),
                record.total_watts(),
            )
            .expect("writing to a string cannot fail");
            writeln!(
                &mut output,
                "    <thermal temperature_c=\"{:.6}\"/>",
                record.temperature_c(),
            )
            .expect("writing to a string cannot fail");
            for (state, ticks) in record.residency_entries() {
                writeln!(
                    &mut output,
                    "    <residency state=\"{state:?}\" ticks=\"{ticks}\" ratio=\"{:.6}\"/>",
                    residency_ratio(record, *ticks),
                )
                .expect("writing to a string cannot fail");
            }
            writeln!(&mut output, "  </component>").expect("writing to a string cannot fail");
        }
        writeln!(
            &mut output,
            "  <totals dynamic_watts=\"{:.6}\" leakage_watts=\"{:.6}\" total_watts=\"{:.6}\"/>",
            self.total_dynamic_watts(),
            self.total_static_watts(),
            self.total_watts(),
        )
        .expect("writing to a string cannot fail");
        writeln!(&mut output, "</mcpat_power>").expect("writing to a string cannot fail");

        Ok(output)
    }

    pub fn to_dsent_compatible_csv(&self) -> Result<String, PowerError> {
        self.require_kind(ExternalPowerAnalysisKind::Dsent)?;

        let mut output = String::new();
        writeln!(
            &mut output,
            "record_type,tick,target,state,temperature_c,dynamic_watts,static_watts,total_watts,residency_state,residency_ticks,residency_ratio",
        )
        .expect("writing to a string cannot fail");
        for record in &self.records {
            for (state, ticks) in record.residency_entries() {
                output.push_str("component,");
                write!(&mut output, "{},", self.tick).expect("writing to a string cannot fail");
                push_csv_field(&mut output, record.target());
                writeln!(
                    &mut output,
                    ",{:?},{:.6},{:.6},{:.6},{:.6},{state:?},{ticks},{:.6}",
                    record.current_state(),
                    record.temperature_c(),
                    record.dynamic_watts(),
                    record.static_watts(),
                    record.total_watts(),
                    residency_ratio(record, *ticks),
                )
                .expect("writing to a string cannot fail");
            }
        }
        writeln!(
            &mut output,
            "total,{},__total__,All,,{:.6},{:.6},{:.6},,{},1.000000",
            self.tick,
            self.total_dynamic_watts(),
            self.total_static_watts(),
            self.total_watts(),
            total_residency_ticks(&self.records),
        )
        .expect("writing to a string cannot fail");

        Ok(output)
    }

    pub fn to_power_analysis_smoke_xml(&self) -> String {
        let mut output = String::new();
        writeln!(
            &mut output,
            "<power_analysis_smoke kind=\"{:?}\" tick=\"{}\">",
            self.kind, self.tick,
        )
        .expect("writing to a string cannot fail");
        writeln!(
            &mut output,
            "  <totals dynamic_watts=\"{:.6}\" static_watts=\"{:.6}\" total_watts=\"{:.6}\"/>",
            self.total_dynamic_watts(),
            self.total_static_watts(),
            self.total_watts(),
        )
        .expect("writing to a string cannot fail");
        for record in &self.records {
            write!(&mut output, "  <component name=\"").expect("writing to a string cannot fail");
            push_xml_attribute_value(&mut output, record.target());
            writeln!(
                &mut output,
                "\" state=\"{:?}\" temperature_c=\"{:.6}\" dynamic_watts=\"{:.6}\" static_watts=\"{:.6}\" total_watts=\"{:.6}\">",
                record.current_state(),
                record.temperature_c(),
                record.dynamic_watts(),
                record.static_watts(),
                record.total_watts(),
            )
            .expect("writing to a string cannot fail");
            for (state, ticks) in record.residency_entries() {
                writeln!(
                    &mut output,
                    "    <residency state=\"{state:?}\" ticks=\"{ticks}\"/>",
                )
                .expect("writing to a string cannot fail");
            }
            writeln!(&mut output, "  </component>").expect("writing to a string cannot fail");
        }
        writeln!(&mut output, "</power_analysis_smoke>").expect("writing to a string cannot fail");

        output
    }

    fn require_kind(&self, expected: ExternalPowerAnalysisKind) -> Result<(), PowerError> {
        if self.kind != expected {
            return Err(PowerError::PowerAnalysisKindMismatch {
                expected,
                actual: self.kind,
            });
        }
        Ok(())
    }
}

fn push_xml_attribute_value(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

fn push_csv_field(output: &mut String, value: &str) {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        output.push('"');
        for character in value.chars() {
            if character == '"' {
                output.push('"');
            }
            output.push(character);
        }
        output.push('"');
    } else {
        output.push_str(value);
    }
}

fn residency_ratio(record: &PowerAnalysisRecord, ticks: Tick) -> f64 {
    ticks as f64 / residency_total_ticks(record) as f64
}

fn residency_total_ticks(record: &PowerAnalysisRecord) -> Tick {
    record
        .residency_entries()
        .iter()
        .map(|(_, ticks)| *ticks)
        .sum()
}

fn total_residency_ticks(records: &[PowerAnalysisRecord]) -> Tick {
    records.iter().map(residency_total_ticks).sum()
}
