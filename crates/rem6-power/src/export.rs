use std::collections::BTreeMap;
use std::fmt::Write as _;

use rem6_kernel::Tick;

use crate::{
    validate_power_value, validate_temperature, PowerDomain, PowerError, PowerEstimate, PowerModel,
    PowerResidency, PowerStateKind,
};

mod dsent_csv;
mod mcpat_xml;

const POWER_ANALYSIS_SERIALIZED_DECIMAL_EPSILON: f64 = 0.000_001;
const EXTERNAL_REPORT_DEFAULT_TEMPERATURE_C: f64 = 0.0;

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

    pub fn from_mcpat_compatible_xml(input: &str) -> Result<Self, PowerError> {
        mcpat_xml::parse(input)
    }

    pub fn from_mcpat_report_text(input: &str, tick: Tick) -> Result<Self, PowerError> {
        if tick == 0 {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::McPat,
                "McPAT report tick must be greater than zero",
            ));
        }

        let mut blocks = Vec::new();
        let mut ancestry: Vec<McpatReportAncestor> = Vec::new();
        let mut current = None;
        for line in input.lines() {
            if let Some((indent, name)) = mcpat_report_component_header(line) {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
                while ancestry
                    .last()
                    .map_or(false, |ancestor| ancestor.indent >= indent)
                {
                    ancestry.pop();
                }
                let target = ancestry
                    .last()
                    .map(|ancestor| format!("{}.{}", ancestor.target, name))
                    .unwrap_or_else(|| name.clone());
                current = Some(McpatReportBlock::new(indent, target.clone()));
                ancestry.push(McpatReportAncestor { indent, target });
            } else if let Some((label, value, unit)) = mcpat_report_metric(line) {
                if let Some(block) = current.as_mut() {
                    block.set_metric(label, value, unit)?;
                }
            }
        }
        if let Some(block) = current {
            blocks.push(block);
        }
        if blocks.is_empty() {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::McPat,
                "no McPAT report components found",
            ));
        }

        let records = blocks
            .iter()
            .enumerate()
            .filter(|(index, block)| {
                blocks
                    .get(index + 1)
                    .map_or(true, |next| next.indent <= block.indent)
            })
            .map(|(_, block)| block.to_record(tick))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(ExternalPowerAnalysisKind::McPat, tick, records)
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

    pub fn from_dsent_compatible_csv(input: &str) -> Result<Self, PowerError> {
        dsent_csv::parse(input)
    }

    pub fn from_dsent_report_text(input: &str, tick: Tick) -> Result<Self, PowerError> {
        if tick == 0 {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                "DSENT report tick must be greater than zero",
            ));
        }
        let records = input
            .lines()
            .filter_map(dsent_report_power_line)
            .map(|(target, tuple)| dsent_report_record(&target, tuple, tick))
            .collect::<Result<Vec<_>, _>>()?;
        if records.is_empty() {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                "no DSENT report power records found",
            ));
        }
        Self::new(ExternalPowerAnalysisKind::Dsent, tick, records)
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

fn parse_power_state_kind(value: &str) -> Result<PowerStateKind, ()> {
    match value {
        "Undefined" => Ok(PowerStateKind::Undefined),
        "On" => Ok(PowerStateKind::On),
        "ClockGated" => Ok(PowerStateKind::ClockGated),
        "SramRetention" => Ok(PowerStateKind::SramRetention),
        "Off" => Ok(PowerStateKind::Off),
        _ => Err(()),
    }
}

#[derive(Clone, Debug)]
struct McpatReportAncestor {
    indent: usize,
    target: String,
}

#[derive(Clone, Debug)]
struct McpatReportBlock {
    indent: usize,
    target: String,
    runtime_dynamic_watts: Option<f64>,
    subthreshold_leakage_watts: Option<f64>,
    gate_leakage_watts: Option<f64>,
}

impl McpatReportBlock {
    fn new(indent: usize, target: String) -> Self {
        Self {
            indent,
            target,
            runtime_dynamic_watts: None,
            subthreshold_leakage_watts: None,
            gate_leakage_watts: None,
        }
    }

