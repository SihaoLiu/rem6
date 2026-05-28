use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

mod bus;
mod distributed;
mod dump;
mod interface;
mod switch;
mod tap;

pub use bus::*;
pub use distributed::*;
pub use dump::*;
pub use interface::*;
pub use switch::*;
pub use tap::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EthernetPacketHandle(u64);

impl EthernetPacketHandle {
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetPacket {
    payload: Vec<u8>,
    wire_length_bytes: u64,
}

impl EthernetPacket {
    pub fn new(payload: Vec<u8>) -> Result<Self, NetworkError> {
        if payload.is_empty() {
            return Err(NetworkError::EmptyPacket);
        }
        Ok(Self {
            wire_length_bytes: payload.len() as u64,
            payload,
        })
    }

    pub fn with_wire_length_bytes(mut self, wire_length_bytes: u64) -> Result<Self, NetworkError> {
        if wire_length_bytes == 0 {
            return Err(NetworkError::InvalidWireLength {
                payload_bytes: self.payload_len(),
                wire_bytes: wire_length_bytes,
            });
        }
        self.wire_length_bytes = wire_length_bytes;
        Ok(self)
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_len(&self) -> u64 {
        self.payload.len() as u64
    }

    pub const fn wire_length_bytes(&self) -> u64 {
        self.wire_length_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EthernetLinkDirection {
    LeftToRight,
    RightToLeft,
}

impl fmt::Display for EthernetLinkDirection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftToRight => write!(formatter, "left-to-right"),
            Self::RightToLeft => write!(formatter, "right-to-left"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EthernetLinkTiming {
    ticks_per_byte: u64,
    link_delay_ticks: u64,
}

impl EthernetLinkTiming {
    pub fn new(ticks_per_byte: u64, link_delay_ticks: u64) -> Result<Self, NetworkError> {
        if ticks_per_byte == 0 {
            return Err(NetworkError::InvalidEthernetLinkRate { ticks_per_byte });
        }
        Ok(Self {
            ticks_per_byte,
            link_delay_ticks,
        })
    }

    pub const fn ticks_per_byte(&self) -> u64 {
        self.ticks_per_byte
    }

    pub const fn link_delay_ticks(&self) -> u64 {
        self.link_delay_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetLinkDelayVariation {
    max_delay_ticks: u64,
    delay_ticks: Vec<u64>,
    next_index: usize,
}

impl EthernetLinkDelayVariation {
    pub fn new(max_delay_ticks: u64, delay_ticks: Vec<u64>) -> Result<Self, NetworkError> {
        for delay_ticks in &delay_ticks {
            if *delay_ticks > max_delay_ticks {
                return Err(NetworkError::InvalidEthernetLinkDelayVariation {
                    max_delay_ticks,
                    delay_ticks: *delay_ticks,
                });
            }
        }
        Ok(Self {
            max_delay_ticks,
            delay_ticks,
            next_index: 0,
        })
    }

    pub const fn max_delay_ticks(&self) -> u64 {
        self.max_delay_ticks
    }

    pub fn delay_count(&self) -> usize {
        self.delay_ticks.len()
    }

    pub const fn next_index(&self) -> usize {
        self.next_index
    }

    pub(crate) fn peek_delay_ticks(&self) -> u64 {
        self.delay_ticks.get(self.next_index).copied().unwrap_or(0)
    }

    pub(crate) fn advance_delay(&mut self) {
        if !self.delay_ticks.is_empty() {
            self.next_index = (self.next_index + 1) % self.delay_ticks.len();
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetFullDuplexLink {
    timing: EthernetLinkTiming,
    next_sequence: u64,
    left_to_right: EthernetLinkLane,
    right_to_left: EthernetLinkLane,
}

impl EthernetFullDuplexLink {
    pub const fn new(timing: EthernetLinkTiming) -> Self {
        Self {
            timing,
            next_sequence: 0,
            left_to_right: EthernetLinkLane::new(),
            right_to_left: EthernetLinkLane::new(),
        }
    }

    pub fn new_with_delay_variation(
        timing: EthernetLinkTiming,
        delay_variation: EthernetLinkDelayVariation,
    ) -> Self {
        Self {
            timing,
            next_sequence: 0,
            left_to_right: EthernetLinkLane::new_with_delay_variation(delay_variation.clone()),
            right_to_left: EthernetLinkLane::new_with_delay_variation(delay_variation),
        }
    }

    pub const fn timing(&self) -> EthernetLinkTiming {
        self.timing
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn is_busy(&self, direction: EthernetLinkDirection, tick: u64) -> bool {
        self.busy_until_tick(direction)
            .is_some_and(|busy_until_tick| tick < busy_until_tick)
    }

    pub fn busy_until_tick(&self, direction: EthernetLinkDirection) -> Option<u64> {
        self.lane(direction).busy_until_tick
    }

    pub fn pending_delivery_count(&self) -> usize {
        self.left_to_right
            .pending_deliveries
            .len()
            .saturating_add(self.right_to_left.pending_deliveries.len())
    }

    pub fn transmit(
        &mut self,
        direction: EthernetLinkDirection,
        packet: EthernetPacket,
        request_tick: u64,
    ) -> Result<EthernetTransmission, NetworkError> {
        if let Some(busy_until_tick) = self.busy_until_tick(direction) {
            if request_tick < busy_until_tick {
                return Err(NetworkError::EthernetLinkBusy {
                    direction,
                    request_tick,
                    busy_until_tick,
                });
            }
        }

        let serialization_ticks = self
            .timing
            .ticks_per_byte
            .checked_mul(packet.wire_length_bytes())
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(NetworkError::EthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.timing.ticks_per_byte,
                link_delay_ticks: self.timing.link_delay_ticks,
            })?;
        let serialization_done_tick = request_tick.checked_add(serialization_ticks).ok_or(
            NetworkError::EthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.timing.ticks_per_byte,
                link_delay_ticks: self.timing.link_delay_ticks,
            },
        )?;
        let delay_variation_ticks = self.lane(direction).peek_delay_variation_ticks();
        let transmit_done_tick = serialization_done_tick
            .checked_add(delay_variation_ticks)
            .ok_or(NetworkError::EthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.timing.ticks_per_byte,
                link_delay_ticks: self.timing.link_delay_ticks,
            })?;
        let delivery_tick = transmit_done_tick
            .checked_add(self.timing.link_delay_ticks)
            .ok_or(NetworkError::EthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.timing.ticks_per_byte,
                link_delay_ticks: self.timing.link_delay_ticks,
            })?;

        let transmission = EthernetTransmission {
            sequence: self.next_sequence,
            direction,
            request_tick,
            serialization_done_tick,
            delay_variation_ticks,
            transmit_done_tick,
            delivery_tick,
            packet,
        };
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkError::EthernetLinkSequenceOverflow)?;

        let lane = self.lane_mut(direction);
        lane.advance_delay_variation();
        lane.busy_until_tick = Some(transmit_done_tick);
        lane.pending_deliveries.push_back(transmission.clone());
        Ok(transmission)
    }

    pub fn drain_ready_deliveries(&mut self, up_to_tick: u64) -> Vec<EthernetTransmission> {
        let mut ready = Vec::new();
        self.left_to_right.drain_ready(up_to_tick, &mut ready);
        self.right_to_left.drain_ready(up_to_tick, &mut ready);
        ready.sort_by_key(|delivery| (delivery.delivery_tick, delivery.sequence));
        ready
    }

    pub fn snapshot(&self) -> EthernetFullDuplexLinkSnapshot {
        EthernetFullDuplexLinkSnapshot {
            timing: self.timing,
            next_sequence: self.next_sequence,
            left_to_right: self.left_to_right.clone(),
            right_to_left: self.right_to_left.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &EthernetFullDuplexLinkSnapshot,
    ) -> Result<(), NetworkError> {
        self.timing = snapshot.timing;
        self.next_sequence = snapshot.next_sequence;
        self.left_to_right = snapshot.left_to_right.clone();
        self.right_to_left = snapshot.right_to_left.clone();
        Ok(())
    }

    fn lane(&self, direction: EthernetLinkDirection) -> &EthernetLinkLane {
        match direction {
            EthernetLinkDirection::LeftToRight => &self.left_to_right,
            EthernetLinkDirection::RightToLeft => &self.right_to_left,
        }
    }

    fn lane_mut(&mut self, direction: EthernetLinkDirection) -> &mut EthernetLinkLane {
        match direction {
            EthernetLinkDirection::LeftToRight => &mut self.left_to_right,
            EthernetLinkDirection::RightToLeft => &mut self.right_to_left,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetFullDuplexLinkSnapshot {
    timing: EthernetLinkTiming,
    next_sequence: u64,
    left_to_right: EthernetLinkLane,
    right_to_left: EthernetLinkLane,
}

impl EthernetFullDuplexLinkSnapshot {
    pub const fn timing(&self) -> EthernetLinkTiming {
        self.timing
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn pending_delivery_count(&self) -> usize {
        self.left_to_right
            .pending_deliveries
            .len()
            .saturating_add(self.right_to_left.pending_deliveries.len())
    }

    pub fn busy_until_tick(&self, direction: EthernetLinkDirection) -> Option<u64> {
        match direction {
            EthernetLinkDirection::LeftToRight => self.left_to_right.busy_until_tick,
            EthernetLinkDirection::RightToLeft => self.right_to_left.busy_until_tick,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetTransmission {
    sequence: u64,
    direction: EthernetLinkDirection,
    request_tick: u64,
    serialization_done_tick: u64,
    delay_variation_ticks: u64,
    transmit_done_tick: u64,
    delivery_tick: u64,
    packet: EthernetPacket,
}

impl EthernetTransmission {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn direction(&self) -> EthernetLinkDirection {
        self.direction
    }

    pub const fn request_tick(&self) -> u64 {
        self.request_tick
    }

    pub const fn serialization_done_tick(&self) -> u64 {
        self.serialization_done_tick
    }

    pub const fn delay_variation_ticks(&self) -> u64 {
        self.delay_variation_ticks
    }

    pub const fn transmit_done_tick(&self) -> u64 {
        self.transmit_done_tick
    }

    pub const fn delivery_tick(&self) -> u64 {
        self.delivery_tick
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }

    pub fn into_packet(self) -> EthernetPacket {
        self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetLinkLane {
    busy_until_tick: Option<u64>,
    delay_variation: Option<EthernetLinkDelayVariation>,
    pending_deliveries: VecDeque<EthernetTransmission>,
}

impl EthernetLinkLane {
    const fn new() -> Self {
        Self {
            busy_until_tick: None,
            delay_variation: None,
            pending_deliveries: VecDeque::new(),
        }
    }

    fn new_with_delay_variation(delay_variation: EthernetLinkDelayVariation) -> Self {
        Self {
            busy_until_tick: None,
            delay_variation: Some(delay_variation),
            pending_deliveries: VecDeque::new(),
        }
    }

    fn peek_delay_variation_ticks(&self) -> u64 {
        self.delay_variation
            .as_ref()
            .map(EthernetLinkDelayVariation::peek_delay_ticks)
            .unwrap_or(0)
    }

    fn advance_delay_variation(&mut self) {
        if let Some(delay_variation) = &mut self.delay_variation {
            delay_variation.advance_delay();
        }
    }

    fn drain_ready(&mut self, up_to_tick: u64, ready: &mut Vec<EthernetTransmission>) {
        while self
            .pending_deliveries
            .front()
            .is_some_and(|delivery| delivery.delivery_tick <= up_to_tick)
        {
            let delivery = self
                .pending_deliveries
                .pop_front()
                .expect("ready ethernet link delivery exists");
            ready.push(delivery);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetPacketFifo {
    capacity_bytes: u64,
    occupied_bytes: u64,
    reserved_bytes: u64,
    next_sequence: u64,
    entries: VecDeque<EthernetPacketFifoEntry>,
}

impl EthernetPacketFifo {
    pub fn new(capacity_bytes: u64) -> Result<Self, NetworkError> {
        if capacity_bytes == 0 {
            return Err(NetworkError::ZeroFifoCapacity);
        }
        Ok(Self {
            capacity_bytes,
            occupied_bytes: 0,
            reserved_bytes: 0,
            next_sequence: 0,
            entries: VecDeque::new(),
        })
    }

    pub const fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes
    }

    pub fn packet_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn queued_payload_bytes(&self) -> u64 {
        self.entries
            .iter()
            .map(|entry| entry.packet.payload_len())
            .fold(0, u64::saturating_add)
    }

    pub const fn occupied_bytes(&self) -> u64 {
        self.occupied_bytes
    }

    pub const fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }

    pub fn available_bytes(&self) -> u64 {
        self.capacity_bytes
            .saturating_sub(self.occupied_bytes.saturating_add(self.reserved_bytes))
    }

    pub fn reserve(&mut self, bytes: u64) -> Result<(), NetworkError> {
        if self.available_bytes() < bytes {
            return Err(NetworkError::ReservationExceedsAvailable {
                capacity_bytes: self.capacity_bytes,
                occupied_bytes: self.occupied_bytes,
                reserved_bytes: self.reserved_bytes,
                requested_bytes: bytes,
            });
        }
        self.reserved_bytes = self.reserved_bytes.saturating_add(bytes);
        Ok(())
    }

    pub fn push(&mut self, packet: EthernetPacket) -> Result<EthernetPacketHandle, NetworkError> {
        let packet_bytes = packet.payload_len();
        if self.reserved_bytes > packet_bytes {
            return Err(NetworkError::ReservationExceedsPacket {
                reserved_bytes: self.reserved_bytes,
                packet_bytes,
            });
        }
        let additional_bytes = packet_bytes.saturating_sub(self.reserved_bytes);
        if self.available_bytes() < additional_bytes {
            return Err(NetworkError::FifoCapacityExceeded {
                capacity_bytes: self.capacity_bytes,
                occupied_bytes: self.occupied_bytes,
                reserved_bytes: self.reserved_bytes,
                packet_bytes,
            });
        }
        let handle = EthernetPacketHandle(self.next_sequence);
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkError::PacketSequenceOverflow)?;
        self.occupied_bytes = self.occupied_bytes.saturating_add(packet_bytes);
        self.reserved_bytes = 0;
        self.entries.push_back(EthernetPacketFifoEntry {
            handle,
            packet,
            slack_bytes: 0,
        });
        Ok(handle)
    }

    pub fn front(&self) -> Option<&EthernetPacket> {
        self.entries.front().map(|entry| &entry.packet)
    }

    pub fn front_handle(&self) -> Option<EthernetPacketHandle> {
        self.entries.front().map(|entry| entry.handle)
    }

    pub fn pop(&mut self) -> Option<EthernetPacket> {
        let entry = self.entries.pop_front()?;
        self.occupied_bytes = self
            .occupied_bytes
            .saturating_sub(entry.packet.payload_len().saturating_add(entry.slack_bytes));
        Some(entry.packet)
    }

    pub fn remove(&mut self, handle: EthernetPacketHandle) -> Result<EthernetPacket, NetworkError> {
        let index = self
            .entry_index(handle)
            .ok_or(NetworkError::UnknownPacketHandle { handle })?;
        let entry = self
            .entries
            .remove(index)
            .expect("validated ethernet packet fifo entry index");
        let occupied = entry.packet.payload_len().saturating_add(entry.slack_bytes);
        if index == 0 {
            self.occupied_bytes = self.occupied_bytes.saturating_sub(occupied);
        } else {
            let previous = self
                .entries
                .get_mut(index - 1)
                .expect("validated ethernet packet fifo previous entry");
            previous.slack_bytes = previous.slack_bytes.saturating_add(occupied);
        }
        Ok(entry.packet)
    }

    pub fn entry_slack_bytes(&self, handle: EthernetPacketHandle) -> Result<u64, NetworkError> {
        self.entries
            .iter()
            .find(|entry| entry.handle == handle)
            .map(|entry| entry.slack_bytes)
            .ok_or(NetworkError::UnknownPacketHandle { handle })
    }

    pub fn count_packets_before(
        &self,
        handle: EthernetPacketHandle,
    ) -> Result<usize, NetworkError> {
        self.entry_index(handle)
            .ok_or(NetworkError::UnknownPacketHandle { handle })
    }

    pub fn count_packets_after(&self, handle: EthernetPacketHandle) -> Result<usize, NetworkError> {
        let index = self
            .entry_index(handle)
            .ok_or(NetworkError::UnknownPacketHandle { handle })?;
        Ok(self.entries.len().saturating_sub(index + 1))
    }

    pub fn copy_payload_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, NetworkError> {
        let queued_payload_bytes = self.queued_payload_bytes();
        let end = offset
            .checked_add(len)
            .ok_or(NetworkError::PayloadRangeOutOfBounds {
                offset,
                len,
                queued_payload_bytes,
            })?;
        if end > queued_payload_bytes {
            return Err(NetworkError::PayloadRangeOutOfBounds {
                offset,
                len,
                queued_payload_bytes,
            });
        }

        let mut remaining_offset = offset;
        let mut remaining_len = len;
        let mut copied = Vec::with_capacity(len as usize);
        for entry in &self.entries {
            if remaining_len == 0 {
                break;
            }
            let payload = entry.packet.payload();
            let payload_len = payload.len() as u64;
            if remaining_offset >= payload_len {
                remaining_offset -= payload_len;
                continue;
            }
            let start = remaining_offset as usize;
            let count = remaining_len.min(payload_len - remaining_offset) as usize;
            copied.extend_from_slice(&payload[start..start + count]);
            remaining_len -= count as u64;
            remaining_offset = 0;
        }
        Ok(copied)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.occupied_bytes = 0;
        self.reserved_bytes = 0;
    }

    pub fn snapshot(&self) -> EthernetPacketFifoSnapshot {
        EthernetPacketFifoSnapshot {
            capacity_bytes: self.capacity_bytes,
            occupied_bytes: self.occupied_bytes,
            reserved_bytes: self.reserved_bytes,
            next_sequence: self.next_sequence,
            entries: self.entries.iter().cloned().collect(),
        }
    }

    pub fn restore(&mut self, snapshot: &EthernetPacketFifoSnapshot) -> Result<(), NetworkError> {
        if snapshot.capacity_bytes != self.capacity_bytes {
            return Err(NetworkError::SnapshotCapacityMismatch {
                fifo_capacity_bytes: self.capacity_bytes,
                snapshot_capacity_bytes: snapshot.capacity_bytes,
            });
        }
        let occupied_bytes = snapshot
            .entries
            .iter()
            .map(EthernetPacketFifoEntry::occupied_bytes)
            .fold(0, u64::saturating_add);
        if occupied_bytes != snapshot.occupied_bytes
            || occupied_bytes.saturating_add(snapshot.reserved_bytes) > snapshot.capacity_bytes
        {
            return Err(NetworkError::InvalidSnapshotOccupancy {
                capacity_bytes: snapshot.capacity_bytes,
                occupied_bytes: snapshot.occupied_bytes,
                reserved_bytes: snapshot.reserved_bytes,
            });
        }
        self.occupied_bytes = snapshot.occupied_bytes;
        self.reserved_bytes = snapshot.reserved_bytes;
        self.next_sequence = snapshot.next_sequence;
        self.entries = snapshot.entries.iter().cloned().collect();
        Ok(())
    }

    fn entry_index(&self, handle: EthernetPacketHandle) -> Option<usize> {
        self.entries.iter().position(|entry| entry.handle == handle)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetPacketFifoSnapshot {
    capacity_bytes: u64,
    occupied_bytes: u64,
    reserved_bytes: u64,
    next_sequence: u64,
    entries: Vec<EthernetPacketFifoEntry>,
}

impl EthernetPacketFifoSnapshot {
    pub const fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes
    }

    pub const fn occupied_bytes(&self) -> u64 {
        self.occupied_bytes
    }

    pub const fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetPacketFifoEntry {
    handle: EthernetPacketHandle,
    packet: EthernetPacket,
    slack_bytes: u64,
}

impl EthernetPacketFifoEntry {
    fn occupied_bytes(&self) -> u64 {
        self.packet.payload_len().saturating_add(self.slack_bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkError {
    EmptyPacket,
    InvalidWireLength {
        payload_bytes: u64,
        wire_bytes: u64,
    },
    ZeroFifoCapacity,
    ReservationExceedsAvailable {
        capacity_bytes: u64,
        occupied_bytes: u64,
        reserved_bytes: u64,
        requested_bytes: u64,
    },
    ReservationExceedsPacket {
        reserved_bytes: u64,
        packet_bytes: u64,
    },
    FifoCapacityExceeded {
        capacity_bytes: u64,
        occupied_bytes: u64,
        reserved_bytes: u64,
        packet_bytes: u64,
    },
    UnknownPacketHandle {
        handle: EthernetPacketHandle,
    },
    PayloadRangeOutOfBounds {
        offset: u64,
        len: u64,
        queued_payload_bytes: u64,
    },
    SnapshotCapacityMismatch {
        fifo_capacity_bytes: u64,
        snapshot_capacity_bytes: u64,
    },
    InvalidSnapshotOccupancy {
        capacity_bytes: u64,
        occupied_bytes: u64,
        reserved_bytes: u64,
    },
    PacketSequenceOverflow,
    InvalidEthernetLinkRate {
        ticks_per_byte: u64,
    },
    InvalidEthernetLinkDelayVariation {
        max_delay_ticks: u64,
        delay_ticks: u64,
    },
    EthernetLinkBusy {
        direction: EthernetLinkDirection,
        request_tick: u64,
        busy_until_tick: u64,
    },
    EthernetLinkTimingOverflow {
        request_tick: u64,
        wire_length_bytes: u64,
        ticks_per_byte: u64,
        link_delay_ticks: u64,
    },
    EthernetLinkSequenceOverflow,
    InvalidEthernetBusPortCount {
        port_count: u16,
    },
    InvalidEthernetBusRate {
        ticks_per_byte: u64,
    },
    UnknownEthernetBusPort {
        port: EthernetBusPortId,
        port_count: u16,
    },
    EthernetBusBusy {
        sender_port: EthernetBusPortId,
        request_tick: u64,
        busy_until_tick: u64,
    },
    EthernetBusTimingOverflow {
        request_tick: u64,
        wire_length_bytes: u64,
        ticks_per_byte: u64,
    },
    EthernetBusSequenceOverflow,
    InvalidEthernetPcapMaxCaptureBytes {
        max_capture_bytes: u32,
    },
    InvalidEthernetPcapClock {
        ticks_per_second: u64,
    },
    EthernetPcapTimestampOverflow {
        tick: u64,
        ticks_per_second: u64,
    },
    EthernetPcapPacketLengthOverflow {
        payload_bytes: u64,
    },
    EthernetPcapSequenceOverflow,
    DuplicateEthernetInterfaceName {
        name: String,
    },
    EthernetInterfaceCountOverflow {
        interface_count: usize,
    },
    UnknownEthernetInterface {
        interface: EthernetInterfaceId,
        interface_count: usize,
    },
    EthernetInterfaceSelfBinding {
        interface: EthernetInterfaceId,
    },
    EthernetInterfacePeerAlreadyBound {
        interface: EthernetInterfaceId,
        current_peer: EthernetInterfaceId,
        requested_peer: EthernetInterfaceId,
    },
    EthernetInterfacePeerMissing {
        interface: EthernetInterfaceId,
    },
    InvalidEthernetTapMaxFrameBytes {
        max_frame_bytes: u32,
    },
    EthernetTapEmptyFrame,
    EthernetTapFrameTooLarge {
        frame_bytes: u32,
        max_frame_bytes: u32,
    },
    EthernetTapFrameLengthOverflow {
        frame_bytes: u64,
    },
    DistributedEthernetHeaderTooShort {
        bytes: usize,
        header_bytes: usize,
    },
    DistributedEthernetBadMagic {
        magic: [u8; 4],
    },
    UnknownDistributedEthernetMessageKind {
        kind: u8,
    },
    UnknownDistributedEthernetRequestType {
        req_type: u8,
    },
    DistributedEthernetPayloadLengthMismatch {
        expected_bytes: u64,
        actual_bytes: u64,
    },
    DistributedEthernetSequenceOverflow,
    DistributedEthernetLinkBusy {
        interface: EthernetInterfaceId,
        request_tick: u64,
        busy_until_tick: u64,
    },
    DistributedEthernetLinkTimingOverflow {
        request_tick: u64,
        wire_length_bytes: u64,
        ticks_per_byte: u64,
        delay_variation_ticks: u64,
    },
    DistributedEthernetLinkSequenceOverflow,
    InvalidDistributedEthernetReceiveWindow {
        previous_sync_tick: u64,
        next_sync_tick: u64,
    },
    DistributedEthernetReceiveMessageNotData {
        kind: DistributedEthernetMessageKind,
    },
    DistributedEthernetReceiveTimingOverflow {
        send_tick: u64,
        send_delay_ticks: u64,
        link_delay_ticks: u64,
    },
    DistributedEthernetReceiveWindowTooSmall {
        previous_receive_tick: u64,
        send_delay_ticks: u64,
        receive_tick: u64,
    },
    DistributedEthernetReceiveMissed {
        current_tick: u64,
        receive_tick: u64,
    },
    DistributedEthernetReceiveOutOfOrder {
        queued_ready_tick: u64,
        receive_tick: u64,
    },
    DistributedEthernetSendOutsideReceiveWindow {
        send_tick: u64,
        previous_sync_tick: u64,
    },
    DistributedEthernetReceiveInsideSyncWindow {
        receive_tick: u64,
        next_sync_tick: u64,
    },
    InvalidEthernetSwitchPortCount {
        port_count: u16,
    },
    InvalidEthernetSwitchRate {
        ticks_per_byte: u64,
    },
    EthernetSwitchTimingOverflow {
        wire_length_bytes: u64,
        ticks_per_byte: u64,
        switch_delay_ticks: u64,
    },
    UnknownEthernetSwitchPort {
        port: EthernetSwitchPortId,
        port_count: usize,
    },
    EthernetFrameTooShort {
        payload_bytes: u64,
    },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPacket => write!(formatter, "ethernet packet payload must be nonempty"),
            Self::InvalidWireLength {
                payload_bytes,
                wire_bytes,
            } => write!(
                formatter,
                "ethernet packet wire length {wire_bytes} is invalid for {payload_bytes} payload bytes"
            ),
            Self::ZeroFifoCapacity => write!(formatter, "ethernet packet FIFO capacity must be positive"),
            Self::ReservationExceedsAvailable {
                capacity_bytes,
                occupied_bytes,
                reserved_bytes,
                requested_bytes,
            } => write!(
                formatter,
                "ethernet packet FIFO reservation {requested_bytes} exceeds available capacity: capacity {capacity_bytes}, occupied {occupied_bytes}, reserved {reserved_bytes}"
            ),
            Self::ReservationExceedsPacket {
                reserved_bytes,
                packet_bytes,
            } => write!(
                formatter,
                "ethernet packet FIFO reservation {reserved_bytes} exceeds packet payload {packet_bytes}"
            ),
            Self::FifoCapacityExceeded {
                capacity_bytes,
                occupied_bytes,
                reserved_bytes,
                packet_bytes,
            } => write!(
                formatter,
                "ethernet packet FIFO cannot enqueue {packet_bytes} bytes with capacity {capacity_bytes}, occupied {occupied_bytes}, reserved {reserved_bytes}"
            ),
            Self::UnknownPacketHandle { handle } => {
                write!(formatter, "unknown ethernet packet FIFO handle {}", handle.sequence())
            }
            Self::PayloadRangeOutOfBounds {
                offset,
                len,
                queued_payload_bytes,
            } => write!(
                formatter,
                "ethernet packet FIFO payload range offset {offset} length {len} exceeds queued payload bytes {queued_payload_bytes}"
            ),
            Self::SnapshotCapacityMismatch {
                fifo_capacity_bytes,
                snapshot_capacity_bytes,
            } => write!(
                formatter,
                "ethernet packet FIFO snapshot capacity {snapshot_capacity_bytes} does not match FIFO capacity {fifo_capacity_bytes}"
            ),
            Self::InvalidSnapshotOccupancy {
                capacity_bytes,
                occupied_bytes,
                reserved_bytes,
            } => write!(
                formatter,
                "ethernet packet FIFO snapshot occupancy is invalid: capacity {capacity_bytes}, occupied {occupied_bytes}, reserved {reserved_bytes}"
            ),
            Self::PacketSequenceOverflow => write!(formatter, "ethernet packet FIFO sequence overflow"),
            Self::InvalidEthernetLinkRate { ticks_per_byte } => write!(
                formatter,
                "ethernet link ticks per byte {ticks_per_byte} must be positive"
            ),
            Self::InvalidEthernetLinkDelayVariation {
                max_delay_ticks,
                delay_ticks,
            } => write!(
                formatter,
                "ethernet link delay variation {delay_ticks} exceeds maximum {max_delay_ticks}"
            ),
            Self::EthernetLinkBusy {
                direction,
                request_tick,
                busy_until_tick,
            } => write!(
                formatter,
                "ethernet link {direction} is busy at tick {request_tick} until tick {busy_until_tick}"
            ),
            Self::EthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes,
                ticks_per_byte,
                link_delay_ticks,
            } => write!(
                formatter,
                "ethernet link timing overflow for request tick {request_tick}, wire length {wire_length_bytes}, ticks per byte {ticks_per_byte}, link delay {link_delay_ticks}"
            ),
            Self::EthernetLinkSequenceOverflow => {
                write!(formatter, "ethernet link transmission sequence overflow")
            }
            Self::InvalidEthernetBusPortCount { port_count } => {
                write!(formatter, "ethernet bus port count {port_count} must be positive")
            }
            Self::InvalidEthernetBusRate { ticks_per_byte } => write!(
                formatter,
                "ethernet bus ticks per byte {ticks_per_byte} must be positive"
            ),
            Self::UnknownEthernetBusPort { port, port_count } => write!(
                formatter,
                "unknown ethernet bus port {} for bus with {port_count} ports",
                port.index()
            ),
            Self::EthernetBusBusy {
                sender_port,
                request_tick,
                busy_until_tick,
            } => write!(
                formatter,
                "ethernet bus sender port {} is busy at tick {request_tick} until tick {busy_until_tick}",
                sender_port.index()
            ),
            Self::EthernetBusTimingOverflow {
                request_tick,
                wire_length_bytes,
                ticks_per_byte,
            } => write!(
                formatter,
                "ethernet bus timing overflow for request tick {request_tick}, wire length {wire_length_bytes}, ticks per byte {ticks_per_byte}"
            ),
            Self::EthernetBusSequenceOverflow => {
                write!(formatter, "ethernet bus transmission sequence overflow")
            }
            Self::InvalidEthernetPcapMaxCaptureBytes { max_capture_bytes } => write!(
                formatter,
                "ethernet pcap max capture bytes {max_capture_bytes} must be positive"
            ),
            Self::InvalidEthernetPcapClock { ticks_per_second } => write!(
                formatter,
                "ethernet pcap ticks per second {ticks_per_second} must be positive"
            ),
            Self::EthernetPcapTimestampOverflow {
                tick,
                ticks_per_second,
            } => write!(
                formatter,
                "ethernet pcap timestamp overflow for tick {tick} at {ticks_per_second} ticks per second"
            ),
            Self::EthernetPcapPacketLengthOverflow { payload_bytes } => write!(
                formatter,
                "ethernet pcap packet payload length {payload_bytes} cannot fit in pcap record"
            ),
            Self::EthernetPcapSequenceOverflow => {
                write!(formatter, "ethernet pcap record sequence overflow")
            }
            Self::DuplicateEthernetInterfaceName { name } => {
                write!(formatter, "duplicate ethernet interface name {name}")
            }
            Self::EthernetInterfaceCountOverflow { interface_count } => write!(
                formatter,
                "ethernet interface count {interface_count} cannot fit in interface id"
            ),
            Self::UnknownEthernetInterface {
                interface,
                interface_count,
            } => write!(
                formatter,
                "unknown ethernet interface {} for registry with {interface_count} interfaces",
                interface.index()
            ),
            Self::EthernetInterfaceSelfBinding { interface } => write!(
                formatter,
                "ethernet interface {} cannot bind to itself",
                interface.index()
            ),
            Self::EthernetInterfacePeerAlreadyBound {
                interface,
                current_peer,
                requested_peer,
            } => write!(
                formatter,
                "ethernet interface {} is already bound to {}, not requested peer {}",
                interface.index(),
                current_peer.index(),
                requested_peer.index()
            ),
            Self::EthernetInterfacePeerMissing { interface } => write!(
                formatter,
                "ethernet interface {} has no peer",
                interface.index()
            ),
            Self::InvalidEthernetTapMaxFrameBytes { max_frame_bytes } => write!(
                formatter,
                "ethernet tap max frame bytes {max_frame_bytes} must be positive"
            ),
            Self::EthernetTapEmptyFrame => write!(formatter, "ethernet tap frame must be nonempty"),
            Self::EthernetTapFrameTooLarge {
                frame_bytes,
                max_frame_bytes,
            } => write!(
                formatter,
                "ethernet tap frame bytes {frame_bytes} exceeds maximum {max_frame_bytes}"
            ),
            Self::EthernetTapFrameLengthOverflow { frame_bytes } => write!(
                formatter,
                "ethernet tap frame bytes {frame_bytes} cannot fit in stub frame length"
            ),
            Self::DistributedEthernetHeaderTooShort {
                bytes,
                header_bytes,
            } => write!(
                formatter,
                "distributed ethernet message has {bytes} bytes but header requires {header_bytes}"
            ),
            Self::DistributedEthernetBadMagic { magic } => write!(
                formatter,
                "distributed ethernet message has bad magic bytes {magic:?}"
            ),
            Self::UnknownDistributedEthernetMessageKind { kind } => write!(
                formatter,
                "unknown distributed ethernet message kind {kind}"
            ),
            Self::UnknownDistributedEthernetRequestType { req_type } => write!(
                formatter,
                "unknown distributed ethernet request type {req_type}"
            ),
            Self::DistributedEthernetPayloadLengthMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "distributed ethernet payload length mismatch: expected {expected_bytes} bytes, got {actual_bytes}"
            ),
            Self::DistributedEthernetSequenceOverflow => {
                write!(formatter, "distributed ethernet record sequence overflow")
            }
            Self::DistributedEthernetLinkBusy {
                interface,
                request_tick,
                busy_until_tick,
            } => write!(
                formatter,
                "distributed ethernet interface {} is busy at tick {request_tick} until tick {busy_until_tick}",
                interface.index()
            ),
            Self::DistributedEthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes,
                ticks_per_byte,
                delay_variation_ticks,
            } => write!(
                formatter,
                "distributed ethernet link timing overflow for request tick {request_tick}, wire length {wire_length_bytes}, ticks per byte {ticks_per_byte}, delay variation {delay_variation_ticks}"
            ),
            Self::DistributedEthernetLinkSequenceOverflow => {
                write!(formatter, "distributed ethernet link transmission sequence overflow")
            }
            Self::InvalidDistributedEthernetReceiveWindow {
                previous_sync_tick,
                next_sync_tick,
            } => write!(
                formatter,
                "distributed ethernet receive window previous sync tick {previous_sync_tick} must be before next sync tick {next_sync_tick}"
            ),
            Self::DistributedEthernetReceiveMessageNotData { kind } => write!(
                formatter,
                "distributed ethernet receive scheduler expected data message, got {kind:?}"
            ),
            Self::DistributedEthernetReceiveTimingOverflow {
                send_tick,
                send_delay_ticks,
                link_delay_ticks,
            } => write!(
                formatter,
                "distributed ethernet receive timing overflow for send tick {send_tick}, send delay {send_delay_ticks}, link delay {link_delay_ticks}"
            ),
            Self::DistributedEthernetReceiveWindowTooSmall {
                previous_receive_tick,
                send_delay_ticks,
                receive_tick,
            } => write!(
                formatter,
                "distributed ethernet receive window is too small: previous receive tick {previous_receive_tick}, send delay {send_delay_ticks}, receive tick {receive_tick}"
            ),
            Self::DistributedEthernetReceiveMissed {
                current_tick,
                receive_tick,
            } => write!(
                formatter,
                "distributed ethernet receive tick {receive_tick} is not after current tick {current_tick}"
            ),
            Self::DistributedEthernetReceiveOutOfOrder {
                queued_ready_tick,
                receive_tick,
            } => write!(
                formatter,
                "distributed ethernet receive tick {receive_tick} is before queued ready tick {queued_ready_tick}"
            ),
            Self::DistributedEthernetSendOutsideReceiveWindow {
                send_tick,
                previous_sync_tick,
            } => write!(
                formatter,
                "distributed ethernet send tick {send_tick} is not after previous sync tick {previous_sync_tick}"
            ),
            Self::DistributedEthernetReceiveInsideSyncWindow {
                receive_tick,
                next_sync_tick,
            } => write!(
                formatter,
                "distributed ethernet receive tick {receive_tick} is not after next sync tick {next_sync_tick}"
            ),
            Self::InvalidEthernetSwitchPortCount { port_count } => write!(
                formatter,
                "ethernet switch port count {port_count} must be positive"
            ),
            Self::InvalidEthernetSwitchRate { ticks_per_byte } => write!(
                formatter,
                "ethernet switch ticks per byte {ticks_per_byte} must be positive"
            ),
            Self::EthernetSwitchTimingOverflow {
                wire_length_bytes,
                ticks_per_byte,
                switch_delay_ticks,
            } => write!(
                formatter,
                "ethernet switch timing overflow for wire length {wire_length_bytes}, ticks per byte {ticks_per_byte}, switch delay {switch_delay_ticks}"
            ),
            Self::UnknownEthernetSwitchPort { port, port_count } => write!(
                formatter,
                "unknown ethernet switch port {} for switch with {port_count} ports",
                port.index()
            ),
            Self::EthernetFrameTooShort { payload_bytes } => write!(
                formatter,
                "ethernet frame has {payload_bytes} bytes and is too short for MAC addresses"
            ),
        }
    }
}

impl Error for NetworkError {}
