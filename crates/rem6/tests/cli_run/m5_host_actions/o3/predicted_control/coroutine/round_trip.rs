use std::path::PathBuf;

#[derive(Clone, Copy)]
struct CoroutineRoundTripCase {
    label: &'static str,
    binary: fn(&str, usize) -> PathBuf,
    memory_system: &'static str,
    max_tick: u64,
    load_pc: &'static str,
    call_pc: &'static str,
    coroutine_pc: &'static str,
    return_pc: &'static str,
    success_store_pc: &'static str,
    call_kind: &'static str,
    call_destination: u8,
    coroutine_destination: u8,
    final_x1: u64,
    final_x5: u64,
    memory_hex: &'static str,
    provider_no_target: u64,
    provider_indirect: u64,
}

const COROUTINE_ROUND_TRIP_CASES: [CoroutineRoundTripCase; 2] = [
    CoroutineRoundTripCase {
        label: "forward-direct",
        binary: direct_coroutine_round_trip_binary,
        memory_system: "direct",
        max_tick: 2_500,
        load_pc: "0x80000010",
        call_pc: "0x80000014",
        coroutine_pc: "0x80000020",
        return_pc: "0x80000018",
        success_store_pc: "0x80000024",
        call_kind: "call_direct",
        call_destination: 1,
        coroutine_destination: 5,
        final_x1: 0x8000_0018,
        final_x5: 0x8000_0024,
        memory_hex: "2a000000240000800000000000000000",
        provider_no_target: 1,
        provider_indirect: 0,
    },
    CoroutineRoundTripCase {
        label: "reverse-indirect",
        binary: reverse_coroutine_round_trip_binary,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
        load_pc: "0x80000018",
        call_pc: "0x8000001c",
        coroutine_pc: "0x8000002c",
        return_pc: "0x80000020",
        success_store_pc: "0x80000030",
        call_kind: "call_indirect",
        call_destination: 5,
        coroutine_destination: 1,
        final_x1: 0x8000_0030,
        final_x5: 0x8000_0020,
        memory_hex: "2a000000300000800000000000000000",
        provider_no_target: 0,
        provider_indirect: 1,
    },
];

fn direct_coroutine_round_trip_binary(name: &str, exit_padding_words: usize) -> PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(0, 5, 0x0, 0, 0x67),
        s_type(8, 7, 18, 0b010),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(4, 5, 18, 0b010),
    ]);
    assert_eq!(
        words.len() * 4,
        0x28,
        "direct coroutine round-trip fixture must end at code offset 0x28"
    );
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn reverse_coroutine_round_trip_binary(name: &str, exit_padding_words: usize) -> PathBuf {
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
        i_type(0, 1, 0x0, 0, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(4, 1, 18, 0b010),
    ]);
    assert_eq!(
        words.len() * 4,
        0x34,
        "reverse coroutine round-trip fixture must end at code offset 0x34"
    );
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn assert_coroutine_round_trip_commits(case: CoroutineRoundTripCase) {
    let path = (case.binary)(
        &format!("o3-same-window-coroutine-round-trip-{}", case.label),
        0,
    );
    let completed = run_coroutine_json(
        &path,
        case.memory_system,
        case.max_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );

    assert_stopped_by_host(&completed);
    assert_eq!(
        register_value(&completed, "x1"),
        case.final_x1,
        "{}: unexpected final x1: {completed}",
        case.label
    );
    assert_eq!(
        register_value(&completed, "x5"),
        case.final_x5,
        "{}: unexpected final x5: {completed}",
        case.label
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex),
        "{}: unexpected final memory: {completed}",
        case.label
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, case.load_pc);
    let call = event_at_pc(&completed, case.call_pc);
    let coroutine = event_at_pc(&completed, case.coroutine_pc);
    let ordinary_return = event_at_pc(&completed, case.return_pc);
    let success_store = event_at_pc(&completed, case.success_store_pc);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, case.call_kind, true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert_branch_kind_and_link(ordinary_return, "return", false);
    for (role, event) in [
        ("call", call),
        ("coroutine", coroutine),
        ("ordinary return", ordinary_return),
    ] {
        assert!(
            event_u64(event, "issue_tick") < response_tick,
            "{}: {role} must issue before load response tick {response_tick}: {event}",
            case.label
        );
    }
    assert!(
        event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"),
        "{}: coroutine must issue after call writeback: call={call}, coroutine={coroutine}",
        case.label
    );
    assert!(
        event_u64(ordinary_return, "issue_tick") > event_u64(coroutine, "writeback_tick"),
        "{}: ordinary return must issue after coroutine writeback: coroutine={coroutine}, return={ordinary_return}",
        case.label
    );
    assert_ordered_commits([load, call, coroutine, ordinary_return, success_store]);
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        Some(3),
        "{}: unexpected writeback-port admission count: {completed}",
        case.label
    );

    let live_tick = event_u64(ordinary_return, "issue_tick") + 1;
    assert!(
        live_tick < response_tick,
        "{}: live tick {live_tick} must precede load response tick {response_tick}",
        case.label
    );
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
            case.coroutine_pc,
            case.return_pc,
        ],
        "{}: unexpected resident ROB at tick {live_tick}: {resident}",
        case.label
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1),
        "{}: expected one resident LSQ row at tick {live_tick}: {resident}",
        case.label
    );
    for register in ["x1", "x5"] {
        assert_register_absent_or_zero_with_context(&resident, register, case.label);
    }
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
    let ordinary_return_row = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/pc").and_then(Value::as_str) == Some(case.return_pc)
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "{}: missing resident ordinary-return row {}: {resident}",
                case.label, case.return_pc
            )
        });
    assert!(
        ordinary_return_row
            .pointer("/destination")
            .is_some_and(Value::is_null),
        "{}: ordinary-return destination must be JSON null: {ordinary_return_row}",
        case.label
    );

    for (pointer, expected) in [
        (
            "/cores/0/branch_predictor/target_provider/no_target",
            case.provider_no_target,
        ),
        (
            "/cores/0/branch_predictor/target_provider/indirect",
            case.provider_indirect,
        ),
        ("/cores/0/branch_predictor/target_provider/ras", 2),
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 2),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 2),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "{}: expected {pointer}={expected}: {completed}",
            case.label
        );
    }

    match case.memory_system {
        "direct" => assert_direct_memory_activity(&resident),
        "cache-fabric-dram" => {
            let response_resident = run_coroutine_json(
                &path,
                case.memory_system,
                response_tick,
                "detailed",
                3,
                &DIRECT_WIDTH_ARGS,
            );
            assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
            assert_hierarchy_activity(&response_resident);
        }
        memory_system => panic!(
            "{}: unsupported coroutine round-trip memory system {memory_system}",
            case.label
        ),
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_commits_direct() {
    assert_coroutine_round_trip_commits(COROUTINE_ROUND_TRIP_CASES[0]);
}

#[test]
fn rem6_run_o3_same_window_indirect_coroutine_round_trip_commits_cache_fabric_dram() {
    assert_coroutine_round_trip_commits(COROUTINE_ROUND_TRIP_CASES[1]);
}