    fn set_metric(&mut self, label: &str, value: &str, unit: &str) -> Result<(), PowerError> {
        match label {
            "Runtime Dynamic Power" => {
                set_mcpat_report_power_metric(
                    &self.target,
                    label,
                    &mut self.runtime_dynamic_watts,
                    value,
                    unit,
                )?;
            }
            "Subthreshold Leakage Power" => {
                set_mcpat_report_power_metric(
                    &self.target,
                    label,
                    &mut self.subthreshold_leakage_watts,
                    value,
                    unit,
                )?;
            }
            "Gate Leakage Power" => {
                set_mcpat_report_power_metric(
                    &self.target,
                    label,
                    &mut self.gate_leakage_watts,
                    value,
                    unit,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn to_record(&self, tick: Tick) -> Result<PowerAnalysisRecord, PowerError> {
        let dynamic_watts = required_mcpat_report_power_metric(
            &self.target,
            "Runtime Dynamic Power",
            self.runtime_dynamic_watts,
        )?;
        let subthreshold_leakage_watts = required_mcpat_report_power_metric(
            &self.target,
            "Subthreshold Leakage Power",
            self.subthreshold_leakage_watts,
        )?;
        let gate_leakage_watts = required_mcpat_report_power_metric(
            &self.target,
            "Gate Leakage Power",
            self.gate_leakage_watts,
        )?;
        PowerAnalysisRecord::new(
            self.target.clone(),
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, tick)]),
            EXTERNAL_REPORT_DEFAULT_TEMPERATURE_C,
            PowerEstimate::new(
                dynamic_watts,
                subthreshold_leakage_watts + gate_leakage_watts,
            ),
        )
    }
}

fn mcpat_report_component_header(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim();
    if !trimmed.ends_with(':') || trimmed.contains('=') {
        return None;
    }
    let name = trimmed.strip_suffix(':')?.trim();
    if name.is_empty() {
        return None;
    }
    let indent = line
        .chars()
        .take_while(|character| character.is_whitespace())
        .count();
    Some((indent, name.to_string()))
}

fn mcpat_report_metric(line: &str) -> Option<(&str, &str, &str)> {
    let (label, value) = line.trim().split_once('=')?;
    let mut fields = value.split_whitespace();
    let value = fields.next()?;
    let unit = fields.next().unwrap_or_default();
    Some((label.trim(), value, unit))
}

fn set_mcpat_report_power_metric(
    target: &str,
    label: &str,
    slot: &mut Option<f64>,
    value: &str,
    unit: &str,
) -> Result<(), PowerError> {
    if slot.is_some() {
        return Err(power_analysis_artifact_error(
            ExternalPowerAnalysisKind::McPat,
            format!("component {target} repeats {label}"),
        ));
    }
    if unit != "W" {
        return Err(power_analysis_artifact_error(
            ExternalPowerAnalysisKind::McPat,
            format!("component {target} {label} must use W"),
        ));
    }
    let value = value.parse::<f64>().map_err(|_| {
        power_analysis_artifact_error(
            ExternalPowerAnalysisKind::McPat,
            format!("component {target} {label} is not a valid number"),
        )
    })?;
    validate_power_value(value)?;
    *slot = Some(value);
    Ok(())
}

fn required_mcpat_report_power_metric(
    target: &str,
    label: &str,
    value: Option<f64>,
) -> Result<f64, PowerError> {
    value.ok_or_else(|| {
        power_analysis_artifact_error(
            ExternalPowerAnalysisKind::McPat,
            format!("component {target} is missing {label}"),
        )
    })
}

fn dsent_report_power_line(line: &str) -> Option<(String, &str)> {
    let (target, tuple) = line.split_once(" Power:")?;
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    Some((target.to_string(), tuple.trim()))
}

fn dsent_report_record(
    target: &str,
    tuple: &str,
    tick: Tick,
) -> Result<PowerAnalysisRecord, PowerError> {
    let values = dsent_report_tuple_values(target, tuple)?;
    let dynamic_watts = dsent_report_power_sum(
        target,
        &values,
        Some("total_dynamic"),
        dsent_report_is_dynamic_power_key,
        "dynamic",
    )?;
    let static_watts = dsent_report_static_power_sum(target, &values)?;
    PowerAnalysisRecord::new(
        target.to_string(),
        PowerStateKind::On,
        PowerResidency::new(vec![(PowerStateKind::On, tick)]),
        EXTERNAL_REPORT_DEFAULT_TEMPERATURE_C,
        PowerEstimate::new(dynamic_watts, static_watts),
    )
}

fn dsent_report_tuple_values(
    target: &str,
    tuple: &str,
) -> Result<BTreeMap<String, f64>, PowerError> {
    let tuple = tuple.trim();
    let tuple = tuple
        .strip_prefix('(')
        .and_then(|tuple| tuple.strip_suffix(')'))
        .ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} tuple is not closed"),
            )
        })?;
    let mut values = BTreeMap::new();
    let mut remaining = tuple.trim();
    while !remaining.is_empty() {
        let entry = remaining.strip_prefix('(').ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} tuple entry is missing '('"),
            )
        })?;
        let quote = entry
            .chars()
            .next()
            .filter(|quote| *quote == '\'' || *quote == '"')
            .ok_or_else(|| {
                power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::Dsent,
                    format!("DSENT report target {target} tuple key is not quoted"),
                )
            })?;
        let after_key_start = &entry[quote.len_utf8()..];
        let key_end = after_key_start.find(quote).ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} has unterminated tuple key"),
            )
        })?;
        let key = &after_key_start[..key_end];
        let after_key = after_key_start[key_end + quote.len_utf8()..].trim_start();
        let after_comma = after_key.strip_prefix(',').ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} tuple entry is missing comma"),
            )
        })?;
        let value_text = after_comma.trim_start();
        let value_end = value_text
            .find(|character: char| {
                character == ')' || character == ',' || character.is_whitespace()
            })
            .unwrap_or(value_text.len());
        let value = value_text[..value_end].parse::<f64>().map_err(|_| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} key {key} is not a valid number"),
            )
        })?;
        validate_power_value(value)?;
        if values.insert(key.to_string(), value).is_some() {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} repeats key {key}"),
            ));
        }
        let after_value = value_text[value_end..].trim_start();
        let after_entry = after_value.strip_prefix(')').ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("DSENT report target {target} tuple entry is missing closing ')'"),
            )
        })?;
        let after_entry = after_entry.trim_start();
        remaining = if after_entry.is_empty() {
            after_entry
        } else {
            after_entry
                .strip_prefix(',')
                .map(str::trim_start)
                .ok_or_else(|| {
                    power_analysis_artifact_error(
                        ExternalPowerAnalysisKind::Dsent,
                        format!(
                            "DSENT report target {target} tuple entries are not comma-separated"
                        ),
                    )
                })?
        };
    }
    if values.is_empty() {
        return Err(power_analysis_artifact_error(
            ExternalPowerAnalysisKind::Dsent,
            format!("DSENT report target {target} has no tuple entries"),
        ));
    }
    Ok(values)
}

