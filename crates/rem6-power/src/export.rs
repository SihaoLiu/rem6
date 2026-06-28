use std::collections::BTreeMap;
use std::fmt::Write as _;

use rem6_kernel::Tick;

use crate::{
    validate_power_value, validate_temperature, PowerDomain, PowerError, PowerEstimate, PowerModel,
    PowerResidency, PowerStateKind,
};

const DSENT_CSV_HEADER: [&str; 11] = [
    "record_type",
    "tick",
    "target",
    "state",
    "temperature_c",
    "dynamic_watts",
    "static_watts",
    "total_watts",
    "residency_state",
    "residency_ticks",
    "residency_ratio",
];
const POWER_ANALYSIS_SERIALIZED_DECIMAL_EPSILON: f64 = 0.000_001;
const MCPAT_REPORT_DEFAULT_TEMPERATURE_C: f64 = 0.0;

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
        let root_tag = xml_first_tag(input, "mcpat_power", ExternalPowerAnalysisKind::McPat)?;
        let root_attributes = xml_attributes(root_tag, ExternalPowerAnalysisKind::McPat)?;
        let tick = parse_tick_attribute(
            &root_attributes,
            "tick",
            ExternalPowerAnalysisKind::McPat,
            "mcpat_power",
        )?;
        let mut records = Vec::new();
        let mut remaining = input;
        while let Some(component_start) = remaining.find("<component ") {
            let component = &remaining[component_start..];
            let open_end = component.find('>').ok_or_else(|| {
                power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::McPat,
                    "component tag is not terminated",
                )
            })?;
            let open_tag = &component[..=open_end];
            let component_attributes = xml_attributes(open_tag, ExternalPowerAnalysisKind::McPat)?;
            let component_body = &component[open_end + 1..];
            let close_start = component_body.find("</component>").ok_or_else(|| {
                power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::McPat,
                    "component tag is missing closing tag",
                )
            })?;
            let body = &component_body[..close_start];
            let target = required_attribute(
                &component_attributes,
                "id",
                ExternalPowerAnalysisKind::McPat,
                "component",
            )?;
            let current_state = parse_power_state_attribute(
                &component_attributes,
                "state",
                ExternalPowerAnalysisKind::McPat,
                "component",
            )?;
            let power_tag = xml_first_tag(body, "power", ExternalPowerAnalysisKind::McPat)?;
            let power_attributes = xml_attributes(power_tag, ExternalPowerAnalysisKind::McPat)?;
            let dynamic_watts = parse_f64_attribute(
                &power_attributes,
                "dynamic_watts",
                ExternalPowerAnalysisKind::McPat,
                "power",
            )?;
            let static_watts = parse_f64_attribute(
                &power_attributes,
                "leakage_watts",
                ExternalPowerAnalysisKind::McPat,
                "power",
            )?;
            let component_total_watts = parse_f64_attribute(
                &power_attributes,
                "total_watts",
                ExternalPowerAnalysisKind::McPat,
                "power",
            )?;
            validate_component_total(
                ExternalPowerAnalysisKind::McPat,
                dynamic_watts,
                static_watts,
                component_total_watts,
                "power",
            )?;
            let thermal_tag = xml_first_tag(body, "thermal", ExternalPowerAnalysisKind::McPat)?;
            let thermal_attributes = xml_attributes(thermal_tag, ExternalPowerAnalysisKind::McPat)?;
            let temperature_c = parse_f64_attribute(
                &thermal_attributes,
                "temperature_c",
                ExternalPowerAnalysisKind::McPat,
                "thermal",
            )?;
            let residency = mcpat_residency_entries(body)?;
            records.push(PowerAnalysisRecord::new(
                target.to_string(),
                current_state,
                PowerResidency::new(residency),
                temperature_c,
                PowerEstimate::new(dynamic_watts, static_watts),
            )?);
            remaining = &component_body[close_start + "</component>".len()..];
        }
        if records.is_empty() {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::McPat,
                "no component records found",
            ));
        }

        match xml_tag_count(input, "totals") {
            0 => {
                return Err(power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::McPat,
                    "missing totals tag",
                ));
            }
            1 => {}
            _ => {
                return Err(power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::McPat,
                    "duplicate totals tag",
                ));
            }
        }
        let totals_tag = xml_first_tag(input, "totals", ExternalPowerAnalysisKind::McPat)?;
        let totals_attributes = xml_attributes(totals_tag, ExternalPowerAnalysisKind::McPat)?;
        let total_dynamic_watts = parse_f64_attribute(
            &totals_attributes,
            "dynamic_watts",
            ExternalPowerAnalysisKind::McPat,
            "totals",
        )?;
        let total_static_watts = parse_f64_attribute(
            &totals_attributes,
            "leakage_watts",
            ExternalPowerAnalysisKind::McPat,
            "totals",
        )?;
        let total_watts = parse_f64_attribute(
            &totals_attributes,
            "total_watts",
            ExternalPowerAnalysisKind::McPat,
            "totals",
        )?;
        let export = Self::new(ExternalPowerAnalysisKind::McPat, tick, records)?;
        validate_imported_total(
            ExternalPowerAnalysisKind::McPat,
            export.total_dynamic_watts(),
            total_dynamic_watts,
            export.records().len(),
            "dynamic watts",
        )?;
        validate_imported_total(
            ExternalPowerAnalysisKind::McPat,
            export.total_static_watts(),
            total_static_watts,
            export.records().len(),
            "static watts",
        )?;
        validate_imported_total(
            ExternalPowerAnalysisKind::McPat,
            export.total_watts(),
            total_watts,
            export.records().len(),
            "watts",
        )?;
        Ok(export)
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
        let mut records = parse_csv_records(input)?.into_iter();
        let header = records.next().ok_or_else(|| {
            power_analysis_artifact_error(ExternalPowerAnalysisKind::Dsent, "missing CSV header")
        })?;
        if header.len() != DSENT_CSV_HEADER.len()
            || !header.iter().map(String::as_str).eq(DSENT_CSV_HEADER)
        {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                "unexpected CSV header",
            ));
        }

        let mut tick = None;
        let mut accumulators = BTreeMap::new();
        let mut totals = None;
        for fields in records {
            if fields.len() == 1 && fields[0].is_empty() {
                continue;
            }
            if fields.len() != 11 {
                return Err(power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::Dsent,
                    "CSV record must contain 11 fields",
                ));
            }
            match fields[0].as_str() {
                "component" => {
                    let line_tick = parse_tick_field(&fields[1], "component tick")?;
                    match tick {
                        Some(previous_tick) if previous_tick != line_tick => {
                            return Err(power_analysis_artifact_error(
                                ExternalPowerAnalysisKind::Dsent,
                                "component ticks are inconsistent",
                            ));
                        }
                        Some(_) => {}
                        None => tick = Some(line_tick),
                    }
                    let target = fields[2].clone();
                    if target.is_empty() {
                        return Err(power_analysis_artifact_error(
                            ExternalPowerAnalysisKind::Dsent,
                            "component target is empty",
                        ));
                    }
                    let current_state = parse_power_state_field(&fields[3], "component state")?;
                    let temperature_c = parse_f64_field(&fields[4], "component temperature")?;
                    let dynamic_watts = parse_f64_field(&fields[5], "component dynamic watts")?;
                    let static_watts = parse_f64_field(&fields[6], "component static watts")?;
                    let component_total_watts =
                        parse_f64_field(&fields[7], "component total watts")?;
                    validate_component_total(
                        ExternalPowerAnalysisKind::Dsent,
                        dynamic_watts,
                        static_watts,
                        component_total_watts,
                        "component",
                    )?;
                    let residency_state =
                        parse_power_state_field(&fields[8], "component residency state")?;
                    let residency_ticks =
                        parse_tick_field(&fields[9], "component residency ticks")?;
                    let entry = accumulators.entry(target.clone()).or_insert_with(|| {
                        DsentRecordAccumulator {
                            target,
                            current_state,
                            temperature_c,
                            dynamic_watts,
                            static_watts,
                            residency: Vec::new(),
                        }
                    });
                    entry.merge(
                        current_state,
                        temperature_c,
                        dynamic_watts,
                        static_watts,
                        residency_state,
                        residency_ticks,
                    )?;
                }
                "total" => {
                    if totals.is_some() {
                        return Err(power_analysis_artifact_error(
                            ExternalPowerAnalysisKind::Dsent,
                            "duplicate total row",
                        ));
                    }
                    let line_tick = parse_tick_field(&fields[1], "total tick")?;
                    totals = Some((
                        line_tick,
                        parse_f64_field(&fields[5], "total dynamic watts")?,
                        parse_f64_field(&fields[6], "total static watts")?,
                        parse_f64_field(&fields[7], "total watts")?,
                    ));
                }
                _ => {
                    return Err(power_analysis_artifact_error(
                        ExternalPowerAnalysisKind::Dsent,
                        "unknown CSV record type",
                    ));
                }
            }
        }
        let tick = tick.ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                "no component records found",
            )
        })?;
        let records = accumulators
            .into_values()
            .map(DsentRecordAccumulator::into_record)
            .collect::<Result<Vec<_>, _>>()?;
        let export = Self::new(ExternalPowerAnalysisKind::Dsent, tick, records)?;
        let (total_tick, total_dynamic_watts, total_static_watts, total_watts) = totals
            .ok_or_else(|| {
                power_analysis_artifact_error(ExternalPowerAnalysisKind::Dsent, "missing total row")
            })?;
        if total_tick != tick {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                "total tick does not match component tick",
            ));
        }
        validate_imported_total(
            ExternalPowerAnalysisKind::Dsent,
            export.total_dynamic_watts(),
            total_dynamic_watts,
            export.records().len(),
            "dynamic watts",
        )?;
        validate_imported_total(
            ExternalPowerAnalysisKind::Dsent,
            export.total_static_watts(),
            total_static_watts,
            export.records().len(),
            "static watts",
        )?;
        validate_imported_total(
            ExternalPowerAnalysisKind::Dsent,
            export.total_watts(),
            total_watts,
            export.records().len(),
            "watts",
        )?;
        Ok(export)
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

