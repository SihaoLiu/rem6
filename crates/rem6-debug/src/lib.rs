use std::error::Error;
use std::fmt;

pub const DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES: usize = 16 * 1024;
const PACKET_START_BYTE: u8 = b'$';
const CHECKSUM_SEPARATOR_BYTE: u8 = b'#';
const ACK_BYTE: u8 = b'+';
const NEGATIVE_ACK_BYTE: u8 = b'-';
const INTERRUPT_BYTE: u8 = 0x03;
const ESCAPE_BYTE: u8 = b'}';
const RUN_LENGTH_BYTE: u8 = b'*';
const ESCAPE_XOR: u8 = 0x20;
const RUN_LENGTH_COUNT_BIAS: u8 = 29;
const MIN_RUN_LENGTH_REPEAT_COUNT: u8 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteError {
    MissingPacketStart,
    MissingChecksumSeparator,
    ShortChecksum,
    InvalidChecksumHex { byte: u8 },
    ChecksumMismatch { expected: u8, actual: u8 },
    TrailingBytes { count: usize },
    ZeroMaxPayloadBytes,
    PayloadTooLong { len: usize, max: usize },
    RunLengthWithoutPreviousByte,
    MissingRunLengthCount,
    InvalidRunLengthCount { byte: u8 },
}

impl fmt::Display for GdbRemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPacketStart => write!(formatter, "GDB remote packet must start with '$'"),
            Self::MissingChecksumSeparator => {
                write!(formatter, "GDB remote packet is missing checksum separator")
            }
            Self::ShortChecksum => write!(formatter, "GDB remote packet checksum is incomplete"),
            Self::InvalidChecksumHex { byte } => write!(
                formatter,
                "GDB remote packet checksum contains invalid hex byte 0x{byte:02x}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "GDB remote packet checksum mismatch: expected 0x{expected:02x}, got 0x{actual:02x}"
            ),
            Self::TrailingBytes { count } => write!(
                formatter,
                "GDB remote packet has {count} trailing byte(s) after checksum"
            ),
            Self::ZeroMaxPayloadBytes => {
                write!(formatter, "GDB remote max payload bytes must be positive")
            }
            Self::PayloadTooLong { len, max } => write!(
                formatter,
                "GDB remote payload has {len} byte(s), exceeding configured maximum {max}"
            ),
            Self::RunLengthWithoutPreviousByte => {
                write!(
                    formatter,
                    "GDB remote run-length marker has no preceding payload byte"
                )
            }
            Self::MissingRunLengthCount => {
                write!(formatter, "GDB remote run-length marker has no count byte")
            }
            Self::InvalidRunLengthCount { byte } => write!(
                formatter,
                "GDB remote run-length count byte 0x{byte:02x} encodes fewer than 3 repeats"
            ),
        }
    }
}

impl Error for GdbRemoteError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GdbRemotePacketConfig {
    max_payload_bytes: usize,
}

impl GdbRemotePacketConfig {
    pub fn new(max_payload_bytes: usize) -> Result<Self, GdbRemoteError> {
        if max_payload_bytes == 0 {
            return Err(GdbRemoteError::ZeroMaxPayloadBytes);
        }
        Ok(Self { max_payload_bytes })
    }

    pub const fn max_payload_bytes(self) -> usize {
        self.max_payload_bytes
    }
}

