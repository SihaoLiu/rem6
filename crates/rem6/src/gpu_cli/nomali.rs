use std::path::{Path, PathBuf};

use crate::formatting::json_escape;

use super::Rem6GpuRunExecutionSummary;

const NOMALI_API_VERSION: u32 = 0;
const NOMALI_REGISTER_WINDOW_BYTES: u64 = 0x4000;
const NOMALI_GPU_TYPE: &str = "T760";
const NOMALI_GPU_INT: u32 = 0;
const NOMALI_JOB_INT: u32 = 1;
const NOMALI_MMU_INT: u32 = 2;
const NOMALI_REGISTER_WINDOW_WORDS: usize = (NOMALI_REGISTER_WINDOW_BYTES as usize) / 4;

const GPU_ID: u32 = 0x000;
const L2_FEATURES: u32 = 0x004;
const TILER_FEATURES: u32 = 0x00c;
const MEM_FEATURES: u32 = 0x010;
const MMU_FEATURES: u32 = 0x014;
const AS_PRESENT: u32 = 0x018;
const JS_PRESENT: u32 = 0x01c;
const GPU_IRQ_RAWSTAT: u32 = 0x020;
const GPU_IRQ_CLEAR: u32 = 0x024;
const GPU_IRQ_MASK: u32 = 0x028;
const GPU_IRQ_STATUS: u32 = 0x02c;
const GPU_COMMAND: u32 = 0x030;
const THREAD_MAX_THREADS: u32 = 0x0a0;
const THREAD_MAX_WORKGROUP_SIZE: u32 = 0x0a4;
const THREAD_MAX_BARRIER_SIZE: u32 = 0x0a8;
const THREAD_FEATURES: u32 = 0x0ac;
const TEXTURE_FEATURES_0: u32 = 0x0b0;
const TEXTURE_FEATURES_1: u32 = 0x0b4;
const TEXTURE_FEATURES_2: u32 = 0x0b8;
const JS0_FEATURES: u32 = 0x0c0;
const JS1_FEATURES: u32 = 0x0c4;
const JS2_FEATURES: u32 = 0x0c8;
const SHADER_PRESENT_LO: u32 = 0x100;
const SHADER_PRESENT_HI: u32 = 0x104;
const TILER_PRESENT_LO: u32 = 0x110;
const TILER_PRESENT_HI: u32 = 0x114;
const L2_PRESENT_LO: u32 = 0x120;
const L2_PRESENT_HI: u32 = 0x124;

const RESET_COMPLETED: u32 = 1 << 8;
const GPU_COMMAND_SOFT_RESET: u32 = 0x01;
const GPU_COMMAND_HARD_RESET: u32 = 0x02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NoMaliRegister {
    name: &'static str,
    offset: u32,
    value: u32,
}

const NOMALI_T760_RESET_REGISTERS: &[NoMaliRegister] = &[
    NoMaliRegister {
        name: "gpu_id",
        offset: GPU_ID,
        value: 0x0750_0000,
    },
    NoMaliRegister {
        name: "l2_features",
        offset: L2_FEATURES,
        value: 0x0713_0206,
    },
    NoMaliRegister {
        name: "tiler_features",
        offset: TILER_FEATURES,
        value: 0x0000_0809,
    },
    NoMaliRegister {
        name: "mem_features",
        offset: MEM_FEATURES,
        value: 0x0000_0001,
    },
    NoMaliRegister {
        name: "mmu_features",
        offset: MMU_FEATURES,
        value: 0x0000_2830,
    },
    NoMaliRegister {
        name: "as_present",
        offset: AS_PRESENT,
        value: 0x0000_00ff,
    },
    NoMaliRegister {
        name: "js_present",
        offset: JS_PRESENT,
        value: 0x0000_0007,
    },
    NoMaliRegister {
        name: "thread_max_threads",
        offset: THREAD_MAX_THREADS,
        value: 0x0000_0100,
    },
    NoMaliRegister {
        name: "thread_max_workgroup_size",
        offset: THREAD_MAX_WORKGROUP_SIZE,
        value: 0x0000_0100,
    },
    NoMaliRegister {
        name: "thread_max_barrier_size",
        offset: THREAD_MAX_BARRIER_SIZE,
        value: 0x0000_0100,
    },
    NoMaliRegister {
        name: "thread_features",
        offset: THREAD_FEATURES,
        value: 0x0a04_0400,
    },
    NoMaliRegister {
        name: "texture_features_0",
        offset: TEXTURE_FEATURES_0,
        value: 0x00fe_001e,
    },
    NoMaliRegister {
        name: "texture_features_1",
        offset: TEXTURE_FEATURES_1,
        value: 0x0000_ffff,
    },
    NoMaliRegister {
        name: "texture_features_2",
        offset: TEXTURE_FEATURES_2,
        value: 0x9f81_ffff,
    },
    NoMaliRegister {
        name: "js0_features",
        offset: JS0_FEATURES,
        value: 0x0000_020e,
    },
    NoMaliRegister {
        name: "js1_features",
        offset: JS1_FEATURES,
        value: 0x0000_01fe,
    },
    NoMaliRegister {
        name: "js2_features",
        offset: JS2_FEATURES,
        value: 0x0000_007e,
    },
    NoMaliRegister {
        name: "shader_present_lo",
        offset: SHADER_PRESENT_LO,
        value: 0x0000_000f,
    },
    NoMaliRegister {
        name: "shader_present_hi",
        offset: SHADER_PRESENT_HI,
        value: 0x0000_0000,
    },
    NoMaliRegister {
        name: "tiler_present_lo",
        offset: TILER_PRESENT_LO,
        value: 0x0000_0001,
    },
    NoMaliRegister {
        name: "tiler_present_hi",
        offset: TILER_PRESENT_HI,
        value: 0x0000_0000,
    },
    NoMaliRegister {
        name: "l2_present_lo",
        offset: L2_PRESENT_LO,
        value: 0x0000_0001,
    },
    NoMaliRegister {
        name: "l2_present_hi",
        offset: L2_PRESENT_HI,
        value: 0x0000_0000,
    },
];

