use rem6_kernel::Tick;

use crate::{
    InterruptClaim, InterruptEvent, InterruptLineId, InterruptPriority, InterruptRoute,
    PendingInterrupt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptSnapshot {
    tick: Tick,
    routes: Vec<InterruptRoute>,
    priorities: Vec<(InterruptLineId, InterruptPriority)>,
    pending: Vec<PendingInterrupt>,
    claimed: Vec<InterruptClaim>,
    history: Vec<InterruptEvent>,
}

impl InterruptSnapshot {
    pub const fn new(
        tick: Tick,
        routes: Vec<InterruptRoute>,
        priorities: Vec<(InterruptLineId, InterruptPriority)>,
        pending: Vec<PendingInterrupt>,
        claimed: Vec<InterruptClaim>,
        history: Vec<InterruptEvent>,
    ) -> Self {
        Self {
            tick,
            routes,
            priorities,
            pending,
            claimed,
            history,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn routes(&self) -> &[InterruptRoute] {
        &self.routes
    }

    pub fn priorities(&self) -> &[(InterruptLineId, InterruptPriority)] {
        &self.priorities
    }

    pub fn pending(&self) -> &[PendingInterrupt] {
        &self.pending
    }

    pub fn claimed(&self) -> &[InterruptClaim] {
        &self.claimed
    }

    pub fn history(&self) -> &[InterruptEvent] {
        &self.history
    }
}
