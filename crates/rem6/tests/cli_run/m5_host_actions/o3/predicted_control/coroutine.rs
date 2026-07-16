use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_ordered_commits,
    assert_register_absent_or_zero, assert_stopped_by_host, finish_control_window_binary,
    resident_rob_pcs, run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const DIRECT_WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];

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
            Some(expected)
        );
    }
    assert_direct_memory_activity(&completed);
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

    assert_eq!(
        completed
            .pointer("/cores/0/branch_predictor/target_provider/indirect")
            .and_then(Value::as_u64),
        Some(1)
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
            Some(expected)
        );
    }
    assert_hierarchy_activity(&completed);
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
        i_type(0x24 - target_auipc_pc, 11, 0x0, 11, 0x13),
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
