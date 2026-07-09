use rem6_cpu::{BranchTargetKind, O3RuntimeCheckpointPayload};
use rem6_stats::{StatDumpRecord, StatSample, StatsResetRecord};
use rem6_system::{
    ExecutionMode, ExecutionModeSwitchCheckerGate, ExecutionModeSwitchQuiescenceGate,
    ExecutionModeSwitchStateTransfer, ExecutionModeSwitchStateTransferComponent,
    SystemActionOutcome, SystemHostController,
};

use crate::o3_branch_mismatch_aliases::{
    O3_BRANCH_MISMATCH_KIND_ALIASES, O3_BRANCH_MISMATCH_SCALAR_ALIASES,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6HostActionSummary {
    pub(crate) total_action_count: u64,
    pub(crate) injected_command_count: u64,
    pub(crate) injected_commands: Vec<Rem6HostInjectedCommandSummary>,
    pub(crate) guest_host_calls: Vec<Rem6GuestHostCallSummary>,
    pub(crate) roi_begin: Vec<Rem6HostWorkMarkerSummary>,
    pub(crate) roi_end: Vec<Rem6HostWorkMarkerSummary>,
    pub(crate) stats_resets: Vec<Rem6HostStatsResetSummary>,
    pub(crate) stats_dumps: Vec<Rem6HostStatsDumpSummary>,
    pub(crate) checkpoints: Vec<Rem6HostCheckpointSummary>,
    pub(crate) checkpoint_restores: Vec<Rem6HostCheckpointSummary>,
    pub(crate) checkpoint_restored_count: u64,
    pub(crate) checkpoint_restored_component_count: u64,
    pub(crate) checkpoint_restored_chunk_count: u64,
    pub(crate) checkpoint_restored_payload_bytes: u64,
    pub(crate) execution_modes: Vec<Rem6HostExecutionModeSummary>,
    pub(crate) execution_mode_switch_count: u64,
    pub(crate) execution_mode_switches: Vec<Rem6HostExecutionModeSwitchSummary>,
    pub(crate) stops: Vec<Rem6HostStopActionSummary>,
}

impl Rem6HostActionSummary {
    pub(crate) fn from_outcomes(outcomes: &[SystemActionOutcome]) -> Self {
        let mut summary = Self {
            total_action_count: outcomes.len() as u64,
            ..Self::default()
        };
        for outcome in outcomes {
            match outcome {
                SystemActionOutcome::InjectedCommand {
                    tick,
                    event,
                    source,
                    command,
                } => {
                    summary.injected_command_count += 1;
                    summary
                        .injected_commands
                        .push(Rem6HostInjectedCommandSummary {
                            tick: *tick,
                            event: event.get(),
                            source: source.get(),
                            command: command.clone(),
                        });
                }
                SystemActionOutcome::GuestHostCall {
                    tick,
                    event,
                    source,
                    selector,
                    arguments,
                    payload,
                    response,
                } => {
                    summary.guest_host_calls.push(Rem6GuestHostCallSummary {
                        tick: *tick,
                        event: event.get(),
                        source: source.get(),
                        selector: *selector,
                        arguments: arguments.clone(),
                        argument_count: arguments.len() as u64,
                        payload_bytes: payload.len() as u64,
                        response_status: response.status(),
                        response_return_count: response.return_values().len() as u64,
                        response_payload_bytes: response.payload().len() as u64,
                    });
                }
                SystemActionOutcome::RoiBegin {
                    tick,
                    event,
                    source,
                    work_id,
                    thread_id,
                } => {
                    summary.roi_begin.push(Rem6HostWorkMarkerSummary {
                        tick: *tick,
                        event: event.get(),
                        source: source.get(),
                        work_id: *work_id,
                        thread_id: *thread_id,
                    });
                }
                SystemActionOutcome::RoiEnd {
                    tick,
                    event,
                    source,
                    work_id,
                    thread_id,
                } => {
                    summary.roi_end.push(Rem6HostWorkMarkerSummary {
                        tick: *tick,
                        event: event.get(),
                        source: source.get(),
                        work_id: *work_id,
                        thread_id: *thread_id,
                    });
                }
                SystemActionOutcome::StatsReset(record) => {
                    summary
                        .stats_resets
                        .push(Rem6HostStatsResetSummary::from_record(record));
                }
                SystemActionOutcome::StatsDump {
                    record,
                    active_o3_cpus,
                } => {
                    summary
                        .stats_dumps
                        .push(Rem6HostStatsDumpSummary::from_record(
                            record,
                            active_o3_cpus,
                        ));
                }
                SystemActionOutcome::Checkpoint {
                    tick,
                    event,
                    source,
                    manifest,
                } => {
                    summary.checkpoints.push(checkpoint_summary_from_manifest(
                        *tick,
                        event.get(),
                        source.get(),
                        manifest,
                        false,
                    ));
                }
                SystemActionOutcome::CheckpointRestored {
                    tick,
                    event,
                    source,
                    manifest,
                } => {
                    let restored = checkpoint_summary_from_manifest(
                        *tick,
                        event.get(),
                        source.get(),
                        manifest,
                        true,
                    );
                    summary.checkpoint_restored_count += 1;
                    summary.checkpoint_restored_component_count += restored.component_count;
                    summary.checkpoint_restored_chunk_count += restored.chunk_count;
                    summary.checkpoint_restored_payload_bytes += restored.payload_bytes;
                    summary.checkpoint_restores.push(restored);
                }
                SystemActionOutcome::ExecutionModeSwitched {
                    tick,
                    event,
                    source,
                    target,
                    previous_mode,
                    mode,
                    stats_epoch,
                    stats_reset_tick,
                    state_transfer,
                } => {
                    summary.execution_mode_switch_count += 1;
                    summary
                        .execution_mode_switches
                        .push(Rem6HostExecutionModeSwitchSummary {
                            tick: *tick,
                            event: event.get(),
                            source: source.get(),
                            target: target.as_str().to_string(),
                            previous_mode: previous_mode.map(execution_mode_name),
                            mode: execution_mode_name(*mode),
                            stats_epoch: *stats_epoch,
                            stats_reset_tick: *stats_reset_tick,
                            state_transfer: state_transfer
                                .as_ref()
                                .map(Rem6ExecutionModeStateTransferSummary::from_transfer),
                        });
                }
                SystemActionOutcome::Stop(stop) => {
                    summary.stops.push(Rem6HostStopActionSummary {
                        tick: stop.tick(),
                        event: stop.event().get(),
                        source: stop.source().get(),
                        code: stop.code(),
                    });
                }
            }
        }
        summary
    }
}

fn checkpoint_summary_from_manifest(
    tick: u64,
    event: u64,
    source: u32,
    manifest: &rem6_checkpoint::CheckpointManifest,
    is_restore: bool,
) -> Rem6HostCheckpointSummary {
    let manifest_summary = manifest.summary();
    let (execution_mode_authority_present, execution_mode_authority_decode_error, execution_modes) =
        execution_mode_authority_from_manifest(manifest);
    let components = manifest
        .states()
        .iter()
        .map(Rem6HostCheckpointComponentSummary::from_checkpoint_state)
        .collect();
    Rem6HostCheckpointSummary {
        tick,
        event,
        source,
        label: manifest.label().to_string(),
        manifest_tick: manifest.tick(),
        component_count: manifest_summary.component_count() as u64,
        chunk_count: manifest_summary.chunk_count() as u64,
        payload_bytes: manifest_summary.payload_bytes() as u64,
        execution_mode_authority_present,
        execution_mode_authority_cleared: is_restore
            && !execution_mode_authority_present
            && !execution_mode_authority_decode_error,
        execution_mode_authority_decode_error,
        execution_modes,
        components,
    }
}

fn execution_mode_authority_from_manifest(
    manifest: &rem6_checkpoint::CheckpointManifest,
) -> (bool, bool, Vec<Rem6HostExecutionModeSummary>) {
    let Some(state) = manifest
        .states()
        .iter()
        .find(|state| state.component().as_str() == "host.execution_modes")
    else {
        return (false, false, Vec::new());
    };
    let Some(chunk) = state.chunks().iter().find(|chunk| chunk.name() == "modes") else {
        return (false, true, Vec::new());
    };
    match decode_execution_mode_authority(chunk.payload()) {
        Some(modes) => (true, false, modes),
        None => (false, true, Vec::new()),
    }
}

fn decode_execution_mode_authority(payload: &[u8]) -> Option<Vec<Rem6HostExecutionModeSummary>> {
    let mut cursor = 0;
    let count = read_checkpoint_u64(payload, &mut cursor)? as usize;
    let mut modes = Vec::new();
    for _ in 0..count {
        let target_len = read_checkpoint_u64(payload, &mut cursor)? as usize;
        let target_end = cursor.checked_add(target_len)?;
        let target = std::str::from_utf8(payload.get(cursor..target_end)?)
            .ok()?
            .to_string();
        cursor = target_end;
        let mode = execution_mode_name_from_code(*payload.get(cursor)?)?;
        cursor += 1;
        modes.push(Rem6HostExecutionModeSummary { target, mode });
    }
    (cursor == payload.len()).then_some(modes)
}

fn read_checkpoint_u64(payload: &[u8], cursor: &mut usize) -> Option<u64> {
    let end = cursor.checked_add(8)?;
    let bytes = payload.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(u64::from_le_bytes(bytes))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostExecutionModeSwitchSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) target: String,
    pub(crate) previous_mode: Option<&'static str>,
    pub(crate) mode: &'static str,
    pub(crate) stats_epoch: u64,
    pub(crate) stats_reset_tick: u64,
    pub(crate) state_transfer: Option<Rem6ExecutionModeStateTransferSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostExecutionModeSummary {
    pub(crate) target: String,
    pub(crate) mode: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6ExecutionModeStateTransferSummary {
    pub(crate) manifest_label: String,
    pub(crate) manifest_tick: u64,
    pub(crate) component_count: u64,
    pub(crate) chunk_count: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary,
    pub(crate) components: Vec<Rem6HostCheckpointComponentSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6ExecutionModeQuiescenceGateSummary {
    pub(crate) validated: bool,
    pub(crate) target: String,
    pub(crate) captured_component_count: u64,
    pub(crate) captured_chunk_count: u64,
    pub(crate) captured_payload_bytes: u64,
    pub(crate) checker: Option<Rem6ExecutionModeSwitchCheckerSummary>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6ExecutionModeSwitchCheckerSummary {
    pub(crate) checked_instructions: u64,
    pub(crate) mismatches: u64,
}

impl Rem6ExecutionModeStateTransferSummary {
    fn from_transfer(transfer: &ExecutionModeSwitchStateTransfer) -> Self {
        let components = transfer
            .components()
            .iter()
            .map(Rem6HostCheckpointComponentSummary::from_execution_mode_transfer_component)
            .collect();
        Self {
            manifest_label: transfer.manifest_label().to_string(),
            manifest_tick: transfer.manifest_tick(),
            component_count: transfer.component_count(),
            chunk_count: transfer.chunk_count(),
            payload_bytes: transfer.payload_bytes(),
            quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary::from_gate(
                transfer.quiescence_gate(),
            ),
            components,
        }
    }
}

impl Rem6ExecutionModeQuiescenceGateSummary {
    fn from_gate(gate: &ExecutionModeSwitchQuiescenceGate) -> Self {
        Self {
            validated: gate.validated(),
            target: gate.target().as_str().to_string(),
            captured_component_count: gate.captured_component_count(),
            captured_chunk_count: gate.captured_chunk_count(),
            captured_payload_bytes: gate.captured_payload_bytes(),
            checker: gate
                .checker()
                .map(Rem6ExecutionModeSwitchCheckerSummary::from_gate),
        }
    }
}

impl Rem6ExecutionModeSwitchCheckerSummary {
    fn from_gate(gate: ExecutionModeSwitchCheckerGate) -> Self {
        Self {
            checked_instructions: gate.checked_instructions(),
            mismatches: gate.mismatches(),
        }
    }
}

const fn execution_mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Functional => "functional",
        ExecutionMode::Timing => "timing",
        ExecutionMode::Detailed => "detailed",
    }
}

const fn execution_mode_name_from_code(code: u8) -> Option<&'static str> {
    match code {
        0 => Some("functional"),
        1 => Some("timing"),
        2 => Some("detailed"),
        _ => None,
    }
}

pub(crate) fn host_action_summary(
    controller: &SystemHostController,
) -> Result<Rem6HostActionSummary, String> {
    if let Some(error) = controller.action_errors().first() {
        return Err(format!("host action failed: {error}"));
    }
    let mut summary = Rem6HostActionSummary::from_outcomes(controller.run().action_outcomes());
    summary.execution_modes = controller
        .executor()
        .execution_modes()
        .iter()
        .map(|(target, mode)| Rem6HostExecutionModeSummary {
            target: target.as_str().to_string(),
            mode: execution_mode_name(*mode),
        })
        .collect();
    Ok(summary)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostInjectedCommandSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6GuestHostCallSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) selector: u64,
    pub(crate) arguments: Vec<u64>,
    pub(crate) argument_count: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) response_status: i32,
    pub(crate) response_return_count: u64,
    pub(crate) response_payload_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostWorkMarkerSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) work_id: u64,
    pub(crate) thread_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStatsResetSummary {
    pub(crate) id: u64,
    pub(crate) tick: u64,
    pub(crate) epoch: u64,
}

