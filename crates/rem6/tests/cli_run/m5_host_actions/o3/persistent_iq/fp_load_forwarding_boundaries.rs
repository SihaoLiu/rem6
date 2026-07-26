use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use super::mixed_compute_fixture::queue_event_at_pc;
use super::*;

const BASE_ADDRESS: u64 = 0x8000_0000;
const DATA_ADDRESS: u64 = 0x8000_00c0;
const DATA_OFFSET: usize = 0xc0;
const MAX_TICK: u64 = 1_200;
const MEMORY_ROUTE_DELAY: u64 = 16;

#[derive(Clone, Copy, Debug)]
enum DeniedPrecision {
    Single,
    Double,
}

impl DeniedPrecision {
    const fn label(self) -> &'static str {
        match self {
            Self::Single => "flw",
            Self::Double => "fld",
        }
    }

    const fn bytes(self) -> u64 {
        match self {
            Self::Single => 4,
            Self::Double => 8,
        }
    }

    const fn load_funct3(self) -> u32 {
        match self {
            Self::Single => 0b010,
            Self::Double => 0b011,
        }
    }

    const fn mul_funct7(self) -> u32 {
        match self {
            Self::Single => 0x08,
            Self::Double => 0x09,
        }
    }

    const fn add_funct7(self) -> u32 {
        match self {
            Self::Single => 0x00,
            Self::Double => 0x01,
        }
    }

    const fn load_bits(self) -> u64 {
        match self {
            Self::Single => 2.0_f32.to_bits() as u64,
            Self::Double => 2.0_f64.to_bits(),
        }
    }

    const fn load_pc(self) -> &'static str {
        "0x8000001c"
    }

    const fn consumer_pc(self) -> &'static str {
        "0x80000020"
    }

    const fn result_address(self) -> u64 {
        DATA_ADDRESS + 8
    }
}

#[test]
fn rem6_run_o3_fp_load_forwarding_denied_load_cleans_dependency() {
    for precision in [DeniedPrecision::Single, DeniedPrecision::Double] {
        let path = denied_load_binary(precision);
        assert_unrestricted_consumer_is_resident_before_response(&path, precision);

        let artifact = temp_output(&format!(
            "o3-fp-load-forwarding-denied-{}.json",
            precision.label()
        ));
        let output = denied_load_output(&path, precision, &artifact);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{} denied status: {output:?}",
            precision.label()
        );
        assert!(
            output.stdout.is_empty(),
            "{} denied stdout: {output:?}",
            precision.label()
        );
        assert_denied_load_diagnostic(&String::from_utf8(output.stderr).unwrap(), precision);
        assert!(
            !artifact.exists(),
            "{} denial emitted {}",
            precision.label(),
            artifact.display()
        );
    }
}

fn assert_unrestricted_consumer_is_resident_before_response(
    path: &std::path::Path,
    precision: DeniedPrecision,
) {
    let completed = run_boundary_json(
        path,
        MAX_TICK,
        precision.result_address(),
        precision.bytes(),
        precision.label(),
    );
    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host"),
        "{} unrestricted completion: {completed}",
        precision.label()
    );
    let target = super::mixed_compute::o3_event_at_pc(&completed, precision.load_pc());
    let response_tick = event_u64(target, "lsq_data_response_tick");
    let queued = queue_event_at_pc(&completed, precision.consumer_pc(), "queued");
    assert!(
        event_u64(queued, "service_tick") < response_tick,
        "{} consumer must queue before response: target={target}, queued={queued}",
        precision.label()
    );

    let bounded_tick = response_tick - 1;
    let bounded = run_boundary_json(
        path,
        bounded_tick,
        precision.result_address(),
        precision.bytes(),
        precision.label(),
    );
    assert_eq!(
        bounded
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_at_tick_limit"),
        "{} bounded status: {bounded}",
        precision.label()
    );
    assert_eq!(
        json_u64(&bounded, "/simulation/final_tick"),
        bounded_tick,
        "{} bounded tick",
        precision.label()
    );
    let bounded_events = super::queue_events(&bounded);
    let bounded_queued = bounded_events
        .iter()
        .find(|event| {
            event.pointer("/pc").and_then(Value::as_str) == Some(precision.consumer_pc())
                && event.pointer("/action").and_then(Value::as_str) == Some("queued")
        })
        .unwrap_or_else(|| {
            panic!(
                "{} consumer was not resident before response: {bounded}",
                precision.label()
            )
        });
    let consumer_sequence = event_u64(bounded_queued, "sequence");
    assert!(bounded_events.iter().any(|event| {
        event.pointer("/sequence").and_then(Value::as_u64) == Some(consumer_sequence)
            && event.pointer("/action").and_then(Value::as_str) == Some("retained_dependency")
    }));
    assert!(bounded_events.iter().all(|event| {
        event.pointer("/sequence").and_then(Value::as_u64) != Some(consumer_sequence)
            || event.pointer("/action").and_then(Value::as_str) != Some("selected")
    }));
    assert!(
        json_u64(
            &bounded,
            "/cores/0/o3_runtime/issue/queue/current_occupancy"
        ) > 0,
        "{} consumer must remain sequence-owned before response: {bounded}",
        precision.label()
    );
}