fn xml_first_tag<'a>(
    input: &'a str,
    name: &str,
    kind: ExternalPowerAnalysisKind,
) -> Result<&'a str, PowerError> {
    let start = xml_tag_start_indices(input, name)
        .into_iter()
        .next()
        .ok_or_else(|| power_analysis_artifact_error(kind, format!("missing {name} tag")))?;
    let tag = &input[start..];
    let end = tag.find('>').ok_or_else(|| {
        power_analysis_artifact_error(kind, format!("{name} tag is not terminated"))
    })?;
    Ok(&tag[..=end])
}

fn xml_tag_count(input: &str, name: &str) -> usize {
    xml_tag_start_indices(input, name).len()
}

fn xml_tag_start_indices(input: &str, name: &str) -> Vec<usize> {
    let marker = format!("<{name}");
    input
        .match_indices(&marker)
        .filter_map(|(index, _)| {
            let boundary = index + marker.len();
            matches!(
                input.as_bytes().get(boundary).copied(),
                Some(b'>') | Some(b'/') | Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n')
            )
            .then_some(index)
        })
        .collect()
}

fn xml_attributes(
    tag: &str,
    kind: ExternalPowerAnalysisKind,
) -> Result<BTreeMap<String, String>, PowerError> {
    let tag = tag
        .strip_prefix('<')
        .ok_or_else(|| power_analysis_artifact_error(kind, "XML tag must start with '<'"))?;
    let tag = tag
        .strip_suffix('>')
        .ok_or_else(|| power_analysis_artifact_error(kind, "XML tag must end with '>'"))?;
    let tag = tag.strip_suffix('/').unwrap_or(tag).trim_end();
    let mut index = tag.find(char::is_whitespace).unwrap_or(tag.len());
    let bytes = tag.as_bytes();
    let mut attributes = BTreeMap::new();
    while index < tag.len() {
        while index < tag.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == tag.len() {
            break;
        }
        let key_start = index;
        while index < tag.len() && !matches!(bytes[index], b'=' | b' ' | b'\t' | b'\r' | b'\n') {
            index += 1;
        }
        let key = &tag[key_start..index];
        while index < tag.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == tag.len() || bytes[index] != b'=' {
            return Err(power_analysis_artifact_error(
                kind,
                format!("attribute {key} is missing '='"),
            ));
        }
        index += 1;
        while index < tag.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == tag.len() || bytes[index] != b'"' {
            return Err(power_analysis_artifact_error(
                kind,
                format!("attribute {key} is not quoted"),
            ));
        }
        index += 1;
        let value_start = index;
        while index < tag.len() && bytes[index] != b'"' {
            index += 1;
        }
        if index == tag.len() {
            return Err(power_analysis_artifact_error(
                kind,
                format!("attribute {key} is missing a closing quote"),
            ));
        }
        let value = decode_xml_attribute_value(&tag[value_start..index], kind)?;
        index += 1;
        if attributes.insert(key.to_string(), value).is_some() {
            return Err(power_analysis_artifact_error(
                kind,
                format!("attribute {key} appears more than once"),
            ));
        }
    }
    Ok(attributes)
}

