use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::support::*;

#[path = "m5_host_actions/o3.rs"]
mod o3;

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
        "sim.host_actions.execution_mode_switch.target.cpu0.mode.detailed",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.none",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.detailed",
        "Count",
        1,
        "monotonic",
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
fn rem6_run_stats_expose_m5_switch_cpu_arch_and_o3_transfer_checksums() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13),  // addi x5, x0, 7 before switch
        m5op(M5_SWITCH_CPU),         // capture architectural baseline, enter detailed
        i_type(1, 5, 0x0, 5, 0x13),  // addi x5, x5, 1 in detailed mode
        i_type(2, 5, 0x0, 6, 0x13),  // addi x6, x5, 2 in detailed mode
        m5op(M5_SWITCH_CPU),         // capture changed xregs plus O3 runtime state
        i_type(3, 6, 0x0, 7, 0x13),  // addi x7, x6, 3 after second switch
        i_type(0, 0, 0x0, 10, 0x13), // addi a0, x0, 0
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-arch-o3-transfer-checksums", &elf);

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
        json.pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some("0x8")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x6")
            .and_then(Value::as_str),
        Some("0xa")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x7")
            .and_then(Value::as_str),
        Some("0xd")
    );
    assert!(
        json.pointer("/cores/0/o3_runtime/instructions")
            .and_then(Value::as_u64)
            .is_some_and(|instructions| instructions >= 5),
        "detailed-mode switch should record post-switch O3 instructions: {json}"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    let first_xregs =
        execution_mode_switch_transfer_component_chunk_checksum(host_actions, 0, "cpu0", "xregs");
    let second_xregs =
        execution_mode_switch_transfer_component_chunk_checksum(host_actions, 1, "cpu0", "xregs");
    let first_o3 = execution_mode_switch_transfer_component_chunk_checksum(
        host_actions,
        0,
        "cpu0",
        "o3-runtime-state",
    );
    let second_o3 = execution_mode_switch_transfer_component_chunk_checksum(
        host_actions,
        1,
        "cpu0",
        "o3-runtime-state",
    );
    assert_ne!(
        second_xregs, first_xregs,
        "second switch should capture changed architectural registers: {host_actions}"
    );
    assert_ne!(
        second_o3, first_o3,
        "second switch should capture changed detailed O3 runtime state: {host_actions}"
    );

    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.chunk.xregs.payload_checksum_accumulator",
        "Unspecified",
        parse_hex_u64(&first_xregs).wrapping_add(parse_hex_u64(&second_xregs)),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.component.cpu0.chunk.o3_runtime_state.payload_checksum_accumulator",
        "Unspecified",
        parse_hex_u64(&first_o3).wrapping_add(parse_hex_u64(&second_o3)),
        "monotonic",
    );
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
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.checker.checked_instructions",
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.checker.mismatches",
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
fn rem6_run_stats_expose_m5_switch_cpu_mode_authority_matrix() {
    for mode in ["detailed", "timing", "functional"] {
        let program = riscv64_program(&[
            m5op(M5_SWITCH_CPU),
            i_type(0, 0, 0x0, 10, 0x13),
            m5op(M5_EXIT),
        ]);
        let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
        let path = temp_binary(&format!("m5-switch-cpu-mode-authority-{mode}"), &elf);
        let mut args = vec![
            "run".to_string(),
            "--isa".to_string(),
            "riscv".to_string(),
            "--binary".to_string(),
            path.to_str().unwrap().to_string(),
            "--max-tick".to_string(),
            "80".to_string(),
            "--stats-format".to_string(),
            "json".to_string(),
            "--execute".to_string(),
            "--memory-system".to_string(),
            "direct".to_string(),
        ];
        if mode != "detailed" {
            args.push("--m5-switch-cpu-mode".to_string());
            args.push(mode.to_string());
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
            mode,
            "execution-mode-switch-cpu0",
        );
        assert!(
            host_actions
                .pointer("/stops/0/tick")
                .and_then(Value::as_u64)
                .is_some_and(|stop_tick| stop_tick > switch_tick),
            "m5_switch_cpu {mode} mode should continue to m5_exit: {host_actions}"
        );
        let execution_modes = host_actions
            .pointer("/execution_modes")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("missing final execution-mode authority: {host_actions}"));
        assert_eq!(
            execution_modes.len(),
            1,
            "single switch should leave one final execution-mode authority: {execution_modes:?}"
        );
        assert_eq!(
            execution_modes[0]
                .pointer("/target")
                .and_then(Value::as_str),
            Some("cpu0")
        );
        assert_eq!(
            execution_modes[0].pointer("/mode").and_then(Value::as_str),
            Some(mode)
        );

        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_authority.targets",
            "Count",
            1,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!("sim.host_actions.execution_mode_authority.target.cpu0.mode.{mode}"),
            "Count",
            1,
            "monotonic",
        );
        for lane in ["functional", "timing", "detailed"] {
            let expected = u64::from(lane == mode);
            assert_json_stat(
                &json,
                &format!("sim.host_actions.execution_mode_authority.mode.{lane}"),
                "Count",
                expected,
                "monotonic",
            );
            assert_json_stat(
                &json,
                &format!("sim.host_actions.execution_mode_authority.target.cpu0.mode.{lane}"),
                "Count",
                expected,
                "monotonic",
            );
            assert_json_stat(
                &json,
                &format!("sim.host_actions.execution_mode_switch.mode.{lane}"),
                "Count",
                expected,
                "monotonic",
            );
            assert_json_stat(
                &json,
                &format!("sim.host_actions.execution_mode_switch.target.cpu0.mode.{lane}"),
                "Count",
                expected,
                "monotonic",
            );
            assert_json_stat(
                &json,
                &format!("sim.host_actions.execution_mode_switch.previous_mode.{lane}"),
                "Count",
                0,
                "monotonic",
            );
            assert_json_stat(
                &json,
                &format!("sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.{lane}"),
                "Count",
                0,
                "monotonic",
            );
        }
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.previous_mode.none",
            "Count",
            1,
            "monotonic",
        );
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.none",
            "Count",
            1,
            "monotonic",
        );

        let transfer = host_actions
            .pointer("/execution_mode_switches/0/state_transfer")
            .unwrap_or_else(|| panic!("missing execution-mode state transfer: {host_actions}"));
        let quiescence_gate = transfer
            .pointer("/quiescence_gate")
            .unwrap_or_else(|| panic!("missing execution-mode quiescence gate: {transfer}"));
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.quiescence.validated",
            "Count",
            1,
            "monotonic",
        );
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.validated",
            "Count",
            1,
            "monotonic",
        );
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.quiescence.captured_components",
            "Count",
            quiescence_gate
                .pointer("/captured_component_count")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing quiescence component count: {quiescence_gate}")),
            "monotonic",
        );
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.quiescence.captured_chunks",
            "Count",
            quiescence_gate
                .pointer("/captured_chunk_count")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing quiescence chunk count: {quiescence_gate}")),
            "monotonic",
        );
        assert_json_stat(
            &json,
            "sim.host_actions.execution_mode_switch.quiescence.captured_payload_bytes",
            "Byte",
            quiescence_gate
                .pointer("/captured_payload_bytes")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing quiescence payload bytes: {quiescence_gate}")),
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_restores_m5_switch_cpu_transfer_and_reports_authority_rollback() {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 5, 0x13),
        i_type(1, 5, 0x0, 5, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-generated-transfer-restore", &elf);

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
            "--host-restore-checkpoint",
            "8:execution-mode-switch-cpu0-3",
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
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let (switch_tick, transfer_label) = assert_execution_mode_switch(
        host_actions,
        0,
        "cpu0",
        None,
        "detailed",
        "execution-mode-switch-cpu0",
    );
    assert_eq!(
        transfer_label,
        format!("execution-mode-switch-cpu0-{switch_tick}")
    );
    assert_eq!(
        transfer_label, "execution-mode-switch-cpu0-3",
        "test assumes the first m5_switch_cpu transfer label remains tick-derived: {host_actions}"
    );

    let restores = host_actions
        .pointer("/checkpoint_restores")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint restores: {host_actions}"));
    let restore = restores
        .first()
        .unwrap_or_else(|| panic!("missing generated transfer restore: {host_actions}"));
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some(transfer_label.as_str())
    );
    assert_eq!(
        restore
            .pointer("/execution_mode_authority_present")
            .and_then(Value::as_bool),
        Some(false),
        "restored pre-switch transfer manifest should report that no authority payload was captured: {restore}"
    );
    assert_eq!(
        restore
            .pointer("/execution_mode_authority_cleared")
            .and_then(Value::as_bool),
        Some(true),
        "restoring the generated pre-switch transfer should explicitly report authority rollback: {restore}"
    );
    assert_eq!(
        restore
            .pointer("/execution_mode_authority_decode_error")
            .and_then(Value::as_bool),
        Some(false),
        "restored absent authority should not be reported as malformed: {restore}"
    );
    assert_eq!(
        restore
            .pointer("/execution_modes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "restoring the pre-switch transfer manifest should roll authority back to no owner: {restore}"
    );
    assert_eq!(
        host_actions
            .pointer("/execution_modes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "final execution-mode authority should remain rolled back after generated transfer restore: {host_actions}"
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.cleared_manifests",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.targets",
        "Count",
        0,
        "monotonic",
    );
    for lane in ["functional", "timing", "detailed"] {
        assert_json_stat(
            &json,
            &format!("sim.host_actions.checkpoint_restore.execution_mode_authority.mode.{lane}"),
            "Count",
            0,
            "monotonic",
        );
    }
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
            "--debug-flags",
            "HostAction",
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
    assert_eq!(
        final_checker.pointer("/execution_mode"),
        Some(&Value::Null),
        "mixed-mode checker runs should not claim a single checker execution mode: {json}"
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
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.checker.checked_instructions",
        "Count",
        second_checked_at_switch,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.checker.checked_instructions",
        "Count",
        second_checked_at_switch,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.validated",
        "Count",
        host_actions
            .pointer("/execution_mode_switch_count")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing execution mode switch count: {host_actions}")),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.captured_components",
        "Count",
        execution_mode_switch_quiescence_target_total(
            host_actions,
            "cpu0",
            "captured_component_count",
        ),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.captured_chunks",
        "Count",
        execution_mode_switch_quiescence_target_total(host_actions, "cpu0", "captured_chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.captured_payload_bytes",
        "Byte",
        execution_mode_switch_quiescence_target_total(
            host_actions,
            "cpu0",
            "captured_payload_bytes",
        ),
        "monotonic",
    );
    for mode in ["functional", "timing", "detailed"] {
        assert_json_stat_absent(
            &json,
            &format!("sim.cpu0.checker.execution_mode.{mode}.checked_instructions"),
        );
        assert_json_stat_absent(
            &json,
            &format!("sim.cpu0.checker.execution_mode.{mode}.mismatches"),
        );
    }
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

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vector_arith_type(funct6: u32, funct3: u32, vs2: u8, vs1_or_rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1_or_rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn fp_r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x53
}

fn fp_r4_type(rs3: u8, funct2: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (u32::from(rs3) << 27)
        | (funct2 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
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

fn detailed_o3_runtime_stats_binary(name: &str) -> std::path::PathBuf {
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
    temp_binary(name, &elf)
}

fn detailed_o3_live_rob_overlap_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        i_type(6, 0, 0x0, 1, 0x13),    // addi x1, x0, 6
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 1, 2, 0x4, 3, 0x33), // div x3, x2, x1
        i_type(5, 0, 0x0, 4, 0x13),    // addi x4, x0, 5
        i_type(11, 4, 0x0, 5, 0x13),   // addi x5, x4, 11
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 3, 12, 0b010),                          // sw x3, 0(x12)
        s_type(4, 5, 12, 0b010),                          // sw x5, 4(x12)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn live_retire_gate_div_witness_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to the CLI-selected mode
        i_type(84, 0, 0x0, 1, 0x13),   // addi x1, x0, 84
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 2, 1, 0x4, 3, 0x33), // div x3, x1, x2
        i_type(-11, 3, 0x0, 4, 0x13),  // addi x4, x3, -11
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 4, 12, 0b010),                          // sw x4, 0(x12)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn live_retire_gate_add_witness_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),         // switch cpu0 to detailed
        i_type(11, 0, 0x0, 1, 0x13), // addi x1, x0, 11
        i_type(14, 1, 0x0, 2, 0x13), // addi x2, x1, 14
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 2, 12, 0b010),                          // sw x2, 0(x12)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_live_rob_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        i_type(6, 0, 0x0, 1, 0x13),    // addi x1, x0, 6
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 2, 1, 0x0, 3, 0x33), // mul x3, x1, x2
        i_type(5, 0, 0x0, 4, 0x13),    // addi x4, x0, 5
        i_type(11, 4, 0x0, 5, 0x13),   // addi x5, x4, 11
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 3, 12, 0b010),                          // sw x3, 0(x12)
        s_type(4, 5, 12, 0b010),                          // sw x5, 4(x12)
        m5op(M5_DUMP_STATS),                              // dump live detailed O3 ROB stats
        r_type(1, 1, 3, 0x4, 6, 0x33),                    // div x6, x3, x1 before the later stop
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_live_lsq_overlap_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),                              // auipc x10, 0
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13), // addi x10, x10, data
        i_type(42, 0, 0x0, 11, 0x13),                     // addi x11, x0, 42
        s_type(0, 11, 10, 0b010),                         // sw x11, 0(x10)
        i_type(0, 10, 0b010, 12, 0x03),                   // lw x12, 0(x10)
        i_type(1, 12, 0x0, 13, 0x13),                     // addi x13, x12, 1
        s_type(4, 13, 10, 0b010),                         // sw x13, 4(x10)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_live_rename_pressure_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),        // switch cpu0 to detailed
        i_type(1, 0, 0x0, 1, 0x13), // addi x1, x0, 1
        i_type(2, 0, 0x0, 2, 0x13), // addi x2, x0, 2
        i_type(3, 1, 0x0, 3, 0x13), // addi x3, x1, 3
        i_type(4, 2, 0x0, 4, 0x13), // addi x4, x2, 4
        i_type(5, 3, 0x0, 5, 0x13), // addi x5, x3, 5
        i_type(6, 4, 0x0, 6, 0x13), // addi x6, x4, 6
        i_type(7, 5, 0x0, 7, 0x13), // addi x7, x5, 7
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),                              // auipc x10, 0
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13), // addi x10, x10, data
        s_type(0, 7, 10, 0b010),                          // sw x7, 0(x10)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_live_rename_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),        // switch cpu0 to detailed
        i_type(1, 0, 0x0, 1, 0x13), // addi x1, x0, 1
        i_type(2, 0, 0x0, 2, 0x13), // addi x2, x0, 2
        i_type(3, 1, 0x0, 3, 0x13), // addi x3, x1, 3
        i_type(4, 2, 0x0, 4, 0x13), // addi x4, x2, 4
        i_type(5, 3, 0x0, 5, 0x13), // addi x5, x3, 5
        i_type(6, 4, 0x0, 6, 0x13), // addi x6, x4, 6
        i_type(7, 5, 0x0, 7, 0x13), // addi x7, x5, 7
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),                              // auipc x12, 0
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13), // addi x12, x12, data
        s_type(0, 7, 12, 0b010),                          // sw x7, 0(x12)
        m5op(M5_DUMP_STATS),                              // dump live rename pressure
        i_type(1, 7, 0x0, 8, 0x13),                       // addi x8, x7, 1
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 6, 0x17),                             // auipc x6, 0
        i_type(data_start - auipc_pc, 6, 0x0, 6, 0x13), // addi x6, x6, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0, 6, 0b010, 12, 0x03),                  // lw x12, 0(x6)
        s_type(4, 12, 6, 0b010),                        // sw x12, 4(x6)
        m5op(M5_CHECKPOINT),                            // checkpoint cpu1 O3 runtime state
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 6, 0x17),                             // auipc x6, 0
        i_type(data_start - auipc_pc, 6, 0x0, 6, 0x13), // addi x6, x6, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0, 6, 0b010, 12, 0x03),                  // lw x12, 0(x6)
        s_type(4, 12, 6, 0b010),                        // sw x12, 4(x6)
        m5op(M5_DUMP_STATS),                            // dump cpu1 O3 runtime aliases
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_float_misc_binary(name: &str) -> std::path::PathBuf {
    let words = vec![
        csr_read(0xf14, 5),                             // csrr x5, mhartid
        b_type(8, 0, 5, 0x1),                           // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0),                           // hart 0: spin until hart 1 exits
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // hart 1: switch cpu1 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_reset_fu_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
    ];
    words.extend(detailed_o3_float_misc_prefix_words());
    words.push(m5op(M5_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_direct_call_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        j_type(8, 1),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        i_type(1, 0, 0x0, 7, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        u_type(0, 5, 0x17),            // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),   // addi x5, x5, data
        i_type(7, 0, 0x0, 11, 0x13),   // addi x11, x0, 7
        i_type(0, 5, 0b010, 12, 0x03), // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),       // sw x12, 4(x5)
        m5op(M5_DUMP_STATS),           // dump live detailed-mode stats
        i_type(99, 0, 0x0, 13, 0x13),  // addi x13, x0, 99 after dump
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        m5op(M5_RESET_STATS),           // reset detailed O3 runtime stats
        i_type(0, 5, 0b010, 12, 0x03),  // lw x12, 0(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_reset_fu_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = detailed_o3_float_misc_prefix_words();
    words.push(m5op(M5_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_dump_reset_fu_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = detailed_o3_float_misc_prefix_words();
    words.push(m5op(M5_DUMP_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_dump_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = detailed_o3_branch_repair_words(data_start);
    words.push(m5op(M5_DUMP_RESET_STATS));
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_predicted_target_match_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(0, 0, 0x0, 9, 0x13),
        b_type(8, 0, 7, 0x1),
        i_type(99, 0, 0x0, 6, 0x13),
        b_type(16, 0, 9, 0x1),
        m5op(M5_SWITCH_CPU),
        i_type(1, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 10, 0x17),
        i_type(data_start - 32, 10, 0x0, 10, 0x13),
        s_type(0, 7, 10, 0b011),
        s_type(8, 9, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
    ];
    append_integer_mul_div_work(&mut words);
    words.extend([i_type(0, 0, 0x0, 10, 0x13), i_type(0, 0, 0x0, 11, 0x13)]);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_indirect_call_wrong_target_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 112_i32;
    let mut words = vec![
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
        u_type(0, 10, 0x17),
        i_type(data_start - 36, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
    ];
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_indirect_jump_wrong_target_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 112_i32;
    let mut words = vec![
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 0, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
        u_type(0, 10, 0x17),
        i_type(data_start - 36, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
    ];
    append_integer_mul_div_work(&mut words);
    words.push(m5op(M5_DUMP_STATS));
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_indirect_call_wrong_target_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 warmup path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU), // hart 1: switch cpu1 to detailed
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
    ];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - data_auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_indirect_call_wrong_target_dump_reset_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 warmup path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU), // hart 1: switch cpu1 to detailed
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
    ];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - data_auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_RESET_STATS),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_restore_indirect_call_ftq_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 warmup path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU), // hart 1: switch cpu1 to detailed
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
    ];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - data_auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_CHECKPOINT),
        m5op(M5_DUMP_STATS),
        j_type(8, 1),
        i_type(9, 0, 0x0, 7, 0x13),
    ]);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_direct_call_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        j_type(8, 1),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        i_type(1, 0, 0x0, 7, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_return_branch_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        u_type(0, 1, 0x17),
        i_type(16, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 0, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_repair_text_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = detailed_o3_branch_repair_words(data_start);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_return_branch_summary_binary(name: &str) -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - 4, 10, 0x0, 10, 0x13),
        u_type(0, 1, 0x17),
        i_type(16, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 0, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_repair_words(data_start: i32) -> Vec<u32> {
    vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(1, 0, 0x0, 9, 0x13),
        b_type(12, 0, 9, 0x1),
        i_type(11, 0, 0x0, 6, 0x13),
        j_type(16, 0),
        m5op(M5_SWITCH_CPU),
        i_type(0, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 5, 0x17),
        i_type(data_start - 32, 5, 0x0, 5, 0x13),
        s_type(0, 6, 5, 0b011),
        s_type(8, 9, 5, 0b011),
    ]
}

fn detailed_o3_lsq_matrix_dump_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0, 5, 0b011, 6, 0x03),                   // ld x6, 0(x5)
        s_type(8, 6, 5, 0b011),                         // sd x6, 8(x5)
        atomic_type(0x02, true, false, 0, 5, 0x3, 7),   // lr.d.aq x7, (x5)
        i_type(3, 0, 0x0, 8, 0x13),                     // addi x8, x0, 3
        atomic_type(0x03, false, true, 8, 5, 0x3, 9),   // sc.d.rl x9, x8, (x5)
        i_type(4, 0, 0x0, 10, 0x13),                    // addi x10, x0, 4
        atomic_type(0x01, true, true, 10, 5, 0x3, 11),  // amoswap.d.aqrl x11, x10, (x5)
        s_type(16, 9, 5, 0b011),                        // sd x9, 16(x5)
        s_type(24, 11, 5, 0b011),                       // sd x11, 24(x5)
        i_type(0, 0, 0x0, 10, 0x13),                    // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),                    // addi x11, x0, 0
        m5op(M5_DUMP_RESET_STATS),
        i_type(32, 5, 0x0, 14, 0x13),   // addi x14, x5, sc-fail data
        i_type(0x2a, 0, 0x0, 13, 0x13), // addi x13, x0, 0x2a
        atomic_type(0x03, false, false, 13, 14, 0x3, 15), // sc.d x15, x13, (x14)
        s_type(40, 15, 5, 0b011),       // sd x15, 40(x5)
        i_type(0, 0, 0x0, 10, 0x13),    // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),    // addi x11, x0, 0
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0, 0, 0, 0, 0, 0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_lsq_forwarding_dump_reset_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        b_type(48, 11, 12, 0x1),                        // bne x12, x11, fail
        m5op(M5_DUMP_RESET_STATS),
        i_type(0x33, 0, 0x0, 13, 0x13), // addi x13, x0, 0x33
        s_type(4, 13, 5, 0b010),        // sw x13, 4(x5)
        i_type(8, 5, 0b010, 14, 0x03),  // lw x14, 8(x5)
        b_type(28, 0, 14, 0x1),         // bne x14, x0, fail
        i_type(0x44, 0, 0x0, 15, 0x13), // addi x15, x0, 0x44
        s_type(12, 15, 5, 0b010),       // sw x15, 12(x5)
        i_type(12, 5, 0b100, 16, 0x03), // lbu x16, 12(x5)
        b_type(12, 15, 16, 0x1),        // bne x16, x15, fail
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_lsq_forwarding_dump_reset_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        b_type(48, 11, 12, 0x1),                        // bne x12, x11, fail
        m5op(M5_DUMP_RESET_STATS),
        i_type(0x33, 0, 0x0, 13, 0x13), // addi x13, x0, 0x33
        s_type(4, 13, 5, 0b010),        // sw x13, 4(x5)
        i_type(8, 5, 0b010, 14, 0x03),  // lw x14, 8(x5)
        b_type(28, 0, 14, 0x1),         // bne x14, x0, fail
        i_type(0x44, 0, 0x0, 15, 0x13), // addi x15, x0, 0x44
        s_type(12, 15, 5, 0b010),       // sw x15, 12(x5)
        i_type(12, 5, 0b100, 16, 0x03), // lbu x16, 12(x5)
        b_type(12, 15, 16, 0x1),        // bne x16, x15, fail
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_lsq_forwarding_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        b_type(20, 11, 12, 0x1),                        // bne x12, x11, fail
        i_type(0, 0, 0x0, 10, 0x13),                    // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),                    // addi x11, x0, 0
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_misc_prefix_words() -> Vec<u32> {
    vec![
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
    ]
}