impl Rem6HostStatsResetSummary {
    fn from_record(record: &StatsResetRecord) -> Self {
        Self {
            id: record.id().get(),
            tick: record.tick(),
            epoch: record.epoch(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStatsDumpSummary {
    pub(crate) id: u64,
    pub(crate) tick: u64,
    pub(crate) epoch: u64,
    pub(crate) reset_tick: u64,
    pub(crate) samples: Vec<Rem6HostStatsDumpSampleSummary>,
}

impl Rem6HostStatsDumpSummary {
    fn from_record(record: &StatDumpRecord, active_o3_cpus: &[u32]) -> Self {
        let snapshot = record.snapshot();
        let mut samples: Vec<_> = snapshot
            .samples()
            .iter()
            .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
            .map(Rem6HostStatsDumpSampleSummary::from_sample)
            .collect();
        append_o3_stats_dump_rate_alias_samples(snapshot.samples(), active_o3_cpus, &mut samples);
        append_o3_stats_dump_phase_alias_samples(snapshot.samples(), active_o3_cpus, &mut samples);
        append_o3_stats_dump_iew_total_bucket_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_inst_type_bucket_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_lsq_count_bucket_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_branch_repair_bucket_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_branch_mismatch_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_ftq_bucket_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_frontend_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        append_o3_stats_dump_lsq_data_response_alias_samples(
            snapshot.samples(),
            active_o3_cpus,
            &mut samples,
        );
        Self {
            id: record.id().get(),
            tick: record.tick(),
            epoch: record.epoch(),
            reset_tick: record.reset_tick(),
            samples,
        }
    }
}

fn append_o3_stats_dump_rate_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_rate_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.iew.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_core_count(record_samples: &[StatSample], active_o3_cpus: &[u32]) -> u64 {
    record_samples
        .iter()
        .find(|sample| sample.path() == "sim.cores")
        .map(StatSample::value)
        .unwrap_or_else(|| {
            active_o3_cpus
                .iter()
                .copied()
                .max()
                .map_or(1, |cpu| u64::from(cpu) + 1)
        })
}

fn o3_stats_dump_alias_prefix(core_count: u64, cpu: u32) -> String {
    if core_count == 1 && cpu == 0 {
        "system.cpu".to_string()
    } else {
        format!("system.cpu{cpu}")
    }
}

fn o3_stats_dump_rate_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.iew.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;
    let suffix = match suffix {
        "writeback_rate_ppm" => "wbRate",
        "producer_consumer_fanout_ppm" => "wbFanout",
        _ => return None,
    };
    Some((cpu, suffix))
}

fn append_o3_stats_dump_phase_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_phase_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.iew.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_phase_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.event_summary.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;
    let suffix = match suffix {
        "issue_to_writeback_ticks" => "issueToWritebackTicks",
        "writeback_to_commit_ticks" => "writebackToCommitTicks",
        "issue_to_commit_ticks" => "issueToCommitTicks",
        _ => return None,
    };
    Some((cpu, suffix))
}

fn append_o3_stats_dump_iew_total_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_path) = o3_stats_dump_iew_total_bucket_alias_path(sample.path()) else {
            continue;
        };
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_iew_total_bucket_alias_path(path: &str) -> Option<String> {
    let (prefix, suffix) = path.split_once(".iew.")?;
    if !is_o3_stats_dump_cpu_alias_prefix(prefix) {
        return None;
    }
    match suffix {
        "instsToCommit.total"
        | "writebackCount.total"
        | "producerInst.total"
        | "consumerInst.total" => Some(format!(
            "{prefix}.iew.{}::total",
            suffix.strip_suffix(".total")?
        )),
        _ => None,
    }
}

fn append_o3_stats_dump_inst_type_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_path) = o3_stats_dump_inst_type_bucket_alias_path(sample.path()) else {
            continue;
        };
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_inst_type_bucket_alias_path(path: &str) -> Option<String> {
    if let Some((prefix, op_class)) = path.split_once(".iq.issuedInstType.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            return Some(format!("{prefix}.iq.issuedInstType_0::{op_class}"));
        }
    }
    if let Some((prefix, op_class)) = path.split_once(".commit.committedInstType.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            return Some(format!("{prefix}.commit.committedInstType_0::{op_class}"));
        }
    }
    None
}

