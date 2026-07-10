use crate::{
    riscv_checker, Address, HtmAbortRecord, HtmBeginRecord, HtmCommitRecord, HtmFailureCause,
    HtmTransactionError, HtmTransactionSnapshot, HtmTransactionUid, RiscvCore,
};

impl RiscvCore {
    pub fn begin_htm_transaction(&self) -> Result<HtmBeginRecord, HtmTransactionError> {
        let mut state = self.state.lock().expect("riscv core lock");
        if !state.htm.in_transaction() {
            state.htm_hart_checkpoint = Some(state.hart.clone());
        }
        state.htm.begin(Vec::new())
    }

    pub fn commit_htm_transaction(
        &self,
        uid: HtmTransactionUid,
    ) -> Result<HtmCommitRecord, HtmTransactionError> {
        let mut state = self.state.lock().expect("riscv core lock");
        let commit = state.htm.commit(uid)?;
        if commit.depth() == 0 {
            state.htm_hart_checkpoint = None;
        }
        Ok(commit)
    }

    pub fn abort_htm_transaction(
        &self,
        uid: HtmTransactionUid,
        cause: HtmFailureCause,
    ) -> Result<HtmAbortRecord, HtmTransactionError> {
        let mut state = self.state.lock().expect("riscv core lock");
        let abort = state.htm.abort(uid, cause)?;
        let checkpoint = state
            .htm_hart_checkpoint
            .take()
            .ok_or(HtmTransactionError::MissingArchitecturalCheckpoint { uid })?;
        let restored_pc = checkpoint.pc();
        state.hart = checkpoint;
        state.pending_trap = None;
        state.pending_fetch_prefix = None;
        state.discard_branch_speculations();
        state.o3_runtime.discard_live_staged_instructions();
        state.live_retire_gate.clear_pending_for_pc_redirect();
        riscv_checker::sync_checker_hart(&mut state);
        drop(state);
        self.core
            .reset_fetch_stream_to_pc(Address::new(restored_pc));
        Ok(abort)
    }

    pub fn htm_transaction_snapshot(&self) -> HtmTransactionSnapshot {
        self.state.lock().expect("riscv core lock").htm.snapshot()
    }

    pub fn in_htm_transaction(&self) -> bool {
        self.state
            .lock()
            .expect("riscv core lock")
            .htm
            .in_transaction()
    }
}