fn append_integer_mul_div_work(words: &mut Vec<u32>) {
    words.extend([
        i_type(42, 0, 0x0, 1, 0x13), // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),  // addi x2, x0, 7
        0x0220_81b3,                 // mul x3, x1, x2
        0x0220_c1b3,                 // div x3, x1, x2
    ]);
}

fn detailed_o3_iq_iew_commit_matrix_binary(name: &str) -> std::path::PathBuf {
    let mut words = detailed_o3_float_misc_prefix_words();
    append_integer_mul_div_work(&mut words);
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 160_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),                        // sw x12, 4(x5)
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn append_host_stop(words: &mut Vec<u32>) {
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
}

fn detailed_o3_lsq_store_load_match_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),  // lw x12, 0(x5)
        b_type(8, 11, 12, 0x1),         // bne x12, x11, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_lsq_store_load_mismatch_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(4, 5, 0b010, 12, 0x03),  // lw x12, 4(x5)
        b_type(8, 0, 12, 0x1),          // bne x12, x0, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_lsq_store_load_byte_mismatch_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(0, 5, 0b100, 12, 0x03),  // lbu x12, 0(x5)
        b_type(8, 11, 12, 0x1),         // bne x12, x11, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_lsq_store_load_address_and_byte_mismatch_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),            // switch cpu0 to detailed
        u_type(0, 5, 0x17),             // auipc x5, 0
        i_type(60, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13), // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),        // sw x11, 0(x5)
        i_type(4, 5, 0b100, 12, 0x03),  // lbu x12, 4(x5)
        b_type(8, 0, 12, 0x1),          // bne x12, x0, fail
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_ordered_atomic_lsq_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0, 5, 0b011, 6, 0x03),                   // ld x6, 0(x5)
        s_type(8, 6, 5, 0b011),                         // sd x6, 8(x5)
        atomic_type(0x02, true, false, 0, 5, 0x3, 7),   // lr.d.aq x7, (x5)
        i_type(3, 0, 0x0, 8, 0x13),                     // addi x8, x0, 3
        atomic_type(0x03, false, true, 8, 5, 0x3, 9),   // sc.d.rl x9, x8, (x5)
        i_type(4, 0, 0x0, 10, 0x13),                    // addi x10, x0, 4
        atomic_type(0x01, true, true, 10, 5, 0x3, 11),  // amoswap.d.aqrl x11, x10, (x5)
        s_type(16, 9, 5, 0b011),                        // sd x9, 16(x5)
        s_type(24, 11, 5, 0b011),                       // sd x11, 24(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_event_window_ordering_binary(
    name: &str,
    acquire: bool,
    release: bool,
) -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                                // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),    // addi x5, x5, data
        i_type(4, 0, 0x0, 6, 0x13),                        // addi x6, x0, 4
        atomic_type(0x01, acquire, release, 6, 5, 0x3, 7), // amoswap.d x7, x6, (x5)
        i_type(9, 0, 0x0, 8, 0x13),                        // addi x8, x0, 9
        b_type(20, 7, 8, 0x1),                             // bne x7, x8, fail
        i_type(0, 0, 0x0, 10, 0x13),                       // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),                       // addi x11, x0, 0
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_conditional_failure_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x2a, 0, 0x0, 6, 0x13),                  // addi x6, x0, 0x2a
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),  // sc.d x7, x6, (x5)
        s_type(8, 7, 5, 0b011),                         // sd x7, 8(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_vector_lsq_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 10, 0x17),                               // auipc x10, 0
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),  // addi x10, x10, data
        i_type(0, 10, 0x3, 1, 0x07),                       // fld f1, 0(x10)
        float_store_type(8, 1, 10, 0x3),                   // fsd f1, 8(x10)
        i_type(16, 10, 0x0, 12, 0x13),                     // addi x12, x10, vector src
        i_type(24, 10, 0x0, 16, 0x13),                     // addi x16, x10, vector dst
        i_type(2, 0, 0x0, 11, 0x13),                       // addi x11, x0, 2
        vsetvli_type(0xd0, 11, 5),                         // e32, m1, vl=2
        vector_unit_stride_load_type(true, 0b110, 12, 1),  // vle v1, (x12)
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse v1, (x16)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&1.0f64.to_bits().to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    program.extend(
        [0x1122_3344, 0x5566_7788, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn functional_store_conditional_failure_binary(name: &str) -> std::path::PathBuf {
    let mut words = Vec::new();
    let auipc_pc = 0_i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x2a, 0, 0x0, 6, 0x13),                  // addi x6, x0, 0x2a
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),  // sc.d x7, x6, (x5)
        s_type(8, 7, 5, 0b011),                         // sd x7, 8(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_store_type(vm_unmasked: bool, width: u32, rs1: u8, vs3: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn float_store_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = (imm as u32) & 0xfff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x27
}

