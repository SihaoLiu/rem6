use rem6_transport::TargetOutcome;

pub(super) struct CliDataCacheBacking {
    pub(super) dram_access_count: usize,
    pub(super) ready_tick: u64,
}

pub(super) struct CliDataCacheLineFill {
    pub(super) data: Vec<u8>,
    pub(super) ready_tick: u64,
}

pub(super) fn delay_target_outcome_until(
    outcome: TargetOutcome,
    start_tick: u64,
    ready_tick: u64,
) -> TargetOutcome {
    let backing_delay = ready_tick
        .checked_sub(start_tick)
        .expect("CLI cache backing is not ready before request arrival");
    match outcome {
        TargetOutcome::Respond(response) if backing_delay != 0 => TargetOutcome::RespondAfter {
            delay: backing_delay,
            response,
        },
        TargetOutcome::RespondAfter { delay, response } => TargetOutcome::RespondAfter {
            delay: delay.max(backing_delay),
            response,
        },
        outcome => outcome,
    }
}
