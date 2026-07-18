use super::*;

#[derive(Clone, Copy)]
struct SecondLinkedCoroutineCase {
    label: &'static str,
    binary: fn(&str) -> std::path::PathBuf,
    memory_system: &'static str,
    max_tick: u64,
    load_pc: &'static str,
    call_pc: &'static str,
    first_coroutine_pc: &'static str,
    second_coroutine_pc: &'static str,
    success_store_pc: &'static str,
    call_kind: &'static str,
    first_destination: u8,
    second_destination: u8,
    final_x1: u64,
    final_x5: u64,
    memory_hex: &'static str,
    provider_no_target: u64,
    provider_indirect: u64,
}

const SECOND_LINKED_COROUTINE_CASES: [SecondLinkedCoroutineCase; 2] = [
    SecondLinkedCoroutineCase {
        label: "forward-direct",
        binary: direct_second_linked_coroutine_binary,
        memory_system: "direct",
        max_tick: 2_500,
        load_pc: "0x80000010",
        call_pc: "0x80000014",
        first_coroutine_pc: "0x80000020",
        second_coroutine_pc: "0x80000018",
        success_store_pc: "0x80000024",
        call_kind: "call_direct",
        first_destination: 5,
        second_destination: 1,
        final_x1: 0x8000_001c,
        final_x5: 0x8000_0024,
        memory_hex: "2a0000001c0000800000000000000000",
        provider_no_target: 1,
        provider_indirect: 0,
    },
    SecondLinkedCoroutineCase {
        label: "reverse-indirect",
        binary: reverse_second_linked_coroutine_binary,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
        load_pc: "0x80000018",
        call_pc: "0x8000001c",
        first_coroutine_pc: "0x8000002c",
        second_coroutine_pc: "0x80000020",
        success_store_pc: "0x80000030",
        call_kind: "call_indirect",
        first_destination: 1,
        second_destination: 5,
        final_x1: 0x8000_0030,
        final_x5: 0x8000_0024,
        memory_hex: "2a000000240000800000000000000000",
        provider_no_target: 0,
        provider_indirect: 1,
    },
];

