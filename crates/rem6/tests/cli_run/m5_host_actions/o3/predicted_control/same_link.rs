use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_ordered_commits, assert_register_absent_or_zero, assert_stopped_by_host,
    fetch_count_at_pc, fetch_tick_at_pc, finish_control_window_binary, resident_rob_pcs,
    run_control_window_json,
};
use super::*;

#[path = "same_link/return_descendant.rs"]
mod return_descendant;
#[path = "same_link/scalar_return.rs"]
mod scalar_return;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const SUCCESS_STORE_ADDRESS: &str = "0x80000104";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const WRONG_STORE_12_ADDRESS: &str = "0x8000010c";
const WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];

const POSITIVE_LOAD_PC: &str = "0x80000018";
const POSITIVE_CALL_PC: &str = "0x8000001c";
const POSITIVE_TARGET_PC: &str = "0x8000002c";
const POSITIVE_DESCENDANT_PC: &str = "0x80000030";
const POSITIVE_STORE_PC: &str = "0x80000034";
const POSITIVE_TARGET: u64 = 0x8000_002c;
const POSITIVE_LINK: u64 = 0x8000_0020;

#[derive(Clone, Copy)]
struct SameLinkCase {
    label: &'static str,
    link: u8,
    memory_system: &'static str,
    max_tick: u64,
}

const SAME_LINK_CASES: [SameLinkCase; 4] = [
    SameLinkCase {
        label: "x1-direct",
        link: 1,
        memory_system: "direct",
        max_tick: 2_500,
    },
    SameLinkCase {
        label: "x5-direct",
        link: 5,
        memory_system: "direct",
        max_tick: 2_500,
    },
    SameLinkCase {
        label: "x1-cache-fabric-dram",
        link: 1,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
    },
    SameLinkCase {
        label: "x5-cache-fabric-dram",
        link: 5,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
    },
];

fn run_same_link_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    branch_lookahead: usize,
) -> Value {
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        "detailed",
        branch_lookahead,
        DATA_ADDRESS,
        16,
        &WIDTH_ARGS,
    )
}

