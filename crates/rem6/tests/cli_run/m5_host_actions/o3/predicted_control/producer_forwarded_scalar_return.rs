use super::window_support::{
    assert_branch_kind_and_link, assert_control_prediction, assert_direct_memory_activity,
    assert_hierarchy_activity, assert_integer_rename_maps_to_row_destination,
    assert_no_data_address, assert_no_fetch_pc, assert_no_o3_stats, assert_ordered_commits,
    assert_stopped_by_host, fetch_count_at_pc, fetch_tick_at_pc, finish_control_window_binary,
    resident_rob_pcs, run_control_window_json,
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

const SCALAR_RETURN_LOAD_PC: &str = "0x8000001c";
const SCALAR_RETURN_PRODUCER_PC: &str = "0x80000020";
const SCALAR_RETURN_CALL_PC: &str = "0x80000024";
const SCALAR_RETURN_LANDING_PC: &str = "0x80000028";
const SCALAR_RETURN_STORE_PC: &str = "0x8000002c";
const SCALAR_RETURN_STALE_TARGET_PC: &str = "0x80000040";
const SCALAR_RETURN_SCALAR_PC: &str = "0x80000044";
const SCALAR_RETURN_PC: &str = "0x80000048";
const SCALAR_RETURN_WRONG_FALLTHROUGH_PC: &str = "0x8000004c";

#[derive(Clone, Copy)]
struct ProducerForwardedScalarReturnCase {
    label: &'static str,
    target_source: u8,
    link: u8,
    memory_system: &'static str,
    max_tick: u64,
}

const PRODUCER_FORWARDED_SCALAR_RETURN_CASES: [ProducerForwardedScalarReturnCase; 4] = [
    ProducerForwardedScalarReturnCase {
        label: "same-link-x1-direct",
        target_source: 1,
        link: 1,
        memory_system: "direct",
        max_tick: 2_500,
    },
    ProducerForwardedScalarReturnCase {
        label: "same-link-x5-hierarchy",
        target_source: 5,
        link: 5,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
    },
    ProducerForwardedScalarReturnCase {
        label: "split-link-x5-direct",
        target_source: 11,
        link: 5,
        memory_system: "direct",
        max_tick: 2_500,
    },
    ProducerForwardedScalarReturnCase {
        label: "split-link-x1-hierarchy",
        target_source: 11,
        link: 1,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
    },
];

fn producer_forwarded_scalar_return_binary(
    name: &str,
    target_source: u8,
    link: u8,
    scalar_consumes_link: bool,
) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    let scalar = if scalar_consumes_link {
        i_type(0, link, 0x0, 13, 0x13)
    } else {
        i_type(7, 0, 0x0, 13, 0x13)
    };
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
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        m5op(M5_FAIL),
        scalar,
        i_type(0, link, 0x0, 0, 0x67),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x50);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn run_scalar_return_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
) -> Value {
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
        DATA_ADDRESS,
        16,
        &WIDTH_ARGS,
    )
}

fn assert_route_activity(json: &Value, memory_system: &str) {
    match memory_system {
        "direct" => assert_direct_memory_activity(json),
        "cache-fabric-dram" => assert_hierarchy_activity(json),
        other => panic!("unsupported producer-forwarded scalar-return route {other}"),
    }
}

fn assert_producer_forwarded_scalar_return(case: ProducerForwardedScalarReturnCase) {
    let path = producer_forwarded_scalar_return_binary(
        &format!("o3-producer-forwarded-scalar-return-{}", case.label),
        case.target_source,
        case.link,
        true,
    );
    let completed = run_scalar_return_json(&path, case.memory_system, case.max_tick, "detailed", 2);

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

    let load = event_at_pc(&completed, SCALAR_RETURN_LOAD_PC);
    let producer = event_at_pc(&completed, SCALAR_RETURN_PRODUCER_PC);
    let call = event_at_pc(&completed, SCALAR_RETURN_CALL_PC);
    let scalar = event_at_pc(&completed, SCALAR_RETURN_SCALAR_PC);
    let return_jump = event_at_pc(&completed, SCALAR_RETURN_PC);
    let landing = event_at_pc(&completed, SCALAR_RETURN_LANDING_PC);
    let store = event_at_pc(&completed, SCALAR_RETURN_STORE_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_indirect", true);
    assert_branch_kind_and_link(return_jump, "return", false);
    assert_control_prediction(call, SCALAR_RETURN_SCALAR_PC);
    assert_control_prediction(return_jump, SCALAR_RETURN_LANDING_PC);
    assert!(event_u64(producer, "issue_tick") < response_tick);
    assert!(event_u64(call, "issue_tick") < response_tick);
    let expected_scalar_issue_tick = match case.memory_system {
        "direct" => response_tick,
        "cache-fabric-dram" => response_tick + 1,
        other => panic!("unsupported producer-forwarded scalar-return memory system {other}"),
    };
    assert_eq!(event_u64(scalar, "issue_tick"), expected_scalar_issue_tick);
    assert_eq!(event_u64(scalar, "iew_dependency_producers"), 1);
    assert!(event_u64(return_jump, "issue_tick") >= response_tick);
    assert!(event_u64(landing, "issue_tick") >= event_u64(return_jump, "writeback_tick"));
    assert_ordered_commits([load, producer, call, scalar, return_jump, landing, store]);
    let max_rob_occupancy = json_stat_u64(&completed, "sim.cpu0.o3.max_rob_occupancy");
    assert!(
        (3..=4).contains(&max_rob_occupancy),
        "unexpected scalar-return max ROB occupancy for {}: {max_rob_occupancy}",
        case.label,
    );
    assert_no_fetch_pc(&completed, SCALAR_RETURN_STALE_TARGET_PC);
    assert_no_fetch_pc(&completed, SCALAR_RETURN_WRONG_FALLTHROUGH_PC);

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
            "expected exact scalar-return predictor evidence at {pointer}: {completed}"
        );
    }
    for pointer in [
        "/cores/0/branch_predictor/squashes/call_indirect",
        "/cores/0/branch_predictor/squashes/return",
        "/cores/0/branch_predictor/ras/incorrect",
    ] {
        assert_eq!(completed.pointer(pointer).and_then(Value::as_u64), Some(0));
    }

    let resident =
        run_scalar_return_json(&path, case.memory_system, response_tick - 1, "detailed", 2);
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            SCALAR_RETURN_LOAD_PC,
            SCALAR_RETURN_PRODUCER_PC,
            SCALAR_RETURN_CALL_PC,
        ]
    );
    assert_eq!(fetch_count_at_pc(&resident, SCALAR_RETURN_SCALAR_PC), 1);
    assert!(fetch_tick_at_pc(&resident, SCALAR_RETURN_SCALAR_PC) < response_tick);
    assert_no_fetch_pc(&resident, SCALAR_RETURN_PC);
    assert_no_fetch_pc(&resident, SCALAR_RETURN_LANDING_PC);
    assert_no_fetch_pc(&resident, SCALAR_RETURN_STALE_TARGET_PC);
    assert_no_fetch_pc(&resident, SCALAR_RETURN_WRONG_FALLTHROUGH_PC);
    assert_integer_rename_maps_to_row_destination(
        &resident,
        SCALAR_RETURN_CALL_PC,
        u64::from(case.link),
    );
    assert_route_activity(&resident, case.memory_system);
}

