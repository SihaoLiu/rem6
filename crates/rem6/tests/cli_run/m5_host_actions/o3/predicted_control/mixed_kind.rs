use super::window_support::{
    assert_no_data_address, control_window_command, resident_rob_pcs, run_control_window_json,
};
use super::*;

const LOAD_PC: &str = "0x80000024";
const JAL_PC: &str = "0x80000028";
const BRANCH_PC: &str = "0x80000034";
const JALR_PC: &str = "0x80000038";
const CONDITIONAL_TARGET_PC: &str = "0x80000040";
const INDIRECT_TARGET_PC: &str = "0x80000048";
const INDIRECT_STORE_PC: &str = "0x8000004c";
const DATA_ADDRESS: &str = "0x80000100";
const JAL_FALLTHROUGH_STORE_ADDRESS: &str = "0x80000108";
const JALR_FALLTHROUGH_STORE_ADDRESS: &str = "0x8000010c";

#[test]
fn rem6_run_o3_mixed_control_kind_commits_direct() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-direct", false, false);
    let completed = run_mixed_control_kind_json(&path, "direct", 2_500, "detailed", 3, &[]);

    assert_mixed_control_kind_success(&path, "direct", &completed);
}

#[test]
fn rem6_run_o3_mixed_control_kind_commits_cache_fabric_dram() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-hierarchy", false, false);
    let completed =
        run_mixed_control_kind_json(&path, "cache-fabric-dram", 3_000, "detailed", 3, &[]);

    assert_mixed_control_kind_success(&path, "cache-fabric-dram", &completed);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            completed
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed mixed control should expose {pointer}: {completed}"
        );
    }
}

fn assert_mixed_control_kind_success(path: &Path, memory_system: &str, completed: &Value) {
    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(register_value(&completed, "x1"), 17);
    assert_eq!(register_value(&completed, "x5"), 85);
    assert_eq!(register_value(&completed, "x13"), 0x44);
    assert_eq!(register_value(&completed, "x15"), 0);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000440000000000000000000000")
    );
    assert_no_data_address(&completed, JAL_FALLTHROUGH_STORE_ADDRESS);
    assert_no_data_address(&completed, JALR_FALLTHROUGH_STORE_ADDRESS);

    let load = event_at_pc(&completed, LOAD_PC);
    let direct_jump = event_at_pc(&completed, JAL_PC);
    let conditional = event_at_pc(&completed, BRANCH_PC);
    let indirect_jump = event_at_pc(&completed, JALR_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    for event in [direct_jump, conditional, indirect_jump] {
        assert!(event_u64(event, "issue_tick") < response_tick);
        assert_eq!(
            event
                .pointer("/branch_link_register_write")
                .and_then(Value::as_bool),
            Some(false)
        );
    }
    assert_eq!(
        conditional
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );
    for (event, kind) in [
        (direct_jump, "direct_unconditional"),
        (conditional, "direct_conditional"),
        (indirect_jump, "indirect_unconditional"),
    ] {
        assert_eq!(
            event.pointer("/branch_kind").and_then(Value::as_str),
            Some(kind)
        );
    }
    assert!([load, direct_jump, conditional, indirect_jump]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));

    let live_tick = event_u64(indirect_jump, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_mixed_control_kind_json(path, memory_system, live_tick, "detailed", 3, &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, JAL_PC, BRANCH_PC, JALR_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    for (kind, count) in [
        ("direct_unconditional", 1),
        ("direct_conditional", 1),
        ("indirect_unconditional", 1),
    ] {
        assert_eq!(
            resident
                .pointer(&format!("/cores/0/branch_predictor/lookups/{kind}"))
                .and_then(Value::as_u64),
            Some(count),
            "{kind}: {resident}"
        );
    }
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    event_at_pc(&completed, INDIRECT_TARGET_PC);
    assert!(event_at_pc_if_present(&completed, CONDITIONAL_TARGET_PC).is_none());
}

#[test]
fn rem6_run_o3_mixed_control_kind_requires_branch_lookahead_three() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-lookahead-two", false, false);
    let completed = run_mixed_control_kind_json(&path, "direct", 2_500, "detailed", 2, &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let conditional = event_at_pc(&completed, BRANCH_PC);
    event_at_pc(&completed, JALR_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(conditional, "issue_tick") < response_tick);

    let live_tick = event_u64(conditional, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_mixed_control_kind_json(&path, "direct", live_tick, "detailed", 2, &[]);
    assert_eq!(resident_rob_pcs(&resident), [LOAD_PC, JAL_PC, BRANCH_PC]);
    assert_eq!(
        resident
            .pointer("/cores/0/branch_predictor/lookups/indirect_unconditional")
            .and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn rem6_run_o3_mixed_control_kind_middle_repair_discards_indirect_jump_hierarchy() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-middle-repair", true, false);
    let completed =
        run_mixed_control_kind_json(&path, "cache-fabric-dram", 3_000, "detailed", 3, &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let direct_jump = event_at_pc(&completed, JAL_PC);
    let conditional = event_at_pc(&completed, BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(direct_jump, "issue_tick") < response_tick);
    assert!(event_u64(conditional, "issue_tick") < response_tick);
    assert_eq!(
        conditional
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        conditional
            .pointer("/branch_squash")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, JALR_PC).is_none());
    event_at_pc(&completed, CONDITIONAL_TARGET_PC);
    assert!(event_at_pc_if_present(&completed, INDIRECT_TARGET_PC).is_none());
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_eq!(register_value(&completed, "x15"), 0x33);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000000000000000000000")
    );
    assert_no_data_address(&completed, JAL_FALLTHROUGH_STORE_ADDRESS);
    assert_no_data_address(&completed, JALR_FALLTHROUGH_STORE_ADDRESS);

    let live_tick = event_u64(conditional, "issue_tick") + 2;
    assert!(live_tick < response_tick);
    let resident =
        run_mixed_control_kind_json(&path, "cache-fabric-dram", live_tick, "detailed", 3, &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, JAL_PC, BRANCH_PC, JALR_PC]
    );
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            completed
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed mixed control should expose {pointer}: {completed}"
        );
    }
}

