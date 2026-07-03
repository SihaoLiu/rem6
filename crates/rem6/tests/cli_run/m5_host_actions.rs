use std::process::Command;

use serde_json::Value;

use crate::support::*;

const M5_WORK_BEGIN: u32 = 0x5a;
const M5_WORK_END: u32 = 0x5b;
const M5_EXIT: u32 = 0x21;
const M5_FAIL: u32 = 0x22;
const M5_SUM: u32 = 0x23;
const M5_RESET_STATS: u32 = 0x40;
const M5_DUMP_STATS: u32 = 0x41;
const M5_DUMP_RESET_STATS: u32 = 0x42;
const M5_CHECKPOINT: u32 = 0x43;
const M5_SWITCH_CPU: u32 = 0x52;
const M5_HYPERCALL: u32 = 0x71;

#[test]
fn rem6_run_emits_m5_work_marker_host_actions_from_real_riscv_execution() {
    let program = riscv64_program(&[
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        i_type(12, 0, 0x0, 10, 0x13),
        i_type(8, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(12, 0, 0x0, 10, 0x13),
        i_type(8, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("host_stop")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(9)
    );
    assert_eq!(
        host_actions
            .pointer("/roi_begin_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/roi_end_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_work_marker(host_actions, "roi_begin", 0, 11, 7);
    assert_work_marker(host_actions, "roi_end", 0, 11, 7);
    assert_work_marker(host_actions, "roi_begin", 1, 12, 8);
    assert_work_marker(host_actions, "roi_end", 1, 12, 8);
}

#[test]
fn rem6_run_text_stats_map_m5_work_markers_to_gem5_cpu_work_item_aliases() {
    let program = riscv64_program(&[
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        i_type(12, 0, 0x0, 10, 0x13),
        i_type(8, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(12, 0, 0x0, 10, 0x13),
        i_type(8, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-work-item-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.host_actions.roi_begin", 2);
    assert_text_count_stat(&stdout, "sim.host_actions.roi_end", 2);
    assert_text_count_stat(&stdout, "system.cpu.numWorkItemsStarted", 2);
    assert_text_count_stat(&stdout, "system.cpu.numWorkItemsCompleted", 2);
}

#[test]
fn rem6_run_text_stats_emit_m5_work_item_duration_histogram_aliases() {
    let program = riscv64_program(&[
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(2, 5, 0x0, 5, 0x13),
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-work-item-duration-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let duration_bucket = assert_text_histogram_sample(
        &stdout,
        "sim.host_actions.roi_work_item_type11.duration_ticks",
        "Tick",
        1,
    );
    assert_text_histogram_sample(&stdout, "system.work_item_type11", "Tick", 1);
    assert_text_histogram_bucket(
        &stdout,
        "system.work_item_type11",
        "Tick",
        duration_bucket,
        1,
    );
}

#[test]
fn rem6_run_emits_m5_hypercall_host_action_detail_from_real_riscv_execution() {
    let program = riscv64_program(&[
        i_type(0x321, 0, 0x0, 10, 0x13),
        i_type(11, 0, 0x0, 11, 0x13),
        i_type(12, 0, 0x0, 12, 0x13),
        i_type(13, 0, 0x0, 13, 0x13),
        i_type(14, 0, 0x0, 14, 0x13),
        i_type(15, 0, 0x0, 15, 0x13),
        m5op(M5_HYPERCALL),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-hypercall-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/guest_host_call_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    let call = host_actions
        .pointer("/guest_host_calls/0")
        .expect("missing guest-host-call detail");
    assert_eq!(
        call.pointer("/selector").and_then(Value::as_u64),
        Some(0x321)
    );
    assert_eq!(
        call.pointer("/argument_count").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        call.pointer("/arguments")
            .and_then(Value::as_array)
            .cloned(),
        Some(vec![
            Value::from(11),
            Value::from(12),
            Value::from(13),
            Value::from(14),
            Value::from(15),
        ])
    );
    assert_eq!(
        call.pointer("/payload_bytes").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        call.pointer("/response_status").and_then(Value::as_i64),
        Some(-1)
    );
    assert_eq!(
        call.pointer("/response_return_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        call.pointer("/response_payload_bytes")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert!(call.pointer("/tick").and_then(Value::as_u64).is_some());
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_arguments",
        "Count",
        call.pointer("/argument_count")
            .and_then(Value::as_u64)
            .expect("missing hypercall argument count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_payload_bytes",
        "Byte",
        call.pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .expect("missing hypercall payload byte count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_response_return_values",
        "Count",
        call.pointer("/response_return_count")
            .and_then(Value::as_u64)
            .expect("missing hypercall response return count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_response_payload_bytes",
        "Byte",
        call.pointer("/response_payload_bytes")
            .and_then(Value::as_u64)
            .expect("missing hypercall response payload byte count"),
        "monotonic",
    );
}

#[test]
fn rem6_run_applies_configured_m5_hypercall_response_from_toml_config() {
    let program = riscv64_program(&[
        i_type(0x321, 0, 0x0, 10, 0x13),
        i_type(11, 0, 0x0, 11, 0x13),
        i_type(12, 0, 0x0, 12, 0x13),
        m5op(M5_HYPERCALL),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("m5-hypercall-response-toml", &elf);
    let config = temp_config(
        "m5-hypercall-response-toml",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 80\nstats_format = \"json\"\nexecute = true\nmemory_system = \"direct\"\nguest_host_call_responses = [\"selector=0x321,status=0,returns=0x55|0x66,payload=deadbeef\"]\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    let call = host_actions
        .pointer("/guest_host_calls/0")
        .expect("missing guest-host-call detail");
    assert_eq!(
        call.pointer("/selector").and_then(Value::as_u64),
        Some(0x321)
    );
    assert_eq!(
        call.pointer("/argument_count").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        call.pointer("/response_status").and_then(Value::as_i64),
        Some(0)
    );
    assert_eq!(
        call.pointer("/response_return_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        call.pointer("/response_payload_bytes")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_arguments",
        "Count",
        5,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_payload_bytes",
        "Byte",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_response_return_values",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.guest_host_call_response_payload_bytes",
        "Byte",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_run_executes_m5_switch_cpu_mode_transfer_from_real_riscv_execution() {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(0, 0, 0x0, 10, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(1, 0, 0x0, 10, 0x13),
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 11, 0x13),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        host_actions
            .pointer("/injected_command_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        host_actions
            .pointer("/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    let (switch_tick, switch_transfer_label) = assert_execution_mode_switch(
        host_actions,
        0,
        "cpu0",
        None,
        "detailed",
        "execution-mode-switch-cpu0",
    );
    let (second_switch_tick, second_switch_transfer_label) = assert_execution_mode_switch(
        host_actions,
        1,
        "cpu0",
        Some("detailed"),
        "detailed",
        "execution-mode-switch-cpu0",
    );
    let stop_tick = host_actions
        .pointer("/stops/0/tick")
        .and_then(Value::as_u64)
        .expect("m5_exit stop should be recorded after switch");
    assert!(
        second_switch_tick > switch_tick,
        "repeated m5_switch_cpu should record a later mode switch: {host_actions}"
    );
    assert_ne!(
        second_switch_transfer_label, switch_transfer_label,
        "repeated m5_switch_cpu should capture distinct transfer manifests: {host_actions}"
    );
    assert!(
        stop_tick > second_switch_tick,
        "m5_switch_cpu should switch modes and continue to the later m5_exit: {host_actions}"
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfers",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer_components",
        "Count",
        execution_mode_switch_transfer_total(host_actions, "component_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer_chunks",
        "Count",
        execution_mode_switch_transfer_total(host_actions, "chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer_payload_bytes",
        "Byte",
        execution_mode_switch_transfer_total(host_actions, "payload_bytes"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.components",
        "Count",
        execution_mode_switch_transfer_component_total(host_actions, "cpu0", "component_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.chunks",
        "Count",
        execution_mode_switch_transfer_component_total(host_actions, "cpu0", "chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.payload_bytes",
        "Byte",
        execution_mode_switch_transfer_component_total(host_actions, "cpu0", "payload_bytes"),
        "monotonic",
    );
    for chunk in ["in-order-pipeline", "o3-pending-state", "o3-runtime-state"] {
        let stat_chunk = stat_path_segment(chunk);
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.chunk.{stat_chunk}.chunks"
            ),
            "Count",
            execution_mode_switch_transfer_component_chunk_total(
                host_actions,
                "cpu0",
                chunk,
                "chunk_count",
            ),
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.chunk.{stat_chunk}.payload_bytes"
            ),
            "Byte",
            execution_mode_switch_transfer_component_chunk_total(
                host_actions,
                "cpu0",
                chunk,
                "payload_bytes",
            ),
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_records_o3_runtime_stats_after_detailed_switch() {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        u_type(0, 5, 0x17),            // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),   // addi x5, x5, data
        i_type(7, 0, 0x0, 11, 0x13),   // addi x11, x0, 7
        i_type(0, 5, 0b010, 12, 0x03), // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),       // sw x12, 4(x5)
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13), // addi x13, x0, 77
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-detailed-o3-runtime-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_json_stat(&json, "sim.cpu0.o3.instructions", "Count", 6, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rob_allocations",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(&json, "sim.cpu0.o3.rob_commits", "Count", 6, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.rename_writes", "Count", 4, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 1, "monotonic");
}

#[test]
fn rem6_run_does_not_record_o3_runtime_stats_after_timing_switch() {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-timing-no-o3-runtime-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--m5-switch-cpu-mode",
            "timing",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.rob_allocations");
}

#[test]
fn rem6_run_executes_m5_switch_cpu_timing_mode_from_real_riscv_execution() {
    let program = riscv64_program(&[m5op(M5_SWITCH_CPU), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-timing-mode", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--m5-switch-cpu-mode",
            "timing",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    let (switch_tick, transfer_label) = assert_execution_mode_switch(
        host_actions,
        0,
        "cpu0",
        None,
        "timing",
        "execution-mode-switch-cpu0",
    );
    assert!(
        host_actions
            .pointer("/execution_mode_switches/0/state_transfer/quiescence_gate/checker")
            .is_none(),
        "non-checker timing-mode switch should not publish checker quiescence data: {host_actions}"
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch_quiescence.checker.checked_instructions",
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch_quiescence.checker.mismatches",
    );
    assert!(transfer_label.ends_with(&format!("-{switch_tick}")));
    assert!(
        host_actions
            .pointer("/stops/0/tick")
            .and_then(Value::as_u64)
            .is_some_and(|stop_tick| stop_tick > switch_tick),
        "m5_switch_cpu timing mode should continue to m5_exit: {host_actions}"
    );
}

#[test]
fn rem6_run_executes_checker_cpu_across_m5_timing_mode_switch() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(5, 5, 0x0, 6, 0x13),
        i_type(1, 6, 0x0, 7, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(1, 7, 0x0, 8, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-timing-checker", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--checker-cpu",
            "--m5-switch-cpu-mode",
            "timing",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    let (switch_tick, _) = assert_execution_mode_switch(
        host_actions,
        0,
        "cpu0",
        None,
        "timing",
        "execution-mode-switch-cpu0",
    );
    assert!(
        host_actions
            .pointer("/stops/0/tick")
            .and_then(Value::as_u64)
            .is_some_and(|stop_tick| stop_tick > switch_tick),
        "checker timing-mode switch should continue to m5_exit: {host_actions}"
    );

    let gate_checker = host_actions
        .pointer("/execution_mode_switches/0/state_transfer/quiescence_gate/checker")
        .unwrap_or_else(|| panic!("missing checker quiescence gate: {host_actions}"));
    let checked_at_switch = gate_checker
        .pointer("/checked_instructions")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checker quiescence checked count: {gate_checker}"));
    assert!(
        checked_at_switch >= 2,
        "checker quiescence should include instructions retired through m5_switch_cpu: {gate_checker}"
    );
    assert_eq!(
        gate_checker.pointer("/mismatches").and_then(Value::as_u64),
        Some(0),
        "checker quiescence should preserve zero mismatches: {gate_checker}"
    );
    let second_gate_checker = host_actions
        .pointer("/execution_mode_switches/1/state_transfer/quiescence_gate/checker")
        .unwrap_or_else(|| panic!("missing second checker quiescence gate: {host_actions}"));
    let second_checked_at_switch = second_gate_checker
        .pointer("/checked_instructions")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!("missing second checker quiescence checked count: {second_gate_checker}")
        });
    assert!(
        second_checked_at_switch > checked_at_switch,
        "second switch should capture later checker progress without resetting: {host_actions}"
    );
    assert_eq!(
        second_gate_checker
            .pointer("/mismatches")
            .and_then(Value::as_u64),
        Some(0),
        "second checker quiescence should preserve zero mismatches: {second_gate_checker}"
    );

    let final_checker = json
        .pointer("/cores/0/checker")
        .unwrap_or_else(|| panic!("missing final checker summary: {json}"));
    let final_checked = final_checker
        .pointer("/checked_instructions")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing final checker count: {final_checker}"));
    assert!(
        final_checked > second_checked_at_switch,
        "checker should continue checking instructions after timing-mode switch: {json}"
    );
    assert_eq!(
        final_checker.pointer("/mismatches").and_then(Value::as_u64),
        Some(0),
        "checker should preserve zero mismatches after timing-mode switch: {json}"
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_quiescence.checker.checked_instructions",
        "Count",
        second_checked_at_switch,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_quiescence.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_loads_m5_switch_cpu_mode_from_toml_config() {
    let program = riscv64_program(&[m5op(M5_SWITCH_CPU), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("m5-switch-cpu-toml-mode", &elf);
    let config = temp_config(
        "m5-switch-cpu-toml-mode",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 80\nstats_format = \"json\"\nexecute = true\nmemory_system = \"direct\"\nm5_switch_cpu_mode = \"functional\"\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    let (switch_tick, transfer_label) = assert_execution_mode_switch(
        host_actions,
        0,
        "cpu0",
        None,
        "functional",
        "execution-mode-switch-cpu0",
    );
    assert!(transfer_label.ends_with(&format!("-{switch_tick}")));
    assert!(
        host_actions
            .pointer("/stops/0/tick")
            .and_then(Value::as_u64)
            .is_some_and(|stop_tick| stop_tick > switch_tick),
        "TOML m5_switch_cpu mode should continue to m5_exit: {host_actions}"
    );
}

#[test]
fn rem6_run_executes_m5_sum_return_value_from_real_riscv_execution() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 10, 0x13), // addi a0, x0, 1
        i_type(2, 0, 0x0, 11, 0x13), // addi a1, x0, 2
        i_type(3, 0, 0x0, 12, 0x13), // addi a2, x0, 3
        i_type(4, 0, 0x0, 13, 0x13), // addi a3, x0, 4
        i_type(5, 0, 0x0, 14, 0x13), // addi a4, x0, 5
        i_type(6, 0, 0x0, 15, 0x13), // addi a5, x0, 6
        m5op(M5_SUM),
        i_type(21, 0, 0x0, 5, 0x13), // addi t0, x0, 21
        b_type(12, 5, 10, 0x1),      // bne a0, t0, fail
        i_type(0, 0, 0x0, 10, 0x13), // addi a0, x0, 0
        m5op(M5_EXIT),
        i_type(99, 0, 0x0, 11, 0x13), // addi a1, x0, 99
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-sum-return-value", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("host_stop")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_run_emits_m5_stats_host_action_details_from_real_riscv_execution() {
    let program = riscv64_program(&[
        m5op(M5_RESET_STATS),
        m5op(M5_DUMP_STATS),
        m5op(M5_DUMP_RESET_STATS),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-stats-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_stats_reset(host_actions, 0, 0, 3, 1);
    assert_stats_dump(host_actions, 0, 0, 5, 1, 3);
    assert_stats_dump(host_actions, 1, 1, 7, 1, 3);
    assert_stats_reset(host_actions, 1, 1, 7, 2);
}

#[test]
fn rem6_run_repeats_m5_stats_host_actions_when_period_is_set_from_real_riscv_execution() {
    let mut words = vec![
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(4, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
        i_type(18, 0, 0x0, 10, 0x13),
        m5op(M5_EXIT),
    ];
    words.extend(std::iter::repeat_n(i_type(0, 0, 0x0, 0, 0x13), 16));
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-periodic-stats-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );

    let reset_ticks = action_ticks(host_actions, "stats_resets");
    let dump_ticks = action_ticks(host_actions, "stats_dumps");
    assert_eq!(reset_ticks, vec![7, 11, 15, 19, 23, 27]);
    assert_eq!(dump_ticks, reset_ticks);
}

#[test]
fn rem6_run_emits_m5_checkpoint_host_action_detail_from_real_riscv_execution() {
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-checkpoint-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_checkpoint(host_actions, 0, "gem5-m5-checkpoint", 3, 3);
    assert_checkpoint_component_chunks(
        host_actions,
        0,
        0,
        "cpu0",
        &[
            "bimode-branch-predictor",
            "branch-predictor",
            "fregs",
            "gshare-branch-predictor",
            "hart-run-state",
            "in-order-pipeline",
            "multiperspective-perceptron",
            "o3-pending-state",
            "o3-runtime-state",
            "pc",
            "pmp",
            "tage-sc-l-branch-predictor",
            "tournament-branch-predictor",
            "xregs",
        ],
    );
    assert_checkpoint_component_chunks(host_actions, 0, 1, "memory0", &["store"]);
    assert_checkpoint_counts_match_nested_details(host_actions, 0);
}

#[test]
fn rem6_run_m5_store_checkpoint_chunk_checksum_tracks_live_memory_state() {
    let (baseline, after_store) =
        run_m5_checkpoint_memory_checksums("m5-store-checkpoint-live", false);

    assert_ne!(after_store, baseline);
}

#[test]
fn rem6_run_emits_m5_dram_checkpoint_host_action_detail_from_real_riscv_execution() {
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-dram-checkpoint-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--dram-memory",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_checkpoint(host_actions, 0, "gem5-m5-checkpoint", 11, 11);
    assert_checkpoint_component_chunks(
        host_actions,
        0,
        0,
        "cpu0",
        &[
            "bimode-branch-predictor",
            "branch-predictor",
            "fregs",
            "gshare-branch-predictor",
            "hart-run-state",
            "in-order-pipeline",
            "multiperspective-perceptron",
            "o3-pending-state",
            "o3-runtime-state",
            "pc",
            "pmp",
            "tage-sc-l-branch-predictor",
            "tournament-branch-predictor",
            "xregs",
        ],
    );
    assert_checkpoint_component_chunks(host_actions, 0, 1, "memory0", &["dram"]);
    assert_checkpoint_counts_match_nested_details(host_actions, 0);
}

#[test]
fn rem6_run_m5_dram_checkpoint_chunk_checksum_tracks_live_memory_state() {
    let (baseline, after_store) =
        run_m5_checkpoint_memory_checksums("m5-dram-checkpoint-live", true);

    assert_ne!(after_store, baseline);
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}

fn run_m5_checkpoint_memory_checksums(name: &str, dram_memory: bool) -> (String, String) {
    let words = [
        m5op(M5_CHECKPOINT),
        u_type(0, 2, 0x17),            // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13),   // addi x2, x2, data offset
        i_type(0x5a, 0, 0x0, 5, 0x13), // addi x5, x0, 0x5a
        s_type(0, 5, 2, 0x2),          // sw x5, 0(x2)
        m5op(M5_CHECKPOINT),
        m5op(M5_EXIT),
    ];
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);
    let mut args = vec![
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "120",
        "--stats-format",
        "json",
        "--execute",
    ];
    if dram_memory {
        args.push("--dram-memory");
    } else {
        args.extend(["--memory-system", "direct"]);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let chunk_name = if dram_memory { "dram" } else { "store" };
    (
        checkpoint_chunk_checksum(host_actions, 0, "memory0", chunk_name),
        checkpoint_chunk_checksum(host_actions, 1, "memory0", chunk_name),
    )
}

fn assert_work_marker(
    host_actions: &Value,
    field: &str,
    index: usize,
    work_id: u64,
    thread_id: u64,
) {
    let action = host_actions
        .pointer(&format!("/{field}/{index}"))
        .unwrap_or_else(|| panic!("missing host action {field}[{index}]"));
    assert_eq!(
        action.pointer("/work_id").and_then(Value::as_u64),
        Some(work_id)
    );
    assert_eq!(
        action.pointer("/thread_id").and_then(Value::as_u64),
        Some(thread_id)
    );
    assert!(action.pointer("/tick").and_then(Value::as_u64).is_some());
}

fn assert_text_count_stat(stdout: &str, path: &str, value: u64) {
    let line = stdout
        .lines()
        .find(|line| line.split_whitespace().next() == Some(path))
        .unwrap_or_else(|| panic!("missing text stat {path} in stdout:\n{stdout}"));
    let actual = line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok());
    assert_eq!(
        actual,
        Some(value),
        "unexpected text stat value for {path} in line: {line}"
    );
    assert!(
        line.contains("unit=Count"),
        "missing Count unit for {path} in line: {line}"
    );
    assert!(
        line.contains("reset_policy=monotonic"),
        "missing monotonic reset policy for {path} in line: {line}"
    );
}

fn assert_text_histogram_sample(stdout: &str, path: &str, unit: &str, value: u64) -> u64 {
    let line = text_stat_line(stdout, path);
    let actual = line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok());
    assert_eq!(
        actual,
        Some(value),
        "unexpected text histogram sample count for {path} in line: {line}"
    );
    assert!(
        line.contains("kind=histogram"),
        "missing histogram kind for {path} in line: {line}"
    );
    assert!(
        line.contains(&format!("unit={unit}")),
        "missing {unit} unit for {path} in line: {line}"
    );

    text_histogram_bucket_lines(stdout, path)
        .into_iter()
        .find_map(|line| histogram_bucket_and_count(line).map(|(bucket, _)| bucket))
        .unwrap_or_else(|| panic!("missing histogram bucket for {path} in stdout:\n{stdout}"))
}

fn assert_text_histogram_bucket(stdout: &str, path: &str, unit: &str, bucket: u64, count: u64) {
    let bucket_lines = text_histogram_bucket_lines(stdout, path);
    let line = bucket_lines
        .iter()
        .copied()
        .find(|line| histogram_bucket_and_count(line) == Some((bucket, count)))
        .unwrap_or_else(|| {
            panic!("missing histogram bucket {bucket}={count} for {path} in stdout:\n{stdout}")
        });
    assert!(
        line.contains(&format!("unit={unit}")),
        "missing {unit} unit for {path} bucket in line: {line}"
    );
    assert!(
        line.contains("reset_policy=monotonic"),
        "missing monotonic reset policy for {path} bucket in line: {line}"
    );
}

fn text_stat_line<'a>(stdout: &'a str, path: &str) -> &'a str {
    stdout
        .lines()
        .find(|line| line.split_whitespace().next() == Some(path))
        .unwrap_or_else(|| panic!("missing text stat {path} in stdout:\n{stdout}"))
}

fn text_histogram_bucket_lines<'a>(stdout: &'a str, path: &str) -> Vec<&'a str> {
    let bucket_path = format!("{path}.bucket");
    stdout
        .lines()
        .filter(|line| line.split_whitespace().next() == Some(bucket_path.as_str()))
        .collect()
}

fn histogram_bucket_and_count(line: &str) -> Option<(u64, u64)> {
    let count = line.split_whitespace().nth(1)?.parse().ok()?;
    let bucket = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("histogram_bucket="))?
        .parse()
        .ok()?;
    Some((bucket, count))
}

fn assert_execution_mode_switch(
    host_actions: &Value,
    index: usize,
    target: &str,
    previous_mode: Option<&str>,
    mode: &str,
    manifest_label_prefix: &str,
) -> (u64, String) {
    let action = host_actions
        .pointer(&format!("/execution_mode_switches/{index}"))
        .unwrap_or_else(|| panic!("missing execution mode switch action {index}"));
    assert_eq!(
        action.pointer("/target").and_then(Value::as_str),
        Some(target)
    );
    assert_eq!(action.pointer("/mode").and_then(Value::as_str), Some(mode));
    match previous_mode {
        Some(previous_mode) => assert_eq!(
            action.pointer("/previous_mode").and_then(Value::as_str),
            Some(previous_mode),
            "execution mode switch action {index}: {action}"
        ),
        None => assert!(
            action.pointer("/previous_mode").is_some_and(Value::is_null),
            "execution mode switch action {index}: {action}"
        ),
    }
    assert!(
        action
            .pointer("/stats_epoch")
            .and_then(Value::as_u64)
            .is_some(),
        "execution mode switch action {index}: {action}"
    );
    assert!(
        action
            .pointer("/stats_reset_tick")
            .and_then(Value::as_u64)
            .is_some(),
        "execution mode switch action {index}: {action}"
    );
    let transfer = action
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing execution mode state transfer {index}"));
    assert!(
        transfer.pointer("/captured").and_then(Value::as_bool) == Some(true),
        "execution mode switch transfer {index}: {transfer}"
    );
    let manifest_label = transfer
        .pointer("/manifest_label")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing execution mode transfer manifest label {index}"));
    assert!(
        manifest_label.starts_with(manifest_label_prefix),
        "execution mode switch transfer {index}: {transfer}"
    );
    assert!(
        transfer
            .pointer("/component_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "execution mode switch transfer {index}: {transfer}"
    );
    assert!(
        transfer
            .pointer("/chunk_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "execution mode switch transfer {index}: {transfer}"
    );
    assert!(
        transfer
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "execution mode switch transfer {index}: {transfer}"
    );
    assert!(
        transfer
            .pointer("/components")
            .and_then(Value::as_array)
            .is_some_and(|components| components.len() >= 2),
        "execution mode switch transfer {index}: {transfer}"
    );
    assert!(
        transfer
            .pointer("/components/0/component")
            .and_then(Value::as_str)
            .is_some(),
        "execution mode switch transfer {index}: {transfer}"
    );
    assert!(
        transfer
            .pointer("/components/0/chunks/0/payload_checksum")
            .and_then(Value::as_str)
            .is_some_and(|checksum| checksum.starts_with("0x") && checksum.len() == 18),
        "execution mode switch transfer {index}: {transfer}"
    );
    let quiescence_gate = transfer
        .pointer("/quiescence_gate")
        .unwrap_or_else(|| panic!("missing execution mode switch quiescence gate {index}"));
    assert_eq!(
        quiescence_gate
            .pointer("/validated")
            .and_then(Value::as_bool),
        Some(true),
        "execution mode switch quiescence gate {index}: {quiescence_gate}"
    );
    assert_eq!(
        quiescence_gate.pointer("/target").and_then(Value::as_str),
        Some(target),
        "execution mode switch quiescence gate {index}: {quiescence_gate}"
    );
    assert_eq!(
        quiescence_gate
            .pointer("/captured_component_count")
            .and_then(Value::as_u64),
        transfer.pointer("/component_count").and_then(Value::as_u64),
        "execution mode switch quiescence gate {index}: {quiescence_gate}"
    );
    assert_eq!(
        quiescence_gate
            .pointer("/captured_chunk_count")
            .and_then(Value::as_u64),
        transfer.pointer("/chunk_count").and_then(Value::as_u64),
        "execution mode switch quiescence gate {index}: {quiescence_gate}"
    );
    assert_eq!(
        quiescence_gate
            .pointer("/captured_payload_bytes")
            .and_then(Value::as_u64),
        transfer.pointer("/payload_bytes").and_then(Value::as_u64),
        "execution mode switch quiescence gate {index}: {quiescence_gate}"
    );
    (
        action
            .pointer("/tick")
            .and_then(Value::as_u64)
            .expect("execution mode switch should record a tick"),
        manifest_label.to_string(),
    )
}

fn assert_stats_reset(host_actions: &Value, index: usize, id: u64, tick: u64, epoch: u64) {
    let action = host_actions
        .pointer(&format!("/stats_resets/{index}"))
        .unwrap_or_else(|| panic!("missing stats reset action {index}"));
    assert_eq!(action.pointer("/id").and_then(Value::as_u64), Some(id));
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(tick),
        "stats reset action {index}: {action}"
    );
    assert_eq!(
        action.pointer("/epoch").and_then(Value::as_u64),
        Some(epoch)
    );
}

fn action_ticks(host_actions: &Value, field: &str) -> Vec<u64> {
    host_actions
        .pointer(&format!("/{field}"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing host action list {field}"))
        .iter()
        .map(|action| {
            action
                .pointer("/tick")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing host action tick in {field}: {action}"))
        })
        .collect()
}

fn execution_mode_switch_transfer_total(host_actions: &Value, field: &str) -> u64 {
    host_actions
        .pointer("/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution mode switch actions: {host_actions}"))
        .iter()
        .map(|action| {
            action
                .pointer(&format!("/state_transfer/{field}"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing switch transfer {field}: {action}"))
        })
        .sum()
}

fn execution_mode_switch_transfer_component_total(
    host_actions: &Value,
    component: &str,
    field: &str,
) -> u64 {
    host_actions
        .pointer("/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution mode switch actions: {host_actions}"))
        .iter()
        .map(|action| {
            let components = action
                .pointer("/state_transfer/components")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("missing switch transfer components: {action}"));
            let component = components
                .iter()
                .find(|entry| {
                    entry.pointer("/component").and_then(Value::as_str) == Some(component)
                })
                .unwrap_or_else(|| {
                    panic!("missing switch transfer component {component}: {action}")
                });
            match field {
                "component_count" => 1,
                "chunk_count" => component
                    .pointer("/chunk_count")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| panic!("missing switch component chunk_count: {component}")),
                "payload_bytes" => component
                    .pointer("/payload_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| {
                        panic!("missing switch component payload_bytes: {component}")
                    }),
                _ => panic!("unsupported switch component field {field}"),
            }
        })
        .sum()
}

fn execution_mode_switch_transfer_component_chunk_total(
    host_actions: &Value,
    component: &str,
    chunk: &str,
    field: &str,
) -> u64 {
    host_actions
        .pointer("/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution mode switch actions: {host_actions}"))
        .iter()
        .map(|action| {
            let components = action
                .pointer("/state_transfer/components")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("missing switch transfer components: {action}"));
            let component = components
                .iter()
                .find(|entry| {
                    entry.pointer("/component").and_then(Value::as_str) == Some(component)
                })
                .unwrap_or_else(|| {
                    panic!("missing switch transfer component {component}: {action}")
                });
            let chunks = component
                .pointer("/chunks")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("missing switch transfer chunks: {component}"));
            let chunk = chunks
                .iter()
                .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
                .unwrap_or_else(|| panic!("missing switch transfer chunk {chunk}: {component}"));
            match field {
                "chunk_count" => 1,
                "payload_bytes" => chunk
                    .pointer("/payload_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| panic!("missing switch chunk payload_bytes: {chunk}")),
                _ => panic!("unsupported switch chunk field {field}"),
            }
        })
        .sum()
}

fn assert_json_stat(json: &Value, path: &str, unit: &str, value: u64, reset_policy: &str) {
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array in run JSON: {json}"));
    let matches = stats
        .iter()
        .filter(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected one stat path {path}, found {} in {stats:?}",
        matches.len()
    );
    let sample = matches[0];
    assert_eq!(
        sample.pointer("/unit").and_then(Value::as_str),
        Some(unit),
        "unexpected unit for {path}: {sample}"
    );
    assert_eq!(
        sample.pointer("/value").and_then(Value::as_u64),
        Some(value),
        "unexpected value for {path}: {sample}"
    );
    assert_eq!(
        sample.pointer("/reset_policy").and_then(Value::as_str),
        Some(reset_policy),
        "unexpected reset policy for {path}: {sample}"
    );
}

fn assert_json_stat_absent(json: &Value, path: &str) {
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array in run JSON: {json}"));
    assert!(
        stats
            .iter()
            .all(|sample| sample.pointer("/path").and_then(Value::as_str) != Some(path)),
        "unexpected stat path {path} in {stats:?}"
    );
}

fn assert_stats_dump(
    host_actions: &Value,
    index: usize,
    id: u64,
    tick: u64,
    epoch: u64,
    reset_tick: u64,
) {
    let action = host_actions
        .pointer(&format!("/stats_dumps/{index}"))
        .unwrap_or_else(|| panic!("missing stats dump action {index}"));
    assert_eq!(action.pointer("/id").and_then(Value::as_u64), Some(id));
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(tick),
        "stats dump action {index}: {action}"
    );
    assert_eq!(
        action.pointer("/epoch").and_then(Value::as_u64),
        Some(epoch)
    );
    assert_eq!(
        action.pointer("/reset_tick").and_then(Value::as_u64),
        Some(reset_tick),
        "stats dump action {index}: {action}"
    );
}

fn assert_checkpoint(
    host_actions: &Value,
    index: usize,
    label: &str,
    tick: u64,
    manifest_tick: u64,
) {
    let action = host_actions
        .pointer(&format!("/checkpoints/{index}"))
        .unwrap_or_else(|| panic!("missing checkpoint action {index}"));
    assert_eq!(
        action.pointer("/label").and_then(Value::as_str),
        Some(label)
    );
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(tick),
        "checkpoint action {index}: {action}"
    );
    assert_eq!(
        action.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(manifest_tick),
        "checkpoint action {index}: {action}"
    );
    assert!(action.pointer("/event").and_then(Value::as_u64).is_some());
    assert!(action.pointer("/source").and_then(Value::as_u64).is_some());
    assert!(
        action
            .pointer("/component_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "checkpoint action {index}: {action}"
    );
    assert!(
        action
            .pointer("/chunk_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "checkpoint action {index}: {action}"
    );
    assert!(
        action
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "checkpoint action {index}: {action}"
    );
}

fn assert_checkpoint_component_chunks(
    host_actions: &Value,
    checkpoint_index: usize,
    component_index: usize,
    component: &str,
    chunks: &[&str],
) {
    let component_summary = host_actions
        .pointer(&format!(
            "/checkpoints/{checkpoint_index}/components/{component_index}"
        ))
        .unwrap_or_else(|| {
            panic!("missing checkpoint component {checkpoint_index}/{component_index}")
        });
    assert_eq!(
        component_summary
            .pointer("/component")
            .and_then(Value::as_str),
        Some(component)
    );
    assert_eq!(
        component_summary
            .pointer("/chunk_count")
            .and_then(Value::as_u64),
        Some(chunks.len() as u64)
    );
    assert!(
        component_summary
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "checkpoint component {component_index}: {component_summary}"
    );
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        let chunk_summary = component_summary
            .pointer(&format!("/chunks/{chunk_index}"))
            .unwrap_or_else(|| panic!("missing checkpoint chunk {chunk_index}"));
        assert_eq!(
            chunk_summary.pointer("/name").and_then(Value::as_str),
            Some(*chunk)
        );
        assert!(
            chunk_summary
                .pointer("/payload_bytes")
                .and_then(Value::as_u64)
                .is_some_and(|bytes| bytes > 0),
            "checkpoint chunk {chunk_index}: {chunk_summary}"
        );
        assert!(
            chunk_summary
                .pointer("/payload_checksum")
                .and_then(Value::as_str)
                .is_some_and(|checksum| checksum.starts_with("0x") && checksum.len() == 18),
            "checkpoint chunk {chunk_index}: {chunk_summary}"
        );
    }
}

fn checkpoint_chunk_checksum(
    host_actions: &Value,
    checkpoint_index: usize,
    component: &str,
    chunk: &str,
) -> String {
    let components = host_actions
        .pointer(&format!("/checkpoints/{checkpoint_index}/components"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint components {checkpoint_index}"));
    let component_summary = components
        .iter()
        .find(|summary| summary.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing checkpoint component {component}"));
    let chunks = component_summary
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint chunks for {component}"));
    chunks
        .iter()
        .find(|summary| summary.pointer("/name").and_then(Value::as_str) == Some(chunk))
        .and_then(|summary| summary.pointer("/payload_checksum"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing checkpoint chunk checksum {component}/{chunk}"))
        .to_string()
}

fn assert_checkpoint_counts_match_nested_details(host_actions: &Value, checkpoint_index: usize) {
    let checkpoint = host_actions
        .pointer(&format!("/checkpoints/{checkpoint_index}"))
        .unwrap_or_else(|| panic!("missing checkpoint action {checkpoint_index}"));
    let components = checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint components {checkpoint_index}"));
    let component_count = components.len() as u64;
    let chunk_count = components
        .iter()
        .map(|component| {
            component
                .pointer("/chunks")
                .and_then(Value::as_array)
                .map_or(0, |chunks| chunks.len() as u64)
        })
        .sum::<u64>();
    assert_eq!(
        checkpoint
            .pointer("/component_count")
            .and_then(Value::as_u64),
        Some(component_count),
        "checkpoint action {checkpoint_index}: {checkpoint}"
    );
    assert_eq!(
        checkpoint.pointer("/chunk_count").and_then(Value::as_u64),
        Some(chunk_count),
        "checkpoint action {checkpoint_index}: {checkpoint}"
    );
}
