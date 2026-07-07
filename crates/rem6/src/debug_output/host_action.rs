use std::collections::BTreeMap;

use crate::formatting::json_escape;
use crate::{
    Rem6ExecutionModeQuiescenceGateSummary, Rem6ExecutionModeStateTransferSummary,
    Rem6GuestHostCallSummary, Rem6HostActionSummary, Rem6HostCheckpointSummary,
    Rem6HostExecutionModeSummary, Rem6HostExecutionModeSwitchSummary,
    Rem6HostInjectedCommandSummary, Rem6HostStatsDumpSummary, Rem6HostStatsResetSummary,
    Rem6HostStopActionSummary, Rem6HostWorkMarkerSummary,
};

const EXECUTION_MODE_AUTHORITY_JSON_LANES: [&str; 3] = ["functional", "timing", "detailed"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostActionTraceRecord {
    kind: &'static str,
    rank: u8,
    tick: u64,
    event: Option<u64>,
    source: Option<u32>,
    fields: Vec<Rem6HostActionTraceField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6HostActionTraceField {
    name: &'static str,
    value: Rem6HostActionTraceValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Rem6HostActionTraceValue {
    Bool(bool),
    I64(i64),
    Json(String),
    String(String),
    U64(u64),
}

impl Rem6HostActionTraceRecord {
    fn new(
        kind: &'static str,
        rank: u8,
        tick: u64,
        event: Option<u64>,
        source: Option<u32>,
        fields: Vec<Rem6HostActionTraceField>,
    ) -> Self {
        Self {
            kind,
            rank,
            tick,
            event,
            source,
            fields,
        }
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn to_json(&self) -> String {
        let mut fields = vec![
            format!("\"kind\":\"{}\"", self.kind),
            format!("\"tick\":{}", self.tick),
        ];
        if let Some(event) = self.event {
            fields.push(format!("\"event\":{event}"));
        }
        if let Some(source) = self.source {
            fields.push(format!("\"source\":{source}"));
        }
        fields.extend(self.fields.iter().map(Rem6HostActionTraceField::to_json));
        format!("{{{}}}", fields.join(","))
    }
}

impl Rem6HostActionTraceField {
    fn to_json(&self) -> String {
        format!("\"{}\":{}", self.name, self.value.to_json())
    }
}

impl Rem6HostActionTraceValue {
    fn to_json(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::I64(value) => value.to_string(),
            Self::Json(value) => value.clone(),
            Self::String(value) => format!("\"{}\"", json_escape(value)),
            Self::U64(value) => value.to_string(),
        }
    }
}

pub(crate) fn host_action_trace_records(
    actions: &Rem6HostActionSummary,
) -> Vec<Rem6HostActionTraceRecord> {
    let mut records = Vec::new();
    records.extend(
        actions
            .injected_commands
            .iter()
            .map(injected_command_record),
    );
    records.extend(actions.guest_host_calls.iter().map(guest_host_call_record));
    records.extend(
        actions
            .roi_begin
            .iter()
            .map(|action| work_marker_record("roi_begin", 30, action)),
    );
    records.extend(
        actions
            .roi_end
            .iter()
            .map(|action| work_marker_record("roi_end", 31, action)),
    );
    records.extend(actions.stats_resets.iter().map(stats_reset_record));
    records.extend(actions.stats_dumps.iter().map(stats_dump_record));
    records.extend(actions.checkpoints.iter().map(checkpoint_record));
    records.extend(
        actions
            .checkpoint_restores
            .iter()
            .map(checkpoint_restore_record),
    );
    records.extend(
        actions
            .execution_mode_switches
            .iter()
            .map(execution_mode_switch_record),
    );
    records.extend(actions.stops.iter().map(stop_record));
    records.sort_by_key(|record| {
        (
            record.tick,
            record.event.unwrap_or(u64::MAX),
            record.rank,
            record.source.unwrap_or(u32::MAX),
        )
    });
    records
}

fn injected_command_record(action: &Rem6HostInjectedCommandSummary) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        "injected_command",
        10,
        action.tick,
        Some(action.event),
        Some(action.source),
        vec![field_string("command", action.command.as_str())],
    )
}

fn guest_host_call_record(action: &Rem6GuestHostCallSummary) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        "guest_host_call",
        20,
        action.tick,
        Some(action.event),
        Some(action.source),
        vec![
            field_u64("selector", action.selector),
            field_u64("argument_count", action.argument_count),
            field_u64("payload_bytes", action.payload_bytes),
            field_i64("response_status", i64::from(action.response_status)),
            field_u64("response_return_count", action.response_return_count),
            field_u64("response_payload_bytes", action.response_payload_bytes),
        ],
    )
}

