mod live_data_handoff;
mod o3_stats_dump_aliases;
pub(crate) mod transfer_stats;

use rem6_cpu::{O3RuntimeCheckpointPayload, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation};
use rem6_stats::{StatDumpRecord, StatSample, StatsResetRecord};
use rem6_system::{
    decode_execution_mode_authority_from_manifest, ExecutionMode, ExecutionModeSwitchCheckerGate,
    ExecutionModeSwitchQuiescenceGate, ExecutionModeSwitchStateTransfer,
    ExecutionModeSwitchStateTransferComponent, SystemActionOutcome, SystemHostController,
    RISCV_O3_RUNTIME_STATE_CHUNK,
};

use self::o3_stats_dump_aliases::samples_with_gem5_aliases;
use live_data_handoff::decode_o3_live_data_handoff_chunk;
pub(crate) use live_data_handoff::Rem6HostO3LiveDataHandoffChunkSummary;

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
    match decode_execution_mode_authority_from_manifest(manifest) {
        Ok(Some(modes)) => (
            true,
            false,
            modes
                .into_iter()
                .map(|(target, mode)| Rem6HostExecutionModeSummary {
                    target: target.as_str().to_string(),
                    mode: execution_mode_name(mode),
                })
                .collect(),
        ),
        Ok(None) => (false, false, Vec::new()),
        Err(_) => (false, true, Vec::new()),
    }
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
    pub(crate) restorable: bool,
    pub(crate) live_data_handoff: bool,
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
            restorable: transfer.restorable(),
            live_data_handoff: transfer.live_data_handoff(),
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
        let samples = samples_with_gem5_aliases(snapshot.samples(), active_o3_cpus);
        Self {
            id: record.id().get(),
            tick: record.tick(),
            epoch: record.epoch(),
            reset_tick: record.reset_tick(),
            samples,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;
    use serde_json::Value;

    #[test]
    fn duplicate_execution_mode_targets_are_not_published() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&2_u64.to_le_bytes());
        for mode in [2_u8, 1_u8] {
            payload.extend_from_slice(&4_u64.to_le_bytes());
            payload.extend_from_slice(b"cpu0");
            payload.push(mode);
        }
        let manifest = rem6_checkpoint::CheckpointManifest::new(
            "duplicate-execution-mode-authority",
            17,
            vec![rem6_checkpoint::CheckpointState::new(
                rem6_checkpoint::CheckpointComponentId::new("host.execution_modes").unwrap(),
                vec![rem6_checkpoint::CheckpointChunk::new("modes", payload)],
            )],
        );

        let summary = checkpoint_summary_from_manifest(23, 29, 0, &manifest, true);

        assert!(!summary.execution_mode_authority_present);
        assert!(summary.execution_mode_authority_decode_error);
        assert!(summary.execution_modes.is_empty());
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
        assert_eq!(o3_runtime.live_retire_gate_request_agent, None);
        assert_eq!(o3_runtime.live_retire_gate_request_sequence, None);
        assert_eq!(o3_runtime.live_retire_gate_ready_tick, None);
        assert_eq!(o3_runtime.snapshot_rob_entries, None);
        assert_eq!(o3_runtime.stats_max_rob_occupancy, None);

        let json: Value = serde_json::from_str(&summary.to_json()).unwrap();
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_runtime/decode_error")
                .and_then(Value::as_bool),
            Some(true)
        );
        for field in [
            "live_retire_gate_request_agent",
            "live_retire_gate_request_sequence",
            "live_retire_gate_ready_tick",
        ] {
            assert_eq!(
                json.pointer(&format!("/components/0/chunks/0/o3_runtime/{field}")),
                Some(&Value::Null)
            );
        }
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_runtime/snapshot_rob_entries"),
            Some(&Value::Null)
        );
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_runtime/stats_max_rob_occupancy"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn malformed_live_data_handoff_chunks_report_decode_error() {
        let manifest = rem6_checkpoint::CheckpointManifest::new(
            "bad-live-data-handoff",
            19,
            vec![rem6_checkpoint::CheckpointState::new(
                rem6_checkpoint::CheckpointComponentId::new("cpu0").unwrap(),
                vec![rem6_checkpoint::CheckpointChunk::new(
                    RISCV_O3_LIVE_DATA_HANDOFF_CHUNK,
                    b"not-live-data-handoff".to_vec(),
                )],
            )],
        );

        let summary = checkpoint_summary_from_manifest(23, 29, 0, &manifest, false);
        let handoff = summary.components[0].chunks[0]
            .o3_live_data_handoff
            .as_ref()
            .expect("handoff chunk should expose a decode summary");
        assert!(handoff.decode_error);
        assert_eq!(handoff.row_count, None);
        assert_eq!(handoff.first_data_request_sequence, None);

        let json: Value = serde_json::from_str(&summary.to_json()).unwrap();
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_live_data_handoff/decode_error")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_live_data_handoff/outstanding_requests"),
            Some(&Value::Null)
        );
        assert_eq!(
            json.pointer("/components/0/chunks/0/o3_live_data_handoff/first_target"),
            Some(&Value::Null)
        );
    }
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
                o3_live_data_handoff: decode_o3_live_data_handoff_chunk(
                    chunk.name(),
                    chunk.payload(),
                ),
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
                    o3_runtime: chunk.o3_runtime_payload().and_then(|payload| {
                        decode_o3_runtime_checkpoint_chunk(chunk.name(), payload)
                    }),
                    o3_live_data_handoff: chunk.o3_live_data_handoff_payload().and_then(
                        |payload| decode_o3_live_data_handoff_chunk(chunk.name(), payload),
                    ),
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
    pub(crate) o3_live_data_handoff: Option<Rem6HostO3LiveDataHandoffChunkSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostO3RuntimeCheckpointChunkSummary {
    pub(crate) decode_error: bool,
    pub(crate) live_retire_gate_request_agent: Option<u64>,
    pub(crate) live_retire_gate_request_sequence: Option<u64>,
    pub(crate) live_retire_gate_ready_tick: Option<u64>,
    pub(crate) snapshot_rob_entries: Option<u64>,
    pub(crate) snapshot_lsq_entries: Option<u64>,
    pub(crate) snapshot_rename_map_entries: Option<u64>,
    pub(crate) stats_max_rob_occupancy: Option<u64>,
    pub(crate) stats_max_lsq_occupancy: Option<u64>,
    pub(crate) stats_rename_map_entries: Option<u64>,
    pub(crate) stats_lsq_operation_load: Option<u64>,
    pub(crate) stats_lsq_operation_store: Option<u64>,
    pub(crate) stats_lsq_data_latency_samples: Option<u64>,
    pub(crate) stats_lsq_data_latency_ticks: Option<u64>,
    pub(crate) stats_lsq_data_latency_max_ticks: Option<u64>,
    pub(crate) stats_lsq_data_latency_min_ticks: Option<u64>,
    pub(crate) stats_lsq_data_latency_avg_ticks: Option<u64>,
    pub(crate) stats_lsq_operation_load_latency_samples: Option<u64>,
    pub(crate) stats_lsq_operation_load_latency_ticks: Option<u64>,
    pub(crate) stats_lsq_operation_store_latency_samples: Option<u64>,
    pub(crate) stats_lsq_operation_store_latency_ticks: Option<u64>,
    pub(crate) stats_fu_latency_instructions: Option<u64>,
    pub(crate) stats_fu_latency_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_mul_instructions: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_mul_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_mul_max_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_mul_min_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_mul_avg_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_div_instructions: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_div_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_div_max_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_div_min_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_integer_div_avg_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_float_misc_instructions: Option<u64>,
    pub(crate) stats_fu_latency_class_float_misc_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_float_misc_max_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_float_misc_min_cycles: Option<u64>,
    pub(crate) stats_fu_latency_class_float_misc_avg_cycles: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Rem6HostO3RuntimeCheckpointStatAggregation {
    Sum,
    Max,
    MinNonZero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostO3RuntimeCheckpointStatValue {
    aggregation: Rem6HostO3RuntimeCheckpointStatAggregation,
    unit: &'static str,
    value: u64,
}

impl Rem6HostO3RuntimeCheckpointStatValue {
    fn new(
        name: &str,
        aggregation: Rem6HostO3RuntimeCheckpointStatAggregation,
        value: u64,
    ) -> Self {
        Self {
            aggregation,
            unit: o3_runtime_checkpoint_unit(name),
            value,
        }
    }

    pub(crate) const fn unit(self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(self) -> u64 {
        self.value
    }

    pub(crate) fn merge_restore_value(&mut self, other: Self) {
        match self.aggregation {
            Rem6HostO3RuntimeCheckpointStatAggregation::Sum => {
                self.value = self.value.saturating_add(other.value);
            }
            Rem6HostO3RuntimeCheckpointStatAggregation::Max => {
                self.value = self.value.max(other.value);
            }
            Rem6HostO3RuntimeCheckpointStatAggregation::MinNonZero => {
                self.value = min_nonzero(self.value, other.value);
            }
        }
    }

    pub(crate) fn merge_trace_duplicate(&mut self, other: Self) {
        match self.aggregation {
            Rem6HostO3RuntimeCheckpointStatAggregation::Sum
            | Rem6HostO3RuntimeCheckpointStatAggregation::Max => {
                self.value = self.value.max(other.value);
            }
            Rem6HostO3RuntimeCheckpointStatAggregation::MinNonZero => {
                self.value = min_nonzero(self.value, other.value);
            }
        }
    }
}

impl Rem6HostO3RuntimeCheckpointChunkSummary {
    fn decode_error() -> Self {
        Self {
            decode_error: true,
            live_retire_gate_request_agent: None,
            live_retire_gate_request_sequence: None,
            live_retire_gate_ready_tick: None,
            snapshot_rob_entries: None,
            snapshot_lsq_entries: None,
            snapshot_rename_map_entries: None,
            stats_max_rob_occupancy: None,
            stats_max_lsq_occupancy: None,
            stats_rename_map_entries: None,
            stats_lsq_operation_load: None,
            stats_lsq_operation_store: None,
            stats_lsq_data_latency_samples: None,
            stats_lsq_data_latency_ticks: None,
            stats_lsq_data_latency_max_ticks: None,
            stats_lsq_data_latency_min_ticks: None,
            stats_lsq_data_latency_avg_ticks: None,
            stats_lsq_operation_load_latency_samples: None,
            stats_lsq_operation_load_latency_ticks: None,
            stats_lsq_operation_store_latency_samples: None,
            stats_lsq_operation_store_latency_ticks: None,
            stats_fu_latency_instructions: None,
            stats_fu_latency_cycles: None,
            stats_fu_latency_class_integer_mul_instructions: None,
            stats_fu_latency_class_integer_mul_cycles: None,
            stats_fu_latency_class_integer_mul_max_cycles: None,
            stats_fu_latency_class_integer_mul_min_cycles: None,
            stats_fu_latency_class_integer_mul_avg_cycles: None,
            stats_fu_latency_class_integer_div_instructions: None,
            stats_fu_latency_class_integer_div_cycles: None,
            stats_fu_latency_class_integer_div_max_cycles: None,
            stats_fu_latency_class_integer_div_min_cycles: None,
            stats_fu_latency_class_integer_div_avg_cycles: None,
            stats_fu_latency_class_float_misc_instructions: None,
            stats_fu_latency_class_float_misc_cycles: None,
            stats_fu_latency_class_float_misc_max_cycles: None,
            stats_fu_latency_class_float_misc_min_cycles: None,
            stats_fu_latency_class_float_misc_avg_cycles: None,
        }
    }

    pub(crate) fn numeric_fields(&self) -> Vec<(&'static str, Option<u64>)> {
        vec![
            (
                "live_retire_gate_request_agent",
                self.live_retire_gate_request_agent,
            ),
            (
                "live_retire_gate_request_sequence",
                self.live_retire_gate_request_sequence,
            ),
            (
                "live_retire_gate_ready_tick",
                self.live_retire_gate_ready_tick,
            ),
            ("snapshot_rob_entries", self.snapshot_rob_entries),
            ("snapshot_lsq_entries", self.snapshot_lsq_entries),
            (
                "snapshot_rename_map_entries",
                self.snapshot_rename_map_entries,
            ),
            ("stats_max_rob_occupancy", self.stats_max_rob_occupancy),
            ("stats_max_lsq_occupancy", self.stats_max_lsq_occupancy),
            ("stats_rename_map_entries", self.stats_rename_map_entries),
            ("stats_lsq_operation_load", self.stats_lsq_operation_load),
            ("stats_lsq_operation_store", self.stats_lsq_operation_store),
            (
                "stats_lsq_data_latency_samples",
                self.stats_lsq_data_latency_samples,
            ),
            (
                "stats_lsq_data_latency_ticks",
                self.stats_lsq_data_latency_ticks,
            ),
            (
                "stats_lsq_data_latency_max_ticks",
                self.stats_lsq_data_latency_max_ticks,
            ),
            (
                "stats_lsq_data_latency_min_ticks",
                self.stats_lsq_data_latency_min_ticks,
            ),
            (
                "stats_lsq_data_latency_avg_ticks",
                self.stats_lsq_data_latency_avg_ticks,
            ),
            (
                "stats_lsq_operation_load_latency_samples",
                self.stats_lsq_operation_load_latency_samples,
            ),
            (
                "stats_lsq_operation_load_latency_ticks",
                self.stats_lsq_operation_load_latency_ticks,
            ),
            (
                "stats_lsq_operation_store_latency_samples",
                self.stats_lsq_operation_store_latency_samples,
            ),
            (
                "stats_lsq_operation_store_latency_ticks",
                self.stats_lsq_operation_store_latency_ticks,
            ),
            (
                "stats_fu_latency_instructions",
                self.stats_fu_latency_instructions,
            ),
            ("stats_fu_latency_cycles", self.stats_fu_latency_cycles),
            (
                "stats_fu_latency_class_integer_mul_instructions",
                self.stats_fu_latency_class_integer_mul_instructions,
            ),
            (
                "stats_fu_latency_class_integer_mul_cycles",
                self.stats_fu_latency_class_integer_mul_cycles,
            ),
            (
                "stats_fu_latency_class_integer_mul_max_cycles",
                self.stats_fu_latency_class_integer_mul_max_cycles,
            ),
            (
                "stats_fu_latency_class_integer_mul_min_cycles",
                self.stats_fu_latency_class_integer_mul_min_cycles,
            ),
            (
                "stats_fu_latency_class_integer_mul_avg_cycles",
                self.stats_fu_latency_class_integer_mul_avg_cycles,
            ),
            (
                "stats_fu_latency_class_integer_div_instructions",
                self.stats_fu_latency_class_integer_div_instructions,
            ),
            (
                "stats_fu_latency_class_integer_div_cycles",
                self.stats_fu_latency_class_integer_div_cycles,
            ),
            (
                "stats_fu_latency_class_integer_div_max_cycles",
                self.stats_fu_latency_class_integer_div_max_cycles,
            ),
            (
                "stats_fu_latency_class_integer_div_min_cycles",
                self.stats_fu_latency_class_integer_div_min_cycles,
            ),
            (
                "stats_fu_latency_class_integer_div_avg_cycles",
                self.stats_fu_latency_class_integer_div_avg_cycles,
            ),
            (
                "stats_fu_latency_class_float_misc_instructions",
                self.stats_fu_latency_class_float_misc_instructions,
            ),
            (
                "stats_fu_latency_class_float_misc_cycles",
                self.stats_fu_latency_class_float_misc_cycles,
            ),
            (
                "stats_fu_latency_class_float_misc_max_cycles",
                self.stats_fu_latency_class_float_misc_max_cycles,
            ),
            (
                "stats_fu_latency_class_float_misc_min_cycles",
                self.stats_fu_latency_class_float_misc_min_cycles,
            ),
            (
                "stats_fu_latency_class_float_misc_avg_cycles",
                self.stats_fu_latency_class_float_misc_avg_cycles,
            ),
        ]
    }

    pub(crate) fn numeric_stat_fields(
        &self,
    ) -> Vec<(&'static str, Rem6HostO3RuntimeCheckpointStatValue)> {
        self.numeric_fields()
            .into_iter()
            .filter_map(|(name, value)| {
                let aggregation = o3_runtime_checkpoint_stat_aggregation(name)?;
                value.map(|value| {
                    (
                        name,
                        Rem6HostO3RuntimeCheckpointStatValue::new(name, aggregation, value),
                    )
                })
            })
            .collect()
    }
}

fn o3_runtime_checkpoint_stat_aggregation(
    name: &str,
) -> Option<Rem6HostO3RuntimeCheckpointStatAggregation> {
    if matches!(
        name,
        "stats_lsq_operation_load"
            | "stats_lsq_operation_store"
            | "stats_lsq_data_latency_samples"
            | "stats_lsq_data_latency_ticks"
            | "stats_lsq_operation_load_latency_samples"
            | "stats_lsq_operation_load_latency_ticks"
            | "stats_lsq_operation_store_latency_samples"
            | "stats_lsq_operation_store_latency_ticks"
            | "stats_fu_latency_instructions"
            | "stats_fu_latency_cycles"
            | "stats_fu_latency_class_integer_mul_instructions"
            | "stats_fu_latency_class_integer_mul_cycles"
            | "stats_fu_latency_class_integer_div_instructions"
            | "stats_fu_latency_class_integer_div_cycles"
            | "stats_fu_latency_class_float_misc_instructions"
            | "stats_fu_latency_class_float_misc_cycles"
    ) {
        Some(Rem6HostO3RuntimeCheckpointStatAggregation::Sum)
    } else if name.starts_with("stats_max_")
        || name.ends_with("_max_ticks")
        || name.ends_with("_max_cycles")
    {
        Some(Rem6HostO3RuntimeCheckpointStatAggregation::Max)
    } else if name.ends_with("_min_ticks") || name.ends_with("_min_cycles") {
        Some(Rem6HostO3RuntimeCheckpointStatAggregation::MinNonZero)
    } else {
        None
    }
}

fn o3_runtime_checkpoint_unit(name: &str) -> &'static str {
    if name.ends_with("_cycles") {
        "Cycle"
    } else if name.ends_with("_ticks") {
        "Tick"
    } else {
        "Count"
    }
}

const fn min_nonzero(left: u64, right: u64) -> u64 {
    if left == 0 {
        right
    } else if right == 0 {
        left
    } else if left < right {
        left
    } else {
        right
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
    if name != RISCV_O3_RUNTIME_STATE_CHUNK {
        return None;
    }
    let Ok(decoded) = O3RuntimeCheckpointPayload::decode(payload) else {
        return Some(Rem6HostO3RuntimeCheckpointChunkSummary::decode_error());
    };
    let snapshot = decoded.snapshot();
    let stats = decoded.stats();
    let pending_live_retire_gate = decoded.pending_live_retire_gate();
    let live_retire_gate_request = pending_live_retire_gate.map(|(request, _)| request);
    let integer_mul = O3RuntimeFuLatencyClass::ScalarIntegerMul;
    let integer_div = O3RuntimeFuLatencyClass::ScalarIntegerDiv;
    let float_misc = O3RuntimeFuLatencyClass::ScalarFloatMisc;
    Some(Rem6HostO3RuntimeCheckpointChunkSummary {
        decode_error: false,
        live_retire_gate_request_agent: live_retire_gate_request
            .map(|request| u64::from(request.agent().get())),
        live_retire_gate_request_sequence: live_retire_gate_request
            .map(|request| request.sequence()),
        live_retire_gate_ready_tick: pending_live_retire_gate.map(|(_, ready_tick)| ready_tick),
        snapshot_rob_entries: Some(snapshot.reorder_buffer().len() as u64),
        snapshot_lsq_entries: Some(snapshot.load_store_queue().len() as u64),
        snapshot_rename_map_entries: Some(snapshot.rename_map().len() as u64),
        stats_max_rob_occupancy: Some(stats.max_rob_occupancy()),
        stats_max_lsq_occupancy: Some(stats.max_lsq_occupancy()),
        stats_rename_map_entries: Some(stats.rename_map_entries()),
        stats_lsq_operation_load: Some(stats.lsq_operation_count(O3RuntimeLsqOperation::Load)),
        stats_lsq_operation_store: Some(stats.lsq_operation_count(O3RuntimeLsqOperation::Store)),
        stats_lsq_data_latency_samples: Some(stats.lsq_data_latency_samples()),
        stats_lsq_data_latency_ticks: Some(stats.lsq_data_latency_ticks()),
        stats_lsq_data_latency_max_ticks: Some(stats.lsq_data_latency_max_ticks()),
        stats_lsq_data_latency_min_ticks: Some(stats.lsq_data_latency_min_ticks()),
        stats_lsq_data_latency_avg_ticks: Some(stats.lsq_data_latency_avg_ticks()),
        stats_lsq_operation_load_latency_samples: Some(
            stats.lsq_operation_latency_samples(O3RuntimeLsqOperation::Load),
        ),
        stats_lsq_operation_load_latency_ticks: Some(
            stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::Load),
        ),
        stats_lsq_operation_store_latency_samples: Some(
            stats.lsq_operation_latency_samples(O3RuntimeLsqOperation::Store),
        ),
        stats_lsq_operation_store_latency_ticks: Some(
            stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::Store),
        ),
        stats_fu_latency_instructions: Some(stats.fu_latency_instructions()),
        stats_fu_latency_cycles: Some(stats.fu_latency_cycles()),
        stats_fu_latency_class_integer_mul_instructions: Some(
            stats.fu_latency_class_instructions(integer_mul),
        ),
        stats_fu_latency_class_integer_mul_cycles: Some(stats.fu_latency_class_cycles(integer_mul)),
        stats_fu_latency_class_integer_mul_max_cycles: Some(
            stats.fu_latency_class_max_cycles(integer_mul),
        ),
        stats_fu_latency_class_integer_mul_min_cycles: Some(
            stats.fu_latency_class_min_cycles(integer_mul),
        ),
        stats_fu_latency_class_integer_mul_avg_cycles: Some(
            stats.fu_latency_class_avg_cycles(integer_mul),
        ),
        stats_fu_latency_class_integer_div_instructions: Some(
            stats.fu_latency_class_instructions(integer_div),
        ),
        stats_fu_latency_class_integer_div_cycles: Some(stats.fu_latency_class_cycles(integer_div)),
        stats_fu_latency_class_integer_div_max_cycles: Some(
            stats.fu_latency_class_max_cycles(integer_div),
        ),
        stats_fu_latency_class_integer_div_min_cycles: Some(
            stats.fu_latency_class_min_cycles(integer_div),
        ),
        stats_fu_latency_class_integer_div_avg_cycles: Some(
            stats.fu_latency_class_avg_cycles(integer_div),
        ),
        stats_fu_latency_class_float_misc_instructions: Some(
            stats.fu_latency_class_instructions(float_misc),
        ),
        stats_fu_latency_class_float_misc_cycles: Some(stats.fu_latency_class_cycles(float_misc)),
        stats_fu_latency_class_float_misc_max_cycles: Some(
            stats.fu_latency_class_max_cycles(float_misc),
        ),
        stats_fu_latency_class_float_misc_min_cycles: Some(
            stats.fu_latency_class_min_cycles(float_misc),
        ),
        stats_fu_latency_class_float_misc_avg_cycles: Some(
            stats.fu_latency_class_avg_cycles(float_misc),
        ),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStopActionSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) code: i32,
}