fn decode_xml_attribute_value(
    value: &str,
    kind: ExternalPowerAnalysisKind,
) -> Result<String, PowerError> {
    let mut output = String::new();
    let mut remaining = value;
    while let Some(entity_start) = remaining.find('&') {
        output.push_str(&remaining[..entity_start]);
        remaining = &remaining[entity_start + 1..];
        let entity_end = remaining.find(';').ok_or_else(|| {
            power_analysis_artifact_error(kind, "XML attribute entity is not terminated")
        })?;
        match &remaining[..entity_end] {
            "amp" => output.push('&'),
            "quot" => output.push('"'),
            "apos" => output.push('\''),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            entity => {
                return Err(power_analysis_artifact_error(
                    kind,
                    format!("unsupported XML entity {entity}"),
                ));
            }
        }
        remaining = &remaining[entity_end + 1..];
    }
    output.push_str(remaining);
    Ok(output)
}

fn required_attribute<'a>(
    attributes: &'a BTreeMap<String, String>,
    name: &str,
    kind: ExternalPowerAnalysisKind,
    context: &str,
) -> Result<&'a str, PowerError> {
    attributes.get(name).map(String::as_str).ok_or_else(|| {
        power_analysis_artifact_error(kind, format!("{context} is missing {name} attribute"))
    })
}