#[test]
fn rem6_run_o3_mixed_control_kind_load_dependent_jalr_stays_terminal() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-dependent-jalr", false, true);
    let completed = run_mixed_control_kind_json(&path, "direct", 2_500, "detailed", 3, &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let conditional = event_at_pc(&completed, BRANCH_PC);
    let indirect_jump = event_at_pc(&completed, JALR_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(conditional, "issue_tick") < response_tick);
    assert!(event_u64(indirect_jump, "issue_tick") >= response_tick);
    assert_eq!(register_value(&completed, "x13"), 0x44);

    let live_tick = event_u64(conditional, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_mixed_control_kind_json(&path, "direct", live_tick, "detailed", 3, &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, JAL_PC, BRANCH_PC, JALR_PC]
    );
    assert_no_fetch_pc(&resident, INDIRECT_TARGET_PC);
}

#[test]
fn rem6_run_host_switch_transfers_o3_mixed_control_kinds() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-switch", false, false);
    let baseline = run_mixed_control_kind_json(&path, "direct", 2_500, "detailed", 3, &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, JALR_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_mixed_control_kind_json(
        &path,
        "direct",
        2_500,
        "detailed",
        3,
        &["--host-switch-cpu-mode", &switch_arg],
    );
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
        .unwrap_or_else(|| panic!("missing mixed-control timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("mixed-control state transfer");
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
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );

    for pc in [LOAD_PC, JAL_PC, BRANCH_PC, JALR_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "mixed-control transfer must preserve {field} for {pc}"
            );
        }
    }
    assert_eq!(register_value(&switched, "x13"), 0x44);
    assert_eq!(
        switched.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000440000000000000000000000")
    );
    let execution_modes = switched
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing final mixed-control execution mode: {switched}"));
    assert_eq!(execution_modes.len(), 1);
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some("timing")
    );
    assert_drained_mixed_control_runtime(&switched);
}

