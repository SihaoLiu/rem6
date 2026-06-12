use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    BranchUpdate, CpuFetchEvent, CpuFetchEventKind, CpuFetchRecord, RiscvCore, RiscvCoreState,
    RiscvCpuError, RiscvCpuExecutionEvent, RiscvGShareBranchUpdate, RISCV_LOCAL_GSHARE_THREAD,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvPendingFetchPrefix {
    fetch: CpuFetchEvent,
    bytes: [u8; 2],
}

impl RiscvPendingFetchPrefix {
    pub(crate) const fn new(fetch: CpuFetchEvent, bytes: [u8; 2]) -> Self {
        Self { fetch, bytes }
    }
}

impl RiscvCore {
    pub fn execute_next_completed_fetch(
        &self,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() {
            return Ok(None);
        }

        if let Some(prefix) = state.pending_fetch_prefix.clone() {
            let architectural = Address::new(state.hart.pc());
            if prefix.fetch.pc() != architectural {
                state.pending_fetch_prefix = None;
                return Err(RiscvCpuError::PcMismatch {
                    fetch: prefix.fetch.pc(),
                    architectural,
                });
            }
            let suffix_pc = Address::new(prefix.fetch.pc().get() + 2);
            let Some(suffix) = fetch_events.iter().find(|event| {
                event.kind() == CpuFetchEventKind::Completed
                    && event.pc() == suffix_pc
                    && !state.executed_fetches.contains(&event.request_id())
            }) else {
                return Ok(None);
            };
            let suffix_data = suffix.data().ok_or(RiscvCpuError::MissingFetchData {
                request: suffix.request_id(),
            })?;
            let [suffix_low, suffix_high] = suffix_data else {
                return Err(RiscvCpuError::InvalidFetchWidth {
                    request: suffix.request_id(),
                    bytes: suffix_data.len() as u64,
                });
            };
            let raw =
                u32::from_le_bytes([prefix.bytes[0], prefix.bytes[1], *suffix_low, *suffix_high]);
            let fetch = CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    prefix.fetch.tick(),
                    prefix.fetch.partition(),
                    prefix.fetch.route(),
                    prefix.fetch.endpoint().clone(),
                    prefix.fetch.request_id(),
                    prefix.fetch.pc(),
                    AccessSize::new(4).expect("RISC-V word fetch width is nonzero"),
                ),
                raw.to_le_bytes().to_vec(),
            );
            let consumed = [prefix.fetch.request_id(), suffix.request_id()];
            state.pending_fetch_prefix = None;
            return self
                .retire_completed_fetch(&mut state, fetch, raw, &consumed)
                .map(Some);
        }

        let Some(fetch) = fetch_events.into_iter().find(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
        }) else {
            return Ok(None);
        };

        let architectural = Address::new(state.hart.pc());
        if fetch.pc() != architectural {
            return Err(RiscvCpuError::PcMismatch {
                fetch: fetch.pc(),
                architectural,
            });
        }

        let data = fetch.data().ok_or(RiscvCpuError::MissingFetchData {
            request: fetch.request_id(),
        })?;
        let raw = match data {
            [low, high] if low & 0x3 != 0x3 => u32::from(u16::from_le_bytes([*low, *high])),
            [_, _] => {
                state.pending_fetch_prefix = Some(RiscvPendingFetchPrefix::new(
                    fetch.clone(),
                    [data[0], data[1]],
                ));
                state.executed_fetches.insert(fetch.request_id());
                return Ok(None);
            }
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => {
                return Err(RiscvCpuError::InvalidFetchWidth {
                    request: fetch.request_id(),
                    bytes: data.len() as u64,
                });
            }
        };
        self.retire_completed_fetch(&mut state, fetch.clone(), raw, &[fetch.request_id()])
            .map(Some)
    }

    fn retire_completed_fetch(
        &self,
        state: &mut RiscvCoreState,
        fetch: CpuFetchEvent,
        raw: u32,
        consumed_requests: &[MemoryRequestId],
    ) -> Result<RiscvCpuExecutionEvent, RiscvCpuError> {
        let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
        let instruction = decoded.instruction();
        let execution = state
            .hart
            .execute_decoded(decoded)
            .map_err(RiscvCpuError::Isa)?;
        let next_pc = Address::new(execution.next_pc());
        self.core.set_pc(next_pc);
        if let Some(trap) = execution.trap().copied() {
            state.pending_trap = Some(trap);
        }
        state.apply_riscv_system_event(execution.system_event());
        let retired_branch = retire_branch_predictions(state, fetch.pc(), instruction, &execution)?;

        let event = RiscvCpuExecutionEvent::with_branch_updates(
            fetch.clone(),
            instruction,
            execution,
            retired_branch.branch_update,
            retired_branch.gshare_branch_update,
        );
        state
            .executed_fetches
            .extend(consumed_requests.iter().copied());
        state.events.push(event.clone());
        Ok(event)
    }
}

struct RetiredBranchUpdates {
    branch_update: Option<BranchUpdate>,
    gshare_branch_update: Option<RiscvGShareBranchUpdate>,
}

fn retire_branch_predictions(
    state: &mut RiscvCoreState,
    pc: Address,
    instruction: RiscvInstruction,
    execution: &rem6_isa_riscv::RiscvExecutionRecord,
) -> Result<RetiredBranchUpdates, RiscvCpuError> {
    if execution.trap().is_some() {
        return Ok(RetiredBranchUpdates {
            branch_update: None,
            gshare_branch_update: None,
        });
    }

    let sequential_pc = pc
        .get()
        .wrapping_add(u64::from(execution.instruction_bytes()));
    let next_pc = execution.next_pc();
    let (conditional, actual_taken, actual_target) = match instruction {
        RiscvInstruction::Beq { .. }
        | RiscvInstruction::Bne { .. }
        | RiscvInstruction::Blt { .. }
        | RiscvInstruction::Bge { .. }
        | RiscvInstruction::Bltu { .. }
        | RiscvInstruction::Bgeu { .. } => {
            let taken = next_pc != sequential_pc;
            (true, taken, taken.then_some(Address::new(next_pc)))
        }
        RiscvInstruction::Jal { .. } | RiscvInstruction::Jalr { .. } => {
            (false, true, Some(Address::new(next_pc)))
        }
        _ => {
            return Ok(RetiredBranchUpdates {
                branch_update: None,
                gshare_branch_update: None,
            });
        }
    };

    let branch_update = state
        .branch_predictor
        .update(pc, actual_taken, actual_target);
    let prediction = if conditional {
        state
            .gshare_branch_predictor
            .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
    } else {
        state
            .gshare_branch_predictor
            .predict_unconditional(RISCV_LOCAL_GSHARE_THREAD, pc)
    }
    .map_err(RiscvCpuError::GShareBranchPredictor)?;
    let history_update = state
        .gshare_branch_predictor
        .update_history(prediction.history(), actual_taken)
        .map_err(RiscvCpuError::GShareBranchPredictor)?;
    let training_update = state
        .gshare_branch_predictor
        .train(prediction.history(), actual_taken, false)
        .map_err(RiscvCpuError::GShareBranchPredictor)?;

    Ok(RetiredBranchUpdates {
        branch_update: Some(branch_update),
        gshare_branch_update: Some(RiscvGShareBranchUpdate::new(
            prediction,
            history_update,
            training_update,
        )),
    })
}
