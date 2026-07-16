#[test]
fn rem6_run_o3_same_window_coroutine_requires_branch_lookahead_two() {
    let path = direct_coroutine_binary("o3-same-window-coroutine-lookahead-one", 0);
    let completed = run_coroutine_json(&path, "direct", 2_500, "detailed", 1, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, "x1"),
        0x8000_0014,
        "unexpected completed x1: {completed}"
    );
    assert_eq!(
        register_value(&completed, "x5"),
        0x8000_0020,
        "unexpected completed x5: {completed}"
    );
    assert_eq!(
        register_value(&completed, "x13"),
        0x8000_0020,
        "unexpected completed x13: {completed}"
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000"),
        "unexpected completed memory: {completed}"
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x8000000c");
    let call = event_at_pc(&completed, "0x80000010");
    let coroutine = event_at_pc(&completed, "0x8000001c");
    let descendant = event_at_pc(&completed, "0x80000014");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert!(
        event_u64(call, "issue_tick") < response_tick,
        "direct coroutine call must issue before the load response at tick {response_tick}: {call}"
    );
    assert!(
        event_u64(descendant, "issue_tick") > response_tick,
        "lookahead one must delay the descendant until strictly after the load response at tick {response_tick}: {descendant}"
    );

    let live_tick = response_tick - 1;
    let resident = run_coroutine_json(
        &path,
        "direct",
        live_tick,
        "detailed",
        1,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x8000000c", "0x80000010"],
        "unexpected lookahead-one resident ROB at tick {live_tick}: {resident}"
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1),
        "expected one resident LSQ entry at tick {live_tick}: {resident}"
    );
    assert_terminal_coroutine_frontend(&resident, "0x8000001c", &["0x80000014"]);
}

#[test]
fn rem6_run_o3_same_window_overwritten_coroutine_source_stays_terminal() {
    let path = overwritten_coroutine_binary("o3-same-window-overwritten-coroutine");
    let completed = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, "x1"),
        0x8000_0034,
        "unexpected completed x1: {completed}"
    );
    assert_eq!(
        register_value(&completed, "x5"),
        0x8000_0028,
        "unexpected completed x5: {completed}"
    );
    assert_eq!(
        register_value(&completed, "x13"),
        0x8000_0028,
        "unexpected completed x13: {completed}"
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000280000800000000000000000"),
        "unexpected completed memory: {completed}"
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_no_data_address(&completed, WRONG_STORE_12_ADDRESS);

    let load = event_at_pc(&completed, "0x8000000c");
    let call = event_at_pc(&completed, "0x80000010");
    let overwrite = event_at_pc(&completed, "0x80000020");
    let coroutine = event_at_pc(&completed, "0x80000024");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    for (name, event) in [
        ("call", call),
        ("overwrite", overwrite),
        ("coroutine", coroutine),
    ] {
        assert!(
            event_u64(event, "issue_tick") < response_tick,
            "{name} must issue before the load response at tick {response_tick}: {event}"
        );
    }
    assert!(
        event_u64(coroutine, "issue_tick") >= event_u64(overwrite, "writeback_tick"),
        "coroutine must issue no earlier than the x1 overwrite writeback: overwrite={overwrite}, coroutine={coroutine}"
    );

    let live_tick = response_tick - 1;
    assert!(
        live_tick < response_tick,
        "coroutine live tick {live_tick} must precede load response tick {response_tick}: load={load}, coroutine={coroutine}"
    );
    let resident = run_coroutine_json(
        &path,
        "direct",
        live_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x8000000c", "0x80000010", "0x80000020", "0x80000024"],
        "unexpected overwritten-coroutine resident ROB at tick {live_tick}: {resident}"
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1),
        "expected one resident LSQ entry at tick {live_tick}: {resident}"
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000020", 1);
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000024", 5);
    assert_terminal_coroutine_frontend(&resident, "0x80000020", &["0x80000014", "0x80000034"]);
}
