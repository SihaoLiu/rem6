use std::collections::VecDeque;

use crate::{
    EthernetInterfaceEvent, EthernetInterfaceId, EthernetInterfaceRegistry,
    EthernetInterfaceSendRecord, EthernetLinkDelayVariation, EthernetPacket, NetworkError,
};

const MAGIC: [u8; 4] = *b"R6DN";
const DATA_KIND: u8 = 1;
const SYNC_REQUEST_KIND: u8 = 2;
const SYNC_ACK_KIND: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedEthernetHeader;

impl DistributedEthernetHeader {
    pub const WIRE_BYTES: usize = 40;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedEthernetMessageKind {
    Data,
    SyncRequest,
    SyncAck,
}

impl DistributedEthernetMessageKind {
    const fn wire_code(self) -> u8 {
        match self {
            Self::Data => DATA_KIND,
            Self::SyncRequest => SYNC_REQUEST_KIND,
            Self::SyncAck => SYNC_ACK_KIND,
        }
    }

    fn from_wire_code(kind: u8) -> Result<Self, NetworkError> {
        match kind {
            DATA_KIND => Ok(Self::Data),
            SYNC_REQUEST_KIND => Ok(Self::SyncRequest),
            SYNC_ACK_KIND => Ok(Self::SyncAck),
            _ => Err(NetworkError::UnknownDistributedEthernetMessageKind { kind }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributedEthernetReqType {
    None,
    Immediate,
    Collective,
    Pending,
}

impl DistributedEthernetReqType {
    const fn wire_code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Immediate => 1,
            Self::Collective => 2,
            Self::Pending => 3,
        }
    }

    fn from_wire_code(req_type: u8) -> Result<Self, NetworkError> {
        match req_type {
            0 => Ok(Self::None),
            1 => Ok(Self::Immediate),
            2 => Ok(Self::Collective),
            3 => Ok(Self::Pending),
            _ => Err(NetworkError::UnknownDistributedEthernetRequestType { req_type }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetMessage {
    kind: DistributedEthernetMessageKind,
    send_tick: u64,
    send_delay_ticks: Option<u64>,
    sync_repeat_ticks: Option<u64>,
    sim_length_bytes: Option<u64>,
    packet: Option<EthernetPacket>,
    need_checkpoint: Option<DistributedEthernetReqType>,
    need_stop_sync: Option<DistributedEthernetReqType>,
    need_exit: Option<DistributedEthernetReqType>,
}

impl DistributedEthernetMessage {
    pub fn data(
        send_tick: u64,
        send_delay_ticks: u64,
        packet: EthernetPacket,
    ) -> Result<Self, NetworkError> {
        Ok(Self {
            kind: DistributedEthernetMessageKind::Data,
            send_tick,
            send_delay_ticks: Some(send_delay_ticks),
            sync_repeat_ticks: None,
            sim_length_bytes: Some(packet.wire_length_bytes()),
            packet: Some(packet),
            need_checkpoint: None,
            need_stop_sync: None,
            need_exit: None,
        })
    }

    pub const fn sync_request(
        send_tick: u64,
        sync_repeat_ticks: u64,
        need_checkpoint: DistributedEthernetReqType,
        need_stop_sync: DistributedEthernetReqType,
        need_exit: DistributedEthernetReqType,
    ) -> Self {
        Self::sync_message(
            DistributedEthernetMessageKind::SyncRequest,
            send_tick,
            sync_repeat_ticks,
            need_checkpoint,
            need_stop_sync,
            need_exit,
        )
    }

    pub const fn sync_ack(
        send_tick: u64,
        sync_repeat_ticks: u64,
        need_checkpoint: DistributedEthernetReqType,
        need_stop_sync: DistributedEthernetReqType,
        need_exit: DistributedEthernetReqType,
    ) -> Self {
        Self::sync_message(
            DistributedEthernetMessageKind::SyncAck,
            send_tick,
            sync_repeat_ticks,
            need_checkpoint,
            need_stop_sync,
            need_exit,
        )
    }

    pub const fn kind(&self) -> DistributedEthernetMessageKind {
        self.kind
    }

    pub const fn send_tick(&self) -> u64 {
        self.send_tick
    }

    pub const fn send_delay_ticks(&self) -> Option<u64> {
        self.send_delay_ticks
    }

    pub const fn sync_repeat_ticks(&self) -> Option<u64> {
        self.sync_repeat_ticks
    }

    pub const fn sim_length_bytes(&self) -> Option<u64> {
        self.sim_length_bytes
    }

    pub fn packet_length_bytes(&self) -> Option<u64> {
        self.packet.as_ref().map(EthernetPacket::payload_len)
    }

    pub const fn need_checkpoint(&self) -> Option<DistributedEthernetReqType> {
        self.need_checkpoint
    }

    pub const fn need_stop_sync(&self) -> Option<DistributedEthernetReqType> {
        self.need_stop_sync
    }

    pub const fn need_exit(&self) -> Option<DistributedEthernetReqType> {
        self.need_exit
    }

    pub const fn packet(&self) -> Option<&EthernetPacket> {
        self.packet.as_ref()
    }

    const fn sync_message(
        kind: DistributedEthernetMessageKind,
        send_tick: u64,
        sync_repeat_ticks: u64,
        need_checkpoint: DistributedEthernetReqType,
        need_stop_sync: DistributedEthernetReqType,
        need_exit: DistributedEthernetReqType,
    ) -> Self {
        Self {
            kind,
            send_tick,
            send_delay_ticks: None,
            sync_repeat_ticks: Some(sync_repeat_ticks),
            sim_length_bytes: None,
            packet: None,
            need_checkpoint: Some(need_checkpoint),
            need_stop_sync: Some(need_stop_sync),
            need_exit: Some(need_exit),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetCodec {
    next_sequence: u64,
    records: Vec<DistributedEthernetRecord>,
}

impl DistributedEthernetCodec {
    pub const fn new() -> Self {
        Self {
            next_sequence: 0,
            records: Vec::new(),
        }
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn records(&self) -> &[DistributedEthernetRecord] {
        &self.records
    }

    pub fn encode(
        &mut self,
        message: &DistributedEthernetMessage,
    ) -> Result<Vec<u8>, NetworkError> {
        let bytes = Self::encode_one(message)?;
        let record = DistributedEthernetRecord {
            sequence: self.next_sequence,
            kind: message.kind,
            send_tick: message.send_tick,
            wire_bytes: bytes.len() as u64,
        };
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkError::DistributedEthernetSequenceOverflow)?;
        self.records.push(record);
        Ok(bytes)
    }

    pub fn encode_one(message: &DistributedEthernetMessage) -> Result<Vec<u8>, NetworkError> {
        let mut bytes = Vec::with_capacity(
            DistributedEthernetHeader::WIRE_BYTES
                + message
                    .packet
                    .as_ref()
                    .map_or(0, |packet| packet.payload().len()),
        );
        bytes.extend_from_slice(&MAGIC);
        bytes.push(message.kind.wire_code());
        bytes.push(
            message
                .need_checkpoint
                .unwrap_or(DistributedEthernetReqType::None)
                .wire_code(),
        );
        bytes.push(
            message
                .need_stop_sync
                .unwrap_or(DistributedEthernetReqType::None)
                .wire_code(),
        );
        bytes.push(
            message
                .need_exit
                .unwrap_or(DistributedEthernetReqType::None)
                .wire_code(),
        );
        push_u64(&mut bytes, message.send_tick);
        push_u64(
            &mut bytes,
            message
                .send_delay_ticks
                .or(message.sync_repeat_ticks)
                .unwrap_or(0),
        );
        push_u64(&mut bytes, message.sim_length_bytes.unwrap_or(0));
        push_u64(
            &mut bytes,
            message
                .packet
                .as_ref()
                .map_or(0, EthernetPacket::payload_len),
        );
        if let Some(packet) = &message.packet {
            bytes.extend_from_slice(packet.payload());
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<DistributedEthernetMessage, NetworkError> {
        if bytes.len() < DistributedEthernetHeader::WIRE_BYTES {
            return Err(NetworkError::DistributedEthernetHeaderTooShort {
                bytes: bytes.len(),
                header_bytes: DistributedEthernetHeader::WIRE_BYTES,
            });
        }
        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if magic != MAGIC {
            return Err(NetworkError::DistributedEthernetBadMagic { magic });
        }
        let kind = DistributedEthernetMessageKind::from_wire_code(bytes[4])?;
        let need_checkpoint = DistributedEthernetReqType::from_wire_code(bytes[5])?;
        let need_stop_sync = DistributedEthernetReqType::from_wire_code(bytes[6])?;
        let need_exit = DistributedEthernetReqType::from_wire_code(bytes[7])?;
        let send_tick = read_u64(bytes, 8);
        let timing = read_u64(bytes, 16);
        let sim_length_bytes = read_u64(bytes, 24);
        let packet_length_bytes = read_u64(bytes, 32);

        match kind {
            DistributedEthernetMessageKind::Data => {
                let payload = payload_bytes(bytes, packet_length_bytes)?;
                let packet = EthernetPacket::new(payload.to_vec())?
                    .with_wire_length_bytes(sim_length_bytes)?;
                DistributedEthernetMessage::data(send_tick, timing, packet)
            }
            DistributedEthernetMessageKind::SyncRequest => {
                Ok(DistributedEthernetMessage::sync_request(
                    send_tick,
                    timing,
                    need_checkpoint,
                    need_stop_sync,
                    need_exit,
                ))
            }
            DistributedEthernetMessageKind::SyncAck => Ok(DistributedEthernetMessage::sync_ack(
                send_tick,
                timing,
                need_checkpoint,
                need_stop_sync,
                need_exit,
            )),
        }
    }

    pub fn snapshot(&self) -> DistributedEthernetCodecSnapshot {
        DistributedEthernetCodecSnapshot {
            next_sequence: self.next_sequence,
            records: self.records.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &DistributedEthernetCodecSnapshot,
    ) -> Result<(), NetworkError> {
        self.next_sequence = snapshot.next_sequence;
        self.records = snapshot.records.clone();
        Ok(())
    }
}

impl Default for DistributedEthernetCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetRecord {
    sequence: u64,
    kind: DistributedEthernetMessageKind,
    send_tick: u64,
    wire_bytes: u64,
}

impl DistributedEthernetRecord {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn kind(&self) -> DistributedEthernetMessageKind {
        self.kind
    }

    pub const fn send_tick(&self) -> u64 {
        self.send_tick
    }

    pub const fn wire_bytes(&self) -> u64 {
        self.wire_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetCodecSnapshot {
    next_sequence: u64,
    records: Vec<DistributedEthernetRecord>,
}

impl DistributedEthernetCodecSnapshot {
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

fn payload_bytes(bytes: &[u8], expected_bytes: u64) -> Result<&[u8], NetworkError> {
    let payload = &bytes[DistributedEthernetHeader::WIRE_BYTES..];
    let actual_bytes = payload.len() as u64;
    if actual_bytes != expected_bytes {
        return Err(NetworkError::DistributedEthernetPayloadLengthMismatch {
            expected_bytes,
            actual_bytes,
        });
    }
    Ok(payload)
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn read_u64(bytes: &[u8], start: usize) -> u64 {
    u64::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
        bytes[start + 4],
        bytes[start + 5],
        bytes[start + 6],
        bytes[start + 7],
    ])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedEthernetReceiveWindow {
    previous_sync_tick: u64,
    next_sync_tick: u64,
}

impl DistributedEthernetReceiveWindow {
    pub fn new(previous_sync_tick: u64, next_sync_tick: u64) -> Result<Self, NetworkError> {
        if previous_sync_tick >= next_sync_tick {
            return Err(NetworkError::InvalidDistributedEthernetReceiveWindow {
                previous_sync_tick,
                next_sync_tick,
            });
        }
        Ok(Self {
            previous_sync_tick,
            next_sync_tick,
        })
    }

    pub const fn previous_sync_tick(&self) -> u64 {
        self.previous_sync_tick
    }

    pub const fn next_sync_tick(&self) -> u64 {
        self.next_sync_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributedEthernetLinkTiming {
    ticks_per_byte: u64,
    link_delay_ticks: u64,
}

impl DistributedEthernetLinkTiming {
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
pub struct DistributedEthernetReceiveScheduler {
    link_delay_ticks: u64,
    previous_receive_tick: u64,
    pending: VecDeque<DistributedEthernetReceiveDescriptor>,
}

impl DistributedEthernetReceiveScheduler {
    pub const fn new(link_delay_ticks: u64) -> Self {
        Self {
            link_delay_ticks,
            previous_receive_tick: 0,
            pending: VecDeque::new(),
        }
    }

    pub const fn link_delay_ticks(&self) -> u64 {
        self.link_delay_ticks
    }

    pub const fn previous_receive_tick(&self) -> u64 {
        self.previous_receive_tick
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn next_receive_tick(&self) -> Option<u64> {
        self.pending
            .front()
            .map(|descriptor| descriptor.receive_tick)
    }

    pub fn push_data(
        &mut self,
        message: DistributedEthernetMessage,
        current_tick: u64,
        window: Option<DistributedEthernetReceiveWindow>,
    ) -> Result<DistributedEthernetReceiveRecord, NetworkError> {
        if message.kind() != DistributedEthernetMessageKind::Data {
            return Err(NetworkError::DistributedEthernetReceiveMessageNotData {
                kind: message.kind(),
            });
        }
        let send_tick = message.send_tick();
        let send_delay_ticks = message
            .send_delay_ticks()
            .expect("distributed ethernet data message has send delay");
        let receive_tick = self.receive_tick(send_tick, send_delay_ticks, current_tick)?;
        if let Some(window) = window {
            self.validate_receive_window(window, send_tick, receive_tick)?;
        }
        if let Some(front) = self.pending.front() {
            let queued_ready_tick = front.send_tick.checked_add(front.send_delay_ticks).ok_or(
                NetworkError::DistributedEthernetReceiveTimingOverflow {
                    send_tick: front.send_tick,
                    send_delay_ticks: front.send_delay_ticks,
                    link_delay_ticks: self.link_delay_ticks,
                },
            )?;
            if queued_ready_tick > receive_tick {
                return Err(NetworkError::DistributedEthernetReceiveOutOfOrder {
                    queued_ready_tick,
                    receive_tick,
                });
            }
        }

        let packet = message
            .packet()
            .expect("distributed ethernet data message has packet")
            .clone();
        let descriptor = DistributedEthernetReceiveDescriptor {
            send_tick,
            send_delay_ticks,
            receive_tick,
            packet,
        };
        self.pending.push_back(descriptor.clone());
        Ok(DistributedEthernetReceiveRecord {
            send_tick,
            send_delay_ticks,
            link_delay_ticks: self.link_delay_ticks,
            receive_tick,
            packet: descriptor.packet,
        })
    }

    pub fn pop_ready(
        &mut self,
        current_tick: u64,
    ) -> Result<Option<DistributedEthernetReceiveDelivery>, NetworkError> {
        let Some(front) = self.pending.front() else {
            return Ok(None);
        };
        if front.receive_tick > current_tick {
            return Ok(None);
        }
        let descriptor = self
            .pending
            .pop_front()
            .expect("distributed ethernet receive descriptor exists");
        self.previous_receive_tick = current_tick;
        Ok(Some(DistributedEthernetReceiveDelivery {
            send_tick: descriptor.send_tick,
            send_delay_ticks: descriptor.send_delay_ticks,
            receive_tick: descriptor.receive_tick,
            delivery_tick: current_tick,
            packet: descriptor.packet,
        }))
    }

    pub fn resume_after_restore(&mut self, current_tick: u64) -> Result<usize, NetworkError> {
        let count = self.pending.len();
        for (index, descriptor) in self.pending.iter_mut().enumerate() {
            descriptor.send_tick = current_tick;
            descriptor.send_delay_ticks = descriptor.packet.wire_length_bytes();
            descriptor.receive_tick = if index == 0 {
                current_tick
            } else {
                current_tick
                    .checked_add(descriptor.send_delay_ticks)
                    .and_then(|tick| tick.checked_add(self.link_delay_ticks))
                    .ok_or(NetworkError::DistributedEthernetReceiveTimingOverflow {
                        send_tick: current_tick,
                        send_delay_ticks: descriptor.send_delay_ticks,
                        link_delay_ticks: self.link_delay_ticks,
                    })?
            };
        }
        Ok(count)
    }

    pub fn snapshot(&self) -> DistributedEthernetReceiveSchedulerSnapshot {
        DistributedEthernetReceiveSchedulerSnapshot {
            link_delay_ticks: self.link_delay_ticks,
            previous_receive_tick: self.previous_receive_tick,
            pending: self.pending.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &DistributedEthernetReceiveSchedulerSnapshot,
    ) -> Result<(), NetworkError> {
        self.link_delay_ticks = snapshot.link_delay_ticks;
        self.previous_receive_tick = snapshot.previous_receive_tick;
        self.pending = snapshot.pending.clone();
        Ok(())
    }

    fn receive_tick(
        &self,
        send_tick: u64,
        send_delay_ticks: u64,
        current_tick: u64,
    ) -> Result<u64, NetworkError> {
        let receive_tick = send_tick
            .checked_add(send_delay_ticks)
            .and_then(|tick| tick.checked_add(self.link_delay_ticks))
            .ok_or(NetworkError::DistributedEthernetReceiveTimingOverflow {
                send_tick,
                send_delay_ticks,
                link_delay_ticks: self.link_delay_ticks,
            })?;
        let minimum_receive_tick = self
            .previous_receive_tick
            .checked_add(send_delay_ticks)
            .ok_or(NetworkError::DistributedEthernetReceiveTimingOverflow {
                send_tick,
                send_delay_ticks,
                link_delay_ticks: self.link_delay_ticks,
            })?;
        if minimum_receive_tick > receive_tick {
            return Err(NetworkError::DistributedEthernetReceiveWindowTooSmall {
                previous_receive_tick: self.previous_receive_tick,
                send_delay_ticks,
                receive_tick,
            });
        }
        if receive_tick <= current_tick {
            return Err(NetworkError::DistributedEthernetReceiveMissed {
                current_tick,
                receive_tick,
            });
        }
        Ok(receive_tick)
    }

    fn validate_receive_window(
        &self,
        window: DistributedEthernetReceiveWindow,
        send_tick: u64,
        receive_tick: u64,
    ) -> Result<(), NetworkError> {
        if send_tick <= window.previous_sync_tick {
            return Err(NetworkError::DistributedEthernetSendOutsideReceiveWindow {
                send_tick,
                previous_sync_tick: window.previous_sync_tick,
            });
        }
        if receive_tick <= window.next_sync_tick {
            return Err(NetworkError::DistributedEthernetReceiveInsideSyncWindow {
                receive_tick,
                next_sync_tick: window.next_sync_tick,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetReceiveRecord {
    send_tick: u64,
    send_delay_ticks: u64,
    link_delay_ticks: u64,
    receive_tick: u64,
    packet: EthernetPacket,
}

impl DistributedEthernetReceiveRecord {
    pub const fn send_tick(&self) -> u64 {
        self.send_tick
    }

    pub const fn send_delay_ticks(&self) -> u64 {
        self.send_delay_ticks
    }

    pub const fn link_delay_ticks(&self) -> u64 {
        self.link_delay_ticks
    }

    pub const fn receive_tick(&self) -> u64 {
        self.receive_tick
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetReceiveDelivery {
    send_tick: u64,
    send_delay_ticks: u64,
    receive_tick: u64,
    delivery_tick: u64,
    packet: EthernetPacket,
}

impl DistributedEthernetReceiveDelivery {
    pub const fn send_tick(&self) -> u64 {
        self.send_tick
    }

    pub const fn send_delay_ticks(&self) -> u64 {
        self.send_delay_ticks
    }

    pub const fn receive_tick(&self) -> u64 {
        self.receive_tick
    }

    pub const fn delivery_tick(&self) -> u64 {
        self.delivery_tick
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetReceiveSchedulerSnapshot {
    link_delay_ticks: u64,
    previous_receive_tick: u64,
    pending: VecDeque<DistributedEthernetReceiveDescriptor>,
}

impl DistributedEthernetReceiveSchedulerSnapshot {
    pub const fn link_delay_ticks(&self) -> u64 {
        self.link_delay_ticks
    }

    pub const fn previous_receive_tick(&self) -> u64 {
        self.previous_receive_tick
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetLinkEndpoint {
    interface: EthernetInterfaceId,
    timing: DistributedEthernetLinkTiming,
    delay_variation: Option<EthernetLinkDelayVariation>,
    codec: DistributedEthernetCodec,
    receive_scheduler: DistributedEthernetReceiveScheduler,
    next_sequence: u64,
    pending_transmit: Option<DistributedEthernetLinkTransmission>,
}

impl DistributedEthernetLinkEndpoint {
    pub fn new(interface: EthernetInterfaceId, timing: DistributedEthernetLinkTiming) -> Self {
        Self {
            interface,
            timing,
            delay_variation: None,
            codec: DistributedEthernetCodec::new(),
            receive_scheduler: DistributedEthernetReceiveScheduler::new(timing.link_delay_ticks),
            next_sequence: 0,
            pending_transmit: None,
        }
    }

    pub fn new_with_delay_variation(
        interface: EthernetInterfaceId,
        timing: DistributedEthernetLinkTiming,
        delay_variation: EthernetLinkDelayVariation,
    ) -> Self {
        Self {
            interface,
            timing,
            delay_variation: Some(delay_variation),
            codec: DistributedEthernetCodec::new(),
            receive_scheduler: DistributedEthernetReceiveScheduler::new(timing.link_delay_ticks),
            next_sequence: 0,
            pending_transmit: None,
        }
    }

    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn timing(&self) -> DistributedEthernetLinkTiming {
        self.timing
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn codec_record_count(&self) -> usize {
        self.codec.record_count()
    }

    pub fn pending_transmit_count(&self) -> usize {
        usize::from(self.pending_transmit.is_some())
    }

    pub fn pending_receive_count(&self) -> usize {
        self.receive_scheduler.pending_count()
    }

    pub fn busy_until_tick(&self) -> Option<u64> {
        self.pending_transmit
            .as_ref()
            .map(DistributedEthernetLinkTransmission::transmit_done_tick)
    }

    pub fn next_receive_tick(&self) -> Option<u64> {
        self.receive_scheduler.next_receive_tick()
    }

    pub fn transmit(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        packet: EthernetPacket,
        request_tick: u64,
    ) -> Result<DistributedEthernetLinkTransmission, NetworkError> {
        self.drain_transmit_done(registry, request_tick)?;
        if let Some(busy_until_tick) = self.busy_until_tick() {
            return Err(NetworkError::DistributedEthernetLinkBusy {
                interface: self.interface,
                request_tick,
                busy_until_tick,
            });
        }
        if !registry.is_connected(self.interface)? {
            return Err(NetworkError::EthernetInterfacePeerMissing {
                interface: self.interface,
            });
        }

        let wire_length_bytes = packet.wire_length_bytes();
        let delay_variation_ticks = self.peek_delay_variation_ticks();
        let serialization_ticks = self
            .timing
            .ticks_per_byte
            .checked_mul(wire_length_bytes)
            .and_then(|ticks| ticks.checked_add(1))
            .ok_or(NetworkError::DistributedEthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes,
                ticks_per_byte: self.timing.ticks_per_byte,
                delay_variation_ticks,
            })?;
        let send_delay_ticks = serialization_ticks
            .checked_add(delay_variation_ticks)
            .ok_or(NetworkError::DistributedEthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes,
                ticks_per_byte: self.timing.ticks_per_byte,
                delay_variation_ticks,
            })?;
        let transmit_done_tick = request_tick.checked_add(send_delay_ticks).ok_or(
            NetworkError::DistributedEthernetLinkTimingOverflow {
                request_tick,
                wire_length_bytes,
                ticks_per_byte: self.timing.ticks_per_byte,
                delay_variation_ticks,
            },
        )?;
        let sequence = self.next_sequence;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkError::DistributedEthernetLinkSequenceOverflow)?;

        let message = DistributedEthernetMessage::data(request_tick, send_delay_ticks, packet)?;
        let encoded_message = self.codec.encode(&message)?;
        let transmission = DistributedEthernetLinkTransmission {
            sequence,
            interface: self.interface,
            request_tick,
            send_delay_ticks,
            delay_variation_ticks,
            transmit_done_tick,
            message,
            encoded_message,
        };

        self.advance_delay_variation();
        self.next_sequence = next_sequence;
        registry.set_busy(self.interface, true)?;
        self.pending_transmit = Some(transmission.clone());
        Ok(transmission)
    }

    pub fn drain_transmit_done(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        up_to_tick: u64,
    ) -> Result<Option<DistributedEthernetLinkSendDone>, NetworkError> {
        let Some(pending) = self.pending_transmit.as_ref() else {
            return Ok(None);
        };
        if pending.transmit_done_tick > up_to_tick {
            return Ok(None);
        }

        let event = registry.recv_done(self.interface, pending.transmit_done_tick)?;
        registry.set_busy(self.interface, false)?;
        let transmission = self
            .pending_transmit
            .take()
            .expect("distributed ethernet pending transmission exists");
        Ok(Some(DistributedEthernetLinkSendDone {
            transmission,
            event,
        }))
    }

    pub fn accept_remote_message(
        &mut self,
        message: DistributedEthernetMessage,
        current_tick: u64,
        window: Option<DistributedEthernetReceiveWindow>,
    ) -> Result<DistributedEthernetReceiveRecord, NetworkError> {
        self.receive_scheduler
            .push_data(message, current_tick, window)
    }

    pub fn drain_ready_receives(
        &mut self,
        registry: &mut EthernetInterfaceRegistry,
        up_to_tick: u64,
    ) -> Result<Vec<DistributedEthernetLinkReceiveDelivery>, NetworkError> {
        let mut deliveries = Vec::new();
        while let Some(delivery) = self.receive_scheduler.pop_ready(up_to_tick)? {
            let send_record =
                registry.send_packet(self.interface, delivery.packet().clone(), up_to_tick)?;
            deliveries.push(DistributedEthernetLinkReceiveDelivery {
                delivery,
                send_record,
            });
        }
        Ok(deliveries)
    }

    pub fn snapshot(&self) -> DistributedEthernetLinkEndpointSnapshot {
        DistributedEthernetLinkEndpointSnapshot {
            interface: self.interface,
            timing: self.timing,
            delay_variation: self.delay_variation.clone(),
            codec: self.codec.snapshot(),
            receive_scheduler: self.receive_scheduler.snapshot(),
            next_sequence: self.next_sequence,
            pending_transmit: self.pending_transmit.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &DistributedEthernetLinkEndpointSnapshot,
    ) -> Result<(), NetworkError> {
        self.interface = snapshot.interface;
        self.timing = snapshot.timing;
        self.delay_variation = snapshot.delay_variation.clone();
        self.codec.restore(&snapshot.codec)?;
        self.receive_scheduler
            .restore(&snapshot.receive_scheduler)?;
        self.next_sequence = snapshot.next_sequence;
        self.pending_transmit = snapshot.pending_transmit.clone();
        Ok(())
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetLinkTransmission {
    sequence: u64,
    interface: EthernetInterfaceId,
    request_tick: u64,
    send_delay_ticks: u64,
    delay_variation_ticks: u64,
    transmit_done_tick: u64,
    message: DistributedEthernetMessage,
    encoded_message: Vec<u8>,
}

impl DistributedEthernetLinkTransmission {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn request_tick(&self) -> u64 {
        self.request_tick
    }

    pub const fn send_delay_ticks(&self) -> u64 {
        self.send_delay_ticks
    }

    pub const fn delay_variation_ticks(&self) -> u64 {
        self.delay_variation_ticks
    }

    pub const fn transmit_done_tick(&self) -> u64 {
        self.transmit_done_tick
    }

    pub const fn message(&self) -> &DistributedEthernetMessage {
        &self.message
    }

    pub fn encoded_message(&self) -> &[u8] {
        &self.encoded_message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetLinkSendDone {
    transmission: DistributedEthernetLinkTransmission,
    event: EthernetInterfaceEvent,
}

impl DistributedEthernetLinkSendDone {
    pub const fn transmission(&self) -> &DistributedEthernetLinkTransmission {
        &self.transmission
    }

    pub const fn event(&self) -> &EthernetInterfaceEvent {
        &self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetLinkReceiveDelivery {
    delivery: DistributedEthernetReceiveDelivery,
    send_record: EthernetInterfaceSendRecord,
}

impl DistributedEthernetLinkReceiveDelivery {
    pub const fn delivery(&self) -> &DistributedEthernetReceiveDelivery {
        &self.delivery
    }

    pub const fn send_record(&self) -> &EthernetInterfaceSendRecord {
        &self.send_record
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributedEthernetLinkEndpointSnapshot {
    interface: EthernetInterfaceId,
    timing: DistributedEthernetLinkTiming,
    delay_variation: Option<EthernetLinkDelayVariation>,
    codec: DistributedEthernetCodecSnapshot,
    receive_scheduler: DistributedEthernetReceiveSchedulerSnapshot,
    next_sequence: u64,
    pending_transmit: Option<DistributedEthernetLinkTransmission>,
}

impl DistributedEthernetLinkEndpointSnapshot {
    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn timing(&self) -> DistributedEthernetLinkTiming {
        self.timing
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn pending_transmit_count(&self) -> usize {
        usize::from(self.pending_transmit.is_some())
    }

    pub fn codec_record_count(&self) -> usize {
        self.codec.record_count()
    }

    pub fn pending_receive_count(&self) -> usize {
        self.receive_scheduler.pending_count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DistributedEthernetReceiveDescriptor {
    send_tick: u64,
    send_delay_ticks: u64,
    receive_tick: u64,
    packet: EthernetPacket,
}
