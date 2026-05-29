use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptEventKind, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse,
};

pub use rem6_amba::{
    ArmPrimecellId, AMBA_CELL_ID0_OFFSET, AMBA_CELL_ID1_OFFSET, AMBA_CELL_ID2_OFFSET,
    AMBA_CELL_ID3_OFFSET, AMBA_PERIPHERAL_ID0_OFFSET, AMBA_PERIPHERAL_ID1_OFFSET,
    AMBA_PERIPHERAL_ID2_OFFSET, AMBA_PERIPHERAL_ID3_OFFSET,
};

pub const UART_MMIO_REGISTER_BYTES: u64 = 1;
pub const UART_MMIO_DATA_OFFSET: u64 = 0x00;
pub const UART_MMIO_STATUS_OFFSET: u64 = 0x05;
pub const UART_STATUS_RX_READY: u8 = 0x01;
pub const UART_STATUS_TX_READY: u8 = 0x20;

pub const PL011_DATA_OFFSET: u64 = 0x000;
pub const PL011_STATUS_OFFSET: u64 = 0x004;
pub const PL011_ERROR_CLEAR_OFFSET: u64 = 0x004;
pub const PL011_FLAG_OFFSET: u64 = 0x018;
pub const PL011_INTEGER_BRD_OFFSET: u64 = 0x024;
pub const PL011_FBRD_OFFSET: u64 = 0x028;
pub const PL011_LINE_CONTROL_OFFSET: u64 = 0x02c;
pub const PL011_CONTROL_OFFSET: u64 = 0x030;
pub const PL011_IFLS_OFFSET: u64 = 0x034;
pub const PL011_IMSC_OFFSET: u64 = 0x038;
pub const PL011_RAW_ISR_OFFSET: u64 = 0x03c;
pub const PL011_MASKED_ISR_OFFSET: u64 = 0x040;
pub const PL011_INT_CLEAR_OFFSET: u64 = 0x044;
pub const PL011_DMACR_OFFSET: u64 = 0x048;
pub const PL011_REGISTER_BYTES: u64 = 4;
pub const PL011_MMIO_SIZE_BYTES: u64 = 0x1000;
pub const PL011_PRIMECELL_ID: ArmPrimecellId = ArmPrimecellId::new(0x0034_1011);

pub const PL011_FLAG_CTS: u16 = 0x001;
pub const PL011_FLAG_RX_EMPTY: u16 = 0x010;
pub const PL011_FLAG_TX_FULL: u16 = 0x020;
pub const PL011_FLAG_RX_FULL: u16 = 0x040;
pub const PL011_FLAG_TX_EMPTY: u16 = 0x080;

pub const PL011_INT_RING: u16 = 1 << 0;
pub const PL011_INT_CTS: u16 = 1 << 1;
pub const PL011_INT_DCD: u16 = 1 << 2;
pub const PL011_INT_DSR: u16 = 1 << 3;
pub const PL011_INT_RX: u16 = 1 << 4;
pub const PL011_INT_TX: u16 = 1 << 5;
pub const PL011_INT_RX_TIMEOUT: u16 = 1 << 6;
pub const PL011_INT_FRAME: u16 = 1 << 7;
pub const PL011_INT_PARITY: u16 = 1 << 8;
pub const PL011_INT_BREAK: u16 = 1 << 9;
pub const PL011_INT_OVERRUN: u16 = 1 << 10;

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

#[derive(Clone, Debug)]
pub struct Pl011UartMmioDevice {
    id: UartId,
    base: Address,
    interrupt: Option<UartInterrupt>,
    state: Arc<Mutex<Pl011State>>,
}

impl Pl011UartMmioDevice {
    pub fn new(id: UartId, base: Address) -> Self {
        Self {
            id,
            base,
            interrupt: None,
            state: Arc::new(Mutex::new(Pl011State::new())),
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
            state: Arc::new(Mutex::new(Pl011State::new())),
        }
    }

