use std::error::Error;
use std::fmt;

use crate::{
    DistributedEthernetMessageKind, EthernetBusPortId, EthernetInterfaceId, EthernetLinkDirection,
    EthernetPacketHandle, EthernetSwitchPortId,
};

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
            Self::ZeroFifoCapacity => {
                write!(formatter, "ethernet packet FIFO capacity must be positive")
            }
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
                write!(
                    formatter,
                    "unknown ethernet packet FIFO handle {}",
                    handle.sequence()
                )
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
            Self::PacketSequenceOverflow => {
                write!(formatter, "ethernet packet FIFO sequence overflow")
            }
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
                write!(
                    formatter,
                    "ethernet bus port count {port_count} must be positive"
                )
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
                write!(
                    formatter,
                    "distributed ethernet link transmission sequence overflow"
                )
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
