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
const CYCLE_COUNT_LO: u32 = 0x090;
const CYCLE_COUNT_HI: u32 = 0x094;
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
const L3_PRESENT_LO: u32 = 0x130;
const L3_PRESENT_HI: u32 = 0x134;
const SHADER_READY_LO: u32 = 0x140;
const SHADER_READY_HI: u32 = 0x144;
const TILER_READY_LO: u32 = 0x150;
const TILER_READY_HI: u32 = 0x154;
const L2_READY_LO: u32 = 0x160;
const L2_READY_HI: u32 = 0x164;
const L3_READY_LO: u32 = 0x170;
const L3_READY_HI: u32 = 0x174;
const SHADER_PWRON_LO: u32 = 0x180;
const SHADER_PWRON_HI: u32 = 0x184;
const TILER_PWRON_LO: u32 = 0x190;
const TILER_PWRON_HI: u32 = 0x194;
const L2_PWRON_LO: u32 = 0x1a0;
const L2_PWRON_HI: u32 = 0x1a4;
const L3_PWRON_LO: u32 = 0x1b0;
const L3_PWRON_HI: u32 = 0x1b4;
const SHADER_PWROFF_LO: u32 = 0x1c0;
const SHADER_PWROFF_HI: u32 = 0x1c4;
const TILER_PWROFF_LO: u32 = 0x1d0;
const TILER_PWROFF_HI: u32 = 0x1d4;
const L2_PWROFF_LO: u32 = 0x1e0;
const L2_PWROFF_HI: u32 = 0x1e4;
const L3_PWROFF_LO: u32 = 0x1f0;
const L3_PWROFF_HI: u32 = 0x1f4;
const JOB_IRQ_RAWSTAT: u32 = 0x1000;
const JOB_IRQ_CLEAR: u32 = 0x1004;
const JOB_IRQ_MASK: u32 = 0x1008;
const JOB_IRQ_STATUS: u32 = 0x100c;
const JOB_SLOT0_BASE: u32 = 0x1800;
const JOB_SLOT_STRIDE: u32 = 0x80;
const JOB_SLOT_COUNT: u32 = 16;
const JS_HEAD_LO: u32 = 0x00;
const JS_HEAD_HI: u32 = 0x04;
const JS_TAIL_LO: u32 = 0x08;
const JS_TAIL_HI: u32 = 0x0c;
const JS_AFFINITY_LO: u32 = 0x10;
const JS_AFFINITY_HI: u32 = 0x14;
const JS_CONFIG: u32 = 0x18;
const JS_COMMAND: u32 = 0x20;
const JS_STATUS: u32 = 0x24;
const JS_HEAD_NEXT_LO: u32 = 0x40;
const JS_HEAD_NEXT_HI: u32 = 0x44;
const JS_AFFINITY_NEXT_LO: u32 = 0x50;
const JS_AFFINITY_NEXT_HI: u32 = 0x54;
const JS_CONFIG_NEXT: u32 = 0x58;
const JS_COMMAND_NEXT: u32 = 0x60;
const MMU_IRQ_RAWSTAT: u32 = 0x2000;
const MMU_IRQ_CLEAR: u32 = 0x2004;
const MMU_IRQ_MASK: u32 = 0x2008;
const MMU_IRQ_STATUS: u32 = 0x200c;
const MMU_AS0_BASE: u32 = 0x2400;
const MMU_ADDRESS_SPACE_STRIDE: u32 = 0x40;
const MMU_ADDRESS_SPACE_COUNT: u32 = 16;
const AS_TRANSTAB_LO: u32 = 0x00;
const AS_TRANSTAB_HI: u32 = 0x04;
const AS_MEMATTR_LO: u32 = 0x08;
const AS_MEMATTR_HI: u32 = 0x0c;
const AS_LOCKADDR_LO: u32 = 0x10;
const AS_LOCKADDR_HI: u32 = 0x14;
const AS_COMMAND: u32 = 0x18;

const RESET_COMPLETED: u32 = 1 << 8;
const POWER_CHANGED_SINGLE: u32 = 1 << 9;
const POWER_CHANGED_ALL: u32 = 1 << 10;
const PRFCNT_SAMPLE_COMPLETED: u32 = 1 << 16;
const CLEAN_CACHES_COMPLETED: u32 = 1 << 17;
const JOB_SLOT0_COMPLETED: u32 = 1 << 0;
const MMU_PAGE_FAULT_AS0: u32 = 1 << 0;
const MMU_BUS_ERROR_AS0: u32 = 1 << 16;
const GPU_COMMAND_NOP: u32 = 0x00;
const GPU_COMMAND_SOFT_RESET: u32 = 0x01;
const GPU_COMMAND_HARD_RESET: u32 = 0x02;
const GPU_COMMAND_PRFCNT_CLEAR: u32 = 0x03;
const GPU_COMMAND_PRFCNT_SAMPLE: u32 = 0x04;
const GPU_COMMAND_CYCLE_COUNT_START: u32 = 0x05;
const GPU_COMMAND_CYCLE_COUNT_STOP: u32 = 0x06;
const GPU_COMMAND_CLEAN_CACHES: u32 = 0x07;
const GPU_COMMAND_CLEAN_INV_CACHES: u32 = 0x08;
const GPU_COMMAND_UNSUPPORTED_PROBE: u32 = 0xdead_dead;
const AS_COMMAND_NOP: u32 = 0x00;
const AS_COMMAND_UPDATE: u32 = 0x01;
const AS_COMMAND_LOCK: u32 = 0x02;
const AS_COMMAND_UNLOCK: u32 = 0x03;
const AS_COMMAND_FLUSH_PT: u32 = 0x04;
const AS_COMMAND_FLUSH_MEM: u32 = 0x05;
const AS_COMMAND_UNSUPPORTED_PROBE: u32 = 0xdead_0006;
const JS_COMMAND_NOP: u32 = 0x00;
const JS_COMMAND_START: u32 = 0x01;
const JS_COMMAND_SOFT_STOP: u32 = 0x02;
const JS_COMMAND_HARD_STOP: u32 = 0x03;
const JS_COMMAND_SOFT_STOP_0: u32 = 0x04;
const JS_COMMAND_HARD_STOP_0: u32 = 0x05;
const JS_COMMAND_SOFT_STOP_1: u32 = 0x06;
const JS_COMMAND_HARD_STOP_1: u32 = 0x07;
const JS_COMMAND_UNSUPPORTED_PROBE: u32 = 0xdead_0008;
const JS_STATUS_DONE: u32 = 0x01;
const JS_CONFIG_START_MMU: u32 = 1 << 10;
const JS_CONFIG_JOB_CHAIN_FLAG: u32 = 1 << 11;
const NOMALI_REGISTER_FAULT_OUT_OF_RANGE_WRITE_OFFSET: u32 = NOMALI_REGISTER_WINDOW_BYTES as u32;
const NOMALI_REGISTER_FAULT_WRITE_VALUE: u32 = 0x1234_5678;

