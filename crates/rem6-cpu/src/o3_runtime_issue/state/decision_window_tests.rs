use super::*;

fn observed_decision(tick: u64) -> O3LiveIssueActiveTick {
    let mut decision = O3LiveIssueActiveTick::at(tick, true);
    decision.observe(&[tick], &[], &[], 1);
    decision
}

#[test]
fn counted_ticks_window_finalizes_past_and_retains_future_decisions() {
    let mut window = O3LiveIssueDecisionWindow::default();
    window.retain(observed_decision(20));
    window.retain(observed_decision(30));

    let finalized = window.finalize_before(21);
    assert_eq!(finalized.issue_cycles, 1);
    assert_eq!(finalized.issued_rows, 1);
    assert_eq!(window.counted_ticks(), [30]);
    assert_eq!(window.projection().issue_cycles, 1);
}

#[test]
fn counted_ticks_window_reset_rebases_retained_decisions() {
    let mut window = O3LiveIssueDecisionWindow::default();
    window.retain(observed_decision(30));
    assert_eq!(window.projection().issued_rows, 1);

    window.reset_baselines();
    assert_eq!(
        window.projection(),
        O3LiveIssueDecisionProjection::default()
    );
    assert!(window.counted_ticks().is_empty());

    let mut decision = window.take(30).unwrap();
    decision.observe(&[31], &[], &[], 1);
    window.retain(decision);
    assert_eq!(window.projection().issue_cycles, 1);
    assert_eq!(window.projection().issued_rows, 1);
}
