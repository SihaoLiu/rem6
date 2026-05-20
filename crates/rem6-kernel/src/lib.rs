mod clock;
mod event;

pub use clock::{ClockDomain, ClockError, Cycles};
pub use event::{ClockScheduleError, EventId, EventQueue, ScheduleError};

pub type Tick = u64;