    pub const fn id(&self) -> UartId {
        self.id
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn inject_rx<I>(&self, bytes: I) -> Result<(), Pl011Error>
    where
        I: IntoIterator<Item = u8>,
    {
        let bytes = bytes.into_iter().collect::<Vec<_>>();
        if bytes.is_empty() {
            return Ok(());
        }

        let mut state = self.state.lock().expect("PL011 UART state lock");
        state.raise_interrupts(PL011_INT_RX | PL011_INT_RX_TIMEOUT);
        state.inject_rx(0, bytes);
        Ok(())
    }

    pub fn inject_rx_after<I>(
        &self,
        context: &mut SchedulerContext<'_>,
        delay: Tick,
        bytes: I,
    ) -> Result<PartitionEventId, Pl011Error>
    where
        I: IntoIterator<Item = u8>,
    {
        let bytes = bytes.into_iter().collect::<Vec<_>>();
        let uart = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                uart.inject_rx_now(context, bytes);
            })
            .map_err(Pl011Error::Scheduler)
    }

    pub fn inject_rx_after_parallel<I>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        delay: Tick,
        bytes: I,
    ) -> Result<PartitionEventId, Pl011Error>
    where
        I: IntoIterator<Item = u8>,
    {
        let bytes = bytes.into_iter().collect::<Vec<_>>();
        let uart = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                uart.inject_rx_now_parallel(context, bytes);
            })
            .map_err(Pl011Error::Scheduler)
    }

    pub fn snapshot(&self) -> Pl011UartSnapshot {
        self.state.lock().expect("PL011 UART state lock").snapshot()
    }

    pub fn restore(&self, snapshot: &Pl011UartSnapshot) {
        *self.state.lock().expect("PL011 UART state lock") = Pl011State::from_snapshot(snapshot);
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        if let Some(value) = PL011_PRIMECELL_ID.read_u32(offset) {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le_response(value, request.size().bytes())),
                )),
                MmioOperation::Write => Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                }),
            };
        }

        match request.operation() {
            MmioOperation::Read => self.read_register(context, request, offset),
            MmioOperation::Write => self.write_register(context, request, offset),
        }
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        if let Some(value) = PL011_PRIMECELL_ID.read_u32(offset) {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le_response(value, request.size().bytes())),
                )),
                MmioOperation::Write => Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                }),
            };
        }

        match request.operation() {
            MmioOperation::Read => self.read_register_parallel(context, request, offset),
            MmioOperation::Write => self.write_register_parallel(context, request, offset),
        }
    }

    fn read_register(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
        offset: u64,
    ) -> Result<MmioResponse, MmioError> {
        let mut state = self.state.lock().expect("PL011 UART state lock");
        let value = match offset {
            PL011_DATA_OFFSET => self.read_data(context, request.id(), &mut state)?,
            PL011_STATUS_OFFSET => 0,
            PL011_FLAG_OFFSET => state.flags() as u32,
            PL011_CONTROL_OFFSET => state.control as u32,
            PL011_INTEGER_BRD_OFFSET => state.integer_baud_divisor as u32,
            PL011_FBRD_OFFSET => state.fractional_baud_divisor as u32,
            PL011_LINE_CONTROL_OFFSET => state.line_control as u32,
            PL011_IFLS_OFFSET => state.interrupt_fifo_level as u32,
            PL011_IMSC_OFFSET => state.interrupt_mask as u32,
            PL011_RAW_ISR_OFFSET => state.raw_interrupt as u32,
            PL011_MASKED_ISR_OFFSET => state.masked_interrupt() as u32,
            PL011_DMACR_OFFSET => 0,
            _ => {
                return Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                });
            }
        };
        Ok(MmioResponse::completed(
            request.id(),
            Some(le_response(value, request.size().bytes())),
        ))
    }

    fn read_register_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
        offset: u64,
    ) -> Result<MmioResponse, MmioError> {
        let mut state = self.state.lock().expect("PL011 UART state lock");
        let value = match offset {
            PL011_DATA_OFFSET => self.read_data_parallel(context, request.id(), &mut state)?,
            PL011_STATUS_OFFSET => 0,
            PL011_FLAG_OFFSET => state.flags() as u32,
            PL011_CONTROL_OFFSET => state.control as u32,
            PL011_INTEGER_BRD_OFFSET => state.integer_baud_divisor as u32,
            PL011_FBRD_OFFSET => state.fractional_baud_divisor as u32,
            PL011_LINE_CONTROL_OFFSET => state.line_control as u32,
            PL011_IFLS_OFFSET => state.interrupt_fifo_level as u32,
            PL011_IMSC_OFFSET => state.interrupt_mask as u32,
            PL011_RAW_ISR_OFFSET => state.raw_interrupt as u32,
            PL011_MASKED_ISR_OFFSET => state.masked_interrupt() as u32,
            PL011_DMACR_OFFSET => 0,
            _ => {
                return Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                });
            }
        };
        Ok(MmioResponse::completed(
            request.id(),
            Some(le_response(value, request.size().bytes())),
        ))
    }

    fn read_data(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
    ) -> Result<u32, MmioError> {
        let Some(byte) = state.rx_pending.front().copied() else {
            return Ok(0);
        };
        self.clear_interrupts(context, request, state, PL011_INT_RX | PL011_INT_RX_TIMEOUT)?;
        state.rx_pending.pop_front();
        state.rx_consumed.push(UartRxByte::new(context.now(), byte));
        if !state.rx_pending.is_empty() {
            self.raise_interrupts(context, request, state, PL011_INT_RX | PL011_INT_RX_TIMEOUT)?;
        }
        Ok(byte as u32)
    }

    fn read_data_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
    ) -> Result<u32, MmioError> {
        let Some(byte) = state.rx_pending.front().copied() else {
            return Ok(0);
        };
        self.clear_interrupts_parallel(
            context,
            request,
            state,
            PL011_INT_RX | PL011_INT_RX_TIMEOUT,
        )?;
        state.rx_pending.pop_front();
        state.rx_consumed.push(UartRxByte::new(context.now(), byte));
        if !state.rx_pending.is_empty() {
            self.raise_interrupts_parallel(
                context,
                request,
                state,
                PL011_INT_RX | PL011_INT_RX_TIMEOUT,
            )?;
        }
        Ok(byte as u32)
    }

    fn write_register(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
        offset: u64,
    ) -> Result<MmioResponse, MmioError> {
        let value = pl011_write_value(request)?;
        let mut state = self.state.lock().expect("PL011 UART state lock");
        match offset {
            PL011_DATA_OFFSET => {
                state
                    .tx_bytes
                    .push(UartTxByte::new(context.now(), value as u8));
                self.clear_interrupts(context, request.id(), &mut state, PL011_INT_TX)?;
                self.raise_interrupts(context, request.id(), &mut state, PL011_INT_TX)?;
            }
            PL011_ERROR_CLEAR_OFFSET => {}
            PL011_CONTROL_OFFSET => state.control = value as u16,
            PL011_INTEGER_BRD_OFFSET => state.integer_baud_divisor = value as u16,
            PL011_FBRD_OFFSET => state.fractional_baud_divisor = value as u16,
            PL011_LINE_CONTROL_OFFSET => state.line_control = value as u16,
            PL011_IFLS_OFFSET => state.interrupt_fifo_level = value as u16,
            PL011_IMSC_OFFSET => {
                let raw_interrupt = state.raw_interrupt;
                self.set_interrupts(
                    context,
                    request.id(),
                    &mut state,
                    raw_interrupt,
                    value as u16,
                )?;
            }
            PL011_INT_CLEAR_OFFSET => {
                self.clear_interrupts(context, request.id(), &mut state, value as u16)?;
                if !state.rx_pending.is_empty() {
                    self.raise_interrupts(
                        context,
                        request.id(),
                        &mut state,
                        PL011_INT_RX | PL011_INT_RX_TIMEOUT,
                    )?;
                }
            }
            PL011_DMACR_OFFSET => {
                if value & 0x7 != 0 {
                    return Err(MmioError::DeviceError {
                        request: request.id(),
                        message: Pl011Error::DmaUnsupported.to_string(),
                    });
                }
            }
            _ => {
                return Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                });
            }
        }
        Ok(MmioResponse::completed(request.id(), None))
    }

    fn write_register_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
        offset: u64,
    ) -> Result<MmioResponse, MmioError> {
        let value = pl011_write_value(request)?;
        let mut state = self.state.lock().expect("PL011 UART state lock");
        match offset {
            PL011_DATA_OFFSET => {
                state
                    .tx_bytes
                    .push(UartTxByte::new(context.now(), value as u8));
                self.clear_interrupts_parallel(context, request.id(), &mut state, PL011_INT_TX)?;
                self.raise_interrupts_parallel(context, request.id(), &mut state, PL011_INT_TX)?;
            }
            PL011_ERROR_CLEAR_OFFSET => {}
            PL011_CONTROL_OFFSET => state.control = value as u16,
            PL011_INTEGER_BRD_OFFSET => state.integer_baud_divisor = value as u16,
            PL011_FBRD_OFFSET => state.fractional_baud_divisor = value as u16,
            PL011_LINE_CONTROL_OFFSET => state.line_control = value as u16,
            PL011_IFLS_OFFSET => state.interrupt_fifo_level = value as u16,
            PL011_IMSC_OFFSET => {
                let raw_interrupt = state.raw_interrupt;
                self.set_interrupts_parallel(
                    context,
                    request.id(),
                    &mut state,
                    raw_interrupt,
                    value as u16,
                )?;
            }
            PL011_INT_CLEAR_OFFSET => {
                self.clear_interrupts_parallel(context, request.id(), &mut state, value as u16)?;
                if !state.rx_pending.is_empty() {
                    self.raise_interrupts_parallel(
                        context,
                        request.id(),
                        &mut state,
                        PL011_INT_RX | PL011_INT_RX_TIMEOUT,
                    )?;
                }
            }
            PL011_DMACR_OFFSET => {
                if value & 0x7 != 0 {
                    return Err(MmioError::DeviceError {
                        request: request.id(),
                        message: Pl011Error::DmaUnsupported.to_string(),
                    });
                }
            }
            _ => {
                return Err(MmioError::UnmappedAddress {
                    address: request.range().start(),
                });
            }
        }
        Ok(MmioResponse::completed(request.id(), None))
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        let bytes = request.size().bytes();
        if !(1..=PL011_REGISTER_BYTES).contains(&bytes) {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: PL011_REGISTER_BYTES,
                actual: bytes,
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

        let mut state = self.state.lock().expect("PL011 UART state lock");
        let raw_interrupt = state.raw_interrupt | PL011_INT_RX | PL011_INT_RX_TIMEOUT;
        let transition = state.interrupt_transition(raw_interrupt, state.interrupt_mask);
        if let Some(kind) = transition {
            if let Err(error) = self.try_emit_interrupt(context, kind) {
                if let Some(interrupt) = &self.interrupt {
                    state.interrupt_errors.push(UartInterruptError::new(
                        context.now(),
                        interrupt.source,
                        kind,
                        error,
                    ));
                }
                return;
            }
        }
        state.raw_interrupt = raw_interrupt;
        state.inject_rx(context.now(), bytes);
    }

    fn inject_rx_now_parallel(&self, context: &mut ParallelSchedulerContext<'_>, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }

        let mut state = self.state.lock().expect("PL011 UART state lock");
        let raw_interrupt = state.raw_interrupt | PL011_INT_RX | PL011_INT_RX_TIMEOUT;
        let transition = state.interrupt_transition(raw_interrupt, state.interrupt_mask);
        if let Some(kind) = transition {
            if let Err(error) = self.try_emit_interrupt_parallel(context, kind) {
                if let Some(interrupt) = &self.interrupt {
                    state.interrupt_errors.push(UartInterruptError::new(
                        context.now(),
                        interrupt.source,
                        kind,
                        error,
                    ));
                }
                return;
            }
        }
        state.raw_interrupt = raw_interrupt;
        state.inject_rx(context.now(), bytes);
    }

    fn set_interrupts(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
        raw: u16,
        mask: u16,
    ) -> Result<(), MmioError> {
        let transition = state.interrupt_transition(raw, mask);
        self.emit_transition(context, request, transition)?;
        state.raw_interrupt = raw;
        state.interrupt_mask = mask;
        Ok(())
    }

    fn raise_interrupts(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
        interrupts: u16,
    ) -> Result<(), MmioError> {
        self.set_interrupts(
            context,
            request,
            state,
            state.raw_interrupt | interrupts,
            state.interrupt_mask,
        )
    }

    fn clear_interrupts(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
        interrupts: u16,
    ) -> Result<(), MmioError> {
        self.set_interrupts(
            context,
            request,
            state,
            state.raw_interrupt & !interrupts,
            state.interrupt_mask,
        )
    }

    fn set_interrupts_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
        raw: u16,
        mask: u16,
    ) -> Result<(), MmioError> {
        let transition = state.interrupt_transition(raw, mask);
        self.emit_transition_parallel(context, request, transition)?;
        state.raw_interrupt = raw;
        state.interrupt_mask = mask;
        Ok(())
    }

    fn raise_interrupts_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
        interrupts: u16,
    ) -> Result<(), MmioError> {
        self.set_interrupts_parallel(
            context,
            request,
            state,
            state.raw_interrupt | interrupts,
            state.interrupt_mask,
        )
    }

    fn clear_interrupts_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MmioRequestId,
        state: &mut Pl011State,
        interrupts: u16,
    ) -> Result<(), MmioError> {
        self.set_interrupts_parallel(
            context,
            request,
            state,
            state.raw_interrupt & !interrupts,
            state.interrupt_mask,
        )
    }

    fn emit_transition(
        &self,
        context: &mut SchedulerContext<'_>,
        request: MmioRequestId,
        transition: Option<InterruptEventKind>,
    ) -> Result<(), MmioError> {
        let Some(kind) = transition else {
            return Ok(());
        };
        self.try_emit_interrupt(context, kind)
            .map_err(|error| MmioError::DeviceError {
                request,
                message: error.to_string(),
            })
    }

    fn emit_transition_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MmioRequestId,
        transition: Option<InterruptEventKind>,
    ) -> Result<(), MmioError> {
        let Some(kind) = transition else {
            return Ok(());
        };
        self.try_emit_interrupt_parallel(context, kind)
            .map_err(|error| MmioError::DeviceError {
                request,
                message: error.to_string(),
            })
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

impl MmioDevice for Pl011UartMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Pl011UartMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Pl011UartMmioDevice::respond_parallel(self, context, request)
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
pub struct Pl011UartSnapshot {
    tx_bytes: Vec<UartTxByte>,
    rx_injected: Vec<UartRxByte>,
    rx_pending: Vec<u8>,
    rx_consumed: Vec<UartRxByte>,
    interrupt_errors: Vec<UartInterruptError>,
    control: u16,
    integer_baud_divisor: u16,
    fractional_baud_divisor: u16,
    line_control: u16,
    interrupt_fifo_level: u16,
    interrupt_mask: u16,
    raw_interrupt: u16,
}

impl Pl011UartSnapshot {
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

    pub const fn control(&self) -> u16 {
        self.control
    }

    pub const fn integer_baud_divisor(&self) -> u16 {
        self.integer_baud_divisor
    }

    pub const fn fractional_baud_divisor(&self) -> u16 {
        self.fractional_baud_divisor
    }

    pub const fn line_control(&self) -> u16 {
        self.line_control
    }

    pub const fn interrupt_fifo_level(&self) -> u16 {
        self.interrupt_fifo_level
    }

    pub const fn interrupt_mask(&self) -> u16 {
        self.interrupt_mask
    }

    pub const fn raw_interrupt(&self) -> u16 {
        self.raw_interrupt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pl011Error {
    DmaUnsupported,
    Scheduler(SchedulerError),
}

impl fmt::Display for Pl011Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DmaUnsupported => write!(formatter, "PL011 DMA is not supported"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for Pl011Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Pl011State {
    tx_bytes: Vec<UartTxByte>,
    rx_injected: Vec<UartRxByte>,
    rx_pending: VecDeque<u8>,
    rx_consumed: Vec<UartRxByte>,
    interrupt_errors: Vec<UartInterruptError>,
    control: u16,
    integer_baud_divisor: u16,
    fractional_baud_divisor: u16,
    line_control: u16,
    interrupt_fifo_level: u16,
    interrupt_mask: u16,
    raw_interrupt: u16,
}

impl Pl011State {
    const fn new() -> Self {
        Self {
            tx_bytes: Vec::new(),
            rx_injected: Vec::new(),
            rx_pending: VecDeque::new(),
            rx_consumed: Vec::new(),
            interrupt_errors: Vec::new(),
            control: 0x300,
            integer_baud_divisor: 0,
            fractional_baud_divisor: 0,
            line_control: 0,
            interrupt_fifo_level: 0x12,
            interrupt_mask: 0,
            raw_interrupt: 0,
        }
    }

    fn flags(&self) -> u16 {
        let mut flags = PL011_FLAG_CTS | PL011_FLAG_TX_EMPTY;
        if self.rx_pending.is_empty() {
            flags |= PL011_FLAG_RX_EMPTY;
        } else {
            flags |= PL011_FLAG_RX_FULL;
        }
        flags
    }

    const fn masked_interrupt(&self) -> u16 {
        self.raw_interrupt & self.interrupt_mask
    }

    fn interrupt_transition(&self, raw: u16, mask: u16) -> Option<InterruptEventKind> {
        let old_masked = self.masked_interrupt() != 0;
        let new_masked = (raw & mask) != 0;
        match (old_masked, new_masked) {
            (false, true) => Some(InterruptEventKind::Assert),
            (true, false) => Some(InterruptEventKind::Deassert),
            _ => None,
        }
    }

    fn raise_interrupts(&mut self, interrupts: u16) {
        self.raw_interrupt |= interrupts;
    }

    fn inject_rx(&mut self, tick: Tick, bytes: Vec<u8>) {
        for byte in bytes {
            self.rx_pending.push_back(byte);
            self.rx_injected.push(UartRxByte::new(tick, byte));
        }
    }

    fn snapshot(&self) -> Pl011UartSnapshot {
        Pl011UartSnapshot {
            tx_bytes: self.tx_bytes.clone(),
            rx_injected: self.rx_injected.clone(),
            rx_pending: self.rx_pending.iter().copied().collect(),
            rx_consumed: self.rx_consumed.clone(),
            interrupt_errors: self.interrupt_errors.clone(),
            control: self.control,
            integer_baud_divisor: self.integer_baud_divisor,
            fractional_baud_divisor: self.fractional_baud_divisor,
            line_control: self.line_control,
            interrupt_fifo_level: self.interrupt_fifo_level,
            interrupt_mask: self.interrupt_mask,
            raw_interrupt: self.raw_interrupt,
        }
    }

    fn from_snapshot(snapshot: &Pl011UartSnapshot) -> Self {
        Self {
            tx_bytes: snapshot.tx_bytes().to_vec(),
            rx_injected: snapshot.rx_injected().to_vec(),
            rx_pending: snapshot.rx_pending().iter().copied().collect(),
            rx_consumed: snapshot.rx_consumed().to_vec(),
            interrupt_errors: snapshot.interrupt_errors().to_vec(),
            control: snapshot.control(),
            integer_baud_divisor: snapshot.integer_baud_divisor(),
            fractional_baud_divisor: snapshot.fractional_baud_divisor(),
            line_control: snapshot.line_control(),
            interrupt_fifo_level: snapshot.interrupt_fifo_level(),
            interrupt_mask: snapshot.interrupt_mask(),
            raw_interrupt: snapshot.raw_interrupt(),
        }
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

fn pl011_write_value(request: &MmioRequest) -> Result<u32, MmioError> {
    let data = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    if data.len() as u64 != request.size().bytes() {
        return Err(MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: request.size().bytes(),
            actual: data.len() as u64,
        });
    }
    let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
        request: request.id(),
    })?;
    if mask.len() != request.size().bytes() {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: request.size().bytes(),
            actual: mask.len(),
        });
    }

    let mut bytes = [0; PL011_REGISTER_BYTES as usize];
    for (index, byte) in data.iter().enumerate() {
        if mask.bits()[index] {
            bytes[index] = *byte;
        }
    }
    Ok(u32::from_le_bytes(bytes))
}

fn le_response(value: u32, bytes: u64) -> Vec<u8> {
    value.to_le_bytes()[..bytes as usize].to_vec()
}
