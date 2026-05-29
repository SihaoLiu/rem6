use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, SchedulerContext};
use rem6_memory::AccessSize;
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciClassCode, PciDeviceIdentity, PciEndpointConfig,
    PciError, PciFunctionAddress, PciInterruptPin, PciLegacyInterruptPort, PciType0HeaderFields,
};

use crate::{IdeController, IdeControllerDispatch, IdeControllerError};

pub const IDE_PCI_VENDOR_ID: u16 = 0x8086;
pub const IDE_PCI_DEVICE_ID: u16 = 0x7111;
pub const IDE_PCI_STATUS: u16 = 0x0280;
pub const IDE_PCI_CLASS_CODE: u8 = 0x01;
pub const IDE_PCI_SUBCLASS_CODE: u8 = 0x01;
pub const IDE_PCI_PROG_IF: u8 = 0x85;
pub const IDE_PCI_INTERRUPT_LINE: u8 = 0x1f;
pub const IDE_PCI_COMMAND_BAR_BYTES: u64 = 8;
pub const IDE_PCI_CONTROL_BAR_BYTES: u64 = 4;
pub const IDE_PCI_BUS_MASTER_BAR_BYTES: u64 = 16;
pub const IDE_PCI_MAX_BAR_INDEX: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdePciEndpointSpec {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    status: u16,
    primary_command_bar: PciBarIndex,
    primary_control_bar: PciBarIndex,
    secondary_command_bar: PciBarIndex,
    secondary_control_bar: PciBarIndex,
    bus_master_bar: PciBarIndex,
    interrupt_line: u8,
    interrupt_pin: PciInterruptPin,
    io_shift: u8,
    control_offset: u64,
}

impl IdePciEndpointSpec {
    pub fn new(function: PciFunctionAddress) -> Self {
        Self {
            function,
            identity: PciDeviceIdentity::new(IDE_PCI_VENDOR_ID, IDE_PCI_DEVICE_ID),
            class: PciClassCode::new(
                IDE_PCI_CLASS_CODE,
                IDE_PCI_SUBCLASS_CODE,
                IDE_PCI_PROG_IF,
                0,
            ),
            status: IDE_PCI_STATUS,
            primary_command_bar: PciBarIndex::new(0).expect("valid IDE primary command BAR"),
            primary_control_bar: PciBarIndex::new(1).expect("valid IDE primary control BAR"),
            secondary_command_bar: PciBarIndex::new(2).expect("valid IDE secondary command BAR"),
            secondary_control_bar: PciBarIndex::new(3).expect("valid IDE secondary control BAR"),
            bus_master_bar: PciBarIndex::new(4).expect("valid IDE bus-master BAR"),
            interrupt_line: IDE_PCI_INTERRUPT_LINE,
            interrupt_pin: PciInterruptPin::IntA,
            io_shift: 0,
            control_offset: 0,
        }
    }

    pub fn with_io_shift(mut self, io_shift: u8) -> Result<Self, IdeControllerError> {
        IdeControllerDispatch::new(io_shift, self.control_offset)?;
        self.io_shift = io_shift;
        Ok(self)
    }

