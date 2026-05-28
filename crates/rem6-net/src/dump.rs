use crate::{EthernetPacket, NetworkError};

const PCAP_MAGIC_MICROSECOND_LE: u32 = 0xa1b2c3d4;
const PCAP_VERSION_MAJOR: u16 = 2;
const PCAP_VERSION_MINOR: u16 = 4;
const PCAP_THISZONE_UTC: i32 = 0;
const PCAP_SIGFIGS_UNKNOWN: u32 = 0;
const PCAP_LINKTYPE_ETHERNET: u32 = 1;
const MICROSECONDS_PER_SECOND: u128 = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetPcapDump {
    max_capture_bytes: u32,
    ticks_per_second: u64,
    next_sequence: u64,
    records: Vec<EthernetPcapRecord>,
}

impl EthernetPcapDump {
    pub fn new(max_capture_bytes: u32, ticks_per_second: u64) -> Result<Self, NetworkError> {
        if max_capture_bytes == 0 {
            return Err(NetworkError::InvalidEthernetPcapMaxCaptureBytes { max_capture_bytes });
        }
        if ticks_per_second == 0 {
            return Err(NetworkError::InvalidEthernetPcapClock { ticks_per_second });
        }
        Ok(Self {
            max_capture_bytes,
            ticks_per_second,
            next_sequence: 0,
            records: Vec::new(),
        })
    }

    pub const fn max_capture_bytes(&self) -> u32 {
        self.max_capture_bytes
    }

    pub const fn ticks_per_second(&self) -> u64 {
        self.ticks_per_second
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn records(&self) -> &[EthernetPcapRecord] {
        &self.records
    }

    pub fn capture(
        &mut self,
        packet: &EthernetPacket,
        tick: u64,
    ) -> Result<EthernetPcapRecord, NetworkError> {
        let (timestamp_seconds, timestamp_microseconds) = self.timestamp_for_tick(tick)?;
        let original_len = u32::try_from(packet.payload_len()).map_err(|_| {
            NetworkError::EthernetPcapPacketLengthOverflow {
                payload_bytes: packet.payload_len(),
            }
        })?;
        let captured_len = original_len.min(self.max_capture_bytes);
        let captured_payload = packet.payload()[..captured_len as usize].to_vec();
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(NetworkError::EthernetPcapSequenceOverflow)?;

        let record = EthernetPcapRecord {
            sequence,
            tick,
            timestamp_seconds,
            timestamp_microseconds,
            captured_len,
            original_len,
            captured_payload,
        };
        self.records.push(record.clone());
        Ok(record)
    }

    pub fn pcap_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.pcap_len_hint());
        push_u32(&mut bytes, PCAP_MAGIC_MICROSECOND_LE);
        push_u16(&mut bytes, PCAP_VERSION_MAJOR);
        push_u16(&mut bytes, PCAP_VERSION_MINOR);
        push_i32(&mut bytes, PCAP_THISZONE_UTC);
        push_u32(&mut bytes, PCAP_SIGFIGS_UNKNOWN);
        push_u32(&mut bytes, self.max_capture_bytes);
        push_u32(&mut bytes, PCAP_LINKTYPE_ETHERNET);

        for record in &self.records {
            push_u32(&mut bytes, record.timestamp_seconds);
            push_u32(&mut bytes, record.timestamp_microseconds);
            push_u32(&mut bytes, record.captured_len);
            push_u32(&mut bytes, record.original_len);
            bytes.extend_from_slice(&record.captured_payload);
        }
        bytes
    }

    pub fn snapshot(&self) -> EthernetPcapDumpSnapshot {
        EthernetPcapDumpSnapshot {
            max_capture_bytes: self.max_capture_bytes,
            ticks_per_second: self.ticks_per_second,
            next_sequence: self.next_sequence,
            records: self.records.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &EthernetPcapDumpSnapshot) -> Result<(), NetworkError> {
        self.max_capture_bytes = snapshot.max_capture_bytes;
        self.ticks_per_second = snapshot.ticks_per_second;
        self.next_sequence = snapshot.next_sequence;
        self.records = snapshot.records.clone();
        Ok(())
    }

    fn timestamp_for_tick(&self, tick: u64) -> Result<(u32, u32), NetworkError> {
        let ticks_per_second = self.ticks_per_second as u128;
        let seconds = (tick as u128) / ticks_per_second;
        let tick_remainder = (tick as u128) % ticks_per_second;
        let microseconds = (tick_remainder * MICROSECONDS_PER_SECOND) / ticks_per_second;
        let timestamp_seconds =
            u32::try_from(seconds).map_err(|_| NetworkError::EthernetPcapTimestampOverflow {
                tick,
                ticks_per_second: self.ticks_per_second,
            })?;
        let timestamp_microseconds = u32::try_from(microseconds).map_err(|_| {
            NetworkError::EthernetPcapTimestampOverflow {
                tick,
                ticks_per_second: self.ticks_per_second,
            }
        })?;
        Ok((timestamp_seconds, timestamp_microseconds))
    }

    fn pcap_len_hint(&self) -> usize {
        24 + self
            .records
            .iter()
            .map(|record| 16usize.saturating_add(record.captured_payload.len()))
            .fold(0usize, usize::saturating_add)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetPcapRecord {
    sequence: u64,
    tick: u64,
    timestamp_seconds: u32,
    timestamp_microseconds: u32,
    captured_len: u32,
    original_len: u32,
    captured_payload: Vec<u8>,
}

impl EthernetPcapRecord {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn timestamp_seconds(&self) -> u32 {
        self.timestamp_seconds
    }

    pub const fn timestamp_microseconds(&self) -> u32 {
        self.timestamp_microseconds
    }

    pub const fn captured_len(&self) -> u32 {
        self.captured_len
    }

    pub const fn original_len(&self) -> u32 {
        self.original_len
    }

    pub fn captured_payload(&self) -> &[u8] {
        &self.captured_payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetPcapDumpSnapshot {
    max_capture_bytes: u32,
    ticks_per_second: u64,
    next_sequence: u64,
    records: Vec<EthernetPcapRecord>,
}

impl EthernetPcapDumpSnapshot {
    pub const fn max_capture_bytes(&self) -> u32 {
        self.max_capture_bytes
    }

    pub const fn ticks_per_second(&self) -> u64 {
        self.ticks_per_second
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
