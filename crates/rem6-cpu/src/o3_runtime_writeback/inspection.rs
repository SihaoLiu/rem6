use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RuntimeWritebackReservation {
    sequence: u64,
    raw_ready_tick: u64,
    admitted_tick: u64,
    slot: usize,
}

impl O3RuntimeWritebackReservation {
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn raw_ready_tick(self) -> u64 {
        self.raw_ready_tick
    }

    pub const fn admitted_tick(self) -> u64 {
        self.admitted_tick
    }

    pub const fn slot(self) -> usize {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3WritebackDebugState {
    width: usize,
    reserved_future_completions: usize,
    earliest_unpublished_tick: Option<u64>,
}

impl RiscvO3WritebackDebugState {
    const fn new(
        width: usize,
        reserved_future_completions: usize,
        earliest_unpublished_tick: Option<u64>,
    ) -> Self {
        Self {
            width,
            reserved_future_completions,
            earliest_unpublished_tick,
        }
    }

    pub const fn width(self) -> usize {
        self.width
    }

    pub const fn reserved_future_completions(self) -> usize {
        self.reserved_future_completions
    }

    pub const fn earliest_unpublished_tick(self) -> Option<u64> {
        self.earliest_unpublished_tick
    }
}

impl From<O3WritebackReservation> for O3RuntimeWritebackReservation {
    fn from(reservation: O3WritebackReservation) -> Self {
        Self {
            sequence: reservation.sequence,
            raw_ready_tick: reservation.raw_ready_tick,
            admitted_tick: reservation.admitted_tick,
            slot: reservation.slot,
        }
    }
}

impl O3RuntimeState {
    pub(crate) fn writeback_debug_state(&self, now: u64) -> RiscvO3WritebackDebugState {
        RiscvO3WritebackDebugState::new(
            self.snapshot
                .pending_state()
                .writeback()
                .policy()
                .writeback_width(),
            self.writeback_calendar.reserved_future_count(now),
            self.writeback_calendar.earliest_unpublished_tick(now),
        )
    }
}

impl crate::RiscvCore {
    pub fn o3_runtime_writeback_reservations(&self) -> Vec<O3RuntimeWritebackReservation> {
        self.with_o3_runtime(|runtime| runtime.writeback_calendar.snapshot())
    }
}
