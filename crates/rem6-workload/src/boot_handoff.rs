use rem6_memory::{AccessSize, Address, AddressRange};

use crate::{WorkloadError, WorkloadResourceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadLinuxInitrd {
    resource: WorkloadResourceId,
    range: AddressRange,
}

impl WorkloadLinuxInitrd {
    pub fn new(
        resource: WorkloadResourceId,
        start: Address,
        size: AccessSize,
    ) -> Result<Self, WorkloadError> {
        let range = AddressRange::new(start, size).map_err(WorkloadError::Memory)?;
        Ok(Self { resource, range })
    }

    pub const fn resource(&self) -> &WorkloadResourceId {
        &self.resource
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub const fn start(&self) -> Address {
        self.range.start()
    }

    pub const fn end(&self) -> Address {
        self.range.end()
    }

    pub const fn size(&self) -> AccessSize {
        self.range.size()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadLinuxBootHandoff {
    dtb_addr: Address,
    device_tree_resource: Option<WorkloadResourceId>,
    bootargs: Option<String>,
    initrd: Option<WorkloadLinuxInitrd>,
}

impl WorkloadLinuxBootHandoff {
    pub const fn new(dtb_addr: Address) -> Self {
        Self {
            dtb_addr,
            device_tree_resource: None,
            bootargs: None,
            initrd: None,
        }
    }

    pub fn with_device_tree_resource(mut self, resource: WorkloadResourceId) -> Self {
        self.device_tree_resource = Some(resource);
        self
    }

    pub fn with_bootargs(mut self, bootargs: impl Into<String>) -> Self {
        self.bootargs = Some(bootargs.into());
        self
    }

    pub fn with_initrd(mut self, initrd: WorkloadLinuxInitrd) -> Self {
        self.initrd = Some(initrd);
        self
    }

    pub const fn dtb_addr(&self) -> Address {
        self.dtb_addr
    }

    pub const fn device_tree_resource(&self) -> Option<&WorkloadResourceId> {
        self.device_tree_resource.as_ref()
    }

    pub fn bootargs(&self) -> Option<&str> {
        self.bootargs.as_deref()
    }

    pub const fn initrd(&self) -> Option<&WorkloadLinuxInitrd> {
        self.initrd.as_ref()
    }
}
