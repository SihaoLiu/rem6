use std::error::Error;
use std::fmt;

use rem6_memory::Address;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReturnAddressStackError {
    ZeroEntries,
    SnapshotEntriesMismatch {
        expected: usize,
        actual: usize,
    },
    UnknownOperation {
        id: ReturnAddressStackOperationId,
    },
    OutOfOrderOperationCommit {
        expected: ReturnAddressStackOperationId,
        actual: ReturnAddressStackOperationId,
    },
}

impl fmt::Display for ReturnAddressStackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEntries => write!(formatter, "return address stack is empty"),
            Self::SnapshotEntriesMismatch { expected, actual } => write!(
                formatter,
                "return address stack snapshot has {actual} entries but stack has {expected}"
            ),
            Self::UnknownOperation { id } => write!(
                formatter,
                "return address stack operation {} is not pending",
                id.get()
            ),
            Self::OutOfOrderOperationCommit { expected, actual } => write!(
                formatter,
                "return address stack operation {} cannot commit before pending operation {}",
                actual.get(),
                expected.get()
            ),
        }
    }
}

impl Error for ReturnAddressStackError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackConfig {
    entries: usize,
}

impl ReturnAddressStackConfig {
    pub fn new(entries: usize) -> Result<Self, ReturnAddressStackError> {
        if entries == 0 {
            return Err(ReturnAddressStackError::ZeroEntries);
        }

        Ok(Self { entries })
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStack {
    config: ReturnAddressStackConfig,
    stack: Vec<Address>,
    next_operation: ReturnAddressStackOperationId,
    pending_operations: Vec<ReturnAddressStackOperation>,
}

impl ReturnAddressStack {
    pub fn new(config: ReturnAddressStackConfig) -> Self {
        Self {
            config,
            stack: Vec::new(),
            next_operation: ReturnAddressStackOperationId::new(0),
            pending_operations: Vec::new(),
        }
    }

    pub const fn config(&self) -> &ReturnAddressStackConfig {
        &self.config
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn top(&self) -> Option<Address> {
        self.stack.last().copied()
    }

    pub fn stack_entries(&self) -> &[Address] {
        &self.stack
    }

    pub const fn next_operation(&self) -> ReturnAddressStackOperationId {
        self.next_operation
    }

    pub fn pending_operations(&self) -> &[ReturnAddressStackOperation] {
        &self.pending_operations
    }

    pub fn pending_operation_count(&self) -> usize {
        self.pending_operations.len()
    }

    pub fn push_speculative(&mut self, return_address: Address) -> ReturnAddressStackOperation {
        let stack_before = self.stack.clone();
        if self.stack.len() == self.config.entries() {
            self.stack.remove(0);
        }
        self.stack.push(return_address);
        let stack_after = self.stack.clone();

        self.record_operation(
            ReturnAddressStackOperationKind::Push,
            Some(return_address),
            None,
            stack_before,
            stack_after,
        )
    }

    pub fn pop_speculative(&mut self) -> ReturnAddressStackOperation {
        let stack_before = self.stack.clone();
        let predicted_return = self.stack.pop();
        let stack_after = self.stack.clone();

        self.record_operation(
            ReturnAddressStackOperationKind::Pop,
            None,
            predicted_return,
            stack_before,
            stack_after,
        )
    }

    pub fn pop_then_push_speculative(
        &mut self,
        return_address: Address,
    ) -> ReturnAddressStackOperation {
        let stack_before = self.stack.clone();
        let predicted_return = self.stack.pop();
        if self.stack.len() == self.config.entries() {
            self.stack.remove(0);
        }
        self.stack.push(return_address);
        let stack_after = self.stack.clone();

        self.record_operation(
            ReturnAddressStackOperationKind::PopThenPush,
            Some(return_address),
            predicted_return,
            stack_before,
            stack_after,
        )
    }

    pub fn commit_operation(
        &mut self,
        id: ReturnAddressStackOperationId,
    ) -> Result<ReturnAddressStackOperation, ReturnAddressStackError> {
        let Some(oldest) = self.pending_operations.first() else {
            return Err(ReturnAddressStackError::UnknownOperation { id });
        };

        if oldest.id() != id {
            return Err(ReturnAddressStackError::OutOfOrderOperationCommit {
                expected: oldest.id(),
                actual: id,
            });
        }

        Ok(self.pending_operations.remove(0))
    }

    pub fn squash_from(
        &mut self,
        id: ReturnAddressStackOperationId,
    ) -> Result<ReturnAddressStackRepair, ReturnAddressStackError> {
        let Some(index) = self
            .pending_operations
            .iter()
            .position(|operation| operation.id() == id)
        else {
            return Err(ReturnAddressStackError::UnknownOperation { id });
        };

        let mut removed = self.pending_operations.split_off(index);
        let reverted = removed.remove(0);
        let removed_youngers = removed;
        self.stack.clone_from(&reverted.stack_before);

        Ok(ReturnAddressStackRepair {
            restored_stack: self.stack.clone(),
            reverted,
            removed_youngers,
        })
    }

    pub fn snapshot(&self) -> ReturnAddressStackSnapshot {
        ReturnAddressStackSnapshot {
            config: self.config.clone(),
            stack: self.stack.clone(),
            next_operation: self.next_operation,
            pending_operations: self.pending_operations.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &ReturnAddressStackSnapshot,
    ) -> Result<(), ReturnAddressStackError> {
        if snapshot.config.entries() != self.config.entries() {
            return Err(ReturnAddressStackError::SnapshotEntriesMismatch {
                expected: self.config.entries(),
                actual: snapshot.config.entries(),
            });
        }

        self.stack.clone_from(&snapshot.stack);
        self.next_operation = snapshot.next_operation;
        self.pending_operations
            .clone_from(&snapshot.pending_operations);
        Ok(())
    }

    fn record_operation(
        &mut self,
        kind: ReturnAddressStackOperationKind,
        pushed_address: Option<Address>,
        predicted_return: Option<Address>,
        stack_before: Vec<Address>,
        stack_after: Vec<Address>,
    ) -> ReturnAddressStackOperation {
        let operation = ReturnAddressStackOperation {
            id: self.next_operation,
            kind,
            pushed_address,
            predicted_return,
            stack_before,
            stack_after,
        };
        self.next_operation = ReturnAddressStackOperationId::new(self.next_operation.get() + 1);
        self.pending_operations.push(operation.clone());
        operation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReturnAddressStackOperationId(u64);

impl ReturnAddressStackOperationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnAddressStackOperationKind {
    Push,
    Pop,
    PopThenPush,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackOperation {
    id: ReturnAddressStackOperationId,
    kind: ReturnAddressStackOperationKind,
    pushed_address: Option<Address>,
    predicted_return: Option<Address>,
    stack_before: Vec<Address>,
    stack_after: Vec<Address>,
}

impl ReturnAddressStackOperation {
    pub(crate) fn from_checkpoint_parts(
        id: ReturnAddressStackOperationId,
        kind: ReturnAddressStackOperationKind,
        pushed_address: Option<Address>,
        predicted_return: Option<Address>,
        stack_before: Vec<Address>,
        stack_after: Vec<Address>,
    ) -> Self {
        Self {
            id,
            kind,
            pushed_address,
            predicted_return,
            stack_before,
            stack_after,
        }
    }

    pub const fn id(&self) -> ReturnAddressStackOperationId {
        self.id
    }

    pub const fn kind(&self) -> ReturnAddressStackOperationKind {
        self.kind
    }

    pub const fn pushed_address(&self) -> Option<Address> {
        self.pushed_address
    }

    pub const fn predicted_return(&self) -> Option<Address> {
        self.predicted_return
    }

    pub fn stack_before(&self) -> &[Address] {
        &self.stack_before
    }

    pub fn stack_after(&self) -> &[Address] {
        &self.stack_after
    }

    pub fn depth_before(&self) -> usize {
        self.stack_before.len()
    }

    pub fn depth_after(&self) -> usize {
        self.stack_after.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackRepair {
    restored_stack: Vec<Address>,
    reverted: ReturnAddressStackOperation,
    removed_youngers: Vec<ReturnAddressStackOperation>,
}

impl ReturnAddressStackRepair {
    pub fn restored_stack(&self) -> &[Address] {
        &self.restored_stack
    }

    pub const fn reverted(&self) -> &ReturnAddressStackOperation {
        &self.reverted
    }

    pub fn removed_youngers(&self) -> &[ReturnAddressStackOperation] {
        &self.removed_youngers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackSnapshot {
    config: ReturnAddressStackConfig,
    stack: Vec<Address>,
    next_operation: ReturnAddressStackOperationId,
    pending_operations: Vec<ReturnAddressStackOperation>,
}

impl ReturnAddressStackSnapshot {
    pub(crate) fn from_checkpoint_parts(
        config: ReturnAddressStackConfig,
        stack: Vec<Address>,
        next_operation: ReturnAddressStackOperationId,
        pending_operations: Vec<ReturnAddressStackOperation>,
    ) -> Self {
        Self {
            config,
            stack,
            next_operation,
            pending_operations,
        }
    }

    pub const fn config(&self) -> &ReturnAddressStackConfig {
        &self.config
    }

    pub fn stack_entries(&self) -> &[Address] {
        &self.stack
    }

    pub const fn next_operation(&self) -> ReturnAddressStackOperationId {
        self.next_operation
    }

    pub fn pending_operations(&self) -> &[ReturnAddressStackOperation] {
        &self.pending_operations
    }
}
