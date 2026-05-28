use std::error::Error;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

#[path = "sinic/fifo.rs"]
mod fifo;
#[path = "sinic/memory.rs"]
mod memory;
#[path = "sinic/mmio.rs"]
mod mmio;
#[path = "sinic/pci.rs"]
mod pci;
pub use fifo::*;
pub use memory::*;
pub use mmio::*;
pub use pci::*;

use crate::NetworkError;

const ADDR_MASK_40: u64 = (1_u64 << 40) - 1;
const LEN_MASK_20: u32 = (1_u32 << 20) - 1;
const INTR_ALL_BITS: u32 = 0x01ff;
const INTR_NO_DELAY_BITS: u32 = 0x01cc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicRegisterOffset(u32);

impl SinicRegisterOffset {
    pub const CONFIG: Self = Self(0x00);
    pub const COMMAND: Self = Self(0x04);
    pub const INTR_STATUS: Self = Self(0x08);
    pub const INTR_MASK: Self = Self(0x0c);
    pub const RX_MAX_COPY: Self = Self(0x10);
    pub const TX_MAX_COPY: Self = Self(0x14);
    pub const ZERO_COPY_SIZE: Self = Self(0x18);
    pub const ZERO_COPY_MARK: Self = Self(0x1c);
    pub const VIRTUAL_COUNT: Self = Self(0x20);
    pub const RX_MAX_INTR: Self = Self(0x24);
    pub const RX_FIFO_SIZE: Self = Self(0x28);
    pub const TX_FIFO_SIZE: Self = Self(0x2c);
    pub const RX_FIFO_LOW: Self = Self(0x30);
    pub const TX_FIFO_LOW: Self = Self(0x34);
    pub const RX_FIFO_HIGH: Self = Self(0x38);
    pub const TX_FIFO_HIGH: Self = Self(0x3c);
    pub const RX_DATA: Self = Self(0x40);
    pub const RX_DONE: Self = Self(0x48);
    pub const RX_WAIT: Self = Self(0x50);
    pub const TX_DATA: Self = Self(0x58);
    pub const TX_DONE: Self = Self(0x60);
    pub const TX_WAIT: Self = Self(0x68);
    pub const HW_ADDR: Self = Self(0x70);
    pub const RX_STATUS: Self = Self(0x78);
    pub const SIZE: u32 = 0x80;

    pub const fn addr(self) -> u32 {
        self.0
    }

