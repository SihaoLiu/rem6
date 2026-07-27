use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use super::*;

pub(super) const FP_LOAD_PC: &str = "0x8000003c";
pub(super) const FP_LOAD_MUL_PC: &str = "0x80000040";
pub(super) const FP_LOAD_ADD_PC: &str = "0x80000044";
pub(super) const FP_LOAD_STORE_PC: &str = "0x80000048";
pub(super) const FP_LOAD_RESULT_HEX: &str = "00002041";
pub(super) const FP_LOAD_INPUT_ADDRESS: u64 = 0x8000_00c0;
pub(super) const FP_LOAD_RESULT_ADDRESS: u64 = FP_LOAD_INPUT_ADDRESS + 4;

pub(super) const FP_LOAD_COLLISION_SECOND_PC: &str = "0x80000040";
pub(super) const FP_LOAD_COLLISION_MUL_PC: &str = "0x80000044";
pub(super) const FP_LOAD_COLLISION_ADD_PC: &str = "0x80000048";
pub(super) const FP_LOAD_COLLISION_STORE_PC: &str = "0x8000004c";
pub(super) const FP_LOAD_HIERARCHY_DIVIDE_BLOCKER_ZERO_PC: &str = "0x80000010";
pub(super) const FP_LOAD_HIERARCHY_BLOCKER_ONE_PC: &str = "0x80000014";
pub(super) const FP_LOAD_HIERARCHY_BLOCKER_TWO_PC: &str = "0x80000018";
pub(super) const FP_LOAD_HIERARCHY_BLOCKER_THREE_PC: &str = "0x8000001c";
pub(super) const FP_LOAD_HIERARCHY_AUX_PREPEER_PC: &str = "0x80000020";
pub(super) const FP_LOAD_HIERARCHY_GATE_PC: &str = "0x80000024";
pub(super) const FP_LOAD_HIERARCHY_FIXED_PC: &str = "0x80000028";
pub(super) const FP_LOAD_HIERARCHY_LOAD_PC: &str = "0x8000002c";
pub(super) const FP_LOAD_HIERARCHY_MUL_PC: &str = "0x80000030";
pub(super) const FP_LOAD_HIERARCHY_ADD_PC: &str = "0x80000034";
pub(super) const FP_LOAD_HIERARCHY_STORE_PC: &str = "0x80000038";
pub(super) const FP_LOAD_D_RESULT_HEX: &str = "0000000000002440";
pub(super) const FP_LOAD_D_RESULT_ADDRESS: u64 = FP_LOAD_INPUT_ADDRESS + 8;

const FP_LOAD_DATA_OFFSET: usize = 0xc0;
const FIXTURE_CACHE_LINE_BYTES: usize = 16;
const COMPLETED_MAX_TICK: u64 = 2_000;
const DIRECT_S_ROUTE_DELAY: u64 = 16;
const DIRECT_D_COLLISION_ROUTE_DELAY: u64 = 11;
const HIERARCHY_S_COLLISION_ROUTE_DELAY: u64 = 1;
const HIERARCHY_D_COLLISION_ROUTE_DELAY: u64 = 1;
static FP_LOAD_BINARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FpLoadPrecision {
    Single,
    Double,
}