fn direct_second_linked_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(8, 7, 18, 0b010),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(4, 1, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x30);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn reverse_second_linked_coroutine_binary(name: &str) -> std::path::PathBuf {
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
        i_type(0x2c - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(4, 5, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x3c);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn assert_second_linked_coroutine_commits(case: SecondLinkedCoroutineCase) {
    let path = (case.binary)(&format!("o3-second-linked-coroutine-{}", case.label));
    let completed = run_coroutine_json(
        &path,
        case.memory_system,
        case.max_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), case.final_x1);
    assert_eq!(register_value(&completed, "x5"), case.final_x5);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex)
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, case.load_pc);
    let call = event_at_pc(&completed, case.call_pc);
    let first_coroutine = event_at_pc(&completed, case.first_coroutine_pc);
    let second_coroutine = event_at_pc(&completed, case.second_coroutine_pc);
    let success_store = event_at_pc(&completed, case.success_store_pc);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, case.call_kind, true);
    assert_branch_kind_and_link(first_coroutine, "return", true);
    assert_branch_kind_and_link(second_coroutine, "return", true);
    for event in [call, first_coroutine, second_coroutine] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(first_coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(
        event_u64(second_coroutine, "issue_tick") > event_u64(first_coroutine, "writeback_tick")
    );
    assert_ordered_commits([load, call, first_coroutine, second_coroutine, success_store]);
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        Some(4)
    );

    let live_tick = event_u64(second_coroutine, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_coroutine_json(
        &path,
        case.memory_system,
        live_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            case.load_pc,
            case.call_pc,
            case.first_coroutine_pc,
            case.second_coroutine_pc,
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
        case.first_coroutine_pc,
        u64::from(case.first_destination),
    );
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.second_coroutine_pc,
        u64::from(case.second_destination),
    );
    assert_eq!(fetch_count_at_pc(&resident, case.success_store_pc), 1);
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/return", 2),
        ("/cores/0/branch_predictor/target_provider/ras", 2),
        ("/cores/0/branch_predictor/ras/pushes", 3),
        ("/cores/0/branch_predictor/ras/pops", 2),
    ] {
        assert_eq!(
            resident.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "{}: expected resident {pointer}={expected}: {resident}",
            case.label
        );
    }

    let response_resident = run_coroutine_json(
        &path,
        case.memory_system,
        response_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
    assert_no_data_address(&response_resident, WRONG_STORE_ADDRESS);
    match case.memory_system {
        "direct" => assert_direct_memory_activity(&response_resident),
        "cache-fabric-dram" => assert_hierarchy_activity(&response_resident),
        memory_system => panic!("unsupported second-linked memory system {memory_system}"),
    }

    let opposite_call_kind = match case.call_kind {
        "call_direct" => "call_indirect",
        "call_indirect" => "call_direct",
        call_kind => panic!("unsupported second-linked call kind {call_kind}"),
    };
    for (pointer, expected) in [
        (
            format!("/cores/0/branch_predictor/lookups/{}", case.call_kind),
            1,
        ),
        (
            format!("/cores/0/branch_predictor/lookups/{opposite_call_kind}"),
            0,
        ),
        ("/cores/0/branch_predictor/lookups/return".to_owned(), 2),
        (
            "/cores/0/branch_predictor/target_provider/no_target".to_owned(),
            case.provider_no_target,
        ),
        (
            "/cores/0/branch_predictor/target_provider/indirect".to_owned(),
            case.provider_indirect,
        ),
        (
            "/cores/0/branch_predictor/target_provider/ras".to_owned(),
            2,
        ),
        ("/cores/0/branch_predictor/ras/pushes".to_owned(), 3),
        ("/cores/0/branch_predictor/ras/pops".to_owned(), 2),
        ("/cores/0/branch_predictor/ras/used".to_owned(), 2),
        ("/cores/0/branch_predictor/ras/correct".to_owned(), 2),
        ("/cores/0/branch_predictor/ras/incorrect".to_owned(), 0),
    ] {
        assert_eq!(
            completed.pointer(&pointer).and_then(Value::as_u64),
            Some(expected),
            "{}: expected {pointer}={expected}: {completed}",
            case.label
        );
    }
}

#[test]
fn rem6_run_o3_second_linked_coroutine_consumes_replacement_direct() {
    assert_second_linked_coroutine_commits(SECOND_LINKED_COROUTINE_CASES[0]);
}

#[test]
fn rem6_run_o3_second_linked_coroutine_consumes_replacement_cache_fabric_dram() {
    assert_second_linked_coroutine_commits(SECOND_LINKED_COROUTINE_CASES[1]);
}

#[test]
fn rem6_run_o3_second_linked_coroutine_requires_branch_lookahead_three() {
    for case in SECOND_LINKED_COROUTINE_CASES {
        let path = (case.binary)(&format!(
            "o3-second-linked-coroutine-lookahead-two-{}",
            case.label
        ));
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
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some(case.memory_hex)
        );

        let response_tick = event_u64(
            event_at_pc(&completed, case.load_pc),
            "lsq_data_response_tick",
        );
        let resident = run_coroutine_json(
            &path,
            case.memory_system,
            response_tick - 1,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [case.load_pc, case.call_pc, case.first_coroutine_pc]
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(fetch_count_at_pc(&resident, case.second_coroutine_pc), 1);
        assert_eq!(fetch_count_at_pc(&resident, case.success_store_pc), 0);
        assert_no_data_address(&resident, SUCCESS_STORE_ADDRESS);
        assert_no_data_address(&resident, WRONG_STORE_ADDRESS);
        for (pointer, expected) in [
            ("/cores/0/branch_predictor/lookups/return", 1),
            ("/cores/0/branch_predictor/lookups/total", 2),
            ("/cores/0/branch_predictor/target_provider/ras", 1),
            ("/cores/0/branch_predictor/target_provider/total", 2),
            ("/cores/0/branch_predictor/ras/pushes", 2),
            ("/cores/0/branch_predictor/ras/pops", 1),
        ] {
            assert_eq!(
                resident.pointer(pointer).and_then(Value::as_u64),
                Some(expected),
                "{}: expected {pointer}={expected}: {resident}",
                case.label
            );
        }
    }
}
