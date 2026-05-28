use crate::{EthernetPacket, NetworkError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EthernetBusPortId(pub u16);

impl EthernetBusPortId {
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EthernetBusTiming {
    ticks_per_byte: u64,
}

impl EthernetBusTiming {
    pub fn new(ticks_per_byte: u64) -> Result<Self, NetworkError> {
        if ticks_per_byte == 0 {
            return Err(NetworkError::InvalidEthernetBusRate { ticks_per_byte });
        }
        Ok(Self { ticks_per_byte })
    }

    pub const fn ticks_per_byte(&self) -> u64 {
        self.ticks_per_byte
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetBus {
    port_count: u16,
    timing: EthernetBusTiming,
    loopback: bool,
    next_sequence: u64,
    pending: Option<EthernetBusPendingTransmission>,
}

impl EthernetBus {
    pub fn new(port_count: u16, timing: EthernetBusTiming) -> Result<Self, NetworkError> {
        Self::new_with_loopback(port_count, timing, false)
    }

    pub fn new_with_loopback(
        port_count: u16,
        timing: EthernetBusTiming,
        loopback: bool,
    ) -> Result<Self, NetworkError> {
        if port_count == 0 {
            return Err(NetworkError::InvalidEthernetBusPortCount { port_count });
        }
        Ok(Self {
            port_count,
            timing,
            loopback,
            next_sequence: 0,
            pending: None,
        })
    }

    pub const fn port_count(&self) -> u16 {
        self.port_count
    }

    pub const fn timing(&self) -> EthernetBusTiming {
        self.timing
    }

    pub const fn loopback(&self) -> bool {
        self.loopback
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn busy_until_tick(&self) -> Option<u64> {
        self.pending
            .as_ref()
            .map(EthernetBusPendingTransmission::completion_tick)
    }

    pub fn pending_transmission_count(&self) -> usize {
        usize::from(self.pending.is_some())
    }

    pub fn transmit(
        &mut self,
        sender_port: EthernetBusPortId,
        packet: EthernetPacket,
        request_tick: u64,
    ) -> Result<EthernetBusTransmission, NetworkError> {
        self.validate_port(sender_port)?;
        if let Some(pending) = &self.pending {
            return Err(NetworkError::EthernetBusBusy {
                sender_port,
                request_tick,
                busy_until_tick: pending.completion_tick,
            });
        }

        let completion_tick = self.completion_tick(&packet, request_tick)?;
        let sequence = self.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(NetworkError::EthernetBusSequenceOverflow)?;
        let recipient_ports = self.recipient_ports(sender_port);
        let transmission = EthernetBusTransmission {
            sequence,
            sender_port,
            request_tick,
            completion_tick,
            recipient_ports: recipient_ports.clone(),
            packet: packet.clone(),
        };

        self.next_sequence = next_sequence;
        self.pending = Some(EthernetBusPendingTransmission {
            sequence,
            sender_port,
            request_tick,
            completion_tick,
            recipient_ports,
            packet,
        });
        Ok(transmission)
    }

    pub fn drain_ready_deliveries(&mut self, up_to_tick: u64) -> Vec<EthernetBusDelivery> {
        let Some(pending) = &self.pending else {
            return Vec::new();
        };
        if pending.completion_tick > up_to_tick {
            return Vec::new();
        }
        let pending = self
            .pending
            .take()
            .expect("ready ethernet bus transmission exists");
        pending
            .recipient_ports
            .into_iter()
            .map(|recipient_port| EthernetBusDelivery {
                sequence: pending.sequence,
                sender_port: pending.sender_port,
                recipient_port,
                request_tick: pending.request_tick,
                delivery_tick: pending.completion_tick,
                packet: pending.packet.clone(),
            })
            .collect()
    }

    pub fn snapshot(&self) -> EthernetBusSnapshot {
        EthernetBusSnapshot {
            port_count: self.port_count,
            timing: self.timing,
            loopback: self.loopback,
            next_sequence: self.next_sequence,
            pending: self.pending.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &EthernetBusSnapshot) -> Result<(), NetworkError> {
        self.port_count = snapshot.port_count;
        self.timing = snapshot.timing;
        self.loopback = snapshot.loopback;
        self.next_sequence = snapshot.next_sequence;
        self.pending = snapshot.pending.clone();
        Ok(())
    }

    fn validate_port(&self, port: EthernetBusPortId) -> Result<(), NetworkError> {
        if port.index() >= self.port_count as usize {
            return Err(NetworkError::UnknownEthernetBusPort {
                port,
                port_count: self.port_count,
            });
        }
        Ok(())
    }

    fn completion_tick(
        &self,
        packet: &EthernetPacket,
        request_tick: u64,
    ) -> Result<u64, NetworkError> {
        let serialization_ticks = self
            .timing
            .ticks_per_byte
            .checked_mul(packet.wire_length_bytes())
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(NetworkError::EthernetBusTimingOverflow {
                request_tick,
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.timing.ticks_per_byte,
            })?;
        request_tick.checked_add(serialization_ticks).ok_or(
            NetworkError::EthernetBusTimingOverflow {
                request_tick,
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.timing.ticks_per_byte,
            },
        )
    }

    fn recipient_ports(&self, sender_port: EthernetBusPortId) -> Vec<EthernetBusPortId> {
        (0..self.port_count)
            .map(EthernetBusPortId::new)
            .filter(|port| self.loopback || *port != sender_port)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetBusTransmission {
    sequence: u64,
    sender_port: EthernetBusPortId,
    request_tick: u64,
    completion_tick: u64,
    recipient_ports: Vec<EthernetBusPortId>,
    packet: EthernetPacket,
}

impl EthernetBusTransmission {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn sender_port(&self) -> EthernetBusPortId {
        self.sender_port
    }

    pub const fn request_tick(&self) -> u64 {
        self.request_tick
    }

    pub const fn completion_tick(&self) -> u64 {
        self.completion_tick
    }

    pub fn recipient_ports(&self) -> &[EthernetBusPortId] {
        &self.recipient_ports
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetBusDelivery {
    sequence: u64,
    sender_port: EthernetBusPortId,
    recipient_port: EthernetBusPortId,
    request_tick: u64,
    delivery_tick: u64,
    packet: EthernetPacket,
}

impl EthernetBusDelivery {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn sender_port(&self) -> EthernetBusPortId {
        self.sender_port
    }

    pub const fn recipient_port(&self) -> EthernetBusPortId {
        self.recipient_port
    }

    pub const fn request_tick(&self) -> u64 {
        self.request_tick
    }

    pub const fn delivery_tick(&self) -> u64 {
        self.delivery_tick
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetBusSnapshot {
    port_count: u16,
    timing: EthernetBusTiming,
    loopback: bool,
    next_sequence: u64,
    pending: Option<EthernetBusPendingTransmission>,
}

impl EthernetBusSnapshot {
    pub const fn port_count(&self) -> u16 {
        self.port_count
    }

    pub const fn timing(&self) -> EthernetBusTiming {
        self.timing
    }

    pub const fn loopback(&self) -> bool {
        self.loopback
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn busy_until_tick(&self) -> Option<u64> {
        self.pending
            .as_ref()
            .map(EthernetBusPendingTransmission::completion_tick)
    }

    pub fn pending_transmission_count(&self) -> usize {
        usize::from(self.pending.is_some())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetBusPendingTransmission {
    sequence: u64,
    sender_port: EthernetBusPortId,
    request_tick: u64,
    completion_tick: u64,
    recipient_ports: Vec<EthernetBusPortId>,
    packet: EthernetPacket,
}

impl EthernetBusPendingTransmission {
    const fn completion_tick(&self) -> u64 {
        self.completion_tick
    }
}
