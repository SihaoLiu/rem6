use rem6_memory::MemoryRequestId;

use crate::{DramBankState, DramRefreshTiming, DramWaitRecord};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DramRefreshWindow {
    request: Option<MemoryRequestId>,
    parallel_port: u32,
    local_bank: u32,
    wait_cycle: u64,
    due_through_cycle: u64,
}

impl DramRefreshWindow {
    pub(crate) const fn new(
        request: MemoryRequestId,
        parallel_port: u32,
        local_bank: u32,
        wait_cycle: u64,
        due_through_cycle: u64,
    ) -> Self {
        Self {
            request: Some(request),
            parallel_port,
            local_bank,
            wait_cycle,
            due_through_cycle,
        }
    }

    pub(crate) const fn maintenance(
        parallel_port: u32,
        local_bank: u32,
        due_through_cycle: u64,
    ) -> Self {
        Self {
            request: None,
            parallel_port,
            local_bank,
            wait_cycle: due_through_cycle,
            due_through_cycle,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramRefreshEvent {
    parallel_port: u32,
    bank: u32,
    start_cycle: u64,
    end_cycle: u64,
}

impl DramRefreshEvent {
    fn new(parallel_port: u32, bank: u32, start_cycle: u64, end_cycle: u64) -> Self {
        Self {
            parallel_port,
            bank,
            start_cycle,
            end_cycle,
        }
    }

    pub const fn parallel_port(&self) -> u32 {
        self.parallel_port
    }

    pub const fn bank(&self) -> u32 {
        self.bank
    }

    pub const fn start_cycle(&self) -> u64 {
        self.start_cycle
    }

    pub const fn end_cycle(&self) -> u64 {
        self.end_cycle
    }

    pub const fn cycle_count(&self) -> u64 {
        self.end_cycle.saturating_sub(self.start_cycle)
    }
}

pub(crate) fn record_due_refresh_events(
    refresh_timing: DramRefreshTiming,
    bank: &mut DramBankState,
    window: DramRefreshWindow,
    waits: &mut Vec<DramWaitRecord>,
) -> Vec<DramRefreshEvent> {
    if bank.next_refresh_cycle == 0 {
        bank.next_refresh_cycle =
            next_refresh_cycle_at_or_after(bank.available_cycle, refresh_timing.interval());
    }
    let mut events = Vec::new();
    while bank.next_refresh_cycle <= window.due_through_cycle {
        let due_cycle = bank.next_refresh_cycle;
        let start_cycle = due_cycle.max(bank.available_cycle);
        let end_cycle = start_cycle.saturating_add(refresh_timing.recovery());
        if end_cycle > window.wait_cycle {
            if let Some(request) = window.request {
                waits.push(DramWaitRecord::bank_queue(
                    request,
                    window.parallel_port,
                    window.local_bank,
                    window.wait_cycle.max(start_cycle),
                    end_cycle - 1,
                ));
            }
        }
        events.push(DramRefreshEvent::new(
            window.parallel_port,
            window.local_bank,
            start_cycle,
            end_cycle,
        ));
        bank.open_row = None;
        bank.available_cycle = bank.available_cycle.max(end_cycle);
        let next_refresh_cycle = due_cycle.saturating_add(refresh_timing.interval());
        bank.next_refresh_cycle = next_refresh_cycle;
        if next_refresh_cycle == due_cycle {
            break;
        }
    }
    events
}

pub(crate) fn record_due_all_bank_refresh_events(
    refresh_timing: DramRefreshTiming,
    banks: &mut [DramBankState],
    window: DramRefreshWindow,
    waits: &mut Vec<DramWaitRecord>,
) -> Vec<DramRefreshEvent> {
    for bank in banks.iter_mut() {
        if bank.next_refresh_cycle == 0 {
            bank.next_refresh_cycle =
                next_refresh_cycle_at_or_after(bank.available_cycle, refresh_timing.interval());
        }
    }

    let mut events = Vec::new();
    while let Some(due_cycle) = next_all_bank_refresh_cycle(banks) {
        if due_cycle > window.due_through_cycle {
            break;
        }
        let start_cycle = banks
            .iter()
            .fold(due_cycle, |cycle, bank| cycle.max(bank.available_cycle));
        let end_cycle = start_cycle.saturating_add(refresh_timing.recovery());
        if end_cycle > window.wait_cycle {
            if let Some(request) = window.request {
                waits.push(DramWaitRecord::bank_queue(
                    request,
                    window.parallel_port,
                    window.local_bank,
                    window.wait_cycle.max(start_cycle),
                    end_cycle - 1,
                ));
            }
        }

        for (local_bank, bank) in banks.iter_mut().enumerate() {
            events.push(DramRefreshEvent::new(
                window.parallel_port,
                local_bank as u32,
                start_cycle,
                end_cycle,
            ));
            bank.open_row = None;
            bank.available_cycle = bank.available_cycle.max(end_cycle);
            bank.next_refresh_cycle = due_cycle.saturating_add(refresh_timing.interval());
        }
        let next_refresh_cycle = due_cycle.saturating_add(refresh_timing.interval());
        if next_refresh_cycle == due_cycle {
            break;
        }
    }
    events
}

fn next_all_bank_refresh_cycle(banks: &[DramBankState]) -> Option<u64> {
    banks.iter().map(|bank| bank.next_refresh_cycle).min()
}

fn next_refresh_cycle_at_or_after(available_cycle: u64, interval: u64) -> u64 {
    if available_cycle == 0 {
        return interval;
    }
    let remainder = available_cycle % interval;
    if remainder == 0 {
        available_cycle
    } else {
        available_cycle.saturating_add(interval - remainder)
    }
}