#[test]
fn rem6_run_o3_producer_forwarded_scalar_returns_cover_link_shape_route_matrix() {
    for case in PRODUCER_FORWARDED_SCALAR_RETURN_CASES {
        assert_producer_forwarded_scalar_return(case);
    }
}

#[test]
fn rem6_run_o3_producer_forwarded_scalar_return_lookahead_one_keeps_return_unfetched() {
    let case = PRODUCER_FORWARDED_SCALAR_RETURN_CASES[2];
    let path = producer_forwarded_scalar_return_binary(
        "o3-producer-forwarded-scalar-return-lookahead-one",
        case.target_source,
        case.link,
        true,
    );
    let completed = run_scalar_return_json(&path, case.memory_system, case.max_tick, "detailed", 1);
    assert_stopped_by_host(&completed);
    let response_tick = event_u64(
        event_at_pc(&completed, SCALAR_RETURN_LOAD_PC),
        "lsq_data_response_tick",
    );
    let resident =
        run_scalar_return_json(&path, case.memory_system, response_tick - 1, "detailed", 1);
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            SCALAR_RETURN_LOAD_PC,
            SCALAR_RETURN_PRODUCER_PC,
            SCALAR_RETURN_CALL_PC,
        ]
    );
    assert_no_fetch_pc(&resident, SCALAR_RETURN_PC);
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
fn rem6_run_o3_producer_forwarded_scalar_return_rejects_non_link_scalar() {
    let case = PRODUCER_FORWARDED_SCALAR_RETURN_CASES[2];
    let path = producer_forwarded_scalar_return_binary(
        "o3-producer-forwarded-scalar-return-non-link-scalar",
        case.target_source,
        case.link,
        false,
    );
    let completed = run_scalar_return_json(&path, case.memory_system, case.max_tick, "detailed", 2);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0028);
    assert_eq!(register_value(&completed, "x11"), 0x8000_0044);
    assert_eq!(register_value(&completed, "x13"), 7);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000070000000000000000000000")
    );
    let load = event_at_pc(&completed, SCALAR_RETURN_LOAD_PC);
    let scalar = event_at_pc(&completed, SCALAR_RETURN_SCALAR_PC);
    assert_eq!(event_u64(scalar, "iew_dependency_producers"), 0);

    let resident = run_scalar_return_json(
        &path,
        case.memory_system,
        event_u64(load, "lsq_data_response_tick") - 1,
        "detailed",
        2,
    );
    assert_no_fetch_pc(&resident, SCALAR_RETURN_PC);
}

#[test]
fn rem6_run_timing_suppresses_o3_producer_forwarded_scalar_returns() {
    for case in [
        PRODUCER_FORWARDED_SCALAR_RETURN_CASES[2],
        PRODUCER_FORWARDED_SCALAR_RETURN_CASES[3],
    ] {
        let path = producer_forwarded_scalar_return_binary(
            &format!("o3-producer-forwarded-scalar-return-timing-{}", case.label),
            case.target_source,
            case.link,
            true,
        );
        let timing = run_scalar_return_json(&path, case.memory_system, case.max_tick, "timing", 2);

        assert_stopped_by_host(&timing);
        assert_eq!(
            register_value(&timing, &format!("x{}", case.link)),
            0x8000_0028
        );
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
