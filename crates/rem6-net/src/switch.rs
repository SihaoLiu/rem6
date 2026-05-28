use std::collections::BTreeMap;

use crate::{EthernetLinkDelayVariation, EthernetPacket, NetworkError};

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
    timing: EthernetSwitchTiming,
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
        Self::new_with_ttl_and_timing(
            port_count,
            output_buffer_bytes,
            ttl_ticks,
            EthernetSwitchTiming::default(),
        )
    }

    pub fn new_with_timing(
        port_count: u16,
        output_buffer_bytes: u64,
        timing: EthernetSwitchTiming,
    ) -> Result<Self, NetworkError> {
        Self::new_with_ttl_and_timing(port_count, output_buffer_bytes, u64::MAX, timing)
    }

    pub fn new_with_ttl_and_timing(
        port_count: u16,
        output_buffer_bytes: u64,
        ttl_ticks: u64,
        timing: EthernetSwitchTiming,
    ) -> Result<Self, NetworkError> {
        if port_count == 0 {
            return Err(NetworkError::InvalidEthernetSwitchPortCount { port_count });
        }
        let ports = (0..port_count)
            .map(|port| {
                EthernetSwitchPort::new(
                    EthernetSwitchPortId::new(port),
                    output_buffer_bytes,
                    timing.delay_variation.clone(),
                )
            })
            .collect();
        Ok(Self {
            ports,
            ttl_ticks,
            timing,
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
            let timing = self.timing.clone();
            let port = self.port_mut(*output_port)?;
            if port.enqueue(packet.clone(), ingress_port, tick, &timing)? {
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

    pub fn drain_ready_outputs(&mut self, up_to_tick: u64) -> Vec<EthernetSwitchReadyOutput> {
        let mut ready = Vec::new();
        loop {
            let Some((port_index, ready_tick)) = self.next_ready_port(up_to_tick) else {
                break;
            };
            let output = self.ports[port_index].pop_ready(ready_tick, &self.timing);
            ready.push(output);
        }
        ready
    }

    pub fn snapshot(&self) -> EthernetSwitchSnapshot {
        EthernetSwitchSnapshot {
            ports: self.ports.clone(),
            ttl_ticks: self.ttl_ticks,
            timing: self.timing.clone(),
            forwarding_table: self.forwarding_table.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &EthernetSwitchSnapshot) -> Result<(), NetworkError> {
        self.ports = snapshot.ports.clone();
        self.ttl_ticks = snapshot.ttl_ticks;
        self.timing = snapshot.timing.clone();
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

    fn next_ready_port(&self, up_to_tick: u64) -> Option<(usize, u64)> {
        self.ports
            .iter()
            .enumerate()
            .filter_map(|(index, port)| {
                let ready_tick = port.ready_tick()?;
                (ready_tick <= up_to_tick).then_some((index, ready_tick, port.id))
            })
            .min_by_key(|(_, ready_tick, port)| (*ready_tick, *port))
            .map(|(index, ready_tick, _)| (index, ready_tick))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetSwitchTiming {
    ticks_per_byte: u64,
    switch_delay_ticks: u64,
    delay_variation: Option<EthernetLinkDelayVariation>,
}

impl EthernetSwitchTiming {
    pub fn new(ticks_per_byte: u64, switch_delay_ticks: u64) -> Result<Self, NetworkError> {
        if ticks_per_byte == 0 {
            return Err(NetworkError::InvalidEthernetSwitchRate { ticks_per_byte });
        }
        Ok(Self {
            ticks_per_byte,
            switch_delay_ticks,
            delay_variation: None,
        })
    }

    pub fn with_delay_variation(mut self, delay_variation: EthernetLinkDelayVariation) -> Self {
        self.delay_variation = Some(delay_variation);
        self
    }

    pub const fn ticks_per_byte(&self) -> u64 {
        self.ticks_per_byte
    }

    pub const fn switch_delay_ticks(&self) -> u64 {
        self.switch_delay_ticks
    }

    fn delay_for_packet(
        &self,
        packet: &EthernetPacket,
        delay_variation: &mut Option<EthernetLinkDelayVariation>,
    ) -> Result<EthernetSwitchScheduledDelay, NetworkError> {
        let serialization_ticks = self
            .ticks_per_byte
            .checked_mul(packet.wire_length_bytes())
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(NetworkError::EthernetSwitchTimingOverflow {
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.ticks_per_byte,
                switch_delay_ticks: self.switch_delay_ticks,
            })?;
        let delay_variation_ticks = delay_variation
            .as_ref()
            .map(EthernetLinkDelayVariation::peek_delay_ticks)
            .unwrap_or(0);
        if let Some(delay_variation) = delay_variation {
            delay_variation.advance_delay();
        }
        let total_delay_ticks = serialization_ticks
            .checked_add(delay_variation_ticks)
            .and_then(|ticks| ticks.checked_add(self.switch_delay_ticks))
            .ok_or(NetworkError::EthernetSwitchTimingOverflow {
                wire_length_bytes: packet.wire_length_bytes(),
                ticks_per_byte: self.ticks_per_byte,
                switch_delay_ticks: self.switch_delay_ticks,
            })?;
        Ok(EthernetSwitchScheduledDelay {
            serialization_ticks,
            delay_variation_ticks,
            total_delay_ticks,
        })
    }
}

impl Default for EthernetSwitchTiming {
    fn default() -> Self {
        Self {
            ticks_per_byte: 1,
            switch_delay_ticks: 0,
            delay_variation: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EthernetSwitchScheduledDelay {
    serialization_ticks: u64,
    delay_variation_ticks: u64,
    total_delay_ticks: u64,
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
pub struct EthernetSwitchReadyOutput {
    egress_port: EthernetSwitchPortId,
    ingress_port: EthernetSwitchPortId,
    receive_tick: u64,
    ready_tick: u64,
    serialization_ticks: u64,
    delay_variation_ticks: u64,
    packet: EthernetPacket,
}

impl EthernetSwitchReadyOutput {
    pub const fn egress_port(&self) -> EthernetSwitchPortId {
        self.egress_port
    }

    pub const fn ingress_port(&self) -> EthernetSwitchPortId {
        self.ingress_port
    }

    pub const fn receive_tick(&self) -> u64 {
        self.receive_tick
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn serialization_ticks(&self) -> u64 {
        self.serialization_ticks
    }

    pub const fn delay_variation_ticks(&self) -> u64 {
        self.delay_variation_ticks
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetSwitchSnapshot {
    ports: Vec<EthernetSwitchPort>,
    ttl_ticks: u64,
    timing: EthernetSwitchTiming,
    forwarding_table: BTreeMap<EthernetMacAddress, EthernetSwitchForwardingEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetSwitchPort {
    id: EthernetSwitchPortId,
    output_buffer_bytes: u64,
    queued_bytes: u64,
    delay_variation: Option<EthernetLinkDelayVariation>,
    queue: Vec<EthernetSwitchQueuedPacket>,
}

impl EthernetSwitchPort {
    fn new(
        id: EthernetSwitchPortId,
        output_buffer_bytes: u64,
        delay_variation: Option<EthernetLinkDelayVariation>,
    ) -> Self {
        Self {
            id,
            output_buffer_bytes,
            queued_bytes: 0,
            delay_variation,
            queue: Vec::new(),
        }
    }

    fn enqueue(
        &mut self,
        packet: EthernetPacket,
        source_port: EthernetSwitchPortId,
        tick: u64,
        timing: &EthernetSwitchTiming,
    ) -> Result<bool, NetworkError> {
        let packet_bytes = packet.payload_len();
        let old_head = self
            .queue
            .first()
            .map(EthernetSwitchQueuedPacket::order_key);
        let insert_key = (tick, source_port);
        let insert_index = self
            .queue
            .partition_point(|entry| entry.order_key() <= insert_key);
        self.queued_bytes = self.queued_bytes.saturating_add(packet_bytes);
        self.queue.insert(
            insert_index,
            EthernetSwitchQueuedPacket {
                packet,
                source_port,
                receive_tick: tick,
                scheduled: None,
            },
        );
        let mut pushed_index = Some(insert_index);
        while self.queued_bytes > self.output_buffer_bytes {
            let remove_index = self.queue.len() - 1;
            let removed = self
                .queue
                .pop()
                .expect("ethernet switch queue has excess bytes");
            self.queued_bytes = self
                .queued_bytes
                .saturating_sub(removed.packet.payload_len());
            if pushed_index == Some(remove_index) {
                pushed_index = None;
            }
        }
        let new_head = self
            .queue
            .first()
            .map(EthernetSwitchQueuedPacket::order_key);
        if old_head != new_head {
            for entry in &mut self.queue {
                entry.scheduled = None;
            }
        }
        self.schedule_head_if_needed(timing, tick)?;
        Ok(pushed_index.is_some())
    }

    fn ready_tick(&self) -> Option<u64> {
        self.queue
            .first()
            .and_then(|entry| entry.scheduled.map(|scheduled| scheduled.ready_tick))
    }

    fn pop_ready(
        &mut self,
        ready_tick: u64,
        timing: &EthernetSwitchTiming,
    ) -> EthernetSwitchReadyOutput {
        let entry = self.queue.remove(0);
        self.queued_bytes = self.queued_bytes.saturating_sub(entry.packet.payload_len());
        let scheduled = entry
            .scheduled
            .expect("ready ethernet switch output is scheduled");
        self.schedule_head_if_needed(timing, ready_tick)
            .expect("validated ethernet switch queued packet timing");
        EthernetSwitchReadyOutput {
            egress_port: self.id,
            ingress_port: entry.source_port,
            receive_tick: entry.receive_tick,
            ready_tick,
            serialization_ticks: scheduled.serialization_ticks,
            delay_variation_ticks: scheduled.delay_variation_ticks,
            packet: entry.packet,
        }
    }

    fn schedule_head_if_needed(
        &mut self,
        timing: &EthernetSwitchTiming,
        base_tick: u64,
    ) -> Result<(), NetworkError> {
        let Some(entry) = self.queue.first_mut() else {
            return Ok(());
        };
        if entry.scheduled.is_some() {
            return Ok(());
        }
        let delay = timing.delay_for_packet(&entry.packet, &mut self.delay_variation)?;
        let ready_tick = base_tick.checked_add(delay.total_delay_ticks).ok_or(
            NetworkError::EthernetSwitchTimingOverflow {
                wire_length_bytes: entry.packet.wire_length_bytes(),
                ticks_per_byte: timing.ticks_per_byte,
                switch_delay_ticks: timing.switch_delay_ticks,
            },
        )?;
        entry.scheduled = Some(EthernetSwitchScheduledOutput {
            ready_tick,
            serialization_ticks: delay.serialization_ticks,
            delay_variation_ticks: delay.delay_variation_ticks,
        });
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetSwitchQueuedPacket {
    packet: EthernetPacket,
    source_port: EthernetSwitchPortId,
    receive_tick: u64,
    scheduled: Option<EthernetSwitchScheduledOutput>,
}

impl EthernetSwitchQueuedPacket {
    fn order_key(&self) -> (u64, EthernetSwitchPortId) {
        (self.receive_tick, self.source_port)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EthernetSwitchScheduledOutput {
    ready_tick: u64,
    serialization_ticks: u64,
    delay_variation_ticks: u64,
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
