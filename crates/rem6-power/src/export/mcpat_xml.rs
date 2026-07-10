use rem6_kernel::Tick;
use roxmltree::{Document, Node};

use crate::{PowerError, PowerEstimate, PowerResidency, PowerStateKind};

use super::{
    parse_power_state_kind, power_analysis_artifact_error, validate_component_total,
    validate_imported_total, validate_residency_tick_sum, ExternalPowerAnalysisKind,
    PowerAnalysisExport, PowerAnalysisRecord,
};

const KIND: ExternalPowerAnalysisKind = ExternalPowerAnalysisKind::McPat;

pub(super) fn parse(input: &str) -> Result<PowerAnalysisExport, PowerError> {
    let document = Document::parse(input)
        .map_err(|_| power_analysis_artifact_error(KIND, "invalid XML syntax"))?;
    let root = document.root_element();
    if !is_unqualified_element(root, "mcpat_power") {
        return Err(power_analysis_artifact_error(
            KIND,
            "root element must be mcpat_power",
        ));
    }

    let tick = parse_tick_attribute(root, "tick", "mcpat_power")?;
    let records = direct_children(root, "component")
        .map(parse_component)
        .collect::<Result<Vec<_>, _>>()?;
    if records.is_empty() {
        return Err(power_analysis_artifact_error(
            KIND,
            "no component records found",
        ));
    }

    let totals = direct_children(root, "totals").collect::<Vec<_>>();
    let totals = match totals.as_slice() {
        [] => return Err(power_analysis_artifact_error(KIND, "missing totals tag")),
        [totals] => *totals,
        _ => return Err(power_analysis_artifact_error(KIND, "duplicate totals tag")),
    };
    let total_dynamic_watts = parse_f64_attribute(totals, "dynamic_watts", "totals")?;
    let total_static_watts = parse_f64_attribute(totals, "leakage_watts", "totals")?;
    let total_watts = parse_f64_attribute(totals, "total_watts", "totals")?;
    let export = PowerAnalysisExport::new(KIND, tick, records)?;
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

fn parse_component(component: Node<'_, '_>) -> Result<PowerAnalysisRecord, PowerError> {
    let target = required_attribute(component, "id", "component")?;
    let current_state = parse_power_state_attribute(component, "state", "component")?;

    let power = single_component_child(component, "power", target)?;
    let dynamic_watts = parse_f64_attribute(power, "dynamic_watts", "power")?;
    let static_watts = parse_f64_attribute(power, "leakage_watts", "power")?;
    let component_total_watts = parse_f64_attribute(power, "total_watts", "power")?;
    validate_component_total(
        KIND,
        dynamic_watts,
        static_watts,
        component_total_watts,
        "power",
    )?;

    let thermal = single_component_child(component, "thermal", target)?;
    let temperature_c = parse_f64_attribute(thermal, "temperature_c", "thermal")?;
    let residency = parse_residency(component)?;

    PowerAnalysisRecord::new(
        target,
        current_state,
        PowerResidency::new(residency),
        temperature_c,
        PowerEstimate::new(dynamic_watts, static_watts),
    )
}

fn parse_residency(component: Node<'_, '_>) -> Result<Vec<(PowerStateKind, Tick)>, PowerError> {
    let mut entries = Vec::new();
    for residency in direct_children(component, "residency") {
        let state = parse_power_state_attribute(residency, "state", "residency")?;
        let ticks = parse_tick_attribute(residency, "ticks", "residency")?;
        if entries
            .iter()
            .any(|(existing_state, _)| *existing_state == state)
        {
            return Err(power_analysis_artifact_error(
                KIND,
                format!("component repeats residency state {state:?}"),
            ));
        }
        entries.push((state, ticks));
    }
    if entries.is_empty() {
        return Err(power_analysis_artifact_error(
            KIND,
            "component has no residency entries",
        ));
    }
    validate_residency_tick_sum(KIND, &entries, "component")?;
    Ok(entries)
}

fn direct_children<'a, 'input>(
    parent: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    parent
        .children()
        .filter(Node::is_element)
        .filter(move |child| is_unqualified_element(*child, name))
}

fn single_component_child<'a, 'input>(
    component: Node<'a, 'input>,
    name: &str,
    target: &str,
) -> Result<Node<'a, 'input>, PowerError> {
    let mut children = component
        .children()
        .filter(Node::is_element)
        .filter(|child| is_unqualified_element(*child, name));
    let child = children
        .next()
        .ok_or_else(|| power_analysis_artifact_error(KIND, format!("missing {name} tag")))?;
    if children.next().is_some() {
        return Err(power_analysis_artifact_error(
            KIND,
            format!("component {target} has duplicate {name} tag"),
        ));
    }
    Ok(child)
}

fn is_unqualified_element(node: Node<'_, '_>, name: &str) -> bool {
    node.tag_name().namespace().is_none() && node.tag_name().name() == name
}

fn required_attribute<'a>(
    node: Node<'a, '_>,
    name: &str,
    context: &str,
) -> Result<&'a str, PowerError> {
    node.attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        .map(|attribute| attribute.value())
        .ok_or_else(|| {
            power_analysis_artifact_error(KIND, format!("{context} is missing {name} attribute"))
        })
}

fn parse_tick_attribute(node: Node<'_, '_>, name: &str, context: &str) -> Result<Tick, PowerError> {
    required_attribute(node, name, context)?
        .parse::<Tick>()
        .map_err(|_| {
            power_analysis_artifact_error(
                KIND,
                format!("{context} attribute {name} is not a valid tick"),
            )
        })
}

fn parse_f64_attribute(node: Node<'_, '_>, name: &str, context: &str) -> Result<f64, PowerError> {
    required_attribute(node, name, context)?
        .parse::<f64>()
        .map_err(|_| {
            power_analysis_artifact_error(
                KIND,
                format!("{context} attribute {name} is not a valid number"),
            )
        })
}

fn parse_power_state_attribute(
    node: Node<'_, '_>,
    name: &str,
    context: &str,
) -> Result<PowerStateKind, PowerError> {
    parse_power_state_kind(required_attribute(node, name, context)?).map_err(|_| {
        power_analysis_artifact_error(
            KIND,
            format!("{context} attribute {name} is not a valid power state"),
        )
    })
}