fn denied_load_output(
    path: &std::path::Path,
    precision: DeniedPrecision,
    artifact: &std::path::Path,
) -> std::process::Output {
    let listen = crate::gdb_support::unused_loopback_addr();
    let listen_text = listen.to_string();
    let mut command = boundary_command(
        path,
        MAX_TICK,
        precision.result_address(),
        precision.bytes(),
    );
    command.args([
        "--gdb-listen",
        listen_text.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to spawn denied {}: {error}", precision.label()));
    let mut stream =
        match crate::gdb_support::connect_with_retry(listen.address(), Duration::from_secs(3)) {
            Ok(stream) => stream,
            Err(error) => {
                let output =
                    crate::gdb_support::wait_with_output_timeout(child, Duration::from_secs(1));
                panic!(
                    "failed to connect to denied {} GDB listener: {error}; stderr: {}",
                    precision.label(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    assert_eq!(
        crate::gdb_support::send_gdb_packet(&mut stream, b"?"),
        crate::gdb_support::gdb_response(b"S05")
    );
    for packet in [
        b"P89=3000002000000000".as_slice(),
        b"P8c=3100002000000000".as_slice(),
        b"P88=8f88000000000000".as_slice(),
    ] {
        assert_eq!(
            crate::gdb_support::send_gdb_packet(&mut stream, packet),
            crate::gdb_support::gdb_response(b"OK"),
            "{} PMP setup rejected {}",
            precision.label(),
            String::from_utf8_lossy(packet)
        );
    }
    std::io::Write::write_all(&mut stream, &crate::gdb_support::gdb_packet(b"c")).unwrap();
    crate::gdb_support::read_gdb_ack(&mut stream);
    drop(stream);
    crate::gdb_support::wait_with_output_timeout(child, Duration::from_secs(30))
}

fn assert_denied_load_diagnostic(stderr: &str, precision: DeniedPrecision) {
    let stderr = stderr.strip_suffix('\n').unwrap_or_else(|| {
        panic!(
            "denied {} stderr needs one newline: {stderr:?}",
            precision.label()
        )
    });
    let (display, diagnostic) = stderr.split_once('\n').unwrap_or_else(|| {
        panic!(
            "denied {} stderr needs two lines: {stderr:?}",
            precision.label()
        )
    });
    assert!(
        !diagnostic.contains('\n'),
        "extra denied stderr: {stderr:?}"
    );
    assert_eq!(
        display,
        format!(
            "failed to execute run: CPU 0 action failed: data PMP check for fetch response 7 from agent 0 failed: RISC-V PMP denied Read access at 0x800000c0 with {} byte(s) for Machine mode at entry Some(1)",
            precision.bytes()
        )
    );
    let diagnostic: Value = serde_json::from_str(diagnostic)
        .unwrap_or_else(|error| panic!("denied {} diagnostic JSON: {error}", precision.label()));
    assert_eq!(
        diagnostic.pointer("/schema").and_then(Value::as_str),
        Some("rem6.cli.riscv_data_pmp_failure.v1")
    );
    assert_eq!(json_u64(&diagnostic, "/completed_cpu_data_events"), 0);
    assert_eq!(
        json_u64(&diagnostic, "/data_channel_request_sent_events"),
        0,
        "PMP authorization must fail before a target transport request"
    );
    assert_eq!(json_u64(&diagnostic, "/cores/0/cpu"), 0);
    for field in ["rob_entries", "lsq_entries", "writeback_reservations"] {
        assert_eq!(
            json_u64(&diagnostic, &format!("/cores/0/{field}")),
            0,
            "denied {} sequence-owned {field}: {diagnostic}",
            precision.label()
        );
    }
    assert_eq!(
        diagnostic
            .pointer("/memory_dumps/0/address")
            .and_then(Value::as_str),
        Some("0x800000c8")
    );
    assert_eq!(
        json_u64(&diagnostic, "/memory_dumps/0/bytes"),
        precision.bytes()
    );
    assert_eq!(
        diagnostic
            .pointer("/memory_dumps/0/hex")
            .and_then(Value::as_str),
        Some(match precision {
            DeniedPrecision::Single => "00000000",
            DeniedPrecision::Double => "0000000000000000",
        }),
        "denied load must not publish a dependent result"
    );
    assert!(
        diagnostic
            .pointer("/capture_errors")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "denied {} diagnostic capture: {diagnostic}",
        precision.label()
    );
}

#[test]
fn rem6_run_o3_fp_load_forwarding_class_mismatch_uses_normal_execution() {
    const MOVE_PC: &str = "0x80000020";
    let path = class_mismatch_binary();
    let json = run_boundary_json(&path, MAX_TICK, DATA_ADDRESS + 4, 8, "class-mismatch");
    assert_stopped_by_host(&json, "class mismatch");
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("0900000000000040"),
        "integer x1 must not replace floating-point f1: {json}"
    );
    assert_no_queue_lifecycle_at(&json, &[MOVE_PC], "same-index FP move");
    let integer_load = super::mixed_compute::o3_event_at_pc(&json, "0x8000001c");
    assert_eq!(
        integer_load
            .pointer("/lsq_load_address")
            .and_then(Value::as_str),
        Some("0x800000c0")
    );
}

#[test]
fn rem6_run_o3_fp_load_forwarding_unsupported_fp_shapes_use_normal_execution() {
    const UNSUPPORTED_PCS: [&str; 5] = [
        "0x80000028",
        "0x8000002c",
        "0x80000030",
        "0x80000034",
        "0x80000038",
    ];
    let path = unsupported_fp_binary();
    let json = run_boundary_json(&path, MAX_TICK, DATA_ADDRESS, 16, "unsupported-fp");
    assert_stopped_by_host(&json, "unsupported FP shapes");
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("02014001000000400100000002000000"),
        "conversion/comparison/move/class/dynamic results: {json}"
    );
    assert_no_queue_lifecycle_at(&json, &UNSUPPORTED_PCS, "unsupported scalar FP forms");
    for pc in UNSUPPORTED_PCS {
        let event = super::mixed_compute::o3_event_at_pc(&json, pc);
        assert_eq!(
            event.pointer("/lsq_operation").and_then(Value::as_str),
            Some("none"),
            "unsupported form at {pc} must use normal non-memory execution: {event}"
        );
    }
}

#[test]
fn rem6_run_o3_fp_load_forwarding_vector_load_boundary_uses_normal_execution() {
    const VECTOR_LOAD_PC: &str = "0x80000018";
    let path = vector_load_boundary_binary();
    let json = run_boundary_json(&path, MAX_TICK, DATA_ADDRESS + 16, 8, "vector-load");
    assert_stopped_by_host(&json, "vector load boundary");
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("0300000007000000"),
        "vector load/store architecture: {json}"
    );
    assert_no_queue_lifecycle_at(&json, &[VECTOR_LOAD_PC], "vector load");
}

fn assert_no_queue_lifecycle_at(json: &Value, pcs: &[&str], label: &str) {
    let leaked = super::queue_events(json)
        .iter()
        .filter(|event| {
            event
                .pointer("/pc")
                .and_then(Value::as_str)
                .is_some_and(|pc| pcs.contains(&pc))
        })
        .collect::<Vec<_>>();
    assert!(
        leaked.is_empty(),
        "{label} entered persistent IQ: {leaked:#?}"
    );
}

fn assert_stopped_by_host(json: &Value, label: &str) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host"),
        "{label}: {json}"
    );
    assert_eq!(json_u64(json, "/simulation/stop_code"), 0, "{label}");
}

fn run_boundary_json(
    path: &std::path::Path,
    max_tick: u64,
    dump_address: u64,
    dump_bytes: u64,
    label: &str,
) -> Value {
    let output = boundary_command(path, max_tick, dump_address, dump_bytes)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{label} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid {label} stdout JSON: {error}"))
}

fn boundary_command(
    path: &std::path::Path,
    max_tick: u64,
    dump_address: u64,
    dump_bytes: u64,
) -> Command {
    let max_tick = max_tick.to_string();
    let route_delay = MEMORY_ROUTE_DELAY.to_string();
    let dump_memory = format!("0x{dump_address:x}:{dump_bytes}");
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        max_tick.as_str(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--riscv-o3-scalar-live-window-depth",
        "8",
        "--riscv-o3-issue-width",
        "4",
        "--riscv-o3-memory-issue-width",
        "1",
        "--riscv-o3-writeback-width",
        "1",
        "--memory-system",
        "direct",
        "--memory-route-delay",
        route_delay.as_str(),
        "--m5-switch-cpu-mode",
        "detailed",
        "--dump-memory",
        dump_memory.as_str(),
    ]);
    command
}