fn is_o3_stats_dump_cpu_alias_prefix(prefix: &str) -> bool {
    if prefix == "system.cpu" {
        return true;
    }
    let Some(cpu) = prefix.strip_prefix("system.cpu") else {
        return false;
    };
    !cpu.is_empty() && cpu.bytes().all(|byte| byte.is_ascii_digit())
}

fn append_o3_stats_dump_lsq_count_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_path) = o3_stats_dump_lsq_count_bucket_alias_path(sample.path()) else {
            continue;
        };
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_lsq_count_bucket_alias_path(path: &str) -> Option<String> {
    if let Some((prefix, suffix)) = path.split_once(".lsq0.operation.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            let bucket = o3_stats_dump_lsq_operation_bucket_alias(suffix)?;
            return Some(format!("{prefix}.lsq0.operation_0::{bucket}"));
        }
    }
    if let Some((prefix, suffix)) = path.split_once(".lsq0.ordering.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            let bucket = o3_stats_dump_lsq_ordering_bucket_alias(suffix)?;
            return Some(format!("{prefix}.lsq0.ordering_0::{bucket}"));
        }
    }
    None
}

fn o3_stats_dump_lsq_operation_bucket_alias(suffix: &str) -> Option<&'static str> {
    match suffix {
        "load" => Some("Load"),
        "store" => Some("Store"),
        "loadReserved" => Some("LoadReserved"),
        "storeConditional" => Some("StoreConditional"),
        "atomic" => Some("Atomic"),
        "floatLoad" => Some("FloatLoad"),
        "floatStore" => Some("FloatStore"),
        "vectorLoad" => Some("VectorLoad"),
        "vectorStore" => Some("VectorStore"),
        "total" => Some("total"),
        _ => None,
    }
}