fn detailed_o3_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),         // switch cpu0 to detailed
        i_type(42, 0, 0x0, 1, 0x13), // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),  // addi x2, x0, 7
        0x0220_81b3,                 // mul x3, x1, x2
        0x0220_c1b3,                 // div x3, x1, x2
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13), // addi x13, x0, 77
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_misc_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_extended_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),                // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),               // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),               // fmv.w.x f2, x8
        fp_r_type(0x78, 0, 8, 0x0, 3),               // fmv.w.x f3, x8
        i_type(2, 0, 0x0, 10, 0x13),                 // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                   // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 8, 1), // vfmv.v.f v1, f8
        vector_arith_type(0b010111, 0b100, 0, 8, 2), // vfmv.v.f v2, f8
        vector_arith_type(0b010111, 0b100, 0, 8, 4), // vfmv.v.f v4, f8
        m5op(M5_SWITCH_CPU),                         // switch cpu0 to detailed
        fp_r_type(0x00, 2, 1, 0x0, 4),               // fadd.s f4, f1, f2
        fp_r4_type(3, 0x0, 2, 1, 0x0, 5, 0x43),      // fmadd.s f5, f1, f2, f3
        fp_r_type(0x2c, 0, 1, 0x0, 6),               // fsqrt.s f6, f1
        vector_arith_type(0b000000, 0b001, 2, 1, 3), // vfadd.vv v3, v2, v1
        vector_arith_type(0b101100, 0b001, 2, 1, 4), // vfmacc.vv v4, v2, v1
        vector_arith_type(0b010011, 0b001, 1, 0, 5), // vfsqrt.v v5, v1
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_integer_fu_latency_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        i_type(2, 0, 0x0, 10, 0x13),                 // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                   // e32, m1, vl=2
        m5op(M5_SWITCH_CPU),                         // switch cpu0 to detailed
        vector_arith_type(0b100101, 0b010, 2, 1, 3), // vmul.vv v3, v2, v1
        vector_arith_type(0b100000, 0b010, 2, 1, 4), // vdivu.vv v4, v2, v1
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_misc_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn pre_dump_then_detailed_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_DUMP_STATS),
        m5op(M5_SWITCH_CPU),
        i_type(42, 0, 0x0, 1, 0x13), // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),  // addi x2, x0, 7
        0x0220_81b3,                 // mul x3, x1, x2
        0x0220_c1b3,                 // div x3, x1, x2
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_checkpoint_state_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),           // switch cpu0 to detailed
        m5op(M5_CHECKPOINT),           // baseline O3 runtime state
        u_type(0, 5, 0x17),            // auipc x5, 0
        i_type(48, 5, 0x0, 5, 0x13),   // addi x5, x5, data
        i_type(7, 0, 0x0, 11, 0x13),   // addi x11, x0, 7
        i_type(0, 5, 0b010, 12, 0x03), // lw x12, 0(x5)
        s_type(4, 12, 5, 0b010),       // sw x12, 4(x5)
        m5op(M5_CHECKPOINT),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 56 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_scheduled_restore_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13)); // nop
    }
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 704_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
    ]);
    while words.len() < 170 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_restore_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.push(m5op(M5_CHECKPOINT)); // exact baseline for the scheduled restore
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.push(m5op(M5_DUMP_STATS)); // dump restored-baseline stats before O3 work
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 704_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
    ]);
    while words.len() < 170 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_restore_fu_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x3f80_0000, 8, 0x37),                   // lui x8, 1.0f bits
        fp_r_type(0x78, 0, 8, 0x0, 1),                  // fmv.w.x f1, x8
        fp_r_type(0x78, 0, 8, 0x0, 2),                  // fmv.w.x f2, x8
        i_type(3, 0, 0x0, 9, 0x13),                     // addi x9, x0, 3
        i_type(2, 0, 0x0, 10, 0x13),                    // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),                      // e32, m1, vl=2
        vector_arith_type(0b010111, 0b100, 0, 1, 1),    // vfmv.v.f v1, f1
        vector_arith_type(0b010111, 0b100, 0, 2, 2),    // vfmv.v.f v2, f2
        m5op(M5_SWITCH_CPU),                            // switch cpu0 to detailed
        i_type(42, 0, 0x0, 1, 0x13),                    // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),                     // addi x2, x0, 7
        0x0220_81b3,                                    // mul x3, x1, x2
        0x0220_c1b3,                                    // div x3, x1, x2
        m5op(M5_CHECKPOINT),                            // checkpoint integer FU stats
        m5op(M5_DUMP_STATS),                            // dump checkpoint-era stats
        fp_r_type(0x68, 0, 9, 0x0, 3),                  // fcvt.s.w f3, x9
        fp_r_type(0x10, 2, 1, 0x0, 4),                  // fsgnj.s f4, f1, f2
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3), // vfsgnj.vv v3, v2, v1
        vector_arith_type(0b001000, 0b001, 2, 1, 4),    // vfsgnj.vv v4, v1, v2
    ];
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_restore_fu_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(3, 0, 0x0, 9, 0x13),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 1, 1),
        vector_arith_type(0b010111, 0b100, 0, 2, 2),
        m5op(M5_SWITCH_CPU),
    ];
    append_integer_mul_div_work(&mut words);
    words.extend([
        m5op(M5_CHECKPOINT),
        m5op(M5_DUMP_STATS),
        fp_r_type(0x68, 0, 9, 0x0, 3),
        fp_r_type(0x10, 2, 1, 0x0, 4),
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3),
        vector_arith_type(0b001000, 0b001, 2, 1, 4),
    ]);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn sparse_three_core_detailed_o3_restore_trace_binary(name: &str) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),         // csrr x5, mhartid
        i_type(1, 0, 0x0, 6, 0x13), // addi x6, x0, 1
        b_type(8, 6, 5, 0x1),       // harts 0/2: branch to detailed path
        b_type(0, 0, 0, 0x0),       // hart 1: spin without detailed O3 authority
        m5op(M5_SWITCH_CPU),        // harts 0/2: switch to detailed
    ];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 7, 0x17),                             // auipc x7, 0
        i_type(data_start - auipc_pc, 7, 0x0, 7, 0x13), // addi x7, x7, data
        i_type(2, 5, 0b001, 8, 0x13),                   // slli x8, x5, 2
        r_type(0, 8, 7, 0x0, 7, 0x33),                  // add x7, x7, x8
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        r_type(0, 5, 11, 0x0, 11, 0x33),                // add x11, x11, x5
        s_type(0, 11, 7, 0b010),                        // sw x11, 0(x7)
        i_type(0, 7, 0b010, 12, 0x03),                  // lw x12, 0(x7)
    ]);
    let check_branch_index = words.len();
    words.push(0);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    let fail_index = words.len() - 1;
    words[check_branch_index] = b_type(((fail_index - check_branch_index) * 4) as i32, 11, 12, 0x1);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multicore_hart1_detailed_o3_restore_lsq_forwarding_dump_stats_binary(
    name: &str,
) -> std::path::PathBuf {
    let data_start = 1024_i32;
    let mut words = vec![
        csr_read(0xf14, 5),   // csrr x5, mhartid
        b_type(8, 0, 5, 0x1), // bne x5, x0, hart 1 detailed path
        b_type(0, 0, 0, 0x0), // hart 0: spin until hart 1 exits
        m5op(M5_SWITCH_CPU),  // hart 1: switch cpu1 to detailed
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x5a, 0, 0x0, 11, 0x13),                 // addi x11, x0, 0x5a
        s_type(0, 11, 5, 0b010),                        // sw x11, 0(x5)
        i_type(0, 5, 0b010, 12, 0x03),                  // lw x12, 0(x5)
    ]);
    let first_check_branch_index = words.len();
    words.push(0);
    words.extend([
        i_type(0, 0, 0x0, 10, 0x13),   // addi x10, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),   // addi x11, x0, 0
        m5op(M5_CHECKPOINT),           // checkpoint one LSQ forwarding match
        m5op(M5_DUMP_STATS),           // dump restored-baseline LSQ counters
        i_type(4, 5, 0b010, 15, 0x03), // lw x15, 4(x5)
    ]);
    let restored_word_branch_index = words.len();
    words.push(0);
    words.extend([
        i_type(0x6b, 0, 0x0, 13, 0x13), // addi x13, x0, 0x6b
        s_type(4, 13, 5, 0b010),        // sw x13, 4(x5)
        i_type(4, 5, 0b010, 14, 0x03),  // lw x14, 4(x5)
    ]);
    let second_check_branch_index = words.len();
    words.push(0);
    while words.len() < 220 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    let fail_index = words.len() - 1;
    words[first_check_branch_index] = b_type(
        ((fail_index - first_check_branch_index) * 4) as i32,
        11,
        12,
        0x1,
    );
    words[restored_word_branch_index] = b_type(
        ((fail_index - restored_word_branch_index) * 4) as i32,
        0,
        15,
        0x1,
    );
    words[second_check_branch_index] = b_type(
        ((fail_index - second_check_branch_index) * 4) as i32,
        13,
        14,
        0x1,
    );
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn scheduled_host_restore_missing_label_binary(name: &str) -> std::path::PathBuf {
    let mut words = Vec::new();
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn timing_switch_o3_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn timing_switch_o3_dump_stats_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
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

fn assert_text_byte_stat(stdout: &str, path: &str, value: u64) {
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
        line.contains("unit=Byte"),
        "missing Byte unit for {path} in line: {line}"
    );
    assert!(
        line.contains("reset_policy=monotonic"),
        "missing monotonic reset policy for {path} in line: {line}"
    );
}

