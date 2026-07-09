use rem6_memory::Address;

use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3ReorderBufferEntry {
    sequence: u64,
    pc: Address,
    destination: Option<O3PhysicalRegisterId>,
    ready: bool,
}

impl O3ReorderBufferEntry {
    pub const fn new(
        sequence: u64,
        pc: Address,
        destination: Option<O3PhysicalRegisterId>,
    ) -> Self {
        Self {
            sequence,
            pc,
            destination,
            ready: false,
        }
    }

    pub const fn with_ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    pub(super) fn mark_ready(&mut self) {
        self.ready = true;
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn pc(self) -> Address {
        self.pc
    }

    pub const fn destination(self) -> Option<O3PhysicalRegisterId> {
        self.destination
    }

    pub const fn is_ready(self) -> bool {
        self.ready
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LoadStoreQueueKind {
    Load,
    Store,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LoadStoreQueueEntry {
    sequence: u64,
    address: Option<Address>,
    bytes: u32,
    kind: O3LoadStoreQueueKind,
    completed: bool,
}

impl O3LoadStoreQueueEntry {
    pub const fn load(sequence: u64, address: Option<Address>, bytes: u32) -> Self {
        Self {
            sequence,
            address,
            bytes,
            kind: O3LoadStoreQueueKind::Load,
            completed: false,
        }
    }

    pub const fn store(sequence: u64, address: Option<Address>, bytes: u32) -> Self {
        Self {
            sequence,
            address,
            bytes,
            kind: O3LoadStoreQueueKind::Store,
            completed: false,
        }
    }

    pub const fn with_completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    pub(super) fn mark_completed(&mut self) {
        self.completed = true;
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub const fn kind(self) -> O3LoadStoreQueueKind {
        self.kind
    }

    pub const fn is_completed(self) -> bool {
        self.completed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RenameMapEntry {
    register_class: O3RegisterClass,
    architectural: u32,
    physical: O3PhysicalRegisterId,
}

impl O3RenameMapEntry {
    pub const fn new(
        register_class: O3RegisterClass,
        architectural: u32,
        physical: O3PhysicalRegisterId,
    ) -> Self {
        Self {
            register_class,
            architectural,
            physical,
        }
    }

    pub const fn register_class(self) -> O3RegisterClass {
        self.register_class
    }

    pub const fn architectural(self) -> u32 {
        self.architectural
    }

    pub const fn physical(self) -> O3PhysicalRegisterId {
        self.physical
    }
}
