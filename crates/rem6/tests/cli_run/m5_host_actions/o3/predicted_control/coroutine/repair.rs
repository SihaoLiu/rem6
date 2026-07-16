#[test]
fn rem6_run_o3_older_branch_discards_same_window_coroutine_chain() {
    let path = older_branch_coroutine_binary("o3-older-branch-coroutine-chain");
    let completed = run_coroutine_json(
        &path,
        "cache-fabric-dram",
        3_000,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, "x1"),
        0x11,
        "unexpected repaired x1: {completed}"
    );
    assert_eq!(
        register_value(&completed, "x5"),
        0x55,
        "unexpected repaired x5: {completed}"
    );
    assert_register_absent_or_zero(&completed, "x13");
    assert_eq!(
        register_value(&completed, "x15"),
        0x33,
        "unexpected repaired x15: {completed}"
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000330000000000000000000000"),
        "unexpected repaired memory: {completed}"
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_no_data_address(&completed, WRONG_STORE_12_ADDRESS);

    let load = event_at_pc(&completed, "0x80000018");
    let branch = event_at_pc(&completed, "0x8000001c");
    for (field, expected) in [
        ("branch_predicted_taken", false),
        ("branch_resolved_taken", true),
        ("branch_mispredicted", true),
        ("branch_squash", true),
    ] {
        assert_eq!(
            branch
                .pointer(&format!("/{field}"))
                .and_then(Value::as_bool),
            Some(expected),
            "unexpected older-branch repair field {field}: {branch}"
        );
    }
    assert!(
        event_at_pc_if_present(&completed, "0x80000020").is_none(),
        "wrong-path call must not survive repair: {completed}"
    );
    assert!(
        event_at_pc_if_present(&completed, "0x8000002c").is_none(),
        "wrong-path coroutine must not survive repair: {completed}"
    );
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/lookups/return", 1),
        ("/cores/0/branch_predictor/committed/call_direct", 0),
        ("/cores/0/branch_predictor/committed/return", 0),
        ("/cores/0/branch_predictor/squashes/call_direct", 1),
        ("/cores/0/branch_predictor/squashes/return", 1),
        ("/cores/0/branch_predictor/ras/pushes", 3),
        ("/cores/0/branch_predictor/ras/pops", 3),
        ("/cores/0/branch_predictor/ras/squashes", 2),
        ("/cores/0/branch_predictor/ras/used", 0),
        ("/cores/0/branch_predictor/ras/correct", 0),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "unexpected repaired coroutine counter {pointer}: {completed}"
        );
    }

    let response_tick = event_u64(load, "lsq_data_response_tick");
    let live_tick = event_u64(branch, "issue_tick") + 2;
    assert!(
        live_tick < response_tick,
        "wrong-path coroutine window must precede load response: branch={branch}, load={load}"
    );
    let resident = run_coroutine_json(
        &path,
        "cache-fabric-dram",
        live_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x80000018", "0x8000001c", "0x80000020", "0x8000002c"],
        "unexpected pre-repair resident ROB at tick {live_tick}: {resident}"
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1),
        "expected one pre-repair resident LSQ row: {resident}"
    );
    assert_eq!(register_value(&resident, "x1"), 0x11);
    assert_eq!(register_value(&resident, "x5"), 0x55);
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000020", 1);
    assert_integer_rename_maps_to_row_destination(&resident, "0x8000002c", 5);

    let response_resident = run_coroutine_json(
        &path,
        "cache-fabric-dram",
        response_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_12_ADDRESS);
    assert_hierarchy_activity(&response_resident);
}