fn denied_load_binary(precision: DeniedPrecision) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x4040_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0, 2),
        u_type(0x4080_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0, 3),
        u_type(0, 12, 0x17),
        i_type(DATA_OFFSET as i32 - 16, 12, 0, 12, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(0, 12, precision.load_funct3(), 1, 0x07),
        fp_r_type(precision.mul_funct7(), 2, 1, 0, 4),
        fp_r_type(precision.add_funct7(), 3, 4, 0, 5),
        float_store_type(8, 5, 12, precision.load_funct3()),
        csr_read(0x001, 6),
        m5op(M5_DUMP_STATS),
    ];
    append_host_stop(&mut words);
    pad_words_to(&mut words, DATA_OFFSET);
    let bits = precision.load_bits();
    words.extend([bits as u32, (bits >> 32) as u32, 0, 0]);
    boundary_binary(
        &format!("o3-fp-load-forwarding-denied-{}", precision.label()),
        words,
    )
}

fn class_mismatch_binary() -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x4000_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0, 1),
        u_type(0, 12, 0x17),
        i_type(DATA_OFFSET as i32 - 8, 12, 0, 12, 0x13),
        i_type(0, 0, 0, 0, 0x13),
        i_type(0, 0, 0, 0, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(0, 12, 0b010, 1, 0x03),
        fp_r_type(0x70, 0, 1, 0, 10),
        s_type(4, 1, 12, 0b010),
        s_type(8, 10, 12, 0b010),
        m5op(M5_DUMP_STATS),
    ];
    append_host_stop(&mut words);
    pad_words_to(&mut words, DATA_OFFSET);
    words.extend([9, 0, 0, 0]);
    boundary_binary("o3-fp-load-forwarding-class-mismatch", words)
}