const NOMALI_T760_RESET_REGISTERS: &[(u32, u32)] = &[
    (GPU_ID, 0x0750_0000),
    (L2_FEATURES, 0x0713_0206),
    (TILER_FEATURES, 0x0000_0809),
    (MEM_FEATURES, 0x0000_0001),
    (MMU_FEATURES, 0x0000_2830),
    (AS_PRESENT, 0x0000_00ff),
    (JS_PRESENT, 0x0000_0007),
    (THREAD_MAX_THREADS, 0x0000_0100),
    (THREAD_MAX_WORKGROUP_SIZE, 0x0000_0100),
    (THREAD_MAX_BARRIER_SIZE, 0x0000_0100),
    (THREAD_FEATURES, 0x0a04_0400),
    (TEXTURE_FEATURES_0, 0x00fe_001e),
    (TEXTURE_FEATURES_1, 0x0000_ffff),
    (TEXTURE_FEATURES_2, 0x9f81_ffff),
    (JS0_FEATURES, 0x0000_020e),
    (JS1_FEATURES, 0x0000_01fe),
    (JS2_FEATURES, 0x0000_007e),
    (SHADER_PRESENT_LO, 0x0000_000f),
    (SHADER_PRESENT_HI, 0x0000_0000),
    (TILER_PRESENT_LO, 0x0000_0001),
    (TILER_PRESENT_HI, 0x0000_0000),
    (L2_PRESENT_LO, 0x0000_0001),
    (L2_PRESENT_HI, 0x0000_0000),
];

const NOMALI_OBSERVED_PIO_READS: &[(&str, u32)] = &[
    ("gpu_id", GPU_ID),
    ("l2_features", L2_FEATURES),
    ("tiler_features", TILER_FEATURES),
    ("thread_features", THREAD_FEATURES),
    ("shader_present_lo", SHADER_PRESENT_LO),
    ("shader_present_hi", SHADER_PRESENT_HI),
    ("gpu_irq_status", GPU_IRQ_STATUS),
    ("job_irq_status", JOB_IRQ_STATUS),
    ("mmu_irq_status", MMU_IRQ_STATUS),
];

type NoMaliInterruptBlock = (&'static str, u32, u32, u32);

const GPU_INTERRUPT_BLOCK: NoMaliInterruptBlock =
    ("gpu", NOMALI_GPU_INT, GPU_IRQ_RAWSTAT, GPU_IRQ_MASK);
const JOB_INTERRUPT_BLOCK: NoMaliInterruptBlock =
    ("job", NOMALI_JOB_INT, JOB_IRQ_RAWSTAT, JOB_IRQ_MASK);
const MMU_INTERRUPT_BLOCK: NoMaliInterruptBlock =
    ("mmu", NOMALI_MMU_INT, MMU_IRQ_RAWSTAT, MMU_IRQ_MASK);

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliCommandWrite {
    name: &'static str,
    offset: u32,
    value: u32,
    command: &'static str,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliAddressSpaceCommandWrite {
    space: u8,
    name: &'static str,
    offset: u32,
    value: u32,
    command: &'static str,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliAddressSpaceWrite {
    space: u8,
    name: &'static str,
    offset: u32,
    value: u32,
    register: &'static str,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliJobSlotCommandWrite {
    slot: u8,
    name: &'static str,
    offset: u32,
    value: u32,
    command: &'static str,
    effect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliJobSlotWrite {
    slot: u8,
    name: &'static str,
    offset: u32,
    value: u32,
    register: &'static str,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NoMaliPowerDomain {
    pwron_name: &'static str,
    pwroff_name: &'static str,
    ready_register: &'static str,
    present_offset: u32,
    ready_offset: u32,
    pwron_offset: u32,
    pwroff_offset: u32,
}

const SHADER_POWER_DOMAIN: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "shader_pwron_lo",
    pwroff_name: "shader_pwroff_lo",
    ready_register: "shader_ready_lo",
    present_offset: SHADER_PRESENT_LO,
    ready_offset: SHADER_READY_LO,
    pwron_offset: SHADER_PWRON_LO,
    pwroff_offset: SHADER_PWROFF_LO,
};

const SHADER_POWER_DOMAIN_HI: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "shader_pwron_hi",
    pwroff_name: "shader_pwroff_hi",
    ready_register: "shader_ready_hi",
    present_offset: SHADER_PRESENT_HI,
    ready_offset: SHADER_READY_HI,
    pwron_offset: SHADER_PWRON_HI,
    pwroff_offset: SHADER_PWROFF_HI,
};

const TILER_POWER_DOMAIN: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "tiler_pwron_lo",
    pwroff_name: "tiler_pwroff_lo",
    ready_register: "tiler_ready_lo",
    present_offset: TILER_PRESENT_LO,
    ready_offset: TILER_READY_LO,
    pwron_offset: TILER_PWRON_LO,
    pwroff_offset: TILER_PWROFF_LO,
};

const TILER_POWER_DOMAIN_HI: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "tiler_pwron_hi",
    pwroff_name: "tiler_pwroff_hi",
    ready_register: "tiler_ready_hi",
    present_offset: TILER_PRESENT_HI,
    ready_offset: TILER_READY_HI,
    pwron_offset: TILER_PWRON_HI,
    pwroff_offset: TILER_PWROFF_HI,
};

const L2_POWER_DOMAIN: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "l2_pwron_lo",
    pwroff_name: "l2_pwroff_lo",
    ready_register: "l2_ready_lo",
    present_offset: L2_PRESENT_LO,
    ready_offset: L2_READY_LO,
    pwron_offset: L2_PWRON_LO,
    pwroff_offset: L2_PWROFF_LO,
};

const L2_POWER_DOMAIN_HI: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "l2_pwron_hi",
    pwroff_name: "l2_pwroff_hi",
    ready_register: "l2_ready_hi",
    present_offset: L2_PRESENT_HI,
    ready_offset: L2_READY_HI,
    pwron_offset: L2_PWRON_HI,
    pwroff_offset: L2_PWROFF_HI,
};

const L3_POWER_DOMAIN: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "l3_pwron_lo",
    pwroff_name: "l3_pwroff_lo",
    ready_register: "l3_ready_lo",
    present_offset: L3_PRESENT_LO,
    ready_offset: L3_READY_LO,
    pwron_offset: L3_PWRON_LO,
    pwroff_offset: L3_PWROFF_LO,
};

