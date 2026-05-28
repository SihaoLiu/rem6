use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

use crate::{
    EthernetPacket, SinicDataDescriptor, SinicError, SinicFifoDevice, SinicFifoDeviceSnapshot,
    SinicInterrupts, SinicReceiveRecord, SinicRegisterBlock, SinicRegisterOffset,
    SinicRxDmaCompletionRecord, SinicTxDmaCompletionRecord,
};

pub const SINIC_MMIO_VIRTUAL_STRIDE: u64 = 0x100;

#[derive(Clone, Debug)]
pub struct SinicMmioDevice {
    base: Address,
    state: Arc<Mutex<SinicFifoDevice>>,
}

impl SinicMmioDevice {
    pub fn new(base: Address, device: SinicFifoDevice) -> Self {
        Self {
            base,
            state: Arc::new(Mutex::new(device)),
        }
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn range_size_bytes(&self) -> u64 {
        let state = self.state.lock().expect("sinic mmio state lock");
        u64::from(state.registers().virtual_count().max(1)) * SINIC_MMIO_VIRTUAL_STRIDE
    }

    pub fn range(&self) -> AddressRange {
        AddressRange::new(
            self.base,
            AccessSize::new(self.range_size_bytes()).expect("valid SINIC MMIO range size"),
        )
        .expect("valid SINIC MMIO range")
    }

    pub fn snapshot(&self) -> SinicFifoDeviceSnapshot {
        self.state.lock().expect("sinic mmio state lock").snapshot()
    }

    pub fn restore(&self, snapshot: &SinicFifoDeviceSnapshot) -> Result<(), SinicError> {
        self.state
            .lock()
            .expect("sinic mmio state lock")
            .restore(snapshot)
    }

    pub fn receive_from_wire(
        &self,
        packet: EthernetPacket,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicReceiveRecord, SinicError> {
        self.state
            .lock()
            .expect("sinic mmio state lock")
            .receive_from_wire(packet, current_tick, interrupt_delay_ticks)
    }

    pub fn complete_rx_dma_copy(
        &self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicRxDmaCompletionRecord, SinicError> {
        self.state
            .lock()
            .expect("sinic mmio state lock")
            .complete_rx_dma_copy(current_tick, interrupt_delay_ticks)
    }

    pub fn complete_tx_dma_copy(
        &self,
        bytes: &[u8],
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicTxDmaCompletionRecord, SinicError> {
        self.state
            .lock()
            .expect("sinic mmio state lock")
            .complete_tx_dma_copy(bytes, current_tick, interrupt_delay_ticks)
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_at(context.now(), request)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_at(context.now(), request)
    }

    fn respond_at(
        &self,
        current_tick: u64,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let access = self.access(request)?;
        if !allows(access.info_access, request.operation()) {
            return Err(MmioError::AccessDenied {
                request: request.id(),
                operation: request.operation(),
                access: access.info_access,
            });
        }
        if access.virtual_index != 0 {
            return Err(MmioError::DeviceError {
                request: request.id(),
                message: format!(
                    "SINIC virtual index {} is not backed by typed state",
                    access.virtual_index
                ),
            });
        }

        let mut state = self.state.lock().expect("sinic mmio state lock");
        match request.operation() {
            MmioOperation::Read => {
                let data = read_register(&mut state, access.register_offset)?;
                Ok(MmioResponse::completed(request.id(), Some(data)))
            }
            MmioOperation::Write => {
                write_register(&mut state, request, access.register_offset, current_tick)?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn access(&self, request: &MmioRequest) -> Result<SinicMmioAccess, MmioError> {
        let raw_offset = request
            .range()
            .start()
            .get()
            .checked_sub(self.base.get())
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })?;
        let virtual_index = raw_offset / SINIC_MMIO_VIRTUAL_STRIDE;
        let register_offset = raw_offset % SINIC_MMIO_VIRTUAL_STRIDE;
        let Some(info) = SinicRegisterOffset::info(register_offset as u32) else {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        };
        if request.size().bytes() != u64::from(info.bytes()) {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: u64::from(info.bytes()),
                actual: request.size().bytes(),
            });
        }

        let state = self.state.lock().expect("sinic mmio state lock");
        let virtual_count = u64::from(state.registers().virtual_count());
        drop(state);
        if virtual_index >= virtual_count.max(1) {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }

        Ok(SinicMmioAccess {
            virtual_index,
            register_offset: register_offset as u32,
            info_access: info_access(info.can_read(), info.can_write()),
        })
    }
}

impl MmioDevice for SinicMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        SinicMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        SinicMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SinicMmioAccess {
    virtual_index: u64,
    register_offset: u32,
    info_access: MmioAccess,
}

fn read_register(device: &mut SinicFifoDevice, offset: u32) -> Result<Vec<u8>, MmioError> {
    let registers = device.registers_mut();
    match offset {
        0x00 => Ok(le32(registers.config_bits())),
        0x08 => Ok(le32(registers.read_interrupt_status().bits())),
        0x0c => Ok(le32(registers.interrupt_mask().bits())),
        0x10 => Ok(le32(registers.rx_max_copy())),
        0x14 => Ok(le32(registers.tx_max_copy())),
        0x18 => Ok(le32(registers.zero_copy_size())),
        0x1c => Ok(le32(registers.zero_copy_mark())),
        0x20 => Ok(le32(registers.virtual_count())),
        0x24 => Ok(le32(registers.rx_max_intr())),
        0x28 => Ok(le32(registers.rx_fifo_size())),
        0x2c => Ok(le32(registers.tx_fifo_size())),
        0x30 => Ok(le32(registers.rx_fifo_low())),
        0x34 => Ok(le32(registers.tx_fifo_low())),
        0x38 => Ok(le32(registers.rx_fifo_high())),
        0x3c => Ok(le32(registers.tx_fifo_high())),
        0x40 => Ok(le64(device.rx_data_descriptor().bits())),
        0x48 | 0x50 => Ok(le64(device.rx_done_status().bits())),
        0x58 => Ok(le64(device.tx_data_descriptor().bits())),
        0x60 | 0x68 => Ok(le64(device.tx_done_status().bits())),
        0x70 => Ok(le64(device.registers().hardware_address())),
        0x78 => Ok(le64(device.rx_status().bits())),
        _ => Err(MmioError::UnmappedAddress {
            address: Address::new(u64::from(offset)),
        }),
    }
}

fn write_register(
    device: &mut SinicFifoDevice,
    request: &MmioRequest,
    offset: u32,
    current_tick: u64,
) -> Result<(), MmioError> {
    match offset {
        0x00 => {
            let current = device.registers().config_bits();
            let value = masked_u32(request, current)?;
            device
                .registers_mut()
                .change_config(value, current_tick)
                .map_err(|error| device_error(request, error))?;
            Ok(())
        }
        0x04 => {
            let command = masked_u32(request, 0)?;
            if (command & SinicRegisterBlock::COMMAND_INTR) != 0 {
                device
                    .registers_mut()
                    .post_interrupt(SinicInterrupts::SOFT, current_tick, 0)
                    .map_err(|error| device_error(request, error))?;
            }
            if (command & SinicRegisterBlock::COMMAND_RESET) != 0 {
                device
                    .reset()
                    .map_err(|error| device_error(request, error))?;
            }
            Ok(())
        }
        0x08 => {
            let clear_bits = masked_u32(request, 0)?;
            device
                .registers_mut()
                .clear_interrupts(SinicInterrupts::from_bits_truncate(clear_bits))
                .map_err(|error| device_error(request, error))
        }
        0x0c => {
            let current = device.registers().interrupt_mask().bits();
            let value = masked_u32(request, current)?;
            device
                .registers_mut()
                .change_interrupt_mask(SinicInterrupts::from_bits_truncate(value), current_tick)
                .map(|_| ())
                .map_err(|error| device_error(request, error))
        }
        0x40 => {
            let descriptor = SinicDataDescriptor::from_bits(masked_u64(
                request,
                device.rx_data_descriptor().bits(),
            )?);
            device
                .begin_rx_dma_copy(descriptor)
                .map(|_| ())
                .map_err(|error| device_error(request, error))
        }
        0x58 => {
            let descriptor = SinicDataDescriptor::from_bits(masked_u64(
                request,
                device.tx_data_descriptor().bits(),
            )?);
            device
                .begin_tx_dma_copy(descriptor)
                .map(|_| ())
                .map_err(|error| device_error(request, error))
        }
        _ => Err(MmioError::AccessDenied {
            request: request.id(),
            operation: request.operation(),
            access: MmioAccess::ReadOnly,
        }),
    }
}

fn masked_u32(request: &MmioRequest, current: u32) -> Result<u32, MmioError> {
    let mut bytes = current.to_le_bytes().to_vec();
    apply_mask(request, &mut bytes)?;
    let mut value = [0; 4];
    value.copy_from_slice(&bytes);
    Ok(u32::from_le_bytes(value))
}

fn masked_u64(request: &MmioRequest, current: u64) -> Result<u64, MmioError> {
    let mut bytes = current.to_le_bytes().to_vec();
    apply_mask(request, &mut bytes)?;
    let mut value = [0; 8];
    value.copy_from_slice(&bytes);
    Ok(u64::from_le_bytes(value))
}

fn apply_mask(request: &MmioRequest, bytes: &mut [u8]) -> Result<(), MmioError> {
    let payload = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    if payload.len() != bytes.len() {
        return Err(MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: bytes.len() as u64,
            actual: payload.len() as u64,
        });
    }
    let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
        request: request.id(),
    })?;
    validate_mask(request, mask, bytes.len() as u64)?;
    for (index, byte) in payload.iter().enumerate() {
        if mask.bits()[index] {
            bytes[index] = *byte;
        }
    }
    Ok(())
}

fn validate_mask(request: &MmioRequest, mask: &ByteMask, expected: u64) -> Result<(), MmioError> {
    if mask.len() != expected {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected,
            actual: mask.len(),
        });
    }
    Ok(())
}

const fn info_access(read: bool, write: bool) -> MmioAccess {
    match (read, write) {
        (true, true) => MmioAccess::ReadWrite,
        (true, false) => MmioAccess::ReadOnly,
        (false, true) => MmioAccess::WriteOnly,
        (false, false) => MmioAccess::ReadOnly,
    }
}

const fn allows(access: MmioAccess, operation: MmioOperation) -> bool {
    matches!(
        (access, operation),
        (MmioAccess::ReadOnly, MmioOperation::Read)
            | (MmioAccess::WriteOnly, MmioOperation::Write)
            | (MmioAccess::ReadWrite, _)
    )
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn device_error(request: &MmioRequest, error: SinicError) -> MmioError {
    MmioError::DeviceError {
        request: request.id(),
        message: error.to_string(),
    }
}
