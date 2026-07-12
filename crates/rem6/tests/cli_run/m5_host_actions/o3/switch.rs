use super::*;

#[path = "switch/checkpoint_rollback.rs"]
mod checkpoint_rollback;
#[path = "switch/mmio_scalar_load.rs"]
mod mmio_scalar_load;
#[path = "switch/multicore_mmio_scalar_load.rs"]
mod multicore_mmio_scalar_load;
#[path = "switch/multicore_scalar_load.rs"]
mod multicore_scalar_load;
#[path = "switch/scalar_load.rs"]
mod scalar_load;
#[path = "switch/translated_scalar_load.rs"]
mod translated_scalar_load;

#[test]
fn rem6_run_host_switch_transfers_live_o3_fu_authority_until_retirement() {
    let path = live_o3_mode_transfer_binary("host-switch-live-o3-fu-authority");
    let baseline = run_live_o3_mode_transfer(&path, &[]);
    let baseline_div = live_mode_transfer_event(&baseline, "0x8000000c");
    let baseline_first = live_mode_transfer_event(&baseline, "0x80000010");
    let baseline_second = live_mode_transfer_event(&baseline, "0x80000014");
    let baseline_third = live_mode_transfer_event(&baseline, "0x80000018");
    assert!(live_mode_transfer_event_if_present(&baseline, "0x8000001c").is_some());
    let switch_tick = event_u64_field(baseline_div, "writeback_tick").saturating_sub(4);
    assert!(switch_tick > event_u64_field(baseline_div, "issue_tick"));
    assert!(switch_tick < event_u64_field(baseline_div, "writeback_tick"));

    let json = run_live_o3_mode_transfer(&path, &[(switch_tick, "timing")]);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("01000000100000000a000000")
    );

    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution-mode switches: {json}"));
    assert_eq!(switches.len(), 3, "switches: {switches:?}");
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .unwrap_or_else(|| panic!("missing live detailed-to-timing switch: {switches:?}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("timing switch tick");
    assert!(timing_action_tick > event_u64_field(baseline_div, "issue_tick"));
    assert!(timing_action_tick < event_u64_field(baseline_div, "writeback_tick"));

    let timing_transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing live timing switch transfer: {timing_switch}"));
    let timing_runtime = latest_transfer_o3_runtime_chunk(timing_transfer, "cpu0");
    assert_eq!(
        timing_runtime
            .pointer("/decode_error")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        timing_runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4),
        "the switch must capture DIV plus three younger live FU rows: {timing_runtime}"
    );
    assert!(
        timing_runtime
            .pointer("/stats_rename_map_entries")
            .and_then(Value::as_u64)
            .is_some_and(|entries| entries >= 6),
        "the switch must carry the live rename owners: {timing_runtime}"
    );
    assert_eq!(
        timing_runtime
            .pointer("/live_retire_gate_ready_tick")
            .and_then(Value::as_u64),
        Some(event_u64_field(baseline_div, "writeback_tick")),
        "the switch transfer must retain the absolute DIV wake: {timing_runtime}"
    );

    let resumed_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("detailed")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("timing")
        })
        .unwrap_or_else(|| panic!("missing timing-to-detailed switch: {switches:?}"));
    let resumed_runtime = latest_transfer_o3_runtime_chunk(
        resumed_switch
            .pointer("/state_transfer")
            .expect("resumed switch transfer"),
        "cpu0",
    );
    assert!(
        resumed_runtime
            .pointer("/live_retire_gate_ready_tick")
            .is_some_and(Value::is_null),
        "the inherited gate must drain before detailed admission resumes: {resumed_runtime}"
    );

    for (pc, baseline_event) in [
        ("0x8000000c", baseline_div),
        ("0x80000010", baseline_first),
        ("0x80000014", baseline_second),
        ("0x80000018", baseline_third),
    ] {
        let transferred = live_mode_transfer_event(&json, pc);
        assert_eq!(
            event_u64_field(transferred, "issue_tick"),
            event_u64_field(baseline_event, "issue_tick"),
            "mode transfer must preserve the original O3 issue tick for {pc}: {transferred}"
        );
        assert_eq!(
            event_u64_field(transferred, "writeback_tick"),
            event_u64_field(baseline_event, "writeback_tick"),
            "mode transfer must preserve the original O3 writeback tick for {pc}: {transferred}"
        );
        assert_eq!(
            event_u64_field(transferred, "commit_tick"),
            event_u64_field(baseline_event, "commit_tick"),
            "mode transfer must preserve the original O3 commit tick for {pc}: {transferred}"
        );
    }
    let transferred_events = [
        live_mode_transfer_event(&json, "0x8000000c"),
        live_mode_transfer_event(&json, "0x80000010"),
        live_mode_transfer_event(&json, "0x80000014"),
        live_mode_transfer_event(&json, "0x80000018"),
    ];
    assert!(transferred_events.windows(2).all(|events| {
        event_u64_field(events[0], "commit_tick") <= event_u64_field(events[1], "commit_tick")
    }));
    assert!(
        live_mode_transfer_event_if_present(&json, "0x8000001c").is_none(),
        "timing mode must not admit the first instruction beyond inherited O3 authority"
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.live_retire_gate.scheduled_waits",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_toml_schedules_host_execution_mode_switch() {
    let path = live_o3_mode_transfer_binary("toml-host-switch-live-o3-fu-authority");
    let baseline = run_live_o3_mode_transfer(&path, &[]);
    let divide = live_mode_transfer_event(&baseline, "0x8000000c");
    let switch_tick = event_u64_field(divide, "writeback_tick").saturating_sub(4);
    let workspace = temp_workspace("toml-host-switch-live-o3");
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        format!(
            r#"[run]
isa = "riscv"
binary = "{}"
max_tick = 260
stats_format = "json"
execute = true
memory_system = "direct"
m5_switch_cpu_mode = "detailed"
host_execution_mode_switches = ["{switch_tick}:cpu0:timing"]
memory_dumps = ["0x80000080:12"]
"#,
            path.display()
        ),
    )
    .unwrap();

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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("01000000100000000a000000")
    );
    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution-mode switches: {json}"));
    assert!(switches.iter().any(|switch| {
        switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
            && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
            && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
    }));
}

