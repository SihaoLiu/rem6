use rem6_boot::{BootError, BootLoadReport};
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_platform::PlatformRiscvDeviceTreeConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDtbHandoffReport {
    dtb_addr: Address,
    dtb_len: usize,
    load_report: BootLoadReport,
}

impl RiscvDtbHandoffReport {
    pub const fn new(dtb_addr: Address, dtb_len: usize, load_report: BootLoadReport) -> Self {
        Self {
            dtb_addr,
            dtb_len,
            load_report,
        }
    }

    pub const fn dtb_addr(&self) -> Address {
        self.dtb_addr
    }

    pub const fn dtb_len(&self) -> usize {
        self.dtb_len
    }

    pub const fn load_report(&self) -> &BootLoadReport {
        &self.load_report
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvLinuxInitrdImage {
    start: Address,
    data: Vec<u8>,
}

impl RiscvLinuxInitrdImage {
    pub fn new(start: Address, data: Vec<u8>) -> Result<Self, BootError> {
        if data.is_empty() {
            return Err(BootError::EmptySegment { start });
        }
        let size = AccessSize::new(data.len() as u64).map_err(BootError::Memory)?;
        AddressRange::new(start, size).map_err(BootError::Memory)?;
        Ok(Self { start, data })
    }

    pub const fn start(&self) -> Address {
        self.start
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub const fn len(&self) -> usize {
        self.data.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvLinuxBootHandoffConfig {
    device_tree: PlatformRiscvDeviceTreeConfig,
    dtb_addr: Address,
    initrd: Option<RiscvLinuxInitrdImage>,
}

impl RiscvLinuxBootHandoffConfig {
    pub fn new(device_tree: PlatformRiscvDeviceTreeConfig, dtb_addr: Address) -> Self {
        Self {
            device_tree,
            dtb_addr,
            initrd: None,
        }
    }

    pub fn with_initrd(mut self, initrd: RiscvLinuxInitrdImage) -> Self {
        self.initrd = Some(initrd);
        self
    }

    pub fn device_tree(&self) -> &PlatformRiscvDeviceTreeConfig {
        &self.device_tree
    }

    pub const fn dtb_addr(&self) -> Address {
        self.dtb_addr
    }

    pub const fn initrd(&self) -> Option<&RiscvLinuxInitrdImage> {
        self.initrd.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvLinuxBootHandoffReport {
    dtb: RiscvDtbHandoffReport,
    initrd_load_report: Option<BootLoadReport>,
}

impl RiscvLinuxBootHandoffReport {
    pub fn new(dtb: RiscvDtbHandoffReport, initrd_load_report: Option<BootLoadReport>) -> Self {
        Self {
            dtb,
            initrd_load_report,
        }
    }

    pub const fn dtb(&self) -> &RiscvDtbHandoffReport {
        &self.dtb
    }

    pub const fn initrd_load_report(&self) -> Option<&BootLoadReport> {
        self.initrd_load_report.as_ref()
    }
}
