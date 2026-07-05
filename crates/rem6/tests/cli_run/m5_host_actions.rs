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
fn rem6_run_records_o3_runtime_stats_after_detailed_switch() {
    let path = detailed_o3_runtime_stats_binary("m5-switch-cpu-detailed-o3-runtime-stats");

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
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.insts_issued",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.mem_insts_issued",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.dispatched_insts",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.insts_to_commit",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.dispatchedInsts",
        "Count",
        6,
        "monotonic",
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime.pointer("/instructions").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        o3_runtime
            .pointer("/rob_allocations")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        o3_runtime.pointer("/rob_commits").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        o3_runtime.pointer("/rename_writes").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_load_bytes")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_store_bytes")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        o3_runtime
            .pointer("/max_rob_occupancy")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/max_lsq_occupancy")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/rename_map_entries")
            .and_then(Value::as_u64),
        Some(3)
    );
}

#[test]
fn rem6_run_records_per_core_detailed_o3_mode_switch_authority() {
    let path = multicore_hart1_detailed_o3_binary("m5-switch-cpu-hart1-detailed-o3-authority");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
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
        json.pointer("/cores")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
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
    assert_execution_mode_switch(
        host_actions,
        0,
        "cpu1",
        None,
        "detailed",
        "execution-mode-switch-cpu1",
    );
    let execution_modes = host_actions
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing final execution-mode authority: {host_actions}"));
    assert_eq!(
        execution_modes.len(),
        1,
        "only hart 1 should own detailed execution-mode authority: {execution_modes:?}"
    );
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu1"),
        "final execution-mode authority should target hart 1: {execution_modes:?}"
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some("detailed"),
        "final execution-mode authority should keep hart 1 detailed: {execution_modes:?}"
    );

    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let cpu0_o3 = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let cpu1_o3 = checkpoint_chunk_checksum(host_actions, 0, "cpu1", "o3-runtime-state");
    assert_ne!(
        cpu1_o3, cpu0_o3,
        "hart 1 detailed O3 checkpoint payload should diverge from functional hart 0"
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.instructions", "Count", 8, "monotonic");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.lsq_load_bytes", "Byte", 4, "monotonic");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.lsq_store_bytes", "Byte", 4, "monotonic");
    assert_json_stat_at_least(
        &json,
        "sim.cpu1.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu1.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    let core0 = json
        .pointer("/cores/0")
        .unwrap_or_else(|| panic!("missing core 0 summary: {json}"));
    let core1_o3 = json
        .pointer("/cores/1/o3_runtime")
        .unwrap_or_else(|| panic!("missing hart 1 O3 runtime summary: {json}"));
    assert!(
        core0.pointer("/o3_runtime").is_none(),
        "functional hart 0 should not emit O3 runtime state: {core0}"
    );
    assert!(
        core1_o3
            .pointer("/instructions")
            .and_then(Value::as_u64)
            .is_some_and(|instructions| instructions >= 8),
        "hart 1 O3 runtime should record detailed instructions: {core1_o3}"
    );
}

#[test]
fn rem6_run_m5_dump_stats_snapshots_detailed_o3_runtime_stats() {
    let path = detailed_o3_dump_stats_binary("m5-switch-cpu-detailed-o3-dump-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(dump.pointer("/epoch").and_then(Value::as_u64), Some(0));
    assert_eq!(dump.pointer("/reset_tick").and_then(Value::as_u64), Some(0));
    let samples = dump
        .pointer("/samples")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("stats dump should include snapshot samples: {dump}"));
    assert_eq!(
        dump.pointer("/sample_count").and_then(Value::as_u64),
        Some(samples.len() as u64)
    );
    assert_stats_dump_sample_absent(dump, "sim.cpu0.o3.instructions");
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.instructions",
        "counter",
        "Count",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rob_allocations",
        "counter",
        "Count",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rob_commits",
        "counter",
        "Count",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rename_writes",
        "counter",
        "Count",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.lsq_load_bytes",
        "counter",
        "Byte",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_bytes",
        "counter",
        "Byte",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rename_map_entries",
        "counter",
        "Count",
        3,
        "resettable",
    );
    assert_json_stat(&json, "sim.cpu0.o3.instructions", "Count", 8, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.rename_writes", "Count", 5, "monotonic");
}

