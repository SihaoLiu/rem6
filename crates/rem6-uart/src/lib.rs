use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{SchedulerContext, Tick};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

pub const UART_MMIO_REGISTER_BYTES: u64 = 1;
pub const UART_MMIO_DATA_OFFSET: u64 = 0x00;
pub const UART_MMIO_STATUS_OFFSET: u64 = 0x05;
pub const UART_STATUS_RX_READY: u8 = 0x01;
pub const UART_STATUS_TX_READY: u8 = 0x20;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UartId(u64);

impl UartId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartTxByte {
    tick: Tick,
    byte: u8,
}

impl UartTxByte {
    pub const fn new(tick: Tick, byte: u8) -> Self {
        Self { tick, byte }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartRxByte {
    tick: Tick,
    byte: u8,
}

impl UartRxByte {
    pub const fn new(tick: Tick, byte: u8) -> Self {
        Self { tick, byte }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }
}

#[derive(Clone, Debug)]
pub struct UartMmioDevice {
    id: UartId,
    base: Address,
    state: Arc<Mutex<UartState>>,
}

impl UartMmioDevice {
    pub fn new(id: UartId, base: Address) -> Self {
        Self {
            id,
            base,
            state: Arc::new(Mutex::new(UartState::new())),
        }
    }

    pub const fn id(&self) -> UartId {
        self.id
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn inject_rx<I>(&self, bytes: I) -> Result<(), UartError>
    where
        I: IntoIterator<Item = u8>,
    {
        let mut state = self.state.lock().expect("uart state lock");
        state.rx_pending.extend(bytes);
        Ok(())
    }

    pub fn snapshot(&self) -> UartSnapshot {
        self.state.lock().expect("uart state lock").snapshot()
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        match (offset, request.operation()) {
            (UART_MMIO_DATA_OFFSET, MmioOperation::Read) => self.read_data(context, request),
            (UART_MMIO_DATA_OFFSET, MmioOperation::Write) => self.write_data(context, request),
            (UART_MMIO_STATUS_OFFSET, MmioOperation::Read) => {
                let status = self.state.lock().expect("uart state lock").status();
                Ok(MmioResponse::completed(request.id(), Some(vec![status])))
            }
            (UART_MMIO_STATUS_OFFSET, MmioOperation::Write) => Err(MmioError::AccessDenied {
                request: request.id(),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            }),
            _ => Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            }),
        }
    }

    fn read_data(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let mut state = self.state.lock().expect("uart state lock");
        let byte = state
            .rx_pending
            .pop_front()
            .ok_or_else(|| MmioError::DeviceError {
                request: request.id(),
                message: UartError::EmptyReceiveQueue.to_string(),
            })?;
        state.rx_consumed.push(UartRxByte::new(context.now(), byte));
        Ok(MmioResponse::completed(request.id(), Some(vec![byte])))
    }

    fn write_data(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let data = request.data().ok_or(MmioError::MissingWriteData {
            request: request.id(),
        })?;
        if data.len() as u64 != UART_MMIO_REGISTER_BYTES {
            return Err(MmioError::PayloadSizeMismatch {
                request: request.id(),
                expected: UART_MMIO_REGISTER_BYTES,
                actual: data.len() as u64,
            });
        }
        let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
            request: request.id(),
        })?;
        validate_uart_mask(request, mask)?;

        if mask.bits()[0] {
            self.state
                .lock()
                .expect("uart state lock")
                .tx_bytes
                .push(UartTxByte::new(context.now(), data[0]));
        }
        Ok(MmioResponse::completed(request.id(), None))
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != UART_MMIO_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: UART_MMIO_REGISTER_BYTES,
                actual: request.size().bytes(),
            });
        }
        Ok(())
    }

    fn offset(&self, request: &MmioRequest) -> Result<u64, MmioError> {
        request
            .range()
            .start()
            .get()
            .checked_sub(self.base.get())
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })
    }
}

impl MmioDevice for UartMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        UartMmioDevice::respond(self, context, request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UartSnapshot {
    tx_bytes: Vec<UartTxByte>,
    rx_pending: Vec<u8>,
    rx_consumed: Vec<UartRxByte>,
}

impl UartSnapshot {
    pub fn tx_bytes(&self) -> &[UartTxByte] {
        &self.tx_bytes
    }

    pub fn rx_pending(&self) -> &[u8] {
        &self.rx_pending
    }

    pub fn rx_consumed(&self) -> &[UartRxByte] {
        &self.rx_consumed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UartError {
    EmptyReceiveQueue,
}

impl fmt::Display for UartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyReceiveQueue => write!(formatter, "UART receive queue is empty"),
        }
    }
}

impl Error for UartError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UartState {
    tx_bytes: Vec<UartTxByte>,
    rx_pending: VecDeque<u8>,
    rx_consumed: Vec<UartRxByte>,
}

impl UartState {
    const fn new() -> Self {
        Self {
            tx_bytes: Vec::new(),
            rx_pending: VecDeque::new(),
            rx_consumed: Vec::new(),
        }
    }

    fn status(&self) -> u8 {
        let mut status = UART_STATUS_TX_READY;
        if !self.rx_pending.is_empty() {
            status |= UART_STATUS_RX_READY;
        }
        status
    }

    fn snapshot(&self) -> UartSnapshot {
        UartSnapshot {
            tx_bytes: self.tx_bytes.clone(),
            rx_pending: self.rx_pending.iter().copied().collect(),
            rx_consumed: self.rx_consumed.clone(),
        }
    }
}

fn validate_uart_mask(request: &MmioRequest, mask: &ByteMask) -> Result<(), MmioError> {
    if mask.len() != UART_MMIO_REGISTER_BYTES {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: UART_MMIO_REGISTER_BYTES,
            actual: mask.len(),
        });
    }
    Ok(())
}