fn o3_stats_dump_lsq_ordering_bucket_alias(suffix: &str) -> Option<&'static str> {
    match suffix {
        "acquire" => Some("Acquire"),
        "release" => Some("Release"),
        "acquireRelease" => Some("AcquireRelease"),
        "total" => Some("total"),
        _ => None,
    }
}

fn append_o3_stats_dump_branch_repair_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_branch_repair_bucket_alias_suffix(sample.path())
        else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.iew.branchRepair_0::{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_branch_repair_bucket_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    if let Some(suffix) = path.strip_prefix("system.cpu.iew.branchRepair.") {
        return Some((0, branch_repair_bucket_alias_suffix(suffix)?));
    }
    let rest = path.strip_prefix("system.cpu")?;
    let (cpu, suffix) = rest.split_once(".iew.branchRepair.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((
        cpu.parse().ok()?,
        branch_repair_bucket_alias_suffix(suffix)?,
    ))
}

fn branch_repair_bucket_alias_suffix(suffix: &str) -> Option<&'static str> {
    match suffix {
        "targetlessMismatch" => Some("TargetlessMismatch"),
        "directionOnly" => Some("DirectionOnly"),
        "wrongTarget" => Some("WrongTarget"),
        "total" => Some("total"),
        _ => None,
    }
}

