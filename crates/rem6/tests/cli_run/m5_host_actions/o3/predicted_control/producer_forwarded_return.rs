use super::window_support::{
    assert_branch_kind_and_link, assert_control_prediction, assert_control_window_route_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_no_o3_stats, assert_ordered_commits, assert_register_absent_or_zero,
    assert_stopped_by_host, fetch_count_at_pc, fetch_tick_at_pc, finish_control_window_binary,
    resident_rob_pcs, run_control_window_json, ProducerForwardedLinkedCase,
    PRODUCER_FORWARDED_LINKED_CASES,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];

const RETURN_LOAD_PC: &str = "0x8000001c";
const RETURN_PRODUCER_PC: &str = "0x80000020";
const RETURN_CALL_PC: &str = "0x80000024";
const RETURN_LANDING_PC: &str = "0x80000028";
const RETURN_STORE_PC: &str = "0x8000002c";
const RETURN_STALE_TARGET_PC: &str = "0x80000040";
const RETURN_PC: &str = "0x80000044";
const RETURN_WRONG_FALLTHROUGH_PC: &str = "0x80000048";

fn producer_forwarded_return_binary(name: &str, target_source: u8, link: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(0x44 - target_auipc_pc, 10, 0x0, 10, 0x13),
        i_type(-4, 10, 0x0, target_source, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 10, 0x0, target_source, 0x13),
        i_type(0, target_source, 0x0, link, 0x67),
        i_type(0, link, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, link, 0x0, 0, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x50);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn run_return_json(
    path: &Path,
    case: ProducerForwardedLinkedCase,
    execution_mode: &str,
    branch_lookahead: usize,
) -> Value {
    run_control_window_json(
        path,
        case.memory_system,
        case.max_tick,
        execution_mode,
        branch_lookahead,
        DATA_ADDRESS,
        16,
        &WIDTH_ARGS,
    )
}

fn assert_producer_forwarded_return(case: ProducerForwardedLinkedCase) {
    let path = producer_forwarded_return_binary(
        &format!("o3-producer-forwarded-return-{}", case.label),
        case.target_source,
        case.link,
    );
    let completed = run_return_json(&path, case, "detailed", 2);

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, &format!("x{}", case.link)),
        0x8000_0028
    );
    assert_eq!(register_value(&completed, "x13"), 0x8000_0028);
    if case.target_source != case.link {
        assert_eq!(register_value(&completed, "x11"), 0x8000_0044);
    }
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000280000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, RETURN_LOAD_PC);
    let producer = event_at_pc(&completed, RETURN_PRODUCER_PC);
    let call = event_at_pc(&completed, RETURN_CALL_PC);
    let return_jump = event_at_pc(&completed, RETURN_PC);
    let landing = event_at_pc(&completed, RETURN_LANDING_PC);
    let store = event_at_pc(&completed, RETURN_STORE_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_indirect", true);
    assert_branch_kind_and_link(return_jump, "return", false);
    assert_control_prediction(call, RETURN_PC);
    assert_control_prediction(return_jump, RETURN_LANDING_PC);
    assert!(event_u64(producer, "issue_tick") < response_tick);
    assert!(event_u64(call, "issue_tick") < response_tick);
    let expected_return_issue_tick = match case.memory_system {
        "direct" => response_tick,
        "cache-fabric-dram" => response_tick + 10,
        other => panic!("unsupported producer-forwarded return memory system {other}"),
    };
    assert_eq!(
        event_u64(return_jump, "issue_tick"),
        expected_return_issue_tick
    );
    assert!(event_u64(landing, "issue_tick") >= event_u64(return_jump, "writeback_tick"));
    assert_ordered_commits([load, producer, call, return_jump, landing, store]);
    assert_no_fetch_pc(&completed, RETURN_STALE_TARGET_PC);
    assert_no_fetch_pc(&completed, RETURN_WRONG_FALLTHROUGH_PC);

    for pointer in [
        "/cores/0/branch_predictor/lookups/call_indirect",
        "/cores/0/branch_predictor/lookups/return",
        "/cores/0/branch_predictor/committed/call_indirect",
        "/cores/0/branch_predictor/committed/return",
        "/cores/0/branch_predictor/target_provider/indirect",
        "/cores/0/branch_predictor/target_provider/ras",
        "/cores/0/branch_predictor/ras/pushes",
        "/cores/0/branch_predictor/ras/pops",
        "/cores/0/branch_predictor/ras/used",
        "/cores/0/branch_predictor/ras/correct",
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(1),
            "expected exact producer-forwarded return evidence at {pointer}: {completed}"
        );
    }
    for pointer in [
        "/cores/0/branch_predictor/squashes/call_indirect",
        "/cores/0/branch_predictor/squashes/return",
        "/cores/0/branch_predictor/ras/incorrect",
    ] {
        assert_eq!(completed.pointer(pointer).and_then(Value::as_u64), Some(0));
    }

    let resident = run_control_window_json(
        &path,
        case.memory_system,
        response_tick - 1,
        "detailed",
        2,
        DATA_ADDRESS,
        16,
        &WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [RETURN_LOAD_PC, RETURN_PRODUCER_PC, RETURN_CALL_PC]
    );
    assert_eq!(fetch_count_at_pc(&resident, RETURN_PC), 1);
    assert!(fetch_tick_at_pc(&resident, RETURN_PC) < response_tick);
    assert_no_fetch_pc(&resident, RETURN_LANDING_PC);
    assert_no_fetch_pc(&resident, RETURN_STALE_TARGET_PC);
    assert_no_fetch_pc(&resident, RETURN_WRONG_FALLTHROUGH_PC);
    assert_eq!(
        register_value(&resident, &format!("x{}", case.target_source)),
        0x8000_0040
    );
    if case.target_source != case.link {
        assert_register_absent_or_zero(&resident, &format!("x{}", case.link));
        assert_integer_rename_maps_to_row_destination(
            &resident,
            RETURN_PRODUCER_PC,
            u64::from(case.target_source),
        );
    }
    assert_integer_rename_maps_to_row_destination(&resident, RETURN_CALL_PC, u64::from(case.link));
    assert_control_window_route_activity(&resident, case.memory_system);
}