impl Default for GdbRemotePacketConfig {
    fn default() -> Self {
        Self {
            max_payload_bytes: DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemotePacket {
    payload: Vec<u8>,
    checksum: u8,
}

impl GdbRemotePacket {
    pub fn new(payload: Vec<u8>) -> Result<Self, GdbRemoteError> {
        Self::with_config(payload, GdbRemotePacketConfig::default())
    }

    pub fn with_config(
        payload: Vec<u8>,
        config: GdbRemotePacketConfig,
    ) -> Result<Self, GdbRemoteError> {
        validate_payload_len(payload.len(), config)?;
        let checksum = checksum(&encode_payload(&payload));
        Ok(Self { payload, checksum })
    }

    pub fn parse_frame(frame: &[u8]) -> Result<Self, GdbRemoteError> {
        Self::parse_frame_with_config(frame, GdbRemotePacketConfig::default())
    }

    pub fn parse_frame_with_config(
        frame: &[u8],
        config: GdbRemotePacketConfig,
    ) -> Result<Self, GdbRemoteError> {
        let (payload, expected, consumed) = parse_packet_parts(frame, config)?;
        if consumed != frame.len() {
            return Err(GdbRemoteError::TrailingBytes {
                count: frame.len() - consumed,
            });
        }
        Ok(Self {
            checksum: expected,
            payload,
        })
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub const fn checksum(&self) -> u8 {
        self.checksum
    }

    pub fn encode_frame(&self) -> Vec<u8> {
        let payload = encode_payload(&self.payload);
        let mut frame = Vec::with_capacity(payload.len() + 4);
        frame.push(PACKET_START_BYTE);
        frame.extend_from_slice(&payload);
        frame.push(CHECKSUM_SEPARATOR_BYTE);
        frame.push(encode_hex_nibble(self.checksum >> 4));
        frame.push(encode_hex_nibble(self.checksum & 0x0f));
        frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteFrame {
    Ack,
    NegativeAck,
    Interrupt,
    Packet(GdbRemotePacket),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteFrameParse {
    frame: GdbRemoteFrame,
    consumed_bytes: usize,
    skipped_bytes: Vec<u8>,
}

impl GdbRemoteFrameParse {
    fn new(frame: GdbRemoteFrame, consumed_bytes: usize, skipped_bytes: Vec<u8>) -> Self {
        Self {
            frame,
            consumed_bytes,
            skipped_bytes,
        }
    }

    pub const fn frame(&self) -> &GdbRemoteFrame {
        &self.frame
    }

    pub const fn consumed_bytes(&self) -> usize {
        self.consumed_bytes
    }

    pub fn skipped_bytes(&self) -> &[u8] {
        &self.skipped_bytes
    }
}

pub fn parse_gdb_remote_frame(input: &[u8]) -> Result<Option<GdbRemoteFrameParse>, GdbRemoteError> {
    parse_gdb_remote_frame_with_config(input, GdbRemotePacketConfig::default())
}

pub fn parse_gdb_remote_frame_with_config(
    input: &[u8],
    config: GdbRemotePacketConfig,
) -> Result<Option<GdbRemoteFrameParse>, GdbRemoteError> {
    let Some(position) = input.iter().position(|byte| {
        matches!(
            *byte,
            ACK_BYTE | NEGATIVE_ACK_BYTE | INTERRUPT_BYTE | PACKET_START_BYTE
        )
    }) else {
        return Ok(None);
    };
    let skipped = input[..position].to_vec();
    match input[position] {
        ACK_BYTE => Ok(Some(GdbRemoteFrameParse::new(
            GdbRemoteFrame::Ack,
            position + 1,
            skipped,
        ))),
        NEGATIVE_ACK_BYTE => Ok(Some(GdbRemoteFrameParse::new(
            GdbRemoteFrame::NegativeAck,
            position + 1,
            skipped,
        ))),
        INTERRUPT_BYTE => Ok(Some(GdbRemoteFrameParse::new(
            GdbRemoteFrame::Interrupt,
            position + 1,
            skipped,
        ))),
        PACKET_START_BYTE => {
            let (payload, expected, consumed) = parse_packet_parts(&input[position..], config)?;
            let packet = GdbRemotePacket {
                payload,
                checksum: expected,
            };
            Ok(Some(GdbRemoteFrameParse::new(
                GdbRemoteFrame::Packet(packet),
                position + consumed,
                skipped,
            )))
        }
        _ => unreachable!("frame marker predicate and match arms must agree"),
    }
}

fn parse_packet_parts(
    frame: &[u8],
    config: GdbRemotePacketConfig,
) -> Result<(Vec<u8>, u8, usize), GdbRemoteError> {
    if frame.first() != Some(&PACKET_START_BYTE) {
        return Err(GdbRemoteError::MissingPacketStart);
    }

    let mut payload = Vec::new();
    let mut expected = 0u8;
    let mut escaped = false;
    let mut index = 1;

    while index < frame.len() {
        let byte = frame[index];
        if escaped {
            expected = expected.wrapping_add(byte);
            payload.push(byte ^ ESCAPE_XOR);
            validate_payload_len(payload.len(), config)?;
            escaped = false;
            index += 1;
            continue;
        }

        if byte == CHECKSUM_SEPARATOR_BYTE {
            if frame.len() < index + 3 {
                return Err(GdbRemoteError::ShortChecksum);
            }
            let actual = decode_checksum(frame[index + 1], frame[index + 2])?;
            if actual != expected {
                return Err(GdbRemoteError::ChecksumMismatch { expected, actual });
            }
            return Ok((payload, expected, index + 3));
        }

        if byte == RUN_LENGTH_BYTE {
            if payload.is_empty() {
                return Err(GdbRemoteError::RunLengthWithoutPreviousByte);
            }
            expected = expected.wrapping_add(byte);
            index += 1;
            if index >= frame.len() || frame[index] == CHECKSUM_SEPARATOR_BYTE {
                return Err(GdbRemoteError::MissingRunLengthCount);
            }
            let repeat_byte = frame[index];
            if repeat_byte < RUN_LENGTH_COUNT_BIAS + MIN_RUN_LENGTH_REPEAT_COUNT {
                return Err(GdbRemoteError::InvalidRunLengthCount { byte: repeat_byte });
            }
            expected = expected.wrapping_add(repeat_byte);
            let previous = payload
                .last()
                .copied()
                .expect("run-length marker already checked preceding payload byte");
            let repeat_count = (repeat_byte - RUN_LENGTH_COUNT_BIAS) as usize;
            validate_payload_len(payload.len() + repeat_count, config)?;
            payload.extend(std::iter::repeat_n(previous, repeat_count));
            index += 1;
            continue;
        }

        expected = expected.wrapping_add(byte);
        if byte == ESCAPE_BYTE {
            escaped = true;
        } else {
            payload.push(byte);
            validate_payload_len(payload.len(), config)?;
        }
        index += 1;
    }

    Err(GdbRemoteError::MissingChecksumSeparator)
}

fn validate_payload_len(len: usize, config: GdbRemotePacketConfig) -> Result<(), GdbRemoteError> {
    if len > config.max_payload_bytes() {
        return Err(GdbRemoteError::PayloadTooLong {
            len,
            max: config.max_payload_bytes(),
        });
    }
    Ok(())
}

fn checksum(payload: &[u8]) -> u8 {
    payload
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

fn encode_payload(payload: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(payload.len());
    for byte in payload {
        match *byte {
            PACKET_START_BYTE | CHECKSUM_SEPARATOR_BYTE | ESCAPE_BYTE | RUN_LENGTH_BYTE => {
                encoded.push(ESCAPE_BYTE);
                encoded.push(byte ^ ESCAPE_XOR);
            }
            _ => encoded.push(*byte),
        }
    }
    encoded
}

fn decode_checksum(high: u8, low: u8) -> Result<u8, GdbRemoteError> {
    let high = decode_hex_nibble(high)?;
    let low = decode_hex_nibble(low)?;
    Ok((high << 4) | low)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, GdbRemoteError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(GdbRemoteError::InvalidChecksumHex { byte }),
    }
}

fn encode_hex_nibble(nibble: u8) -> u8 {
    debug_assert!(nibble < 16);
    match nibble {
        0..=9 => b'0' + nibble,
        10..=15 => b'a' + (nibble - 10),
        _ => unreachable!("nibble must be less than 16"),
    }
}
