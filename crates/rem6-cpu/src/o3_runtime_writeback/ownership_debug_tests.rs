use super::*;

impl O3RuntimeState {
    pub(crate) fn writeback_partial_ownership_debug(&self) -> (usize, usize, usize, bool) {
        let live = self
            .live_writeback_schedule()
            .expect("live writeback schedule is coherent");
        let finalized = &self.finalized_writeback_port_stats;
        let reservations = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let is_published = |reservation: &O3WritebackReservation| {
            self.published_writeback_sequences
                .contains(&reservation.sequence)
        };
        let bounded = finalized.partial_finalized_cycle_ticks.iter().all(|tick| {
            reservations.iter().any(|reservation| {
                is_published(reservation)
                    && reservation.raw_ready_tick <= *tick
                    && *tick <= reservation.admitted_tick
            }) || (*tick < finalized.closed_before_tick && live.cycle_ticks.contains(tick))
        }) && finalized
            .partial_finalized_ready_rows_by_tick
            .keys()
            .all(|tick| {
                reservations.iter().any(|reservation| {
                    is_published(reservation) && reservation.raw_ready_tick == *tick
                }) || (*tick < finalized.closed_before_tick
                    && live.ready_rows_by_tick.contains_key(tick))
            })
            && finalized
                .partial_finalized_deferred_rows_by_tick
                .keys()
                .all(|tick| {
                    reservations.iter().any(|reservation| {
                        is_published(reservation)
                            && reservation.raw_ready_tick <= *tick
                            && *tick < reservation.admitted_tick
                    }) || (*tick < finalized.closed_before_tick
                        && live.deferred_rows_by_tick.contains_key(tick))
                });
        (
            finalized.partial_finalized_cycle_ticks.len(),
            finalized.partial_finalized_ready_rows_by_tick.len(),
            finalized.partial_finalized_deferred_rows_by_tick.len(),
            bounded,
        )
    }
}
