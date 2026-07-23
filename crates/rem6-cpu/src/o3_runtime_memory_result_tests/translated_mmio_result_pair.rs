use super::*;

#[test]
fn translated_result_pair_memory_width_one_selects_the_next_tick() {
    let mut runtime = memory_result_head_at_tick_40();
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_memory_issue_width(1));

    assert_eq!(runtime.next_memory_result_issue_tick(40), Some(41));
}

#[test]
fn translated_result_pair_memory_width_two_reuses_the_head_tick() {
    let mut runtime = memory_result_head_at_tick_40();
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_memory_issue_width(2));

    assert_eq!(runtime.next_memory_result_issue_tick(40), Some(40));
}

#[test]
fn translated_result_pair_total_width_one_still_selects_the_next_tick() {
    let mut runtime = memory_result_head_at_tick_40();
    runtime.issue_width = 1;
    runtime.memory_issue_width = 2;

    assert_eq!(runtime.next_memory_result_issue_tick(40), Some(41));
}

#[test]
fn translated_result_pair_max_tick_free_selects_max_tick() {
    let mut runtime = memory_result_head_at_tick_40();
    assert!(runtime.set_issue_width(1));
    assert!(runtime.set_memory_issue_width(1));

    assert_eq!(
        runtime.next_memory_result_issue_tick(u64::MAX),
        Some(u64::MAX)
    );
}

#[test]
fn translated_result_pair_max_tick_occupied_returns_none() {
    let mut runtime = memory_result_head_at_tick(u64::MAX);
    assert!(runtime.set_issue_width(1));
    assert!(runtime.set_memory_issue_width(1));

    assert_eq!(runtime.next_memory_result_issue_tick(u64::MAX), None);
}

#[test]
fn scalar_memory_prefix_is_not_an_exact_memory_result_head() {
    let mut runtime = O3RuntimeState::default();
    let event = load_event(0x8000, 1, 5);
    let data_request = request(20);
    assert!(runtime.stage_live_data_access_issue(
        &event,
        data_request,
        40,
        O3DataAccessWindowPolicy::ScalarMemoryPrefix,
    ));
    let sequence = runtime.live_data_accesses[0].sequence;
    let access = event.execution().memory_access().unwrap();

    assert!(!runtime.matches_exact_memory_result_head(
        event.fetch().request_id(),
        data_request,
        40,
        sequence,
        access,
    ));
    assert_eq!(runtime.next_memory_result_issue_tick(40), None);
}

#[test]
fn memory_result_window_head_matches_its_exact_identity() {
    let runtime = memory_result_head_at_tick_40();
    let head = &runtime.live_data_accesses[0];

    assert!(runtime.matches_exact_memory_result_head(
        head.fetch_request,
        head.data_request,
        head.issue_tick,
        head.sequence,
        head.execution.execution().memory_access().unwrap(),
    ));
}

fn memory_result_head_at_tick_40() -> O3RuntimeState {
    memory_result_head_at_tick(40)
}

fn memory_result_head_at_tick(issue_tick: u64) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.stage_live_data_access_issue(
        &load_event(0x8000, 1, 5),
        request(20),
        issue_tick,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    runtime
}
