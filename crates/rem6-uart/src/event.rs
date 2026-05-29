use rem6_interrupt::{InterruptError, InterruptEventKind, InterruptSourceId};
use rem6_kernel::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UartId(u64);

impl UartId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartTxByte {
    tick: Tick,
    byte: u8,
}

impl UartTxByte {
    pub const fn new(tick: Tick, byte: u8) -> Self {
        Self { tick, byte }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartRxByte {
    tick: Tick,
    byte: u8,
}

impl UartRxByte {
    pub const fn new(tick: Tick, byte: u8) -> Self {
        Self { tick, byte }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UartInterruptError {
    tick: Tick,
    source: InterruptSourceId,
    kind: InterruptEventKind,
    error: InterruptError,
}

impl UartInterruptError {
    pub const fn new(
        tick: Tick,
        source: InterruptSourceId,
        kind: InterruptEventKind,
        error: InterruptError,
    ) -> Self {
        Self {
            tick,
            source,
            kind,
            error,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn kind(&self) -> InterruptEventKind {
        self.kind
    }

    pub const fn error(&self) -> &InterruptError {
        &self.error
    }
}
