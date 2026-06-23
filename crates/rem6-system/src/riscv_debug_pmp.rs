use rem6_cpu::RiscvCore;
use rem6_isa_riscv::{RiscvGdbXlen, RiscvPmpError, RiscvPmpTable};

pub(crate) fn read_core_pmp_config_csr(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
    first_index: usize,
) -> u64 {
    core.pmp_snapshot()
        .entries()
        .iter()
        .skip(first_index)
        .take(pmp_config_entries_per_csr(xlen))
        .enumerate()
        .fold(0, |bits, (offset, entry)| {
            bits | (u64::from(entry.config().bits()) << (offset * 8))
        })
}

pub(crate) fn read_core_pmp_addr_csr(core: &RiscvCore, index: usize) -> u64 {
    core.pmp_snapshot()
        .entries()
        .get(index)
        .map(|entry| entry.raw_addr())
        .unwrap_or(0)
}

pub(crate) fn write_pmp_config_csr(
    xlen: RiscvGdbXlen,
    pmp: &mut RiscvPmpTable,
    first_index: usize,
    value: u64,
) -> Result<(), RiscvPmpError> {
    for offset in 0..pmp_config_entries_per_csr(xlen) {
        pmp.write_config_bits(first_index + offset, ((value >> (offset * 8)) & 0xff) as u8)?;
    }
    Ok(())
}

pub(crate) fn write_core_pmp_config_csr(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
    first_index: usize,
    value: u64,
) -> Result<(), RiscvPmpError> {
    for offset in 0..pmp_config_entries_per_csr(xlen) {
        core.write_pmp_config_bits(first_index + offset, ((value >> (offset * 8)) & 0xff) as u8)?;
    }
    Ok(())
}

const fn pmp_config_entries_per_csr(xlen: RiscvGdbXlen) -> usize {
    match xlen {
        RiscvGdbXlen::Rv32 => 4,
        RiscvGdbXlen::Rv64 => 8,
    }
}
