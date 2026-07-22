use crate::riscv_defaults::{
    MAX_RISCV_O3_ISSUE_WIDTH, MAX_RISCV_O3_MEMORY_ISSUE_WIDTH, MAX_RISCV_O3_WRITEBACK_WIDTH,
    MIN_RISCV_O3_ISSUE_WIDTH, MIN_RISCV_O3_MEMORY_ISSUE_WIDTH, MIN_RISCV_O3_WRITEBACK_WIDTH,
};

use super::O3RuntimeState;

impl O3RuntimeState {
    pub(crate) const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub(crate) fn set_issue_width(&mut self, issue_width: usize) -> bool {
        if !(MIN_RISCV_O3_ISSUE_WIDTH..=MAX_RISCV_O3_ISSUE_WIDTH).contains(&issue_width) {
            return false;
        }
        if issue_width < self.memory_issue_width {
            return false;
        }
        self.issue_width = issue_width;
        true
    }

    pub(crate) const fn memory_issue_width(&self) -> usize {
        self.memory_issue_width
    }

    pub(crate) fn set_memory_issue_width(&mut self, memory_issue_width: usize) -> bool {
        if !(MIN_RISCV_O3_MEMORY_ISSUE_WIDTH..=MAX_RISCV_O3_MEMORY_ISSUE_WIDTH)
            .contains(&memory_issue_width)
        {
            return false;
        }
        if memory_issue_width > self.issue_width {
            return false;
        }
        self.memory_issue_width = memory_issue_width;
        true
    }

    pub(crate) const fn writeback_width(&self) -> usize {
        self.snapshot
            .pending_state()
            .writeback()
            .policy()
            .writeback_width()
    }

    pub(crate) fn set_writeback_width(&mut self, writeback_width: usize) -> bool {
        if !(MIN_RISCV_O3_WRITEBACK_WIDTH..=MAX_RISCV_O3_WRITEBACK_WIDTH).contains(&writeback_width)
        {
            return false;
        }
        if !self.writeback_calendar.is_empty() {
            return false;
        }

        self.rebuild_writeback_policy(writeback_width)
            .expect("rebuilt O3 pending-state snapshot is valid");
        debug_assert_eq!(self.writeback_width(), writeback_width);
        true
    }
}
