use super::result_classes::{unique_result_temp_binary, RESULT_MAX_TICK, RESULT_TEMP_ID};
use super::result_support::{
    assert_register, assert_register_absent, assert_resource_counter, assert_rob_sequence_absent,
    data_trace, event_str, json_u64, memory_dump_hex, memory_result_event_at_pc,
    rob_entry_at_sequence,
};
use super::*;

const SC_SOURCE: u64 = 0x8000_0080;
const SUCCESS_LR_PC: &str = "0x8000001c";
const SUCCESS_DIV_PC: &str = "0x80000020";
const SUCCESS_SC_PC: &str = "0x80000024";
const SUCCESS_RESULT_STORE_PC: &str = "0x80000028";
const DIRECT_ROUTE_DELAY: u64 = 4;
const HIERARCHY_ROUTE_DELAY: u64 = 3;
const ROUTE_DELAY_CANDIDATES: [u64; 13] = [1, 2, 3, 4, 6, 8, 9, 10, 12, 14, 16, 20, 24];

#[test]
fn rem6_run_o3_store_conditional_result_width_one_serializes_direct() {
    let fixture = StoreConditionalFixture::successful();
    let route_delay = calibrate_success_collision(&fixture, "direct");
    assert_eq!(route_delay, DIRECT_ROUTE_DELAY);
    assert_success_collision(&fixture, "direct", 1, route_delay);
}

#[test]
fn rem6_run_o3_store_conditional_result_width_two_exact_fit_direct() {
    let fixture = StoreConditionalFixture::successful();
    assert_success_collision(&fixture, "direct", 2, DIRECT_ROUTE_DELAY);
}

#[test]
fn rem6_run_o3_store_conditional_result_cache_fabric_dram() {
    let fixture = StoreConditionalFixture::successful();
    let route_delay = calibrate_success_collision(&fixture, "cache-fabric-dram");
    assert_eq!(route_delay, HIERARCHY_ROUTE_DELAY);
    let completed = assert_success_collision(&fixture, "cache-fabric-dram", 1, route_delay);

    assert_memory_hierarchy_activity(&completed);
    assert_sc_hierarchy_request(&completed);
}

#[test]
fn rem6_run_o3_store_conditional_failure_is_local_and_deferred() {
    let fixture = StoreConditionalFixture::failed();
    let completed = fixture.run("direct", 1, 1, RESULT_MAX_TICK, "detailed");
    let sc = event_at_pc(&completed, SUCCESS_SC_PC);
    let result_store = event_at_pc(&completed, SUCCESS_RESULT_STORE_PC);
    let response_tick = event_u64(sc, "lsq_data_response_tick");
    let admitted_tick = event_u64(sc, "writeback_tick");
    assert_sc_result_event(sc, true);
    assert_eq!(admitted_tick, response_tick + 1);
    assert_eq!(event_u64(sc, "commit_tick"), admitted_tick);
    assert!(event_u64(result_store, "issue_tick") >= admitted_tick);
    assert_register(&completed, "x7", "0x1");
    assert_eq!(
        memory_dump_hex(&completed, SC_SOURCE),
        Some("09000000000000000100000000000000")
    );
    assert_local_failure_has_no_data_or_target_request(&completed, event_u64(sc, "issue_tick"));
    assert_local_failure_traffic(&completed, event_u64(result_store, "issue_tick"));
    assert_writeback_counters(&completed, [1, 1, 0, 0, 1, 0]);
    assert_sc_operation_counters(&completed, 1);
    assert_resource_counter(&completed, "transport.data.activity", 1);
    for suffix in ["cache.data", "fabric", "dram"] {
        assert_resource_counter(&completed, &format!("{suffix}.activity"), 0);
    }
    assert_eq!(
        json_u64(
            &completed,
            "/cores/0/o3_runtime/lsq_store_conditional_failures"
        ),
        1
    );
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.lsq_store_conditional_failures",
        "Count",
        1,
        "monotonic",
    );

    let before = fixture.run("direct", 1, 1, response_tick, "detailed");
    assert_register(&before, "x7", "0x9");
    assert_eq!(
        memory_dump_hex(&before, SC_SOURCE),
        Some("09000000000000008877665544332211")
    );
    assert!(data_trace(&before).is_empty());
    assert!(before
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records
            .iter()
            .all(|record| record.pointer("/channel").and_then(Value::as_str) != Some("data"))));
    assert_resource_counter(&before, "transport.data.activity", 0);
    let sequence = event_u64(sc, "sequence");
    let at_admission = fixture.run("direct", 1, 1, admitted_tick, "detailed");
    assert_register(&at_admission, "x7", "0x1");
    assert_rob_sequence_absent(&at_admission, sequence);
}