impl FpLoadPrecision {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Single => "flw",
            Self::Double => "fld",
        }
    }

    pub(super) const fn bytes(self) -> u64 {
        match self {
            Self::Single => 4,
            Self::Double => 8,
        }
    }

    pub(super) const fn result_address(self) -> u64 {
        match self {
            Self::Single => FP_LOAD_RESULT_ADDRESS,
            Self::Double => FP_LOAD_D_RESULT_ADDRESS,
        }
    }

    pub(super) const fn result_hex(self) -> &'static str {
        match self {
            Self::Single => FP_LOAD_RESULT_HEX,
            Self::Double => FP_LOAD_D_RESULT_HEX,
        }
    }

    pub(super) const fn zero_hex(self) -> &'static str {
        match self {
            Self::Single => "00000000",
            Self::Double => "0000000000000000",
        }
    }
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

    pub(super) const fn width_two_fld_direct() -> Self {
        Self {
            precision: FpLoadPrecision::Double,
            issue_width: 2,
            writeback_width: 2,
            memory_system: "direct",
            switch_mode: "detailed",
        }
    }

    pub(super) const fn width_four_hierarchy(precision: FpLoadPrecision) -> Self {
        Self {
            precision,
            issue_width: 4,
            writeback_width: 1,
            memory_system: "cache-fabric-dram",
            switch_mode: "detailed",
        }
    }

    pub(super) fn route_delay(self) -> u64 {
        match (
            self.precision,
            self.issue_width,
            self.writeback_width,
            self.memory_system,
            self.switch_mode,
        ) {
            (FpLoadPrecision::Single, 1, 1, "direct", "detailed") => DIRECT_S_ROUTE_DELAY,
            (FpLoadPrecision::Double, 2, 2, "direct", "detailed") => DIRECT_D_COLLISION_ROUTE_DELAY,
            (FpLoadPrecision::Single, 4, 1, "cache-fabric-dram", "detailed") => {
                HIERARCHY_S_COLLISION_ROUTE_DELAY
            }
            (FpLoadPrecision::Double, 4, 1, "cache-fabric-dram", "detailed") => {
                HIERARCHY_D_COLLISION_ROUTE_DELAY
            }
            _ => panic!("unsupported FP load forwarding run: {self:?}"),
        }
    }

    fn scalar_live_window_depth(self) -> usize {
        if self.hierarchy_collision() {
            8
        } else {
            5
        }
    }

    fn scalar_memory_depth(self) -> usize {
        if self.hierarchy_collision() {
            4
        } else {
            1
        }
    }

    fn memory_issue_width(self) -> usize {
        1
    }

    pub(super) const fn hierarchy_collision(self) -> bool {
        self.issue_width == 4 && self.writeback_width == 1
    }

    pub(super) const fn schedules_collision_peer(self) -> bool {
        matches!(self.precision, FpLoadPrecision::Double)
            && self.issue_width == 2
            && self.writeback_width == 2
    }

    pub(super) const fn load_pc(self) -> &'static str {
        if self.hierarchy_collision() {
            FP_LOAD_HIERARCHY_LOAD_PC
        } else {
            FP_LOAD_PC
        }
    }

    pub(super) const fn hierarchy_fixed_pc(self) -> Option<&'static str> {
        if self.hierarchy_collision() {
            Some(FP_LOAD_HIERARCHY_FIXED_PC)
        } else {
            None
        }
    }

    pub(super) const fn peer_pc(self) -> Option<&'static str> {
        if self.schedules_collision_peer() {
            Some(FP_LOAD_COLLISION_SECOND_PC)
        } else {
            None
        }
    }

    pub(super) const fn multiply_pc(self) -> &'static str {
        if self.hierarchy_collision() {
            FP_LOAD_HIERARCHY_MUL_PC
        } else if self.schedules_collision_peer() {
            FP_LOAD_COLLISION_MUL_PC
        } else {
            FP_LOAD_MUL_PC
        }
    }

    pub(super) const fn collision_rows_before_multiply(self) -> u64 {
        if self.hierarchy_collision() {
            0
        } else if self.schedules_collision_peer() {
            1
        } else {
            0
        }
    }

    pub(super) const fn add_pc(self) -> &'static str {
        if self.hierarchy_collision() {
            FP_LOAD_HIERARCHY_ADD_PC
        } else if self.schedules_collision_peer() {
            FP_LOAD_COLLISION_ADD_PC
        } else {
            FP_LOAD_ADD_PC
        }
    }

    pub(super) const fn store_pc(self) -> &'static str {
        if self.hierarchy_collision() {
            FP_LOAD_HIERARCHY_STORE_PC
        } else if self.schedules_collision_peer() {
            FP_LOAD_COLLISION_STORE_PC
        } else {
            FP_LOAD_STORE_PC
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
        let path = fp_load_forwarding_binary(self);
        let output = self
            .command(&path, max_tick, self.route_delay())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "FP load forwarding stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("invalid FP load forwarding stdout JSON: {error}"))
    }

    pub(super) fn command(
        self,
        path: &std::path::Path,
        max_tick: u64,
        route_delay: u64,
    ) -> Command {
        let issue_width = self.issue_width.to_string();
        let memory_issue_width = self.memory_issue_width().to_string();
        let writeback_width = self.writeback_width.to_string();
        let scalar_memory_depth = self.scalar_memory_depth().to_string();
        let scalar_live_window_depth = self.scalar_live_window_depth().to_string();
        let max_tick = max_tick.to_string();
        let route_delay = route_delay.to_string();
        let dump_memory = format!(
            "0x{:x}:{}",
            self.precision.result_address(),
            self.precision.bytes()
        );
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
            scalar_memory_depth.as_str(),
            "--riscv-o3-scalar-live-window-depth",
            scalar_live_window_depth.as_str(),
            "--riscv-o3-issue-width",
            issue_width.as_str(),
            "--riscv-o3-memory-issue-width",
            memory_issue_width.as_str(),
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
        if self.hierarchy_collision() {
            command.args(["--instruction-cache-prefetcher", "tagged-next-line"]);
        }
        command
    }
}

