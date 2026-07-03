use crate::formatting::json_escape;
use crate::{
    Rem6GuestHostCallSummary, Rem6HostActionSummary, Rem6HostCheckpointSummary,
    Rem6HostExecutionModeSwitchSummary, Rem6HostInjectedCommandSummary, Rem6HostStatsDumpSummary,
    Rem6HostStatsResetSummary, Rem6HostStopActionSummary, Rem6HostWorkMarkerSummary,
};

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

fn field_u64(name: &'static str, value: u64) -> Rem6HostActionTraceField {
    Rem6HostActionTraceField {
        name,
        value: Rem6HostActionTraceValue::U64(value),
    }
}
