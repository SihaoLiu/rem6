use super::*;

#[test]
fn counted_ticks_prune_past_evidence_and_retain_exact_future_ticks() {
    let mut ticks = O3LiveIssueCountedTicks::default();
    assert!(ticks.record(20));
    assert!(ticks.record(30));
    assert!(!ticks.record(30));

    ticks.prune_before(21);
    assert_eq!(ticks.values(), [30]);
    assert!(ticks.record(21));
    assert_eq!(ticks.values(), [21, 30]);

    ticks.prune_before(31);
    assert!(ticks.values().is_empty());
}

#[test]
fn counted_ticks_reset_clears_cycle_evidence() {
    let mut ticks = O3LiveIssueCountedTicks::default();
    assert!(ticks.record(20));
    assert!(ticks.record(30));
    ticks.clear();
    assert_eq!(ticks.len(), 0);
}