#[test]
fn rem6_run_o3_mixed_control_kind_checkpoint_boundary() {
    let path = mixed_control_kind_binary_with_exit_padding(
        "o3-mixed-control-kind-checkpoint",
        false,
        false,
        8,
    );
    let baseline = run_mixed_control_kind_json(&path, "direct", 2_500, "detailed", 3, &[]);
    let live_tick = event_u64(event_at_pc(&baseline, JALR_PC), "issue_tick") + 1;
    let live_arg = format!("{live_tick}:mixed-control-live");
    let mut live_command =
        control_window_command(&path, "direct", 2_500, "detailed", 3, DATA_ADDRESS, 16);
    live_command.args(["--host-checkpoint", &live_arg]);
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live mixed-control checkpoint should fail closed: {stderr}"
    );

    let checkpoint_tick = event_u64(event_at_pc(&baseline, INDIRECT_STORE_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:mixed-control-drained");
    let restore_arg = format!("{restore_tick}:mixed-control-drained");
    let restored = run_mixed_control_kind_json(
        &path,
        "direct",
        2_500,
        "detailed",
        3,
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("drained mixed-control checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    assert!(checkpoint_component_chunks(cpu0).iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained mixed-control O3 runtime checkpoint");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(register_value(&restored, "x1"), 17);
    assert_eq!(register_value(&restored, "x5"), 85);
    assert_eq!(register_value(&restored, "x13"), 0x44);
    assert_eq!(
        restored.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000440000000000000000000000")
    );
    assert_drained_mixed_control_runtime(&restored);
}

#[test]
fn rem6_run_timing_suppresses_o3_mixed_control_kinds() {
    let path = mixed_control_kind_binary("o3-mixed-control-kind-timing", false, false);
    let timing = run_mixed_control_kind_json(&path, "direct", 2_500, "timing", 3, &[]);

    assert_eq!(register_value(&timing, "x13"), 0x44);
    assert_eq!(
        timing.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000440000000000000000000000")
    );
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing mixed-control stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode leaked mixed-control O3 stats: {unexpected:?}"
    );
}

fn mixed_control_kind_binary(
    name: &str,
    conditional_taken: bool,
    jalr_uses_load_destination: bool,
) -> std::path::PathBuf {
    mixed_control_kind_binary_with_exit_padding(
        name,
        conditional_taken,
        jalr_uses_load_destination,
        0,
    )
}

fn mixed_control_kind_binary_with_exit_padding(
    name: &str,
    conditional_taken: bool,
    jalr_uses_load_destination: bool,
    exit_padding_words: usize,
) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.push(u_type(0, 18, 0x17));
    words.push(i_type(data_start - data_auipc_pc, 18, 0x0, 18, 0x13));
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0x48 - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(if conditional_taken { 1 } else { 2 }, 0, 0x0, 8, 0x13),
        i_type(17, 0, 0x0, 1, 0x13),
        i_type(85, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b110, 12, 0x03),
        j_type(12, 0),
        i_type(0x55, 0, 0x0, 20, 0x13),
        s_type(8, 20, 18, 0b010),
        b_type(12, 8, 7, 0b000),
        i_type(
            0,
            if jalr_uses_load_destination { 12 } else { 11 },
            0x0,
            0,
            0x67,
        ),
        s_type(12, 7, 18, 0b010),
        i_type(0x33, 0, 0x0, 15, 0x13),
        m5op(M5_EXIT),
        i_type(0x44, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
    ]);
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([
        if jalr_uses_load_destination {
            0x8000_0048
        } else {
            42
        },
        0,
        0,
        0,
    ]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn run_mixed_control_kind_json(
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

fn assert_drained_mixed_control_runtime(json: &Value) {
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}

fn assert_no_fetch_pc(json: &Value, pc: &str) {
    assert!(
        json.pointer("/debug/fetch_trace")
            .and_then(Value::as_array)
            .is_some_and(|records| records
                .iter()
                .all(|record| record.pointer("/pc").and_then(Value::as_str) != Some(pc))),
        "unexpected fetch at {pc}: {json}"
    );
}
