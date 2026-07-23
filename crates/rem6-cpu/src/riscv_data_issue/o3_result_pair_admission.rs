use crate::riscv_execution_mode_handoff::RiscvO3LiveDataHandoffOperation;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3ResultPairProgress {
    Ordinary,
    Ready { issue_tick: Tick },
    WaitUntil(Tick),
    Blocked,
}

impl RiscvCore {
    pub(crate) fn translated_result_pair_progress(&self, now: Tick) -> O3ResultPairProgress {
        {
            let state = self.state.lock().expect("riscv core lock");
            if state.outstanding_data.is_empty() {
                return O3ResultPairProgress::Ordinary;
            }
        }
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
        if state.outstanding_data.is_empty() {
            return O3ResultPairProgress::Ordinary;
        }
        let Some((resident, forwarded, completed_partial, younger_rows)) =
            state.o3_runtime.live_scalar_memory_handoff()
        else {
            return O3ResultPairProgress::Blocked;
        };
        if resident.len() != 1
            || !forwarded.is_empty()
            || !completed_partial.is_empty()
            || younger_rows != 0
            || state.outstanding_data.len() != 1
        {
            return O3ResultPairProgress::Blocked;
        }
        let head = resident[0];
        let Some(outstanding) = state.outstanding_data.get(&head.data_request) else {
            return O3ResultPairProgress::Blocked;
        };
        let exact_span = access_size(&outstanding.access)
            .ok()
            .and_then(|size| {
                masked_vector_memory_request_span(
                    &outstanding.access,
                    Address::new(access_address(&outstanding.access)),
                    size,
                )
                .ok()
            })
            .is_some_and(|span| {
                outstanding.size == span.size && outstanding.request_byte_offset == span.byte_offset
            });
        if outstanding.request != head.data_request
            || outstanding.fetch_request != head.fetch_request
            || outstanding.tick != head.issue_tick
            || head.operation != RiscvO3LiveDataHandoffOperation::Load
            || !matches!(&outstanding.target, RiscvDataAccessTarget::Memory { .. })
            || !matches!(
                state
                    .pma
                    .is_uncacheable(outstanding.physical_address.get(), outstanding.size.bytes(),),
                Ok(false)
            )
            || !exact_span
            || !matches!(
                outstanding.access,
                rem6_isa_riscv::MemoryAccessKind::Load {
                    rd,
                    width: rem6_isa_riscv::MemoryWidth::Doubleword,
                    ..
                } if !rd.is_zero()
            )
            || !state.o3_runtime.matches_exact_memory_result_head(
                head.fetch_request,
                head.data_request,
                head.issue_tick,
                head.o3_sequence,
                &outstanding.access,
            )
            || !state.has_exact_translated_result_pair_window(
                &fetch_events,
                head.fetch_request,
                head.o3_sequence,
            )
        {
            return O3ResultPairProgress::Blocked;
        }
        let Some(issue_tick) = state.o3_runtime.next_memory_result_issue_tick(now) else {
            return O3ResultPairProgress::Blocked;
        };
        if issue_tick <= now {
            O3ResultPairProgress::Ready { issue_tick }
        } else {
            O3ResultPairProgress::WaitUntil(issue_tick)
        }
    }
}
