use crate::{UartInterruptError, UartRxByte, UartTxByte};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pl011UartSnapshotFields {
    pub tx_bytes: Vec<UartTxByte>,
    pub rx_injected: Vec<UartRxByte>,
    pub rx_pending: Vec<u8>,
    pub rx_consumed: Vec<UartRxByte>,
    pub interrupt_errors: Vec<UartInterruptError>,
    pub control: u16,
    pub integer_baud_divisor: u16,
    pub fractional_baud_divisor: u16,
    pub line_control: u16,
    pub interrupt_fifo_level: u16,
    pub interrupt_mask: u16,
    pub raw_interrupt: u16,
}

impl Pl011UartSnapshot {
    pub fn from_fields(fields: Pl011UartSnapshotFields) -> Self {
        Self {
            tx_bytes: fields.tx_bytes,
            rx_injected: fields.rx_injected,
            rx_pending: fields.rx_pending,
            rx_consumed: fields.rx_consumed,
            interrupt_errors: fields.interrupt_errors,
            control: fields.control,
            integer_baud_divisor: fields.integer_baud_divisor,
            fractional_baud_divisor: fields.fractional_baud_divisor,
            line_control: fields.line_control,
            interrupt_fifo_level: fields.interrupt_fifo_level,
            interrupt_mask: fields.interrupt_mask,
            raw_interrupt: fields.raw_interrupt,
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
