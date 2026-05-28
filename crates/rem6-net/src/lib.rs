use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

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
        }
    }
}

impl Error for NetworkError {}
