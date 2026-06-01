use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{LivelockDiagnostic, LivelockTransitionKind, Tick, WaitForNode};

pub(super) fn collect_livelock_diagnostic_subjects<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
) -> Vec<WaitForNode> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.subject().clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn collect_livelock_diagnostic_subject_summaries<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
) -> Vec<(WaitForNode, usize, u64, Tick, Tick)> {
    let mut summaries = BTreeMap::<WaitForNode, (usize, u64, Tick, Tick)>::new();
    for diagnostic in diagnostics {
        summaries
            .entry(diagnostic.subject().clone())
            .and_modify(|summary| {
                summary.0 += 1;
                summary.1 += diagnostic.transition_count();
                summary.2 = summary.2.min(diagnostic.first_transition_tick());
                summary.3 = summary.3.max(diagnostic.last_transition_tick());
            })
            .or_insert((
                1,
                diagnostic.transition_count(),
                diagnostic.first_transition_tick(),
                diagnostic.last_transition_tick(),
            ));
    }
    summaries
        .into_iter()
        .map(
            |(subject, (diagnostic_count, transition_count, first_tick, last_tick))| {
                (
                    subject,
                    diagnostic_count,
                    transition_count,
                    first_tick,
                    last_tick,
                )
            },
        )
        .collect()
}

pub(super) fn collect_livelock_diagnostics_by_subject<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
    subject: &WaitForNode,
) -> Vec<LivelockDiagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.subject() == subject)
        .cloned()
        .collect()
}

pub(super) fn livelock_diagnostic_tick_window<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
    mut predicate: impl FnMut(&LivelockDiagnostic) -> bool,
) -> Option<(Tick, Tick)> {
    let mut window: Option<(Tick, Tick)> = None;
    for diagnostic in diagnostics {
        if predicate(diagnostic) {
            window = Some(match window {
                Some((first_tick, last_tick)) => (
                    first_tick.min(diagnostic.first_transition_tick()),
                    last_tick.max(diagnostic.last_transition_tick()),
                ),
                None => (
                    diagnostic.first_transition_tick(),
                    diagnostic.last_transition_tick(),
                ),
            });
        }
    }
    window
}

pub(super) fn collect_livelock_diagnostic_subjects_by_transition_kind<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
    kind: LivelockTransitionKind,
) -> Vec<WaitForNode> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.transition_count_by_kind(kind) != 0)
        .map(|diagnostic| diagnostic.subject().clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn collect_livelock_diagnostics_by_transition_kind<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
    kind: LivelockTransitionKind,
) -> Vec<LivelockDiagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.transition_count_by_kind(kind) != 0)
        .cloned()
        .collect()
}

pub(super) fn livelock_diagnostic_transition_count_by_kind<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
    kind: LivelockTransitionKind,
) -> u64 {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.transition_count_by_kind(kind))
        .sum()
}

pub(super) fn livelock_diagnostic_transition_kind_tick_window<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
    kind: LivelockTransitionKind,
) -> Option<(Tick, Tick)> {
    let mut window: Option<(Tick, Tick)> = None;
    for diagnostic in diagnostics {
        for count in diagnostic.transition_kind_counts() {
            if count.kind() == kind {
                window = Some(match window {
                    Some((first_tick, last_tick)) => (
                        first_tick.min(count.first_transition_tick()),
                        last_tick.max(count.last_transition_tick()),
                    ),
                    None => (count.first_transition_tick(), count.last_transition_tick()),
                });
            }
        }
    }
    window
}

pub(super) fn collect_livelock_diagnostic_transition_kind_summaries<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
) -> Vec<(LivelockTransitionKind, u64)> {
    let mut summaries = BTreeMap::<LivelockTransitionKind, u64>::new();
    for diagnostic in diagnostics {
        for count in diagnostic.transition_kind_counts() {
            *summaries.entry(count.kind()).or_insert(0) += count.count();
        }
    }
    summaries.into_iter().collect()
}

pub(super) fn collect_livelock_diagnostic_transition_kind_window_summaries<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
) -> Vec<(LivelockTransitionKind, usize, u64, Tick, Tick)> {
    let mut summaries = BTreeMap::<LivelockTransitionKind, (usize, u64, Tick, Tick)>::new();
    for diagnostic in diagnostics {
        for count in diagnostic.transition_kind_counts() {
            summaries
                .entry(count.kind())
                .and_modify(|summary| {
                    summary.0 += 1;
                    summary.1 += count.count();
                    summary.2 = summary.2.min(count.first_transition_tick());
                    summary.3 = summary.3.max(count.last_transition_tick());
                })
                .or_insert((
                    1,
                    count.count(),
                    count.first_transition_tick(),
                    count.last_transition_tick(),
                ));
        }
    }
    summaries
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
