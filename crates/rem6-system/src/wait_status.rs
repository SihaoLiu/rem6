use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestWaitStatusError {
    InvalidSignal { signal: u8 },
}

impl fmt::Display for GuestWaitStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSignal { signal } => {
                write!(
                    formatter,
                    "guest signal {signal} is outside wait-status range"
                )
            }
        }
    }
}

impl Error for GuestWaitStatusError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestSignal(u8);

impl GuestSignal {
    pub const fn new(signal: u8) -> Result<Self, GuestWaitStatusError> {
        if signal == 0 || signal > 127 {
            return Err(GuestWaitStatusError::InvalidSignal { signal });
        }
        Ok(Self(signal))
    }

    pub const fn number(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GuestWaitStatus {
    Exited {
        code: u8,
    },
    Signaled {
        signal: GuestSignal,
        core_dumped: bool,
    },
    Stopped {
        signal: GuestSignal,
    },
    Continued,
}

impl GuestWaitStatus {
    pub const fn exited(code: u8) -> Self {
        Self::Exited { code }
    }

    pub const fn signaled(signal: GuestSignal, core_dumped: bool) -> Self {
        Self::Signaled {
            signal,
            core_dumped,
        }
    }

    pub const fn stopped(signal: GuestSignal) -> Self {
        Self::Stopped { signal }
    }

    pub const fn continued() -> Self {
        Self::Continued
    }

    pub const fn raw_wait_status(self) -> i32 {
        match self {
            Self::Exited { code } => (code as i32) << 8,
            Self::Signaled {
                signal,
                core_dumped,
            } => {
                let core_bit = if core_dumped { 0x80 } else { 0 };
                (signal.number() as i32) | core_bit
            }
            Self::Stopped { signal } => ((signal.number() as i32) << 8) | 0x7f,
            Self::Continued => 0xffff,
        }
    }

    pub const fn is_exited(self) -> bool {
        matches!(self, Self::Exited { .. })
    }

    pub const fn exit_code(self) -> Option<u8> {
        match self {
            Self::Exited { code } => Some(code),
            _ => None,
        }
    }

    pub const fn is_signaled(self) -> bool {
        matches!(self, Self::Signaled { .. })
    }

    pub const fn terminating_signal(self) -> Option<GuestSignal> {
        match self {
            Self::Signaled { signal, .. } => Some(signal),
            _ => None,
        }
    }

    pub const fn core_dumped(self) -> bool {
        match self {
            Self::Signaled { core_dumped, .. } => core_dumped,
            _ => false,
        }
    }

    pub const fn is_stopped(self) -> bool {
        matches!(self, Self::Stopped { .. })
    }

    pub const fn stop_signal(self) -> Option<GuestSignal> {
        match self {
            Self::Stopped { signal } => Some(signal),
            _ => None,
        }
    }

    pub const fn is_continued(self) -> bool {
        matches!(self, Self::Continued)
    }
}
