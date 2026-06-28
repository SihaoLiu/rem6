use rem6_isa_riscv::RiscvGdbXlen;

use crate::{riscv_checker, RiscvCore};

impl RiscvCore {
    pub fn xlen(&self) -> RiscvGdbXlen {
        self.state.lock().expect("riscv core lock").hart.xlen()
    }

    pub fn set_xlen(&self, xlen: RiscvGdbXlen) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_xlen(xlen);
        riscv_checker::sync_checker_hart(&mut state);
    }
}
