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

    pub fn prepare_source_local_checkpoint_restore(&self, deadline: Tick) {
        self.prepare_source_local_checkpoint_restore_reference(None, deadline);
    }

    pub fn prepare_source_local_checkpoint_restore_after(&self, source_tick: Tick, deadline: Tick) {
        self.prepare_source_local_checkpoint_restore_reference(Some(source_tick), deadline);
    }

    fn prepare_source_local_checkpoint_restore_reference(
        &self,
        source_tick: Option<Tick>,
        deadline: Tick,
    ) {
        let mut state = self.state.lock().expect("riscv core lock");
        let references = state
            .source_local_checkpoint_restore_deadlines
            .entry((source_tick, deadline))
            .or_default();
        *references = references
            .checked_add(1)
            .expect("source-local checkpoint restore reference overflow");
    }

    pub fn release_source_local_checkpoint_restore(&self, deadline: Tick) {
        self.release_source_local_checkpoint_restore_reference(None, deadline);
    }

    pub fn release_source_local_checkpoint_restore_after(&self, source_tick: Tick, deadline: Tick) {
        self.release_source_local_checkpoint_restore_reference(Some(source_tick), deadline);
    }

    fn release_source_local_checkpoint_restore_reference(
        &self,
        source_tick: Option<Tick>,
        deadline: Tick,
    ) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(references) = state
            .source_local_checkpoint_restore_deadlines
            .get_mut(&(source_tick, deadline))
        else {
            return;
        };
        *references -= 1;
        if *references == 0 {
            state
                .source_local_checkpoint_restore_deadlines
                .remove(&(source_tick, deadline));
        }
    }

    pub(crate) fn source_local_checkpoint_restore_blocks_drive(&self, now: Tick) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .source_local_checkpoint_restore_deadlines
            .retain(|(_, deadline), references| *references > 0 && *deadline >= now);
        state
            .source_local_checkpoint_restore_deadlines
            .iter()
            .any(|((source_tick, _), _)| source_tick.is_none())
    }

    pub fn source_local_checkpoint_restore_blocks_new_work(&self, now: Tick) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .source_local_checkpoint_restore_deadlines
            .retain(|(_, deadline), references| *references > 0 && *deadline >= now);
        !state.source_local_checkpoint_restore_deadlines.is_empty()
    }
}
