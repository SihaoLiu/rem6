use super::*;

impl crate::RiscvCore {
    pub(crate) fn reserve_test_fixed_fu_writeback(
        &self,
        sequence: u64,
        raw_ready_tick: u64,
    ) -> Result<(), O3RuntimeError> {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
                sequence,
                raw_ready_tick,
            )])
            .map(|_| ())
    }
}