fn same_link_binary(name: &str, link: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    assert_eq!(target_auipc_pc, 0x10);
    words.extend([
        u_type(0, link, 0x17),
        i_type(0x2c - target_auipc_pc, link, 0x0, link, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, link, 0x0, link, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, link, 0x0, 13, 0x13),
        i_type(1, 13, 0x0, 14, 0x13),
        s_type(4, 14, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x40);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn assert_route_activity(json: &Value, memory_system: &str) {
    match memory_system {
        "direct" => assert_direct_memory_activity(json),
        "cache-fabric-dram" => assert_hierarchy_activity(json),
        other => panic!("unsupported same-link memory system {other}"),
    }
}

fn assert_same_link_predictor_evidence(json: &Value, committed: u64, squashed: u64) {
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_indirect", 1),
        ("/cores/0/branch_predictor/target_provider/indirect", 1),
        ("/cores/0/branch_predictor/target_provider/ras", 0),
        (
            "/cores/0/branch_predictor/committed/call_indirect",
            committed,
        ),
        ("/cores/0/branch_predictor/squashes/call_indirect", squashed),
        ("/cores/0/branch_predictor/ras/used", 0),
        ("/cores/0/branch_predictor/ras/correct", 0),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "unexpected same-link predictor evidence at {pointer}: {json}"
        );
    }
}

fn assert_same_link_commits(case: SameLinkCase) {
    let path = same_link_binary(&format!("o3-same-link-{}", case.label), case.link);
    let completed = run_same_link_json(&path, case.memory_system, case.max_tick, 1);

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, &format!("x{}", case.link)),
        POSITIVE_LINK
    );
    assert_eq!(register_value(&completed, "x13"), POSITIVE_LINK);
    assert_eq!(register_value(&completed, "x14"), POSITIVE_LINK + 1);
    let other_link = if case.link == 1 { "x5" } else { "x1" };
    assert_register_absent_or_zero(&completed, other_link);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000210000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, POSITIVE_LOAD_PC);
    let call = event_at_pc(&completed, POSITIVE_CALL_PC);
    let target = event_at_pc(&completed, POSITIVE_TARGET_PC);
    let descendant = event_at_pc(&completed, POSITIVE_DESCENDANT_PC);
    let store = event_at_pc(&completed, POSITIVE_STORE_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_indirect", true);
    for event in [call, target, descendant] {
        assert!(
            event_u64(event, "issue_tick") < response_tick,
            "{} must issue before load response at tick {response_tick}: {event}",
            case.label
        );
    }
    assert!(event_u64(target, "issue_tick") >= event_u64(call, "writeback_tick"));
    assert!(event_u64(descendant, "issue_tick") >= event_u64(target, "writeback_tick"));
    assert_ordered_commits([load, call, target, descendant, store]);
    assert_eq!(
        call.pointer("/branch_predicted_target")
            .and_then(Value::as_str),
        None
    );
    assert_eq!(
        call.pointer("/branch_resolved_target")
            .and_then(Value::as_str),
        Some("0x8000002c")
    );
    assert_eq!(
        call.pointer("/branch_repair").and_then(Value::as_str),
        Some("direction_only")
    );
    assert_eq!(
        call.pointer("/branch_squashed_target")
            .and_then(Value::as_str),
        Some("0x80000020")
    );
    for (field, expected) in [
        ("branch_predicted_taken", false),
        ("branch_resolved_taken", true),
        ("branch_wrong_target", false),
        ("branch_mispredicted", true),
        ("branch_squash", true),
    ] {
        assert_eq!(
            call.pointer(&format!("/{field}")).and_then(Value::as_bool),
            Some(expected),
            "unexpected same-link branch flag {field}: {call}"
        );
    }
    assert_same_link_predictor_evidence(&completed, 1, 0);
    assert_eq!(
        completed
            .pointer("/cores/0/branch_predictor/ras/pushes")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        completed
            .pointer("/cores/0/branch_predictor/ras/pops")
            .and_then(Value::as_u64),
        Some(0)
    );

    let live_tick = event_u64(descendant, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_same_link_json(&path, case.memory_system, live_tick, 1);
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            POSITIVE_LOAD_PC,
            POSITIVE_CALL_PC,
            POSITIVE_TARGET_PC,
            POSITIVE_DESCENDANT_PC,
        ]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        register_value(&resident, &format!("x{}", case.link)),
        POSITIVE_TARGET,
        "architectural link must retain the committed target while the call is live"
    );
    assert_integer_rename_maps_to_row_destination(
        &resident,
        POSITIVE_CALL_PC,
        u64::from(case.link),
    );
    assert_integer_rename_maps_to_row_destination(&resident, POSITIVE_TARGET_PC, 13);
    assert_integer_rename_maps_to_row_destination(&resident, POSITIVE_DESCENDANT_PC, 14);
    assert_no_data_address(&resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&resident, WRONG_STORE_ADDRESS);

    let response_resident = run_same_link_json(&path, case.memory_system, response_tick, 1);
    assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_ADDRESS);
    assert_route_activity(&response_resident, case.memory_system);
}

#[test]
fn rem6_run_o3_committed_same_link_calls_cover_link_and_route_matrix() {
    for case in SAME_LINK_CASES {
        assert_same_link_commits(case);
    }
}

const LIVE_LOAD_PC: &str = "0x8000001c";
const LIVE_PRODUCER_PC: &str = "0x80000020";
const LIVE_CALL_PC: &str = "0x80000024";
const LIVE_STALE_TARGET_PC: &str = "0x80000030";
const LIVE_TARGET_PC: &str = "0x80000034";

