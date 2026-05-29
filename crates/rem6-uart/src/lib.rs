use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptEventKind, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, SchedulerContext, SchedulerError, Tick,
};
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
    interrupt: Option<UartInterrupt>,
    state: Arc<Mutex<UartState>>,
}

impl UartMmioDevice {
    pub fn new(id: UartId, base: Address) -> Self {
        Self {
            id,
            base,
            interrupt: None,
            state: Arc::new(Mutex::new(UartState::new())),
        }
    }

    pub fn with_interrupt(
        id: UartId,
        base: Address,
        source: InterruptSourceId,
        port: InterruptLinePort,
    ) -> Self {
        Self {
            id,
            base,
            interrupt: Some(UartInterrupt { source, port }),
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

    pub fn inject_rx_after<I>(
        &self,
        context: &mut SchedulerContext<'_>,
        delay: Tick,
        bytes: I,
    ) -> Result<PartitionEventId, UartError>
    where
        I: IntoIterator<Item = u8>,
    {
        let bytes = bytes.into_iter().collect::<Vec<_>>();
        let uart = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                uart.inject_rx_now(context, bytes);
            })
            .map_err(UartError::Scheduler)
    }

    pub fn inject_rx_after_parallel<I>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        delay: Tick,
        bytes: I,
    ) -> Result<PartitionEventId, UartError>
    where
        I: IntoIterator<Item = u8>,
    {
        let bytes = bytes.into_iter().collect::<Vec<_>>();
        let uart = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                uart.inject_rx_now_parallel(context, bytes);
            })
            .map_err(UartError::Scheduler)
    }

    pub fn snapshot(&self) -> UartSnapshot {
        self.state.lock().expect("uart state lock").snapshot()
    }

    pub fn restore(&self, snapshot: &UartSnapshot) {
        *self.state.lock().expect("uart state lock") = UartState::from_snapshot(snapshot);
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

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        match (offset, request.operation()) {
            (UART_MMIO_DATA_OFFSET, MmioOperation::Read) => {
                self.read_data_parallel(context, request)
            }
            (UART_MMIO_DATA_OFFSET, MmioOperation::Write) => {
                self.write_data_parallel(context, request)
            }
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
        let byte = *state
            .rx_pending
            .front()
            .ok_or_else(|| MmioError::DeviceError {
                request: request.id(),
                message: UartError::EmptyReceiveQueue.to_string(),
            })?;
        let should_deassert = state.rx_pending.len() == 1;
        if should_deassert {
            self.try_emit_interrupt(context, InterruptEventKind::Deassert)
                .map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                })?;
        }
        state.consume_rx(context.now());
        Ok(MmioResponse::completed(request.id(), Some(vec![byte])))
    }

    fn read_data_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let mut state = self.state.lock().expect("uart state lock");
        let byte = *state
            .rx_pending
            .front()
            .ok_or_else(|| MmioError::DeviceError {
                request: request.id(),
                message: UartError::EmptyReceiveQueue.to_string(),
            })?;
        let should_deassert = state.rx_pending.len() == 1;
        if should_deassert {
            self.try_emit_interrupt_parallel(context, InterruptEventKind::Deassert)
                .map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                })?;
        }
        state.consume_rx(context.now());
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

    fn write_data_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
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

    fn inject_rx_now(&self, context: &mut SchedulerContext<'_>, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }

        let mut state = self.state.lock().expect("uart state lock");
        let should_assert = state.rx_pending.is_empty();
        if should_assert {
            if let Err(error) = self.try_emit_interrupt(context, InterruptEventKind::Assert) {
                if let Some(interrupt) = &self.interrupt {
                    state.interrupt_errors.push(UartInterruptError::new(
                        context.now(),
                        interrupt.source,
                        InterruptEventKind::Assert,
                        error,
                    ));
                }
                return;
            }
        }
        state.inject_rx(context.now(), bytes);
    }

    fn inject_rx_now_parallel(&self, context: &mut ParallelSchedulerContext<'_>, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }

        let mut state = self.state.lock().expect("uart state lock");
        let should_assert = state.rx_pending.is_empty();
        if should_assert {
            if let Err(error) =
                self.try_emit_interrupt_parallel(context, InterruptEventKind::Assert)
            {
                if let Some(interrupt) = &self.interrupt {
                    state.interrupt_errors.push(UartInterruptError::new(
                        context.now(),
                        interrupt.source,
                        InterruptEventKind::Assert,
                        error,
                    ));
                }
                return;
            }
        }
        state.inject_rx(context.now(), bytes);
    }

    fn try_emit_interrupt(
        &self,
        context: &mut SchedulerContext<'_>,
        kind: InterruptEventKind,
    ) -> Result<(), InterruptError> {
        let Some(interrupt) = &self.interrupt else {
            return Ok(());
        };
        let result = match kind {
            InterruptEventKind::Assert => interrupt.port.assert(context, interrupt.source),
            InterruptEventKind::Deassert => interrupt.port.deassert(context, interrupt.source),
            InterruptEventKind::Claim | InterruptEventKind::Complete => return Ok(()),
        };
        result.map(|_| ())
    }

    fn try_emit_interrupt_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        kind: InterruptEventKind,
    ) -> Result<(), InterruptError> {
        let Some(interrupt) = &self.interrupt else {
            return Ok(());
        };
        let result = match kind {
            InterruptEventKind::Assert => interrupt.port.assert_parallel(context, interrupt.source),
            InterruptEventKind::Deassert => {
                interrupt.port.deassert_parallel(context, interrupt.source)
            }
            InterruptEventKind::Claim | InterruptEventKind::Complete => return Ok(()),
        };
        result.map(|_| ())
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

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        UartMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UartSnapshot {
    tx_bytes: Vec<UartTxByte>,
    rx_injected: Vec<UartRxByte>,
    rx_pending: Vec<u8>,
    rx_consumed: Vec<UartRxByte>,
    interrupt_errors: Vec<UartInterruptError>,
}

impl UartSnapshot {
    pub fn new(
        tx_bytes: Vec<UartTxByte>,
        rx_injected: Vec<UartRxByte>,
        rx_pending: Vec<u8>,
        rx_consumed: Vec<UartRxByte>,
        interrupt_errors: Vec<UartInterruptError>,
    ) -> Self {
        Self {
            tx_bytes,
            rx_injected,
            rx_pending,
            rx_consumed,
            interrupt_errors,
        }
    }

    pub fn tx_bytes(&self) -> &[UartTxByte] {
        &self.tx_bytes
    }

    pub fn rx_injected(&self) -> &[UartRxByte] {
        &self.rx_injected
    }

    pub fn rx_pending(&self) -> &[u8] {
        &self.rx_pending
    }

    pub fn rx_consumed(&self) -> &[UartRxByte] {
        &self.rx_consumed
    }

    pub fn interrupt_errors(&self) -> &[UartInterruptError] {
        &self.interrupt_errors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UartError {
    EmptyReceiveQueue,
    Scheduler(SchedulerError),
}

impl fmt::Display for UartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyReceiveQueue => write!(formatter, "UART receive queue is empty"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for UartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct UartInterrupt {
    source: InterruptSourceId,
    port: InterruptLinePort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UartInterruptError {
    tick: Tick,
    source: InterruptSourceId,
    kind: InterruptEventKind,
    error: InterruptError,
}

impl UartInterruptError {
    pub const fn new(
        tick: Tick,
        source: InterruptSourceId,
        kind: InterruptEventKind,
        error: InterruptError,
    ) -> Self {
        Self {
            tick,
            source,
            kind,
            error,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn kind(&self) -> InterruptEventKind {
        self.kind
    }

    pub const fn error(&self) -> &InterruptError {
        &self.error
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UartState {
    tx_bytes: Vec<UartTxByte>,
    rx_injected: Vec<UartRxByte>,
    rx_pending: VecDeque<u8>,
    rx_consumed: Vec<UartRxByte>,
    interrupt_errors: Vec<UartInterruptError>,
}

impl UartState {
    const fn new() -> Self {
        Self {
            tx_bytes: Vec::new(),
            rx_injected: Vec::new(),
            rx_pending: VecDeque::new(),
            rx_consumed: Vec::new(),
            interrupt_errors: Vec::new(),
        }
    }

    fn status(&self) -> u8 {
        let mut status = UART_STATUS_TX_READY;
        if !self.rx_pending.is_empty() {
            status |= UART_STATUS_RX_READY;
        }
        status
    }

    fn inject_rx(&mut self, tick: Tick, bytes: Vec<u8>) {
        for byte in bytes {
            self.rx_pending.push_back(byte);
            self.rx_injected.push(UartRxByte::new(tick, byte));
        }
    }

    fn consume_rx(&mut self, tick: Tick) {
        let byte = self.rx_pending.pop_front().expect("validated UART RX byte");
        self.rx_consumed.push(UartRxByte::new(tick, byte));
    }

    fn snapshot(&self) -> UartSnapshot {
        UartSnapshot::new(
            self.tx_bytes.clone(),
            self.rx_injected.clone(),
            self.rx_pending.iter().copied().collect(),
            self.rx_consumed.clone(),
            self.interrupt_errors.clone(),
        )
    }

    fn from_snapshot(snapshot: &UartSnapshot) -> Self {
        Self {
            tx_bytes: snapshot.tx_bytes().to_vec(),
            rx_injected: snapshot.rx_injected().to_vec(),
            rx_pending: snapshot.rx_pending().iter().copied().collect(),
            rx_consumed: snapshot.rx_consumed().to_vec(),
            interrupt_errors: snapshot.interrupt_errors().to_vec(),
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
