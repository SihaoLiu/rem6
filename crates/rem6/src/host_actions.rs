use rem6_stats::{StatDumpRecord, StatsResetRecord};
use rem6_system::{
    ExecutionMode, ExecutionModeSwitchStateTransfer, ExecutionModeSwitchStateTransferComponent,
    SystemActionOutcome,
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
    pub(crate) checkpoint_restored_count: u64,
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
                SystemActionOutcome::StatsDump(record) => {
                    summary
                        .stats_dumps
                        .push(Rem6HostStatsDumpSummary::from_record(record));
                }
                SystemActionOutcome::Checkpoint {
                    tick,
                    event,
                    source,
                    manifest,
                } => {
                    let manifest_summary = manifest.summary();
                    let components = manifest
                        .states()
                        .iter()
                        .map(Rem6HostCheckpointComponentSummary::from_checkpoint_state)
                        .collect();
                    summary.checkpoints.push(Rem6HostCheckpointSummary {
                        tick: *tick,
                        event: event.get(),
                        source: source.get(),
                        label: manifest.label().to_string(),
                        manifest_tick: manifest.tick(),
                        component_count: manifest_summary.component_count() as u64,
                        chunk_count: manifest_summary.chunk_count() as u64,
                        payload_bytes: manifest_summary.payload_bytes() as u64,
                        components,
                    });
                }
                SystemActionOutcome::CheckpointRestored { .. } => {
                    summary.checkpoint_restored_count += 1;
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
pub(crate) struct Rem6ExecutionModeStateTransferSummary {
    pub(crate) manifest_label: String,
    pub(crate) manifest_tick: u64,
    pub(crate) component_count: u64,
    pub(crate) chunk_count: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) components: Vec<Rem6HostCheckpointComponentSummary>,
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
            components,
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
}

impl Rem6HostStatsDumpSummary {
    fn from_record(record: &StatDumpRecord) -> Self {
        Self {
            id: record.id().get(),
            tick: record.tick(),
            epoch: record.epoch(),
            reset_tick: record.reset_tick(),
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
}

fn payload_checksum(payload: &[u8]) -> u64 {
    payload.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStopActionSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) code: i32,
}
