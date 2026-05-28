use std::collections::VecDeque;
use std::fmt;

use crate::{
    EthernetInterfaceId, EthernetInterfaceRegistry, EthernetInterfaceSendRecord, EthernetPacket,
    NetworkError, SinicDataDescriptor, SinicDoneStatus, SinicError, SinicInterruptRecord,
    SinicInterrupts, SinicRegisterBlock, SinicRegisterBlockSnapshot, SinicRegisterParams,
    SinicRxStatus,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinicDmaDirection {
    Receive,
    Transmit,
}

impl fmt::Display for SinicDmaDirection {
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
    rx_data_descriptor: SinicDataDescriptor,
    tx_data_descriptor: SinicDataDescriptor,
    rx_dma_done: SinicDoneStatus,
    tx_dma_done: SinicDoneStatus,
    rx_dma_offset: u64,
    rx_dma_pending: Option<SinicDmaCopyPlan>,
    tx_dma_buffer: Vec<u8>,
    tx_dma_pending: Option<SinicDmaCopyPlan>,
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
            rx_data_descriptor: SinicDataDescriptor::from_bits(0),
            tx_data_descriptor: SinicDataDescriptor::from_bits(0),
            rx_dma_done: SinicDoneStatus::new(),
            tx_dma_done: SinicDoneStatus::new(),
            rx_dma_offset: 0,
            rx_dma_pending: None,
            tx_dma_buffer: Vec::new(),
            tx_dma_pending: None,
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

    pub fn begin_rx_dma_copy(
        &mut self,
        descriptor: SinicDataDescriptor,
    ) -> Result<Option<SinicDmaCopyPlan>, SinicError> {
        if self.rx_dma_pending.is_some() {
            return Err(SinicError::DmaCopyAlreadyPending {
                direction: SinicDmaDirection::Receive,
            });
        }
        self.rx_data_descriptor = descriptor;
        let Some(packet) = self.rx_fifo.front() else {
            return Ok(None);
        };
        let packet_remaining = packet.payload_len().saturating_sub(self.rx_dma_offset);
        let descriptor_len = descriptor.byte_len();
        if descriptor_len == 0 {
            return Err(SinicError::DmaCopyLengthZero {
                direction: SinicDmaDirection::Receive,
            });
        }

        let mut copy_len = u64::from(descriptor_len).min(packet_remaining);
        let zero_candidate = self.rx_zero_or_delay_copy_enabled()
            && !descriptor.no_delay()
            && self.rx_low
            && copy_len > u64::from(self.registers.zero_copy_mark());
        if zero_candidate {
            copy_len = u64::from(self.registers.zero_copy_size());
        }
        let plan = SinicDmaCopyPlan::new(
            SinicDmaDirection::Receive,
            descriptor,
            saturating_u32(copy_len),
            self.rx_dma_offset,
            zero_candidate,
        );
        self.rx_dma_done = SinicDoneStatus::new().with_busy(true);
        self.rx_dma_pending = Some(plan.clone());
        Ok(Some(plan))
    }

    pub fn complete_rx_dma_copy(
        &mut self,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicRxDmaCompletionRecord, SinicError> {
        let plan = self
            .rx_dma_pending
            .take()
            .ok_or(SinicError::DmaCopyNotPending {
                direction: SinicDmaDirection::Receive,
            })?;
        let copied_bytes = u64::from(plan.copy_len);
        let packet_len = self
            .rx_fifo
            .front()
            .ok_or(SinicError::PacketQueueEmpty {
                queue: SinicQueueKind::Receive,
            })?
            .payload_len();
        let copied_end = plan.packet_offset.saturating_add(copied_bytes);
        let remaining_packet_bytes = packet_len.saturating_sub(copied_end);

        if remaining_packet_bytes == 0 {
            self.rx_fifo.pop();
            self.rx_dma_offset = 0;
        } else {
            self.rx_dma_offset = copied_end;
        }
        if self.rx_fifo.occupied_bytes() < u64::from(self.registers.rx_fifo_low()) {
            self.rx_low = true;
        }
        if self.rx_fifo.occupied_bytes() > u64::from(self.registers.rx_fifo_high()) {
            self.rx_low = false;
        }

        let copy_len_for_status = if remaining_packet_bytes == 0 {
            plan.copy_len
        } else {
            saturating_u32(remaining_packet_bytes)
        };
        let done_status = SinicDoneStatus::new()
            .with_complete(true)
            .with_more(remaining_packet_bytes != 0)
            .with_copy_len(copy_len_for_status)?;
        self.rx_dma_done = done_status;
        let mut interrupt_record = Some(self.registers.post_interrupt(
            SinicInterrupts::RX_DMA,
            current_tick,
            interrupt_delay_ticks,
        )?);
        if self.rx_fifo.is_empty() {
            let empty_record = self
                .registers
                .record_rx_empty(current_tick, interrupt_delay_ticks)?;
            interrupt_record = Some(merge_interrupt_records(interrupt_record, empty_record));
        }

        Ok(SinicRxDmaCompletionRecord {
            plan,
            copied_bytes,
            remaining_packet_bytes,
            rx_packet_count: self.rx_packet_count(),
            rx_occupied_bytes: self.rx_occupied_bytes(),
            done_status,
            interrupt_record,
        })
    }

    pub fn begin_tx_dma_copy(
        &mut self,
        descriptor: SinicDataDescriptor,
    ) -> Result<SinicDmaCopyPlan, SinicError> {
        if self.tx_dma_pending.is_some() {
            return Err(SinicError::DmaCopyAlreadyPending {
                direction: SinicDmaDirection::Transmit,
            });
        }
        self.tx_data_descriptor = descriptor;
        let descriptor_len = descriptor.byte_len();
        if descriptor_len == 0 {
            return Err(SinicError::DmaCopyLengthZero {
                direction: SinicDmaDirection::Transmit,
            });
        }
        let plan = SinicDmaCopyPlan::new(
            SinicDmaDirection::Transmit,
            descriptor,
            descriptor_len,
            self.tx_dma_buffer.len() as u64,
            false,
        );
        self.tx_dma_done = SinicDoneStatus::new().with_busy(true);
        self.tx_dma_pending = Some(plan.clone());
        Ok(plan)
    }

    pub fn pending_rx_dma_payload(&self) -> Result<(SinicDmaCopyPlan, Vec<u8>), SinicError> {
        let plan = self
            .rx_dma_pending
            .clone()
            .ok_or(SinicError::DmaCopyNotPending {
                direction: SinicDmaDirection::Receive,
            })?;
        let packet = self.rx_fifo.front().ok_or(SinicError::PacketQueueEmpty {
            queue: SinicQueueKind::Receive,
        })?;
        let available = packet.payload_len().saturating_sub(plan.packet_offset);
        if available < u64::from(plan.copy_len) {
            return Err(SinicError::DmaCompletionLengthMismatch {
                direction: SinicDmaDirection::Receive,
                expected_bytes: plan.copy_len,
                actual_bytes: available,
            });
        }
        let start = plan.packet_offset as usize;
        let end = start.saturating_add(plan.copy_len as usize);
        Ok((plan, packet.payload()[start..end].to_vec()))
    }

    pub fn pending_tx_dma_copy_plan(&self) -> Result<SinicDmaCopyPlan, SinicError> {
        self.tx_dma_pending
            .clone()
            .ok_or(SinicError::DmaCopyNotPending {
                direction: SinicDmaDirection::Transmit,
            })
    }

    pub fn complete_tx_dma_copy(
        &mut self,
        bytes: &[u8],
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicTxDmaCompletionRecord, SinicError> {
        let plan = self
            .tx_dma_pending
            .take()
            .ok_or(SinicError::DmaCopyNotPending {
                direction: SinicDmaDirection::Transmit,
            })?;
        if bytes.len() as u64 != u64::from(plan.copy_len) {
            let expected_bytes = plan.copy_len;
            self.tx_dma_pending = Some(plan);
            return Err(SinicError::DmaCompletionLengthMismatch {
                direction: SinicDmaDirection::Transmit,
                expected_bytes,
                actual_bytes: bytes.len() as u64,
            });
        }

        if plan.more_fragment() {
            self.tx_dma_buffer.extend_from_slice(bytes);
            let done_status = SinicDoneStatus::new()
                .with_complete(true)
                .with_copy_len(plan.copy_len)?;
            self.tx_dma_done = done_status;
            let interrupt_record = Some(self.registers.post_interrupt(
                SinicInterrupts::TX_DMA,
                current_tick,
                interrupt_delay_ticks,
            )?);
            return Ok(SinicTxDmaCompletionRecord {
                plan,
                packet_complete: false,
                assembled_bytes: self.tx_dma_buffer.len() as u64,
                tx_packet_count: self.tx_packet_count(),
                tx_occupied_bytes: self.tx_occupied_bytes(),
                done_status,
                interrupt_record,
            });
        }

        let mut assembled = self.tx_dma_buffer.clone();
        assembled.extend_from_slice(bytes);
        let packet = EthernetPacket::new(assembled).map_err(network_error)?;
        let packet_bytes = packet.payload_len();
        self.tx_fifo.check_push(&packet)?;
        self.tx_dma_buffer.clear();
        let enqueue_record = self.enqueue_tx_packet(packet, current_tick, interrupt_delay_ticks)?;
        let dma_record = self.registers.post_interrupt(
            SinicInterrupts::TX_DMA,
            current_tick,
            interrupt_delay_ticks,
        )?;
        let done_status = SinicDoneStatus::new()
            .with_complete(true)
            .with_copy_len(plan.copy_len)?;
        self.tx_dma_done = done_status;
        let interrupt_record = Some(merge_interrupt_records(
            enqueue_record.interrupt_record,
            dma_record,
        ));

        Ok(SinicTxDmaCompletionRecord {
            plan,
            packet_complete: true,
            assembled_bytes: packet_bytes,
            tx_packet_count: self.tx_packet_count(),
            tx_occupied_bytes: self.tx_occupied_bytes(),
            done_status,
            interrupt_record,
        })
    }

    pub fn rx_done_status(&self) -> SinicDoneStatus {
        SinicDoneStatus::from_bits(self.rx_dma_done.bits())
            .with_packets(saturating_u16(self.rx_packet_count()))
            .with_empty(self.rx_fifo.is_empty())
            .with_high(self.rx_fifo.occupied_bytes() > self.registers.rx_fifo_high() as u64)
            .with_not_high(self.rx_low)
    }

    pub fn tx_done_status(&self) -> SinicDoneStatus {
        SinicDoneStatus::from_bits(self.tx_dma_done.bits())
            .with_packets(saturating_u16(self.tx_packet_count()))
            .with_full(self.tx_fifo.available_bytes() < self.registers.tx_max_copy() as u64)
            .with_low(self.tx_fifo.occupied_bytes() < self.registers.tx_fifo_low() as u64)
    }

    pub fn rx_status(&self) -> SinicRxStatus {
        let busy = u16::from(self.rx_dma_pending.is_some());
        let mapped = u16::from(self.rx_dma_offset > 0 || self.rx_dma_pending.is_some());
        let dirty = u16::from(self.rx_dma_offset > 0);
        let head = if self.rx_dma_offset > 0 { 0 } else { u16::MAX };
        SinicRxStatus::new()
            .with_dirty(dirty)
            .with_mapped(mapped)
            .with_busy(busy)
            .with_head(head)
    }

    pub const fn rx_data_descriptor(&self) -> SinicDataDescriptor {
        self.rx_data_descriptor
    }

    pub const fn tx_data_descriptor(&self) -> SinicDataDescriptor {
        self.tx_data_descriptor
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

    pub fn reset(&mut self) -> Result<(), SinicError> {
        let params = self.registers.params();
        *self = Self::new(params)?;
        Ok(())
    }

    fn rx_zero_or_delay_copy_enabled(&self) -> bool {
        let config_bits = self.registers.config_bits();
        (config_bits & SinicRegisterBlock::CONFIG_ZERO_COPY) != 0
            || (config_bits & SinicRegisterBlock::CONFIG_DELAY_COPY) != 0
    }

    pub fn snapshot(&self) -> SinicFifoDeviceSnapshot {
        SinicFifoDeviceSnapshot {
            registers: self.registers.snapshot(),
            rx_fifo: self.rx_fifo.clone(),
            tx_fifo: self.tx_fifo.clone(),
            rx_low: self.rx_low,
            rx_data_descriptor: self.rx_data_descriptor,
            tx_data_descriptor: self.tx_data_descriptor,
            rx_dma_done: self.rx_dma_done,
            tx_dma_done: self.tx_dma_done,
            rx_dma_offset: self.rx_dma_offset,
            rx_dma_pending: self.rx_dma_pending.clone(),
            tx_dma_buffer: self.tx_dma_buffer.clone(),
            tx_dma_pending: self.tx_dma_pending.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &SinicFifoDeviceSnapshot) -> Result<(), SinicError> {
        self.registers.restore(&snapshot.registers)?;
        self.rx_fifo = snapshot.rx_fifo.clone();
        self.tx_fifo = snapshot.tx_fifo.clone();
        self.rx_low = snapshot.rx_low;
        self.rx_data_descriptor = snapshot.rx_data_descriptor;
        self.tx_data_descriptor = snapshot.tx_data_descriptor;
        self.rx_dma_done = snapshot.rx_dma_done;
        self.tx_dma_done = snapshot.tx_dma_done;
        self.rx_dma_offset = snapshot.rx_dma_offset;
        self.rx_dma_pending = snapshot.rx_dma_pending.clone();
        self.tx_dma_buffer = snapshot.tx_dma_buffer.clone();
        self.tx_dma_pending = snapshot.tx_dma_pending.clone();
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicDmaCopyPlan {
    direction: SinicDmaDirection,
    descriptor: SinicDataDescriptor,
    copy_len: u32,
    packet_offset: u64,
    zero_limited: bool,
}

impl SinicDmaCopyPlan {
    const fn new(
        direction: SinicDmaDirection,
        descriptor: SinicDataDescriptor,
        copy_len: u32,
        packet_offset: u64,
        zero_limited: bool,
    ) -> Self {
        Self {
            direction,
            descriptor,
            copy_len,
            packet_offset,
            zero_limited,
        }
    }

    pub const fn direction(&self) -> SinicDmaDirection {
        self.direction
    }

    pub const fn descriptor(&self) -> SinicDataDescriptor {
        self.descriptor
    }

    pub const fn guest_address(&self) -> u64 {
        self.descriptor.address()
    }

    pub const fn copy_len(&self) -> u32 {
        self.copy_len
    }

    pub const fn packet_offset(&self) -> u64 {
        self.packet_offset
    }

    pub const fn zero_limited(&self) -> bool {
        self.zero_limited
    }

    pub const fn more_fragment(&self) -> bool {
        self.descriptor.more()
    }

    pub const fn checksum(&self) -> bool {
        self.descriptor.checksum()
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
pub struct SinicRxDmaCompletionRecord {
    plan: SinicDmaCopyPlan,
    copied_bytes: u64,
    remaining_packet_bytes: u64,
    rx_packet_count: usize,
    rx_occupied_bytes: u64,
    done_status: SinicDoneStatus,
    interrupt_record: Option<SinicInterruptRecord>,
}

impl SinicRxDmaCompletionRecord {
    pub const fn plan(&self) -> &SinicDmaCopyPlan {
        &self.plan
    }

    pub const fn copied_bytes(&self) -> u64 {
        self.copied_bytes
    }

    pub const fn remaining_packet_bytes(&self) -> u64 {
        self.remaining_packet_bytes
    }

    pub const fn rx_packet_count(&self) -> usize {
        self.rx_packet_count
    }

    pub const fn rx_occupied_bytes(&self) -> u64 {
        self.rx_occupied_bytes
    }

    pub const fn done_status(&self) -> SinicDoneStatus {
        self.done_status
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicTxDmaCompletionRecord {
    plan: SinicDmaCopyPlan,
    packet_complete: bool,
    assembled_bytes: u64,
    tx_packet_count: usize,
    tx_occupied_bytes: u64,
    done_status: SinicDoneStatus,
    interrupt_record: Option<SinicInterruptRecord>,
}

impl SinicTxDmaCompletionRecord {
    pub const fn plan(&self) -> &SinicDmaCopyPlan {
        &self.plan
    }

    pub const fn packet_complete(&self) -> bool {
        self.packet_complete
    }

    pub const fn assembled_bytes(&self) -> u64 {
        self.assembled_bytes
    }

    pub const fn tx_packet_count(&self) -> usize {
        self.tx_packet_count
    }

    pub const fn tx_occupied_bytes(&self) -> u64 {
        self.tx_occupied_bytes
    }

    pub const fn done_status(&self) -> SinicDoneStatus {
        self.done_status
    }

    pub const fn interrupt_record(&self) -> Option<&SinicInterruptRecord> {
        self.interrupt_record.as_ref()
    }
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
    rx_data_descriptor: SinicDataDescriptor,
    tx_data_descriptor: SinicDataDescriptor,
    rx_dma_done: SinicDoneStatus,
    tx_dma_done: SinicDoneStatus,
    rx_dma_offset: u64,
    rx_dma_pending: Option<SinicDmaCopyPlan>,
    tx_dma_buffer: Vec<u8>,
    tx_dma_pending: Option<SinicDmaCopyPlan>,
}

impl SinicFifoDeviceSnapshot {
    pub fn rx_packet_count(&self) -> usize {
        self.rx_fifo.packet_count()
    }

    pub fn tx_packet_count(&self) -> usize {
        self.tx_fifo.packet_count()
    }
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

    fn front(&self) -> Option<&EthernetPacket> {
        self.packets.front()
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

fn saturating_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn network_error(source: NetworkError) -> SinicError {
    SinicError::Network { source }
}