fn append_o3_stats_dump_branch_mismatch_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffixes)) = o3_stats_dump_branch_mismatch_alias_suffixes(sample.path())
        else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        for suffix in suffixes {
            let alias_path = format!("{alias_prefix}.iew.{suffix}");
            if samples.iter().any(|sample| sample.path == alias_path) {
                continue;
            }
            samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
                sample, alias_path,
            ));
        }
    }
}

fn o3_stats_dump_branch_mismatch_alias_suffixes(path: &str) -> Option<(u32, Vec<String>)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;

    for alias in O3_BRANCH_MISMATCH_SCALAR_ALIASES {
        if suffix == alias.source_suffix {
            return Some((
                cpu,
                vec![
                    alias.alias_suffix.to_string(),
                    format!("{}_0::total", alias.bucket_alias),
                ],
            ));
        }
    }

    for alias in O3_BRANCH_MISMATCH_KIND_ALIASES {
        let Some(kind_name) = suffix.strip_prefix(alias.source_family) else {
            continue;
        };
        let Some(kind_name) = kind_name.strip_prefix('.') else {
            continue;
        };
        for kind in BranchTargetKind::ALL {
            if kind.canonical_stat_name() == kind_name {
                return Some((
                    cpu,
                    vec![format!(
                        "{}_0::{}",
                        alias.alias_family,
                        kind.gem5_branch_type_name()
                    )],
                ));
            }
        }
    }
    None
}

