use std::{collections::BTreeMap, process::Command};

use serde_json::Value;

use crate::support::{
    assert_stat, b_type, csr_read, i_type, riscv64_elf, riscv64_program, temp_binary, u_type,
};

const SBI_TIME_EXTENSION: i32 = 0x5449_4d45u32 as i32;
const SBI_TIME_SET_TIMER: i32 = 0;
const RISCV_SBI_ENTRY: u64 = 0x8000_0000;
const RISCV_INTERRUPT_BIT: u64 = 1 << 63;
const RISCV_SUPERVISOR_TIMER_INTERRUPT: u64 = 5;

#[test]
fn rem6_run_pipeline_debug_flag_attributes_interrupt_redirect_cause() {
    let mut words = Vec::new();
    let stvec_auipc_index = words.len();
    words.extend([
        u_type(0, 5, 0x17), // auipc x5, handler
        i_type(0, 5, 0x0, 5, 0x13),
        csr_write(0x105, 5), // stvec
        i_type(1 << 5, 0, 0x0, 5, 0x13),
        csr_write(0x104, 5), // sie.STIE
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x100, 5), // sstatus.SIE
        load_time_extension(17)[0],
        load_time_extension(17)[1],
        i_type(SBI_TIME_SET_TIMER, 0, 0x0, 16, 0x13),
        i_type(128, 0, 0x0, 10, 0x13),
        0x0000_0073, // SBI set_timer
    ]);
    for _ in 0..96 {
        words.push(i_type(1, 8, 0x0, 8, 0x13)); // addi x8, x8, 1
    }
    words.push(b_type(0, 0, 0, 0x0)); // fallback self-loop if the timer does not fire
    let handler_index = words.len();
    words.extend([
        csr_read(0x142, 5), // scause
        csr_read(0x141, 6), // sepc
        i_type(0x5a, 0, 0x0, 7, 0x13),
        0x0010_0073, // ebreak
    ]);
    words[stvec_auipc_index + 1] = i_type(
        ((handler_index - stvec_auipc_index) * 4) as i32,
        5,
        0x0,
        5,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("pipeline-interrupt-redirect-cause", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "512",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--memory-route-delay",
            "5",
            "--riscv-sbi",
            "--riscv-in-order-width",
            "2",
            "--debug-flags",
            "Pipeline",
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
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let interrupt_redirect = trace
        .iter()
        .find(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
        })
        .unwrap_or_else(|| panic!("missing interrupt redirect in trace: {trace:?}"));
    let flushed = record_array(interrupt_redirect, "flushed").len() as u64;
    let mut stage_flushed = BTreeMap::<String, u64>::new();
    for flushed in record_array(interrupt_redirect, "flushed") {
        let stage = flushed
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
        *stage_flushed.entry(stage.to_string()).or_default() += 1;
    }

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("breakpoint")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some(
            format!(
                "0x{:x}",
                RISCV_INTERRUPT_BIT | RISCV_SUPERVISOR_TIMER_INTERRUPT
            )
            .as_str()
        )
    );
    assert_eq!(
        interrupt_redirect
            .get("flush_cause")
            .and_then(Value::as_str),
        if flushed == 0 {
            None
        } else {
            Some("interrupt_redirect")
        },
        "interrupt redirects that squash younger in-flight instructions need an explicit flush cause: {interrupt_redirect:?}"
    );
    assert_eq!(
        interrupt_redirect
            .get("redirect_target")
            .and_then(Value::as_str),
        Some(format!("0x{:x}", RISCV_SBI_ENTRY + handler_index as u64 * 4).as_str())
    );
    assert_eq!(json_record_u64(interrupt_redirect, "branch_predictions"), 0);
    assert_eq!(
        json_record_u64(interrupt_redirect, "branch_prediction_flushed"),
        0
    );
    assert_eq!(
        json_path_u64(&json, "/cores/0/in_order_pipeline/interrupt_redirects"),
        1
    );
    let redirects = json_path_u64(&json, "/cores/0/in_order_pipeline/redirects");
    let branch_prediction_redirects = json_path_u64(
        &json,
        "/cores/0/in_order_pipeline/branch_prediction_redirects",
    );
    let interrupt_redirects =
        json_path_u64(&json, "/cores/0/in_order_pipeline/interrupt_redirects");
    let trap_redirects = json_path_u64(&json, "/cores/0/in_order_pipeline/trap_redirects");
    assert_eq!(branch_prediction_redirects, 0, "{stdout}");
    assert_eq!(
        branch_prediction_redirects + interrupt_redirects + trap_redirects,
        redirects,
        "redirect counters should partition by cause: {stdout}"
    );
    assert_eq!(
        json_path_u64(
            &json,
            "/cores/0/in_order_pipeline/interrupt_redirect_flushes"
        ),
        flushed
    );
    let interrupt_redirect_flush_cycles = json_path_u64(
        &json,
        "/cores/0/in_order_pipeline/interrupt_redirect_flush_cycles",
    );
    if flushed == 0 {
        assert_eq!(interrupt_redirect_flush_cycles, 0, "{stdout}");
        for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
            assert_eq!(
                json_path_u64(
                    &json,
                    &format!(
                        "/cores/0/in_order_pipeline/redirect_cause/interrupt_redirect/stage_flushed/{stage}"
                    ),
                ),
                0,
                "{stdout}"
            );
        }
    } else {
        assert!(interrupt_redirect_flush_cycles > 0, "{stdout}");
    }
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.interrupt_redirects",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.interrupt_redirect_flushes",
        "Count",
        flushed,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.interrupt_redirect_flush_cycles",
        "Cycle",
        interrupt_redirect_flush_cycles,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.redirect_cause.interrupt_redirect.records",
        "Count",
        1,
        "monotonic",
    );
    if flushed > 0 {
        assert_stat(
            &stdout,
            "sim.debug.pipeline_trace.flush_cause.interrupt_redirect.flushed",
            "Count",
            flushed,
            "monotonic",
        );
    }
    for (stage, flushed) in stage_flushed {
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.redirect_cause.interrupt_redirect.stage.{stage}.flushed"),
            "Count",
            flushed,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.flush_cause.interrupt_redirect.stage.{stage}.flushed"
            ),
            "Count",
            flushed,
            "monotonic",
        );
    }
}

fn load_time_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_TIME_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_TIME_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn csr_write(csr: u32, rs1: u8) -> u32 {
    (csr << 20) | (u32::from(rs1) << 15) | (0x1 << 12) | 0x73
}

fn record_array<'a>(record: &'a Value, key: &str) -> &'a [Value] {
    record
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing array field {key}: {record}"))
}

fn json_record_u64(record: &Value, key: &str) -> u64 {
    record
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 field {key}: {record}"))
}

fn json_path_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 at {pointer}: {json}"))
}