const NOMALI_OBSERVED_PIO_READS: &[(&str, u32)] = &[
    ("gpu_id", GPU_ID),
    ("l2_features", L2_FEATURES),
    ("tiler_features", TILER_FEATURES),
    ("thread_features", THREAD_FEATURES),
    ("shader_present_lo", SHADER_PRESENT_LO),
    ("shader_present_hi", SHADER_PRESENT_HI),
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliCommandWrite {
    name: &'static str,
    offset: u32,
    value: u32,
    command: &'static str,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliT760RegisterFile {
    registers: Vec<u32>,
    reset_count: u64,
    command_writes: Vec<NoMaliCommandWrite>,
}

impl NoMaliT760RegisterFile {
    fn new() -> Self {
        Self {
            registers: vec![0; NOMALI_REGISTER_WINDOW_WORDS],
            reset_count: 0,
            command_writes: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.registers.fill(0);
        for register in NOMALI_T760_RESET_REGISTERS {
            self.write_raw(register.offset, register.value);
        }
        self.reset_count += 1;
    }

    fn read_raw(&self, offset: u32) -> u32 {
        self.registers[(offset / 4) as usize]
    }

    fn write_raw(&mut self, offset: u32, value: u32) {
        self.registers[(offset / 4) as usize] = value;
    }

    fn write_reg(&mut self, offset: u32, value: u32) {
        match offset {
            GPU_IRQ_RAWSTAT => self.raise_interrupt(value),
            GPU_IRQ_CLEAR => self.clear_interrupt(value),
            GPU_IRQ_MASK => self.write_raw(offset, value),
            GPU_IRQ_STATUS => {}
            GPU_COMMAND => self.gpu_command(value),
            _ => self.write_raw(offset, value),
        }
    }

    fn gpu_command(&mut self, value: u32) {
        match value {
            GPU_COMMAND_SOFT_RESET => {
                self.command_writes.push(NoMaliCommandWrite {
                    name: "gpu_command",
                    offset: GPU_COMMAND,
                    value,
                    command: "soft_reset",
                    effect: "reset_completed_interrupt",
                });
                self.reset();
                self.raise_interrupt(RESET_COMPLETED);
            }
            GPU_COMMAND_HARD_RESET => {
                self.command_writes.push(NoMaliCommandWrite {
                    name: "gpu_command",
                    offset: GPU_COMMAND,
                    value,
                    command: "hard_reset",
                    effect: "reset_completed_interrupt",
                });
                self.reset();
                self.raise_interrupt(RESET_COMPLETED);
            }
            _ => {
                self.command_writes.push(NoMaliCommandWrite {
                    name: "gpu_command",
                    offset: GPU_COMMAND,
                    value,
                    command: "unsupported",
                    effect: "ignored",
                });
            }
        }
    }

    fn raise_interrupt(&mut self, flags: u32) {
        self.write_raw(GPU_IRQ_RAWSTAT, self.read_raw(GPU_IRQ_RAWSTAT) | flags);
    }

    fn clear_interrupt(&mut self, flags: u32) {
        self.write_raw(GPU_IRQ_RAWSTAT, self.read_raw(GPU_IRQ_RAWSTAT) & !flags);
    }

    fn irq_status(&self) -> u32 {
        self.read_raw(GPU_IRQ_RAWSTAT) & self.read_raw(GPU_IRQ_MASK)
    }

    fn irq_asserted(&self) -> bool {
        self.irq_status() != 0
    }

    fn checkpoint_word_count(&self) -> usize {
        self.registers.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6GpuNoMaliAdapterArtifact {
    output: PathBuf,
    contents: String,
}

impl Rem6GpuNoMaliAdapterArtifact {
    pub(crate) fn output(&self) -> &Path {
        &self.output
    }

    pub(crate) fn contents(&self) -> &str {
        &self.contents
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"rem6.nomali.gpu-adapter.v1\",\"artifact\":\"{}\"}}",
            json_escape(&self.output.display().to_string())
        )
    }
}

pub(crate) fn gpu_run_nomali_adapter_artifact(
    output: PathBuf,
    execution: &Rem6GpuRunExecutionSummary,
) -> Rem6GpuNoMaliAdapterArtifact {
    let global_memory_reads = execution
        .compute_unit_activity()
        .iter()
        .map(|activity| activity.global_memory_reads())
        .sum::<u64>();
    let global_memory_writes = execution
        .compute_unit_activity()
        .iter()
        .map(|activity| activity.global_memory_writes())
        .sum::<u64>();
    let compute_units = execution.compute_unit_activity().len();
    let compute_unit_activity = execution
        .compute_unit_activity()
        .iter()
        .map(nomali_compute_unit_activity_json)
        .collect::<Vec<_>>()
        .join(",");
    let mut pio = NoMaliT760RegisterFile::new();
    pio.reset();
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_SOFT_RESET);
    pio.write_reg(GPU_IRQ_MASK, RESET_COMPLETED);
    let contents = format!(
        "{{\"schema\":\"rem6.nomali.gpu-adapter.v1\",\"source_schema\":\"rem6.cli.gpu-run.v1\",\"scope\":\"gpu-run-execution-summary-adapter\",\"gpu\":{},\"interface\":{},\"pio\":{},\"execution\":{{\"status\":\"completed\",\"final_tick\":{},\"compute_units\":{},\"workgroup_completions\":{},\"coalesced_memory_accesses\":{},\"global_memory_reads\":{},\"global_memory_writes\":{},\"memory_read_callback_observations\":{},\"memory_write_callback_observations\":{},\"job_event_observations\":{},\"compute_unit_activity\":[{}]}}}}\n",
        nomali_gpu_json(&pio),
        nomali_interface_json(),
        nomali_pio_json(&pio),
        execution.final_tick(),
        compute_units,
        execution.workgroup_completions(),
        execution.coalesced_memory_accesses(),
        global_memory_reads,
        global_memory_writes,
        global_memory_reads,
        global_memory_writes,
        execution.workgroup_completions(),
        compute_unit_activity,
    );
    Rem6GpuNoMaliAdapterArtifact { output, contents }
}

fn nomali_gpu_json(pio: &NoMaliT760RegisterFile) -> String {
    format!(
        "{{\"type\":\"{}\",\"api_version\":{},\"version\":{{\"major\":0,\"minor\":0,\"status\":0}},\"register_window_bytes\":{},\"config_registers\":{}}}",
        NOMALI_GPU_TYPE,
        NOMALI_API_VERSION,
        NOMALI_REGISTER_WINDOW_BYTES,
        nomali_t760_config_registers_json(pio),
    )
}

fn nomali_interface_json() -> String {
    format!(
        "{{\"callbacks\":[\"interrupt\",\"memread\",\"memwrite\",\"reset\"],\"interrupts\":{{\"gpu\":{{\"nomali_int\":{}}},\"job\":{{\"nomali_int\":{}}},\"mmu\":{{\"nomali_int\":{}}}}}}}",
        NOMALI_GPU_INT, NOMALI_JOB_INT, NOMALI_MMU_INT,
    )
}

fn nomali_t760_config_registers_json(pio: &NoMaliT760RegisterFile) -> String {
    format!(
        "{{\"gpu_id\":\"{}\",\"l2_features\":\"{}\",\"tiler_features\":\"{}\",\"mem_features\":\"{}\",\"mmu_features\":\"{}\",\"as_present\":\"{}\",\"js_present\":\"{}\",\"thread_max_threads\":\"{}\",\"thread_max_workgroup_size\":\"{}\",\"thread_max_barrier_size\":\"{}\",\"thread_features\":\"{}\",\"texture_features\":[\"{}\",\"{}\",\"{}\"],\"js_features\":[\"{}\",\"{}\",\"{}\"],\"shader_present\":\"{}\",\"tiler_present\":\"{}\",\"l2_present\":\"{}\"}}",
        register_hex(pio.read_raw(GPU_ID)),
        register_hex(pio.read_raw(L2_FEATURES)),
        register_hex(pio.read_raw(TILER_FEATURES)),
        register_hex(pio.read_raw(MEM_FEATURES)),
        register_hex(pio.read_raw(MMU_FEATURES)),
        register_hex(pio.read_raw(AS_PRESENT)),
        register_hex(pio.read_raw(JS_PRESENT)),
        register_hex(pio.read_raw(THREAD_MAX_THREADS)),
        register_hex(pio.read_raw(THREAD_MAX_WORKGROUP_SIZE)),
        register_hex(pio.read_raw(THREAD_MAX_BARRIER_SIZE)),
        register_hex(pio.read_raw(THREAD_FEATURES)),
        register_hex(pio.read_raw(TEXTURE_FEATURES_0)),
        register_hex(pio.read_raw(TEXTURE_FEATURES_1)),
        register_hex(pio.read_raw(TEXTURE_FEATURES_2)),
        register_hex(pio.read_raw(JS0_FEATURES)),
        register_hex(pio.read_raw(JS1_FEATURES)),
        register_hex(pio.read_raw(JS2_FEATURES)),
        register_hex(pio.read_raw(SHADER_PRESENT_LO)),
        register_hex(pio.read_raw(TILER_PRESENT_LO)),
        register_hex(pio.read_raw(L2_PRESENT_LO)),
    )
}

fn nomali_pio_json(pio: &NoMaliT760RegisterFile) -> String {
    let command_writes = pio
        .command_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"name\":\"{}\",\"offset\":\"0x{:03x}\",\"value\":\"{}\",\"command\":\"{}\",\"effect\":\"{}\"}}",
                write.name,
                write.offset,
                register_hex(write.value),
                write.command,
                write.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let register_reads = NOMALI_OBSERVED_PIO_READS
        .iter()
        .map(|(name, offset)| {
            format!(
                "{{\"name\":\"{}\",\"offset\":\"0x{:03x}\",\"value\":\"0x{:08x}\"}}",
                name,
                offset,
                pio.read_raw(*offset),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"register_window_bytes\":{},\"reset_count\":{},\"command_writes\":[{}],\"irq\":{{\"rawstat\":\"{}\",\"mask\":\"{}\",\"status\":\"{}\",\"asserted\":{}}},\"checkpoint\":{{\"word_count\":{}}},\"register_reads\":[{}]}}",
        NOMALI_REGISTER_WINDOW_BYTES,
        pio.reset_count,
        command_writes,
        register_hex(pio.read_raw(GPU_IRQ_RAWSTAT)),
        register_hex(pio.read_raw(GPU_IRQ_MASK)),
        register_hex(pio.irq_status()),
        pio.irq_asserted(),
        pio.checkpoint_word_count(),
        register_reads,
    )
}

fn register_hex(value: u32) -> String {
    format!("0x{value:08x}")
}

fn nomali_compute_unit_activity_json(activity: &super::Rem6GpuComputeUnitActivity) -> String {
    format!(
        "{{\"compute_unit\":{},\"workgroup_completions\":{},\"busy_cycles\":{},\"coalesced_memory_accesses\":{},\"global_memory_reads\":{},\"global_memory_writes\":{},\"first_started_at\":{},\"last_completed_at\":{}}}",
        activity.compute_unit(),
        activity.workgroup_completions(),
        activity.busy_cycles(),
        activity.coalesced_memory_accesses(),
        activity.global_memory_reads(),
        activity.global_memory_writes(),
        optional_tick_json(activity.first_started_at()),
        optional_tick_json(activity.last_completed_at()),
    )
}

fn optional_tick_json(tick: Option<u64>) -> String {
    tick.map(|tick| tick.to_string())
        .unwrap_or_else(|| "null".to_string())
}