fn append_o3_stats_dump_ftq_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_ftq_bucket_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.ftq.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_ftq_bucket_alias_suffix(path: &str) -> Option<(u32, String)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.branch_event.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;
    for (source_family, source_total, alias_family) in [
        ("squash_kind", "squashes", "squashes"),
        (
            "squashed_target_kind",
            "squashed_targets",
            "squashedTargets",
        ),
        (
            "squashed_target_link_write_kind",
            "squashed_targets_with_link_writes",
            "squashedTargetsWithLinkWrites",
        ),
        (
            "squashed_target_without_link_write_kind",
            "squashed_targets_without_link_writes",
            "squashedTargetsWithoutLinkWrites",
        ),
    ] {
        if suffix == source_total {
            return Some((cpu, format!("{alias_family}_0::total")));
        }
        let Some((family, kind_name)) = suffix.split_once('.') else {
            continue;
        };
        if family != source_family {
            continue;
        }
        for kind in BranchTargetKind::ALL {
            if kind.canonical_stat_name() == kind_name {
                return Some((
                    cpu,
                    format!("{alias_family}_0::{}", kind.gem5_branch_type_name()),
                ));
            }
        }
    }
    None
}

fn append_o3_stats_dump_frontend_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_frontend_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_frontend_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.branch_event.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let suffix = match suffix {
        "predicted_taken" => "fetch.predictedBranches",
        "mispredictions" => "bac.branchMisspredict",
        _ => return None,
    };
    Some((cpu.parse().ok()?, suffix))
}

fn append_o3_stats_dump_lsq_data_response_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_paths) =
            o3_stats_dump_lsq_data_response_alias_paths(core_count, sample.path())
        else {
            continue;
        };
        for alias_path in alias_paths {
            if samples.iter().any(|sample| sample.path == alias_path) {
                continue;
            }
            samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
                sample, alias_path,
            ));
        }
    }
}

fn o3_stats_dump_lsq_data_response_alias_paths(core_count: u64, path: &str) -> Option<Vec<String>> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu.parse().ok()?);
    if let Some(metric_suffix) = suffix.strip_prefix("lsq_data_latency_") {
        let metric = o3_stats_dump_lsq_data_response_metric_alias(metric_suffix)?;
        return Some(vec![
            format!("{alias_prefix}.lsq0.dataResponse.{metric}"),
            format!("{alias_prefix}.lsq0.operation.total.dataResponse.{metric}"),
        ]);
    }

    let suffix = suffix.strip_prefix("lsq_operation.")?;
    let (operation, metric_suffix) =
        o3_stats_dump_lsq_data_response_operation_metric_suffix(suffix)?;
    let operation_alias = o3_stats_dump_lsq_operation_alias(operation)?;
    let metric = o3_stats_dump_lsq_data_response_metric_alias(metric_suffix)?;
    Some(vec![
        format!("{alias_prefix}.lsq0.dataResponse.{operation_alias}.{metric}"),
        format!("{alias_prefix}.lsq0.operation.{operation_alias}.dataResponse.{metric}"),
    ])
}

fn o3_stats_dump_lsq_data_response_operation_metric_suffix(suffix: &str) -> Option<(&str, &str)> {
    if let Some((operation, metric_suffix)) = suffix.split_once("_latency_") {
        return Some((operation, metric_suffix));
    }
    let (operation, metric_suffix) = suffix.split_once(".latency.")?;
    Some((operation, metric_suffix))
}

fn o3_stats_dump_lsq_data_response_metric_alias(suffix: &str) -> Option<&'static str> {
    match suffix {
        "samples" => Some("samples"),
        "ticks" => Some("totalLatency"),
        "max_ticks" => Some("maxLatency"),
        "min_ticks" => Some("minLatency"),
        "avg_ticks" => Some("avgLatency"),
        _ => None,
    }
}