pub(super) fn fp_load_forwarding_binary(run: FpLoadForwardingRun) -> std::path::PathBuf {
    match run.precision {
        FpLoadPrecision::Single => flw_forwarding_binary(run),
        FpLoadPrecision::Double => fld_forwarding_binary(run),
    }
}

fn flw_forwarding_binary(run: FpLoadForwardingRun) -> std::path::PathBuf {
    let mut words = if run.hierarchy_collision() {
        hierarchy_operand_prefix(FpLoadPrecision::Single)
    } else {
        let mut words = super::mixed_compute_fixture::mixed_compute_prefix();
        words[2] = u_type(0x4040_0000, 8, 0x37); // 3.0f bits
        words[3] = fp_r_type(0x78, 0, 8, 0, 2); // fmv.w.x f2, x8
        words[4] = u_type(0x4080_0000, 8, 0x37); // 4.0f bits
        words[5] = fp_r_type(0x78, 0, 8, 0, 3); // fmv.w.x f3, x8
        words[11] = i_type(2, 0, 0, 2, 0x13);
        let auipc_pc = i32::try_from(words.len() * std::mem::size_of::<u32>()).unwrap();
        words.extend([
            u_type(0, 12, 0x17),
            i_type(FP_LOAD_DATA_OFFSET as i32 - auipc_pc, 12, 0, 12, 0x13),
        ]);
        words
    };
    words.push(m5op(M5_SWITCH_CPU));
    if run.hierarchy_collision() {
        append_hierarchy_collision_prefix(&mut words);
    }
    let load_offset = if run.hierarchy_collision() {
        FP_LOAD_DATA_OFFSET as i32
    } else {
        0
    };
    words.push(i_type(load_offset, 12, 0b010, 1, 0x07)); // flw f1: 2.0
    let store_offset = if run.hierarchy_collision() {
        FP_LOAD_DATA_OFFSET as i32 + 4
    } else {
        4
    };
    words.extend([
        fp_r_type(0x08, 2, 1, 0, 4), // fmul.s f4, f1, f2
        fp_r_type(0x00, 3, 4, 0, 5), // fadd.s f5, f4, f3
        float_store_type(store_offset, 5, 12, 0b010),
        csr_read(0x001, 6),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    assert!(words.len() * 4 <= FP_LOAD_DATA_OFFSET);
    while words.len() * 4 < FP_LOAD_DATA_OFFSET {
        words.push(0);
    }
    words.extend([2.0f32.to_bits(), 0]);
    while words.len() * std::mem::size_of::<u32>() < FP_LOAD_DATA_OFFSET + 16 {
        words.push(0);
    }
    words.extend([3.0f32.to_bits(), 4.0f32.to_bits()]);
    pad_program_to_cache_line(&mut words);
    let program = riscv64_program(&words);
    let name = unique_fp_load_binary_name(if run.hierarchy_collision() {
        "o3-fp-load-forwarding-flw-hierarchy-collision"
    } else {
        "o3-fp-load-forwarding-flw-direct-width-one"
    });
    temp_binary(&name, &riscv64_elf(0x8000_0000, 0x8000_0000, &program))
}

fn fld_forwarding_binary(run: FpLoadForwardingRun) -> std::path::PathBuf {
    let mut words = if run.hierarchy_collision() {
        hierarchy_operand_prefix(FpLoadPrecision::Double)
    } else {
        let mut words = hierarchy_operand_prefix(FpLoadPrecision::Double);
        while words.len() < 12 {
            words.push(i_type(0, 0, 0, 0, 0x13));
        }
        let auipc_pc = i32::try_from(words.len() * std::mem::size_of::<u32>()).unwrap();
        words.extend([
            u_type(0, 12, 0x17),
            i_type(FP_LOAD_DATA_OFFSET as i32 - auipc_pc, 12, 0, 12, 0x13),
        ]);
        words
    };
    words.push(m5op(M5_SWITCH_CPU));
    if run.hierarchy_collision() {
        append_hierarchy_collision_prefix(&mut words);
    }
    let load_offset = if run.hierarchy_collision() {
        FP_LOAD_DATA_OFFSET as i32
    } else {
        0
    };
    words.push(i_type(load_offset, 12, 0b011, 1, 0x07)); // fld f1: 2.0
    if !run.hierarchy_collision() {
        words.push(fp_r_type(0x2d, 0, 3, 0, 6)); // fsqrt.d f6, f3: exact peer
    }
    let store_offset = if run.hierarchy_collision() {
        FP_LOAD_DATA_OFFSET as i32 + 8
    } else {
        8
    };
    words.extend([
        fp_r_type(0x09, 2, 1, 0, 4), // fmul.d f4, f1, f2
        fp_r_type(0x01, 3, 4, 0, 5), // fadd.d f5, f4, f3
        float_store_type(store_offset, 5, 12, 0b011),
        csr_read(0x001, 6),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    assert!(words.len() * 4 <= FP_LOAD_DATA_OFFSET);
    while words.len() * 4 < FP_LOAD_DATA_OFFSET {
        words.push(0);
    }
    for bits in [2.0f64.to_bits(), 0_u64, 3.0f64.to_bits(), 4.0f64.to_bits()] {
        words.extend([bits as u32, (bits >> 32) as u32]);
    }
    pad_program_to_cache_line(&mut words);
    let program = riscv64_program(&words);
    let name = unique_fp_load_binary_name(if run.hierarchy_collision() {
        "o3-fp-load-forwarding-fld-hierarchy-collision"
    } else {
        "o3-fp-load-forwarding-fld-direct-width-two"
    });
    temp_binary(&name, &riscv64_elf(0x8000_0000, 0x8000_0000, &program))
}

fn unique_fp_load_binary_name(base: &str) -> String {
    let sequence = FP_LOAD_BINARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{base}-{sequence}")
}

fn hierarchy_operand_prefix(precision: FpLoadPrecision) -> Vec<u32> {
    let (load_funct3, second_offset) = match precision {
        FpLoadPrecision::Single => (0b010, FP_LOAD_DATA_OFFSET as i32 + 20),
        FpLoadPrecision::Double => (0b011, FP_LOAD_DATA_OFFSET as i32 + 24),
    };
    vec![
        u_type(0, 12, 0x17),
        i_type(FP_LOAD_DATA_OFFSET as i32 + 16, 12, load_funct3, 2, 0x07),
        i_type(second_offset, 12, load_funct3, 3, 0x07),
    ]
}

fn append_hierarchy_collision_prefix(words: &mut Vec<u32>) {
    words.extend([
        r_type(0x01, 12, 12, 0b100, 7, 0x33),  // div x7, x12, x12
        r_type(0x01, 12, 12, 0b100, 13, 0x33), // div x13, x12, x12
        r_type(0x01, 12, 12, 0b100, 14, 0x33), // div x14, x12, x12
        r_type(0x01, 12, 12, 0b100, 15, 0x33), // div x15, x12, x12
        r_type(0x01, 12, 12, 0b100, 8, 0x33),  // div x8, x12, x12
        r_type(0x01, 12, 7, 0b000, 16, 0x33),  // mul x16, x7, x12
        r_type(0x01, 12, 8, 0b100, 17, 0x33),  // div x17, x8, x12
    ]);
}

fn pad_program_to_cache_line(words: &mut Vec<u32>) {
    let bytes = words.len() * std::mem::size_of::<u32>();
    let padded_bytes = bytes.next_multiple_of(FIXTURE_CACHE_LINE_BYTES);
    words.resize(padded_bytes / std::mem::size_of::<u32>(), 0);
}