fn parse_tick_attribute(
    attributes: &BTreeMap<String, String>,
    name: &str,
    kind: ExternalPowerAnalysisKind,
    context: &str,
) -> Result<Tick, PowerError> {
    let value = required_attribute(attributes, name, kind, context)?;
    value.parse::<Tick>().map_err(|_| {
        power_analysis_artifact_error(
            kind,
            format!("{context} attribute {name} is not a valid tick"),
        )
    })
}

fn parse_f64_attribute(
    attributes: &BTreeMap<String, String>,
    name: &str,
    kind: ExternalPowerAnalysisKind,
    context: &str,
) -> Result<f64, PowerError> {
    let value = required_attribute(attributes, name, kind, context)?;
    value.parse::<f64>().map_err(|_| {
        power_analysis_artifact_error(
            kind,
            format!("{context} attribute {name} is not a valid number"),
        )
    })
}

fn parse_power_state_attribute(
    attributes: &BTreeMap<String, String>,
    name: &str,
    kind: ExternalPowerAnalysisKind,
    context: &str,
) -> Result<PowerStateKind, PowerError> {
    parse_power_state_kind(required_attribute(attributes, name, kind, context)?).map_err(|_| {
        power_analysis_artifact_error(
            kind,
            format!("{context} attribute {name} is not a valid power state"),
        )
    })
}

fn mcpat_residency_entries(body: &str) -> Result<Vec<(PowerStateKind, Tick)>, PowerError> {
    let mut entries = Vec::new();
    let mut remaining = body;
    while let Some(residency_start) = remaining.find("<residency ") {
        let residency = &remaining[residency_start..];
        let tag_end = residency.find('>').ok_or_else(|| {
            power_analysis_artifact_error(
                ExternalPowerAnalysisKind::McPat,
                "residency tag is not terminated",
            )
        })?;
        let tag = &residency[..=tag_end];
        let attributes = xml_attributes(tag, ExternalPowerAnalysisKind::McPat)?;
        let state = parse_power_state_attribute(
            &attributes,
            "state",
            ExternalPowerAnalysisKind::McPat,
            "residency",
        )?;
        let ticks = parse_tick_attribute(
            &attributes,
            "ticks",
            ExternalPowerAnalysisKind::McPat,
            "residency",
        )?;
        if entries
            .iter()
            .any(|(existing_state, _)| *existing_state == state)
        {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::McPat,
                format!("component repeats residency state {state:?}"),
            ));
        }
        entries.push((state, ticks));
        remaining = &residency[tag_end + 1..];
    }
    if entries.is_empty() {
        return Err(power_analysis_artifact_error(
            ExternalPowerAnalysisKind::McPat,
            "component has no residency entries",
        ));
    }
    Ok(entries)
}

