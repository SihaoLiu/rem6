use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestWaitStatusError {
    InvalidSignal { signal: u8 },
    InvalidProcessId { pid: u32 },
    InvalidProcessGroupId { pgid: u32 },
    InvalidWaitPid { pid: i32 },
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
            Self::InvalidProcessId { pid } => write!(formatter, "invalid guest process id {pid}"),
            Self::InvalidProcessGroupId { pgid } => {
                write!(formatter, "invalid guest process group id {pgid}")
            }
            Self::InvalidWaitPid { pid } => write!(formatter, "invalid guest wait pid {pid}"),
        }
    }
}

impl Error for GuestWaitStatusError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestProcessId(u32);

impl GuestProcessId {
    pub const fn new(pid: u32) -> Result<Self, GuestWaitStatusError> {
        if pid == 0 {
            return Err(GuestWaitStatusError::InvalidProcessId { pid });
        }
        Ok(Self(pid))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestProcessGroupId(u32);

impl GuestProcessGroupId {
    pub const fn new(pgid: u32) -> Result<Self, GuestWaitStatusError> {
        if pgid == 0 {
            return Err(GuestWaitStatusError::InvalidProcessGroupId { pgid });
        }
        Ok(Self(pgid))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestChildStatus {
    pid: GuestProcessId,
    process_group: GuestProcessGroupId,
    status: GuestWaitStatus,
}

impl GuestChildStatus {
    pub const fn new(
        pid: GuestProcessId,
        process_group: GuestProcessGroupId,
        status: GuestWaitStatus,
    ) -> Self {
        Self {
            pid,
            process_group,
            status,
        }
    }

    pub const fn pid(self) -> GuestProcessId {
        self.pid
    }

    pub const fn process_group(self) -> GuestProcessGroupId {
        self.process_group
    }

    pub const fn status(self) -> GuestWaitStatus {
        self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestWaitSelector {
    AnyChild,
    CurrentProcessGroup,
    Process(GuestProcessId),
    ProcessGroup(GuestProcessGroupId),
}

impl GuestWaitSelector {
    pub fn from_wait4_pid(pid: i32) -> Result<Self, GuestWaitStatusError> {
        match pid {
            -1 => Ok(Self::AnyChild),
            0 => Ok(Self::CurrentProcessGroup),
            pid if pid < -1 => {
                let process_group = pid
                    .checked_neg()
                    .ok_or(GuestWaitStatusError::InvalidWaitPid { pid })?
                    as u32;
                Ok(Self::ProcessGroup(GuestProcessGroupId::new(process_group)?))
            }
            pid => Ok(Self::Process(GuestProcessId::new(pid as u32)?)),
        }
    }

    fn matches(self, child: GuestChildStatus, current_process_group: GuestProcessGroupId) -> bool {
        match self {
            Self::AnyChild => true,
            Self::CurrentProcessGroup => child.process_group() == current_process_group,
            Self::Process(pid) => child.pid() == pid,
            Self::ProcessGroup(process_group) => child.process_group() == process_group,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestWaitOptions {
    nonblocking: bool,
}

impl GuestWaitOptions {
    pub const fn blocking() -> Self {
        Self { nonblocking: false }
    }

    pub const fn nonblocking() -> Self {
        Self { nonblocking: true }
    }

    pub const fn is_nonblocking(self) -> bool {
        self.nonblocking
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestWaitOutcome {
    Ready(GuestChildStatus),
    NoReady,
    Retry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestWaitQueue {
    current_process_group: GuestProcessGroupId,
    pending: Vec<GuestChildStatus>,
}

impl GuestWaitQueue {
    pub const fn new(current_process_group: GuestProcessGroupId) -> Self {
        Self {
            current_process_group,
            pending: Vec::new(),
        }
    }

    pub fn push(&mut self, child: GuestChildStatus) {
        self.pending.push(child);
    }

    pub fn wait(
        &mut self,
        selector: GuestWaitSelector,
        options: GuestWaitOptions,
    ) -> GuestWaitOutcome {
        if let Some(index) = self
            .pending
            .iter()
            .position(|child| selector.matches(*child, self.current_process_group))
        {
            return GuestWaitOutcome::Ready(self.pending.remove(index));
        }

        if options.is_nonblocking() {
            GuestWaitOutcome::NoReady
        } else {
            GuestWaitOutcome::Retry
        }
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
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
