use super::*;

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
