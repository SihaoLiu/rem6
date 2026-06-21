use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::RiscvMachineTrapCsr;
use rem6_memory::{Address, CacheLineLayout};
use rem6_system::RiscvSystemRunDriver;

use crate::config::Rem6RunConfig;
use crate::riscv_guest_output::{
    Rem6RiscvSbiConsoleSummary, Rem6RiscvSbiIpiSummary, Rem6RiscvSbiResetSummary,
    Rem6RiscvSbiRfenceSummary, Rem6RiscvSbiTimerSummary,
};
use crate::runtime_memory::CliMemoryRuntime;

const SUPERVISOR_SOFTWARE_INTERRUPT: u64 = 1;
const SUPERVISOR_TIMER_INTERRUPT: u64 = 5;
const SUPERVISOR_EXTERNAL_INTERRUPT: u64 = 9;
const SBI_SUPERVISOR_INTERRUPT_DELEGATION: u64 = (1 << SUPERVISOR_SOFTWARE_INTERRUPT)
    | (1 << SUPERVISOR_TIMER_INTERRUPT)
    | (1 << SUPERVISOR_EXTERNAL_INTERRUPT);

pub(crate) fn configure_cli_riscv_sbi_core(
    config: &Rem6RunConfig,
    core_index: u32,
    core: &RiscvCore,
    start_address: Address,
) {
    if !config.riscv_sbi() {
        return;
    }
    let delegated_interrupts =
        core.machine_trap_csr(RiscvMachineTrapCsr::Mideleg) | SBI_SUPERVISOR_INTERRUPT_DELEGATION;
    core.set_machine_trap_csr(RiscvMachineTrapCsr::Mideleg, delegated_interrupts);
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
) -> (
    Rem6RiscvSbiConsoleSummary,
    Vec<Rem6RiscvSbiTimerSummary>,
    Vec<Rem6RiscvSbiIpiSummary>,
    Vec<Rem6RiscvSbiRfenceSummary>,
    Vec<Rem6RiscvSbiResetSummary>,
) {
    let Some(firmware) = driver.riscv_sbi_firmware() else {
        return (
            Rem6RiscvSbiConsoleSummary::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
    };
    let console = Rem6RiscvSbiConsoleSummary::from_bytes(firmware.debug_console_bytes());
    let timers = (0..core_count)
        .filter_map(|cpu| {
            firmware
                .timer_deadline(CpuId::new(cpu))
                .map(|deadline| Rem6RiscvSbiTimerSummary::new(cpu, deadline))
        })
        .collect();
    let ipis = firmware
        .ipi_records()
        .iter()
        .map(Rem6RiscvSbiIpiSummary::from_record)
        .collect();
    let rfences = firmware
        .rfence_records()
        .iter()
        .map(Rem6RiscvSbiRfenceSummary::from_record)
        .collect();
    let resets = firmware
        .reset_records()
        .iter()
        .map(Rem6RiscvSbiResetSummary::from_record)
        .collect();
    (console, timers, ipis, rfences, resets)
}
