use std::collections::BTreeMap;

use csv::{ReaderBuilder, StringRecord};
use rem6_kernel::Tick;

use crate::{PowerError, PowerEstimate, PowerResidency, PowerStateKind};

use super::{
    parse_power_state_kind, power_analysis_artifact_error, validate_component_total,
    validate_imported_total, validate_residency_tick_sum, ExternalPowerAnalysisKind,
    PowerAnalysisExport, PowerAnalysisRecord,
};

const KIND: ExternalPowerAnalysisKind = ExternalPowerAnalysisKind::Dsent;
const HEADER: [&str; 11] = [
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

pub(super) fn parse(input: &str) -> Result<PowerAnalysisExport, PowerError> {
    let mut records = read_records(input)?.into_iter();
    let header = records
        .next()
        .ok_or_else(|| power_analysis_artifact_error(KIND, "missing CSV header"))?;
    if header.len() != HEADER.len() || !header.iter().eq(HEADER) {
        return Err(power_analysis_artifact_error(KIND, "unexpected CSV header"));
    }

    let mut tick = None;
    let mut accumulators = BTreeMap::new();
    let mut totals = None;
    for fields in records {
        if fields.len() != HEADER.len() {
            return Err(power_analysis_artifact_error(
                KIND,
                "CSV record must contain 11 fields",
            ));
        }
        match fields.get(0).expect("record length was checked") {
            "component" => {
                let line_tick = parse_tick_field(
                    fields.get(1).expect("record length was checked"),
                    "component tick",
                )?;
                match tick {
                    Some(previous_tick) if previous_tick != line_tick => {
                        return Err(power_analysis_artifact_error(
                            KIND,
                            "component ticks are inconsistent",
                        ));
                    }
                    Some(_) => {}
                    None => tick = Some(line_tick),
                }
                let target = fields
                    .get(2)
                    .expect("record length was checked")
                    .to_string();
                if target.is_empty() {
                    return Err(power_analysis_artifact_error(
                        KIND,
                        "component target is empty",
                    ));
                }
                let current_state = parse_power_state_field(
                    fields.get(3).expect("record length was checked"),
                    "component state",
                )?;
                let temperature_c = parse_f64_field(
                    fields.get(4).expect("record length was checked"),
                    "component temperature",
                )?;
                let dynamic_watts = parse_f64_field(
                    fields.get(5).expect("record length was checked"),
                    "component dynamic watts",
                )?;
                let static_watts = parse_f64_field(
                    fields.get(6).expect("record length was checked"),
                    "component static watts",
                )?;
                let component_total_watts = parse_f64_field(
                    fields.get(7).expect("record length was checked"),
                    "component total watts",
                )?;
                validate_component_total(
                    KIND,
                    dynamic_watts,
                    static_watts,
                    component_total_watts,
                    "component",
                )?;
                let residency_state = parse_power_state_field(
                    fields.get(8).expect("record length was checked"),
                    "component residency state",
                )?;
                let residency_ticks = parse_tick_field(
                    fields.get(9).expect("record length was checked"),
                    "component residency ticks",
                )?;
                let entry =
                    accumulators
                        .entry(target.clone())
                        .or_insert_with(|| DsentRecordAccumulator {
                            target,
                            current_state,
                            temperature_c,
                            dynamic_watts,
                            static_watts,
                            residency: Vec::new(),
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
                    return Err(power_analysis_artifact_error(KIND, "duplicate total row"));
                }
                totals = Some((
                    parse_tick_field(
                        fields.get(1).expect("record length was checked"),
                        "total tick",
                    )?,
                    parse_f64_field(
                        fields.get(5).expect("record length was checked"),
                        "total dynamic watts",
                    )?,
                    parse_f64_field(
                        fields.get(6).expect("record length was checked"),
                        "total static watts",
                    )?,
                    parse_f64_field(
                        fields.get(7).expect("record length was checked"),
                        "total watts",
                    )?,
                ));
            }
            _ => {
                return Err(power_analysis_artifact_error(
                    KIND,
                    "unknown CSV record type",
                ))
            }
        }
    }

    let tick =
        tick.ok_or_else(|| power_analysis_artifact_error(KIND, "no component records found"))?;
    let records = accumulators
        .into_values()
        .map(DsentRecordAccumulator::into_record)
        .collect::<Result<Vec<_>, _>>()?;
    let export = PowerAnalysisExport::new(KIND, tick, records)?;
    let (total_tick, total_dynamic_watts, total_static_watts, total_watts) =
        totals.ok_or_else(|| power_analysis_artifact_error(KIND, "missing total row"))?;
    if total_tick != tick {
        return Err(power_analysis_artifact_error(
            KIND,
            "total tick does not match component tick",
        ));
    }
    validate_imported_total(
        KIND,
        export.total_dynamic_watts(),
        total_dynamic_watts,
        export.records().len(),
        "dynamic watts",
    )?;
    validate_imported_total(
        KIND,
        export.total_static_watts(),
        total_static_watts,
        export.records().len(),
        "static watts",
    )?;
    validate_imported_total(
        KIND,
        export.total_watts(),
        total_watts,
        export.records().len(),
        "watts",
    )?;
    Ok(export)
}

fn read_records(input: &str) -> Result<Vec<StringRecord>, PowerError> {
    validate_quote_boundaries(input)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(input.as_bytes());
    reader
        .records()
        .map(|record| record.map_err(|_| power_analysis_artifact_error(KIND, "invalid CSV syntax")))
        .filter(|record| {
            record.as_ref().map_or(true, |record| {
                !(record.len() == 1 && record.get(0) == Some(""))
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QuoteState {
    FieldStart,
    Unquoted,
    Quoted,
    AfterQuote,
}

fn validate_quote_boundaries(input: &str) -> Result<(), PowerError> {
    let mut state = QuoteState::FieldStart;
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        state = match state {
            QuoteState::FieldStart => match character {
                '"' => QuoteState::Quoted,
                ',' | '\r' | '\n' => QuoteState::FieldStart,
                _ => QuoteState::Unquoted,
            },
            QuoteState::Unquoted => match character {
                '"' => {
                    return Err(power_analysis_artifact_error(
                        KIND,
                        "CSV quote appears outside a quoted field",
                    ));
                }
                ',' | '\r' | '\n' => QuoteState::FieldStart,
                _ => QuoteState::Unquoted,
            },
            QuoteState::Quoted => {
                if character == '"' {
                    if characters.peek() == Some(&'"') {
                        characters.next();
                        QuoteState::Quoted
                    } else {
                        QuoteState::AfterQuote
                    }
                } else {
                    QuoteState::Quoted
                }
            }
            QuoteState::AfterQuote => match character {
                ',' | '\r' | '\n' => QuoteState::FieldStart,
                _ => {
                    return Err(power_analysis_artifact_error(
                        KIND,
                        "CSV field contains characters after a closing quote",
                    ));
                }
            },
        };
    }
    if state == QuoteState::Quoted {
        return Err(power_analysis_artifact_error(
            KIND,
            "CSV quoted field is not terminated",
        ));
    }
    Ok(())
}

fn parse_tick_field(value: &str, context: &str) -> Result<Tick, PowerError> {
    value
        .parse::<Tick>()
        .map_err(|_| power_analysis_artifact_error(KIND, format!("{context} is not a valid tick")))
}

fn parse_f64_field(value: &str, context: &str) -> Result<f64, PowerError> {
    value.parse::<f64>().map_err(|_| {
        power_analysis_artifact_error(KIND, format!("{context} is not a valid number"))
    })
}

fn parse_power_state_field(value: &str, context: &str) -> Result<PowerStateKind, PowerError> {
    parse_power_state_kind(value).map_err(|_| {
        power_analysis_artifact_error(KIND, format!("{context} is not a valid power state"))
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
                KIND,
                format!("component {} rows disagree", self.target),
            ));
        }
        if self
            .residency
            .iter()
            .any(|(existing_state, _)| *existing_state == residency_state)
        {
            return Err(power_analysis_artifact_error(
                KIND,
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
        validate_residency_tick_sum(KIND, &self.residency, &format!("component {}", self.target))?;
        PowerAnalysisRecord::new(
            self.target,
            self.current_state,
            PowerResidency::new(self.residency),
            self.temperature_c,
            PowerEstimate::new(self.dynamic_watts, self.static_watts),
        )
    }
}