#[test]
fn rem6_run_o3_store_conditional_result_live_actions_reject() {
    let fixture = StoreConditionalFixture::successful();
    let route_delay = DIRECT_ROUTE_DELAY;
    let baseline = fixture.run("direct", 1, route_delay, RESULT_MAX_TICK, "detailed");
    let sc = memory_result_event_at_pc(&baseline, SUCCESS_SC_PC);
    assert_sc_result_event(sc, false);
    let action_tick = event_u64(sc, "lsq_data_response_tick");
    let effective_action_tick = action_tick + 1;
    assert!(event_u64(sc, "lsq_data_response_tick") < effective_action_tick);
    assert!(effective_action_tick < event_u64(sc, "writeback_tick"));

    for (flag, argument, label) in [
        (
            "--host-checkpoint",
            format!("{action_tick}:sc-result-live"),
            "live SC checkpoint",
        ),
        (
            "--host-switch-cpu-mode",
            format!("{action_tick}:cpu0:timing"),
            "live SC mode switch",
        ),
    ] {
        let artifact = unique_sc_output(label);
        let mut command = fixture.command("direct", 1, route_delay, RESULT_MAX_TICK, "detailed");
        command.args([
            flag,
            argument.as_str(),
            "--output",
            artifact.to_str().unwrap(),
        ]);
        let output = wait_for_sc_command(command, label);
        assert_eq!(output.status.code(), Some(2), "{label} status: {output:?}");
        assert!(output.stdout.is_empty(), "{label} stdout: {output:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
        );
        assert!(!artifact.exists(), "{label} emitted {}", artifact.display());
    }
}

#[test]
fn rem6_run_timing_suppresses_o3_store_conditional_result_surface() {
    for (fixture, route_delay, failed, expected_hex) in [
        (
            StoreConditionalFixture::successful(),
            DIRECT_ROUTE_DELAY,
            false,
            "2a000000000000000000000000000000",
        ),
        (
            StoreConditionalFixture::failed(),
            1,
            true,
            "09000000000000000100000000000000",
        ),
    ] {
        assert_timing_suppression(&fixture, route_delay, failed, expected_hex);
    }
}

struct StoreConditionalFixture {
    binary: std::path::PathBuf,
}

impl StoreConditionalFixture {
    fn successful() -> Self {
        Self {
            binary: store_conditional_binary("success", true),
        }
    }

    fn failed() -> Self {
        Self {
            binary: store_conditional_binary("failure", false),
        }
    }

    fn command(
        &self,
        memory_system: &str,
        writeback_width: usize,
        route_delay: u64,
        max_tick: u64,
        mode: &str,
    ) -> Command {
        let config = WritebackRunConfig::detailed_json(
            memory_system,
            writeback_width,
            route_delay,
            max_tick,
        )
        .with_switch_mode(mode);
        let mut command = writeback_command(&self.binary, config);
        command.args(["--host-event-delay", "1", "--dump-memory", "0x80000080:16"]);
        command
    }

    fn run(
        &self,
        memory_system: &str,
        writeback_width: usize,
        route_delay: u64,
        max_tick: u64,
        mode: &str,
    ) -> Value {
        let output = wait_for_sc_command(
            self.command(memory_system, writeback_width, route_delay, max_tick, mode),
            "SC result fixture",
        );
        assert!(
            output.status.success(),
            "{memory_system} width {writeback_width} route {route_delay} mode {mode} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("invalid SC result JSON: {error}"));
        if max_tick == RESULT_MAX_TICK {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host")
            );
            assert_eq!(json_u64(&json, "/simulation/stop_code"), 0);
        } else {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_at_tick_limit")
            );
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
        }
        json
    }
}

