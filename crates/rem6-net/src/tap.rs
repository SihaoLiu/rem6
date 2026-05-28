use std::collections::VecDeque;

use crate::{EthernetInterfaceId, EthernetInterfaceRegistry, EthernetPacket, NetworkError};

const STUB_FRAME_PREFIX_BYTES: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EthernetTapRecordKind {
    RealToSim,
    SimToReal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetTapGateway {
    interface: EthernetInterfaceId,
    max_frame_bytes: u32,
    real_input: Vec<u8>,
    queued_real_frames: VecDeque<EthernetTapQueuedFrame>,
    next_sequence: u64,
    records: Vec<EthernetTapRecord>,
}

impl EthernetTapGateway {
    pub fn new(interface: EthernetInterfaceId, max_frame_bytes: u32) -> Result<Self, NetworkError> {
        if max_frame_bytes == 0 {
            return Err(NetworkError::InvalidEthernetTapMaxFrameBytes { max_frame_bytes });
        }
        Ok(Self {
            interface,
            max_frame_bytes,
            real_input: Vec::new(),
            queued_real_frames: VecDeque::new(),
            next_sequence: 0,
            records: Vec::new(),
        })
    }

    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn max_frame_bytes(&self) -> u32 {
        self.max_frame_bytes
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn real_buffered_bytes(&self) -> usize {
        self.real_input.len()
    }

    pub fn queued_real_frame_count(&self) -> usize {
        self.queued_real_frames.len()
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn records(&self) -> &[EthernetTapRecord] {
        &self.records
    }

    pub fn ingest_real_stub_bytes(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        bytes: &[u8],
        tick: u64,
    ) -> Result<Vec<EthernetTapRecord>, NetworkError> {
        let (frames, remaining_input) = self.parse_real_stub_bytes(bytes)?;
        self.real_input = remaining_input;

        let mut records = Vec::new();
        for frame in frames {
            let packet = EthernetPacket::new(frame)?;
            if let Some(record) = self.try_send_real_frame(registry, packet, tick)? {
                records.push(record);
            }
        }
        Ok(records)
    }

    pub fn retransmit(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        tick: u64,
    ) -> Result<Option<EthernetTapRecord>, NetworkError> {
        if self.queued_real_frames.is_empty() || registry.ask_busy(self.interface)? {
            return Ok(None);
        }
        let queued = self
            .queued_real_frames
            .pop_front()
            .expect("ethernet tap queued frame exists");
        let send = registry.send_packet(self.interface, queued.packet.clone(), tick)?;
        let record = self.record(
            EthernetTapRecordKind::RealToSim,
            send.peer(),
            tick,
            queued.packet,
            queued.real_frame_bytes,
            false,
        )?;
        Ok(Some(record))
    }

    pub fn receive_simulated(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        packet: EthernetPacket,
        tick: u64,
    ) -> Result<EthernetTapRecord, NetworkError> {
        let real_frame_bytes = stub_frame_bytes(&packet, self.max_frame_bytes)?;
        let event = registry.recv_done(self.interface, tick)?;
        self.record(
            EthernetTapRecordKind::SimToReal,
            Some(event.interface()),
            tick,
            packet,
            real_frame_bytes,
            false,
        )
    }

    pub fn snapshot(&self) -> EthernetTapGatewaySnapshot {
        EthernetTapGatewaySnapshot {
            interface: self.interface,
            max_frame_bytes: self.max_frame_bytes,
            real_input: self.real_input.clone(),
            queued_real_frames: self.queued_real_frames.clone(),
            next_sequence: self.next_sequence,
            records: self.records.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &EthernetTapGatewaySnapshot) -> Result<(), NetworkError> {
        self.interface = snapshot.interface;
        self.max_frame_bytes = snapshot.max_frame_bytes;
        self.real_input = snapshot.real_input.clone();
        self.queued_real_frames = snapshot.queued_real_frames.clone();
        self.next_sequence = snapshot.next_sequence;
        self.records = snapshot.records.clone();
        Ok(())
    }

    fn parse_real_stub_bytes(&self, bytes: &[u8]) -> Result<(Vec<Vec<u8>>, Vec<u8>), NetworkError> {
        let mut input = self.real_input.clone();
        input.extend_from_slice(bytes);

        let mut cursor = 0usize;
        let mut frames = Vec::new();
        while input.len().saturating_sub(cursor) >= STUB_FRAME_PREFIX_BYTES {
            let frame_bytes = u32::from_be_bytes([
                input[cursor],
                input[cursor + 1],
                input[cursor + 2],
                input[cursor + 3],
            ]);
            if frame_bytes == 0 {
                return Err(NetworkError::EthernetTapEmptyFrame);
            }
            if frame_bytes > self.max_frame_bytes {
                return Err(NetworkError::EthernetTapFrameTooLarge {
                    frame_bytes,
                    max_frame_bytes: self.max_frame_bytes,
                });
            }

            let frame_len = frame_bytes as usize;
            let frame_start = cursor + STUB_FRAME_PREFIX_BYTES;
            let frame_end = frame_start + frame_len;
            if input.len() < frame_end {
                break;
            }

            frames.push(input[frame_start..frame_end].to_vec());
            cursor = frame_end;
        }
        Ok((frames, input[cursor..].to_vec()))
    }

    fn try_send_real_frame(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        packet: EthernetPacket,
        tick: u64,
    ) -> Result<Option<EthernetTapRecord>, NetworkError> {
        let real_frame_bytes = stub_frame_bytes(&packet, self.max_frame_bytes)?;
        if !self.queued_real_frames.is_empty() || registry.ask_busy(self.interface)? {
            self.queued_real_frames.push_back(EthernetTapQueuedFrame {
                packet,
                real_frame_bytes,
            });
            return Ok(None);
        }

        let send = registry.send_packet(self.interface, packet.clone(), tick)?;
        let record = self.record(
            EthernetTapRecordKind::RealToSim,
            send.peer(),
            tick,
            packet,
            real_frame_bytes,
            false,
        )?;
        Ok(Some(record))
    }

    fn record(
        &mut self,
        kind: EthernetTapRecordKind,
        peer: Option<EthernetInterfaceId>,
        tick: u64,
        packet: EthernetPacket,
        real_frame_bytes: Vec<u8>,
        queued: bool,
    ) -> Result<EthernetTapRecord, NetworkError> {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkError::PacketSequenceOverflow)?;
        let record = EthernetTapRecord {
            sequence,
            kind,
            interface: self.interface,
            peer,
            tick,
            packet,
            real_frame_bytes,
            queued,
        };
        self.records.push(record.clone());
        Ok(record)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetTapRecord {
    sequence: u64,
    kind: EthernetTapRecordKind,
    interface: EthernetInterfaceId,
    peer: Option<EthernetInterfaceId>,
    tick: u64,
    packet: EthernetPacket,
    real_frame_bytes: Vec<u8>,
    queued: bool,
}

impl EthernetTapRecord {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn kind(&self) -> EthernetTapRecordKind {
        self.kind
    }

    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn peer(&self) -> Option<EthernetInterfaceId> {
        self.peer
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }

    pub fn real_frame_bytes(&self) -> &[u8] {
        &self.real_frame_bytes
    }

    pub const fn queued(&self) -> bool {
        self.queued
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetTapGatewaySnapshot {
    interface: EthernetInterfaceId,
    max_frame_bytes: u32,
    real_input: Vec<u8>,
    queued_real_frames: VecDeque<EthernetTapQueuedFrame>,
    next_sequence: u64,
    records: Vec<EthernetTapRecord>,
}

impl EthernetTapGatewaySnapshot {
    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn max_frame_bytes(&self) -> u32 {
        self.max_frame_bytes
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn real_buffered_bytes(&self) -> usize {
        self.real_input.len()
    }

    pub fn queued_real_frame_count(&self) -> usize {
        self.queued_real_frames.len()
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetTapQueuedFrame {
    packet: EthernetPacket,
    real_frame_bytes: Vec<u8>,
}

fn stub_frame_bytes(
    packet: &EthernetPacket,
    max_frame_bytes: u32,
) -> Result<Vec<u8>, NetworkError> {
    let frame_bytes = u32::try_from(packet.payload_len()).map_err(|_| {
        NetworkError::EthernetTapFrameLengthOverflow {
            frame_bytes: packet.payload_len(),
        }
    })?;
    if frame_bytes > max_frame_bytes {
        return Err(NetworkError::EthernetTapFrameTooLarge {
            frame_bytes,
            max_frame_bytes,
        });
    }
    let mut bytes = frame_bytes.to_be_bytes().to_vec();
    bytes.extend_from_slice(packet.payload());
    Ok(bytes)
}
