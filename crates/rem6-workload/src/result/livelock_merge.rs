use std::collections::BTreeMap;

use rem6_kernel::{LivelockTransitionKind, Tick, WaitForNode};

use crate::{WorkloadError, WorkloadParallelDiagnosticScope};

pub(super) fn validate_livelock_subject_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &[(WaitForNode, usize, u64, Tick, Tick)],
    scoped: &[(WaitForNode, usize, u64, Tick, Tick)],
) -> Result<(), WorkloadError> {
    for (
        scoped_subject,
        scoped_diagnostic_count,
        scoped_transition_count,
        scoped_first_tick,
        scoped_last_tick,
    ) in scoped
    {
        let merged_summary = merged
            .iter()
            .find(|(merged_subject, _, _, _, _)| merged_subject == scoped_subject);
        let Some((
            _,
            merged_diagnostic_count,
            merged_transition_count,
            merged_first_tick,
            merged_last_tick,
        )) = merged_summary
        else {
            return Err(WorkloadError::InvalidParallelLivelockSubjectMergeSummary {
                scope,
                subject: scoped_subject.clone(),
                merged_diagnostic_count: 0,
                scoped_diagnostic_count: *scoped_diagnostic_count,
                merged_transition_count: 0,
                scoped_transition_count: *scoped_transition_count,
                merged_first_tick: None,
                scoped_first_tick: *scoped_first_tick,
                merged_last_tick: None,
                scoped_last_tick: *scoped_last_tick,
            });
        };

        if merged_diagnostic_count < scoped_diagnostic_count
            || merged_transition_count < scoped_transition_count
            || merged_first_tick > scoped_first_tick
            || merged_last_tick < scoped_last_tick
        {
            return Err(WorkloadError::InvalidParallelLivelockSubjectMergeSummary {
                scope,
                subject: scoped_subject.clone(),
                merged_diagnostic_count: *merged_diagnostic_count,
                scoped_diagnostic_count: *scoped_diagnostic_count,
                merged_transition_count: *merged_transition_count,
                scoped_transition_count: *scoped_transition_count,
                merged_first_tick: Some(*merged_first_tick),
                scoped_first_tick: *scoped_first_tick,
                merged_last_tick: Some(*merged_last_tick),
                scoped_last_tick: *scoped_last_tick,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_livelock_transition_kind_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &[(LivelockTransitionKind, usize, u64, Tick, Tick)],
    scoped: &[(LivelockTransitionKind, usize, u64, Tick, Tick)],
) -> Result<(), WorkloadError> {
    for (
        scoped_kind,
        scoped_diagnostic_count,
        scoped_transition_count,
        scoped_first_tick,
        scoped_last_tick,
    ) in scoped
    {
        let merged_summary = merged
            .iter()
            .find(|(merged_kind, _, _, _, _)| merged_kind == scoped_kind);
        let Some((
            _,
            merged_diagnostic_count,
            merged_transition_count,
            merged_first_tick,
            merged_last_tick,
        )) = merged_summary
        else {
            return Err(
                WorkloadError::InvalidParallelLivelockTransitionKindMergeSummary {
                    scope,
                    kind: *scoped_kind,
                    merged_diagnostic_count: 0,
                    scoped_diagnostic_count: *scoped_diagnostic_count,
                    merged_transition_count: 0,
                    scoped_transition_count: *scoped_transition_count,
                    merged_first_tick: None,
                    scoped_first_tick: *scoped_first_tick,
                    merged_last_tick: None,
                    scoped_last_tick: *scoped_last_tick,
                },
            );
        };

        if merged_diagnostic_count < scoped_diagnostic_count
            || merged_transition_count < scoped_transition_count
            || merged_first_tick > scoped_first_tick
            || merged_last_tick < scoped_last_tick
        {
            return Err(
                WorkloadError::InvalidParallelLivelockTransitionKindMergeSummary {
                    scope,
                    kind: *scoped_kind,
                    merged_diagnostic_count: *merged_diagnostic_count,
                    scoped_diagnostic_count: *scoped_diagnostic_count,
                    merged_transition_count: *merged_transition_count,
                    scoped_transition_count: *scoped_transition_count,
                    merged_first_tick: Some(*merged_first_tick),
                    scoped_first_tick: *scoped_first_tick,
                    merged_last_tick: Some(*merged_last_tick),
                    scoped_last_tick: *scoped_last_tick,
                },
            );
        }
    }
    Ok(())
}

pub(super) fn merge_livelock_transition_kind_window_summaries(
    summaries: impl IntoIterator<Item = (LivelockTransitionKind, usize, u64, Tick, Tick)>,
) -> Vec<(LivelockTransitionKind, usize, u64, Tick, Tick)> {
    let mut merged = BTreeMap::<LivelockTransitionKind, (usize, u64, Tick, Tick)>::new();
    for (kind, diagnostic_count, transition_count, first_tick, last_tick) in summaries {
        merged
            .entry(kind)
            .and_modify(|summary| {
                summary.0 += diagnostic_count;
                summary.1 += transition_count;
                summary.2 = summary.2.min(first_tick);
                summary.3 = summary.3.max(last_tick);
            })
            .or_insert((diagnostic_count, transition_count, first_tick, last_tick));
    }
    merged
        .into_iter()
        .map(
            |(kind, (diagnostic_count, transition_count, first_tick, last_tick))| {
                (
                    kind,
                    diagnostic_count,
                    transition_count,
                    first_tick,
                    last_tick,
                )
            },
        )
        .collect()
}