const L3_POWER_DOMAIN_HI: NoMaliPowerDomain = NoMaliPowerDomain {
    pwron_name: "l3_pwron_hi",
    pwroff_name: "l3_pwroff_hi",
    ready_register: "l3_ready_hi",
    present_offset: L3_PRESENT_HI,
    ready_offset: L3_READY_HI,
    pwron_offset: L3_PWRON_HI,
    pwroff_offset: L3_PWROFF_HI,
};

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
struct NoMaliInterruptCallback {
    block: &'static str,
    nomali_int: u32,
    trigger: &'static str,
    rawstat_offset: u32,
    mask_offset: u32,
    rawstat: u32,
    mask: u32,
    status: u32,
    set: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliJobSlotSnapshot {
    slot: u8,
    name: &'static str,
    head_lo: u32,
    head_hi: u32,
    tail_lo: u32,
    tail_hi: u32,
    affinity_lo: u32,
    affinity_hi: u32,
    config: u32,
    status: u32,
    head_next_lo: u32,
    head_next_hi: u32,
    command_next: u32,
    job_irq_rawstat: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoMaliAddressSpaceSnapshot {
    space: u8,
    name: &'static str,
    transtab_lo: u32,
    transtab_hi: u32,
    memattr_lo: u32,
    memattr_hi: u32,
    lockaddr_lo: u32,
    lockaddr_hi: u32,
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
    cycle_counter_source_tick: u64,
    cycle_counter_running: bool,
    cycle_counter_start_tick: u64,
    cycle_counter_stop_tick: u64,
    cycle_counter_elapsed_ticks: u64,
    command_writes: Vec<NoMaliCommandWrite>,
    address_space_command_writes: Vec<NoMaliAddressSpaceCommandWrite>,
    address_space_writes: Vec<NoMaliAddressSpaceWrite>,
    job_slot_command_writes: Vec<NoMaliJobSlotCommandWrite>,
    job_slot_writes: Vec<NoMaliJobSlotWrite>,
    irq_writes: Vec<NoMaliIrqWrite>,
    power_writes: Vec<NoMaliPowerWrite>,
    irq_snapshots: Vec<NoMaliIrqSnapshot>,
    interrupt_block_snapshots: Vec<NoMaliInterruptBlockSnapshot>,
    interrupt_callbacks: Vec<NoMaliInterruptCallback>,
    address_space_snapshots: Vec<NoMaliAddressSpaceSnapshot>,
    job_slot_snapshots: Vec<NoMaliJobSlotSnapshot>,
    register_faults: Vec<NoMaliRegisterFault>,
}

impl NoMaliT760RegisterFile {
    fn new() -> Self {
        Self {
            registers: vec![0; NOMALI_REGISTER_WINDOW_WORDS],
            reset_count: 0,
            cycle_counter_source_tick: 0,
            cycle_counter_running: false,
            cycle_counter_start_tick: 0,
            cycle_counter_stop_tick: 0,
            cycle_counter_elapsed_ticks: 0,
            command_writes: Vec::new(),
            address_space_command_writes: Vec::new(),
            address_space_writes: Vec::new(),
            job_slot_command_writes: Vec::new(),
            job_slot_writes: Vec::new(),
            irq_writes: Vec::new(),
            power_writes: Vec::new(),
            irq_snapshots: Vec::new(),
            interrupt_block_snapshots: Vec::new(),
            interrupt_callbacks: Vec::new(),
            address_space_snapshots: Vec::new(),
            job_slot_snapshots: Vec::new(),
            register_faults: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.registers.fill(0);
        self.cycle_counter_running = false;
        self.cycle_counter_start_tick = 0;
        self.cycle_counter_stop_tick = 0;
        self.cycle_counter_elapsed_ticks = 0;
        for &(offset, value) in NOMALI_T760_RESET_REGISTERS {
            self.write_raw(offset, value);
        }
        self.reset_count += 1;
    }

    fn set_cycle_counter_source_tick(&mut self, tick: u64) {
        self.cycle_counter_source_tick = tick;
    }

    fn read_raw(&self, offset: u32) -> u32 {
        self.registers[(offset / 4) as usize]
    }

    fn read_reg_value(&self, offset: u32) -> u32 {
        match offset {
            GPU_IRQ_STATUS => self.interrupt_status_at(GPU_INTERRUPT_BLOCK),
            JOB_IRQ_STATUS => self.interrupt_status_at(JOB_INTERRUPT_BLOCK),
            MMU_IRQ_STATUS => self.interrupt_status_at(MMU_INTERRUPT_BLOCK),
            _ => self.read_raw(offset),
        }
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
        if let Some(slot) = job_slot_command_next_slot(offset) {
            self.job_slot_command_next(slot, offset, value);
            return;
        }
        if let Some(slot) = job_slot_command_slot(offset) {
            self.job_slot_command(slot, offset, value);
            return;
        }
        if let Some((slot, name, register)) = job_slot_rw_register(offset) {
            self.job_slot_write(slot, offset, value, name, register);
            return;
        }
        if let Some(space) = mmu_address_space_command_space(offset) {
            self.mmu_address_space_command(space, offset, value);
            return;
        }
        if let Some((space, name, register)) = mmu_address_space_rw_register(offset) {
            self.mmu_address_space_write(space, offset, value, name, register);
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
            GPU_IRQ_MASK => self.write_interrupt_mask(GPU_INTERRUPT_BLOCK, value),
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
            JOB_IRQ_MASK => self.write_interrupt_mask(JOB_INTERRUPT_BLOCK, value),
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
            MMU_IRQ_MASK => self.write_interrupt_mask(MMU_INTERRUPT_BLOCK, value),
            MMU_IRQ_STATUS => {}
            GPU_COMMAND => self.gpu_command(value),
            SHADER_PWRON_LO => self.power_on(&SHADER_POWER_DOMAIN, value),
            SHADER_PWRON_HI => self.power_on(&SHADER_POWER_DOMAIN_HI, value),
            TILER_PWRON_LO => self.power_on(&TILER_POWER_DOMAIN, value),
            TILER_PWRON_HI => self.power_on(&TILER_POWER_DOMAIN_HI, value),
            L2_PWRON_LO => self.power_on(&L2_POWER_DOMAIN, value),
            L2_PWRON_HI => self.power_on(&L2_POWER_DOMAIN_HI, value),
            L3_PWRON_LO => self.power_on(&L3_POWER_DOMAIN, value),
            L3_PWRON_HI => self.power_on(&L3_POWER_DOMAIN_HI, value),
            SHADER_PWROFF_LO => self.power_off(&SHADER_POWER_DOMAIN, value),
            SHADER_PWROFF_HI => self.power_off(&SHADER_POWER_DOMAIN_HI, value),
            TILER_PWROFF_LO => self.power_off(&TILER_POWER_DOMAIN, value),
            TILER_PWROFF_HI => self.power_off(&TILER_POWER_DOMAIN_HI, value),
            L2_PWROFF_LO => self.power_off(&L2_POWER_DOMAIN, value),
            L2_PWROFF_HI => self.power_off(&L2_POWER_DOMAIN_HI, value),
            L3_PWROFF_LO => self.power_off(&L3_POWER_DOMAIN, value),
            L3_PWROFF_HI => self.power_off(&L3_POWER_DOMAIN_HI, value),
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
        let _ = self.checked_register_index("read", 0x003, None);
        self.write_reg(
            NOMALI_REGISTER_FAULT_OUT_OF_RANGE_WRITE_OFFSET,
            NOMALI_REGISTER_FAULT_WRITE_VALUE,
        );
    }

    fn gpu_command(&mut self, value: u32) {
        match value {
            GPU_COMMAND_NOP => self.gpu_command_no_effect(value, "nop"),
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
            GPU_COMMAND_PRFCNT_SAMPLE => {
                self.command_writes.push(NoMaliCommandWrite {
                    name: "gpu_command",
                    offset: GPU_COMMAND,
                    value,
                    command: "perf_counter_sample",
                    effect: "perf_counter_sample_completed_interrupt",
                });
                self.raise_interrupt(PRFCNT_SAMPLE_COMPLETED);
            }
            GPU_COMMAND_CLEAN_CACHES => {
                self.command_writes.push(NoMaliCommandWrite {
                    name: "gpu_command",
                    offset: GPU_COMMAND,
                    value,
                    command: "clean_caches",
                    effect: "clean_caches_completed_interrupt",
                });
                self.raise_interrupt(CLEAN_CACHES_COMPLETED);
            }
            GPU_COMMAND_CLEAN_INV_CACHES => {
                self.command_writes.push(NoMaliCommandWrite {
                    name: "gpu_command",
                    offset: GPU_COMMAND,
                    value,
                    command: "clean_invalidate_caches",
                    effect: "clean_invalidate_caches_completed_interrupt",
                });
                self.raise_interrupt(CLEAN_CACHES_COMPLETED);
            }
            GPU_COMMAND_PRFCNT_CLEAR => self.gpu_command_no_effect(value, "perf_counter_clear"),
            GPU_COMMAND_CYCLE_COUNT_START => {
                self.start_cycle_counter(value);
            }
            GPU_COMMAND_CYCLE_COUNT_STOP => {
                self.stop_cycle_counter(value);
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

    fn gpu_command_no_effect(&mut self, value: u32, command: &'static str) {
        self.command_writes.push(NoMaliCommandWrite {
            name: "gpu_command",
            offset: GPU_COMMAND,
            value,
            command,
            effect: "no_effect",
        });
    }

    fn start_cycle_counter(&mut self, value: u32) {
        self.cycle_counter_running = true;
        self.cycle_counter_start_tick = 0;
        self.cycle_counter_stop_tick = 0;
        self.cycle_counter_elapsed_ticks = 0;
        self.write_cycle_counter_registers(0);
        self.command_writes.push(NoMaliCommandWrite {
            name: "gpu_command",
            offset: GPU_COMMAND,
            value,
            command: "cycle_count_start",
            effect: "cycle_counter_started",
        });
    }

    fn stop_cycle_counter(&mut self, value: u32) {
        self.cycle_counter_stop_tick = self.cycle_counter_source_tick;
        self.cycle_counter_elapsed_ticks = self
            .cycle_counter_stop_tick
            .saturating_sub(self.cycle_counter_start_tick);
        self.cycle_counter_running = false;
        self.write_cycle_counter_registers(self.cycle_counter_elapsed_ticks);
        self.command_writes.push(NoMaliCommandWrite {
            name: "gpu_command",
            offset: GPU_COMMAND,
            value,
            command: "cycle_count_stop",
            effect: "cycle_counter_stopped",
        });
    }

    fn write_cycle_counter_registers(&mut self, elapsed_ticks: u64) {
        self.write_raw(CYCLE_COUNT_LO, elapsed_ticks as u32);
        self.write_raw(CYCLE_COUNT_HI, (elapsed_ticks >> 32) as u32);
    }

    fn job_slot_command_next(&mut self, slot: u8, offset: u32, value: u32) {
        self.write_raw(offset, value);
        let (command, effect) = if value == JS_COMMAND_START {
            self.start_next_job(slot);
            ("start", "job_completed_interrupt")
        } else {
            ("pending_command", "pending_command_recorded")
        };
        self.job_slot_command_writes
            .push(NoMaliJobSlotCommandWrite {
                slot,
                name: "job_slot_command_next",
                offset,
                value,
                command,
                effect,
            });
    }

    fn start_next_job(&mut self, slot: u8) {
        let base = job_slot_base(slot);
        if self.read_raw(base + JS_COMMAND_NEXT) != JS_COMMAND_START {
            return;
        }
        let head_lo = self.read_raw(base + JS_HEAD_NEXT_LO);
        let head_hi = self.read_raw(base + JS_HEAD_NEXT_HI);
        self.write_raw(base + JS_HEAD_LO, head_lo);
        self.write_raw(base + JS_HEAD_HI, head_hi);
        self.write_raw(base + JS_TAIL_LO, head_lo);
        self.write_raw(base + JS_TAIL_HI, head_hi);
        self.write_raw(
            base + JS_AFFINITY_LO,
            self.read_raw(base + JS_AFFINITY_NEXT_LO),
        );
        self.write_raw(
            base + JS_AFFINITY_HI,
            self.read_raw(base + JS_AFFINITY_NEXT_HI),
        );
        self.write_raw(base + JS_CONFIG, self.read_raw(base + JS_CONFIG_NEXT));
        self.write_raw(base + JS_HEAD_NEXT_LO, 0);
        self.write_raw(base + JS_HEAD_NEXT_HI, 0);
        self.write_raw(base + JS_COMMAND_NEXT, 0);
        self.write_raw(base + JS_STATUS, JS_STATUS_DONE);
        self.raise_interrupt_at(JOB_IRQ_RAWSTAT, 1 << slot);
    }

    fn job_slot_command(&mut self, slot: u8, offset: u32, value: u32) {
        let base = job_slot_base(slot);
        let chain_flag_set = self.read_raw(base + JS_CONFIG) & JS_CONFIG_JOB_CHAIN_FLAG != 0;
        let (command, effect) = match value {
            JS_COMMAND_NOP => ("nop", "no_effect"),
            JS_COMMAND_START => ("start", "invalid_command_register"),
            JS_COMMAND_SOFT_STOP => ("soft_stop", "no_effect"),
            JS_COMMAND_HARD_STOP => ("hard_stop", "no_effect"),
            JS_COMMAND_SOFT_STOP_0 => (
                "soft_stop_if_chain_flag_clear",
                if chain_flag_set {
                    "condition_not_met"
                } else {
                    "no_effect"
                },
            ),
            JS_COMMAND_HARD_STOP_0 => (
                "hard_stop_if_chain_flag_clear",
                if chain_flag_set {
                    "condition_not_met"
                } else {
                    "no_effect"
                },
            ),
            JS_COMMAND_SOFT_STOP_1 => (
                "soft_stop_if_chain_flag_set",
                if chain_flag_set {
                    "no_effect"
                } else {
                    "condition_not_met"
                },
            ),
            JS_COMMAND_HARD_STOP_1 => (
                "hard_stop_if_chain_flag_set",
                if chain_flag_set {
                    "no_effect"
                } else {
                    "condition_not_met"
                },
            ),
            _ => ("unsupported", "ignored"),
        };
        self.job_slot_command_writes
            .push(NoMaliJobSlotCommandWrite {
                slot,
                name: "job_slot_command",
                offset,
                value,
                command,
                effect,
            });
    }

    fn job_slot_write(
        &mut self,
        slot: u8,
        offset: u32,
        value: u32,
        name: &'static str,
        register: &'static str,
    ) {
        self.write_raw(offset, value);
        self.job_slot_writes.push(NoMaliJobSlotWrite {
            slot,
            name,
            offset,
            value,
            register,
            effect: "stored",
        });
    }

    fn mmu_address_space_command(&mut self, space: u8, offset: u32, value: u32) {
        let (command, effect) = match value {
            AS_COMMAND_NOP => ("nop", "no_effect"),
            AS_COMMAND_UPDATE => ("update", "no_effect"),
            AS_COMMAND_LOCK => ("lock", "no_effect"),
            AS_COMMAND_UNLOCK => ("unlock", "no_effect"),
            AS_COMMAND_FLUSH_PT => ("flush_page_table", "no_effect"),
            AS_COMMAND_FLUSH_MEM => ("flush_memory", "no_effect"),
            _ => ("unsupported", "ignored"),
        };
        self.address_space_command_writes
            .push(NoMaliAddressSpaceCommandWrite {
                space,
                name: "mmu_as_command",
                offset,
                value,
                command,
                effect,
            });
    }

    fn mmu_address_space_write(
        &mut self,
        space: u8,
        offset: u32,
        value: u32,
        name: &'static str,
        register: &'static str,
    ) {
        self.write_raw(offset, value);
        self.address_space_writes.push(NoMaliAddressSpaceWrite {
            space,
            name,
            offset,
            value,
            register,
            effect: "stored",
        });
    }

    fn power_on(&mut self, domain: &NoMaliPowerDomain, value: u32) {
        let ready =
            self.read_raw(domain.ready_offset) | (value & self.read_raw(domain.present_offset));
        self.write_raw(domain.ready_offset, ready);
        self.raise_power_changed_interrupt();
        self.power_writes.push(NoMaliPowerWrite {
            name: domain.pwron_name,
            offset: domain.pwron_offset,
            value,
            ready_register: domain.ready_register,
            ready_offset: domain.ready_offset,
            ready_value: ready,
            effect: "power_changed_interrupt",
        });
    }

    fn power_off(&mut self, domain: &NoMaliPowerDomain, value: u32) {
        let ready = self.read_raw(domain.ready_offset) & !value;
        self.write_raw(domain.ready_offset, ready);
        self.raise_power_changed_interrupt();
        self.power_writes.push(NoMaliPowerWrite {
            name: domain.pwroff_name,
            offset: domain.pwroff_offset,
            value,
            ready_register: domain.ready_register,
            ready_offset: domain.ready_offset,
            ready_value: ready,
            effect: "power_changed_interrupt",
        });
    }

    fn raise_power_changed_interrupt(&mut self) {
        self.raise_interrupt(POWER_CHANGED_SINGLE | POWER_CHANGED_ALL);
    }

    fn raise_interrupt(&mut self, flags: u32) {
        self.raise_interrupt_block(GPU_INTERRUPT_BLOCK, flags);
    }

    fn clear_interrupt(&mut self, flags: u32) {
        self.clear_interrupt_block(GPU_INTERRUPT_BLOCK, flags);
    }

    fn raise_interrupt_at(&mut self, rawstat_offset: u32, flags: u32) {
        self.raise_interrupt_block(interrupt_block_for_rawstat(rawstat_offset), flags);
    }

    fn clear_interrupt_at(&mut self, rawstat_offset: u32, flags: u32) {
        self.clear_interrupt_block(interrupt_block_for_rawstat(rawstat_offset), flags);
    }

    fn raise_interrupt_block(&mut self, interrupt: NoMaliInterruptBlock, flags: u32) {
        let (_, _, rawstat_offset, _) = interrupt;
        let old_asserted = self.interrupt_asserted_at(interrupt);
        self.write_raw(rawstat_offset, self.read_raw(rawstat_offset) | flags);
        self.record_interrupt_callback_transition(interrupt, "raise", old_asserted);
    }

    fn clear_interrupt_block(&mut self, interrupt: NoMaliInterruptBlock, flags: u32) {
        let (_, _, rawstat_offset, _) = interrupt;
        let old_asserted = self.interrupt_asserted_at(interrupt);
        self.write_raw(rawstat_offset, self.read_raw(rawstat_offset) & !flags);
        self.record_interrupt_callback_transition(interrupt, "clear", old_asserted);
    }

    fn write_interrupt_mask(&mut self, interrupt: NoMaliInterruptBlock, value: u32) {
        let (_, _, _, mask_offset) = interrupt;
        let old_asserted = self.interrupt_asserted_at(interrupt);
        self.write_raw(mask_offset, value);
        self.record_interrupt_callback_transition(interrupt, "mask", old_asserted);
    }

    fn interrupt_status_at(&self, interrupt: NoMaliInterruptBlock) -> u32 {
        let (_, _, rawstat_offset, mask_offset) = interrupt;
        self.read_raw(rawstat_offset) & self.read_raw(mask_offset)
    }

    fn interrupt_asserted_at(&self, interrupt: NoMaliInterruptBlock) -> bool {
        self.interrupt_status_at(interrupt) != 0
    }

    fn record_interrupt_callback_transition(
        &mut self,
        interrupt: NoMaliInterruptBlock,
        trigger: &'static str,
        old_asserted: bool,
    ) {
        let (block, nomali_int, rawstat_offset, mask_offset) = interrupt;
        let rawstat = self.read_raw(rawstat_offset);
        let mask = self.read_raw(mask_offset);
        let status = rawstat & mask;
        let set = status != 0;
        if old_asserted != set {
            self.interrupt_callbacks.push(NoMaliInterruptCallback {
                block,
                nomali_int,
                trigger,
                rawstat_offset,
                mask_offset,
                rawstat,
                mask,
                status,
                set,
            });
        }
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

    fn record_address_space_snapshot(&mut self, name: &'static str, space: u8) {
        let base = mmu_address_space_base(space);
        self.address_space_snapshots
            .push(NoMaliAddressSpaceSnapshot {
                space,
                name,
                transtab_lo: self.read_raw(base + AS_TRANSTAB_LO),
                transtab_hi: self.read_raw(base + AS_TRANSTAB_HI),
                memattr_lo: self.read_raw(base + AS_MEMATTR_LO),
                memattr_hi: self.read_raw(base + AS_MEMATTR_HI),
                lockaddr_lo: self.read_raw(base + AS_LOCKADDR_LO),
                lockaddr_hi: self.read_raw(base + AS_LOCKADDR_HI),
            });
    }

    fn record_job_slot_snapshot(&mut self, name: &'static str, slot: u8) {
        let base = job_slot_base(slot);
        self.job_slot_snapshots.push(NoMaliJobSlotSnapshot {
            slot,
            name,
            head_lo: self.read_raw(base + JS_HEAD_LO),
            head_hi: self.read_raw(base + JS_HEAD_HI),
            tail_lo: self.read_raw(base + JS_TAIL_LO),
            tail_hi: self.read_raw(base + JS_TAIL_HI),
            affinity_lo: self.read_raw(base + JS_AFFINITY_LO),
            affinity_hi: self.read_raw(base + JS_AFFINITY_HI),
            config: self.read_raw(base + JS_CONFIG),
            status: self.read_raw(base + JS_STATUS),
            head_next_lo: self.read_raw(base + JS_HEAD_NEXT_LO),
            head_next_hi: self.read_raw(base + JS_HEAD_NEXT_HI),
            command_next: self.read_raw(base + JS_COMMAND_NEXT),
            job_irq_rawstat: self.read_raw(JOB_IRQ_RAWSTAT),
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
    pio.set_cycle_counter_source_tick(execution.final_tick());
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
    pio.write_reg(TILER_PWRON_LO, pio.read_raw(TILER_PRESENT_LO));
    pio.write_reg(TILER_PWROFF_LO, pio.read_raw(TILER_PRESENT_LO));
    pio.write_reg(L2_PWRON_LO, pio.read_raw(L2_PRESENT_LO));
    pio.write_reg(L2_PWROFF_LO, pio.read_raw(L2_PRESENT_LO));
    pio.write_reg(SHADER_PWRON_HI, u32::MAX);
    pio.write_reg(SHADER_PWROFF_HI, u32::MAX);
    pio.write_reg(TILER_PWRON_HI, u32::MAX);
    pio.write_reg(TILER_PWROFF_HI, u32::MAX);
    pio.write_reg(L2_PWRON_HI, u32::MAX);
    pio.write_reg(L2_PWROFF_HI, u32::MAX);
    pio.write_reg(L3_PWRON_LO, 0x0000_0001);
    pio.write_reg(L3_PWROFF_LO, 0x0000_0001);
    pio.write_reg(L3_PWRON_HI, u32::MAX);
    pio.write_reg(L3_PWROFF_HI, u32::MAX);
    pio.write_reg(JOB_SLOT0_BASE + JS_HEAD_NEXT_LO, 0x0000_3400);
    pio.write_reg(JOB_SLOT0_BASE + JS_HEAD_NEXT_HI, 0x0000_0000);
    pio.write_reg(JOB_SLOT0_BASE + JS_AFFINITY_NEXT_LO, 0x0000_000f);
    pio.write_reg(JOB_SLOT0_BASE + JS_AFFINITY_NEXT_HI, 0x0000_0000);
    pio.write_reg(
        JOB_SLOT0_BASE + JS_CONFIG_NEXT,
        JS_CONFIG_START_MMU | JS_CONFIG_JOB_CHAIN_FLAG,
    );
    pio.record_job_slot_snapshot("after_job_slot0_next_register_writes", 0);
    pio.write_reg(JOB_SLOT0_BASE + JS_COMMAND_NEXT, JS_COMMAND_START);
    pio.record_job_slot_snapshot("after_job_slot0_start_next", 0);
    pio.write_reg(JOB_IRQ_MASK, JOB_SLOT0_COMPLETED);
    pio.record_interrupt_block_snapshot(
        "job",
        NOMALI_JOB_INT,
        "after_job_slot0_masked",
        JOB_IRQ_RAWSTAT,
        JOB_IRQ_MASK,
        JOB_IRQ_STATUS,
    );
    let job_irq_status_read = pio.read_reg_value(JOB_IRQ_STATUS);
    pio.write_reg(JOB_IRQ_CLEAR, JOB_SLOT0_COMPLETED);
    pio.record_interrupt_block_snapshot(
        "job",
        NOMALI_JOB_INT,
        "after_job_slot0_clear",
        JOB_IRQ_RAWSTAT,
        JOB_IRQ_MASK,
        JOB_IRQ_STATUS,
    );
    for command in [
        JS_COMMAND_NOP,
        JS_COMMAND_SOFT_STOP,
        JS_COMMAND_HARD_STOP,
        JS_COMMAND_SOFT_STOP_0,
        JS_COMMAND_HARD_STOP_0,
        JS_COMMAND_SOFT_STOP_1,
        JS_COMMAND_HARD_STOP_1,
        JS_COMMAND_UNSUPPORTED_PROBE,
    ] {
        pio.write_reg(JOB_SLOT0_BASE + JS_COMMAND, command);
    }
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
    let irq_status_reads = (job_irq_status_read, pio.read_reg_value(MMU_IRQ_STATUS));
    pio.write_reg(MMU_IRQ_CLEAR, MMU_BUS_ERROR_AS0);
    pio.record_interrupt_block_snapshot(
        "mmu",
        NOMALI_MMU_INT,
        "after_mmu_bus_error_clear",
        MMU_IRQ_RAWSTAT,
        MMU_IRQ_MASK,
        MMU_IRQ_STATUS,
    );
    pio.write_reg(
        GPU_IRQ_MASK,
        RESET_COMPLETED
            | POWER_CHANGED_SINGLE
            | POWER_CHANGED_ALL
            | PRFCNT_SAMPLE_COMPLETED
            | CLEAN_CACHES_COMPLETED,
    );
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_PRFCNT_SAMPLE);
    pio.record_irq_snapshot("after_perf_sample_command");
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_CLEAN_CACHES);
    pio.record_irq_snapshot("after_clean_caches_command");
    pio.write_reg(
        GPU_IRQ_CLEAR,
        PRFCNT_SAMPLE_COMPLETED | CLEAN_CACHES_COMPLETED,
    );
    pio.record_irq_snapshot("after_command_irq_clear");
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_CLEAN_INV_CACHES);
    pio.record_irq_snapshot("after_clean_invalidate_caches_command");
    pio.write_reg(GPU_IRQ_CLEAR, CLEAN_CACHES_COMPLETED);
    pio.record_irq_snapshot("after_clean_invalidate_irq_clear");
    pio.write_reg(
        GPU_IRQ_MASK,
        RESET_COMPLETED | POWER_CHANGED_SINGLE | POWER_CHANGED_ALL,
    );
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_NOP);
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_PRFCNT_CLEAR);
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_CYCLE_COUNT_START);
    pio.write_reg(GPU_COMMAND, GPU_COMMAND_CYCLE_COUNT_STOP);
    let as0_base = MMU_AS0_BASE;
    pio.write_reg(as0_base + AS_TRANSTAB_LO, 0x0000_5007);
    pio.write_reg(as0_base + AS_TRANSTAB_HI, 0x0000_0001);
    pio.write_reg(as0_base + AS_MEMATTR_LO, 0xff00_ff00);
    pio.write_reg(as0_base + AS_MEMATTR_HI, 0x00ff_00ff);
    pio.write_reg(as0_base + AS_LOCKADDR_LO, 0x0000_6000);
    pio.write_reg(as0_base + AS_LOCKADDR_HI, 0x0000_0000);
    pio.record_address_space_snapshot("after_mmu_as0_register_writes", 0);
    let as1_base = mmu_address_space_base(1);
    for (offset, value) in [
        (AS_TRANSTAB_LO, 0x0000_7007),
        (AS_TRANSTAB_HI, 0x0000_0002),
        (AS_MEMATTR_LO, 0xaa55_aa55),
        (AS_MEMATTR_HI, 0x55aa_55aa),
        (AS_LOCKADDR_LO, 0x0000_8000),
        (AS_LOCKADDR_HI, 0x0000_0000),
    ] {
        pio.write_reg(as1_base + offset, value);
    }
    pio.record_address_space_snapshot("after_mmu_as1_register_writes", 1);
    let as0_command = MMU_AS0_BASE + AS_COMMAND;
    for command in [
        AS_COMMAND_NOP,
        AS_COMMAND_UPDATE,
        AS_COMMAND_LOCK,
        AS_COMMAND_UNLOCK,
        AS_COMMAND_FLUSH_PT,
        AS_COMMAND_FLUSH_MEM,
        AS_COMMAND_UNSUPPORTED_PROBE,
    ] {
        pio.write_reg(as0_command, command);
    }
    for command in [AS_COMMAND_UPDATE, AS_COMMAND_FLUSH_MEM] {
        pio.write_reg(as1_base + AS_COMMAND, command);
    }
    pio.probe_register_faults();
    let contents = format!(
        "{{\"schema\":\"rem6.nomali.gpu-adapter.v1\",\"source_schema\":\"rem6.cli.gpu-run.v1\",\"scope\":\"gpu-run-execution-summary-adapter\",\"gpu\":{},\"interface\":{},\"pio\":{},\"execution\":{{\"status\":\"completed\",\"final_tick\":{},\"compute_units\":{},\"workgroup_completions\":{},\"coalesced_memory_accesses\":{},\"global_memory_reads\":{},\"global_memory_writes\":{},\"memory_read_callback_observations\":{},\"memory_write_callback_observations\":{},\"job_event_observations\":{},\"compute_unit_activity\":[{}]}}}}\n",
        nomali_gpu_json(&pio),
        nomali_interface_json(),
        nomali_pio_json(&pio, irq_status_reads),
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

fn interrupt_block_for_rawstat(rawstat_offset: u32) -> NoMaliInterruptBlock {
    match rawstat_offset {
        GPU_IRQ_RAWSTAT => GPU_INTERRUPT_BLOCK,
        JOB_IRQ_RAWSTAT => JOB_INTERRUPT_BLOCK,
        MMU_IRQ_RAWSTAT => MMU_INTERRUPT_BLOCK,
        _ => panic!("unknown NoMali interrupt rawstat offset 0x{rawstat_offset:04x}"),
    }
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

fn nomali_pio_json(pio: &NoMaliT760RegisterFile, irq_status_reads: (u32, u32)) -> String {
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
    let address_space_command_writes = pio
        .address_space_command_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"space\":{},\"name\":\"{}\",\"offset\":\"0x{:04x}\",\"value\":\"{}\",\"command\":\"{}\",\"effect\":\"{}\"}}",
                write.space,
                write.name,
                write.offset,
                register_hex(write.value),
                write.command,
                write.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let address_space_writes = pio
        .address_space_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"space\":{},\"name\":\"{}\",\"offset\":\"0x{:04x}\",\"value\":\"{}\",\"register\":\"{}\",\"effect\":\"{}\"}}",
                write.space,
                write.name,
                write.offset,
                register_hex(write.value),
                write.register,
                write.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let job_slot_command_writes = pio
        .job_slot_command_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"slot\":{},\"name\":\"{}\",\"offset\":\"0x{:04x}\",\"value\":\"{}\",\"command\":\"{}\",\"effect\":\"{}\"}}",
                write.slot,
                write.name,
                write.offset,
                register_hex(write.value),
                write.command,
                write.effect,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let job_slot_writes = pio
        .job_slot_writes
        .iter()
        .map(|write| {
            format!(
                "{{\"slot\":{},\"name\":\"{}\",\"offset\":\"0x{:04x}\",\"value\":\"{}\",\"register\":\"{}\",\"effect\":\"{}\"}}",
                write.slot,
                write.name,
                write.offset,
                register_hex(write.value),
                write.register,
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
    let interrupt_callbacks = pio
        .interrupt_callbacks
        .iter()
        .map(|callback| {
            format!(
                "{{\"block\":\"{}\",\"nomali_int\":{},\"trigger\":\"{}\",\"rawstat_offset\":\"0x{:04x}\",\"mask_offset\":\"0x{:04x}\",\"rawstat\":\"{}\",\"mask\":\"{}\",\"status\":\"{}\",\"set\":{}}}",
                callback.block,
                callback.nomali_int,
                callback.trigger,
                callback.rawstat_offset,
                callback.mask_offset,
                register_hex(callback.rawstat),
                register_hex(callback.mask),
                register_hex(callback.status),
                callback.set,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let address_space_snapshots = pio
        .address_space_snapshots
        .iter()
        .map(|snapshot| {
            format!(
                "{{\"space\":{},\"name\":\"{}\",\"transtab_lo\":\"{}\",\"transtab_hi\":\"{}\",\"memattr_lo\":\"{}\",\"memattr_hi\":\"{}\",\"lockaddr_lo\":\"{}\",\"lockaddr_hi\":\"{}\"}}",
                snapshot.space,
                snapshot.name,
                register_hex(snapshot.transtab_lo),
                register_hex(snapshot.transtab_hi),
                register_hex(snapshot.memattr_lo),
                register_hex(snapshot.memattr_hi),
                register_hex(snapshot.lockaddr_lo),
                register_hex(snapshot.lockaddr_hi),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let job_slot_snapshots = pio
        .job_slot_snapshots
        .iter()
        .map(|snapshot| {
            format!(
                "{{\"slot\":{},\"name\":\"{}\",\"head_lo\":\"{}\",\"head_hi\":\"{}\",\"tail_lo\":\"{}\",\"tail_hi\":\"{}\",\"affinity_lo\":\"{}\",\"affinity_hi\":\"{}\",\"config\":\"{}\",\"status\":\"{}\",\"head_next_lo\":\"{}\",\"head_next_hi\":\"{}\",\"command_next\":\"{}\",\"job_irq_rawstat\":\"{}\"}}",
                snapshot.slot,
                snapshot.name,
                register_hex(snapshot.head_lo),
                register_hex(snapshot.head_hi),
                register_hex(snapshot.tail_lo),
                register_hex(snapshot.tail_hi),
                register_hex(snapshot.affinity_lo),
                register_hex(snapshot.affinity_hi),
                register_hex(snapshot.config),
                register_hex(snapshot.status),
                register_hex(snapshot.head_next_lo),
                register_hex(snapshot.head_next_hi),
                register_hex(snapshot.command_next),
                register_hex(snapshot.job_irq_rawstat),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let register_reads = NOMALI_OBSERVED_PIO_READS
        .iter()
        .map(|(name, offset)| {
            let value = match *offset {
                JOB_IRQ_STATUS => irq_status_reads.0,
                MMU_IRQ_STATUS => irq_status_reads.1,
                _ => pio.read_reg_value(*offset),
            };
            format!(
                "{{\"name\":\"{}\",\"offset\":\"0x{:03x}\",\"value\":\"0x{:08x}\"}}",
                name, offset, value,
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
        "{{\"register_window_bytes\":{},\"reset_count\":{},\"register_fault_count\":{},\"command_writes\":[{}],\"address_space_command_writes\":[{}],\"address_space_writes\":[{}],\"job_slot_command_writes\":[{}],\"job_slot_writes\":[{}],\"irq_writes\":[{}],\"power_writes\":[{}],\"irq_snapshots\":[{}],\"interrupt_block_snapshots\":[{}],\"interrupt_callbacks\":[{}],\"address_space_snapshots\":[{}],\"job_slot_snapshots\":[{}],\"irq\":{{\"rawstat\":\"{}\",\"mask\":\"{}\",\"status\":\"{}\",\"asserted\":{}}},\"cycle_counter\":{{\"running\":{},\"start_tick\":{},\"stop_tick\":{},\"elapsed_ticks\":{},\"lo_offset\":\"0x{:03x}\",\"hi_offset\":\"0x{:03x}\",\"lo\":\"{}\",\"hi\":\"{}\"}},\"checkpoint\":{{\"word_count\":{}}},\"register_reads\":[{}],\"register_faults\":[{}]}}",
        NOMALI_REGISTER_WINDOW_BYTES,
        pio.reset_count,
        pio.register_faults.len(),
        command_writes,
        address_space_command_writes,
        address_space_writes,
        job_slot_command_writes,
        job_slot_writes,
        irq_writes,
        power_writes,
        irq_snapshots,
        interrupt_block_snapshots,
        interrupt_callbacks,
        address_space_snapshots,
        job_slot_snapshots,
        register_hex(pio.read_raw(GPU_IRQ_RAWSTAT)),
        register_hex(pio.read_raw(GPU_IRQ_MASK)),
        register_hex(pio.irq_status()),
        pio.irq_asserted(),
        pio.cycle_counter_running,
        pio.cycle_counter_start_tick,
        pio.cycle_counter_stop_tick,
        pio.cycle_counter_elapsed_ticks,
        CYCLE_COUNT_LO,
        CYCLE_COUNT_HI,
        register_hex(pio.read_raw(CYCLE_COUNT_LO)),
        register_hex(pio.read_raw(CYCLE_COUNT_HI)),
        pio.checkpoint_word_count(),
        register_reads,
        register_faults,
    )
}

fn register_hex(value: u32) -> String {
    format!("0x{value:08x}")
}

fn job_slot_base(slot: u8) -> u32 {
    JOB_SLOT0_BASE + u32::from(slot) * JOB_SLOT_STRIDE
}

fn job_slot_command_slot(offset: u32) -> Option<u8> {
    job_slot_reg_slot(offset, JS_COMMAND)
}

fn job_slot_command_next_slot(offset: u32) -> Option<u8> {
    job_slot_reg_slot(offset, JS_COMMAND_NEXT)
}

fn job_slot_rw_register(offset: u32) -> Option<(u8, &'static str, &'static str)> {
    let job_slot_span = JOB_SLOT_STRIDE * JOB_SLOT_COUNT;
    if !(JOB_SLOT0_BASE..JOB_SLOT0_BASE + job_slot_span).contains(&offset) {
        return None;
    }
    let relative = offset - JOB_SLOT0_BASE;
    let slot = (relative / JOB_SLOT_STRIDE) as u8;
    match relative % JOB_SLOT_STRIDE {
        JS_HEAD_NEXT_LO => Some((slot, "job_slot_head_next_lo", "head_next_lo")),
        JS_HEAD_NEXT_HI => Some((slot, "job_slot_head_next_hi", "head_next_hi")),
        JS_AFFINITY_NEXT_LO => Some((slot, "job_slot_affinity_next_lo", "affinity_next_lo")),
        JS_AFFINITY_NEXT_HI => Some((slot, "job_slot_affinity_next_hi", "affinity_next_hi")),
        JS_CONFIG_NEXT => Some((slot, "job_slot_config_next", "config_next")),
        _ => None,
    }
}

fn job_slot_reg_slot(offset: u32, register: u32) -> Option<u8> {
    let job_slot_span = JOB_SLOT_STRIDE * JOB_SLOT_COUNT;
    if !(JOB_SLOT0_BASE..JOB_SLOT0_BASE + job_slot_span).contains(&offset) {
        return None;
    }
    let relative = offset - JOB_SLOT0_BASE;
    if relative % JOB_SLOT_STRIDE == register {
        Some((relative / JOB_SLOT_STRIDE) as u8)
    } else {
        None
    }
}

fn mmu_address_space_base(space: u8) -> u32 {
    MMU_AS0_BASE + u32::from(space) * MMU_ADDRESS_SPACE_STRIDE
}

fn mmu_address_space_rw_register(offset: u32) -> Option<(u8, &'static str, &'static str)> {
    let address_space_span = MMU_ADDRESS_SPACE_STRIDE * MMU_ADDRESS_SPACE_COUNT;
    if !(MMU_AS0_BASE..MMU_AS0_BASE + address_space_span).contains(&offset) {
        return None;
    }
    let relative = offset - MMU_AS0_BASE;
    let space = (relative / MMU_ADDRESS_SPACE_STRIDE) as u8;
    match relative % MMU_ADDRESS_SPACE_STRIDE {
        AS_TRANSTAB_LO => Some((space, "mmu_as_transtab_lo", "transtab_lo")),
        AS_TRANSTAB_HI => Some((space, "mmu_as_transtab_hi", "transtab_hi")),
        AS_MEMATTR_LO => Some((space, "mmu_as_memattr_lo", "memattr_lo")),
        AS_MEMATTR_HI => Some((space, "mmu_as_memattr_hi", "memattr_hi")),
        AS_LOCKADDR_LO => Some((space, "mmu_as_lockaddr_lo", "lockaddr_lo")),
        AS_LOCKADDR_HI => Some((space, "mmu_as_lockaddr_hi", "lockaddr_hi")),
        _ => None,
    }
}

fn mmu_address_space_command_space(offset: u32) -> Option<u8> {
    let address_space_span = MMU_ADDRESS_SPACE_STRIDE * MMU_ADDRESS_SPACE_COUNT;
    if !(MMU_AS0_BASE..MMU_AS0_BASE + address_space_span).contains(&offset) {
        return None;
    }
    let relative = offset - MMU_AS0_BASE;
    if relative % MMU_ADDRESS_SPACE_STRIDE == AS_COMMAND {
        Some((relative / MMU_ADDRESS_SPACE_STRIDE) as u8)
    } else {
        None
    }
}

fn irq_clear_effect(value: u32) -> &'static str {
    if value & RESET_COMPLETED != 0 {
        "clear_reset_completed"
    } else if value & (POWER_CHANGED_SINGLE | POWER_CHANGED_ALL) != 0 {
        "clear_power_changed"
    } else if value & (PRFCNT_SAMPLE_COMPLETED | CLEAN_CACHES_COMPLETED) != 0 {
        "clear_command_completed"
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
