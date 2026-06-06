use std::io::Read;

use flate2::read::GzDecoder;

use crate::TrafficGeneratorError;

const GEM5_PROTO_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
const WIRE_VARINT: u64 = 0;
const WIRE_FIXED64: u64 = 1;
const WIRE_LENGTH_DELIMITED: u64 = 2;
const WIRE_START_GROUP: u64 = 3;
const WIRE_END_GROUP: u64 = 4;
const WIRE_FIXED32: u64 = 5;

pub(crate) struct Gem5PacketTraceReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Gem5PacketTraceReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self, TrafficGeneratorError> {
        if bytes.len() < GEM5_PROTO_MAGIC.len() {
            return Err(TrafficGeneratorError::TraceTruncatedMagic {
                length: bytes.len(),
            });
        }

        let actual = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if actual != GEM5_PROTO_MAGIC {
            return Err(TrafficGeneratorError::TraceBadMagic { actual });
        }

        Ok(Self {
            bytes,
            offset: GEM5_PROTO_MAGIC.len(),
        })
    }

    pub(crate) fn next_message(&mut self) -> Result<Option<&'a [u8]>, TrafficGeneratorError> {
        if self.offset == self.bytes.len() {
            return Ok(None);
        }

        let length_offset = self.offset;
        let length = read_varint_u32(self.bytes, &mut self.offset)?;
        let length = usize::try_from(length).expect("u32 message length fits usize");
        let remaining = self.bytes.len() - self.offset;
        if length > remaining {
            return Err(TrafficGeneratorError::TraceTruncatedMessage {
                offset: length_offset,
                length,
                remaining,
            });
        }

        let start = self.offset;
        self.offset += length;
        Ok(Some(&self.bytes[start..self.offset]))
    }
}

pub(crate) fn is_gzip_stream(bytes: &[u8]) -> bool {
    bytes.starts_with(&GZIP_MAGIC)
}

pub(crate) fn decompress_gzip_trace(bytes: &[u8]) -> Result<Vec<u8>, TrafficGeneratorError> {
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|error| {
        TrafficGeneratorError::TraceGzipDecode {
            message: error.to_string(),
        }
    })?;
    Ok(decompressed)
}

#[derive(Clone, Copy)]
pub(crate) struct ProtoField {
    pub(crate) number: u32,
    wire_type: u64,
    value_offset: usize,
    varint_value: Option<u64>,
}

impl ProtoField {
    pub(crate) fn varint(
        self,
        message: &'static str,
        field: &'static str,
    ) -> Result<u64, TrafficGeneratorError> {
        if self.wire_type != WIRE_VARINT {
            return Err(TrafficGeneratorError::TraceInvalidFieldWireType {
                message,
                field,
                wire_type: self.wire_type,
            });
        }

        self.varint_value
            .ok_or(TrafficGeneratorError::TraceInvalidFieldWireType {
                message,
                field,
                wire_type: self.wire_type,
            })
    }
}

pub(crate) struct ProtoMessageParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ProtoMessageParser<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn next_field(&mut self) -> Result<Option<ProtoField>, TrafficGeneratorError> {
        if self.offset == self.bytes.len() {
            return Ok(None);
        }

        let tag = read_varint_u64(self.bytes, &mut self.offset)?;
        let number = tag >> 3;
        let wire_type = tag & 0x7;
        if number == 0 {
            return Err(TrafficGeneratorError::TraceInvalidFieldNumber);
        }
        let number = u32::try_from(number)
            .map_err(|_| TrafficGeneratorError::TraceFieldNumberTooLarge { number })?;
        let value_offset = self.offset;
        let varint_value = if wire_type == WIRE_VARINT {
            let mut value_end = value_offset;
            Some(read_varint_u64(self.bytes, &mut value_end)?)
        } else {
            None
        };

        Ok(Some(ProtoField {
            number,
            wire_type,
            value_offset,
            varint_value,
        }))
    }

    pub(crate) fn skip(&mut self, field: ProtoField) -> Result<(), TrafficGeneratorError> {
        match field.wire_type {
            WIRE_VARINT => {
                self.offset = field.value_offset;
                let _ = read_varint_u64(self.bytes, &mut self.offset)?;
                Ok(())
            }
            WIRE_FIXED64 => self.skip_bytes(field.value_offset, 8),
            WIRE_LENGTH_DELIMITED => {
                self.offset = field.value_offset;
                let length = read_varint_u64(self.bytes, &mut self.offset)?;
                let length = usize::try_from(length).map_err(|_| {
                    TrafficGeneratorError::TraceLengthDelimitedFieldTooLarge {
                        offset: field.value_offset,
                        length,
                    }
                })?;
                self.skip_bytes(self.offset, length)
            }
            WIRE_FIXED32 => self.skip_bytes(field.value_offset, 4),
            WIRE_START_GROUP | WIRE_END_GROUP => {
                Err(TrafficGeneratorError::TraceUnsupportedWireType {
                    wire_type: field.wire_type,
                })
            }
            wire_type => Err(TrafficGeneratorError::TraceInvalidWireType { wire_type }),
        }
    }

    fn skip_bytes(&mut self, start: usize, length: usize) -> Result<(), TrafficGeneratorError> {
        let remaining = self.bytes.len().saturating_sub(start);
        if length > remaining {
            return Err(TrafficGeneratorError::TraceTruncatedField {
                offset: start,
                length,
                remaining,
            });
        }

        self.offset = start + length;
        Ok(())
    }
}

pub(crate) fn read_u32_field(
    field: ProtoField,
    message: &'static str,
    name: &'static str,
) -> Result<u32, TrafficGeneratorError> {
    let value = field.varint(message, name)?;
    u32::try_from(value).map_err(|_| TrafficGeneratorError::TraceFieldOutOfRange {
        message,
        field: name,
        value,
    })
}

fn read_varint_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, TrafficGeneratorError> {
    let start = *offset;
    let mut value = 0u64;

    for byte_index in 0..10 {
        let byte = *bytes
            .get(*offset)
            .ok_or(TrafficGeneratorError::TraceTruncatedVarint { offset: start })?;
        *offset += 1;

        let payload = u64::from(byte & 0x7f);
        if byte_index == 9 && payload > 1 {
            return Err(TrafficGeneratorError::TraceVarintTooLong { offset: start });
        }
        value |= payload << (byte_index * 7);

        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }

    Err(TrafficGeneratorError::TraceVarintTooLong { offset: start })
}

fn read_varint_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, TrafficGeneratorError> {
    let start = *offset;
    let mut value = 0u64;

    for byte_index in 0..5 {
        let byte = *bytes
            .get(*offset)
            .ok_or(TrafficGeneratorError::TraceTruncatedVarint { offset: start })?;
        *offset += 1;

        let payload = u64::from(byte & 0x7f);
        value |= payload << (byte_index * 7);

        if byte & 0x80 == 0 {
            if value > u64::from(u32::MAX) {
                return Err(TrafficGeneratorError::TraceMessageTooLarge {
                    offset: start,
                    length: value,
                });
            }
            return Ok(value as u32);
        }
    }

    Err(TrafficGeneratorError::TraceVarint32TooLong { offset: start })
}