fn work_marker_record(
    kind: &'static str,
    rank: u8,
    action: &Rem6HostWorkMarkerSummary,
) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        kind,
        rank,
        action.tick,
        Some(action.event),
        Some(action.source),
        vec![
            field_u64("work_id", action.work_id),
            field_u64("thread_id", action.thread_id),
        ],
    )
}

fn stats_reset_record(action: &Rem6HostStatsResetSummary) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        "stats_reset",
        41,
        action.tick,
        None,
        None,
        vec![field_u64("id", action.id), field_u64("epoch", action.epoch)],
    )
}

fn stats_dump_record(action: &Rem6HostStatsDumpSummary) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        "stats_dump",
        40,
        action.tick,
        None,
        None,
        vec![
            field_u64("id", action.id),
            field_u64("epoch", action.epoch),
            field_u64("reset_tick", action.reset_tick),
        ],
    )
}

fn checkpoint_record(action: &Rem6HostCheckpointSummary) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        "checkpoint",
        50,
        action.tick,
        Some(action.event),
        Some(action.source),
        vec![
            field_string("label", action.label.as_str()),
            field_u64("manifest_tick", action.manifest_tick),
            field_u64("component_count", action.component_count),
            field_u64("chunk_count", action.chunk_count),
            field_u64("payload_bytes", action.payload_bytes),
        ],
    )
}

fn checkpoint_restore_record(action: &Rem6HostCheckpointSummary) -> Rem6HostActionTraceRecord {
    let fields = vec![
        field_string("label", action.label.as_str()),
        field_u64("manifest_tick", action.manifest_tick),
        field_u64("component_count", action.component_count),
        field_u64("chunk_count", action.chunk_count),
        field_u64("payload_bytes", action.payload_bytes),
        field_json(
            "execution_mode_authority",
            checkpoint_restore_authority_json(action),
        ),
    ];
    Rem6HostActionTraceRecord::new(
        "checkpoint_restore",
        51,
        action.tick,
        Some(action.event),
        Some(action.source),
        fields,
    )
}

fn checkpoint_restore_authority_json(action: &Rem6HostCheckpointSummary) -> String {
    execution_mode_authority_to_json(
        u64::from(action.execution_mode_authority_present),
        u64::from(action.execution_mode_authority_cleared),
        u64::from(action.execution_mode_authority_decode_error),
        &action.execution_modes,
    )
}

fn execution_mode_authority_to_json(
    present_manifests: u64,
    cleared_manifests: u64,
    decode_errors: u64,
    execution_modes: &[Rem6HostExecutionModeSummary],
) -> String {
    let mode = execution_mode_counts_to_json(execution_modes.iter().map(|mode| mode.mode));
    let target = execution_mode_targets_to_json(execution_modes);
    format!(
        "{{\"present_manifests\":{},\"cleared_manifests\":{},\"decode_errors\":{},\"targets\":{},\"mode\":{},\"target\":{}}}",
        present_manifests,
        cleared_manifests,
        decode_errors,
        execution_modes.len(),
        mode,
        target
    )
}

