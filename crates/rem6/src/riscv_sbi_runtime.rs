use rem6_cpu::{CpuId, RiscvCore};
use rem6_memory::{Address, CacheLineLayout};
use rem6_system::RiscvSystemRunDriver;

use crate::config::Rem6RunConfig;
use crate::riscv_guest_output::{Rem6RiscvSbiConsoleSummary, Rem6RiscvSbiTimerSummary};
use crate::runtime_memory::CliMemoryRuntime;

pub(crate) fn configure_cli_riscv_sbi_core(
    config: &Rem6RunConfig,
    core_index: u32,
    core: &RiscvCore,
    start_address: Address,
) {
    if !config.riscv_sbi() {
        return;
    }
    if core_index == 0 {
        core.start_supervisor_hart(start_address, config.riscv_boot_a1());
    } else {
        core.set_hart_stopped();
    }
}

pub(crate) fn attach_cli_riscv_sbi_firmware(
    config: &Rem6RunConfig,
    driver: RiscvSystemRunDriver,
    memory: &CliMemoryRuntime,
    line_layout: CacheLineLayout,
) -> RiscvSystemRunDriver {
    if !config.riscv_sbi() {
        return driver;
    }

    let read_memory = memory.clone();
    let write_memory = memory.clone();
    driver
        .with_riscv_sbi_firmware()
        .with_riscv_sbi_firmware_and_functional_guest_memory_reader(move |address, bytes| {
            read_memory.read_guest_memory(address, bytes, line_layout)
        })
        .with_riscv_sbi_firmware_and_functional_guest_memory_writer(move |address, bytes| {
            write_memory.write_guest_memory(address, bytes, line_layout)
        })
}

pub(crate) fn collect_cli_riscv_sbi_output(
    driver: &RiscvSystemRunDriver,
    core_count: u32,
) -> (Rem6RiscvSbiConsoleSummary, Vec<Rem6RiscvSbiTimerSummary>) {
    let Some(firmware) = driver.riscv_sbi_firmware() else {
        return (Rem6RiscvSbiConsoleSummary::default(), Vec::new());
    };
    let console = Rem6RiscvSbiConsoleSummary::from_bytes(firmware.debug_console_bytes());
    let timers = (0..core_count)
        .filter_map(|cpu| {
            firmware
                .timer_deadline(CpuId::new(cpu))
                .map(|deadline| Rem6RiscvSbiTimerSummary::new(cpu, deadline))
        })
        .collect();
    (console, timers)
}