#[test]
fn rem6_run_orders_same_tick_host_mode_switch_before_checkpoint() {
    let mut words = vec![i_type(1, 0, 0x0, 5, 0x13)];
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("same-tick-host-mode-switch-before-checkpoint", &elf);

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
            "--host-switch-cpu-mode",
            "0:cpu0:detailed",
            "--host-checkpoint",
            "0:same-tick-mode",
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
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .unwrap_or_else(|| panic!("missing same-tick checkpoint: {json}"));
    assert_eq!(
        checkpoint.pointer("/label").and_then(Value::as_str),
        Some("same-tick-mode")
    );
    let modes = checkpoint
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint execution modes: {checkpoint}"));
    assert_eq!(modes.len(), 1, "checkpoint execution modes: {modes:?}");
    assert_eq!(
        modes[0].pointer("/target").and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        modes[0].pointer("/mode").and_then(Value::as_str),
        Some("detailed")
    );
}

#[test]
fn rem6_run_scopes_multicore_o3_switch_transfer_stats_by_target() {
    let path = multicore_hart1_detailed_o3_binary("m5-switch-cpu-hart1-detailed-o3-transfer-scope");

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
    let latest_switch = host_actions
        .pointer("/execution_mode_switches/0")
        .unwrap_or_else(|| panic!("missing execution-mode switch: {host_actions}"));
    for (stat_path, unit, artifact_pointer) in [
        (
            "sim.host_actions.execution_mode_switch.latest_tick",
            "Tick",
            "/tick",
        ),
        (
            "sim.host_actions.execution_mode_switch.latest_event",
            "Count",
            "/event",
        ),
        (
            "sim.host_actions.execution_mode_switch.latest_source",
            "Count",
            "/source",
        ),
        (
            "sim.host_actions.execution_mode_switch.latest_stats_epoch",
            "Count",
            "/stats_epoch",
        ),
        (
            "sim.host_actions.execution_mode_switch.latest_stats_reset_tick",
            "Tick",
            "/stats_reset_tick",
        ),
    ] {
        assert_json_stat(
            &json,
            stat_path,
            unit,
            latest_switch
                .pointer(artifact_pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "latest execution-mode switch should expose {artifact_pointer}: {latest_switch}"
                    )
                }),
            "monotonic",
        );
    }
    for (mode, expected) in [("functional", 0), ("timing", 0), ("detailed", 1)] {
        assert_json_stat(
            &json,
            &format!("sim.host_actions.execution_mode_switch.latest_mode.{mode}"),
            "Count",
            expected,
            "monotonic",
        );
    }
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.latest_previous_mode.none",
        "Count",
        1,
        "monotonic",
    );
    for mode in ["functional", "timing", "detailed"] {
        assert_json_stat(
            &json,
            &format!("sim.host_actions.execution_mode_switch.latest_previous_mode.{mode}"),
            "Count",
            0,
            "monotonic",
        );
    }
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.latest_target.cpu1.mode.detailed",
        "Count",
        1,
        "monotonic",
    );
    let latest_transfer = latest_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing switch state transfer: {latest_switch}"));
    for (stat_path, unit, artifact_pointer) in [
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_manifest_tick",
            "Tick",
            "/manifest_tick",
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_component_count",
            "Count",
            "/component_count",
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_chunk_count",
            "Count",
            "/chunk_count",
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_payload_bytes",
            "Byte",
            "/payload_bytes",
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_quiescence_captured_components",
            "Count",
            "/quiescence_gate/captured_component_count",
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_quiescence_captured_chunks",
            "Count",
            "/quiescence_gate/captured_chunk_count",
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_quiescence_captured_payload_bytes",
            "Byte",
            "/quiescence_gate/captured_payload_bytes",
        ),
    ] {
        assert_json_stat(
            &json,
            stat_path,
            unit,
            latest_transfer
                .pointer(artifact_pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "latest execution-mode switch transfer should expose {artifact_pointer}: {latest_transfer}"
                    )
                }),
            "monotonic",
        );
    }
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.latest_quiescence_validated",
        "Count",
        1,
        "monotonic",
    );

    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.components",
        "Count",
        execution_mode_switch_transfer_total(host_actions, "component_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.chunks",
        "Count",
        execution_mode_switch_transfer_total(host_actions, "chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.payload_bytes",
        "Byte",
        execution_mode_switch_transfer_total(host_actions, "payload_bytes"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu1.captured_components",
        "Count",
        execution_mode_switch_quiescence_target_total(
            host_actions,
            "cpu1",
            "captured_component_count",
        ),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu1.captured_chunks",
        "Count",
        execution_mode_switch_quiescence_target_total(host_actions, "cpu1", "captured_chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu1.captured_payload_bytes",
        "Byte",
        execution_mode_switch_quiescence_target_total(
            host_actions,
            "cpu1",
            "captured_payload_bytes",
        ),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.components",
        "Count",
        execution_mode_switch_transfer_total(host_actions, "component_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.chunks",
        "Count",
        execution_mode_switch_transfer_total(host_actions, "chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.payload_bytes",
        "Byte",
        execution_mode_switch_transfer_total(host_actions, "payload_bytes"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu1.validated",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu1.captured_components",
        "Count",
        execution_mode_switch_quiescence_target_total(
            host_actions,
            "cpu1",
            "captured_component_count",
        ),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu1.captured_chunks",
        "Count",
        execution_mode_switch_quiescence_target_total(host_actions, "cpu1", "captured_chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu1.captured_payload_bytes",
        "Byte",
        execution_mode_switch_quiescence_target_total(
            host_actions,
            "cpu1",
            "captured_payload_bytes",
        ),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.components",
        "Count",
        execution_mode_switch_transfer_component_total(host_actions, "cpu1", "component_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.chunks",
        "Count",
        execution_mode_switch_transfer_component_total(host_actions, "cpu1", "chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.payload_bytes",
        "Byte",
        execution_mode_switch_transfer_component_total(host_actions, "cpu1", "payload_bytes"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.components",
        "Count",
        execution_mode_switch_transfer_component_total(host_actions, "cpu1", "component_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.chunks",
        "Count",
        execution_mode_switch_transfer_component_total(host_actions, "cpu1", "chunk_count"),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.payload_bytes",
        "Byte",
        execution_mode_switch_transfer_component_total(host_actions, "cpu1", "payload_bytes"),
        "monotonic",
    );
    for chunk in ["xregs", "in-order-pipeline", "o3-runtime-state"] {
        let stat_chunk = stat_path_segment(chunk);
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.chunk.{stat_chunk}.chunks"
            ),
            "Count",
            execution_mode_switch_transfer_component_chunk_total(
                host_actions,
                "cpu1",
                chunk,
                "chunk_count",
            ),
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.chunk.{stat_chunk}.payload_bytes"
            ),
            "Byte",
            execution_mode_switch_transfer_component_chunk_total(
                host_actions,
                "cpu1",
                chunk,
                "payload_bytes",
            ),
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.chunk.{stat_chunk}.chunks"
            ),
            "Count",
            execution_mode_switch_transfer_component_chunk_total(
                host_actions,
                "cpu1",
                chunk,
                "chunk_count",
            ),
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.chunk.{stat_chunk}.payload_bytes"
            ),
            "Byte",
            execution_mode_switch_transfer_component_chunk_total(
                host_actions,
                "cpu1",
                chunk,
                "payload_bytes",
            ),
            "monotonic",
        );
    }
    let o3_checksum = execution_mode_switch_transfer_component_chunk_checksum(
        host_actions,
        0,
        "cpu1",
        "o3-runtime-state",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.chunk.o3_runtime_state.payload_checksum_accumulator",
        "Unspecified",
        parse_hex_u64(&o3_checksum),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.chunk.o3_runtime_state.payload_checksum_accumulator",
        "Unspecified",
        parse_hex_u64(&o3_checksum),
        "monotonic",
    );
    let o3_runtime = latest_transfer_o3_runtime_chunk(latest_transfer, "cpu1");
    assert_eq!(
        o3_runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false),
        "O3 switch transfer runtime chunk should decode cleanly: {o3_runtime}"
    );
    for (field, unit) in [
        ("stats_lsq_operation_load", "Count"),
        ("stats_lsq_operation_store", "Count"),
        ("stats_lsq_data_latency_ticks", "Tick"),
        ("stats_lsq_data_latency_max_ticks", "Tick"),
        ("stats_lsq_data_latency_min_ticks", "Tick"),
    ] {
        let value = o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("missing decoded switch transfer field {field}: {o3_runtime}")
            });
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu1.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.cpu1.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu1.component.cpu1.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu1.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu1.component.cpu1.chunk.o3_runtime_state.o3_runtime.{field}"
            ),
            unit,
            value,
            "monotonic",
        );
    }
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu0.components",
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.components",
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.captured_components",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu0.components",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu0.component.cpu0.components",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu0.component.cpu0.chunk.o3_runtime_state.payload_checksum_accumulator",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.stats_lsq_operation_load",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu0.captured_components",
    );
}

