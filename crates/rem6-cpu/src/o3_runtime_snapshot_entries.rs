use rem6_memory::Address;

use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3ReorderBufferEntry {
    sequence: u64,
    pc: Address,
    destination: Option<O3PhysicalRegisterId>,
    rename_destination: Option<(O3RegisterClass, u32)>,
    ready: bool,
    ready_tick: u64,
    live_staged: bool,
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
            rename_destination: None,
            ready: false,
            ready_tick: 0,
            live_staged: false,
        }
    }

    pub const fn with_ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    pub const fn with_ready_tick(mut self, ready_tick: u64) -> Self {
        self.ready_tick = ready_tick;
        self
    }

    pub(crate) const fn with_live_staged_rename_destination(
        mut self,
        rename_destination: Option<(O3RegisterClass, u32)>,
    ) -> Self {
        self.rename_destination = rename_destination;
        self.live_staged = true;
        self
    }

    pub(super) fn mark_ready(&mut self) {
        self.ready = true;
    }

    pub(super) fn mark_ready_at(&mut self, ready_tick: u64) {
        self.ready = true;
        self.ready_tick = ready_tick;
    }

    pub(super) fn clear_live_staged_destination(&mut self) {
        self.destination = None;
        self.rename_destination = None;
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

    pub(crate) const fn rename_destination(self) -> Option<(O3RegisterClass, u32)> {
        self.rename_destination
    }

    pub const fn is_ready(self) -> bool {
        self.ready
    }

    pub const fn ready_tick(self) -> u64 {
        self.ready_tick
    }

    pub const fn is_live_staged(self) -> bool {
        self.live_staged
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

    pub(super) fn resolve_address(&mut self, address: Address) -> bool {
        match self.address {
            Some(existing) => existing == address,
            None => {
                self.address = Some(address);
                true
            }
        }
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