fn dsent_report_power_sum(
    target: &str,
    values: &BTreeMap<String, f64>,
    preferred_key: Option<&str>,
    include: impl Fn(&str) -> bool,
    field: &str,
) -> Result<f64, PowerError> {
    if let Some(preferred_key) = preferred_key {
        if let Some(value) = values.get(preferred_key) {
            return Ok(*value);
        }
    }
    let mut found = false;
    let mut sum = 0.0;
    for (name, value) in values {
        if include(name.as_str()) {
            found = true;
            sum += *value;
        }
    }
    if !found {
        return Err(power_analysis_artifact_error(
            ExternalPowerAnalysisKind::Dsent,
            format!("DSENT report target {target} is missing {field} power"),
        ));
    }
    validate_power_value(sum)?;
    Ok(sum)
}

fn dsent_report_static_power_sum(
    target: &str,
    values: &BTreeMap<String, f64>,
) -> Result<f64, PowerError> {
    if let Some(value) = values.get("total_leakage") {
        return Ok(*value);
    }
    if let Some(value) = values.get("total_static") {
        return Ok(*value);
    }
    dsent_report_power_sum(
        target,
        values,
        None,
        dsent_report_is_static_power_key,
        "static",
    )
}

fn dsent_report_is_dynamic_power_key(name: &str) -> bool {
    let trimmed = name.trim();
    dsent_report_label_name(trimmed) == "dynamic power"
        || trimmed == "dynamic"
        || trimmed.ends_with("_dynamic")
}

fn dsent_report_is_static_power_key(name: &str) -> bool {
    let trimmed = name.trim();
    matches!(
        dsent_report_label_name(trimmed).as_str(),
        "leakage power" | "static power"
    ) || trimmed == "leakage"
        || trimmed == "static"
        || trimmed.ends_with("_leakage")
        || trimmed.ends_with("_static")
}

fn dsent_report_label_name(name: &str) -> String {
    name.trim()
        .trim_end_matches(':')
        .trim()
        .to_ascii_lowercase()
}

fn validate_component_total(
    kind: ExternalPowerAnalysisKind,
    dynamic_watts: f64,
    static_watts: f64,
    total_watts: f64,
    context: &str,
) -> Result<(), PowerError> {
    validate_power_value(total_watts)?;
    if (dynamic_watts + static_watts - total_watts).abs()
        > POWER_ANALYSIS_SERIALIZED_DECIMAL_EPSILON
    {
        return Err(power_analysis_artifact_error(
            kind,
            format!("{context} total watts does not match dynamic plus static watts"),
        ));
    }
    Ok(())
}

fn validate_imported_total(
    kind: ExternalPowerAnalysisKind,
    actual: f64,
    imported: f64,
    record_count: usize,
    field: &str,
) -> Result<(), PowerError> {
    validate_power_value(imported)?;
    let tolerance = POWER_ANALYSIS_SERIALIZED_DECIMAL_EPSILON * (record_count as f64 + 1.0);
    if (actual - imported).abs() > tolerance {
        return Err(power_analysis_artifact_error(
            kind,
            format!("total {field} does not match component records"),
        ));
    }
    Ok(())
}

fn validate_residency_tick_sum(
    kind: ExternalPowerAnalysisKind,
    entries: &[(PowerStateKind, Tick)],
    context: &str,
) -> Result<(), PowerError> {
    entries
        .iter()
        .try_fold(0_u64, |total, (_, ticks)| total.checked_add(*ticks))
        .ok_or_else(|| {
            power_analysis_artifact_error(kind, format!("{context} residency ticks overflow"))
        })?;
    Ok(())
}

fn power_analysis_artifact_error(
    kind: ExternalPowerAnalysisKind,
    message: impl Into<String>,
) -> PowerError {
    PowerError::InvalidPowerAnalysisArtifact {
        kind,
        message: message.into(),
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