    pub fn info(addr: u32) -> Option<SinicRegisterInfo> {
        match addr {
            0x00 => Some(SinicRegisterInfo::new(4, true, true, "Config")),
            0x04 => Some(SinicRegisterInfo::new(4, false, true, "Command")),
            0x08 => Some(SinicRegisterInfo::new(4, true, true, "IntrStatus")),
            0x0c => Some(SinicRegisterInfo::new(4, true, true, "IntrMask")),
            0x10 => Some(SinicRegisterInfo::new(4, true, false, "RxMaxCopy")),
            0x14 => Some(SinicRegisterInfo::new(4, true, false, "TxMaxCopy")),
            0x18 => Some(SinicRegisterInfo::new(4, true, false, "ZeroCopySize")),
            0x1c => Some(SinicRegisterInfo::new(4, true, false, "ZeroCopyMark")),
            0x20 => Some(SinicRegisterInfo::new(4, true, false, "VirtualCount")),
            0x24 => Some(SinicRegisterInfo::new(4, true, false, "RxMaxIntr")),
            0x28 => Some(SinicRegisterInfo::new(4, true, false, "RxFifoSize")),
            0x2c => Some(SinicRegisterInfo::new(4, true, false, "TxFifoSize")),
            0x30 => Some(SinicRegisterInfo::new(4, true, false, "RxFifoLow")),
            0x34 => Some(SinicRegisterInfo::new(4, true, false, "TxFifoLow")),
            0x38 => Some(SinicRegisterInfo::new(4, true, false, "RxFifoHigh")),
            0x3c => Some(SinicRegisterInfo::new(4, true, false, "TxFifoHigh")),
            0x40 => Some(SinicRegisterInfo::new(8, true, true, "RxData")),
            0x48 => Some(SinicRegisterInfo::new(8, true, false, "RxDone")),
            0x50 => Some(SinicRegisterInfo::new(8, true, false, "RxWait")),
            0x58 => Some(SinicRegisterInfo::new(8, true, true, "TxData")),
            0x60 => Some(SinicRegisterInfo::new(8, true, false, "TxDone")),
            0x68 => Some(SinicRegisterInfo::new(8, true, false, "TxWait")),
            0x70 => Some(SinicRegisterInfo::new(8, true, false, "HwAddr")),
            0x78 => Some(SinicRegisterInfo::new(8, true, false, "RxStatus")),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicRegisterInfo {
    bytes: u8,
    read: bool,
    write: bool,
    name: &'static str,
}

impl SinicRegisterInfo {
    const fn new(bytes: u8, read: bool, write: bool, name: &'static str) -> Self {
        Self {
            bytes,
            read,
            write,
            name,
        }
    }

    pub const fn bytes(&self) -> u8 {
        self.bytes
    }

    pub const fn can_read(&self) -> bool {
        self.read
    }

    pub const fn can_write(&self) -> bool {
        self.write
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicDataDescriptor {
    bits: u64,
}

impl SinicDataDescriptor {
    pub fn new(address: u64, len: u32) -> Result<Self, SinicError> {
        if address > ADDR_MASK_40 {
            return Err(SinicError::DescriptorAddressTooWide { address });
        }
        if len > LEN_MASK_20 {
            return Err(SinicError::DescriptorLengthTooWide { len });
        }
        Ok(Self {
            bits: address | ((len as u64) << 40),
        })
    }

    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn address(self) -> u64 {
        self.bits & ADDR_MASK_40
    }

    pub const fn byte_len(self) -> u32 {
        ((self.bits >> 40) as u32) & LEN_MASK_20
    }

    pub const fn no_delay(self) -> bool {
        self.bit(61)
    }

    pub const fn more(self) -> bool {
        self.bit(63)
    }

    pub const fn checksum(self) -> bool {
        self.bit(62)
    }

    pub const fn virtual_address(self) -> bool {
        self.bit(60)
    }

    pub const fn with_no_delay(mut self, enabled: bool) -> Self {
        self.set_bit(61, enabled);
        self
    }

    pub const fn with_more(mut self, enabled: bool) -> Self {
        self.set_bit(63, enabled);
        self
    }

    pub const fn with_checksum(mut self, enabled: bool) -> Self {
        self.set_bit(62, enabled);
        self
    }

    pub const fn with_virtual_address(mut self, enabled: bool) -> Self {
        self.set_bit(60, enabled);
        self
    }

    const fn bit(self, offset: u64) -> bool {
        ((self.bits >> offset) & 1) != 0
    }

    const fn set_bit(&mut self, offset: u64, enabled: bool) {
        let mask = 1_u64 << offset;
        if enabled {
            self.bits |= mask;
        } else {
            self.bits &= !mask;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicDoneStatus {
    bits: u64,
}

impl SinicDoneStatus {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn packets(self) -> u16 {
        ((self.bits >> 32) & 0xffff) as u16
    }

    pub const fn copy_len(self) -> u32 {
        (self.bits as u32) & LEN_MASK_20
    }

    pub const fn with_packets(mut self, packets: u16) -> Self {
        self.bits = set_u64_field(self.bits, 32, 16, packets as u64);
        self
    }

    pub const fn with_busy(mut self, enabled: bool) -> Self {
        self.set_bit(31, enabled);
        self
    }

    pub const fn with_complete(mut self, enabled: bool) -> Self {
        self.set_bit(30, enabled);
        self
    }

    pub const fn with_more(mut self, enabled: bool) -> Self {
        self.set_bit(29, enabled);
        self
    }

    pub const fn with_full(mut self, enabled: bool) -> Self {
        self.set_bit(29, enabled);
        self
    }

    pub const fn with_empty(mut self, enabled: bool) -> Self {
        self.set_bit(28, enabled);
        self
    }

    pub const fn with_low(mut self, enabled: bool) -> Self {
        self.set_bit(28, enabled);
        self
    }

    pub const fn with_high(mut self, enabled: bool) -> Self {
        self.set_bit(27, enabled);
        self
    }

    pub const fn with_not_high(mut self, enabled: bool) -> Self {
        self.set_bit(26, enabled);
        self
    }

    pub const fn with_tcp_error(mut self, enabled: bool) -> Self {
        self.set_bit(25, enabled);
        self
    }

    pub const fn with_udp_error(mut self, enabled: bool) -> Self {
        self.set_bit(24, enabled);
        self
    }

    pub const fn with_ip_error(mut self, enabled: bool) -> Self {
        self.set_bit(23, enabled);
        self
    }

    pub const fn with_tcp_packet(mut self, enabled: bool) -> Self {
        self.set_bit(22, enabled);
        self
    }

    pub const fn with_udp_packet(mut self, enabled: bool) -> Self {
        self.set_bit(21, enabled);
        self
    }

    pub const fn with_ip_packet(mut self, enabled: bool) -> Self {
        self.set_bit(20, enabled);
        self
    }

    pub fn with_copy_len(mut self, copy_len: u32) -> Result<Self, SinicError> {
        if copy_len > LEN_MASK_20 {
            return Err(SinicError::DescriptorLengthTooWide { len: copy_len });
        }
        self.bits = set_u64_field(self.bits, 0, 20, copy_len as u64);
        Ok(self)
    }

    const fn set_bit(&mut self, offset: u64, enabled: bool) {
        let mask = 1_u64 << offset;
        if enabled {
            self.bits |= mask;
        } else {
            self.bits &= !mask;
        }
    }
}

impl Default for SinicDoneStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicRxStatus {
    bits: u64,
}

impl SinicRxStatus {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn with_dirty(mut self, dirty: u16) -> Self {
        self.bits = set_u64_field(self.bits, 48, 16, dirty as u64);
        self
    }

    pub const fn with_mapped(mut self, mapped: u16) -> Self {
        self.bits = set_u64_field(self.bits, 32, 16, mapped as u64);
        self
    }

    pub const fn with_busy(mut self, busy: u16) -> Self {
        self.bits = set_u64_field(self.bits, 16, 16, busy as u64);
        self
    }

    pub const fn with_head(mut self, head: u16) -> Self {
        self.bits = set_u64_field(self.bits, 0, 16, head as u64);
        self
    }
}

impl Default for SinicRxStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicInterrupts(u32);

impl SinicInterrupts {
    pub const NONE: Self = Self(0);
    pub const RX_PACKET: Self = Self(1 << 0);
    pub const RX_DMA: Self = Self(1 << 1);
    pub const RX_EMPTY: Self = Self(1 << 2);
    pub const RX_HIGH: Self = Self(1 << 3);
    pub const TX_PACKET: Self = Self(1 << 4);
    pub const TX_DMA: Self = Self(1 << 5);
    pub const TX_FULL: Self = Self(1 << 6);
    pub const TX_LOW: Self = Self(1 << 7);
    pub const SOFT: Self = Self(1 << 8);
    pub const ALL: Self = Self(INTR_ALL_BITS);
    pub const NO_DELAY: Self = Self(INTR_NO_DELAY_BITS);

    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    fn validate(self) -> Result<(), SinicError> {
        let reserved_bits = self.0 & !INTR_ALL_BITS;
        if reserved_bits != 0 {
            return Err(SinicError::ReservedInterruptBits {
                bits: self.0,
                reserved_bits,
            });
        }
        Ok(())
    }
}

impl BitOr for SinicInterrupts {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SinicInterrupts {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SinicInterrupts {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for SinicInterrupts {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for SinicInterrupts {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicRegisterParams {
    config_bits: u32,
    interrupt_mask: SinicInterrupts,
    rx_max_copy: u32,
    tx_max_copy: u32,
    zero_copy_size: u32,
    zero_copy_mark: u32,
    virtual_count: u32,
    rx_max_intr: u32,
    rx_fifo_size: u32,
    tx_fifo_size: u32,
    rx_fifo_low: u32,
    tx_fifo_low: u32,
    rx_fifo_high: u32,
    tx_fifo_high: u32,
    hardware_address: u64,
}

impl SinicRegisterParams {
    pub const fn with_zero_copy(mut self, enabled: bool) -> Self {
        self.config_bits = set_u32_bit(self.config_bits, 12, enabled);
        self
    }

    pub const fn with_delay_copy(mut self, enabled: bool) -> Self {
        self.config_bits = set_u32_bit(self.config_bits, 11, enabled);
        self
    }

    pub const fn with_virtual_address(mut self, enabled: bool) -> Self {
        self.config_bits = set_u32_bit(self.config_bits, 5, enabled);
        self
    }

    pub const fn with_interrupt_mask(mut self, interrupt_mask: SinicInterrupts) -> Self {
        self.interrupt_mask = interrupt_mask;
        self
    }

    pub const fn with_virtual_count(mut self, virtual_count: u32) -> Self {
        self.virtual_count = virtual_count;
        self
    }

    pub const fn with_rx_copy_limits(
        mut self,
        rx_max_copy: u32,
        zero_copy_mark: u32,
        zero_copy_size: u32,
    ) -> Self {
        self.rx_max_copy = rx_max_copy;
        self.zero_copy_mark = zero_copy_mark;
        self.zero_copy_size = zero_copy_size;
        self
    }

    pub const fn with_tx_max_copy(mut self, tx_max_copy: u32) -> Self {
        self.tx_max_copy = tx_max_copy;
        self
    }

    pub const fn with_fifo_limits(
        mut self,
        rx_fifo_size: u32,
        tx_fifo_size: u32,
        rx_fifo_low: u32,
        tx_fifo_low: u32,
        rx_fifo_high: u32,
        tx_fifo_high: u32,
    ) -> Self {
        self.rx_fifo_size = rx_fifo_size;
        self.tx_fifo_size = tx_fifo_size;
        self.rx_fifo_low = rx_fifo_low;
        self.tx_fifo_low = tx_fifo_low;
        self.rx_fifo_high = rx_fifo_high;
        self.tx_fifo_high = tx_fifo_high;
        self
    }

    pub const fn with_hardware_address(mut self, hardware_address: u64) -> Self {
        self.hardware_address = hardware_address;
        self
    }
}

impl Default for SinicRegisterParams {
    fn default() -> Self {
        Self {
            config_bits: 0,
            interrupt_mask: SinicInterrupts::SOFT
                .bitor(SinicInterrupts::RX_HIGH)
                .bitor(SinicInterrupts::RX_PACKET)
                .bitor(SinicInterrupts::TX_LOW),
            rx_max_copy: 1514,
            tx_max_copy: 1514,
            zero_copy_size: 64,
            zero_copy_mark: 256,
            virtual_count: 1,
            rx_max_intr: 1,
            rx_fifo_size: 16 * 1024,
            tx_fifo_size: 16 * 1024,
            rx_fifo_low: 256,
            tx_fifo_low: 256,
            rx_fifo_high: 8 * 1024,
            tx_fifo_high: 8 * 1024,
            hardware_address: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicRegisterBlock {
    params: SinicRegisterParams,
    config_bits: u32,
    interrupt_status: SinicInterrupts,
    interrupt_mask: SinicInterrupts,
    rx_empty_seen: bool,
    tx_full_seen: bool,
    pending_interrupt_tick: Option<u64>,
}

impl SinicRegisterBlock {
    pub const COMMAND_RESET: u32 = 1 << 0;
    pub const COMMAND_INTR: u32 = 1 << 1;

    pub const CONFIG_RX_EN: u32 = 1 << 0;
    pub const CONFIG_TX_EN: u32 = 1 << 1;
    pub const CONFIG_INT_EN: u32 = 1 << 2;
    pub const CONFIG_POLL: u32 = 1 << 3;
    pub const CONFIG_DESC: u32 = 1 << 4;
    pub const CONFIG_VADDR: u32 = 1 << 5;
    pub const CONFIG_VLAN: u32 = 1 << 6;
    pub const CONFIG_FILTER: u32 = 1 << 7;
    pub const CONFIG_TX_THREAD: u32 = 1 << 8;
    pub const CONFIG_RX_THREAD: u32 = 1 << 9;
    pub const CONFIG_RSS: u32 = 1 << 10;
    pub const CONFIG_DELAY_COPY: u32 = 1 << 11;
    pub const CONFIG_ZERO_COPY: u32 = 1 << 12;

    pub fn new(params: SinicRegisterParams) -> Result<Self, SinicError> {
        validate_params(params)?;
        params.interrupt_mask.validate()?;
        Ok(Self {
            params,
            config_bits: params.config_bits,
            interrupt_status: SinicInterrupts::NONE,
            interrupt_mask: params.interrupt_mask,
            rx_empty_seen: false,
            tx_full_seen: false,
            pending_interrupt_tick: None,
        })
    }

    pub const fn config_bits(&self) -> u32 {
        self.config_bits
    }

    pub const fn params(&self) -> SinicRegisterParams {
        self.params
    }

    pub const fn rx_enabled(&self) -> bool {
        (self.config_bits & Self::CONFIG_RX_EN) != 0
    }

    pub const fn tx_enabled(&self) -> bool {
        (self.config_bits & Self::CONFIG_TX_EN) != 0
    }

    pub const fn cpu_interrupt_enabled(&self) -> bool {
        (self.config_bits & Self::CONFIG_INT_EN) != 0
    }

    pub const fn interrupt_status(&self) -> SinicInterrupts {
        self.interrupt_status
    }

    pub const fn interrupt_mask(&self) -> SinicInterrupts {
        self.interrupt_mask
    }

    pub const fn pending_interrupt_tick(&self) -> Option<u64> {
        self.pending_interrupt_tick
    }

    pub const fn virtual_count(&self) -> u32 {
        self.params.virtual_count
    }

    pub const fn rx_fifo_size(&self) -> u32 {
        self.params.rx_fifo_size
    }

    pub const fn tx_fifo_size(&self) -> u32 {
        self.params.tx_fifo_size
    }

    pub const fn rx_fifo_low(&self) -> u32 {
        self.params.rx_fifo_low
    }

    pub const fn tx_fifo_low(&self) -> u32 {
        self.params.tx_fifo_low
    }

    pub const fn rx_fifo_high(&self) -> u32 {
        self.params.rx_fifo_high
    }

    pub const fn tx_fifo_high(&self) -> u32 {
        self.params.tx_fifo_high
    }

    pub const fn rx_max_copy(&self) -> u32 {
        self.params.rx_max_copy
    }

    pub const fn tx_max_copy(&self) -> u32 {
        self.params.tx_max_copy
    }

    pub const fn rx_max_intr(&self) -> u32 {
        self.params.rx_max_intr
    }

    pub const fn zero_copy_size(&self) -> u32 {
        self.params.zero_copy_size
    }

    pub const fn zero_copy_mark(&self) -> u32 {
        self.params.zero_copy_mark
    }

    pub const fn hardware_address(&self) -> u64 {
        self.params.hardware_address
    }

    pub fn change_config(
        &mut self,
        config_bits: u32,
        current_tick: u64,
    ) -> Result<Option<SinicInterruptRecord>, SinicError> {
        let was_enabled = self.cpu_interrupt_enabled();
        self.config_bits = config_bits;
        let enabled = self.cpu_interrupt_enabled();
        if enabled {
            if !was_enabled {
                return Ok(self.schedule_masked_interrupt(current_tick, 0));
            }
        } else {
            self.pending_interrupt_tick = None;
        }
        Ok(None)
    }

    pub fn change_interrupt_mask(
        &mut self,
        interrupt_mask: SinicInterrupts,
        current_tick: u64,
    ) -> Result<Option<SinicInterruptRecord>, SinicError> {
        interrupt_mask.validate()?;
        if self.interrupt_mask == interrupt_mask {
            return Ok(None);
        }
        self.interrupt_mask = interrupt_mask;
        Ok(self.schedule_masked_interrupt(current_tick, 0))
    }

    pub fn post_interrupt(
        &mut self,
        interrupts: SinicInterrupts,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicInterruptRecord, SinicError> {
        interrupts.validate()?;
        self.interrupt_status |= interrupts;
        let masked_bits = self.gated_masked_interrupts();
        let scheduled_tick =
            self.schedule_interrupt(masked_bits, current_tick, interrupt_delay_ticks);
        Ok(SinicInterruptRecord {
            requested_bits: interrupts,
            status_bits: self.interrupt_status,
            masked_bits,
            scheduled_tick,
        })
    }

    pub fn record_rx_empty(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicInterruptRecord, SinicError> {
        let record = self.post_interrupt(
            SinicInterrupts::RX_EMPTY,
            current_tick,
            interrupt_delay_ticks,
        )?;
        self.rx_empty_seen = true;
        Ok(record)
    }

    pub fn record_tx_full(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicInterruptRecord, SinicError> {
        let record = self.post_interrupt(
            SinicInterrupts::TX_FULL,
            current_tick,
            interrupt_delay_ticks,
        )?;
        self.tx_full_seen = true;
        Ok(record)
    }

    pub fn clear_interrupts(&mut self, interrupts: SinicInterrupts) -> Result<(), SinicError> {
        interrupts.validate()?;
        self.interrupt_status &= !interrupts;
        if (self.interrupt_status & self.interrupt_mask).is_empty() {
            self.pending_interrupt_tick = None;
        }
        Ok(())
    }

    pub fn read_interrupt_status(&mut self) -> SinicInterrupts {
        let status = self.interrupt_status;
        self.interrupt_status = SinicInterrupts::NONE;
        self.pending_interrupt_tick = None;
        status
    }

    pub fn snapshot(&self) -> SinicRegisterBlockSnapshot {
        SinicRegisterBlockSnapshot {
            params: self.params,
            config_bits: self.config_bits,
            interrupt_status: self.interrupt_status,
            interrupt_mask: self.interrupt_mask,
            rx_empty_seen: self.rx_empty_seen,
            tx_full_seen: self.tx_full_seen,
            pending_interrupt_tick: self.pending_interrupt_tick,
        }
    }

    pub fn restore(&mut self, snapshot: &SinicRegisterBlockSnapshot) -> Result<(), SinicError> {
        validate_params(snapshot.params)?;
        snapshot.interrupt_mask.validate()?;
        snapshot.interrupt_status.validate()?;
        self.params = snapshot.params;
        self.config_bits = snapshot.config_bits;
        self.interrupt_status = snapshot.interrupt_status;
        self.interrupt_mask = snapshot.interrupt_mask;
        self.rx_empty_seen = snapshot.rx_empty_seen;
        self.tx_full_seen = snapshot.tx_full_seen;
        self.pending_interrupt_tick = snapshot.pending_interrupt_tick;
        Ok(())
    }

    fn gated_masked_interrupts(&mut self) -> SinicInterrupts {
        let mut masked_bits = self.interrupt_status & self.interrupt_mask;
        if masked_bits.contains(SinicInterrupts::RX_HIGH) {
            if self.rx_empty_seen {
                self.rx_empty_seen = false;
            } else {
                masked_bits &= !SinicInterrupts::RX_HIGH;
            }
        }
        if masked_bits.contains(SinicInterrupts::TX_LOW) {
            if self.tx_full_seen {
                self.tx_full_seen = false;
            } else {
                masked_bits &= !SinicInterrupts::TX_LOW;
            }
        }
        masked_bits
    }

    fn schedule_masked_interrupt(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Option<SinicInterruptRecord> {
        let masked_bits = self.gated_masked_interrupts();
        let scheduled_tick =
            self.schedule_interrupt(masked_bits, current_tick, interrupt_delay_ticks);
        scheduled_tick.map(|scheduled_tick| SinicInterruptRecord {
            requested_bits: SinicInterrupts::NONE,
            status_bits: self.interrupt_status,
            masked_bits,
            scheduled_tick: Some(scheduled_tick),
        })
    }

    fn schedule_interrupt(
        &mut self,
        masked_bits: SinicInterrupts,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Option<u64> {
        if masked_bits.is_empty() || !self.cpu_interrupt_enabled() {
            return None;
        }
        let scheduled_tick = if (masked_bits & SinicInterrupts::NO_DELAY).is_empty() {
            current_tick.saturating_add(interrupt_delay_ticks)
        } else {
            current_tick
        };
        if self
            .pending_interrupt_tick
            .is_none_or(|pending_tick| scheduled_tick <= pending_tick)
        {
            self.pending_interrupt_tick = Some(scheduled_tick);
        }
        Some(scheduled_tick)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicInterruptRecord {
    requested_bits: SinicInterrupts,
    status_bits: SinicInterrupts,
    masked_bits: SinicInterrupts,
    scheduled_tick: Option<u64>,
}

impl SinicInterruptRecord {
    pub const fn new(
        requested_bits: SinicInterrupts,
        status_bits: SinicInterrupts,
        masked_bits: SinicInterrupts,
        scheduled_tick: Option<u64>,
    ) -> Self {
        Self {
            requested_bits,
            status_bits,
            masked_bits,
            scheduled_tick,
        }
    }

    pub const fn requested_bits(&self) -> SinicInterrupts {
        self.requested_bits
    }

    pub const fn status_bits(&self) -> SinicInterrupts {
        self.status_bits
    }

    pub const fn masked_bits(&self) -> SinicInterrupts {
        self.masked_bits
    }

    pub const fn scheduled_tick(&self) -> Option<u64> {
        self.scheduled_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicRegisterBlockSnapshot {
    params: SinicRegisterParams,
    config_bits: u32,
    interrupt_status: SinicInterrupts,
    interrupt_mask: SinicInterrupts,
    rx_empty_seen: bool,
    tx_full_seen: bool,
    pending_interrupt_tick: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SinicError {
    DescriptorAddressTooWide {
        address: u64,
    },
    DescriptorLengthTooWide {
        len: u32,
    },
    ReservedInterruptBits {
        bits: u32,
        reserved_bits: u32,
    },
    IncompatibleCopyModes,
    RxMaxCopyBelowZeroCopyMark {
        rx_max_copy: u32,
        zero_copy_mark: u32,
    },
    ZeroCopySizeNotBelowMark {
        zero_copy_size: u32,
        zero_copy_mark: u32,
    },
    PacketQueueCapacityExceeded {
        queue: SinicQueueKind,
        capacity_bytes: u64,
        occupied_bytes: u64,
        packet_bytes: u64,
    },
    PacketQueueEmpty {
        queue: SinicQueueKind,
    },
    DmaCopyAlreadyPending {
        direction: SinicDmaDirection,
    },
    DmaCopyNotPending {
        direction: SinicDmaDirection,
    },
    DmaCopyLengthZero {
        direction: SinicDmaDirection,
    },
    DmaCompletionLengthMismatch {
        direction: SinicDmaDirection,
        expected_bytes: u32,
        actual_bytes: u64,
    },
    EthernetPeerBusy {
        interface: crate::EthernetInterfaceId,
    },
    Memory {
        source: rem6_memory::MemoryError,
    },
    PciBarBindingMismatch {
        expected_function: rem6_pci::PciFunctionAddress,
        actual_function: rem6_pci::PciFunctionAddress,
        expected_bar: rem6_pci::PciBarIndex,
        actual_bar: rem6_pci::PciBarIndex,
    },
    PciBarSizeMismatch {
        expected_bytes: u64,
        actual_bytes: u64,
    },
    PciEndpoint {
        source: rem6_pci::PciError,
    },
    Network {
        source: NetworkError,
    },
}

impl fmt::Display for SinicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescriptorAddressTooWide { address } => write!(
                formatter,
                "SINIC descriptor address {address:#x} exceeds 40 bits"
            ),
            Self::DescriptorLengthTooWide { len } => {
                write!(
                    formatter,
                    "SINIC descriptor length {len:#x} exceeds 20 bits"
                )
            }
            Self::ReservedInterruptBits {
                bits,
                reserved_bits,
            } => write!(
                formatter,
                "SINIC interrupt bits {bits:#x} include reserved bits {reserved_bits:#x}"
            ),
            Self::IncompatibleCopyModes => {
                write!(
                    formatter,
                    "SINIC zero-copy and delay-copy modes are exclusive"
                )
            }
            Self::RxMaxCopyBelowZeroCopyMark {
                rx_max_copy,
                zero_copy_mark,
            } => write!(
                formatter,
                "SINIC rx max copy {rx_max_copy} is below zero-copy mark {zero_copy_mark}"
            ),
            Self::ZeroCopySizeNotBelowMark {
                zero_copy_size,
                zero_copy_mark,
            } => write!(
                formatter,
                "SINIC zero-copy size {zero_copy_size} must be below mark {zero_copy_mark}"
            ),
            Self::PacketQueueCapacityExceeded {
                queue,
                capacity_bytes,
                occupied_bytes,
                packet_bytes,
            } => write!(
                formatter,
                "SINIC {queue} packet queue cannot fit {packet_bytes} bytes with capacity {capacity_bytes} and occupied {occupied_bytes}"
            ),
            Self::PacketQueueEmpty { queue } => {
                write!(formatter, "SINIC {queue} packet queue is empty")
            }
            Self::DmaCopyAlreadyPending { direction } => {
                write!(formatter, "SINIC {direction} DMA copy is already pending")
            }
            Self::DmaCopyNotPending { direction } => {
                write!(formatter, "SINIC {direction} DMA copy is not pending")
            }
            Self::DmaCopyLengthZero { direction } => {
                write!(formatter, "SINIC {direction} DMA copy length is zero")
            }
            Self::DmaCompletionLengthMismatch {
                direction,
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "SINIC {direction} DMA completion copied {actual_bytes} bytes but expected {expected_bytes}"
            ),
            Self::EthernetPeerBusy { interface } => write!(
                formatter,
                "SINIC ethernet interface {} peer is busy",
                interface.index()
            ),
            Self::Memory { source } => write!(formatter, "SINIC memory error: {source}"),
            Self::PciBarBindingMismatch {
                expected_function,
                actual_function,
                expected_bar,
                actual_bar,
            } => write!(
                formatter,
                "SINIC PCI BAR binding expected {:?} BAR {} but got {:?} BAR {}",
                expected_function,
                expected_bar.get(),
                actual_function,
                actual_bar.get()
            ),
            Self::PciBarSizeMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "SINIC PCI BAR binding expected {expected_bytes} bytes but got {actual_bytes}"
            ),
            Self::PciEndpoint { source } => {
                write!(formatter, "SINIC PCI endpoint error: {source}")
            }
            Self::Network { source } => write!(formatter, "SINIC network error: {source}"),
        }
    }
}

impl Error for SinicError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory { source } => Some(source),
            Self::PciEndpoint { source } => Some(source),
            Self::Network { source } => Some(source),
            _ => None,
        }
    }
}

fn validate_params(params: SinicRegisterParams) -> Result<(), SinicError> {
    let zero_copy = (params.config_bits & SinicRegisterBlock::CONFIG_ZERO_COPY) != 0;
    let delay_copy = (params.config_bits & SinicRegisterBlock::CONFIG_DELAY_COPY) != 0;
    if zero_copy && delay_copy {
        return Err(SinicError::IncompatibleCopyModes);
    }
    if params.rx_max_copy < params.zero_copy_mark {
        return Err(SinicError::RxMaxCopyBelowZeroCopyMark {
            rx_max_copy: params.rx_max_copy,
            zero_copy_mark: params.zero_copy_mark,
        });
    }
    if params.zero_copy_size >= params.zero_copy_mark {
        return Err(SinicError::ZeroCopySizeNotBelowMark {
            zero_copy_size: params.zero_copy_size,
            zero_copy_mark: params.zero_copy_mark,
        });
    }
    Ok(())
}

const fn set_u32_bit(bits: u32, offset: u32, enabled: bool) -> u32 {
    let mask = 1_u32 << offset;
    if enabled {
        bits | mask
    } else {
        bits & !mask
    }
}

const fn set_u64_field(bits: u64, offset: u64, width: u64, value: u64) -> u64 {
    let mask = ((1_u64 << width) - 1) << offset;
    (bits & !mask) | ((value << offset) & mask)
}