fn execution_mode_counts_to_json<'a>(modes: impl Iterator<Item = &'a str>) -> String {
    let mut counts = [0_u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()];
    for mode in modes {
        if let Some(index) = execution_mode_authority_lane_index(mode) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    execution_mode_count_array_to_json(counts)
}

fn execution_mode_count_array_to_json(
    counts: [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()],
) -> String {
    let fields = EXECUTION_MODE_AUTHORITY_JSON_LANES
        .iter()
        .zip(counts)
        .map(|(mode, count)| format!("\"{mode}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn execution_mode_targets_to_json(execution_modes: &[Rem6HostExecutionModeSummary]) -> String {
    let mut targets = BTreeMap::<&str, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]>::new();
    for execution_mode in execution_modes {
        let Some(index) = execution_mode_authority_lane_index(execution_mode.mode) else {
            continue;
        };
        let counts = targets.entry(execution_mode.target.as_str()).or_default();
        counts[index] = counts[index].saturating_add(1);
    }
    let fields = targets
        .into_iter()
        .map(|(target, counts)| {
            format!(
                "\"{}\":{{\"mode\":{}}}",
                json_escape(target),
                execution_mode_count_array_to_json(counts)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn execution_mode_authority_lane_index(mode: &str) -> Option<usize> {
    EXECUTION_MODE_AUTHORITY_JSON_LANES
        .iter()
        .position(|lane| *lane == mode)
}

fn execution_mode_switch_record(
    action: &Rem6HostExecutionModeSwitchSummary,
) -> Rem6HostActionTraceRecord {
    let mut fields = vec![
        field_string("target", action.target.as_str()),
        field_string("mode", action.mode),
        field_u64("stats_epoch", action.stats_epoch),
        field_u64("stats_reset_tick", action.stats_reset_tick),
        field_bool("state_transfer_captured", action.state_transfer.is_some()),
    ];
    if let Some(previous_mode) = action.previous_mode {
        fields.push(field_string("previous_mode", previous_mode));
    }
    if let Some(transfer) = &action.state_transfer {
        fields.extend([
            field_u64("state_transfer_components", transfer.component_count),
            field_u64("state_transfer_chunks", transfer.chunk_count),
            field_u64("state_transfer_payload_bytes", transfer.payload_bytes),
            field_json("state_transfer", state_transfer_json(transfer)),
        ]);
        if let Some(checker) = transfer.quiescence_gate.checker {
            fields.extend([
                field_u64("checker_checked_instructions", checker.checked_instructions),
                field_u64("checker_mismatches", checker.mismatches),
            ]);
        }
    }
    Rem6HostActionTraceRecord::new(
        "execution_mode_switch",
        60,
        action.tick,
        Some(action.event),
        Some(action.source),
        fields,
    )
}

fn state_transfer_json(transfer: &Rem6ExecutionModeStateTransferSummary) -> String {
    format!(
        "{{\"captured\":true,\"manifest_label\":\"{}\",\"manifest_tick\":{},\"component_count\":{},\"chunk_count\":{},\"payload_bytes\":{},\"quiescence_gate\":{}}}",
        json_escape(&transfer.manifest_label),
        transfer.manifest_tick,
        transfer.component_count,
        transfer.chunk_count,
        transfer.payload_bytes,
        quiescence_gate_json(&transfer.quiescence_gate),
    )
}

fn quiescence_gate_json(gate: &Rem6ExecutionModeQuiescenceGateSummary) -> String {
    let checker = gate
        .checker
        .map(|checker| {
            format!(
                ",\"checker\":{{\"checked_instructions\":{},\"mismatches\":{}}}",
                checker.checked_instructions, checker.mismatches
            )
        })
        .unwrap_or_default();
    format!(
        "{{\"validated\":{},\"target\":\"{}\",\"captured_component_count\":{},\"captured_chunk_count\":{},\"captured_payload_bytes\":{}{}}}",
        gate.validated,
        json_escape(&gate.target),
        gate.captured_component_count,
        gate.captured_chunk_count,
        gate.captured_payload_bytes,
        checker,
    )
}

fn stop_record(action: &Rem6HostStopActionSummary) -> Rem6HostActionTraceRecord {
    Rem6HostActionTraceRecord::new(
        "stop",
        70,
        action.tick,
        Some(action.event),
        Some(action.source),
        vec![field_i64("code", i64::from(action.code))],
    )
}

fn field_bool(name: &'static str, value: bool) -> Rem6HostActionTraceField {
    Rem6HostActionTraceField {
        name,
        value: Rem6HostActionTraceValue::Bool(value),
    }
}

fn field_i64(name: &'static str, value: i64) -> Rem6HostActionTraceField {
    Rem6HostActionTraceField {
        name,
        value: Rem6HostActionTraceValue::I64(value),
    }
}

fn field_string(name: &'static str, value: impl Into<String>) -> Rem6HostActionTraceField {
    Rem6HostActionTraceField {
        name,
        value: Rem6HostActionTraceValue::String(value.into()),
    }
}

fn field_json(name: &'static str, value: impl Into<String>) -> Rem6HostActionTraceField {
    Rem6HostActionTraceField {
        name,
        value: Rem6HostActionTraceValue::Json(value.into()),
    }
}

fn field_u64(name: &'static str, value: u64) -> Rem6HostActionTraceField {
    Rem6HostActionTraceField {
        name,
        value: Rem6HostActionTraceValue::U64(value),
    }
}
