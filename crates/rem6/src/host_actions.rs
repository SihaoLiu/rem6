use rem6_system::SystemActionOutcome;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6HostActionSummary {
    pub(crate) total_action_count: u64,
    pub(crate) injected_command_count: u64,
    pub(crate) guest_host_call_count: u64,
    pub(crate) roi_begin: Vec<Rem6HostWorkMarkerSummary>,
    pub(crate) roi_end: Vec<Rem6HostWorkMarkerSummary>,
    pub(crate) stats_reset_count: u64,
    pub(crate) stats_dump_count: u64,
    pub(crate) checkpoint_count: u64,
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
                SystemActionOutcome::GuestHostCall { .. } => {
                    summary.guest_host_call_count += 1;
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
                SystemActionOutcome::StatsReset(_) => {
                    summary.stats_reset_count += 1;
                }
                SystemActionOutcome::StatsDump(_) => {
                    summary.stats_dump_count += 1;
                }
                SystemActionOutcome::Checkpoint { .. } => {
                    summary.checkpoint_count += 1;
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
pub(crate) struct Rem6HostWorkMarkerSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) work_id: u64,
    pub(crate) thread_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostStopActionSummary {
    pub(crate) tick: u64,
    pub(crate) event: u64,
    pub(crate) source: u32,
    pub(crate) code: i32,
}
