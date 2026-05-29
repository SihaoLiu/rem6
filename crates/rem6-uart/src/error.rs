use std::error::Error;
use std::fmt;

use rem6_kernel::SchedulerError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UartError {
    EmptyReceiveQueue,
    Scheduler(SchedulerError),
}

impl fmt::Display for UartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyReceiveQueue => write!(formatter, "UART receive queue is empty"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for UartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pl011Error {
    DmaUnsupported,
    Scheduler(SchedulerError),
}

impl fmt::Display for Pl011Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DmaUnsupported => write!(formatter, "PL011 DMA is not supported"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for Pl011Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}