fn live_same_link_target_binary(name: &str, link: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0x34 - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(-4, 11, 0x0, link, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, link, 0x13),
        i_type(0, link, 0x0, link, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, link, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x44);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

#[test]
fn rem6_run_o3_live_same_link_targets_forward_across_link_and_route_matrix() {
    for case in SAME_LINK_CASES {
        let path = live_same_link_target_binary(
            &format!("o3-live-same-link-target-{}", case.label),
            case.link,
        );
        let completed = run_same_link_json(&path, case.memory_system, case.max_tick, 1);
        assert_stopped_by_host(&completed);
        assert_eq!(
            register_value(&completed, &format!("x{}", case.link)),
            0x8000_0028
        );
        assert_eq!(register_value(&completed, "x13"), 0x8000_0028);
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a000000280000800000000000000000")
        );
        assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

        let load = event_at_pc(&completed, LIVE_LOAD_PC);
        let producer = event_at_pc(&completed, LIVE_PRODUCER_PC);
        let call = event_at_pc(&completed, LIVE_CALL_PC);
        let target = event_at_pc(&completed, LIVE_TARGET_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(call, "call_indirect", true);
        assert_eq!(
            call.pointer("/branch_predicted_target")
                .and_then(Value::as_str),
            Some(LIVE_TARGET_PC)
        );
        assert_eq!(
            call.pointer("/branch_resolved_target")
                .and_then(Value::as_str),
            Some(LIVE_TARGET_PC)
        );
        assert_eq!(
            call.pointer("/branch_repair").and_then(Value::as_str),
            Some("none")
        );
        for (field, expected) in [
            ("branch_predicted_taken", true),
            ("branch_resolved_taken", true),
            ("branch_wrong_target", false),
            ("branch_mispredicted", false),
            ("branch_squash", false),
        ] {
            assert_eq!(
                call.pointer(&format!("/{field}")).and_then(Value::as_bool),
                Some(expected),
                "unexpected live same-link branch flag {field}: {call}"
            );
        }
        assert_same_link_predictor_evidence(&completed, 1, 0);
        assert_eq!(
            completed
                .pointer("/cores/0/branch_predictor/ras/pushes")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            completed
                .pointer("/cores/0/branch_predictor/ras/pops")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert!(event_u64(producer, "issue_tick") < response_tick);
        assert!(event_u64(call, "issue_tick") >= event_u64(producer, "writeback_tick"));
        assert!(event_u64(call, "issue_tick") < response_tick);
        let expected_target_issue_tick = match case.memory_system {
            "direct" => response_tick,
            "cache-fabric-dram" => response_tick + 1,
            other => panic!("unsupported same-link memory system {other}"),
        };
        assert_eq!(
            event_u64(target, "issue_tick"),
            expected_target_issue_tick,
            "live same-link target issue tick must match the route boundary: {target}"
        );

        let live_tick = response_tick - 1;
        let resident = run_same_link_json(&path, case.memory_system, live_tick, 1);
        assert_eq!(
            resident_rob_pcs(&resident),
            [LIVE_LOAD_PC, LIVE_PRODUCER_PC, LIVE_CALL_PC]
        );
        assert_eq!(fetch_count_at_pc(&resident, LIVE_TARGET_PC), 1);
        assert!(fetch_tick_at_pc(&resident, LIVE_TARGET_PC) < response_tick);
        assert_no_fetch_pc(&resident, LIVE_STALE_TARGET_PC);
        assert_eq!(
            register_value(&resident, &format!("x{}", case.link)),
            0x8000_0030,
            "architectural link must retain the stale committed target while the live call owns rename state"
        );
        assert_integer_rename_maps_to_row_destination(
            &resident,
            LIVE_CALL_PC,
            u64::from(case.link),
        );
        assert_no_data_address(&resident, SUCCESS_STORE_ADDRESS);
        assert_no_data_address(&resident, WRONG_STORE_ADDRESS);
        assert_route_activity(&resident, case.memory_system);
    }
}

fn unresolved_live_same_link_target_binary(name: &str, link: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);
    let stale_target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, link, 0x17),
        i_type(0x30 - stale_target_auipc_pc, link, 0x0, link, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 18, 0b110, 12, 0x03),
        i_type(0x34, 12, 0x0, link, 0x13),
        i_type(0, link, 0x0, link, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, link, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x44);
    finish_control_window_binary(name, words, DATA_START as usize, [0x8000_0000, 0, 0, 0])
}

