use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarMmioDevice, PciBarSpec, PciClassCode, PciDeviceIdentity,
    PciEndpointConfig, PciFunctionAddress, PciHostBarRange, PciInterruptPin, PciType0HeaderFields,
};

use crate::{SinicError, SinicMmioDevice};

pub const SINIC_PCI_VENDOR_ID: u16 = 0x1291;
pub const SINIC_PCI_DEVICE_ID: u16 = 0x1293;
pub const SINIC_PCI_BAR_BYTES: u64 = 64 * 1024;
pub const SINIC_PCI_INTERRUPT_LINE: u8 = 0x1e;
pub const SINIC_PCI_MINIMUM_GRANT: u8 = 0xb0;
pub const SINIC_PCI_MAXIMUM_LATENCY: u8 = 0x34;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicPciEndpointSpec {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    bar_index: PciBarIndex,
    bar_kind: PciBarKind,
    bar_size: AccessSize,
    interrupt_line: u8,
    interrupt_pin: PciInterruptPin,
}

impl SinicPciEndpointSpec {
    pub fn new(function: PciFunctionAddress) -> Self {
        Self {
            function,
            identity: PciDeviceIdentity::new(SINIC_PCI_VENDOR_ID, SINIC_PCI_DEVICE_ID),
            class: PciClassCode::new(0x02, 0x00, 0x00, 0x00),
            bar_index: PciBarIndex::new(0).expect("valid SINIC PCI BAR index"),
            bar_kind: PciBarKind::Memory32 {
                prefetchable: false,
            },
            bar_size: AccessSize::new(SINIC_PCI_BAR_BYTES).expect("valid SINIC PCI BAR size"),
            interrupt_line: SINIC_PCI_INTERRUPT_LINE,
            interrupt_pin: PciInterruptPin::IntA,
        }
    }

    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn identity(self) -> PciDeviceIdentity {
        self.identity
    }

    pub const fn class(self) -> PciClassCode {
        self.class
    }

    pub const fn bar_index(self) -> PciBarIndex {
        self.bar_index
    }

    pub const fn bar_kind(self) -> PciBarKind {
        self.bar_kind
    }

    pub const fn bar_size(self) -> AccessSize {
        self.bar_size
    }

    pub const fn interrupt_line(self) -> u8 {
        self.interrupt_line
    }

    pub const fn interrupt_pin(self) -> PciInterruptPin {
        self.interrupt_pin
    }

    pub fn build_endpoint(self) -> Result<PciEndpointConfig, SinicError> {
        let mut endpoint = PciEndpointConfig::new(self.function, self.identity, self.class)
            .with_interrupt(self.interrupt_line, self.interrupt_pin)
            .with_type0_header(PciType0HeaderFields::new(
                0,
                0,
                0,
                0,
                SINIC_PCI_MINIMUM_GRANT,
                SINIC_PCI_MAXIMUM_LATENCY,
            ));
        endpoint
            .install_bar(
                PciBarSpec::new(self.bar_index, self.bar_kind, self.bar_size).map_err(pci_error)?,
            )
            .map_err(pci_error)?;
        Ok(endpoint)
    }

    pub fn build_bar_mmio_device(
        self,
        host_bar_range: PciHostBarRange,
        device: SinicMmioDevice,
    ) -> Result<PciBarMmioDevice<SinicMmioDevice>, SinicError> {
        self.validate_host_bar_range(&host_bar_range)?;
        Ok(PciBarMmioDevice::new(host_bar_range, device))
    }

    fn validate_host_bar_range(self, host_bar_range: &PciHostBarRange) -> Result<(), SinicError> {
        if host_bar_range.function() != self.function || host_bar_range.bar() != self.bar_index {
            return Err(SinicError::PciBarBindingMismatch {
                expected_function: self.function,
                actual_function: host_bar_range.function(),
                expected_bar: self.bar_index,
                actual_bar: host_bar_range.bar(),
            });
        }
        let actual_bytes = host_bar_range.host_range().size().bytes();
        if actual_bytes != self.bar_size.bytes() {
            return Err(SinicError::PciBarSizeMismatch {
                expected_bytes: self.bar_size.bytes(),
                actual_bytes,
            });
        }
        Ok(())
    }

    pub fn local_mmio_device(self, device: crate::SinicFifoDevice) -> SinicMmioDevice {
        SinicMmioDevice::new(Address::new(0), device)
    }
}

fn pci_error(source: rem6_pci::PciError) -> SinicError {
    SinicError::PciEndpoint { source }
}
