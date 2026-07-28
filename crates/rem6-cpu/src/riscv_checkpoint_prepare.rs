use rem6_kernel::Tick;

use crate::RiscvCore;

impl RiscvCore {
    pub fn prepare_source_local_checkpoint_capture(&self, deadline: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        let references = state
            .source_local_checkpoint_capture_deadlines
            .entry(deadline)
            .or_default();
        *references = references
            .checked_add(1)
            .expect("source-local checkpoint prepare reference overflow");
    }

    pub fn release_source_local_checkpoint_capture(&self, deadline: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(references) = state
            .source_local_checkpoint_capture_deadlines
            .get_mut(&deadline)
        else {
            return;
        };
        *references -= 1;
        if *references == 0 {
            state
                .source_local_checkpoint_capture_deadlines
                .remove(&deadline);
        }
    }

    pub(crate) fn source_local_checkpoint_capture_blocks_fetch(&self, now: Tick) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .source_local_checkpoint_capture_deadlines
            .retain(|deadline, references| *references > 0 && *deadline >= now);
        !state.source_local_checkpoint_capture_deadlines.is_empty()
    }
}
