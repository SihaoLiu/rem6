use std::collections::BTreeMap;

use crate::{EthernetPacket, NetworkError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EthernetSwitchPortId(pub u16);

impl EthernetSwitchPortId {
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EthernetMacAddress([u8; 6]);

impl EthernetMacAddress {
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 6] {
        self.0
    }

    pub const fn is_broadcast(self) -> bool {
        self.0[0] == 0xff
            && self.0[1] == 0xff
            && self.0[2] == 0xff
            && self.0[3] == 0xff
            && self.0[4] == 0xff
            && self.0[5] == 0xff
    }

    pub const fn is_multicast(self) -> bool {
        self.0[0] & 1 != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetSwitch {
    ports: Vec<EthernetSwitchPort>,
    ttl_ticks: u64,
    forwarding_table: BTreeMap<EthernetMacAddress, EthernetSwitchForwardingEntry>,
}

impl EthernetSwitch {
    pub fn new(port_count: u16, output_buffer_bytes: u64) -> Result<Self, NetworkError> {
        Self::new_with_ttl(port_count, output_buffer_bytes, u64::MAX)
    }

    pub fn new_with_ttl(
        port_count: u16,
        output_buffer_bytes: u64,
        ttl_ticks: u64,
    ) -> Result<Self, NetworkError> {
        if port_count == 0 {
            return Err(NetworkError::InvalidEthernetSwitchPortCount { port_count });
        }
        let ports = (0..port_count)
            .map(|port| {
                EthernetSwitchPort::new(EthernetSwitchPortId::new(port), output_buffer_bytes)
            })
            .collect();
        Ok(Self {
            ports,
            ttl_ticks,
            forwarding_table: BTreeMap::new(),
        })
    }

    pub fn receive(
        &mut self,
        ingress_port: EthernetSwitchPortId,
        packet: EthernetPacket,
        tick: u64,
    ) -> Result<EthernetSwitchDecision, NetworkError> {
        self.validate_port(ingress_port)?;
        let (destination_mac, source_mac) = parse_frame_addresses(&packet)?;
        self.learn(source_mac, ingress_port, tick);

        let destination_port = if destination_mac.is_broadcast() || destination_mac.is_multicast() {
            None
        } else {
            self.lookup_port(destination_mac, tick)?
        };
        let output_ports = destination_port
            .map(|port| vec![port])
            .unwrap_or_else(|| self.flood_ports(ingress_port));

        let mut enqueued_ports = Vec::new();
        let mut dropped_ports = Vec::new();
        for output_port in &output_ports {
            let port = self.port_mut(*output_port)?;
            if port.enqueue(packet.clone(), ingress_port, tick) {
                enqueued_ports.push(*output_port);
            } else {
                dropped_ports.push(*output_port);
            }
        }

        Ok(EthernetSwitchDecision {
            ingress_port,
            source_mac,
            destination_mac,
            flooded: destination_port.is_none(),
            output_ports: enqueued_ports,
            dropped_ports,
        })
    }

    pub fn lookup_port(
        &mut self,
        mac: EthernetMacAddress,
        tick: u64,
    ) -> Result<Option<EthernetSwitchPortId>, NetworkError> {
        let Some(entry) = self.forwarding_table.get(&mac).copied() else {
            return Ok(None);
        };
        if tick.saturating_sub(entry.last_use_tick) > self.ttl_ticks {
            self.forwarding_table.remove(&mac);
            return Ok(None);
        }
        self.validate_port(entry.port)?;
        Ok(Some(entry.port))
    }

    pub fn forwarding_table_len(&self) -> usize {
        self.forwarding_table.len()
    }

    pub fn port_queue_len(&self, port: EthernetSwitchPortId) -> Result<usize, NetworkError> {
        Ok(self.port(port)?.queue.len())
    }

    pub fn snapshot(&self) -> EthernetSwitchSnapshot {
        EthernetSwitchSnapshot {
            ports: self.ports.clone(),
            ttl_ticks: self.ttl_ticks,
            forwarding_table: self.forwarding_table.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &EthernetSwitchSnapshot) -> Result<(), NetworkError> {
        self.ports = snapshot.ports.clone();
        self.ttl_ticks = snapshot.ttl_ticks;
        self.forwarding_table = snapshot.forwarding_table.clone();
        Ok(())
    }

    fn learn(&mut self, mac: EthernetMacAddress, port: EthernetSwitchPortId, tick: u64) {
        self.forwarding_table.insert(
            mac,
            EthernetSwitchForwardingEntry {
                port,
                last_use_tick: tick,
            },
        );
    }

    fn flood_ports(&self, ingress_port: EthernetSwitchPortId) -> Vec<EthernetSwitchPortId> {
        self.ports
            .iter()
            .map(|port| port.id)
            .filter(|port| *port != ingress_port)
            .collect()
    }

    fn validate_port(&self, port: EthernetSwitchPortId) -> Result<(), NetworkError> {
        if port.index() >= self.ports.len() {
            return Err(NetworkError::UnknownEthernetSwitchPort {
                port,
                port_count: self.ports.len(),
            });
        }
        Ok(())
    }

    fn port(&self, port: EthernetSwitchPortId) -> Result<&EthernetSwitchPort, NetworkError> {
        self.validate_port(port)?;
        Ok(&self.ports[port.index()])
    }

    fn port_mut(
        &mut self,
        port: EthernetSwitchPortId,
    ) -> Result<&mut EthernetSwitchPort, NetworkError> {
        self.validate_port(port)?;
        Ok(&mut self.ports[port.index()])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetSwitchDecision {
    ingress_port: EthernetSwitchPortId,
    source_mac: EthernetMacAddress,
    destination_mac: EthernetMacAddress,
    flooded: bool,
    output_ports: Vec<EthernetSwitchPortId>,
    dropped_ports: Vec<EthernetSwitchPortId>,
}

impl EthernetSwitchDecision {
    pub const fn ingress_port(&self) -> EthernetSwitchPortId {
        self.ingress_port
    }

    pub const fn source_mac(&self) -> EthernetMacAddress {
        self.source_mac
    }

    pub const fn destination_mac(&self) -> EthernetMacAddress {
        self.destination_mac
    }

    pub const fn flooded(&self) -> bool {
        self.flooded
    }

    pub fn output_ports(&self) -> &[EthernetSwitchPortId] {
        &self.output_ports
    }

    pub fn dropped_ports(&self) -> &[EthernetSwitchPortId] {
        &self.dropped_ports
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetSwitchSnapshot {
    ports: Vec<EthernetSwitchPort>,
    ttl_ticks: u64,
    forwarding_table: BTreeMap<EthernetMacAddress, EthernetSwitchForwardingEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetSwitchPort {
    id: EthernetSwitchPortId,
    output_buffer_bytes: u64,
    queued_bytes: u64,
    queue: Vec<EthernetSwitchQueuedPacket>,
}

impl EthernetSwitchPort {
    fn new(id: EthernetSwitchPortId, output_buffer_bytes: u64) -> Self {
        Self {
            id,
            output_buffer_bytes,
            queued_bytes: 0,
            queue: Vec::new(),
        }
    }

    fn enqueue(
        &mut self,
        packet: EthernetPacket,
        source_port: EthernetSwitchPortId,
        tick: u64,
    ) -> bool {
        let packet_bytes = packet.payload_len();
        if self.queued_bytes.saturating_add(packet_bytes) > self.output_buffer_bytes {
            return false;
        }
        self.queued_bytes = self.queued_bytes.saturating_add(packet_bytes);
        self.queue.push(EthernetSwitchQueuedPacket {
            packet,
            source_port,
            receive_tick: tick,
        });
        self.queue
            .sort_by_key(|entry| (entry.receive_tick, entry.source_port));
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetSwitchQueuedPacket {
    packet: EthernetPacket,
    source_port: EthernetSwitchPortId,
    receive_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EthernetSwitchForwardingEntry {
    port: EthernetSwitchPortId,
    last_use_tick: u64,
}

fn parse_frame_addresses(
    packet: &EthernetPacket,
) -> Result<(EthernetMacAddress, EthernetMacAddress), NetworkError> {
    let payload = packet.payload();
    if payload.len() < 12 {
        return Err(NetworkError::EthernetFrameTooShort {
            payload_bytes: payload.len() as u64,
        });
    }
    let destination = [
        payload[0], payload[1], payload[2], payload[3], payload[4], payload[5],
    ];
    let source = [
        payload[6],
        payload[7],
        payload[8],
        payload[9],
        payload[10],
        payload[11],
    ];
    Ok((
        EthernetMacAddress::new(destination),
        EthernetMacAddress::new(source),
    ))
}
