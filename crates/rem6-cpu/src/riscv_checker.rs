use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvError, RiscvExecutionRecord, RiscvHartState};
use rem6_memory::Address;

use crate::{RiscvCore, RiscvCoreState};

pub(crate) fn sync_checker_hart(state: &mut RiscvCoreState) {
    let hart = state.hart.clone();
    if let Some(checker) = &mut state.checker {
        checker.sync_hart(&hart);
    }
}

impl RiscvCore {
    pub fn enable_checker_cpu(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.checker = Some(RiscvCheckerCpu::new(state.hart.clone()));
    }

    pub fn enable_checker_cpu_with_hart(&self, hart: RiscvHartState) {
        self.state.lock().expect("riscv core lock").checker = Some(RiscvCheckerCpu::new(hart));
    }

    pub fn checker_cpu_snapshot(&self) -> Option<RiscvCheckerSnapshot> {
        self.state
            .lock()
            .expect("riscv core lock")
            .checker
            .as_ref()
            .map(RiscvCheckerCpu::snapshot)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCheckerMismatch {
    sequence: u64,
    pc: Address,
    primary_execution: RiscvExecutionRecord,
    checker_execution: RiscvExecutionRecord,
    primary_hart: RiscvHartState,
    checker_hart: RiscvHartState,
}

impl RiscvCheckerMismatch {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn primary_execution(&self) -> &RiscvExecutionRecord {
        &self.primary_execution
    }

    pub const fn checker_execution(&self) -> &RiscvExecutionRecord {
        &self.checker_execution
    }

    pub const fn primary_hart(&self) -> &RiscvHartState {
        &self.primary_hart
    }

    pub const fn checker_hart(&self) -> &RiscvHartState {
        &self.checker_hart
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCheckerSnapshot {
    hart: RiscvHartState,
    checked_instructions: u64,
    mismatches: Vec<RiscvCheckerMismatch>,
}

impl RiscvCheckerSnapshot {
    pub const fn hart(&self) -> &RiscvHartState {
        &self.hart
    }

    pub const fn checked_instructions(&self) -> u64 {
        self.checked_instructions
    }

    pub fn mismatches(&self) -> &[RiscvCheckerMismatch] {
        &self.mismatches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvCheckerCpu {
    hart: RiscvHartState,
    checked_instructions: u64,
    mismatches: Vec<RiscvCheckerMismatch>,
}

impl RiscvCheckerCpu {
    pub(crate) const fn new(hart: RiscvHartState) -> Self {
        Self {
            hart,
            checked_instructions: 0,
            mismatches: Vec::new(),
        }
    }

    pub(crate) fn check_retired(
        &mut self,
        sequence: u64,
        pc: Address,
        decoded: RiscvDecodedInstruction,
        primary_execution: &RiscvExecutionRecord,
        primary_hart: &RiscvHartState,
    ) -> Result<(), RiscvError> {
        let checker_execution = self.hart.execute_decoded(decoded)?;
        self.checked_instructions += 1;
        if &checker_execution != primary_execution || &self.hart != primary_hart {
            self.mismatches.push(RiscvCheckerMismatch {
                sequence,
                pc,
                primary_execution: primary_execution.clone(),
                checker_execution,
                primary_hart: primary_hart.clone(),
                checker_hart: self.hart.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> RiscvCheckerSnapshot {
        RiscvCheckerSnapshot {
            hart: self.hart.clone(),
            checked_instructions: self.checked_instructions,
            mismatches: self.mismatches.clone(),
        }
    }

    pub(crate) fn sync_hart(&mut self, hart: &RiscvHartState) {
        self.hart = hart.clone();
    }
}