fn parse_csv_records(input: &str) -> Result<Vec<Vec<String>>, PowerError> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;
    let mut field_was_quoted = false;
    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' if in_quotes => {
                in_quotes = false;
            }
            '"' if field.is_empty() && !field_was_quoted => {
                in_quotes = true;
                field_was_quoted = true;
            }
            ',' if !in_quotes => {
                fields.push(std::mem::take(&mut field));
                field_was_quoted = false;
            }
            '\n' if !in_quotes => {
                fields.push(std::mem::take(&mut field));
                record.append(&mut fields);
                if !(record.len() == 1 && record[0].is_empty()) {
                    records.push(std::mem::take(&mut record));
                } else {
                    record.clear();
                }
                field_was_quoted = false;
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                fields.push(std::mem::take(&mut field));
                record.append(&mut fields);
                if !(record.len() == 1 && record[0].is_empty()) {
                    records.push(std::mem::take(&mut record));
                } else {
                    record.clear();
                }
                field_was_quoted = false;
            }
            '"' => {
                return Err(power_analysis_artifact_error(
                    ExternalPowerAnalysisKind::Dsent,
                    "CSV quote appears outside a quoted field",
                ));
            }
            _ => field.push(character),
        }
    }
    if in_quotes {
        return Err(power_analysis_artifact_error(
            ExternalPowerAnalysisKind::Dsent,
            "CSV quoted field is not terminated",
        ));
    }
    if !field.is_empty() || field_was_quoted || !fields.is_empty() {
        fields.push(field);
        record.append(&mut fields);
        if !(record.len() == 1 && record[0].is_empty()) {
            records.push(record);
        }
    }
    Ok(records)
}

fn parse_tick_field(value: &str, context: &str) -> Result<Tick, PowerError> {
    value.parse::<Tick>().map_err(|_| {
        power_analysis_artifact_error(
            ExternalPowerAnalysisKind::Dsent,
            format!("{context} is not a valid tick"),
        )
    })
}

fn parse_f64_field(value: &str, context: &str) -> Result<f64, PowerError> {
    value.parse::<f64>().map_err(|_| {
        power_analysis_artifact_error(
            ExternalPowerAnalysisKind::Dsent,
            format!("{context} is not a valid number"),
        )
    })
}

fn parse_power_state_field(value: &str, context: &str) -> Result<PowerStateKind, PowerError> {
    parse_power_state_kind(value).map_err(|_| {
        power_analysis_artifact_error(
            ExternalPowerAnalysisKind::Dsent,
            format!("{context} is not a valid power state"),
        )
    })
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
            MCPAT_REPORT_DEFAULT_TEMPERATURE_C,
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

#[derive(Clone, Debug)]
struct DsentRecordAccumulator {
    target: String,
    current_state: PowerStateKind,
    temperature_c: f64,
    dynamic_watts: f64,
    static_watts: f64,
    residency: Vec<(PowerStateKind, Tick)>,
}

impl DsentRecordAccumulator {
    fn merge(
        &mut self,
        current_state: PowerStateKind,
        temperature_c: f64,
        dynamic_watts: f64,
        static_watts: f64,
        residency_state: PowerStateKind,
        residency_ticks: Tick,
    ) -> Result<(), PowerError> {
        if self.current_state != current_state
            || self.temperature_c != temperature_c
            || self.dynamic_watts != dynamic_watts
            || self.static_watts != static_watts
        {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!("component {} rows disagree", self.target),
            ));
        }
        if self
            .residency
            .iter()
            .any(|(existing_state, _)| *existing_state == residency_state)
        {
            return Err(power_analysis_artifact_error(
                ExternalPowerAnalysisKind::Dsent,
                format!(
                    "component {} repeats residency state {residency_state:?}",
                    self.target
                ),
            ));
        }
        self.residency.push((residency_state, residency_ticks));
        Ok(())
    }

    fn into_record(self) -> Result<PowerAnalysisRecord, PowerError> {
        PowerAnalysisRecord::new(
            self.target,
            self.current_state,
            PowerResidency::new(self.residency),
            self.temperature_c,
            PowerEstimate::new(self.dynamic_watts, self.static_watts),
        )
    }
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