fn assert_text_cycle_stat(stdout: &str, path: &str, value: u64) {
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
        line.contains("unit=Cycle"),
        "missing Cycle unit for {path} in line: {line}"
    );
    assert!(
        line.contains("reset_policy=monotonic"),
        "missing monotonic reset policy for {path} in line: {line}"
    );
}

fn assert_text_ratio_stat(stdout: &str, path: &str, value: &str, unit: &str) {
    let line = text_stat_line(stdout, path);
    let actual = line.split_whitespace().nth(1);
    assert_eq!(
        actual,
        Some(value),
        "unexpected text stat value for {path} in line: {line}"
    );
    assert!(
        line.contains("kind=derived"),
        "missing derived kind for {path} in line: {line}"
    );
    assert!(
        line.contains(&format!("unit={unit}")),
        "missing {unit} unit for {path} in line: {line}"
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

fn text_stat_u64(stdout: &str, path: &str) -> u64 {
    text_stat_line(stdout, path)
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("text stat {path} should have an integer value"))
}

fn fixed_ratio_text(numerator: u64, denominator: u64) -> String {
    assert_ne!(denominator, 0, "ratio denominator must be nonzero");
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn assert_text_stat_absent(stdout: &str, path: &str) {
    assert!(
        stdout
            .lines()
            .all(|line| line.split_whitespace().next() != Some(path)),
        "unexpected text stat {path} in stdout:\n{stdout}"
    );
}

fn assert_text_stat_occurs_once(stdout: &str, path: &str) {
    let count = stdout
        .lines()
        .filter(|line| line.split_whitespace().next() == Some(path))
        .count();
    assert_eq!(
        count, 1,
        "expected exactly one text stat {path}, found {count} in stdout:\n{stdout}"
    );
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

fn execution_mode_switch_quiescence_target_total(
    host_actions: &Value,
    target: &str,
    field: &str,
) -> u64 {
    host_actions
        .pointer("/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution mode switch actions: {host_actions}"))
        .iter()
        .filter(|action| {
            action
                .pointer("/state_transfer/quiescence_gate/target")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing switch quiescence target: {action}"))
                == target
        })
        .map(|action| {
            action
                .pointer(&format!("/state_transfer/quiescence_gate/{field}"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing switch quiescence {field}: {action}"))
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

fn execution_mode_switch_transfer_component_chunk_checksum(
    host_actions: &Value,
    switch_index: usize,
    component: &str,
    chunk: &str,
) -> String {
    let components = host_actions
        .pointer(&format!(
            "/execution_mode_switches/{switch_index}/state_transfer/components"
        ))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing switch transfer components {switch_index}"));
    let component = components
        .iter()
        .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing switch transfer component {component}"));
    let chunks = component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing switch transfer chunks for {component}"));
    chunks
        .iter()
        .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
        .and_then(|entry| entry.pointer("/payload_checksum"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing switch transfer chunk checksum {component}/{chunk}"))
        .to_string()
}

fn parse_hex_u64(value: &str) -> u64 {
    let digits = value
        .strip_prefix("0x")
        .unwrap_or_else(|| panic!("checksum should use 0x prefix: {value}"));
    u64::from_str_radix(digits, 16)
        .unwrap_or_else(|error| panic!("invalid checksum {value}: {error}"))
}

fn json_stat_u64(json: &Value, path: &str) -> u64 {
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array in run JSON: {json}"));
    stats
        .iter()
        .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        .and_then(|sample| sample.pointer("/value"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 stat path {path} in {stats:?}"))
}

fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    ppm.min(u128::from(u64::MAX)) as u64
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

fn assert_json_stat_at_least(
    json: &Value,
    path: &str,
    unit: &str,
    minimum: u64,
    reset_policy: &str,
) {
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array in run JSON: {json}"));
    let sample = stats
        .iter()
        .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        .unwrap_or_else(|| panic!("missing stat path {path} in {stats:?}"));
    assert_eq!(
        sample.pointer("/unit").and_then(Value::as_str),
        Some(unit),
        "unexpected unit for {path}: {sample}"
    );
    assert!(
        sample
            .pointer("/value")
            .and_then(Value::as_u64)
            .is_some_and(|value| value >= minimum),
        "expected {path} to be at least {minimum}: {sample}"
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

fn assert_o3_lsq_count_alias(json: &Value, field: &str, value: u64) {
    let Some((family, alias)) = field
        .strip_prefix("lsq_operation_")
        .map(|operation| ("operation", o3_lsq_operation_count_alias(operation)))
        .or_else(|| {
            field
                .strip_prefix("lsq_ordering_")
                .map(|ordering| ("ordering", o3_lsq_ordering_count_alias(ordering)))
        })
    else {
        return;
    };

    assert_json_stat(
        json,
        &format!("system.cpu.lsq0.{family}.{alias}"),
        "Count",
        value,
        "monotonic",
    );
    let bucket_alias = match family {
        "operation" => o3_lsq_operation_bucket_alias(alias),
        "ordering" => o3_lsq_ordering_bucket_alias(alias),
        _ => unreachable!("unexpected O3 LSQ alias family {family}"),
    };
    assert_json_stat(
        json,
        &format!("system.cpu.lsq0.{family}_0::{bucket_alias}"),
        "Count",
        value,
        "monotonic",
    );
}

fn assert_o3_lsq_count_alias_totals(json: &Value, operation_total: u64, ordering_total: u64) {
    for (family, value) in [("operation", operation_total), ("ordering", ordering_total)] {
        assert_json_stat(
            json,
            &format!("system.cpu.lsq0.{family}.total"),
            "Count",
            value,
            "monotonic",
        );
        assert_json_stat(
            json,
            &format!("system.cpu.lsq0.{family}_0::total"),
            "Count",
            value,
            "monotonic",
        );
    }
}

fn o3_lsq_operation_count_alias(operation: &str) -> &'static str {
    match operation {
        "load" => "load",
        "store" => "store",
        "load_reserved" => "loadReserved",
        "store_conditional" => "storeConditional",
        "atomic" => "atomic",
        "float_load" => "floatLoad",
        "float_store" => "floatStore",
        "vector_load" => "vectorLoad",
        "vector_store" => "vectorStore",
        _ => panic!("unexpected O3 LSQ operation field {operation}"),
    }
}

fn o3_lsq_ordering_count_alias(ordering: &str) -> &'static str {
    match ordering {
        "acquire" => "acquire",
        "release" => "release",
        "acquire_release" => "acquireRelease",
        _ => panic!("unexpected O3 LSQ ordering field {ordering}"),
    }
}

fn o3_lsq_operation_bucket_alias(alias: &str) -> &'static str {
    match alias {
        "load" => "Load",
        "store" => "Store",
        "loadReserved" => "LoadReserved",
        "storeConditional" => "StoreConditional",
        "atomic" => "Atomic",
        "floatLoad" => "FloatLoad",
        "floatStore" => "FloatStore",
        "vectorLoad" => "VectorLoad",
        "vectorStore" => "VectorStore",
        _ => panic!("unexpected O3 LSQ operation alias {alias}"),
    }
}

fn o3_lsq_ordering_bucket_alias(alias: &str) -> &'static str {
    match alias {
        "acquire" => "Acquire",
        "release" => "Release",
        "acquireRelease" => "AcquireRelease",
        _ => panic!("unexpected O3 LSQ ordering alias {alias}"),
    }
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

fn assert_stats_dump_sample(
    dump: &Value,
    path: &str,
    kind: &str,
    unit: &str,
    value: u64,
    reset_policy: &str,
) {
    let sample = dump
        .pointer("/samples")
        .and_then(Value::as_array)
        .and_then(|samples| {
            samples
                .iter()
                .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        })
        .unwrap_or_else(|| panic!("missing stats dump sample {path}: {dump}"));
    assert_eq!(
        sample.pointer("/kind").and_then(Value::as_str),
        Some(kind),
        "stats dump sample {path}: {sample}"
    );
    assert_eq!(
        sample.pointer("/unit").and_then(Value::as_str),
        Some(unit),
        "stats dump sample {path}: {sample}"
    );
    assert_eq!(
        sample.pointer("/value").and_then(Value::as_u64),
        Some(value),
        "stats dump sample {path}: {sample}"
    );
    assert_eq!(
        sample.pointer("/reset_policy").and_then(Value::as_str),
        Some(reset_policy),
        "stats dump sample {path}: {sample}"
    );
}

fn assert_stats_dump_sample_at_least(
    dump: &Value,
    path: &str,
    kind: &str,
    unit: &str,
    minimum: u64,
    reset_policy: &str,
) {
    let sample = dump
        .pointer("/samples")
        .and_then(Value::as_array)
        .and_then(|samples| {
            samples
                .iter()
                .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        })
        .unwrap_or_else(|| panic!("missing stats dump sample {path}: {dump}"));
    assert_eq!(
        sample.pointer("/kind").and_then(Value::as_str),
        Some(kind),
        "stats dump sample {path}: {sample}"
    );
    assert_eq!(
        sample.pointer("/unit").and_then(Value::as_str),
        Some(unit),
        "stats dump sample {path}: {sample}"
    );
    assert!(
        sample
            .pointer("/value")
            .and_then(Value::as_u64)
            .is_some_and(|value| value >= minimum),
        "stats dump sample {path} should be at least {minimum}: {sample}"
    );
    assert_eq!(
        sample.pointer("/reset_policy").and_then(Value::as_str),
        Some(reset_policy),
        "stats dump sample {path}: {sample}"
    );
}

fn stats_dump_sample_value(dump: &Value, path: &str) -> u64 {
    dump.pointer("/samples")
        .and_then(Value::as_array)
        .and_then(|samples| {
            samples
                .iter()
                .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        })
        .and_then(|sample| sample.pointer("/value").and_then(Value::as_u64))
        .unwrap_or_else(|| panic!("missing stats dump sample value {path}: {dump}"))
}

fn assert_stats_dump_sample_absent(dump: &Value, path: &str) {
    let samples = dump
        .pointer("/samples")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats dump samples: {dump}"));
    assert!(
        samples
            .iter()
            .all(|sample| sample.pointer("/path").and_then(Value::as_str) != Some(path)),
        "unexpected stats dump sample {path}: {dump}"
    );
}

fn json_stat_value(json: &Value, path: &str) -> u64 {
    json.pointer("/stats")
        .and_then(Value::as_array)
        .and_then(|stats| {
            stats
                .iter()
                .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        })
        .and_then(|sample| sample.pointer("/value").and_then(Value::as_u64))
        .unwrap_or_else(|| panic!("missing JSON stat value {path}: {json}"))
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
    checkpoint_chunk_summary(host_actions, checkpoint_index, component, chunk)
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing checkpoint chunk checksum {component}/{chunk}"))
        .to_string()
}

fn checkpoint_chunk_summary<'a>(
    host_actions: &'a Value,
    checkpoint_index: usize,
    component: &str,
    chunk: &str,
) -> &'a Value {
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
        .unwrap_or_else(|| panic!("missing checkpoint chunk summary {component}/{chunk}"))
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
