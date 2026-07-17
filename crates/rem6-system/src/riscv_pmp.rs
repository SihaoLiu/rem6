use rem6_cpu::RiscvCore;
use rem6_isa_riscv::{RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpError};

pub fn configure_riscv_unrestricted_pmp(core: &RiscvCore) -> Result<(), RiscvPmpError> {
    core.write_pmp_addr(0, u64::MAX)?;
    core.write_pmp_config(
        0,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Napot)
            .with_read(true)
            .with_write(true)
            .with_execute(true),
    )
}