#[test]
fn rem6_run_m5_dump_stats_snapshots_o3_float_misc_fu_latency_classes() {
    let path = detailed_o3_float_misc_dump_stats_binary("m5-switch-cpu-o3-float-misc-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
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
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "counter",
        "Count",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        "counter",
        "Cycle",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
        "counter",
        "Cycle",
        3,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
        "counter",
        "Cycle",
        3,
        "resettable",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_stats_omits_o3_runtime_snapshot_after_timing_switch() {
    let path = timing_switch_o3_dump_stats_binary("m5-switch-cpu-timing-no-o3-dump-stats");

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
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
    ] {
        assert_stats_dump_sample_absent(dump, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_before_detailed_switch_keeps_later_o3_snapshot() {
    let path = pre_dump_then_detailed_o3_dump_stats_binary("m5-pre-dump-before-o3-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
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
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump action: {host_actions}"));
    let second_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing second stats dump action: {host_actions}"));
    assert_stats_dump_sample_absent(
        first_dump,
        "sim.host_actions.stats_dump.cpu0.o3.instructions",
    );
    assert_stats_dump_sample(
        second_dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        second_dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
        "counter",
        "Cycle",
        19,
        "resettable",
    );
}

#[test]
fn rem6_run_m5_reset_stats_clears_detailed_o3_runtime_stats() {
    let path = detailed_o3_reset_stats_binary("m5-switch-cpu-detailed-o3-reset-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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
        json.pointer("/host_actions/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_json_stat(&json, "sim.cpu0.o3.instructions", "Count", 2, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rob_allocations",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(&json, "sim.cpu0.o3.rob_commits", "Count", 2, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 0, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_load_bytes", "Byte", 4, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_store_bytes", "Byte", 0, "monotonic");
}

#[test]
fn rem6_run_m5_reset_stats_scopes_o3_fu_class_dump_stats() {
    let path = detailed_o3_reset_fu_dump_stats_binary("m5-switch-cpu-o3-reset-fu-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "280",
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
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {dump}"
    );
    assert!(
        dump.pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {dump}"
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        "counter",
        "Cycle",
        21,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
        "counter",
        "Cycle",
        19,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        21,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_snapshots_then_resets_o3_fu_classes() {
    let path = detailed_o3_dump_reset_fu_stats_binary("m5-switch-cpu-o3-dump-reset-fu-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
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
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the old epoch before resetting: {dump}"
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "counter",
        "Count",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        "counter",
        "Cycle",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
        "counter",
        "Cycle",
        0,
        "resettable",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        21,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "Cycle",
        19,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_records_o3_lsq_store_load_matches_after_detailed_switch() {
    let path =
        detailed_o3_lsq_store_load_match_binary("m5-switch-cpu-detailed-o3-lsq-store-load-match");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 1, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_records_o3_fu_latency_stats_after_detailed_switch() {
    let path = detailed_o3_fu_latency_binary("m5-switch-cpu-detailed-o3-fu-latency-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        21,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "Cycle",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.int_div",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "Cycle",
        19,
        "monotonic",
    );
}

#[test]
fn rem6_run_records_o3_float_misc_fu_latency_stats_after_detailed_switch() {
    let path =
        detailed_o3_float_misc_fu_latency_binary("m5-switch-cpu-detailed-o3-float-misc-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
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
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_latency_cycles",
        "Cycle",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_latency_cycles",
        "Cycle",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_extended_float_fu_latency_classes() {
    let path = detailed_o3_float_extended_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-float-extended-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/fu_latency_instructions")
            .and_then(Value::as_u64),
        Some(6)
    );

    for class in [
        "float_add",
        "float_fma",
        "float_sqrt",
        "vector_float_add",
        "vector_float_fma",
        "vector_float_sqrt",
    ] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        let stat_instruction_path = format!("sim.cpu0.o3.fu_{class}_instructions");
        let stat_latency_path = format!("sim.cpu0.o3.fu_{class}_latency_cycles");
        let runtime_instructions = o3_runtime
            .pointer(&instruction_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {instruction_path}: {o3_runtime}")
            });
        let runtime_latency_cycles = o3_runtime
            .pointer(&latency_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {latency_path}: {o3_runtime}")
            });
        assert_eq!(
            runtime_instructions, 1,
            "structured O3 runtime JSON should count {instruction_path}: {o3_runtime}"
        );
        assert!(
            runtime_latency_cycles > 0,
            "structured O3 runtime JSON should count positive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_instruction_path),
            runtime_instructions,
            "stat registry should match structured runtime {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_latency_path),
            runtime_latency_cycles,
            "stat registry should match structured runtime {latency_path}"
        );
    }
    for class in [
        "float_mul",
        "float_div",
        "vector_float_mul",
        "vector_float_div",
    ] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        assert_eq!(
            o3_runtime
                .pointer(&instruction_path)
                .and_then(Value::as_u64),
            Some(0),
            "structured O3 runtime JSON should expose inactive {instruction_path}: {o3_runtime}"
        );
        assert_eq!(
            o3_runtime.pointer(&latency_path).and_then(Value::as_u64),
            Some(0),
            "structured O3 runtime JSON should expose inactive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &format!("sim.cpu0.o3.fu_{class}_instructions")),
            0,
            "stat registry should expose inactive {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &format!("sim.cpu0.o3.fu_{class}_latency_cycles")),
            0,
            "stat registry should expose inactive {latency_path}"
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_ordered_atomic_lsq_matrix() {
    let path = detailed_o3_ordered_atomic_lsq_binary(
        "m5-switch-cpu-detailed-o3-ordered-atomic-lsq-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("04000000000000000900000000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000300000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_load", 1),
        ("lsq_operation_store", 3),
        ("lsq_operation_load_reserved", 1),
        ("lsq_operation_store_conditional", 1),
        ("lsq_operation_atomic", 1),
        ("lsq_operation_float_load", 0),
        ("lsq_operation_float_store", 0),
        ("lsq_operation_vector_load", 0),
        ("lsq_operation_vector_store", 0),
        ("lsq_ordering_acquire", 1),
        ("lsq_ordering_release", 1),
        ("lsq_ordering_acquire_release", 1),
        ("lsq_store_conditional_failures", 0),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_counts_store_conditional_failures() {
    let path = detailed_o3_store_conditional_failure_binary(
        "m5-switch-cpu-detailed-o3-store-conditional-failure-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000040:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_store", 1),
        ("lsq_operation_store_conditional", 1),
        ("lsq_ordering_acquire", 0),
        ("lsq_ordering_release", 0),
        ("lsq_ordering_acquire_release", 0),
        ("lsq_store_conditional_failures", 1),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
    }
}

#[test]
fn rem6_run_does_not_record_o3_store_conditional_failure_after_functional_run() {
    let path = functional_store_conditional_failure_binary(
        "functional-store-conditional-failure-omits-o3-runtime-json",
    );

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
            "--dump-memory",
            "0x80000040:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
    );
    assert!(
        json.pointer("/cores/0/o3_runtime").is_none(),
        "functional failed SC run should not emit O3 runtime state: {json}"
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_conditional_failures");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_operation.store_conditional");
}

#[test]
fn rem6_run_o3_runtime_json_exposes_float_vector_lsq_matrix() {
    let path = detailed_o3_float_vector_lsq_binary(
        "m5-switch-cpu-detailed-o3-float-vector-lsq-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("000000000000f03f000000000000f03f")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("44332211887766554433221188776655")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_float_load", 1),
        ("lsq_operation_float_store", 1),
        ("lsq_operation_vector_load", 1),
        ("lsq_operation_vector_store", 1),
        ("lsq_operation_atomic", 0),
        ("lsq_operation_store_conditional", 0),
        ("lsq_ordering_acquire", 0),
        ("lsq_ordering_release", 0),
        ("lsq_ordering_acquire_release", 0),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
    }
}

#[test]
fn rem6_run_checkpoints_o3_runtime_state_after_detailed_execution() {
    let path = detailed_o3_checkpoint_state_binary("m5-switch-cpu-detailed-o3-checkpoint-state");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let baseline = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let after_detailed = checkpoint_chunk_checksum(host_actions, 1, "cpu0", "o3-runtime-state");
    assert_ne!(
        after_detailed, baseline,
        "detailed O3 runtime checkpoint chunk should change after retired rename and LSQ work"
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rename_map_entries",
        "Count",
        3,
        "monotonic",
    );
}

#[test]
fn rem6_run_restores_scheduled_o3_checkpoint_and_replays_detailed_work() {
    let path = detailed_o3_scheduled_restore_binary("m5-switch-cpu-detailed-o3-scheduled-restore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-checkpoint",
            "8:o3-baseline",
            "--host-checkpoint",
            "50:o3-mutated",
            "--host-restore-checkpoint",
            "70:o3-baseline",
            "--host-checkpoint",
            "113:o3-replayed",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let restored_checkpoint = host_actions
        .pointer("/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing restored checkpoint detail: {host_actions}"));
    assert_eq!(
        restored_checkpoint
            .pointer("/label")
            .and_then(Value::as_str),
        Some("o3-baseline"),
        "restored checkpoint detail should identify the restored manifest: {restored_checkpoint}"
    );
    let restored_execution_mode_component = restored_checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str)
                    == Some("host.execution_modes")
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "restored detailed checkpoint should expose host.execution_modes: {restored_checkpoint}"
            )
        });
    assert_eq!(
        restored_execution_mode_component
            .pointer("/chunk_count")
            .and_then(Value::as_u64),
        Some(1),
        "restored execution-mode component should contain the modes chunk: {restored_execution_mode_component}"
    );
    assert!(
        restored_execution_mode_component
            .pointer("/chunks")
            .and_then(Value::as_array)
            .is_some_and(|chunks| chunks
                .iter()
                .any(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("modes"))),
        "restored execution-mode component should expose the modes chunk: {restored_execution_mode_component}"
    );
    assert_checkpoint(host_actions, 0, "o3-baseline", 9, 9);
    assert_checkpoint(host_actions, 1, "o3-mutated", 51, 51);
    assert_checkpoint(host_actions, 2, "o3-replayed", 114, 114);
    let restored_components = host_actions
        .pointer("/checkpoint_restored_component_count")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint components");
    let restored_chunks = host_actions
        .pointer("/checkpoint_restored_chunk_count")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint chunks");
    let restored_payload_bytes = host_actions
        .pointer("/checkpoint_restored_payload_bytes")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint payload bytes");
    assert_eq!(
        restored_components,
        host_actions
            .pointer("/checkpoints/0/component_count")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest component count should match the restored baseline checkpoint"
    );
    assert_eq!(
        restored_chunks,
        host_actions
            .pointer("/checkpoints/0/chunk_count")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest chunk count should match the restored baseline checkpoint"
    );
    assert_eq!(
        restored_payload_bytes,
        host_actions
            .pointer("/checkpoints/0/payload_bytes")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest payload bytes should match the restored baseline checkpoint"
    );

    let baseline = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let mutated = checkpoint_chunk_checksum(host_actions, 1, "cpu0", "o3-runtime-state");
    let replayed = checkpoint_chunk_checksum(host_actions, 2, "cpu0", "o3-runtime-state");
    assert_ne!(
        mutated, baseline,
        "detailed O3 runtime state should change after ROB/LSQ/rename/FU work"
    );
    assert_eq!(
        replayed, mutated,
        "restoring the earlier O3 checkpoint should replay deterministic detailed work"
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoints",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_components",
        "Count",
        restored_components,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_chunks",
        "Count",
        restored_chunks,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_payload_bytes",
        "Byte",
        restored_payload_bytes,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_stats_resets_o3_snapshot_after_scheduled_restore() {
    let path = detailed_o3_restore_dump_stats_binary("m5-switch-cpu-o3-restore-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));

    for (path, unit) in [
        ("sim.host_actions.stats_dump.cpu0.o3.instructions", "Count"),
        (
            "sim.host_actions.stats_dump.cpu0.o3.rob_allocations",
            "Count",
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_bytes",
            "Byte",
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.rename_map_entries",
            "Count",
        ),
    ] {
        assert_stats_dump_sample(
            restored_dump,
            path,
            "counter",
            unit,
            stats_dump_sample_value(first_dump, path),
            "resettable",
        );
    }
    assert!(
        json_stat_value(&json, "sim.cpu0.o3.instructions")
            > stats_dump_sample_value(
                first_dump,
                "sim.host_actions.stats_dump.cpu0.o3.instructions"
            )
    );
    assert_json_stat(&json, "sim.cpu0.o3.lsq_store_bytes", "Byte", 4, "monotonic");
}

#[test]
fn rem6_run_m5_dump_stats_restores_o3_fu_class_snapshot_after_scheduled_restore() {
    let path = detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-fu-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );
    for dump in [first_dump, restored_dump] {
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
            "counter",
            "Count",
            2,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
            "counter",
            "Cycle",
            21,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
            "counter",
            "Count",
            1,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_latency_cycles",
            "counter",
            "Cycle",
            2,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_instructions",
            "counter",
            "Count",
            1,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
            "counter",
            "Cycle",
            19,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
            "counter",
            "Count",
            0,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
            "counter",
            "Cycle",
            0,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
            "counter",
            "Count",
            0,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
            "counter",
            "Cycle",
            0,
            "resettable",
        );
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        27,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_reports_scheduled_restore_missing_checkpoint_label() {
    let path = scheduled_host_restore_missing_label_binary("scheduled-restore-missing-label");

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
            "--host-restore-checkpoint",
            "8:missing-label",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "run should fail when a scheduled restore references a missing checkpoint label"
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("checkpoint manifest missing-label is not available"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_text_stats_alias_o3_runtime_stats_after_detailed_switch() {
    let path = detailed_o3_runtime_stats_binary("m5-switch-cpu-detailed-o3-runtime-text-stats");

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

    assert_text_count_stat(&stdout, "sim.cpu0.o3.instructions", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rob_allocations", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rob_commits", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rename_writes", 4);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_loads", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_stores", 1);
    assert_text_byte_stat(&stdout, "sim.cpu0.o3.lsq_load_bytes", 4);
    assert_text_byte_stat(&stdout, "sim.cpu0.o3.lsq_store_bytes", 4);
    assert_text_count_stat(&stdout, "system.cpu.rename.renamedInsts", 6);
    assert_text_count_stat(&stdout, "system.cpu.rename.renamedOperands", 4);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispatchedInsts", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispLoadInsts", 1);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispStoreInsts", 1);
    assert_text_count_stat(&stdout, "system.cpu.iew.instsToCommit::total", 6);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.addedLoadsAndStores", 2);
    assert_text_byte_stat(&stdout, "system.cpu.lsq0.loadBytes", 4);
    assert_text_byte_stat(&stdout, "system.cpu.lsq0.storeBytes", 4);
    assert_text_count_stat(&stdout, "system.cpu.iq.instsIssued", 6);
    assert_text_count_stat(&stdout, "system.cpu.iq.memInstsIssued", 2);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::MemRead", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::MemWrite", 1);
    assert_text_count_stat(&stdout, "system.cpu.rob.writes", 6);
    assert_text_count_stat(&stdout, "system.cpu.rob.reads", 6);
}

#[test]
fn rem6_run_text_stats_alias_o3_lsq_store_load_matches_after_detailed_switch() {
    let path = detailed_o3_lsq_store_load_match_binary(
        "m5-switch-cpu-detailed-o3-lsq-store-load-text-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_loads", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_stores", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.lsq0.storeLoadForwardingCandidates", 1);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.storeLoadForwardingMatches", 1);
}

#[test]
fn rem6_run_text_stats_alias_o3_fu_latency_after_detailed_switch() {
    let path = detailed_o3_fu_latency_binary("m5-switch-cpu-detailed-o3-fu-latency-text-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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

    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_latency_instructions", 2);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_latency_cycles", 21);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_integer_mul_instructions", 1);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_integer_mul_latency_cycles", 2);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_integer_div_instructions", 1);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_integer_div_latency_cycles", 19);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::IntDiv", 1);
}

#[test]
fn rem6_run_text_stats_alias_o3_float_misc_fu_latency_after_detailed_switch() {
    let path =
        detailed_o3_float_misc_fu_latency_binary("m5-switch-cpu-detailed-o3-float-misc-text-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
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

    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_latency_instructions", 4);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_latency_cycles", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_float_misc_instructions", 2);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_float_misc_latency_cycles", 3);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_vector_float_misc_instructions", 2);
    assert_text_cycle_stat(
        &stdout,
        "sim.cpu0.o3.fu_vector_float_misc_latency_cycles",
        3,
    );
    assert_text_count_stat(&stdout, "sim.cpu0.o3.iq.issued_inst_type.float_misc", 2);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        2,
    );
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::FloatMisc", 2);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::SimdFloatMisc", 2);
}

#[test]
fn rem6_run_does_not_record_o3_runtime_stats_after_timing_switch() {
    let path = timing_switch_o3_stats_binary("m5-switch-cpu-timing-no-o3-runtime-stats");

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
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_load_bytes");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_bytes");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_mul_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_mul_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_div_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_div_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.max_rob_occupancy");
    assert_json_stat_absent(&json, "sim.cpu0.o3.max_lsq_occupancy");
    assert_json_stat_absent(&json, "sim.cpu0.o3.rename_map_entries");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.mem_insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.mem_read");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.mem_write");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.int_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.int_div");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.dispatched_insts");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.insts_to_commit");
    assert_json_stat_absent(&json, "system.cpu.iew.dispatchedInsts");
    assert!(
        json.pointer("/cores/0/o3_runtime").is_none(),
        "timing-mode run should not emit inactive O3 runtime state: {json}"
    );
}

#[test]
fn rem6_run_text_stats_omit_o3_runtime_aliases_after_timing_switch() {
    let path = timing_switch_o3_stats_binary("m5-switch-cpu-timing-no-o3-runtime-text-stats");

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
            "text",
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    for path in [
        "sim.cpu0.o3.instructions",
        "sim.cpu0.o3.rob_allocations",
        "sim.cpu0.o3.rob_commits",
        "sim.cpu0.o3.rename_writes",
        "sim.cpu0.o3.lsq_loads",
        "sim.cpu0.o3.lsq_stores",
        "sim.cpu0.o3.lsq_load_bytes",
        "sim.cpu0.o3.lsq_store_bytes",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.cpu0.o3.fu_latency_instructions",
        "sim.cpu0.o3.fu_latency_cycles",
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "sim.cpu0.o3.fu_integer_div_instructions",
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "sim.cpu0.o3.max_rob_occupancy",
        "sim.cpu0.o3.max_lsq_occupancy",
        "sim.cpu0.o3.rename_map_entries",
        "sim.cpu0.o3.iq.insts_issued",
        "sim.cpu0.o3.iq.mem_insts_issued",
        "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        "sim.cpu0.o3.iq.issued_inst_type.int_div",
        "system.cpu.rename.renamedInsts",
        "system.cpu.rename.renamedOperands",
        "system.cpu.iew.dispatchedInsts",
        "system.cpu.iew.dispLoadInsts",
        "system.cpu.iew.dispStoreInsts",
        "system.cpu.iew.instsToCommit::total",
        "system.cpu.lsq0.addedLoadsAndStores",
        "system.cpu.lsq0.loadBytes",
        "system.cpu.lsq0.storeBytes",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.storeLoadForwardingMatches",
        "system.cpu.iq.instsIssued",
        "system.cpu.iq.memInstsIssued",
        "system.cpu.iq.issuedInstType_0::MemRead",
        "system.cpu.iq.issuedInstType_0::MemWrite",
        "system.cpu.iq.issuedInstType_0::IntMult",
        "system.cpu.iq.issuedInstType_0::IntDiv",
        "system.cpu.rob.writes",
        "system.cpu.rob.reads",
    ] {
        assert_text_stat_absent(&stdout, path);
    }
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

fn assert_text_stat_absent(stdout: &str, path: &str) {
    assert!(
        stdout
            .lines()
            .all(|line| line.split_whitespace().next() != Some(path)),
        "unexpected text stat {path} in stdout:\n{stdout}"
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
    assert_eq!(sample.pointer("/kind").and_then(Value::as_str), Some(kind));
    assert_eq!(sample.pointer("/unit").and_then(Value::as_str), Some(unit));
    assert_eq!(
        sample.pointer("/value").and_then(Value::as_u64),
        Some(value)
    );
    assert_eq!(
        sample.pointer("/reset_policy").and_then(Value::as_str),
        Some(reset_policy)
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
