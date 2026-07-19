use super::window_support::{
    assert_branch_kind_and_link, assert_control_prediction, assert_direct_memory_activity,
    assert_hierarchy_activity, assert_integer_rename_maps_to_row_destination,
    assert_no_data_address, assert_no_fetch_pc, assert_no_o3_stats, assert_pointer_u64_gt,
    assert_stopped_by_host, control_window_command, fetch_count_at_pc, fetch_tick_at_pc,
    finish_control_window_binary, resident_rob_pcs, run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const NONADJACENT_LOAD_PC: &str = "0x80000018";
const NONADJACENT_PRODUCER_PC: &str = "0x8000001c";
const NONADJACENT_SPACER_PC: &str = "0x80000020";
const NONADJACENT_JALR_PC: &str = "0x80000024";
const NONADJACENT_TARGET_PC: &str = "0x80000034";
const WARM_LOAD_PC: &str = "0x8000002c";
const WARM_PRODUCER_PC: &str = "0x80000030";
const WARM_JALR_PC: &str = "0x80000034";
const WARM_TARGET_PC: &str = "0x80000054";
const UNRESOLVED_LOAD_PC: &str = "0x80000024";
const UNRESOLVED_JALR_PC: &str = "0x80000028";
const WRONG_STORE_ADDRESS: &str = "0x8000010c";
const WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "2",
];
#[derive(Clone, Copy)]
struct LineageCase {
    label: &'static str,
    memory_system: &'static str,
    destination: u8,
    branch_kind: &'static str,
    max_tick: u64,
}

const LINEAGE_CASES: [LineageCase; 4] = [
    LineageCase {
        label: "no-link-direct",
        memory_system: "direct",
        destination: 0,
        branch_kind: "indirect_unconditional",
        max_tick: 2_500,
    },
    LineageCase {
        label: "no-link-hierarchy",
        memory_system: "cache-fabric-dram",
        destination: 0,
        branch_kind: "indirect_unconditional",
        max_tick: 3_500,
    },
    LineageCase {
        label: "split-link-direct",
        memory_system: "direct",
        destination: 5,
        branch_kind: "call_indirect",
        max_tick: 2_500,
    },
    LineageCase {
        label: "split-link-hierarchy",
        memory_system: "cache-fabric-dram",
        destination: 5,
        branch_kind: "call_indirect",
        max_tick: 3_500,
    },
];