fn unsupported_fp_binary() -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x4000_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0, 1),
        u_type(0x4040_0000, 9, 0x37),
        fp_r_type(0x78, 0, 9, 0, 2),
        u_type(0x3fe0_0000, 18, 0x37),
        fp_r_type(0x78, 0, 18, 0, 3),
        csr_write_immediate(0x002, 2),
        u_type(0, 12, 0x17),
        i_type(DATA_OFFSET as i32 - 28, 12, 0, 12, 0x13),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x60, 0, 1, 0, 10),
        fp_r_type(0x50, 2, 1, 1, 11),
        fp_r_type(0x70, 0, 1, 0, 13),
        fp_r_type(0x70, 0, 1, 1, 14),
        fp_r_type(0x60, 0, 3, 0b111, 15),
        csr_read(0x001, 16),
        csr_read(0x002, 17),
        s_type(0, 10, 12, 0b000),
        s_type(1, 11, 12, 0b000),
        s_type(2, 14, 12, 0b000),
        s_type(3, 15, 12, 0b000),
        s_type(4, 13, 12, 0b010),
        s_type(8, 16, 12, 0b010),
        s_type(12, 17, 12, 0b010),
        m5op(M5_DUMP_STATS),
    ];
    append_host_stop(&mut words);
    pad_words_to(&mut words, DATA_OFFSET);
    words.resize(DATA_OFFSET / 4 + 4, 0);
    boundary_binary("o3-fp-load-forwarding-unsupported-fp", words)
}

fn vector_load_boundary_binary() -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 12, 0x17),
        i_type(DATA_OFFSET as i32, 12, 0, 12, 0x13),
        i_type(16, 12, 0, 13, 0x13),
        i_type(2, 0, 0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        m5op(M5_SWITCH_CPU),
        vector_unit_stride_load_type(true, 0b110, 12, 1),
        vector_unit_stride_store_type(true, 0b110, 13, 1),
        m5op(M5_DUMP_STATS),
    ];
    append_host_stop(&mut words);
    pad_words_to(&mut words, DATA_OFFSET);
    words.extend([3, 7, 0, 0, 0, 0, 0, 0]);
    boundary_binary("o3-fp-load-forwarding-vector-load", words)
}

fn csr_write_immediate(csr: u32, value: u8) -> u32 {
    (csr << 20) | (u32::from(value) << 15) | (0b101 << 12) | 0x73
}

fn pad_words_to(words: &mut Vec<u32>, bytes: usize) {
    assert!(words.len() * 4 <= bytes);
    words.resize(bytes / 4, 0);
}

fn boundary_binary(name: &str, words: Vec<u32>) -> std::path::PathBuf {
    let program = riscv64_program(&words);
    temp_binary(name, &riscv64_elf(BASE_ADDRESS, BASE_ADDRESS, &program))
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 {pointer}: {json}"))
}