fn o3_stats_dump_lsq_operation_alias(operation: &str) -> Option<&'static str> {
    match operation {
        "load" => Some("load"),
        "store" => Some("store"),
        "load_reserved" => Some("loadReserved"),
        "store_conditional" => Some("storeConditional"),
        "atomic" => Some("atomic"),
        "float_load" => Some("floatLoad"),
        "float_store" => Some("floatStore"),
        "vector_load" => Some("vectorLoad"),
        "vector_store" => Some("vectorStore"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn lsq_data_response_dump_aliases_accept_nested_operation_latency_sources() {
        let aliases = o3_stats_dump_lsq_data_response_alias_paths(
            2,
            "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.vector_store.latency.avg_ticks",
        )
        .expect("nested vector-store latency source should produce gem5 aliases");

        assert_eq!(
            aliases,
            [
                "system.cpu1.lsq0.dataResponse.vectorStore.avgLatency",
                "system.cpu1.lsq0.operation.vectorStore.dataResponse.avgLatency",
            ]
        );
    }

    #[test]
    fn malformed_o3_runtime_checkpoint_chunks_report_decode_error() {
        let manifest = rem6_checkpoint::CheckpointManifest::new(
            "bad-o3",
            17,
            vec![rem6_checkpoint::CheckpointState::new(
                rem6_checkpoint::CheckpointComponentId::new("cpu0").unwrap(),
                vec![rem6_checkpoint::CheckpointChunk::new(
                    "o3-runtime-state",
                    b"not-o3-runtime".to_vec(),
                )],
            )],
        );

        let summary = checkpoint_summary_from_manifest(23, 29, 0, &manifest, false);
        let chunk = &summary.components[0].chunks[0];
        let o3_runtime = chunk
            .o3_runtime
            .as_ref()
            .expect("o3-runtime-state chunk should have O3 decode summary");
        assert!(o3_runtime.decode_error);
        assert_eq!(o3_runtime.snapshot_rob_entries, None);
        assert_eq!(o3_runtime.stats_max_rob_occupancy, None);

        let json: Value = serde_json::from_str(&summary.to_json()).unwrap();
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_runtime/decode_error")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_runtime/snapshot_rob_entries"),
            Some(&Value::Null)
        );
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_runtime/stats_max_rob_occupancy"),
            Some(&Value::Null)
        );
    }
}

fn stats_dump_sample_is_active(sample: &StatSample, active_o3_cpus: &[u32]) -> bool {
    let path = sample.path().to_string();
    if is_single_cpu_o3_alias_path(&path) {
        return !active_o3_cpus.is_empty();
    }
    let Some(cpu) = o3_stats_dump_sample_cpu(&path) else {
        return true;
    };
    active_o3_cpus.contains(&cpu)
}

fn is_single_cpu_o3_alias_path(path: &str) -> bool {
    [
        "system.cpu.rob.",
        "system.cpu.rename.",
        "system.cpu.iew.",
        "system.cpu.lsq0.",
        "system.cpu.iq.",
        "system.cpu.commit.",
        "system.cpu.ftq.",
    ]
    .into_iter()
    .any(|prefix| path.starts_with(prefix))
}

fn o3_stats_dump_sample_cpu(path: &str) -> Option<u32> {
    if let Some(rest) = path.strip_prefix("sim.host_actions.stats_dump.cpu") {
        return parse_o3_stats_dump_cpu(rest, ".o3.");
    }
    let rest = path.strip_prefix("system.cpu")?;
    [
        ".rob.", ".rename.", ".iew.", ".lsq0.", ".iq.", ".commit.", ".ftq.",
    ]
    .into_iter()
    .find_map(|separator| parse_o3_stats_dump_cpu(rest, separator))
}

fn parse_o3_stats_dump_cpu(rest: &str, separator: &str) -> Option<u32> {
    let (cpu, _suffix) = rest.split_once(separator)?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    cpu.parse().ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStatsDumpSampleSummary {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) unit: String,
    pub(crate) value: u64,
    pub(crate) reset_policy: String,
}

impl Rem6HostStatsDumpSampleSummary {
    fn from_sample(sample: &StatSample) -> Self {
        Self::from_sample_with_path(sample, sample.path().to_string())
    }