#[test]
fn rem6_run_o3_nonadjacent_producer_forwarded_jalr_targets_cover_link_route_matrix() {
    for case in LINEAGE_CASES {
        let path = nonadjacent_lineage_binary(
            &format!("o3-nonadjacent-producer-forwarded-jalr-{}", case.label),
            case.destination,
        );
        let completed = run_lineage_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        assert_eq!(register_value(&completed, "x13"), 42);
        assert_eq!(register_value(&completed, "x14"), 7);
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a0000002a0000000700000000000000")
        );
        assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
        assert_eq!(
            register_value(&completed, "x5"),
            if case.destination == 0 {
                0x55
            } else {
                0x8000_0028
            }
        );

        let load = event_at_pc(&completed, NONADJACENT_LOAD_PC);
        let producer = event_at_pc(&completed, NONADJACENT_PRODUCER_PC);
        let spacer = event_at_pc(&completed, NONADJACENT_SPACER_PC);
        let jalr = event_at_pc(&completed, NONADJACENT_JALR_PC);
        let target = event_at_pc(&completed, NONADJACENT_TARGET_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(jalr, case.branch_kind, case.destination != 0);
        assert_control_prediction(jalr, NONADJACENT_TARGET_PC);
        assert!(
            event_u64(producer, "issue_tick") < response_tick,
            "producer did not issue before load response for {}: load={load} producer={producer}",
            case.label
        );
        assert!(
            event_u64(spacer, "issue_tick") < response_tick,
            "spacer did not issue before load response for {}: load={load} spacer={spacer}",
            case.label
        );
        assert!(
            event_u64(jalr, "issue_tick") < response_tick,
            "JALR did not issue before load response for {}: load={load} jalr={jalr}",
            case.label
        );
        assert!(
            event_u64(jalr, "issue_tick") >= event_u64(producer, "writeback_tick"),
            "JALR issued before its producer for {}: producer={producer} jalr={jalr}",
            case.label
        );
        assert!(
            event_u64(target, "issue_tick") >= response_tick,
            "full four-row non-adjacent window must not claim a pre-response target row for {}: load={load} target={target}",
            case.label
        );
        assert_eq!(fetch_count_at_pc(&completed, NONADJACENT_TARGET_PC), 1);
        assert!(fetch_tick_at_pc(&completed, NONADJACENT_TARGET_PC) < response_tick);

        let resident = run_lineage_json(
            &path,
            case.memory_system,
            response_tick - 1,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [
                NONADJACENT_LOAD_PC,
                NONADJACENT_PRODUCER_PC,
                NONADJACENT_SPACER_PC,
                NONADJACENT_JALR_PC,
            ]
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        for fallthrough in ["0x80000028", "0x8000002c", "0x80000030"] {
            assert_no_fetch_pc(&resident, fallthrough);
        }
        if case.destination != 0 {
            assert_integer_rename_maps_to_row_destination(
                &resident,
                NONADJACENT_JALR_PC,
                u64::from(case.destination),
            );
        }
        assert_eq!(
            completed
                .pointer("/cores/0/branch_predictor/ras/pushes")
                .and_then(Value::as_u64),
            Some(u64::from(case.destination != 0))
        );
        assert_pointer_u64_gt(
            &completed,
            "/cores/0/branch_predictor/target_provider/indirect",
            0,
        );
        assert_eq!(
            json_stat_u64(&completed, "sim.cpu0.o3.max_rob_occupancy"),
            4
        );
        assert_eq!(json_stat_u64(&resident, "sim.cpu0.o3.max_lsq_occupancy"), 1);
        match case.memory_system {
            "direct" => assert_direct_memory_activity(&completed),
            "cache-fabric-dram" => assert_hierarchy_activity(&completed),
            other => panic!("unsupported lineage route {other}"),
        }
    }
}

#[test]
fn rem6_run_o3_warmed_producer_forwarded_targets_issue_descendants_before_load_response() {
    for (label, destination, branch_kind) in [
        ("no-link", 0, "indirect_unconditional"),
        ("split-link", 5, "call_indirect"),
    ] {
        let path = warmed_target_binary(
            &format!("o3-warmed-producer-forwarded-target-{label}"),
            destination,
        );
        let completed = run_lineage_json(
            &path,
            "cache-fabric-dram",
            3_500,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        let load = event_at_pc(&completed, WARM_LOAD_PC);
        let producer = event_at_pc(&completed, WARM_PRODUCER_PC);
        let jalr = event_at_pc(&completed, WARM_JALR_PC);
        let target = event_at_pc(&completed, WARM_TARGET_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        assert_branch_kind_and_link(jalr, branch_kind, destination != 0);
        assert_control_prediction(jalr, WARM_TARGET_PC);
        assert!(event_u64(producer, "issue_tick") < response_tick);
        assert!(event_u64(jalr, "issue_tick") < response_tick);
        assert!(event_u64(jalr, "issue_tick") >= event_u64(producer, "writeback_tick"));
        assert!(event_u64(target, "issue_tick") < response_tick);
        assert!(event_u64(target, "writeback_tick") >= event_u64(target, "issue_tick"));
        assert!([
            event_u64(load, "commit_tick"),
            event_u64(producer, "commit_tick"),
            event_u64(jalr, "commit_tick"),
            event_u64(target, "commit_tick"),
        ]
        .windows(2)
        .all(|ticks| ticks[0] <= ticks[1]));

        let target_fetch_ticks = fetch_ticks_at_pc(&completed, WARM_TARGET_PC);
        assert_eq!(target_fetch_ticks.len(), 2);
        assert!(target_fetch_ticks[0] < event_u64(load, "issue_tick"));
        assert!(target_fetch_ticks[1] >= event_u64(jalr, "issue_tick"));
        assert!(target_fetch_ticks[1] >= event_u64(producer, "writeback_tick"));
        assert!(target_fetch_ticks[1] < response_tick);

        let resident = run_lineage_json(
            &path,
            "cache-fabric-dram",
            response_tick - 1,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [WARM_LOAD_PC, WARM_PRODUCER_PC, WARM_JALR_PC, WARM_TARGET_PC]
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(register_value(&resident, "x13"), 0);
        assert_eq!(register_value(&resident, "x5"), 0x55);
        for fallthrough in [
            "0x80000038",
            "0x8000003c",
            "0x80000040",
            "0x80000044",
            "0x80000048",
            "0x8000004c",
            "0x80000050",
        ] {
            assert_no_fetch_pc(&resident, fallthrough);
        }
        if destination != 0 {
            assert_integer_rename_maps_to_row_destination(
                &resident,
                WARM_JALR_PC,
                u64::from(destination),
            );
        }
        assert_eq!(
            completed
                .pointer("/cores/0/branch_predictor/ras/pushes")
                .and_then(Value::as_u64),
            Some(u64::from(destination != 0))
        );
        let target_fetch_tick = target_fetch_ticks[1];
        assert_eq!(
            fetch_pcs_between(&resident, target_fetch_tick, response_tick),
            [WARM_TARGET_PC]
        );
        let before_target = run_lineage_json(
            &path,
            "cache-fabric-dram",
            target_fetch_tick - 1,
            "detailed",
            4,
            &WIDTH_ARGS,
        );
        assert_eq!(
            pointer_u64(
                &resident,
                "/simulation/instruction_cache_bank_immediate_hits"
            ),
            pointer_u64(
                &before_target,
                "/simulation/instruction_cache_bank_immediate_hits"
            ) + 1,
            "the second warmed target fetch must account for exactly one immediate L1 hit"
        );
        assert_eq!(register_value(&completed, "x13"), 42);
        assert_eq!(
            register_value(&completed, "x5"),
            if destination == 0 { 0x55 } else { 0x8000_0038 }
        );
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("2a0000002a0000000000000000000000")
        );
        assert_eq!(
            json_stat_u64(&completed, "sim.cpu0.o3.max_rob_occupancy"),
            4
        );
        assert_eq!(json_stat_u64(&resident, "sim.cpu0.o3.max_lsq_occupancy"), 1);
        assert_hierarchy_activity(&completed);
    }
}

#[test]
fn rem6_run_o3_warmed_producer_forwarded_target_descendant_requires_depth_four() {
    let path = warmed_target_binary("o3-warmed-target-depth-three", 0);
    let baseline = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        3,
        &WIDTH_ARGS,
    );
    let load = event_at_pc(&baseline, WARM_LOAD_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let target_fetch_ticks = fetch_ticks_at_pc(&baseline, WARM_TARGET_PC);
    assert_eq!(target_fetch_ticks.len(), 2);
    assert!(target_fetch_ticks[1] < response_tick);

    let resident = run_lineage_json(
        &path,
        "cache-fabric-dram",
        response_tick - 1,
        "detailed",
        3,
        &WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [WARM_LOAD_PC, WARM_PRODUCER_PC, WARM_JALR_PC]
    );
    assert!(event_at_pc_if_present(&resident, WARM_TARGET_PC).is_none());
}

#[test]
fn rem6_run_o3_warmed_target_does_not_bypass_unresolved_jalr_source() {
    let path = warmed_unresolved_target_binary("o3-warmed-unresolved-target");
    let completed = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    assert_stopped_by_host(&completed);
    let load = event_at_pc(&completed, UNRESOLVED_LOAD_PC);
    let jalr = event_at_pc(&completed, UNRESOLVED_JALR_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(jalr, "issue_tick") >= response_tick);
    assert_eq!(register_value(&completed, "x13"), 42);

    let resident = run_lineage_json(
        &path,
        "cache-fabric-dram",
        response_tick - 1,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [UNRESOLVED_LOAD_PC, UNRESOLVED_JALR_PC]
    );
    assert!(event_at_pc_if_present(&resident, WARM_TARGET_PC).is_none());
}

#[test]
fn rem6_run_host_switch_transfers_nonadjacent_producer_forwarded_jalr_window() {
    let path = nonadjacent_lineage_binary("o3-nonadjacent-lineage-switch", 5);
    let baseline = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    let load = event_at_pc(&baseline, NONADJACENT_LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, NONADJACENT_JALR_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let mut args = WIDTH_ARGS.to_vec();
    args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
    let switched = run_lineage_json(&path, "cache-fabric-dram", 3_500, "detailed", 4, &args);
    let timing_switch = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing non-adjacent timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("non-adjacent state transfer");
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1)
    );
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    for (field, expected) in [
        ("/schema_version", 7),
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", 3),
    ] {
        assert_eq!(
            handoff.pointer(field).and_then(Value::as_u64),
            Some(expected)
        );
    }
    for pc in [
        NONADJACENT_LOAD_PC,
        NONADJACENT_PRODUCER_PC,
        NONADJACENT_SPACER_PC,
        NONADJACENT_JALR_PC,
    ] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "state transfer changed {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
    assert_eq!(register_value(&switched, "x5"), 0x8000_0028);
    assert_eq!(
        switched.pointer("/memory/0/hex").and_then(Value::as_str),
        baseline.pointer("/memory/0/hex").and_then(Value::as_str)
    );
}

#[test]
fn rem6_run_rejects_live_warmed_producer_forwarded_jalr_checkpoint() {
    let path = warmed_target_binary("o3-warmed-lineage-live-checkpoint", 5);
    let baseline = run_lineage_json(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        4,
        &WIDTH_ARGS,
    );
    let load = event_at_pc(&baseline, WARM_LOAD_PC);
    let target = event_at_pc(&baseline, WARM_TARGET_PC);
    let checkpoint_tick = event_u64(target, "issue_tick") + 1;
    assert!(checkpoint_tick < event_u64(load, "lsq_data_response_tick"));
    assert!(checkpoint_tick < event_u64(target, "commit_tick"));

    let checkpoint_arg = format!("{checkpoint_tick}:warmed-lineage-live");
    let mut command = control_window_command(
        &path,
        "cache-fabric-dram",
        3_500,
        "detailed",
        1,
        DATA_ADDRESS,
        16,
    );
    command.args([
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--riscv-o3-issue-width",
        "4",
        "--riscv-o3-writeback-width",
        "2",
        "--host-checkpoint",
        checkpoint_arg.as_str(),
    ]);
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live warmed-lineage checkpoint should fail closed: {stderr}"
    );
}

#[test]
fn rem6_run_timing_suppresses_producer_forwarded_lineage_windows() {
    let cases = [
        (
            "nonadjacent-direct",
            nonadjacent_lineage_binary("o3-nonadjacent-lineage-timing", 0),
            "direct",
            2_500,
            7,
            "2a0000002a0000000700000000000000",
        ),
        (
            "warmed-hierarchy",
            warmed_target_binary("o3-warmed-lineage-timing", 0),
            "cache-fabric-dram",
            3_500,
            0,
            "2a0000002a0000000000000000000000",
        ),
    ];
    for (label, path, memory_system, max_tick, expected_x14, expected_memory) in cases {
        let timing = run_lineage_json(&path, memory_system, max_tick, "timing", 4, &WIDTH_ARGS);
        assert_stopped_by_host(&timing);
        assert_eq!(register_value(&timing, "x5"), 0x55, "{label}");
        assert_eq!(register_value(&timing, "x13"), 42, "{label}");
        assert_eq!(register_value(&timing, "x14"), expected_x14, "{label}");
        assert_eq!(
            timing.pointer("/memory/0/hex").and_then(Value::as_str),
            Some(expected_memory),
            "{label}"
        );
        assert!(timing.pointer("/cores/0/o3_runtime").is_none(), "{label}");
        assert!(
            timing
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "{label}"
        );
        assert_no_o3_stats(&timing);
    }
}

fn nonadjacent_lineage_binary(name: &str, destination: u8) -> std::path::PathBuf {
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
        i_type(0x34 - target_auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 10, 0x0, 11, 0x13),
        i_type(7, 0, 0x0, 14, 0x13),
        i_type(0, 11, 0x0, destination, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        s_type(8, 14, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    assert_eq!(words.len() * 4, 0x48);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn warmed_target_binary(name: &str, destination: u8) -> std::path::PathBuf {
    let mut words = vec![i_type(0, 0, 0x0, 17, 0x13), 0, m5op(M5_FAIL)];
    let post_warm_index = words.len();
    words.push(i_type(0x55, 0, 0x0, 5, 0x13));
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
    ]);
    let target_auipc_index = words.len();
    words.extend([
        u_type(0, 19, 0x17),
        0,
        i_type(0, 0, 0x0, 13, 0x13),
        i_type(1, 0, 0x0, 17, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 19, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, destination, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < 0x54 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let target_index = words.len();
    words.extend([
        i_type(42, 0, 0x0, 13, 0x13),
        b_type(8, 0, 17, 0b001),
        0,
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let warm_return_index = target_index + 2;

    let pc = |index: usize| (index * 4) as i32;
    words[1] = j_type(pc(target_index) - pc(1), 0);
    words[target_auipc_index + 1] =
        i_type(pc(target_index) - pc(target_auipc_index), 19, 0x0, 19, 0x13);
    words[warm_return_index] = j_type(pc(post_warm_index) - pc(warm_return_index), 0);

    assert_eq!(pc(post_warm_index), 0x0c);
    assert_eq!(pc(target_index), 0x54);
    assert_eq!(words.len() * 4, 0x6c);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}

fn warmed_unresolved_target_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![i_type(0, 0, 0x0, 17, 0x13), 0, m5op(M5_FAIL)];
    let post_warm_index = words.len();
    words.push(i_type(0x55, 0, 0x0, 5, 0x13));
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 0, 0x0, 13, 0x13),
        i_type(1, 0, 0x0, 17, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(0, 18, 0b110, 11, 0x03),
        i_type(0, 11, 0x0, 0, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < 0x54 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let target_index = words.len();
    words.extend([
        i_type(42, 0, 0x0, 13, 0x13),
        b_type(8, 0, 17, 0b001),
        0,
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let warm_return_index = target_index + 2;
    let pc = |index: usize| (index * 4) as i32;
    words[1] = j_type(pc(target_index) - pc(1), 0);
    words[warm_return_index] = j_type(pc(post_warm_index) - pc(warm_return_index), 0);

    assert_eq!(pc(post_warm_index), 0x0c);
    assert_eq!(pc(target_index), 0x54);
    assert_eq!(words.len() * 4, 0x6c);
    finish_control_window_binary(name, words, DATA_START as usize, [0x8000_0054, 0, 0, 0])
}

fn fetch_ticks_at_pc(json: &Value, pc: &str) -> Vec<u64> {
    json.pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .filter_map(|event| event.pointer("/tick").and_then(Value::as_u64))
        .collect()
}

fn fetch_pcs_between(json: &Value, start_tick: u64, end_tick: u64) -> Vec<&str> {
    json.pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|event| {
            event
                .pointer("/tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| start_tick <= tick && tick < end_tick)
        })
        .filter_map(|event| event.pointer("/pc").and_then(Value::as_str))
        .collect()
}

fn pointer_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing unsigned value at {pointer}: {json}"))
}

fn run_lineage_json(
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