#[test]
fn rem6_run_traces_sparse_multicore_o3_switch_transfer_components_by_target() {
    let path =
        sparse_three_core_detailed_o3_restore_trace_binary("m5-switch-cpu-sparse-o3-host-trace");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "340",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "3",
            "--parallel-workers",
            "3",
            "--memory-system",
            "direct",
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
    let host_switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing host switch actions: {json}"));
    assert_eq!(host_switches.len(), 2, "host switches: {host_switches:?}");
    let trace_switches = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing HostAction trace: {json}"))
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        trace_switches.len(),
        host_switches.len(),
        "trace switches: {trace_switches:?}"
    );
    let latest_switch = host_switch_by_target(host_switches, "cpu2");
    let latest_transfer = latest_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing latest switch transfer: {latest_switch}"));
    let latest_transfer_component = latest_transfer
        .pointer("/components/0")
        .unwrap_or_else(|| panic!("missing latest transfer component: {latest_transfer}"));
    let latest_o3_chunk = latest_transfer_component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
        })
        .unwrap_or_else(|| panic!("missing latest O3 runtime chunk: {latest_transfer_component}"));
    for prefix in [
        "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu2",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu2",
    ] {
        for (suffix, unit, value) in [
            (
                "components",
                "Count",
                latest_transfer
                    .pointer("/component_count")
                    .and_then(Value::as_u64)
                    .unwrap(),
            ),
            (
                "chunks",
                "Count",
                latest_transfer
                    .pointer("/chunk_count")
                    .and_then(Value::as_u64)
                    .unwrap(),
            ),
            (
                "payload_bytes",
                "Byte",
                latest_transfer
                    .pointer("/payload_bytes")
                    .and_then(Value::as_u64)
                    .unwrap(),
            ),
        ] {
            assert_json_stat(
                &json,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
    for prefix in [
        "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu2.component.cpu2",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu2.component.cpu2",
    ] {
        for (suffix, unit, value) in [
            ("components", "Count", 1),
            (
                "chunks",
                "Count",
                latest_transfer_component
                    .pointer("/chunk_count")
                    .and_then(Value::as_u64)
                    .unwrap(),
            ),
            (
                "payload_bytes",
                "Byte",
                latest_transfer_component
                    .pointer("/payload_bytes")
                    .and_then(Value::as_u64)
                    .unwrap(),
            ),
        ] {
            assert_json_stat(
                &json,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
    for prefix in [
        "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu0",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu0",
    ] {
        for (suffix, unit) in [
            ("components", "Count"),
            ("chunks", "Count"),
            ("payload_bytes", "Byte"),
        ] {
            assert_json_stat(&json, &format!("{prefix}.{suffix}"), unit, 0, "monotonic");
        }
    }

    for target in ["cpu0", "cpu2"] {
        let trace_switch = trace_switch_by_target(&trace_switches, target);
        let host_switch = host_switch_by_target(host_switches, target);
        assert_eq!(
            trace_switch.pointer("/state_transfer/components"),
            host_switch.pointer("/state_transfer/components"),
            "HostAction trace should preserve component/chunk details for {target}: trace {trace_switch}; host {host_switch}"
        );
        assert_trace_switch_component_chunk(trace_switch, target, "xregs");
        assert_trace_switch_component_chunk(trace_switch, target, "in-order-pipeline");
        assert_trace_switch_component_chunk(trace_switch, target, "o3-runtime-state");
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.{target}.component.{target}.chunk.o3_runtime_state.payload_checksum_accumulator"
            ),
            "Unspecified",
            parse_hex_u64(&execution_mode_switch_transfer_component_chunk_checksum(
                json.pointer("/host_actions").expect("host actions"),
                host_switch_index(host_switches, target),
                target,
                "o3-runtime-state",
            )),
            "monotonic",
        );
    }
    for prefix in [
        "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu2.component.cpu2.chunk.o3_runtime_state",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu2.component.cpu2.chunk.o3_runtime_state",
    ] {
        for (suffix, unit, value) in [
            ("chunks", "Count", 1),
            (
                "payload_bytes",
                "Byte",
                latest_o3_chunk
                    .pointer("/payload_bytes")
                    .and_then(Value::as_u64)
                    .unwrap(),
            ),
            (
                "payload_checksum_accumulator",
                "Unspecified",
                parse_hex_u64(&execution_mode_switch_transfer_component_chunk_checksum(
                    json.pointer("/host_actions").expect("host actions"),
                    host_switch_index(host_switches, "cpu2"),
                    "cpu2",
                    "o3-runtime-state",
                )),
            ),
        ] {
            assert_json_stat(
                &json,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu0.component.cpu0.components",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu0.component.cpu0.chunk.o3_runtime_state.chunks",
    );
    assert!(
        trace_switches
            .iter()
            .all(|record| record.pointer("/target").and_then(Value::as_str) != Some("cpu1")),
        "CPU1 should remain suppressed from sparse switch trace: {trace_switches:?}"
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.components",
    );
}

fn trace_switch_by_target<'a>(switches: &'a [&Value], target: &str) -> &'a Value {
    switches
        .iter()
        .copied()
        .find(|switch| switch.pointer("/target").and_then(Value::as_str) == Some(target))
        .unwrap_or_else(|| panic!("missing trace switch for {target}: {switches:?}"))
}

fn host_switch_by_target<'a>(switches: &'a [Value], target: &str) -> &'a Value {
    switches
        .iter()
        .find(|switch| switch.pointer("/target").and_then(Value::as_str) == Some(target))
        .unwrap_or_else(|| panic!("missing host switch for {target}: {switches:?}"))
}

fn host_switch_index(switches: &[Value], target: &str) -> usize {
    switches
        .iter()
        .position(|switch| switch.pointer("/target").and_then(Value::as_str) == Some(target))
        .unwrap_or_else(|| panic!("missing host switch for {target}: {switches:?}"))
}

fn assert_trace_switch_component_chunk(switch: &Value, component: &str, chunk: &str) {
    let components = switch
        .pointer("/state_transfer/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing trace switch components: {switch}"));
    let component = components
        .iter()
        .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing trace component {component}: {switch}"));
    let chunks = component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing trace chunks for {component}: {switch}"));
    let chunk = chunks
        .iter()
        .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
        .unwrap_or_else(|| panic!("missing trace chunk {chunk}: {component}"));
    assert!(
        chunk
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "trace chunk should expose payload bytes: {chunk}"
    );
    assert!(
        chunk
            .pointer("/payload_checksum")
            .and_then(Value::as_str)
            .is_some_and(|checksum| checksum.starts_with("0x") && checksum.len() == 18),
        "trace chunk should expose payload checksum: {chunk}"
    );
}

