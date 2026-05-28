use std::collections::VecDeque;
use std::fmt;

use crate::{
    EthernetInterfaceId, EthernetInterfaceRegistry, EthernetInterfaceSendRecord, EthernetPacket,
    NetworkError, SinicDoneStatus, SinicError, SinicInterruptRecord, SinicInterrupts,
    SinicRegisterBlock, SinicRegisterBlockSnapshot, SinicRegisterParams,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinicQueueKind {
    Receive,
    Transmit,
}

impl fmt::Display for SinicQueueKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receive => write!(formatter, "receive"),
            Self::Transmit => write!(formatter, "transmit"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicFifoDevice {
    registers: SinicRegisterBlock,
    rx_fifo: SinicPacketQueue,
    tx_fifo: SinicPacketQueue,
    rx_low: bool,
}

impl SinicFifoDevice {
    pub fn new(params: SinicRegisterParams) -> Result<Self, SinicError> {
        let registers = SinicRegisterBlock::new(params)?;
        Ok(Self {
            rx_fifo: SinicPacketQueue::new(
                SinicQueueKind::Receive,
                registers.rx_fifo_size() as u64,
            ),
            tx_fifo: SinicPacketQueue::new(
                SinicQueueKind::Transmit,
                registers.tx_fifo_size() as u64,
            ),
            registers,
            rx_low: true,
        })
    }

    pub const fn registers(&self) -> &SinicRegisterBlock {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut SinicRegisterBlock {
        &mut self.registers
    }

    pub fn receive_from_wire(
        &mut self,
        packet: EthernetPacket,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicReceiveRecord, SinicError> {
        if !self.registers.rx_enabled() {
            return Ok(SinicReceiveRecord {
                queued: false,
                rx_packet_count: self.rx_packet_count(),
                rx_occupied_bytes: self.rx_occupied_bytes(),
                interrupt_record: None,
            });
        }

        self.rx_fifo.check_push(&packet)?;
        let mut interrupt_record = None;
        if self.rx_fifo.occupied_bytes() >= self.registers.rx_fifo_high() as u64 {
            interrupt_record = Some(self.registers.post_interrupt(
                SinicInterrupts::RX_HIGH,
                current_tick,
                interrupt_delay_ticks,
            )?);
        }
        self.rx_fifo.push(packet)?;
        let packet_record = self.registers.post_interrupt(
            SinicInterrupts::RX_PACKET,
            current_tick,
            interrupt_delay_ticks,
        )?;
        interrupt_record = Some(merge_interrupt_records(interrupt_record, packet_record));

        Ok(SinicReceiveRecord {
            queued: true,
            rx_packet_count: self.rx_packet_count(),
            rx_occupied_bytes: self.rx_occupied_bytes(),
            interrupt_record,
        })
    }

    pub fn pop_rx_packet(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<Option<SinicRxPopRecord>, SinicError> {
        let Some(packet) = self.rx_fifo.pop() else {
            return Ok(None);
        };

        let mut interrupt_record = None;
        if self.rx_fifo.is_empty() {
            interrupt_record = Some(
                self.registers
                    .record_rx_empty(current_tick, interrupt_delay_ticks)?,
            );
        }
        if self.rx_fifo.occupied_bytes() < self.registers.rx_fifo_low() as u64 {
            self.rx_low = true;
        }

        Ok(Some(SinicRxPopRecord {
            packet,
            rx_packet_count: self.rx_packet_count(),
            rx_occupied_bytes: self.rx_occupied_bytes(),
            interrupt_record,
        }))
    }

    pub fn mark_rx_empty(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicInterruptRecord, SinicError> {
        self.registers
            .record_rx_empty(current_tick, interrupt_delay_ticks)
    }

    pub fn enqueue_tx_packet(
        &mut self,
        packet: EthernetPacket,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicTxEnqueueRecord, SinicError> {
        self.tx_fifo.check_push(&packet)?;
        self.tx_fifo.push(packet)?;

        let interrupt_record =
            if self.tx_fifo.available_bytes() < self.registers.tx_max_copy() as u64 {
                Some(
                    self.registers
                        .record_tx_full(current_tick, interrupt_delay_ticks)?,
                )
            } else {
                None
            };

        Ok(SinicTxEnqueueRecord {
            tx_packet_count: self.tx_packet_count(),
            tx_occupied_bytes: self.tx_occupied_bytes(),
            interrupt_record,
        })
    }

    pub fn transmit_one(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        interface: EthernetInterfaceId,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<Option<SinicTransmitRecord>, SinicError> {
        if self.tx_fifo.is_empty() {
            return Ok(None);
        }
        if registry.ask_busy(interface).map_err(network_error)? {
            return Err(SinicError::EthernetPeerBusy { interface });
        }
        let packet = self.tx_fifo.pop().ok_or(SinicError::PacketQueueEmpty {
            queue: SinicQueueKind::Transmit,
        })?;
        let send_record = registry
            .send_packet(interface, packet, current_tick)
            .map_err(network_error)?;
        let mut interrupt_record = Some(self.registers.post_interrupt(
            SinicInterrupts::TX_PACKET,
            current_tick,
            interrupt_delay_ticks,
        )?);
        if self.tx_fifo.occupied_bytes() < self.registers.tx_fifo_low() as u64 {
            let low_record = self.registers.post_interrupt(
                SinicInterrupts::TX_LOW,
                current_tick,
                interrupt_delay_ticks,
            )?;
            interrupt_record = Some(merge_interrupt_records(interrupt_record, low_record));
        }
        Ok(Some(SinicTransmitRecord {
            send_record,
            tx_packet_count: self.tx_packet_count(),
            tx_occupied_bytes: self.tx_occupied_bytes(),
            interrupt_record,
        }))
    }

    pub fn mark_tx_full(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicInterruptRecord, SinicError> {
        self.registers
            .record_tx_full(current_tick, interrupt_delay_ticks)
    }

    pub fn rx_done_status(&self) -> SinicDoneStatus {
        SinicDoneStatus::new()
            .with_packets(saturating_u16(self.rx_packet_count()))
            .with_empty(self.rx_fifo.is_empty())
            .with_high(self.rx_fifo.occupied_bytes() > self.registers.rx_fifo_high() as u64)
            .with_not_high(self.rx_low)
    }

    pub fn tx_done_status(&self) -> SinicDoneStatus {
        SinicDoneStatus::new()
            .with_packets(saturating_u16(self.tx_packet_count()))
            .with_full(self.tx_fifo.available_bytes() < self.registers.tx_max_copy() as u64)
            .with_low(self.tx_fifo.occupied_bytes() < self.registers.tx_fifo_low() as u64)
    }

    pub fn rx_packet_count(&self) -> usize {
        self.rx_fifo.packet_count()
    }

    pub fn tx_packet_count(&self) -> usize {
        self.tx_fifo.packet_count()
    }

    pub fn rx_occupied_bytes(&self) -> u64 {
        self.rx_fifo.occupied_bytes()
    }

    pub fn tx_occupied_bytes(&self) -> u64 {
        self.tx_fifo.occupied_bytes()
    }

    pub fn snapshot(&self) -> SinicFifoDeviceSnapshot {
        SinicFifoDeviceSnapshot {
            registers: self.registers.snapshot(),
            rx_fifo: self.rx_fifo.clone(),
            tx_fifo: self.tx_fifo.clone(),
            rx_low: self.rx_low,
        }
    }

    pub fn restore(&mut self, snapshot: &SinicFifoDeviceSnapshot) -> Result<(), SinicError> {
        self.registers.restore(&snapshot.registers)?;
        self.rx_fifo = snapshot.rx_fifo.clone();
        self.tx_fifo = snapshot.tx_fifo.clone();
        self.rx_low = snapshot.rx_low;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicReceiveRecord {
    queued: bool,
    rx_packet_count: usize,
    rx_occupied_bytes: u64,
    interrupt_record: Option<SinicInterruptRecord>,
}

impl SinicReceiveRecord {
    pub const fn queued(&self) -> bool {
        self.queued
    }

    pub const fn rx_packet_count(&self) -> usize {
        self.rx_packet_count
    }

    pub const fn rx_occupied_bytes(&self) -> u64 {
        self.rx_occupied_bytes
    }

    pub const fn interrupt_record(&self) -> Option<&SinicInterruptRecord> {
        self.interrupt_record.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicRxPopRecord {
    packet: EthernetPacket,
    rx_packet_count: usize,
    rx_occupied_bytes: u64,
    interrupt_record: Option<SinicInterruptRecord>,
}

impl SinicRxPopRecord {
    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }

    pub const fn rx_packet_count(&self) -> usize {
        self.rx_packet_count
    }

    pub const fn rx_occupied_bytes(&self) -> u64 {
        self.rx_occupied_bytes
    }

    pub const fn interrupt_record(&self) -> Option<&SinicInterruptRecord> {
        self.interrupt_record.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicTxEnqueueRecord {
    tx_packet_count: usize,
    tx_occupied_bytes: u64,
    interrupt_record: Option<SinicInterruptRecord>,
}

impl SinicTxEnqueueRecord {
    pub const fn tx_packet_count(&self) -> usize {
        self.tx_packet_count
    }

    pub const fn tx_occupied_bytes(&self) -> u64 {
        self.tx_occupied_bytes
    }

    pub const fn interrupt_record(&self) -> Option<&SinicInterruptRecord> {
        self.interrupt_record.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicTransmitRecord {
    send_record: EthernetInterfaceSendRecord,
    tx_packet_count: usize,
    tx_occupied_bytes: u64,
    interrupt_record: Option<SinicInterruptRecord>,
}

impl SinicTransmitRecord {
    pub const fn send_record(&self) -> &EthernetInterfaceSendRecord {
        &self.send_record
    }

    pub const fn tx_packet_count(&self) -> usize {
        self.tx_packet_count
    }

    pub const fn tx_occupied_bytes(&self) -> u64 {
        self.tx_occupied_bytes
    }

    pub const fn interrupt_record(&self) -> Option<&SinicInterruptRecord> {
        self.interrupt_record.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicFifoDeviceSnapshot {
    registers: SinicRegisterBlockSnapshot,
    rx_fifo: SinicPacketQueue,
    tx_fifo: SinicPacketQueue,
    rx_low: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SinicPacketQueue {
    kind: SinicQueueKind,
    capacity_bytes: u64,
    occupied_bytes: u64,
    packets: VecDeque<EthernetPacket>,
}

impl SinicPacketQueue {
    fn new(kind: SinicQueueKind, capacity_bytes: u64) -> Self {
        Self {
            kind,
            capacity_bytes,
            occupied_bytes: 0,
            packets: VecDeque::new(),
        }
    }

    fn check_push(&self, packet: &EthernetPacket) -> Result<(), SinicError> {
        let packet_bytes = packet.payload_len();
        if self.available_bytes() < packet_bytes {
            return Err(SinicError::PacketQueueCapacityExceeded {
                queue: self.kind,
                capacity_bytes: self.capacity_bytes,
                occupied_bytes: self.occupied_bytes,
                packet_bytes,
            });
        }
        Ok(())
    }

    fn push(&mut self, packet: EthernetPacket) -> Result<(), SinicError> {
        self.check_push(&packet)?;
        self.occupied_bytes = self.occupied_bytes.saturating_add(packet.payload_len());
        self.packets.push_back(packet);
        Ok(())
    }

    fn pop(&mut self) -> Option<EthernetPacket> {
        let packet = self.packets.pop_front()?;
        self.occupied_bytes = self.occupied_bytes.saturating_sub(packet.payload_len());
        Some(packet)
    }

    fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    fn packet_count(&self) -> usize {
        self.packets.len()
    }

    fn occupied_bytes(&self) -> u64 {
        self.occupied_bytes
    }

    fn available_bytes(&self) -> u64 {
        self.capacity_bytes.saturating_sub(self.occupied_bytes)
    }
}

fn merge_interrupt_records(
    prior: Option<SinicInterruptRecord>,
    next: SinicInterruptRecord,
) -> SinicInterruptRecord {
    let Some(prior) = prior else {
        return next;
    };
    let scheduled_tick = match (prior.scheduled_tick(), next.scheduled_tick()) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(tick), None) | (None, Some(tick)) => Some(tick),
        (None, None) => None,
    };
    SinicInterruptRecord::new(
        prior.requested_bits() | next.requested_bits(),
        next.status_bits(),
        prior.masked_bits() | next.masked_bits(),
        scheduled_tick,
    )
}

fn saturating_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn network_error(source: NetworkError) -> SinicError {
    SinicError::Network { source }
}
