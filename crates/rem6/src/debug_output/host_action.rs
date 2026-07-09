use std::collections::BTreeMap;

use crate::formatting::json_escape;
use crate::{
    Rem6ExecutionModeQuiescenceGateSummary, Rem6ExecutionModeStateTransferSummary,
    Rem6GuestHostCallSummary, Rem6HostActionSummary, Rem6HostCheckpointChunkSummary,
    Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary, Rem6HostExecutionModeSwitchSummary,
    Rem6HostInjectedCommandSummary, Rem6HostO3RuntimeCheckpointStatValue, Rem6HostStatsDumpSummary,
    Rem6HostStatsResetSummary, Rem6HostStopActionSummary, Rem6HostWorkMarkerSummary,
};

const EXECUTION_MODE_AUTHORITY_JSON_LANES: [&str; 3] = ["functional", "timing", "detailed"];

use super::checkpoint_components_json::checkpoint_components_to_json;

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
pub(crate) struct Rem6HostActionTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Rem6HostActionTraceTransferStats {
    components: u64,
    chunks: u64,
    payload_bytes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Rem6HostActionTraceChunkStats {
    chunks: u64,
    payload_bytes: u64,
    payload_checksum_accumulator: u64,
    o3_runtime_numeric: BTreeMap<String, Rem6HostO3RuntimeCheckpointStatValue>,
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

impl Rem6HostActionTraceStat {
    fn count(path: String, value: u64) -> Self {
        Self::new(path, "Count", value)
    }

    fn byte(path: String, value: u64) -> Self {
        Self::new(path, "Byte", value)
    }

    fn unspecified(path: String, value: u64) -> Self {
        Self::new(path, "Unspecified", value)
    }

    fn new(path: String, unit: &'static str, value: u64) -> Self {
        Self { path, unit, value }
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn unit(&self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

fn add_host_action_trace_chunk_stats(
    stats: &mut Rem6HostActionTraceChunkStats,
    chunk: &Rem6HostCheckpointChunkSummary,
) {
    stats.chunks += 1;
    stats.payload_bytes += chunk.payload_bytes;
    stats.payload_checksum_accumulator = stats
        .payload_checksum_accumulator
        .wrapping_add(chunk.payload_checksum);
    let Some(o3_runtime) = &chunk.o3_runtime else {
        return;
    };
    for (field, value) in o3_runtime.numeric_stat_fields() {
        stats
            .o3_runtime_numeric
            .entry(field.to_string())
            .and_modify(|current| current.merge_restore_value(value))
            .or_insert(value);
    }
}

fn push_host_action_trace_chunk_stats(
    stats: &mut Vec<Rem6HostActionTraceStat>,
    prefix: String,
    chunk_stats: Rem6HostActionTraceChunkStats,
) {
    stats.push(Rem6HostActionTraceStat::count(
        format!("{prefix}.chunks"),
        chunk_stats.chunks,
    ));
    stats.push(Rem6HostActionTraceStat::byte(
        format!("{prefix}.payload_bytes"),
        chunk_stats.payload_bytes,
    ));
    stats.push(Rem6HostActionTraceStat::unspecified(
        format!("{prefix}.payload_checksum_accumulator"),
        chunk_stats.payload_checksum_accumulator,
    ));
    for (field, value) in chunk_stats.o3_runtime_numeric {
        stats.push(Rem6HostActionTraceStat::new(
            format!("{prefix}.o3_runtime.{field}"),
            value.unit(),
            value.value(),
        ));
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

pub(crate) fn host_action_trace_checkpoint_stats(
    checkpoints: &[Rem6HostCheckpointSummary],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6HostActionTraceStat> {
    let mut component_transfers = BTreeMap::<String, Rem6HostActionTraceTransferStats>::new();
    let mut chunk_transfers = BTreeMap::<(String, String), Rem6HostActionTraceChunkStats>::new();
    for checkpoint in checkpoints {
        for component in &checkpoint.components {
            let component_path = stat_path_segment(&component.component);
            let component_stats = component_transfers
                .entry(component_path.clone())
                .or_default();
            component_stats.components += 1;
            component_stats.chunks += component.chunk_count;
            component_stats.payload_bytes += component.payload_bytes;
            for chunk in &component.chunks {
                let chunk_path = stat_path_segment(&chunk.name);
                let chunk_stats = chunk_transfers
                    .entry((component_path.clone(), chunk_path))
                    .or_default();
                add_host_action_trace_chunk_stats(chunk_stats, chunk);
            }
        }
    }

    let mut stats = Vec::new();
    for (component, transfer) in component_transfers {
        stats.push(Rem6HostActionTraceStat::count(
            format!("checkpoint.component.{component}.components"),
            transfer.components,
        ));
        stats.push(Rem6HostActionTraceStat::count(
            format!("checkpoint.component.{component}.chunks"),
            transfer.chunks,
        ));
        stats.push(Rem6HostActionTraceStat::byte(
            format!("checkpoint.component.{component}.payload_bytes"),
            transfer.payload_bytes,
        ));
    }
    for ((component, chunk), transfer) in chunk_transfers {
        push_host_action_trace_chunk_stats(
            &mut stats,
            format!("checkpoint.component.{component}.chunk.{chunk}"),
            transfer,
        );
    }
    stats
}

pub(crate) fn host_action_trace_checkpoint_restore_authority_stats(
    checkpoint_restores: &[Rem6HostCheckpointSummary],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6HostActionTraceStat> {
    let mut present_manifests = 0;
    let mut cleared_manifests = 0;
    let mut decode_errors = 0;
    let mut targets = 0;
    let mut modes = [0_u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()];
    let mut target_modes =
        BTreeMap::<String, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]>::new();

    for restore in checkpoint_restores {
        if restore.execution_mode_authority_present {
            present_manifests += 1;
        }
        if restore.execution_mode_authority_cleared {
            cleared_manifests += 1;
        }
        if restore.execution_mode_authority_decode_error {
            decode_errors += 1;
        }
        targets += restore.execution_modes.len() as u64;
        for authority in &restore.execution_modes {
            let Some(index) = execution_mode_authority_lane_index(authority.mode) else {
                continue;
            };
            modes[index] = modes[index].saturating_add(1);
            let target = stat_path_segment(&authority.target);
            let counts = target_modes.entry(target).or_default();
            counts[index] = counts[index].saturating_add(1);
        }
    }

    let mut stats = Vec::new();
    for (path, value) in [
        (
            "checkpoint_restore.execution_mode_authority.manifests",
            present_manifests,
        ),
        (
            "checkpoint_restore.execution_mode_authority.cleared_manifests",
            cleared_manifests,
        ),
        (
            "checkpoint_restore.execution_mode_authority.decode_errors",
            decode_errors,
        ),
        (
            "checkpoint_restore.execution_mode_authority.targets",
            targets,
        ),
    ] {
        stats.push(Rem6HostActionTraceStat::count(path.to_string(), value));
    }
    for (index, mode) in EXECUTION_MODE_AUTHORITY_JSON_LANES.iter().enumerate() {
        stats.push(Rem6HostActionTraceStat::count(
            format!("checkpoint_restore.execution_mode_authority.mode.{mode}"),
            modes[index],
        ));
    }
    for (target, counts) in target_modes {
        for (index, mode) in EXECUTION_MODE_AUTHORITY_JSON_LANES.iter().enumerate() {
            stats.push(Rem6HostActionTraceStat::count(
                format!("checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"),
                counts[index],
            ));
        }
    }
    stats
}

pub(crate) fn host_action_trace_execution_mode_switch_stats(
    execution_mode_switches: &[Rem6HostExecutionModeSwitchSummary],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6HostActionTraceStat> {
    let mut target_transfers = BTreeMap::<String, Rem6HostActionTraceTransferStats>::new();
    let mut target_component_transfers =
        BTreeMap::<(String, String), Rem6HostActionTraceTransferStats>::new();
    let mut target_chunk_transfers =
        BTreeMap::<(String, String, String), Rem6HostActionTraceChunkStats>::new();
    let mut quiescence_validated = 0;
    let mut quiescence_captured = Rem6HostActionTraceTransferStats::default();
    let mut target_quiescence_validated = BTreeMap::<String, u64>::new();
    let mut target_quiescence_captured =
        BTreeMap::<String, Rem6HostActionTraceTransferStats>::new();
    let mut latest_checker = None;
    let mut target_checkers = BTreeMap::new();
    for switch in execution_mode_switches {
        let Some(transfer) = switch.state_transfer.as_ref() else {
            continue;
        };
        let target = stat_path_segment(&switch.target);
        let transfer_stats = target_transfers.entry(target.clone()).or_default();
        transfer_stats.components += transfer.component_count;
        transfer_stats.chunks += transfer.chunk_count;
        transfer_stats.payload_bytes += transfer.payload_bytes;
        for component in &transfer.components {
            let component_path = stat_path_segment(&component.component);
            let component_stats = target_component_transfers
                .entry((target.clone(), component_path.clone()))
                .or_default();
            component_stats.components += 1;
            component_stats.chunks += component.chunk_count;
            component_stats.payload_bytes += component.payload_bytes;
            for chunk in &component.chunks {
                let chunk_path = stat_path_segment(&chunk.name);
                let chunk_stats = target_chunk_transfers
                    .entry((target.clone(), component_path.clone(), chunk_path))
                    .or_default();
                add_host_action_trace_chunk_stats(chunk_stats, chunk);
            }
        }

        let quiescence_target = stat_path_segment(&transfer.quiescence_gate.target);
        if transfer.quiescence_gate.validated {
            quiescence_validated += 1;
            *target_quiescence_validated
                .entry(quiescence_target.clone())
                .or_default() += 1;
        }
        if transfer.quiescence_gate.captured_component_count > 0
            || transfer.quiescence_gate.captured_chunk_count > 0
            || transfer.quiescence_gate.captured_payload_bytes > 0
        {
            quiescence_captured.components += transfer.quiescence_gate.captured_component_count;
            quiescence_captured.chunks += transfer.quiescence_gate.captured_chunk_count;
            quiescence_captured.payload_bytes += transfer.quiescence_gate.captured_payload_bytes;
            let captured_stats = target_quiescence_captured
                .entry(quiescence_target.clone())
                .or_default();
            captured_stats.components += transfer.quiescence_gate.captured_component_count;
            captured_stats.chunks += transfer.quiescence_gate.captured_chunk_count;
            captured_stats.payload_bytes += transfer.quiescence_gate.captured_payload_bytes;
        }
        let Some(checker) = transfer.quiescence_gate.checker else {
            continue;
        };
        latest_checker = Some(checker);
        target_checkers.insert(quiescence_target, checker);
    }

    let mut stats = Vec::new();
    for (target, transfer) in target_transfers {
        stats.push(Rem6HostActionTraceStat::count(
            format!("execution_mode_switch.state_transfer.target.{target}.components"),
            transfer.components,
        ));
        stats.push(Rem6HostActionTraceStat::count(
            format!("execution_mode_switch.state_transfer.target.{target}.chunks"),
            transfer.chunks,
        ));
        stats.push(Rem6HostActionTraceStat::byte(
            format!("execution_mode_switch.state_transfer.target.{target}.payload_bytes"),
            transfer.payload_bytes,
        ));
    }
    for ((target, component), transfer) in target_component_transfers {
        stats.push(Rem6HostActionTraceStat::count(
            format!(
                "execution_mode_switch.state_transfer.target.{target}.component.{component}.components"
            ),
            transfer.components,
        ));
        stats.push(Rem6HostActionTraceStat::count(
            format!(
                "execution_mode_switch.state_transfer.target.{target}.component.{component}.chunks"
            ),
            transfer.chunks,
        ));
        stats.push(Rem6HostActionTraceStat::byte(
            format!(
                "execution_mode_switch.state_transfer.target.{target}.component.{component}.payload_bytes"
            ),
            transfer.payload_bytes,
        ));
    }
    for ((target, component, chunk), transfer) in target_chunk_transfers {
        push_host_action_trace_chunk_stats(
            &mut stats,
            format!("execution_mode_switch.state_transfer.target.{target}.component.{component}.chunk.{chunk}"),
            transfer,
        );
    }
    stats.push(Rem6HostActionTraceStat::count(
        "execution_mode_switch.quiescence.validated".to_string(),
        quiescence_validated,
    ));
    stats.push(Rem6HostActionTraceStat::count(
        "execution_mode_switch.quiescence.captured_components".to_string(),
        quiescence_captured.components,
    ));
    stats.push(Rem6HostActionTraceStat::count(
        "execution_mode_switch.quiescence.captured_chunks".to_string(),
        quiescence_captured.chunks,
    ));
    stats.push(Rem6HostActionTraceStat::byte(
        "execution_mode_switch.quiescence.captured_payload_bytes".to_string(),
        quiescence_captured.payload_bytes,
    ));
    for (target, validated) in target_quiescence_validated {
        stats.push(Rem6HostActionTraceStat::count(
            format!("execution_mode_switch.quiescence.target.{target}.validated"),
            validated,
        ));
    }
    for (target, captured) in target_quiescence_captured {
        stats.push(Rem6HostActionTraceStat::count(
            format!("execution_mode_switch.quiescence.target.{target}.captured_components"),
            captured.components,
        ));
        stats.push(Rem6HostActionTraceStat::count(
            format!("execution_mode_switch.quiescence.target.{target}.captured_chunks"),
            captured.chunks,
        ));
        stats.push(Rem6HostActionTraceStat::byte(
            format!("execution_mode_switch.quiescence.target.{target}.captured_payload_bytes"),
            captured.payload_bytes,
        ));
    }
    for (target, checker) in target_checkers {
        stats.push(Rem6HostActionTraceStat::count(
            format!(
                "execution_mode_switch.quiescence.target.{target}.checker.checked_instructions"
            ),
            checker.checked_instructions,
        ));
        stats.push(Rem6HostActionTraceStat::count(
            format!("execution_mode_switch.quiescence.target.{target}.checker.mismatches"),
            checker.mismatches,
        ));
    }
    if let Some(checker) = latest_checker {
        stats.push(Rem6HostActionTraceStat::count(
            "execution_mode_switch.quiescence.checker.checked_instructions".to_string(),
            checker.checked_instructions,
        ));
        stats.push(Rem6HostActionTraceStat::count(
            "execution_mode_switch.quiescence.checker.mismatches".to_string(),
            checker.mismatches,
        ));
    }
    stats
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
        field_json(
            "components",
            checkpoint_components_to_json(&action.components),
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
        "{{\"captured\":true,\"manifest_label\":\"{}\",\"manifest_tick\":{},\"component_count\":{},\"chunk_count\":{},\"payload_bytes\":{},\"quiescence_gate\":{},\"components\":{}}}",
        json_escape(&transfer.manifest_label),
        transfer.manifest_tick,
        transfer.component_count,
        transfer.chunk_count,
        transfer.payload_bytes,
        quiescence_gate_json(&transfer.quiescence_gate),
        checkpoint_components_to_json(&transfer.components),
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