fn run_live_o3_mode_transfer(path: &Path, switches: &[(u64, &str)]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
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
        "--m5-switch-cpu-mode",
        "detailed",
        "--debug-flags",
        "O3,HostAction",
        "--dump-memory",
        "0x80000080:12",
    ]);
    for (tick, mode) in switches {
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu0:{mode}")]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "scheduled switches {switches:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn live_o3_mode_transfer_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        r_type(1, 1, 2, 0x4, 3, 0x33),
        i_type(5, 0, 0x0, 4, 0x13),
        i_type(11, 4, 0x0, 5, 0x13),
        i_type(9, 0, 0x0, 6, 0x13),
        i_type(10, 0, 0x0, 7, 0x13),
        m5op(M5_SWITCH_CPU),
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13),
        s_type(0, 3, 12, 0b010),
        s_type(4, 5, 12, 0b010),
        s_type(8, 7, 12, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn live_mode_transfer_event<'a>(json: &'a Value, pc: &str) -> &'a Value {
    live_mode_transfer_event_if_present(json, pc)
        .unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
}

fn live_mode_transfer_event_if_present<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
}

fn event_u64_field(event: &Value, field: &str) -> u64 {
    event
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}

fn latest_transfer_o3_runtime_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    let components = transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing switch transfer components: {transfer}"));
    let component = components
        .iter()
        .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing switch transfer component {component}: {transfer}"));
    let chunks = component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing switch transfer chunks: {component}"));
    chunks
        .iter()
        .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap_or_else(|| panic!("missing decoded O3 switch transfer chunk: {component}"))
}