#[test]
fn rem6_run_o3_same_window_coroutine_wrong_target_repairs_descendants() {
    let path = wrong_target_coroutine_binary("o3-same-window-coroutine-wrong-target");
    let completed = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0014);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0020);
    assert_eq!(register_value(&completed, "x13"), 0x8000_0020);
    assert_register_absent_or_zero(&completed, "x14");
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000"),
        "unexpected wrong-target repair memory: {completed}"
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x8000000c");
    let call = event_at_pc(&completed, "0x80000010");
    let coroutine = event_at_pc(&completed, "0x8000001c");
    let repaired_descendant = event_at_pc(&completed, "0x80000028");
    let later_return = event_at_pc(&completed, "0x8000002c");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert_branch_kind_and_link(later_return, "return", false);
    for event in [call, coroutine] {
        assert!(
            event_u64(event, "issue_tick") < response_tick,
            "speculative coroutine chain must issue before load response at tick {response_tick}: {event}"
        );
    }
    assert!(
        event_u64(repaired_descendant, "issue_tick") > response_tick,
        "repaired descendant must restart after the older load response at tick {response_tick}: {repaired_descendant}"
    );
    assert!(
        event_u64(later_return, "issue_tick") > event_u64(repaired_descendant, "writeback_tick"),
        "later return must consume the repaired descendant's published x5 value: descendant={repaired_descendant}, return={later_return}"
    );
    assert_eq!(
        coroutine
            .pointer("/branch_predicted_target")
            .and_then(Value::as_str),
        Some("0x80000014")
    );
    assert_eq!(
        coroutine
            .pointer("/branch_resolved_target")
            .and_then(Value::as_str),
        Some("0x80000028")
    );
    for (field, expected) in [
        ("branch_predicted_taken", true),
        ("branch_resolved_taken", true),
        ("branch_wrong_target", true),
        ("branch_mispredicted", true),
        ("branch_squash", true),
    ] {
        assert_eq!(
            coroutine
                .pointer(&format!("/{field}"))
                .and_then(Value::as_bool),
            Some(expected),
            "unexpected wrong-target coroutine field {field}: {coroutine}"
        );
    }
    assert_eq!(
        coroutine.pointer("/branch_repair").and_then(Value::as_str),
        Some("wrong_target")
    );
    assert_eq!(
        coroutine
            .pointer("/branch_squashed_target")
            .and_then(Value::as_str),
        Some("0x80000014")
    );
    assert_eq!(
        later_return
            .pointer("/branch_predicted_target")
            .and_then(Value::as_str),
        None,
        "ordinary return must retain basic-update trace semantics: {later_return}"
    );
    assert_eq!(
        later_return
            .pointer("/branch_resolved_target")
            .and_then(Value::as_str),
        Some("0x80000020")
    );
    let wrong_target_fetches = completed
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("wrong-target coroutine fetch trace")
        .iter()
        .filter(|record| record.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .collect::<Vec<_>>();
    assert_eq!(
        wrong_target_fetches.len(),
        1,
        "wrong-target descendant must be fetched exactly once before repair: {completed}"
    );
    assert!(
        event_u64(wrong_target_fetches[0], "tick") <= event_u64(coroutine, "issue_tick"),
        "wrong-target descendant fetch must precede coroutine resolution: fetch={} coroutine={coroutine}",
        wrong_target_fetches[0]
    );
    assert!(
        event_at_pc_if_present(&completed, "0x80000014").is_none(),
        "wrong-target descendant must be squashed: {completed}"
    );
    assert_ordered_commits([load, call, coroutine, repaired_descendant, later_return]);

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/lookups/return", 2),
        ("/cores/0/branch_predictor/committed/call_direct", 1),
        ("/cores/0/branch_predictor/committed/return", 2),
        ("/cores/0/branch_predictor/squashes/call_direct", 0),
        ("/cores/0/branch_predictor/squashes/return", 0),
        ("/cores/0/branch_predictor/squashes/total", 0),
        ("/cores/0/branch_predictor/target_provider/no_target", 1),
        ("/cores/0/branch_predictor/target_provider/btb", 0),
        ("/cores/0/branch_predictor/target_provider/indirect", 0),
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 2),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 1),
        ("/cores/0/branch_predictor/target_provider/ras", 2),
        ("/cores/0/branch_predictor/target_provider/total", 3),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "unexpected wrong-target coroutine counter {pointer}: {completed}"
        );
    }
    assert_eq!(
        completed
            .pointer("/cores/0/branch_predictor/lookups/return")
            .and_then(Value::as_u64),
        completed
            .pointer("/cores/0/branch_predictor/target_provider/ras")
            .and_then(Value::as_u64),
        "the two return lookups must exhaust the two RAS-provided targets: {completed}"
    );
}
