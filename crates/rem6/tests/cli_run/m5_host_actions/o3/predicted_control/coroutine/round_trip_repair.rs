use super::round_trip::COROUTINE_ROUND_TRIP_CASES;
use super::*;

fn push_round_trip_repair_word(words: &mut Vec<u32>, expected_pc: usize, word: u32, fixture: &str) {
    assert_eq!(
        words.len() * 4,
        expected_pc,
        "{fixture} fixture must place its next instruction at {expected_pc:#04x}"
    );
    words.push(word);
}

fn middle_round_trip_repair_binary(name: &str) -> std::path::PathBuf {
    let mut words = Vec::new();
    push_round_trip_repair_word(&mut words, 0x00, m5op(M5_SWITCH_CPU), "middle repair");
    push_round_trip_repair_word(&mut words, 0x04, u_type(0, 18, 0x17), "middle repair");
    push_round_trip_repair_word(
        &mut words,
        0x08,
        i_type(DATA_START - 0x04, 18, 0x0, 18, 0x13),
        "middle repair",
    );
    push_round_trip_repair_word(
        &mut words,
        0x0c,
        i_type(99, 0, 0x0, 7, 0x13),
        "middle repair",
    );
    push_round_trip_repair_word(
        &mut words,
        0x10,
        i_type(0, 18, 0b010, 12, 0x03),
        "middle repair",
    );
    push_round_trip_repair_word(&mut words, 0x14, j_type(12, 1), "middle repair");
    push_round_trip_repair_word(
        &mut words,
        0x18,
        i_type(0, 5, 0x0, 0, 0x67),
        "middle repair",
    );
    push_round_trip_repair_word(&mut words, 0x1c, s_type(8, 7, 18, 0b010), "middle repair");
    push_round_trip_repair_word(
        &mut words,
        0x20,
        i_type(24, 1, 0x0, 5, 0x67),
        "middle repair",
    );
    push_round_trip_repair_word(&mut words, 0x24, s_type(4, 5, 18, 0b010), "middle repair");
    push_round_trip_repair_word(&mut words, 0x28, m5op(M5_EXIT), "middle repair");
    push_round_trip_repair_word(&mut words, 0x2c, m5op(M5_FAIL), "middle repair");
    push_round_trip_repair_word(
        &mut words,
        0x30,
        i_type(0, 5, 0x0, 13, 0x13),
        "middle repair",
    );
    push_round_trip_repair_word(
        &mut words,
        0x34,
        i_type(0, 5, 0x0, 0, 0x67),
        "middle repair",
    );
    push_round_trip_repair_word(&mut words, 0x38, m5op(M5_FAIL), "middle repair");
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn terminal_round_trip_direction_only_binary(name: &str) -> std::path::PathBuf {
    let mut words = Vec::new();
    push_round_trip_repair_word(&mut words, 0x00, m5op(M5_SWITCH_CPU), "terminal repair");
    push_round_trip_repair_word(&mut words, 0x04, u_type(0, 18, 0x17), "terminal repair");
    push_round_trip_repair_word(
        &mut words,
        0x08,
        i_type(DATA_START - 0x04, 18, 0x0, 18, 0x13),
        "terminal repair",
    );
    push_round_trip_repair_word(
        &mut words,
        0x0c,
        i_type(99, 0, 0x0, 7, 0x13),
        "terminal repair",
    );
    push_round_trip_repair_word(
        &mut words,
        0x10,
        i_type(0, 18, 0b010, 12, 0x03),
        "terminal repair",
    );
    push_round_trip_repair_word(&mut words, 0x14, j_type(12, 1), "terminal repair");
    push_round_trip_repair_word(
        &mut words,
        0x18,
        i_type(8, 5, 0x0, 0, 0x67),
        "terminal repair",
    );
    push_round_trip_repair_word(&mut words, 0x1c, s_type(8, 7, 18, 0b010), "terminal repair");
    push_round_trip_repair_word(
        &mut words,
        0x20,
        i_type(0, 1, 0x0, 5, 0x67),
        "terminal repair",
    );
    push_round_trip_repair_word(&mut words, 0x24, s_type(8, 7, 18, 0b010), "terminal repair");
    push_round_trip_repair_word(&mut words, 0x28, m5op(M5_FAIL), "terminal repair");
    push_round_trip_repair_word(&mut words, 0x2c, s_type(4, 5, 18, 0b010), "terminal repair");
    push_round_trip_repair_word(&mut words, 0x30, m5op(M5_EXIT), "terminal repair");
    push_round_trip_repair_word(&mut words, 0x34, m5op(M5_FAIL), "terminal repair");
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn assert_round_trip_counter(json: &Value, label: &str, pointer: &str, expected: u64) {
    assert_eq!(
        json.pointer(pointer).and_then(Value::as_u64),
        Some(expected),
        "{label}: expected {pointer}={expected}: {json}"
    );
}

fn assert_round_trip_no_data_address(json: &Value, label: &str, address: &str) {
    for pointer in ["/debug/data_trace", "/debug/memory_trace"] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_array)
                .is_some_and(|records| records.iter().all(|record| {
                    record.pointer("/address").and_then(Value::as_str) != Some(address)
                })),
            "{label}: unexpected data access at {address} in {pointer}: {json}"
        );
    }
}

