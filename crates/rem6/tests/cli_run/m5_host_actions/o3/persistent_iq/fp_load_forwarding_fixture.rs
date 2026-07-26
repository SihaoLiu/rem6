use std::process::Command;

use serde_json::Value;

use super::*;

pub(super) const FP_LOAD_PC: &str = "0x8000003c";
pub(super) const FP_LOAD_MUL_PC: &str = "0x80000040";
pub(super) const FP_LOAD_ADD_PC: &str = "0x80000044";
pub(super) const FP_LOAD_STORE_PC: &str = "0x80000048";
pub(super) const FP_LOAD_RESULT_HEX: &str = "00002041";
pub(super) const FP_LOAD_INPUT_ADDRESS: u64 = 0x8000_00c0;
pub(super) const FP_LOAD_RESULT_ADDRESS: u64 = FP_LOAD_INPUT_ADDRESS + 4;

const FP_LOAD_DATA_OFFSET: usize = 0xc0;
const COMPLETED_MAX_TICK: u64 = 2_000;
const ROUTE_DELAY: u64 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FpLoadPrecision {
    Single,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FpLoadForwardingRun {
    pub(super) precision: FpLoadPrecision,
    pub(super) issue_width: usize,
    pub(super) writeback_width: usize,
    pub(super) memory_system: &'static str,
    pub(super) switch_mode: &'static str,
}

impl FpLoadForwardingRun {
    pub(super) const fn width_one_flw_direct() -> Self {
        Self {
            precision: FpLoadPrecision::Single,
            issue_width: 1,
            writeback_width: 1,
            memory_system: "direct",
            switch_mode: "detailed",
        }
    }

    pub(super) fn completed_json(self) -> Value {
        let json = self.run_json(COMPLETED_MAX_TICK);
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("stopped_by_host"),
        );
        assert_eq!(
            json.pointer("/host_actions/stats_dump_count")
                .and_then(Value::as_u64),
            Some(1),
        );
        json
    }

    pub(super) fn bounded_json(self, max_tick: u64) -> Value {
        let json = self.run_json(max_tick);
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("stopped_at_tick_limit"),
        );
        assert_eq!(
            json.pointer("/simulation/final_tick")
                .and_then(Value::as_u64),
            Some(max_tick),
        );
        json
    }

    fn run_json(self, max_tick: u64) -> Value {
        let path = fp_load_forwarding_binary(self.precision);
        let output = self.command(&path, max_tick).output().unwrap();
        assert!(
            output.status.success(),
            "FP load forwarding stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("invalid FP load forwarding stdout JSON: {error}"))
    }

    fn command(self, path: &std::path::Path, max_tick: u64) -> Command {
        let issue_width = self.issue_width.to_string();
        let writeback_width = self.writeback_width.to_string();
        let max_tick = max_tick.to_string();
        let route_delay = ROUTE_DELAY.to_string();
        let dump_memory = format!("0x{FP_LOAD_RESULT_ADDRESS:x}:4");
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
            "1",
            "--riscv-o3-scalar-live-window-depth",
            "5",
            "--riscv-o3-issue-width",
            issue_width.as_str(),
            "--riscv-o3-writeback-width",
            writeback_width.as_str(),
            "--memory-system",
            self.memory_system,
            "--memory-route-delay",
            route_delay.as_str(),
            "--m5-switch-cpu-mode",
            self.switch_mode,
            "--dump-memory",
            dump_memory.as_str(),
        ]);
        command
    }
}

fn fp_load_forwarding_binary(precision: FpLoadPrecision) -> std::path::PathBuf {
    match precision {
        FpLoadPrecision::Single => flw_forwarding_binary(),
    }
}

fn flw_forwarding_binary() -> std::path::PathBuf {
    let mut words = super::mixed_compute_fixture::mixed_compute_prefix();
    words[2] = u_type(0x4040_0000, 8, 0x37); // 3.0f bits
    words[3] = fp_r_type(0x78, 0, 8, 0, 2); // fmv.w.x f2, x8
    words[4] = u_type(0x4080_0000, 8, 0x37); // 4.0f bits
    words[5] = fp_r_type(0x78, 0, 8, 0, 3); // fmv.w.x f3, x8
    words[11] = i_type(2, 0, 0, 2, 0x13);
    words.extend([
        u_type(0, 12, 0x17),           // auipc x12, 0
        i_type(0x90, 12, 0, 12, 0x13), // addi x12, x12, data
        m5op(M5_SWITCH_CPU),
        i_type(0, 12, 0b010, 1, 0x07), // flw f1, 0(x12)
        fp_r_type(0x08, 2, 1, 0, 4),   // fmul.s f4, f1, f2
        fp_r_type(0x00, 3, 4, 0, 5),   // fadd.s f5, f4, f3
        float_store_type(4, 5, 12, 0b010),
        csr_read(0x001, 6),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    assert!(words.len() * 4 <= FP_LOAD_DATA_OFFSET);
    while words.len() * 4 < FP_LOAD_DATA_OFFSET {
        words.push(0);
    }
    words.extend([2.0f32.to_bits(), 0]);
    let program = riscv64_program(&words);
    temp_binary(
        "o3-fp-load-forwarding-flw-direct-width-one",
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}
