use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_no_o3_stats, assert_pointer_u64_gt, assert_stopped_by_host, fetch_count_at_pc,
    fetch_tick_at_pc, finish_control_window_binary, resident_rob_pcs, run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const LOAD_PC: &str = "0x80000018";
const PRODUCER_PC: &str = "0x8000001c";
const JALR_PC: &str = "0x80000020";
const TARGET_PC: &str = "0x80000030";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "2",
];

#[derive(Clone, Copy)]
struct ProducerForwardedJalrCase {
    label: &'static str,
    memory_system: &'static str,
    destination: u8,
    branch_kind: &'static str,
    link_write: bool,
    max_tick: u64,
}

const PRODUCER_FORWARDED_JALR_CASES: [ProducerForwardedJalrCase; 4] = [
    ProducerForwardedJalrCase {
        label: "no-link-direct",
        memory_system: "direct",
        destination: 0,
        branch_kind: "indirect_unconditional",
        link_write: false,
        max_tick: 2_500,
    },
    ProducerForwardedJalrCase {
        label: "no-link-hierarchy",
        memory_system: "cache-fabric-dram",
        destination: 0,
        branch_kind: "indirect_unconditional",
        link_write: false,
        max_tick: 3_500,
    },
    ProducerForwardedJalrCase {
        label: "split-link-direct",
        memory_system: "direct",
        destination: 5,
        branch_kind: "call_indirect",
        link_write: true,
        max_tick: 2_500,
    },
    ProducerForwardedJalrCase {
        label: "split-link-hierarchy",
        memory_system: "cache-fabric-dram",
        destination: 5,
        branch_kind: "call_indirect",
        link_write: true,
        max_tick: 3_500,
    },
];

#[test]
fn rem6_run_o3_producer_forwarded_jalr_targets_cover_link_route_matrix() {
    for case in PRODUCER_FORWARDED_JALR_CASES {
        let path = live_scalar_target_binary(
            &format!("o3-producer-forwarded-jalr-{}", case.label),
            case.destination,
        );
        let completed = run_jalr_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        assert_eq!(register_value(&completed, "x13"), 42);
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a0000002a0000000000000000000000")
        );
        assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
        let expected_link = if case.link_write { 0x8000_0024 } else { 0x55 };
        assert_eq!(register_value(&completed, "x5"), expected_link);

        let load = event_at_pc(&completed, LOAD_PC);
        let producer = event_at_pc(&completed, PRODUCER_PC);
        let jalr = event_at_pc(&completed, JALR_PC);
        let target = event_at_pc(&completed, TARGET_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(jalr, case.branch_kind, case.link_write);
        assert!(
            event_u64(producer, "issue_tick") < response_tick,
            "target producer did not issue before the delayed load response for {}: load={load} producer={producer}",
            case.label
        );
        assert!(
            event_u64(jalr, "issue_tick") >= event_u64(producer, "writeback_tick"),
            "JALR issued before its live target producer for {}: producer={producer} jalr={jalr}",
            case.label
        );
        assert!(
            event_u64(jalr, "issue_tick") < response_tick,
            "producer-forwarded JALR did not issue before the delayed load response for {}: load={load} producer={producer} jalr={jalr}",
            case.label
        );
        assert_eq!(
            jalr.pointer("/branch_predicted_target")
                .and_then(Value::as_str),
            Some(TARGET_PC),
            "missing producer-forwarded predicted target for {}: {jalr}",
            case.label
        );
        assert_eq!(
            jalr.pointer("/branch_resolved_target")
                .and_then(Value::as_str),
            Some(TARGET_PC)
        );
        for (field, expected) in [
            ("branch_predicted_taken", true),
            ("branch_resolved_taken", true),
            ("branch_wrong_target", false),
            ("branch_mispredicted", false),
            ("branch_squash", false),
        ] {
            assert_eq!(
                jalr.pointer(&format!("/{field}")).and_then(Value::as_bool),
                Some(expected),
                "unexpected producer-forwarded JALR flag {field}: {jalr}"
            );
        }
        assert!(event_u64(target, "issue_tick") >= response_tick);

        let resident = run_jalr_json(
            &path,
            case.memory_system,
            response_tick - 1,
            "detailed",
            3,
            &WIDTH_ARGS,
        );
        assert_eq!(resident_rob_pcs(&resident), [LOAD_PC, PRODUCER_PC, JALR_PC]);
        assert_eq!(fetch_count_at_pc(&resident, TARGET_PC), 1);
        let target_fetch_tick = fetch_tick_at_pc(&resident, TARGET_PC);
        assert!(target_fetch_tick >= event_u64(producer, "writeback_tick"));
        assert!(target_fetch_tick >= event_u64(jalr, "issue_tick"));
        assert!(target_fetch_tick < response_tick);
        for fallthrough_pc in ["0x80000024", "0x80000028", "0x8000002c"] {
            assert_no_fetch_pc(&resident, fallthrough_pc);
        }
        assert_eq!(register_value(&resident, "x5"), 0x55);
        if case.link_write {
            assert_integer_rename_maps_to_row_destination(&resident, JALR_PC, 5);
            assert_eq!(
                completed
                    .pointer("/cores/0/branch_predictor/ras/pushes")
                    .and_then(Value::as_u64),
                Some(1)
            );
        } else {
            assert_eq!(
                completed
                    .pointer("/cores/0/branch_predictor/ras/pushes")
                    .and_then(Value::as_u64),
                Some(0)
            );
        }
        assert_pointer_u64_gt(
            &completed,
            "/cores/0/branch_predictor/target_provider/indirect",
            0,
        );
        match case.memory_system {
            "direct" => assert_direct_memory_activity(&completed),
            "cache-fabric-dram" => assert_hierarchy_activity(&completed),
            other => panic!("unsupported producer-forwarded route {other}"),
        }
    }
}