#[test]
fn rem6_run_o3_unresolved_live_same_link_targets_stay_terminal() {
    for case in [SAME_LINK_CASES[0], SAME_LINK_CASES[3]] {
        let path = unresolved_live_same_link_target_binary(
            &format!("o3-unresolved-live-same-link-target-{}", case.label),
            case.link,
        );
        let completed = run_same_link_json(&path, case.memory_system, case.max_tick, 1);
        assert_stopped_by_host(&completed);
        assert_eq!(
            register_value(&completed, &format!("x{}", case.link)),
            0x8000_0028
        );
        assert_eq!(register_value(&completed, "x13"), 0x8000_0028);

        let load = event_at_pc(&completed, LIVE_LOAD_PC);
        let call = event_at_pc(&completed, LIVE_CALL_PC);
        let target = event_at_pc(&completed, LIVE_TARGET_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert!(event_u64(call, "issue_tick") >= response_tick);
        assert!(event_u64(target, "issue_tick") > response_tick);

        let resident = run_same_link_json(&path, case.memory_system, response_tick - 1, 1);
        assert_eq!(
            resident_rob_pcs(&resident),
            [LIVE_LOAD_PC, LIVE_PRODUCER_PC]
        );
        assert_no_fetch_pc(&resident, LIVE_TARGET_PC);
        assert_no_fetch_pc(&resident, LIVE_STALE_TARGET_PC);
        assert_no_data_address(&resident, SUCCESS_STORE_ADDRESS);
        assert_no_data_address(&resident, WRONG_STORE_ADDRESS);
        assert_route_activity(&resident, case.memory_system);
    }
}

const REPAIR_LOAD_PC: &str = "0x80000018";
const REPAIR_BRANCH_PC: &str = "0x8000001c";
const REPAIR_CALL_PC: &str = "0x80000020";
const REPAIR_DESCENDANT_PC: &str = "0x8000002c";

fn older_branch_same_link_binary(name: &str, link: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(1, 0, 0x0, 7, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, link, 0x17),
        i_type(0x2c - target_auipc_pc, link, 0x0, link, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        b_type(0x1c, 7, 7, 0b000),
        i_type(0, link, 0x0, link, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, link, 0x0, 13, 0x13),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0x33, 0, 0x0, 15, 0x13),
        s_type(4, 15, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x48);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

#[test]
fn rem6_run_o3_older_branch_discards_same_link_call_and_descendant() {
    let case = SAME_LINK_CASES[3];
    let path = older_branch_same_link_binary("o3-older-branch-same-link", case.link);
    let completed = run_same_link_json(&path, case.memory_system, case.max_tick, 2);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x5"), 0x8000_002c);
    assert_register_absent_or_zero(&completed, "x13");
    assert_eq!(register_value(&completed, "x15"), 0x33);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000330000000000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_no_data_address(&completed, WRONG_STORE_12_ADDRESS);

    let load = event_at_pc(&completed, REPAIR_LOAD_PC);
    let branch = event_at_pc(&completed, REPAIR_BRANCH_PC);
    assert_branch_kind_and_link(branch, "direct_conditional", false);
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
            "unexpected older-branch field {field}: {branch}"
        );
    }
    assert!(event_at_pc_if_present(&completed, REPAIR_CALL_PC).is_none());
    assert!(event_at_pc_if_present(&completed, REPAIR_DESCENDANT_PC).is_none());
    assert_same_link_predictor_evidence(&completed, 0, 1);
    assert_eq!(
        completed
            .pointer("/cores/0/branch_predictor/ras/squashes")
            .and_then(Value::as_u64),
        Some(1)
    );

    let response_tick = event_u64(load, "lsq_data_response_tick");
    let live_tick = event_u64(branch, "issue_tick") + 2;
    assert!(live_tick < response_tick);
    let resident = run_same_link_json(&path, case.memory_system, live_tick, 2);
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            REPAIR_LOAD_PC,
            REPAIR_BRANCH_PC,
            REPAIR_CALL_PC,
            REPAIR_DESCENDANT_PC,
        ]
    );
    assert_eq!(register_value(&resident, "x5"), 0x8000_002c);
    assert_integer_rename_maps_to_row_destination(&resident, REPAIR_CALL_PC, 5);
    assert_integer_rename_maps_to_row_destination(&resident, REPAIR_DESCENDANT_PC, 13);
    assert_no_data_address(&resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&resident, WRONG_STORE_ADDRESS);
    assert_no_data_address(&resident, WRONG_STORE_12_ADDRESS);

    let response_resident = run_same_link_json(&path, case.memory_system, response_tick, 2);
    assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_12_ADDRESS);
    assert_hierarchy_activity(&response_resident);
}