fn calibrate_success_collision(fixture: &StoreConditionalFixture, memory_system: &str) -> u64 {
    let mut collisions = Vec::new();
    let mut observations = Vec::new();
    for route_delay in ROUTE_DELAY_CANDIDATES {
        let json = fixture.run(memory_system, 2, route_delay, RESULT_MAX_TICK, "detailed");
        let sc = memory_result_event_at_pc(&json, SUCCESS_SC_PC);
        let div = event_at_pc(&json, SUCCESS_DIV_PC);
        let sc_raw_ready = event_u64(sc, "lsq_data_response_tick") + 1;
        let div_raw_ready = event_u64(div, "issue_tick") + 19;
        observations.push((route_delay, sc_raw_ready, div_raw_ready));
        if sc_raw_ready == div_raw_ready {
            collisions.push(route_delay);
        }
    }
    assert_eq!(
        collisions.len(),
        1,
        "{memory_system} SC result must have one bounded collision: {observations:?}"
    );
    collisions[0]
}

fn assert_success_collision(
    fixture: &StoreConditionalFixture,
    memory_system: &str,
    writeback_width: usize,
    route_delay: u64,
) -> Value {
    let completed = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        RESULT_MAX_TICK,
        "detailed",
    );
    let sc = memory_result_event_at_pc(&completed, SUCCESS_SC_PC);
    let div = event_at_pc(&completed, SUCCESS_DIV_PC);
    let result_store = event_at_pc(&completed, SUCCESS_RESULT_STORE_PC);
    let sc_raw_ready = event_u64(sc, "lsq_data_response_tick") + 1;
    let div_raw_ready = event_u64(div, "issue_tick") + 19;
    let admitted_tick = event_u64(sc, "writeback_tick");
    assert_sc_result_event(sc, false);
    assert_eq!(event_str(div, "fu_latency_class"), "scalar_integer_div");
    assert_eq!(event_u64(div, "fu_latency_cycles"), 19);
    assert_eq!(sc_raw_ready, div_raw_ready);
    assert_eq!(event_u64(div, "writeback_tick"), div_raw_ready);
    assert_eq!(
        admitted_tick,
        sc_raw_ready + u64::from(writeback_width == 1)
    );
    assert_eq!(event_u64(sc, "commit_tick"), admitted_tick);
    assert!(event_u64(result_store, "issue_tick") >= admitted_tick);
    assert_register_absent(&completed, "x7");
    assert_eq!(
        memory_dump_hex(&completed, SC_SOURCE),
        Some("2a000000000000000000000000000000")
    );
    assert_success_sc_data_records(&completed);
    assert_success_writeback_counters(&completed, writeback_width);
    assert_sc_operation_counters(&completed, 0);
    if memory_system == "direct" {
        assert_resource_counter(&completed, "transport.data.activity", 3);
        for suffix in ["cache.data", "fabric", "dram"] {
            assert_resource_counter(&completed, &format!("{suffix}.activity"), 0);
        }
    }

    let sequence = event_u64(sc, "sequence");
    let before = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        admitted_tick,
        "detailed",
    );
    assert_register(&before, "x7", "0x9");
    assert_eq!(
        memory_dump_hex(&before, SC_SOURCE),
        Some("2a000000000000008877665544332211")
    );
    let row = rob_entry_at_sequence(&before, sequence);
    assert_eq!(row.pointer("/ready").and_then(Value::as_bool), Some(false));
    assert_eq!(
        row.pointer("/live_staged").and_then(Value::as_bool),
        Some(true)
    );
    let reservation = writeback_reservation_at_sequence(&before, sequence);
    assert_eq!(event_u64(reservation, "raw_ready_tick"), sc_raw_ready);
    assert_eq!(event_u64(reservation, "admitted_tick"), admitted_tick);
    assert_eq!(
        event_u64(reservation, "slot"),
        u64::from(writeback_width == 2)
    );

    let at_admission = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        admitted_tick + 1,
        "detailed",
    );
    assert_register_absent(&at_admission, "x7");
    assert_rob_sequence_absent(&at_admission, sequence);
    completed
}

fn assert_sc_result_event(event: &Value, failed: bool) {
    assert_eq!(event_str(event, "lsq_operation"), "store_conditional");
    assert_eq!(event_u64(event, "rename_writes"), 1);
    assert_eq!(event_u64(event, "lsq_stores"), 1);
    assert_eq!(event_str(event, "lsq_store_address"), "0x80000080");
    assert_eq!(event_u64(event, "lsq_store_bytes"), 8);
    assert_eq!(
        event
            .pointer("/lsq_store_conditional_failed")
            .and_then(Value::as_bool),
        Some(failed)
    );
}

