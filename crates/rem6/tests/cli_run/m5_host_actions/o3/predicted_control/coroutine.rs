use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_ordered_commits, assert_register_absent_or_zero, assert_stopped_by_host,
    finish_control_window_binary, resident_rob_pcs, run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const INDIRECT_COROUTINE_TARGET_PC: i32 = 0x24;
const DATA_ADDRESS: &str = "0x80000100";
const SUCCESS_STORE_ADDRESS: &str = "0x80000104";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const WRONG_STORE_12_ADDRESS: &str = "0x8000010c";
const DIRECT_WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];

fn assert_terminal_coroutine_frontend(resident: &Value, fetched_pc: &str, suppressed_pcs: &[&str]) {
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/ras/pushes", 1),
        ("/cores/0/branch_predictor/lookups/return", 0),
        ("/cores/0/branch_predictor/ras/pops", 0),
        ("/cores/0/branch_predictor/ras/used", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 0),
    ] {
        assert_eq!(
            resident.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "unexpected terminal-coroutine predictor evidence at {pointer}: {resident}"
        );
    }
    assert!(
        resident
            .pointer("/debug/fetch_trace")
            .and_then(Value::as_array)
            .is_some_and(|records| records.iter().any(|record| {
                record.pointer("/pc").and_then(Value::as_str) == Some(fetched_pc)
            })),
        "expected positive fetch witness for {fetched_pc}: {resident}"
    );
    for pc in suppressed_pcs {
        assert_no_fetch_pc(resident, pc);
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_commits_direct() {
    let path = direct_coroutine_binary("o3-same-window-coroutine-direct", 0);
    let completed = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0014);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0020);
    assert_eq!(register_value(&completed, "x13"), 0x8000_0020);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x8000000c");
    let call = event_at_pc(&completed, "0x80000010");
    let coroutine = event_at_pc(&completed, "0x8000001c");
    let descendant = event_at_pc(&completed, "0x80000014");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    for event in [call, coroutine, descendant] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(event_u64(descendant, "issue_tick") > event_u64(coroutine, "writeback_tick"));
    assert_ordered_commits([load, call, coroutine, descendant]);
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/max_ready_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(1)
    );

    let live_tick = event_u64(descendant, "issue_tick") + 1;
    assert!(live_tick < response_tick);
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
        ["0x8000000c", "0x80000010", "0x8000001c", "0x80000014"]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000010", 1);
    assert_integer_rename_maps_to_row_destination(&resident, "0x8000001c", 5);
    assert_direct_memory_activity(&resident);

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 1),
        ("/cores/0/branch_predictor/ras/used", 1),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 1),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "expected {pointer}={expected}: {completed}"
        );
    }
}

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
        Some("0x80000020")
    );
    assert_eq!(
        later_return
            .pointer("/branch_resolved_target")
            .and_then(Value::as_str),
        Some("0x80000020")
    );
    assert_eq!(
        later_return
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
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

#[test]
fn rem6_run_o3_same_window_indirect_coroutine_commits_cache_fabric_dram() {
    let path = indirect_coroutine_binary("o3-same-window-indirect-coroutine");
    let completed = run_coroutine_json(
        &path,
        "cache-fabric-dram",
        3_000,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x5"), 0x8000_001c);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0028);
    assert_eq!(register_value(&completed, "x13"), 0x8000_0028);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000280000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x80000014");
    let call = event_at_pc(&completed, "0x80000018");
    let coroutine = event_at_pc(&completed, "0x80000024");
    let descendant = event_at_pc(&completed, "0x8000001c");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_indirect", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    for event in [call, coroutine, descendant] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(event_u64(descendant, "issue_tick") > event_u64(coroutine, "writeback_tick"));
    assert_ordered_commits([load, call, coroutine, descendant]);
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/max_ready_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(1)
    );

    let live_tick = event_u64(descendant, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_coroutine_json(
        &path,
        "cache-fabric-dram",
        live_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x80000014", "0x80000018", "0x80000024", "0x8000001c"]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000018", 5);
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000024", 1);

    let response_resident = run_coroutine_json(
        &path,
        "cache-fabric-dram",
        response_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_ADDRESS);
    assert_hierarchy_activity(&response_resident);

    assert_eq!(
        completed
            .pointer("/cores/0/branch_predictor/target_provider/indirect")
            .and_then(Value::as_u64),
        Some(1),
        "expected /cores/0/branch_predictor/target_provider/indirect=1: {completed}"
    );
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 1),
        ("/cores/0/branch_predictor/ras/used", 1),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 1),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "expected {pointer}={expected}: {completed}"
        );
    }
}

fn run_coroutine_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    extra_args: &[&str],
) -> Value {
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
        DATA_ADDRESS,
        16,
        extra_args,
    )
}

fn direct_coroutine_binary(name: &str, exit_padding_words: usize) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(0, 5, 0x0, 13, 0x13),
        j_type(16, 0),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        s_type(4, 13, 18, 0b010),
    ]);
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn overwritten_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(16, 1),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(32, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 5, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn older_branch_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(0x11, 0, 0x0, 1, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        b_type(28, 7, 7, 0b000),
        j_type(12, 1),
        i_type(0, 5, 0x0, 13, 0x13),
        s_type(8, 7, 18, 0b010),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0x33, 0, 0x0, 15, 0x13),
        s_type(4, 15, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn wrong_target_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(99, 0, 0x0, 14, 0x13),
        s_type(8, 7, 18, 0b010),
        i_type(20, 1, 0x0, 5, 0x67),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        i_type(0, 5, 0x0, 13, 0x13),
        i_type(0, 5, 0x0, 0, 0x67),
        m5op(M5_FAIL),
    ]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn indirect_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(
            INDIRECT_COROUTINE_TARGET_PC - target_auipc_pc,
            11,
            0x0,
            11,
            0x13,
        ),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
        i_type(0, 1, 0x0, 13, 0x13),
        j_type(16, 0),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
