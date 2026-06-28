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
const SHADER_READY_LO: u32 = 0x140;
const SHADER_PWRON_LO: u32 = 0x180;
const SHADER_PWROFF_LO: u32 = 0x1c0;
const JOB_IRQ_RAWSTAT: u32 = 0x1000;
const JOB_IRQ_CLEAR: u32 = 0x1004;
const JOB_IRQ_MASK: u32 = 0x1008;
const JOB_IRQ_STATUS: u32 = 0x100c;
const MMU_IRQ_RAWSTAT: u32 = 0x2000;
const MMU_IRQ_CLEAR: u32 = 0x2004;
const MMU_IRQ_MASK: u32 = 0x2008;
const MMU_IRQ_STATUS: u32 = 0x200c;

const RESET_COMPLETED: u32 = 1 << 8;
const POWER_CHANGED_SINGLE: u32 = 1 << 9;
const POWER_CHANGED_ALL: u32 = 1 << 10;
const JOB_SLOT0_COMPLETED: u32 = 1 << 0;
const MMU_PAGE_FAULT_AS0: u32 = 1 << 0;
const MMU_BUS_ERROR_AS0: u32 = 1 << 16;
const GPU_COMMAND_SOFT_RESET: u32 = 0x01;
const GPU_COMMAND_HARD_RESET: u32 = 0x02;
const GPU_COMMAND_UNSUPPORTED_PROBE: u32 = 0xdead_dead;
const NOMALI_REGISTER_FAULT_MISALIGNED_READ_OFFSET: u32 = 0x003;
const NOMALI_REGISTER_FAULT_OUT_OF_RANGE_WRITE_OFFSET: u32 = NOMALI_REGISTER_WINDOW_BYTES as u32;
const NOMALI_REGISTER_FAULT_WRITE_VALUE: u32 = 0x1234_5678;

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
struct NoMaliIrqWrite {
    name: &'static str,
    offset: u32,
    value: u32,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliPowerWrite {
    name: &'static str,
    offset: u32,
    value: u32,
    ready_register: &'static str,
    ready_offset: u32,
    ready_value: u32,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliIrqSnapshot {
    name: &'static str,
    rawstat: u32,
    mask: u32,
    status: u32,
    asserted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliInterruptBlockSnapshot {
    block: &'static str,
    nomali_int: u32,
    name: &'static str,
    rawstat_offset: u32,
    mask_offset: u32,
    status_offset: u32,
    rawstat: u32,
    mask: u32,
    status: u32,
    asserted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliRegisterFault {
    operation: &'static str,
    offset: u32,
    value: Option<u32>,
    reason: &'static str,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliT760RegisterFile {
    registers: Vec<u32>,
    reset_count: u64,
    command_writes: Vec<NoMaliCommandWrite>,
    irq_writes: Vec<NoMaliIrqWrite>,
    power_writes: Vec<NoMaliPowerWrite>,
    irq_snapshots: Vec<NoMaliIrqSnapshot>,
    interrupt_block_snapshots: Vec<NoMaliInterruptBlockSnapshot>,
    register_faults: Vec<NoMaliRegisterFault>,
}

impl NoMaliT760RegisterFile {
    fn new() -> Self {
        Self {
            registers: vec![0; NOMALI_REGISTER_WINDOW_WORDS],
            reset_count: 0,
            command_writes: Vec::new(),
            irq_writes: Vec::new(),
            power_writes: Vec::new(),
            irq_snapshots: Vec::new(),
            interrupt_block_snapshots: Vec::new(),
            register_faults: Vec::new(),
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

    fn read_reg(&mut self, offset: u32) -> Option<u32> {
        let index = self.checked_register_index("read", offset, None)?;
        Some(self.registers[index])
    }

    fn write_raw(&mut self, offset: u32, value: u32) {
        self.registers[(offset / 4) as usize] = value;
    }

    fn write_reg(&mut self, offset: u32, value: u32) {
        if self
            .checked_register_index("write", offset, Some(value))
            .is_none()
        {
            return;
        }
        match offset {
            GPU_IRQ_RAWSTAT => self.raise_interrupt(value),
            GPU_IRQ_CLEAR => {
                self.irq_writes.push(NoMaliIrqWrite {
                    name: "gpu_irq_clear",
                    offset,
                    value,
                    effect: irq_clear_effect(value),
                });
                self.clear_interrupt(value);
            }
            GPU_IRQ_MASK => self.write_raw(offset, value),
            GPU_IRQ_STATUS => {}
            JOB_IRQ_RAWSTAT => self.raise_interrupt_at(JOB_IRQ_RAWSTAT, value),
            JOB_IRQ_CLEAR => {
                self.irq_writes.push(NoMaliIrqWrite {
                    name: "job_irq_clear",
                    offset,
                    value,
                    effect: job_irq_clear_effect(value),
                });
                self.clear_interrupt_at(JOB_IRQ_RAWSTAT, value);
            }
            JOB_IRQ_MASK => self.write_raw(offset, value),
            JOB_IRQ_STATUS => {}
            MMU_IRQ_RAWSTAT => self.raise_interrupt_at(MMU_IRQ_RAWSTAT, value),
            MMU_IRQ_CLEAR => {
                self.irq_writes.push(NoMaliIrqWrite {
                    name: "mmu_irq_clear",
                    offset,
                    value,
                    effect: mmu_irq_clear_effect(value),
                });
                self.clear_interrupt_at(MMU_IRQ_RAWSTAT, value);
            }
            MMU_IRQ_MASK => self.write_raw(offset, value),
            MMU_IRQ_STATUS => {}
            GPU_COMMAND => self.gpu_command(value),
            SHADER_PWRON_LO => self.shader_power_on(value),
            SHADER_PWROFF_LO => self.shader_power_off(value),
            _ => self.write_raw(offset, value),
        }
    }

    fn checked_register_index(
        &mut self,
        operation: &'static str,
        offset: u32,
        value: Option<u32>,
    ) -> Option<usize> {
        let reason = if offset % 4 != 0 {
            Some("misaligned_offset")
        } else if u64::from(offset) >= NOMALI_REGISTER_WINDOW_BYTES {
            Some("offset_out_of_range")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.register_faults.push(NoMaliRegisterFault {
                operation,
                offset,
                value,
                reason,
                effect: "fault_recorded",
            });
            return None;
        }
        Some((offset / 4) as usize)
    }

    fn probe_register_faults(&mut self) {
        let _ = self.read_reg(NOMALI_REGISTER_FAULT_MISALIGNED_READ_OFFSET);
        self.write_reg(
            NOMALI_REGISTER_FAULT_OUT_OF_RANGE_WRITE_OFFSET,
            NOMALI_REGISTER_FAULT_WRITE_VALUE,
        );
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

    fn shader_power_on(&mut self, value: u32) {
        let ready = self.read_raw(SHADER_READY_LO) | (value & self.read_raw(SHADER_PRESENT_LO));
        self.write_raw(SHADER_READY_LO, ready);
        self.raise_power_changed_interrupt();
        self.power_writes.push(NoMaliPowerWrite {
            name: "shader_pwron_lo",
            offset: SHADER_PWRON_LO,
            value,
            ready_register: "shader_ready_lo",
            ready_offset: SHADER_READY_LO,
            ready_value: ready,
            effect: "power_changed_interrupt",
        });
    }

    fn shader_power_off(&mut self, value: u32) {
        let ready = self.read_raw(SHADER_READY_LO) & !value;
        self.write_raw(SHADER_READY_LO, ready);
        self.raise_power_changed_interrupt();
        self.power_writes.push(NoMaliPowerWrite {
            name: "shader_pwroff_lo",
            offset: SHADER_PWROFF_LO,
            value,
            ready_register: "shader_ready_lo",
            ready_offset: SHADER_READY_LO,
            ready_value: ready,
            effect: "power_changed_interrupt",
        });
    }

    fn raise_power_changed_interrupt(&mut self) {
        self.raise_interrupt(POWER_CHANGED_SINGLE | POWER_CHANGED_ALL);
    }

    fn raise_interrupt(&mut self, flags: u32) {
        self.raise_interrupt_at(GPU_IRQ_RAWSTAT, flags);
    }

    fn clear_interrupt(&mut self, flags: u32) {
        self.clear_interrupt_at(GPU_IRQ_RAWSTAT, flags);
    }

    fn raise_interrupt_at(&mut self, rawstat_offset: u32, flags: u32) {
        self.write_raw(rawstat_offset, self.read_raw(rawstat_offset) | flags);
    }

    fn clear_interrupt_at(&mut self, rawstat_offset: u32, flags: u32) {
        self.write_raw(rawstat_offset, self.read_raw(rawstat_offset) & !flags);
    }

    fn irq_status(&self) -> u32 {
        self.read_raw(GPU_IRQ_RAWSTAT) & self.read_raw(GPU_IRQ_MASK)
    }

    fn irq_asserted(&self) -> bool {
        self.irq_status() != 0
    }

    fn record_irq_snapshot(&mut self, name: &'static str) {
        self.irq_snapshots.push(NoMaliIrqSnapshot {
            name,
            rawstat: self.read_raw(GPU_IRQ_RAWSTAT),
            mask: self.read_raw(GPU_IRQ_MASK),
            status: self.irq_status(),
            asserted: self.irq_asserted(),
        });
    }

    fn record_interrupt_block_snapshot(
        &mut self,
        block: &'static str,
        nomali_int: u32,
        name: &'static str,
        rawstat_offset: u32,
        mask_offset: u32,
        status_offset: u32,
    ) {
        let rawstat = self.read_raw(rawstat_offset);
        let mask = self.read_raw(mask_offset);
        let status = rawstat & mask;
        self.interrupt_block_snapshots
            .push(NoMaliInterruptBlockSnapshot {
                block,
                nomali_int,
                name,
                rawstat_offset,
                mask_offset,
                status_offset,
                rawstat,
                mask,
                status,
                asserted: status != 0,
            });
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
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_HARD_RESET);
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_UNSUPPORTED_PROBE);
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_SOFT_RESET);
    pio.write_reg(GPU_IRQ_MASK, RESET_COMPLETED);
    pio.record_irq_snapshot("after_soft_reset_masked");
    pio.write_reg(GPU_IRQ_CLEAR, RESET_COMPLETED);
    pio.record_irq_snapshot("after_irq_clear");
    pio.write_reg(
        GPU_IRQ_MASK,
        RESET_COMPLETED | POWER_CHANGED_SINGLE | POWER_CHANGED_ALL,
    );
    pio.write_reg(SHADER_PWRON_LO, pio.read_raw(SHADER_PRESENT_LO));
    pio.record_irq_snapshot("after_shader_power_on");
    pio.write_reg(GPU_IRQ_CLEAR, POWER_CHANGED_SINGLE | POWER_CHANGED_ALL);
    pio.record_irq_snapshot("after_power_irq_clear");
    pio.write_reg(SHADER_PWROFF_LO, 0x0000_0003);
    pio.record_irq_snapshot("after_shader_power_off");
    pio.write_reg(JOB_IRQ_RAWSTAT, JOB_SLOT0_COMPLETED);
    pio.write_reg(JOB_IRQ_MASK, JOB_SLOT0_COMPLETED);
    pio.record_interrupt_block_snapshot(
        "job",
        NOMALI_JOB_INT,
        "after_job_slot0_masked",
        JOB_IRQ_RAWSTAT,
        JOB_IRQ_MASK,
        JOB_IRQ_STATUS,
    );
    pio.write_reg(JOB_IRQ_CLEAR, JOB_SLOT0_COMPLETED);
    pio.record_interrupt_block_snapshot(
        "job",
        NOMALI_JOB_INT,
        "after_job_slot0_clear",
        JOB_IRQ_RAWSTAT,
        JOB_IRQ_MASK,
        JOB_IRQ_STATUS,
    );
    pio.write_reg(MMU_IRQ_RAWSTAT, MMU_PAGE_FAULT_AS0 | MMU_BUS_ERROR_AS0);
    pio.write_reg(MMU_IRQ_MASK, MMU_BUS_ERROR_AS0);
    pio.record_interrupt_block_snapshot(
        "mmu",
        NOMALI_MMU_INT,
        "after_mmu_bus_error_masked",
        MMU_IRQ_RAWSTAT,
        MMU_IRQ_MASK,
        MMU_IRQ_STATUS,
    );
    pio.write_reg(MMU_IRQ_CLEAR, MMU_BUS_ERROR_AS0);
    pio.record_interrupt_block_snapshot(
        "mmu",
        NOMALI_MMU_INT,
        "after_mmu_bus_error_clear",
        MMU_IRQ_RAWSTAT,
        MMU_IRQ_MASK,
        MMU_IRQ_STATUS,
    );
    pio.probe_register_faults();
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
    let irq_writes = pio
        .irq_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"name\":\"{}\",\"offset\":\"0x{:03x}\",\"value\":\"{}\",\"effect\":\"{}\"}}",
                write.name,
                write.offset,
                register_hex(write.value),
                write.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let power_writes = pio
        .power_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"name\":\"{}\",\"offset\":\"0x{:03x}\",\"value\":\"{}\",\"ready_register\":\"{}\",\"ready_offset\":\"0x{:03x}\",\"ready_value\":\"{}\",\"effect\":\"{}\"}}",
                write.name,
                write.offset,
                register_hex(write.value),
                write.ready_register,
                write.ready_offset,
                register_hex(write.ready_value),
                write.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let irq_snapshots = pio
        .irq_snapshots
        .iter()
        .map(|snapshot| {
            format!(
                "{{\"name\":\"{}\",\"rawstat\":\"{}\",\"mask\":\"{}\",\"status\":\"{}\",\"asserted\":{}}}",
                snapshot.name,
                register_hex(snapshot.rawstat),
                register_hex(snapshot.mask),
                register_hex(snapshot.status),
                snapshot.asserted,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let interrupt_block_snapshots = pio
        .interrupt_block_snapshots
        .iter()
        .map(|snapshot| {
            format!(
                "{{\"block\":\"{}\",\"nomali_int\":{},\"name\":\"{}\",\"rawstat_offset\":\"0x{:04x}\",\"mask_offset\":\"0x{:04x}\",\"status_offset\":\"0x{:04x}\",\"rawstat\":\"{}\",\"mask\":\"{}\",\"status\":\"{}\",\"asserted\":{}}}",
                snapshot.block,
                snapshot.nomali_int,
                snapshot.name,
                snapshot.rawstat_offset,
                snapshot.mask_offset,
                snapshot.status_offset,
                register_hex(snapshot.rawstat),
                register_hex(snapshot.mask),
                register_hex(snapshot.status),
                snapshot.asserted,
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
    let register_faults = pio
        .register_faults
        .iter()
        .map(|fault| {
            let value = fault
                .value
                .map(|value| format!("\"{}\"", register_hex(value)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"operation\":\"{}\",\"offset\":\"0x{:03x}\",\"value\":{},\"reason\":\"{}\",\"effect\":\"{}\"}}",
                fault.operation,
                fault.offset,
                value,
                fault.reason,
                fault.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"register_window_bytes\":{},\"reset_count\":{},\"register_fault_count\":{},\"command_writes\":[{}],\"irq_writes\":[{}],\"power_writes\":[{}],\"irq_snapshots\":[{}],\"interrupt_block_snapshots\":[{}],\"irq\":{{\"rawstat\":\"{}\",\"mask\":\"{}\",\"status\":\"{}\",\"asserted\":{}}},\"checkpoint\":{{\"word_count\":{}}},\"register_reads\":[{}],\"register_faults\":[{}]}}",
        NOMALI_REGISTER_WINDOW_BYTES,
        pio.reset_count,
        pio.register_faults.len(),
        command_writes,
        irq_writes,
        power_writes,
        irq_snapshots,
        interrupt_block_snapshots,
        register_hex(pio.read_raw(GPU_IRQ_RAWSTAT)),
        register_hex(pio.read_raw(GPU_IRQ_MASK)),
        register_hex(pio.irq_status()),
        pio.irq_asserted(),
        pio.checkpoint_word_count(),
        register_reads,
        register_faults,
    )
}

fn register_hex(value: u32) -> String {
    format!("0x{value:08x}")
}

fn irq_clear_effect(value: u32) -> &'static str {
    if value & RESET_COMPLETED != 0 {
        "clear_reset_completed"
    } else if value & (POWER_CHANGED_SINGLE | POWER_CHANGED_ALL) != 0 {
        "clear_power_changed"
    } else {
        "clear_irq_bits"
    }
}

fn job_irq_clear_effect(value: u32) -> &'static str {
    if value & JOB_SLOT0_COMPLETED != 0 {
        "clear_job_slot_0"
    } else {
        "clear_job_interrupt_bits"
    }
}

fn mmu_irq_clear_effect(value: u32) -> &'static str {
    if value & MMU_BUS_ERROR_AS0 != 0 {
        "clear_mmu_bus_error_as0"
    } else if value & MMU_PAGE_FAULT_AS0 != 0 {
        "clear_mmu_page_fault_as0"
    } else {
        "clear_mmu_interrupt_bits"
    }
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