    fn from_sample_with_path(sample: &StatSample, path: String) -> Self {
        Self {
            path,
            kind: sample.kind().to_string(),
            unit: sample.unit().to_string(),
            value: sample.value(),
            reset_policy: sample.reset_policy().to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostCheckpointSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) label: String,
    pub(crate) manifest_tick: u64,
    pub(crate) component_count: u64,
    pub(crate) chunk_count: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) execution_mode_authority_present: bool,
    pub(crate) execution_mode_authority_cleared: bool,
    pub(crate) execution_mode_authority_decode_error: bool,
    pub(crate) execution_modes: Vec<Rem6HostExecutionModeSummary>,
    pub(crate) components: Vec<Rem6HostCheckpointComponentSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostCheckpointComponentSummary {
    pub(crate) component: String,
    pub(crate) chunk_count: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) chunks: Vec<Rem6HostCheckpointChunkSummary>,
}

impl Rem6HostCheckpointComponentSummary {
    fn from_checkpoint_state(state: &rem6_checkpoint::CheckpointState) -> Self {
        let mut chunks = state
            .chunks()
            .iter()
            .map(|chunk| Rem6HostCheckpointChunkSummary {
                name: chunk.name().to_string(),
                payload_bytes: chunk.payload().len() as u64,
                payload_checksum: payload_checksum(chunk.payload()),
                o3_runtime: decode_o3_runtime_checkpoint_chunk(chunk.name(), chunk.payload()),
            })
            .collect::<Vec<_>>();
        chunks.sort_by(|left, right| left.name.cmp(&right.name));
        let payload_bytes = chunks.iter().map(|chunk| chunk.payload_bytes).sum();
        Self {
            component: state.component().as_str().to_string(),
            chunk_count: chunks.len() as u64,
            payload_bytes,
            chunks,
        }
    }

    fn from_execution_mode_transfer_component(
        component: &ExecutionModeSwitchStateTransferComponent,
    ) -> Self {
        Self {
            component: component.component().to_string(),
            chunk_count: component.chunk_count(),
            payload_bytes: component.payload_bytes(),
            chunks: component
                .chunks()
                .iter()
                .map(|chunk| Rem6HostCheckpointChunkSummary {
                    name: chunk.name().to_string(),
                    payload_bytes: chunk.payload_bytes(),
                    payload_checksum: chunk.payload_checksum(),
                    o3_runtime: None,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostCheckpointChunkSummary {
    pub(crate) name: String,
    pub(crate) payload_bytes: u64,
    pub(crate) payload_checksum: u64,
    pub(crate) o3_runtime: Option<Rem6HostO3RuntimeCheckpointChunkSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostO3RuntimeCheckpointChunkSummary {
    pub(crate) decode_error: bool,
    pub(crate) snapshot_rob_entries: Option<u64>,
    pub(crate) snapshot_lsq_entries: Option<u64>,
    pub(crate) snapshot_rename_map_entries: Option<u64>,
    pub(crate) stats_max_rob_occupancy: Option<u64>,
    pub(crate) stats_max_lsq_occupancy: Option<u64>,
    pub(crate) stats_rename_map_entries: Option<u64>,
}

impl Rem6HostO3RuntimeCheckpointChunkSummary {
    fn decode_error() -> Self {
        Self {
            decode_error: true,
            snapshot_rob_entries: None,
            snapshot_lsq_entries: None,
            snapshot_rename_map_entries: None,
            stats_max_rob_occupancy: None,
            stats_max_lsq_occupancy: None,
            stats_rename_map_entries: None,
        }
    }
}

fn payload_checksum(payload: &[u8]) -> u64 {
    payload.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn decode_o3_runtime_checkpoint_chunk(
    name: &str,
    payload: &[u8],
) -> Option<Rem6HostO3RuntimeCheckpointChunkSummary> {
    if name != "o3-runtime-state" {
        return None;
    }
    let Ok(decoded) = O3RuntimeCheckpointPayload::decode(payload) else {
        return Some(Rem6HostO3RuntimeCheckpointChunkSummary::decode_error());
    };
    let snapshot = decoded.snapshot();
    let stats = decoded.stats();
    Some(Rem6HostO3RuntimeCheckpointChunkSummary {
        decode_error: false,
        snapshot_rob_entries: Some(snapshot.reorder_buffer().len() as u64),
        snapshot_lsq_entries: Some(snapshot.load_store_queue().len() as u64),
        snapshot_rename_map_entries: Some(snapshot.rename_map().len() as u64),
        stats_max_rob_occupancy: Some(stats.max_rob_occupancy()),
        stats_max_lsq_occupancy: Some(stats.max_lsq_occupancy()),
        stats_rename_map_entries: Some(stats.rename_map_entries()),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStopActionSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) code: i32,
}
