use rem6_cpu::RiscvCore;
use rem6_memory::Address;
use rem6_system::RiscvSystemRunDriver;

use crate::config::Rem6RunConfig;

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
) -> RiscvSystemRunDriver {
    if config.riscv_sbi() {
        driver.with_riscv_sbi_firmware()
    } else {
        driver
    }
}
