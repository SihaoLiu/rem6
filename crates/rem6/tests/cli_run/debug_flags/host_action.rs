use super::*;

#[test]
fn rem6_run_host_action_debug_flag_emits_real_m5_host_action_trace() {
    let program = riscv64_program(&[
        i_type(21, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_RESET_STATS),
        m5op(M5_DUMP_STATS),
        m5op(M5_DUMP_RESET_STATS),
        i_type(21, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-host-action", &elf);

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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("HostAction".to_string())])
    );
    let trace = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(host_action_trace_kind_count(trace, "roi_begin"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "roi_end"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "stats_reset"), 3);
    assert_eq!(host_action_trace_kind_count(trace, "stats_dump"), 3);
    assert_eq!(host_action_trace_kind_count(trace, "stop"), 1);
    assert_eq!(trace.len(), 9);
    assert_dump_reset_trace_order(trace);
    let roi_begin = trace
        .iter()
        .find(|record| record.get("kind").and_then(Value::as_str) == Some("roi_begin"))
        .expect("roi begin trace");
    assert_eq!(
        roi_begin.pointer("/work_id").and_then(Value::as_u64),
        Some(21)
    );
    assert_eq!(
        roi_begin.pointer("/thread_id").and_then(Value::as_u64),
        Some(3)
    );
    let stats_dump = trace
        .iter()
        .find(|record| record.get("kind").and_then(Value::as_str) == Some("stats_dump"))
        .expect("stats dump trace");
    assert!(
        stats_dump
            .pointer("/epoch")
            .and_then(Value::as_u64)
            .is_some_and(|epoch| epoch > 0),
        "stats dump trace: {stats_dump:?}"
    );
    assert!(
        stats_dump
            .pointer("/reset_tick")
            .and_then(Value::as_u64)
            .zip(stats_dump.pointer("/tick").and_then(Value::as_u64))
            .is_some_and(|(reset_tick, tick)| reset_tick <= tick),
        "stats dump trace: {stats_dump:?}"
    );
    assert_eq!(
        trace
            .last()
            .and_then(|record| record.pointer("/code"))
            .and_then(Value::as_i64),
        Some(0)
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.roi_begin",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.roi_end",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stats_resets",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stats_dumps",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stops",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.validated",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.captured_components",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.captured_chunks",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.captured_payload_bytes",
        "Byte",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_m5_hypercall_checkpoint_and_switch_trace() {
    let program = riscv64_program(&[
        i_type(0x321, 0, 0x0, 10, 0x13),
        i_type(11, 0, 0x0, 11, 0x13),
        i_type(12, 0, 0x0, 12, 0x13),
        i_type(13, 0, 0x0, 13, 0x13),
        i_type(14, 0, 0x0, 14, 0x13),
        i_type(15, 0, 0x0, 15, 0x13),
        m5op(M5_HYPERCALL),
        m5op(M5_CHECKPOINT),
        m5op(M5_SWITCH_CPU),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-host-action-m5-detail", &elf);

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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(host_action_trace_kind_count(trace, "injected_command"), 0);
    assert_eq!(host_action_trace_kind_count(trace, "guest_host_call"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "checkpoint"), 1);
    assert_eq!(
        host_action_trace_kind_count(trace, "execution_mode_switch"),
        1
    );
    assert_eq!(host_action_trace_kind_count(trace, "stop"), 1);
    assert_eq!(trace.len(), 4);

    let call = host_action_trace_record(trace, "guest_host_call");
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
        Some(-1)
    );
    let checkpoint = host_action_trace_record(trace, "checkpoint");
    assert_eq!(
        checkpoint.pointer("/label").and_then(Value::as_str),
        Some("gem5-m5-checkpoint")
    );
    assert!(
        checkpoint
            .pointer("/component_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "checkpoint trace: {checkpoint:?}"
    );
    assert!(
        checkpoint
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "checkpoint trace: {checkpoint:?}"
    );
    let switch = host_action_trace_record(trace, "execution_mode_switch");
    assert_eq!(
        switch.pointer("/target").and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        switch.pointer("/mode").and_then(Value::as_str),
        Some("detailed")
    );
    assert_eq!(
        switch
            .pointer("/state_transfer_captured")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        switch
            .pointer("/state_transfer_components")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "switch trace: {switch:?}"
    );
    assert_eq!(
        trace
            .last()
            .and_then(|record| record.pointer("/code"))
            .and_then(Value::as_i64),
        Some(0)
    );

    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("stats array");
    for path in [
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu0.checker.checked_instructions",
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu0.checker.mismatches",
    ] {
        assert!(
            stats
                .iter()
                .all(|sample| sample.pointer("/path").and_then(Value::as_str) != Some(path)),
            "unexpected checker-only HostAction debug stat {path}: {stats:?}"
        );
    }

    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.injected_commands",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.guest_host_calls",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.checkpoints",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switches",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stops",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_checker_quiescence_switch_scope() {
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
    let path = temp_binary("debug-flags-host-action-checker-switch", &elf);

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
            "--debug-flags",
            "HostAction",
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(
        host_action_trace_kind_count(trace, "execution_mode_switch"),
        2
    );
    let switches = trace
        .iter()
        .filter(|record| {
            record.get("kind").and_then(Value::as_str) == Some("execution_mode_switch")
        })
        .collect::<Vec<_>>();
    let host_switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .expect("host execution-mode switch array");
    assert_eq!(host_switches.len(), switches.len());

    let mut previous_checked = 0;
    for (switch, host_switch) in switches.iter().zip(host_switches) {
        assert_eq!(switch.pointer("/target"), host_switch.pointer("/target"));
        assert_eq!(switch.pointer("/mode"), host_switch.pointer("/mode"));
        assert_eq!(
            switch
                .pointer("/state_transfer/captured")
                .and_then(Value::as_bool),
            Some(true),
            "HostAction switch trace should expose nested state transfer: {switch}"
        );
        let transfer = host_switch
            .pointer("/state_transfer")
            .expect("host switch state transfer");
        for field in [
            "manifest_label",
            "manifest_tick",
            "component_count",
            "chunk_count",
            "payload_bytes",
        ] {
            let pointer = format!("/{field}");
            assert_eq!(
                switch.pointer(&format!("/state_transfer{pointer}")),
                transfer.pointer(&pointer),
                "state transfer field {field}: switch trace {switch}; host switch {host_switch}"
            );
        }
        let quiescence = switch
            .pointer("/state_transfer/quiescence_gate")
            .expect("HostAction switch trace should expose nested quiescence gate");
        let host_quiescence = transfer
            .pointer("/quiescence_gate")
            .expect("host switch quiescence gate");
        for field in [
            "validated",
            "target",
            "captured_component_count",
            "captured_chunk_count",
            "captured_payload_bytes",
        ] {
            let pointer = format!("/{field}");
            assert_eq!(
                quiescence.pointer(&pointer),
                host_quiescence.pointer(&pointer),
                "quiescence field {field}: switch trace {switch}; host switch {host_switch}"
            );
        }
        let checker = quiescence
            .pointer("/checker")
            .expect("HostAction switch trace should expose checker quiescence");
        let host_checker = host_quiescence
            .pointer("/checker")
            .expect("host switch checker quiescence");
        assert_eq!(
            checker.pointer("/checked_instructions"),
            host_checker.pointer("/checked_instructions")
        );
        assert_eq!(
            checker.pointer("/mismatches"),
            host_checker.pointer("/mismatches")
        );
        let checked = checker
            .pointer("/checked_instructions")
            .and_then(Value::as_u64)
            .expect("checker checked instructions");
        assert!(
            checked > previous_checked,
            "checker quiescence should advance across switches: {switches:?}"
        );
        previous_checked = checked;
    }

    assert_stat(
        &stdout,
        "sim.host_actions.execution_mode_switch_quiescence.checker.checked_instructions",
        "Count",
        previous_checked,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.host_actions.execution_mode_switch_quiescence.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu0.checker.checked_instructions",
        "Count",
        previous_checked,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switch.quiescence.target.cpu0.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switches",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_scheduled_checkpoint_restore_trace() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-host-action-checkpoint-restore");

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
            "--debug-flags",
            "HostAction",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(host_action_trace_kind_count(trace, "checkpoint"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "checkpoint_restore"), 1);
    assert_eq!(
        host_action_trace_kind_count(trace, "execution_mode_switch"),
        1
    );
    assert_eq!(host_action_trace_kind_count(trace, "stop"), 1);

    let checkpoint = host_action_trace_record(trace, "checkpoint");
    let restore = host_action_trace_record(trace, "checkpoint_restore");
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert!(
        host_action_trace_tick(restore) > host_action_trace_tick(checkpoint),
        "checkpoint trace: {checkpoint:?}; restore trace: {restore:?}"
    );
    assert_eq!(
        restore.pointer("/manifest_tick").and_then(Value::as_u64),
        checkpoint.pointer("/manifest_tick").and_then(Value::as_u64)
    );
    for field in ["component_count", "chunk_count", "payload_bytes"] {
        let field_pointer = format!("/{field}");
        let checkpoint_value = checkpoint
            .pointer(&field_pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("checkpoint {field}: {checkpoint:?}"));
        let restore_value = restore
            .pointer(&field_pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("checkpoint restore {field}: {restore:?}"));
        assert!(restore_value > 0, "checkpoint restore {field}: {restore:?}");
        assert_eq!(
            restore_value, checkpoint_value,
            "restored manifest {field} should match the baseline checkpoint"
        );
    }
    let authority = restore
        .pointer("/execution_mode_authority")
        .unwrap_or_else(|| panic!("checkpoint restore trace should expose authority: {restore}"));
    for (path, expected) in [
        ("/present_manifests", 1),
        ("/cleared_manifests", 0),
        ("/decode_errors", 0),
        ("/targets", 1),
        ("/mode/functional", 0),
        ("/mode/timing", 0),
        ("/mode/detailed", 1),
        ("/target/cpu0/mode/functional", 0),
        ("/target/cpu0/mode/timing", 0),
        ("/target/cpu0/mode/detailed", 1),
    ] {
        assert_eq!(
            authority.pointer(path).and_then(Value::as_u64),
            Some(expected),
            "checkpoint restore authority path {path}: {authority}"
        );
    }

    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.checkpoints",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
    for (path, value) in [
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.manifests",
            1,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.cleared_manifests",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.decode_errors",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.targets",
            1,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.functional",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.timing",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
            1,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.functional",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.timing",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
            1,
        ),
    ] {
        assert_stat(&stdout, path, "Count", value, "monotonic");
    }
}