#[test]
fn rem6_run_o3_unresolved_producer_forwarded_jalr_targets_stay_terminal() {
    for case in PRODUCER_FORWARDED_JALR_CASES {
        let path = unresolved_load_target_binary(
            &format!("o3-unresolved-producer-forwarded-jalr-{}", case.label),
            case.destination,
        );
        let completed = run_jalr_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        assert_eq!(register_value(&completed, "x13"), 42);
        let load = event_at_pc(&completed, "0x80000010");
        let jalr = event_at_pc(&completed, "0x80000014");
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(jalr, case.branch_kind, case.link_write);
        assert!(event_u64(jalr, "issue_tick") >= response_tick);

        let resident = run_jalr_json(
            &path,
            case.memory_system,
            response_tick - 1,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_eq!(resident_rob_pcs(&resident), ["0x80000010", "0x80000014"]);
        assert_no_fetch_pc(&resident, "0x80000024");
    }
}

#[test]
fn rem6_run_o3_producer_forwarded_jalr_target_requires_depth_three() {
    for case in PRODUCER_FORWARDED_JALR_CASES {
        let path = live_scalar_target_binary(
            &format!("o3-producer-forwarded-jalr-depth-two-{}", case.label),
            case.destination,
        );
        let completed = run_jalr_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        let load = event_at_pc(&completed, LOAD_PC);
        let jalr = event_at_pc(&completed, JALR_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert!(event_u64(jalr, "issue_tick") >= response_tick);

        let resident = run_jalr_json(
            &path,
            case.memory_system,
            response_tick - 1,
            "detailed",
            2,
            &WIDTH_ARGS,
        );
        assert_eq!(resident_rob_pcs(&resident), [LOAD_PC, PRODUCER_PC]);
        assert_no_fetch_pc(&resident, TARGET_PC);
    }
}

#[test]
fn rem6_run_timing_suppresses_o3_producer_forwarded_jalr_targets() {
    for case in PRODUCER_FORWARDED_JALR_CASES {
        let path = live_scalar_target_binary(
            &format!("o3-producer-forwarded-jalr-timing-{}", case.label),
            case.destination,
        );
        let timing = run_jalr_json(&path, case.memory_system, case.max_tick, "timing", 3, &[]);
        assert_stopped_by_host(&timing);
        let expected_link = if case.link_write { 0x8000_0024 } else { 0x55 };
        assert_eq!(register_value(&timing, "x5"), expected_link);
        assert_eq!(register_value(&timing, "x13"), 42);
        assert_eq!(
            timing.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a0000002a0000000000000000000000")
        );
        assert!(timing.pointer("/cores/0/o3_runtime").is_none());
        assert!(timing
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
        assert_no_o3_stats(&timing);
    }
}

fn live_scalar_target_binary(name: &str, destination: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(0x30 - target_auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 10, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, destination, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x40);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn unresolved_load_target_binary(name: &str, destination: u8) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b110, 11, 0x03),
        i_type(0, 11, 0x0, destination, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_control_window_binary(name, words, DATA_START as usize, [0x8000_0024, 0, 0, 0])
}

fn run_jalr_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    scalar_memory_depth: usize,
    extra_args: &[&str],
) -> Value {
    let depth = scalar_memory_depth.to_string();
    let mut args = vec!["--riscv-o3-scalar-memory-depth", depth.as_str()];
    args.extend_from_slice(extra_args);
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        1,
        DATA_ADDRESS,
        16,
        &args,
    )
}
