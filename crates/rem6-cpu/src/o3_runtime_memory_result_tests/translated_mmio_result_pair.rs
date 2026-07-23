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

fn memory_result_head_at_tick_40() -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.stage_live_data_access_issue(
        &load_event(0x8000, 1, 5),
        request(20),
        40,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    runtime
}
