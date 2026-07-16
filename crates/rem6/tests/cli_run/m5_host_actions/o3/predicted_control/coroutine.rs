use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_no_o3_stats_with_context, assert_ordered_commits, assert_register_absent_or_zero,
    assert_register_absent_or_zero_with_context, assert_stopped_by_host, control_window_command,
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

#[derive(Clone, Copy)]
struct CoroutineLifecycleCase {
    label: &'static str,
    binary: fn(&str, usize) -> std::path::PathBuf,
    memory_system: &'static str,
    max_tick: u64,
    load_pc: &'static str,
    call_pc: &'static str,
    coroutine_pc: &'static str,
    descendant_pc: &'static str,
    success_store_pc: &'static str,
    call_kind: &'static str,
    call_destination: u8,
    coroutine_destination: u8,
    final_x1: u64,
    final_x5: u64,
    final_x13: u64,
    memory_hex: &'static str,
    provider_no_target: u64,
    provider_indirect: u64,
}

const COROUTINE_LIFECYCLE_CASES: [CoroutineLifecycleCase; 2] = [
    CoroutineLifecycleCase {
        label: "forward-direct",
        binary: direct_coroutine_binary,
        memory_system: "direct",
        max_tick: 2_500,
        load_pc: "0x8000000c",
        call_pc: "0x80000010",
        coroutine_pc: "0x8000001c",
        descendant_pc: "0x80000014",
        success_store_pc: "0x80000028",
        call_kind: "call_direct",
        call_destination: 1,
        coroutine_destination: 5,
        final_x1: 0x8000_0014,
        final_x5: 0x8000_0020,
        final_x13: 0x8000_0020,
        memory_hex: "2a000000200000800000000000000000",
        provider_no_target: 1,
        provider_indirect: 0,
    },
    CoroutineLifecycleCase {
        label: "reverse-indirect",
        binary: indirect_coroutine_binary,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
        load_pc: "0x80000014",
        call_pc: "0x80000018",
        coroutine_pc: "0x80000024",
        descendant_pc: "0x8000001c",
        success_store_pc: "0x80000030",
        call_kind: "call_indirect",
        call_destination: 5,
        coroutine_destination: 1,
        final_x1: 0x8000_0028,
        final_x5: 0x8000_001c,
        final_x13: 0x8000_0028,
        memory_hex: "2a000000280000800000000000000000",
        provider_no_target: 0,
        provider_indirect: 1,
    },
];

#[test]
fn rem6_run_o3_same_window_coroutine_commits_direct() {
    let case = COROUTINE_LIFECYCLE_CASES[0];
    let path = (case.binary)(
        &format!("o3-same-window-coroutine-commits-{}", case.label),
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
    assert_eq!(register_value(&completed, "x1"), case.final_x1);
    assert_eq!(register_value(&completed, "x5"), case.final_x5);
    assert_eq!(register_value(&completed, "x13"), case.final_x13);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex)
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, case.load_pc);
    let call = event_at_pc(&completed, case.call_pc);
    let coroutine = event_at_pc(&completed, case.coroutine_pc);
    let descendant = event_at_pc(&completed, case.descendant_pc);
    let success_store = event_at_pc(&completed, case.success_store_pc);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, case.call_kind, true);
    assert_branch_kind_and_link(coroutine, "return", true);
    for event in [call, coroutine, descendant] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(event_u64(descendant, "issue_tick") > event_u64(coroutine, "writeback_tick"));
    assert_ordered_commits([load, call, coroutine, descendant, success_store]);
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
        case.memory_system,
        live_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            case.load_pc,
            case.call_pc,
            case.coroutine_pc,
            case.descendant_pc,
        ]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.call_pc,
        u64::from(case.call_destination),
    );
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.coroutine_pc,
        u64::from(case.coroutine_destination),
    );
    assert_direct_memory_activity(&resident);

    for (pointer, expected) in [
        (
            "/cores/0/branch_predictor/target_provider/no_target",
            case.provider_no_target,
        ),
        (
            "/cores/0/branch_predictor/target_provider/indirect",
            case.provider_indirect,
        ),
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
fn rem6_run_o3_same_window_indirect_coroutine_commits_cache_fabric_dram() {
    let case = COROUTINE_LIFECYCLE_CASES[1];
    let path = (case.binary)(
        &format!("o3-same-window-coroutine-commits-{}", case.label),
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
    assert_eq!(register_value(&completed, "x1"), case.final_x1);
    assert_eq!(register_value(&completed, "x5"), case.final_x5);
    assert_eq!(register_value(&completed, "x13"), case.final_x13);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex)
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, case.load_pc);
    let call = event_at_pc(&completed, case.call_pc);
    let coroutine = event_at_pc(&completed, case.coroutine_pc);
    let descendant = event_at_pc(&completed, case.descendant_pc);
    let success_store = event_at_pc(&completed, case.success_store_pc);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, case.call_kind, true);
    assert_branch_kind_and_link(coroutine, "return", true);
    for event in [call, coroutine, descendant] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(event_u64(descendant, "issue_tick") > event_u64(coroutine, "writeback_tick"));
    assert_ordered_commits([load, call, coroutine, descendant, success_store]);
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
        case.memory_system,
        live_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            case.load_pc,
            case.call_pc,
            case.coroutine_pc,
            case.descendant_pc,
        ]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.call_pc,
        u64::from(case.call_destination),
    );
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.coroutine_pc,
        u64::from(case.coroutine_destination),
    );

    let response_resident = run_coroutine_json(
        &path,
        case.memory_system,
        response_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_ADDRESS);
    assert_hierarchy_activity(&response_resident);

    for (pointer, expected) in [
        (
            "/cores/0/branch_predictor/target_provider/no_target",
            case.provider_no_target,
        ),
        (
            "/cores/0/branch_predictor/target_provider/indirect",
            case.provider_indirect,
        ),
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

include!("coroutine/suppression.rs");
include!("coroutine/repair.rs");
include!("coroutine/lifecycle.rs");

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

fn indirect_coroutine_prefix() -> Vec<u32> {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    let target_offset = INDIRECT_COROUTINE_TARGET_PC - target_auipc_pc;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(target_offset, 11, 0x0, 11, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
    ]);
    words
}

fn overwritten_indirect_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = indirect_coroutine_prefix();
    words.extend([
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(24, 5, 0x0, 5, 0x13),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 1, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn indirect_coroutine_binary(name: &str, exit_padding_words: usize) -> std::path::PathBuf {
    let mut words = indirect_coroutine_prefix();
    words.extend([
        i_type(0, 1, 0x0, 13, 0x13),
        j_type(16, 0),
        i_type(0, 5, 0x0, 1, 0x67),
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
