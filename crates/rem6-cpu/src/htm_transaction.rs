use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HtmTransactionUid(u64);

impl HtmTransactionUid {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HtmFailureCause {
    Explicit,
    Nest,
    Size,
    Exception,
    Memory,
    Other,
}

impl fmt::Display for HtmFailureCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Explicit => write!(formatter, "explicit"),
            Self::Nest => write!(formatter, "nesting_limit"),
            Self::Size => write!(formatter, "transaction_size"),
            Self::Exception => write!(formatter, "exception"),
            Self::Memory => write!(formatter, "memory_conflict"),
            Self::Other => write!(formatter, "other"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmBeginRecord {
    uid: HtmTransactionUid,
    depth: u64,
}

impl HtmBeginRecord {
    pub const fn uid(&self) -> HtmTransactionUid {
        self.uid
    }

    pub const fn depth(&self) -> u64 {
        self.depth
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmCommitRecord {
    uid: HtmTransactionUid,
    depth: u64,
}

impl HtmCommitRecord {
    pub const fn uid(&self) -> HtmTransactionUid {
        self.uid
    }

    pub const fn depth(&self) -> u64 {
        self.depth
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmAbortRecord {
    uid: HtmTransactionUid,
    cause: HtmFailureCause,
    restored_checkpoint: Vec<u8>,
}

impl HtmAbortRecord {
    pub const fn uid(&self) -> HtmTransactionUid {
        self.uid
    }

    pub const fn cause(&self) -> HtmFailureCause {
        self.cause
    }

    pub fn restored_checkpoint(&self) -> &[u8] {
        &self.restored_checkpoint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmTransactionSnapshot {
    next_uid: u64,
    active: Option<HtmActiveTransactionSnapshot>,
    last_abort: Option<HtmAbortRecord>,
}

impl HtmTransactionSnapshot {
    pub const fn next_uid(&self) -> u64 {
        self.next_uid
    }

    pub const fn active(&self) -> Option<&HtmActiveTransactionSnapshot> {
        self.active.as_ref()
    }

    pub const fn last_abort(&self) -> Option<&HtmAbortRecord> {
        self.last_abort.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmActiveTransactionSnapshot {
    uid: HtmTransactionUid,
    depth: u64,
    checkpoint: Vec<u8>,
}

impl HtmActiveTransactionSnapshot {
    pub const fn uid(&self) -> HtmTransactionUid {
        self.uid
    }

    pub const fn depth(&self) -> u64 {
        self.depth
    }

    pub fn checkpoint(&self) -> &[u8] {
        &self.checkpoint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmTransactionState {
    next_uid: u64,
    active: Option<HtmActiveTransaction>,
    last_abort: Option<HtmAbortRecord>,
}

impl HtmTransactionState {
    pub const fn new() -> Self {
        Self {
            next_uid: 1,
            active: None,
            last_abort: None,
        }
    }

    pub fn restore(snapshot: HtmTransactionSnapshot) -> Result<Self, HtmTransactionError> {
        if snapshot.next_uid == 0 {
            return Err(HtmTransactionError::InvalidNextUid {
                next_uid: snapshot.next_uid,
            });
        }
        let active = snapshot
            .active
            .map(HtmActiveTransaction::restore)
            .transpose()?;

        Ok(Self {
            next_uid: snapshot.next_uid,
            active,
            last_abort: snapshot.last_abort,
        })
    }

    pub fn snapshot(&self) -> HtmTransactionSnapshot {
        HtmTransactionSnapshot {
            next_uid: self.next_uid,
            active: self.active.as_ref().map(HtmActiveTransaction::snapshot),
            last_abort: self.last_abort.clone(),
        }
    }

    pub fn begin(&mut self, checkpoint: Vec<u8>) -> Result<HtmBeginRecord, HtmTransactionError> {
        self.last_abort = None;
        let active = match &mut self.active {
            Some(active) => {
                active.depth =
                    active
                        .depth
                        .checked_add(1)
                        .ok_or(HtmTransactionError::DepthOverflow {
                            uid: active.uid,
                            depth: active.depth,
                        })?;
                active
            }
            None => {
                let uid = HtmTransactionUid::new(self.next_uid);
                self.next_uid =
                    self.next_uid
                        .checked_add(1)
                        .ok_or(HtmTransactionError::UidOverflow {
                            uid: HtmTransactionUid::new(self.next_uid),
                        })?;
                self.active.insert(HtmActiveTransaction {
                    uid,
                    depth: 1,
                    checkpoint,
                })
            }
        };

        Ok(HtmBeginRecord {
            uid: active.uid,
            depth: active.depth,
        })
    }

    pub fn commit(
        &mut self,
        uid: HtmTransactionUid,
    ) -> Result<HtmCommitRecord, HtmTransactionError> {
        let active = self
            .active
            .as_mut()
            .ok_or(HtmTransactionError::NoActiveTransaction)?;
        active.validate_uid(uid)?;
        active.depth -= 1;
        let depth = active.depth;
        if depth == 0 {
            self.active = None;
        }
        Ok(HtmCommitRecord { uid, depth })
    }

    pub fn abort(
        &mut self,
        uid: HtmTransactionUid,
        cause: HtmFailureCause,
    ) -> Result<HtmAbortRecord, HtmTransactionError> {
        let active = self
            .active
            .as_ref()
            .ok_or(HtmTransactionError::NoActiveTransaction)?;
        active.validate_uid(uid)?;
        let record = HtmAbortRecord {
            uid,
            cause,
            restored_checkpoint: active.checkpoint.clone(),
        };
        self.active = None;
        self.last_abort = Some(record.clone());
        Ok(record)
    }

    pub const fn in_transaction(&self) -> bool {
        self.active.is_some()
    }

    pub const fn active_uid(&self) -> Option<HtmTransactionUid> {
        match &self.active {
            Some(active) => Some(active.uid),
            None => None,
        }
    }

    pub const fn active_depth(&self) -> Option<u64> {
        match &self.active {
            Some(active) => Some(active.depth),
            None => None,
        }
    }

    pub const fn last_abort(&self) -> Option<&HtmAbortRecord> {
        self.last_abort.as_ref()
    }
}

impl Default for HtmTransactionState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HtmActiveTransaction {
    uid: HtmTransactionUid,
    depth: u64,
    checkpoint: Vec<u8>,
}

impl HtmActiveTransaction {
    fn restore(snapshot: HtmActiveTransactionSnapshot) -> Result<Self, HtmTransactionError> {
        if snapshot.depth == 0 {
            return Err(HtmTransactionError::InvalidActiveDepth {
                uid: snapshot.uid,
                depth: snapshot.depth,
            });
        }
        Ok(Self {
            uid: snapshot.uid,
            depth: snapshot.depth,
            checkpoint: snapshot.checkpoint,
        })
    }

    fn snapshot(&self) -> HtmActiveTransactionSnapshot {
        HtmActiveTransactionSnapshot {
            uid: self.uid,
            depth: self.depth,
            checkpoint: self.checkpoint.clone(),
        }
    }

    fn validate_uid(&self, actual: HtmTransactionUid) -> Result<(), HtmTransactionError> {
        if actual != self.uid {
            return Err(HtmTransactionError::TransactionUidMismatch {
                expected: self.uid,
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HtmTransactionError {
    NoActiveTransaction,
    TransactionUidMismatch {
        expected: HtmTransactionUid,
        actual: HtmTransactionUid,
    },
    InvalidNextUid {
        next_uid: u64,
    },
    InvalidActiveDepth {
        uid: HtmTransactionUid,
        depth: u64,
    },
    UidOverflow {
        uid: HtmTransactionUid,
    },
    DepthOverflow {
        uid: HtmTransactionUid,
        depth: u64,
    },
    MissingArchitecturalCheckpoint {
        uid: HtmTransactionUid,
    },
}

impl fmt::Display for HtmTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoActiveTransaction => write!(formatter, "no HTM transaction is active"),
            Self::TransactionUidMismatch { expected, actual } => write!(
                formatter,
                "active HTM transaction uid {} does not match requested uid {}",
                expected.get(),
                actual.get()
            ),
            Self::InvalidNextUid { next_uid } => {
                write!(formatter, "HTM next uid {next_uid} is invalid")
            }
            Self::InvalidActiveDepth { uid, depth } => write!(
                formatter,
                "active HTM transaction uid {} has invalid depth {depth}",
                uid.get()
            ),
            Self::UidOverflow { uid } => {
                write!(
                    formatter,
                    "HTM transaction uid overflow after {}",
                    uid.get()
                )
            }
            Self::DepthOverflow { uid, depth } => write!(
                formatter,
                "HTM transaction uid {} depth overflow after {depth}",
                uid.get()
            ),
            Self::MissingArchitecturalCheckpoint { uid } => write!(
                formatter,
                "HTM transaction uid {} has no architectural checkpoint",
                uid.get()
            ),
        }
    }
}

impl Error for HtmTransactionError {}