fn assert_success_sc_data_records(json: &Value) {
    let records = data_trace(json);
    let expected = [
        ("load", SC_SOURCE, SUCCESS_LR_PC),
        ("store", SC_SOURCE, SUCCESS_SC_PC),
        ("store", SC_SOURCE + 8, SUCCESS_RESULT_STORE_PC),
    ];
    assert_eq!(
        records.len(),
        expected.len(),
        "unexpected Data trace: {records:?}"
    );
    for (record, (kind, address, pc)) in records.iter().zip(expected) {
        assert_eq!(event_str(record, "kind"), kind);
        assert_eq!(event_str(record, "address"), format!("0x{address:x}"));
        assert_eq!(event_u64(record, "size"), 8);
        assert_eq!(
            event_u64(record, "tick"),
            event_u64(event_at_pc(json, pc), "lsq_data_response_tick")
        );
    }
}

fn assert_sc_hierarchy_request(json: &Value) {
    assert_resource_counter(json, "transport.data.activity", 3);
    for (suffix, expected) in [
        ("cache.data.l1.activity", 3),
        ("cache.data.l1.cpu_responses", 3),
        ("dram.writes", 2),
    ] {
        assert_resource_counter(json, suffix, expected);
    }
    assert_eq!(json_u64(json, "/memory_resources/dram/write_bytes"), 16);
    assert_json_stat(
        json,
        "sim.memory.resources.dram.write_bytes",
        "Byte",
        16,
        "monotonic",
    );

    let sc = event_at_pc(json, SUCCESS_SC_PC);
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("SC hierarchy Memory trace missing: {json}"));
    let sent = memory
        .iter()
        .find(|record| {
            event_str(record, "channel") == "data"
                && event_str(record, "kind") == "request_sent"
                && event_u64(record, "tick") == event_u64(sc, "issue_tick")
        })
        .unwrap_or_else(|| panic!("SC hierarchy request missing: {memory:?}"));
    let request = event_u64(sent, "request");
    let route = event_u64(sent, "route");
    let arrived = memory
        .iter()
        .find(|record| {
            event_str(record, "kind") == "request_arrived"
                && event_u64(record, "request") == request
                && event_u64(record, "route") == route
        })
        .unwrap_or_else(|| panic!("SC hierarchy arrival missing: {memory:?}"));
    let packet = (route << 48) | request;
    let hop = json
        .pointer("/memory_resources/fabric/hop_activities")
        .and_then(Value::as_array)
        .and_then(|hops| hops.iter().find(|hop| event_u64(hop, "packet") == packet))
        .unwrap_or_else(|| panic!("SC hierarchy fabric packet {packet} missing: {json}"));
    assert_eq!(event_u64(hop, "ready_tick"), event_u64(sent, "tick"));
    assert_eq!(event_u64(hop, "arrival_tick"), event_u64(arrived, "tick"));
    assert_eq!(event_u64(hop, "bytes"), 8);
    assert_eq!(event_u64(hop, "virtual_network"), 1);
}

fn assert_local_failure_has_no_data_or_target_request(json: &Value, issue_tick: u64) {
    let address = format!("0x{SC_SOURCE:x}");
    assert!(
        data_trace(json)
            .iter()
            .all(|record| event_str(record, "address") != address),
        "local SC failure emitted a Data request: {:?}",
        data_trace(json)
    );
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("local SC Memory trace missing: {json}"));
    assert!(
        memory.iter().all(|record| {
            record.pointer("/channel").and_then(Value::as_str) != Some("data")
                || record.pointer("/kind").and_then(Value::as_str) != Some("request_sent")
                || record.pointer("/tick").and_then(Value::as_u64) != Some(issue_tick)
        }),
        "local SC failure emitted a target request: {memory:?}"
    );
}

fn assert_local_failure_traffic(json: &Value, result_store_issue_tick: u64) {
    let records = data_trace(json);
    assert_eq!(records.len(), 1, "local failure Data trace: {records:?}");
    assert_eq!(event_str(&records[0], "kind"), "store");
    assert_eq!(event_str(&records[0], "address"), "0x80000088");
    assert_eq!(event_u64(&records[0], "size"), 8);

    let requests = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("local SC Memory trace missing: {json}"))
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        requests.len(),
        1,
        "local failure target requests: {requests:?}"
    );
    assert_eq!(event_u64(requests[0], "tick"), result_store_issue_tick);
}