#[test]
fn rem6_run_o3_producer_forwarded_return_descendants_cover_link_shape_route_matrix() {
    for case in PRODUCER_FORWARDED_LINKED_CASES {
        assert_producer_forwarded_return(case);
    }
}

#[test]
fn rem6_run_o3_producer_forwarded_return_requires_branch_lookahead_two() {
    let case = PRODUCER_FORWARDED_LINKED_CASES[2];
    let path = producer_forwarded_return_binary(
        "o3-producer-forwarded-return-lookahead-one",
        case.target_source,
        case.link,
    );
    let completed = run_return_json(&path, case, "detailed", 1);
    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0028);
    assert_eq!(register_value(&completed, "x11"), 0x8000_0044);

    let response_tick = event_u64(
        event_at_pc(&completed, RETURN_LOAD_PC),
        "lsq_data_response_tick",
    );
    let resident = run_control_window_json(
        &path,
        case.memory_system,
        response_tick - 1,
        "detailed",
        1,
        DATA_ADDRESS,
        16,
        &WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [RETURN_LOAD_PC, RETURN_PRODUCER_PC, RETURN_CALL_PC]
    );
    assert_eq!(fetch_count_at_pc(&resident, RETURN_PC), 1);
    assert_no_fetch_pc(&resident, RETURN_LANDING_PC);
    for pointer in [
        "/cores/0/branch_predictor/lookups/return",
        "/cores/0/branch_predictor/target_provider/ras",
        "/cores/0/branch_predictor/ras/pops",
        "/cores/0/branch_predictor/ras/used",
    ] {
        assert_eq!(resident.pointer(pointer).and_then(Value::as_u64), Some(0));
    }
}

#[test]
fn rem6_run_timing_suppresses_o3_producer_forwarded_returns() {
    for case in [
        PRODUCER_FORWARDED_LINKED_CASES[2],
        PRODUCER_FORWARDED_LINKED_CASES[3],
    ] {
        let path = producer_forwarded_return_binary(
            &format!("o3-producer-forwarded-return-timing-{}", case.label),
            case.target_source,
            case.link,
        );
        let timing = run_return_json(&path, case, "timing", 2);

        assert_stopped_by_host(&timing);
        assert_eq!(
            register_value(&timing, &format!("x{}", case.link)),
            0x8000_0028
        );
        assert_eq!(register_value(&timing, "x11"), 0x8000_0044);
        assert_eq!(register_value(&timing, "x13"), 0x8000_0028);
        assert_eq!(
            timing.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a000000280000800000000000000000")
        );
        assert!(timing.pointer("/cores/0/o3_runtime").is_none());
        assert!(timing
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
        assert_no_o3_stats(&timing);
    }
}
