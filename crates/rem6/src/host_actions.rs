use rem6_stats::{StatDumpRecord, StatsResetRecord};
use rem6_system::SystemActionOutcome;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6HostActionSummary {
    pub(crate) total_action_count: u64,
    pub(crate) injected_command_count: u64,
    pub(crate) guest_host_calls: Vec<Rem6GuestHostCallSummary>,
    pub(crate) roi_begin: Vec<Rem6HostWorkMarkerSummary>,
    pub(crate) roi_end: Vec<Rem6HostWorkMarkerSummary>,
    pub(crate) stats_resets: Vec<Rem6HostStatsResetSummary>,
    pub(crate) stats_dumps: Vec<Rem6HostStatsDumpSummary>,
    pub(crate) checkpoints: Vec<Rem6HostCheckpointSummary>,
    pub(crate) checkpoint_restored_count: u64,
    pub(crate) execution_mode_switch_count: u64,
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
                SystemActionOutcome::InjectedCommand { .. } => {
                    summary.injected_command_count += 1;
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
                    summary.checkpoints.push(Rem6HostCheckpointSummary {
                        tick: *tick,
                        event: event.get(),
                        source: source.get(),
                        label: manifest.label().to_string(),
                        manifest_tick: manifest.tick(),
                        component_count: manifest_summary.component_count() as u64,
                        chunk_count: manifest_summary.chunk_count() as u64,
                        payload_bytes: manifest_summary.payload_bytes() as u64,
                    });
                }
                SystemActionOutcome::CheckpointRestored { .. } => {
                    summary.checkpoint_restored_count += 1;
                }
                SystemActionOutcome::ExecutionModeSwitched { .. } => {
                    summary.execution_mode_switch_count += 1;
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
pub(crate) struct Rem6GuestHostCallSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) selector: u64,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStopActionSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) code: i32,
}
