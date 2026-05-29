use rem6_kernel::Tick;
use rem6_memory::Address;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioQueueIndex(u16);

impl VirtioQueueIndex {
    pub const fn new(value: u16) -> Option<Self> {
        Some(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueSpec {
    size: u16,
    notify_offset: u16,
    notify_config_data: u16,
}

impl VirtioQueueSpec {
    pub const fn available(size: u16, notify_offset: u16) -> Self {
        Self {
            size,
            notify_offset,
            notify_config_data: notify_offset,
        }
    }

    pub const fn with_notify_config_data(mut self, notify_config_data: u16) -> Self {
        self.notify_config_data = notify_config_data;
        self
    }

    pub const fn size(self) -> u16 {
        self.size
    }

    pub const fn notify_offset(self) -> u16 {
        self.notify_offset
    }

    pub const fn notify_config_data(self) -> u16 {
        self.notify_config_data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueNotifySpec {
    queue: VirtioQueueIndex,
    notify_offset: u16,
}

impl VirtioQueueNotifySpec {
    pub const fn new(queue: VirtioQueueIndex, notify_offset: u16) -> Self {
        Self {
            queue,
            notify_offset,
        }
    }

    pub const fn queue(self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn notify_offset(self) -> u16 {
        self.notify_offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioQueueNotification {
    tick: Tick,
    queue: VirtioQueueIndex,
    value: u16,
    address: Address,
}

impl VirtioQueueNotification {
    pub const fn new(tick: Tick, queue: VirtioQueueIndex, value: u16, address: Address) -> Self {
        Self {
            tick,
            queue,
            value,
            address,
        }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn queue(self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn value(self) -> u16 {
        self.value
    }

    pub const fn address(self) -> Address {
        self.address
    }
}