    pub const fn with_control_offset(mut self, control_offset: u64) -> Self {
        self.control_offset = control_offset;
        self
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

    pub const fn status(self) -> u16 {
        self.status
    }

    pub const fn primary_command_bar(self) -> PciBarIndex {
        self.primary_command_bar
    }

    pub const fn primary_control_bar(self) -> PciBarIndex {
        self.primary_control_bar
    }

    pub const fn secondary_command_bar(self) -> PciBarIndex {
        self.secondary_command_bar
    }

    pub const fn secondary_control_bar(self) -> PciBarIndex {
        self.secondary_control_bar
    }

    pub const fn bus_master_bar(self) -> PciBarIndex {
        self.bus_master_bar
    }

    pub const fn max_bar_index(self) -> PciBarIndex {
        self.bus_master_bar
    }

    pub const fn interrupt_line(self) -> u8 {
        self.interrupt_line
    }

    pub const fn interrupt_pin(self) -> PciInterruptPin {
        self.interrupt_pin
    }

    pub const fn io_shift(self) -> u8 {
        self.io_shift
    }

    pub const fn control_offset(self) -> u64 {
        self.control_offset
    }

    pub fn dispatch(
        self,
        bus_master_enabled: bool,
    ) -> Result<IdeControllerDispatch, IdeControllerError> {
        IdeControllerDispatch::new(self.io_shift, self.control_offset)
            .map(|dispatch| dispatch.with_bus_master_enabled(bus_master_enabled))
    }

    pub fn build_endpoint(self) -> Result<PciEndpointConfig, PciError> {
        let mut endpoint = PciEndpointConfig::new(self.function, self.identity, self.class)
            .with_status(self.status)
            .with_interrupt(self.interrupt_line, self.interrupt_pin)
            .with_type0_header(PciType0HeaderFields::new(0, 0, 0, 0, 0, 0));
        for spec in self.bar_specs()? {
            endpoint.install_bar(spec)?;
        }
        Ok(endpoint)
    }

    pub fn build_legacy_interrupt_port(
        self,
        port: PciLegacyInterruptPort,
        source: InterruptSourceId,
    ) -> Result<IdePciInterruptPort, IdeControllerError> {
        if port.function() != self.function || port.pin() != self.interrupt_pin {
            return Err(IdeControllerError::PciInterruptBindingMismatch {
                expected_function: self.function,
                actual_function: port.function(),
                expected_pin: self.interrupt_pin,
                actual_pin: port.pin(),
            });
        }
        Ok(IdePciInterruptPort::new(port, source))
    }

    fn bar_specs(self) -> Result<[PciBarSpec; 5], PciError> {
        Ok([
            io_bar(self.primary_command_bar, IDE_PCI_COMMAND_BAR_BYTES)?,
            io_bar(self.primary_control_bar, IDE_PCI_CONTROL_BAR_BYTES)?,
            io_bar(self.secondary_command_bar, IDE_PCI_COMMAND_BAR_BYTES)?,
            io_bar(self.secondary_control_bar, IDE_PCI_CONTROL_BAR_BYTES)?,
            io_bar(self.bus_master_bar, IDE_PCI_BUS_MASTER_BAR_BYTES)?,
        ])
    }
}

fn io_bar(index: PciBarIndex, bytes: u64) -> Result<PciBarSpec, PciError> {
    PciBarSpec::new(
        index,
        PciBarKind::Io,
        AccessSize::new(bytes).map_err(PciError::Memory)?,
    )
}

#[derive(Clone, Debug)]
pub struct IdePciInterruptPort {
    port: PciLegacyInterruptPort,
    source: InterruptSourceId,
    line_asserted: Arc<Mutex<bool>>,
}

impl IdePciInterruptPort {
    fn new(port: PciLegacyInterruptPort, source: InterruptSourceId) -> Self {
        Self {
            port,
            source,
            line_asserted: Arc::new(Mutex::new(false)),
        }
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn port(&self) -> &PciLegacyInterruptPort {
        &self.port
    }

    pub fn line_asserted(&self) -> bool {
        *self
            .line_asserted
            .lock()
            .expect("IDE PCI interrupt line state lock")
    }

    pub fn delivery_errors(&self) -> Arc<Mutex<Vec<InterruptError>>> {
        self.port.delivery_errors()
    }

    pub fn sync_controller(
        &self,
        context: &mut SchedulerContext<'_>,
        controller: &IdeController,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        self.sync_line(context, controller.shared_interrupt_asserted())
    }

    pub fn sync_controller_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        controller: &IdeController,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        self.sync_line_parallel(context, controller.shared_interrupt_asserted())
    }

    pub fn sync_line(
        &self,
        context: &mut SchedulerContext<'_>,
        asserted: bool,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        let mut line_asserted = self
            .line_asserted
            .lock()
            .expect("IDE PCI interrupt line state lock");
        if *line_asserted == asserted {
            return Ok(None);
        }

        let event = if asserted {
            self.port.post(context, self.source)
        } else {
            self.port.clear(context, self.source)
        }
        .map_err(pci_error)?;
        *line_asserted = asserted;
        Ok(Some(event))
    }

    pub fn sync_line_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        asserted: bool,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        let mut line_asserted = self
            .line_asserted
            .lock()
            .expect("IDE PCI interrupt line state lock");
        if *line_asserted == asserted {
            return Ok(None);
        }

        let event = if asserted {
            self.port.post_parallel(context, self.source)
        } else {
            self.port.clear_parallel(context, self.source)
        }
        .map_err(pci_error)?;
        *line_asserted = asserted;
        Ok(Some(event))
    }
}

fn pci_error(source: PciError) -> IdeControllerError {
    IdeControllerError::PciEndpoint { source }
}