fn round_trip_fetches_at_pc<'a>(json: &'a Value, label: &str, pc: &str) -> Vec<&'a Value> {
    json.pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{label}: missing fetch trace: {json}"))
        .iter()
        .filter(|record| record.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .collect()
}

fn assert_round_trip_fetch_count(json: &Value, label: &str, pc: &str, expected: usize) {
    let actual = round_trip_fetches_at_pc(json, label, pc).len();
    assert_eq!(
        actual, expected,
        "{label}: expected exactly {expected} fetches at {pc}: {json}"
    );
}

fn assert_round_trip_commit_order(label: &str, events: &[(&str, &Value)]) {
    for pair in events.windows(2) {
        let (older_role, older) = pair[0];
        let (younger_role, younger) = pair[1];
        let older_tick = event_u64(older, "commit_tick");
        let younger_tick = event_u64(younger, "commit_tick");
        assert!(
            older_tick <= younger_tick,
            "{label}: {older_role} commit at {older_tick} must precede {younger_role} commit at {younger_tick}: older={older}, younger={younger}"
        );
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_requires_branch_lookahead_three() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-lookahead-two-{}", case.label),
            0,
        );
        let completed = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );

        assert_stopped_by_host(&completed);
        assert_eq!(
            register_value(&completed, "x1"),
            case.final_x1,
            "{}: unexpected completed x1: {completed}",
            case.label
        );
        assert_eq!(
            register_value(&completed, "x5"),
            case.final_x5,
            "{}: unexpected completed x5: {completed}",
            case.label
        );
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some(case.memory_hex),
            "{}: unexpected completed memory: {completed}",
            case.label
        );
        assert_round_trip_no_data_address(&completed, case.label, WRONG_STORE_ADDRESS);

        let load = event_at_pc(&completed, case.load_pc);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        let resident_tick = response_tick - 1;
        let resident = run_coroutine_json(
            &path,
            case.memory_system,
            resident_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [case.load_pc, case.call_pc, case.coroutine_pc],
            "{}: unexpected resident ROB at pre-response tick {resident_tick}: {resident}",
            case.label
        );
        assert_round_trip_counter(
            &resident,
            case.label,
            "/cores/0/o3_runtime/snapshot/lsq/count",
            1,
        );
        assert_round_trip_counter(
            &resident,
            case.label,
            "/cores/0/branch_predictor/lookups/return",
            1,
        );
        assert_round_trip_counter(
            &resident,
            case.label,
            "/cores/0/branch_predictor/target_provider/ras",
            1,
        );
        assert_round_trip_counter(
            &resident,
            case.label,
            "/cores/0/branch_predictor/lookups/total",
            2,
        );
        assert_round_trip_counter(
            &resident,
            case.label,
            "/cores/0/branch_predictor/target_provider/total",
            2,
        );
        assert_round_trip_fetch_count(&resident, case.label, case.return_pc, 1);
        assert_round_trip_fetch_count(&resident, case.label, case.success_store_pc, 0);
        assert_round_trip_no_data_address(&resident, case.label, WRONG_STORE_ADDRESS);
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_middle_repair_discards_return() {
    let label = "middle round-trip repair";
    let path = middle_round_trip_repair_binary("o3-coroutine-round-trip-middle-repair");
    let completed = run_coroutine_json(&path, "direct", 3_000, "detailed", 3, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    for (register, expected) in [
        ("x1", 0x8000_0018),
        ("x5", 0x8000_0024),
        ("x13", 0x8000_0024),
    ] {
        assert_eq!(
            register_value(&completed, register),
            expected,
            "{label}: unexpected final {register}: {completed}"
        );
    }
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000240000800000000000000000"),
        "{label}: unexpected final memory: {completed}"
    );
    assert_round_trip_no_data_address(&completed, label, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x80000010");
    let call = event_at_pc(&completed, "0x80000014");
    let coroutine = event_at_pc(&completed, "0x80000020");
    let repaired = event_at_pc(&completed, "0x80000030");
    let later_return = event_at_pc(&completed, "0x80000034");
    let witness = event_at_pc(&completed, "0x80000024");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert_branch_kind_and_link(later_return, "return", false);
    for (pointer, expected) in [
        ("/branch_predicted_target", "0x80000018"),
        ("/branch_resolved_target", "0x80000030"),
        ("/branch_squashed_target", "0x80000018"),
        ("/branch_repair", "wrong_target"),
    ] {
        assert_eq!(
            coroutine.pointer(pointer).and_then(Value::as_str),
            Some(expected),
            "{label}: unexpected coroutine field {pointer}: {coroutine}"
        );
    }
    for field in [
        "branch_predicted_taken",
        "branch_resolved_taken",
        "branch_wrong_target",
        "branch_mispredicted",
        "branch_squash",
    ] {
        assert_eq!(
            coroutine
                .pointer(&format!("/{field}"))
                .and_then(Value::as_bool),
            Some(true),
            "{label}: unexpected coroutine flag {field}: {coroutine}"
        );
    }
    let speculative_return_fetches = round_trip_fetches_at_pc(&completed, label, "0x80000018");
    assert_eq!(
        speculative_return_fetches.len(),
        1,
        "{label}: speculative ordinary return must be fetched exactly once: {completed}"
    );
    let target_fetches = round_trip_fetches_at_pc(&completed, label, "0x80000024");
    assert_eq!(
        target_fetches.len(),
        2,
        "{label}: expected one speculative and one later legitimate target fetch: {completed}"
    );
    let speculative_return_fetch_tick = event_u64(speculative_return_fetches[0], "tick");
    let speculative_target_fetch_tick = event_u64(target_fetches[0], "tick");
    let legitimate_target_fetch_tick = event_u64(target_fetches[1], "tick");
    let coroutine_issue_tick = event_u64(coroutine, "issue_tick");
    let later_return_issue_tick = event_u64(later_return, "issue_tick");
    assert!(
        speculative_return_fetch_tick < speculative_target_fetch_tick
            && speculative_target_fetch_tick < coroutine_issue_tick,
        "{label}: speculative return and target fetches must both precede coroutine repair: return_fetch={}, target_fetch={}, coroutine={coroutine}",
        speculative_return_fetches[0],
        target_fetches[0]
    );
    assert_eq!(
        legitimate_target_fetch_tick, later_return_issue_tick,
        "{label}: the only post-repair target fetch must be opened by the later legitimate return: fetch={}, return={later_return}",
        target_fetches[1]
    );
    assert!(
        event_at_pc_if_present(&completed, "0x80000018").is_none(),
        "{label}: squashed wrong-path ordinary return must not commit: {completed}"
    );
    assert!(
        event_u64(repaired, "issue_tick") > event_u64(coroutine, "commit_tick"),
        "{label}: repaired descendant must issue after coroutine commit: coroutine={coroutine}, repaired={repaired}"
    );
    assert!(
        event_u64(later_return, "issue_tick") > event_u64(repaired, "writeback_tick"),
        "{label}: later return must issue after repaired writeback: repaired={repaired}, return={later_return}"
    );
    assert!(
        event_u64(witness, "issue_tick") > event_u64(later_return, "issue_tick"),
        "{label}: final witness must issue after the later return: return={later_return}, witness={witness}"
    );
    assert_round_trip_commit_order(
        label,
        &[
            ("load", load),
            ("call", call),
            ("coroutine", coroutine),
            ("repaired descendant", repaired),
            ("later return", later_return),
            ("final witness", witness),
        ],
    );

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/lookups/call_indirect", 0),
        ("/cores/0/branch_predictor/lookups/return", 3),
        ("/cores/0/branch_predictor/lookups/total", 4),
        ("/cores/0/branch_predictor/committed/call_direct", 1),
        ("/cores/0/branch_predictor/committed/call_indirect", 0),
        ("/cores/0/branch_predictor/committed/return", 2),
        ("/cores/0/branch_predictor/committed/total", 3),
        ("/cores/0/branch_predictor/squashes/call_direct", 0),
        ("/cores/0/branch_predictor/squashes/call_indirect", 0),
        ("/cores/0/branch_predictor/squashes/return", 1),
        ("/cores/0/branch_predictor/squashes/total", 1),
        ("/cores/0/branch_predictor/mispredicted/call_direct", 0),
        ("/cores/0/branch_predictor/mispredicted/call_indirect", 0),
        ("/cores/0/branch_predictor/mispredicted/return", 1),
        ("/cores/0/branch_predictor/mispredicted/total", 1),
        ("/cores/0/branch_predictor/target_provider/no_target", 1),
        ("/cores/0/branch_predictor/target_provider/btb", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 3),
        ("/cores/0/branch_predictor/target_provider/indirect", 0),
        ("/cores/0/branch_predictor/target_provider/total", 4),
        ("/cores/0/branch_predictor/btb/mispredictions", 3),
        ("/cores/0/branch_predictor/indirect_hits", 0),
        ("/cores/0/branch_predictor/indirect_mispredicted", 0),
        ("/cores/0/branch_predictor/ras/pushes", 3),
        ("/cores/0/branch_predictor/ras/pops", 3),
        ("/cores/0/branch_predictor/ras/squashes", 1),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 1),
        ("/cores/0/o3_runtime/branch_repair/wrong_targets", 1),
        (
            "/cores/0/o3_runtime/branch_repair/wrong_target_kind/return",
            1,
        ),
    ] {
        assert_round_trip_counter(&completed, label, pointer, expected);
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_terminal_return_repairs_direction() {
    let label = "terminal round-trip direction-only repair";
    let path = terminal_round_trip_direction_only_binary(
        "o3-coroutine-round-trip-terminal-return-direction-only",
    );
    let completed = run_coroutine_json(&path, "direct", 3_000, "detailed", 3, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, "x1"),
        0x8000_0018,
        "{label}: unexpected final x1: {completed}"
    );
    assert_eq!(
        register_value(&completed, "x5"),
        0x8000_0024,
        "{label}: unexpected final x5: {completed}"
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000240000800000000000000000"),
        "{label}: unexpected final memory: {completed}"
    );
    assert_round_trip_no_data_address(&completed, label, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x80000010");
    let call = event_at_pc(&completed, "0x80000014");
    let coroutine = event_at_pc(&completed, "0x80000020");
    let ordinary_return = event_at_pc(&completed, "0x80000018");
    let witness = event_at_pc(&completed, "0x8000002c");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert_branch_kind_and_link(ordinary_return, "return", false);
    assert!(
        ordinary_return
            .pointer("/branch_predicted_target")
            .is_some_and(Value::is_null),
        "{label}: ordinary-return predicted target must remain JSON null: {ordinary_return}"
    );
    for (pointer, expected) in [
        ("/branch_resolved_target", "0x8000002c"),
        ("/branch_squashed_target", "0x8000001c"),
        ("/branch_repair", "direction_only"),
    ] {
        assert_eq!(
            ordinary_return.pointer(pointer).and_then(Value::as_str),
            Some(expected),
            "{label}: unexpected ordinary-return field {pointer}: {ordinary_return}"
        );
    }
    for (field, expected) in [
        ("branch_predicted_taken", false),
        ("branch_resolved_taken", true),
        ("branch_wrong_target", false),
        ("branch_mispredicted", true),
        ("branch_squash", true),
        ("branch_targetless_mismatch", false),
    ] {
        assert_eq!(
            ordinary_return
                .pointer(&format!("/{field}"))
                .and_then(Value::as_bool),
            Some(expected),
            "{label}: unexpected ordinary-return flag {field}: {ordinary_return}"
        );
    }
    let return_fetches = round_trip_fetches_at_pc(&completed, label, "0x80000018");
    assert_eq!(
        return_fetches.len(),
        1,
        "{label}: terminal ordinary return must be fetched exactly once: {completed}"
    );
    let speculative_target_fetches = round_trip_fetches_at_pc(&completed, label, "0x80000024");
    assert_eq!(
        speculative_target_fetches.len(),
        1,
        "{label}: RAS replacement target must be fetched exactly once: {completed}"
    );
    let return_fetch_tick = event_u64(return_fetches[0], "tick");
    let target_fetch_tick = event_u64(speculative_target_fetches[0], "tick");
    assert!(
        return_fetch_tick < target_fetch_tick
            && target_fetch_tick < event_u64(ordinary_return, "issue_tick"),
        "{label}: RAS replacement target must be fetched after the return instruction and before its resolution: return_fetch={}, target_fetch={}, return={ordinary_return}",
        return_fetches[0],
        speculative_target_fetches[0]
    );
    for wrong_path_pc in ["0x8000001c", "0x80000024"] {
        assert!(
            event_at_pc_if_present(&completed, wrong_path_pc).is_none(),
            "{label}: wrong-path event {wrong_path_pc} must not commit: {completed}"
        );
    }
    assert_round_trip_commit_order(
        label,
        &[
            ("load", load),
            ("call", call),
            ("coroutine", coroutine),
            ("ordinary return", ordinary_return),
            ("success witness", witness),
        ],
    );

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/lookups/call_indirect", 0),
        ("/cores/0/branch_predictor/lookups/return", 2),
        ("/cores/0/branch_predictor/lookups/total", 3),
        ("/cores/0/branch_predictor/committed/call_direct", 1),
        ("/cores/0/branch_predictor/committed/call_indirect", 0),
        ("/cores/0/branch_predictor/committed/return", 2),
        ("/cores/0/branch_predictor/committed/total", 3),
        ("/cores/0/branch_predictor/squashes/call_direct", 0),
        ("/cores/0/branch_predictor/squashes/call_indirect", 0),
        ("/cores/0/branch_predictor/squashes/return", 0),
        ("/cores/0/branch_predictor/squashes/total", 0),
        ("/cores/0/branch_predictor/mispredicted/call_direct", 0),
        ("/cores/0/branch_predictor/mispredicted/call_indirect", 0),
        ("/cores/0/branch_predictor/mispredicted/return", 1),
        ("/cores/0/branch_predictor/mispredicted/total", 1),
        ("/cores/0/branch_predictor/target_provider/no_target", 1),
        ("/cores/0/branch_predictor/target_provider/btb", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 2),
        ("/cores/0/branch_predictor/target_provider/indirect", 0),
        ("/cores/0/branch_predictor/target_provider/total", 3),
        ("/cores/0/branch_predictor/btb/mispredictions", 3),
        ("/cores/0/branch_predictor/indirect_hits", 0),
        ("/cores/0/branch_predictor/indirect_mispredicted", 0),
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 2),
        ("/cores/0/branch_predictor/ras/squashes", 0),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 1),
        (
            "/cores/0/o3_runtime/branch_repair/direction_only_mismatches",
            2,
        ),
        (
            "/cores/0/o3_runtime/branch_repair/direction_only_kind/call_direct",
            1,
        ),
        (
            "/cores/0/o3_runtime/branch_repair/direction_only_kind/return",
            1,
        ),
        ("/cores/0/o3_runtime/branch_repair/targetless_mismatches", 0),
        ("/cores/0/o3_runtime/branch_repair/wrong_targets", 0),
        (
            "/cores/0/o3_runtime/branch_repair/wrong_target_kind/return",
            0,
        ),
    ] {
        assert_round_trip_counter(&completed, label, pointer, expected);
    }
}