fn assert_success_writeback_counters(json: &Value, writeback_width: usize) {
    let expected = match writeback_width {
        1 => [2, 2, 1, 1, 2, 1],
        2 => [1, 2, 0, 0, 2, 0],
        _ => panic!("unexpected SC writeback width {writeback_width}"),
    };
    assert_writeback_counters(json, expected);
}

fn assert_writeback_counters(json: &Value, expected: [u64; 6]) {
    let writeback = writeback_port_artifact(json);
    for ((field, _), expected) in WRITEBACK_PORT_STATS.iter().zip(expected) {
        assert_eq!(
            writeback_port_u64(writeback, field),
            expected,
            "unexpected SC writeback {field}: {writeback}"
        );
    }
}

fn assert_sc_operation_counters(json: &Value, failures: u64) {
    assert_eq!(
        json_u64(
            json,
            "/cores/0/o3_runtime/lsq/operation/store_conditional/count"
        ),
        1
    );
    assert_eq!(
        json_u64(
            json,
            "/cores/0/o3_runtime/lsq/operation/store_conditional/store_bytes"
        ),
        8
    );
    assert_eq!(
        json_u64(
            json,
            "/cores/0/o3_runtime/lsq/operation/store_conditional/store_conditional_failures"
        ),
        failures
    );
}

fn wait_for_sc_command(mut command: Command, label: &str) -> std::process::Output {
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to spawn {label}: {error}"));
    crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30))
}

fn unique_sc_output(label: &str) -> std::path::PathBuf {
    let id = RESULT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    temp_output(&format!("o3-store-conditional-result-{label}-{id}"))
}

fn assert_timing_suppression(
    fixture: &StoreConditionalFixture,
    route_delay: u64,
    failed: bool,
    expected_hex: &str,
) {
    let detailed = fixture.run("direct", 1, route_delay, RESULT_MAX_TICK, "detailed");
    let timing = fixture.run("direct", 1, route_delay, RESULT_MAX_TICK, "timing");
    let sc = memory_result_event_at_pc(&detailed, SUCCESS_SC_PC);
    assert_sc_result_event(sc, failed);
    assert!(detailed
        .pointer("/cores/0/o3_runtime/writeback_port")
        .is_some());
    for pointer in ["/cores/0/registers", "/memory"] {
        assert_eq!(
            timing.pointer(pointer),
            detailed.pointer(pointer),
            "{pointer}"
        );
    }
    assert_eq!(memory_dump_hex(&timing, SC_SOURCE), Some(expected_hex));
    assert!(timing
        .pointer("/cores/0/o3_runtime/writeback_port")
        .is_none());
    assert!(timing
        .pointer("/cores/0/o3_runtime/writeback_calendar/entries")
        .is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    for (field, _) in WRITEBACK_PORT_STATS {
        assert_json_stat_absent(&timing, &format!("sim.cpu0.o3.writeback_port.{field}"));
    }
    assert_json_stat_absent(&timing, "sim.cpu0.o3.lsq_operation.store_conditional");
    assert_json_stat_absent(&timing, "sim.cpu0.o3.lsq_store_conditional_failures");
}

fn store_conditional_binary(name: &str, with_reservation: bool) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(data_start, 5, 0x0, 5, 0x13),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        i_type(42, 0, 0x0, 6, 0x13),
        i_type(9, 0, 0x0, 7, 0x13),
        m5op(M5_SWITCH_CPU),
    ];
    words.push(if with_reservation {
        atomic_type(0x02, false, false, 0, 5, 0x3, 0)
    } else {
        i_type(0, 0, 0x0, 0, 0x13)
    });
    words.push(if with_reservation {
        r_type(0x01, 2, 1, 0b100, 3, 0x33)
    } else {
        i_type(0, 0, 0x0, 0, 0x13)
    });
    words.extend([
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),
        s_type(8, 7, 5, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0x5566_7788, 0x1122_3344]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    unique_result_temp_binary(&format!("o3-store-conditional-result-{name}"), &elf)
}
