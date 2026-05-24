use std::ops::Range;

use crate::{ProtoError, TraceFrame, TraceFrameKind};

const FRAME_STREAM_MAGIC: &[u8; 4] = b"RM6S";
const FRAME_STREAM_VERSION: u16 = 1;
const STREAM_HEADER_BYTES: usize = 4 + 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrameStream {
    frames: Vec<TraceFrame>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrameStreamRecord {
    index: usize,
    length_offset: usize,
    frame_offset: usize,
    frame_len: usize,
    frame: TraceFrame,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrameStreamIndexRecord {
    index: usize,
    kind: TraceFrameKind,
    identity: String,
    length_offset: usize,
    frame_offset: usize,
    frame_len: usize,
    payload_len: usize,
}

impl TraceFrameStreamIndexRecord {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn kind(&self) -> TraceFrameKind {
        self.kind
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn length_offset(&self) -> usize {
        self.length_offset
    }

    pub const fn frame_offset(&self) -> usize {
        self.frame_offset
    }

    pub const fn frame_len(&self) -> usize {
        self.frame_len
    }

    pub const fn payload_len(&self) -> usize {
        self.payload_len
    }

    pub fn byte_range(&self) -> Range<usize> {
        self.frame_offset..self.frame_offset + self.frame_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrameStreamIndex {
    records: Vec<TraceFrameStreamIndexRecord>,
    total_frame_bytes: usize,
    total_payload_bytes: usize,
}

impl TraceFrameStreamIndex {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        let mut cursor = TraceFrameStreamCursor::new(bytes)?;
        let mut records = Vec::new();
        let mut total_frame_bytes = 0usize;
        let mut total_payload_bytes = 0usize;

        while let Some(record) = cursor.next_frame()? {
            let frame = record.frame();
            let frame_len = record.frame_len();
            let payload_len = frame.payload().len();
            total_frame_bytes = total_frame_bytes
                .checked_add(frame_len)
                .ok_or(ProtoError::InvalidFrameStreamLength)?;
            total_payload_bytes = total_payload_bytes
                .checked_add(payload_len)
                .ok_or(ProtoError::InvalidFrameStreamLength)?;
            records.push(TraceFrameStreamIndexRecord {
                index: record.index(),
                kind: frame.kind(),
                identity: frame.identity().to_string(),
                length_offset: record.length_offset(),
                frame_offset: record.frame_offset(),
                frame_len,
                payload_len,
            });
        }

        Ok(Self {
            records,
            total_frame_bytes,
            total_payload_bytes,
        })
    }

    pub fn records(&self) -> &[TraceFrameStreamIndexRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub const fn total_frame_bytes(&self) -> usize {
        self.total_frame_bytes
    }

    pub const fn total_payload_bytes(&self) -> usize {
        self.total_payload_bytes
    }

    pub fn count_kind(&self, kind: TraceFrameKind) -> usize {
        self.records_for_kind(kind).count()
    }

    pub fn records_for_kind(
        &self,
        kind: TraceFrameKind,
    ) -> impl Iterator<Item = &TraceFrameStreamIndexRecord> {
        self.records
            .iter()
            .filter(move |record| record.kind() == kind)
    }
}

impl TraceFrameStreamRecord {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn length_offset(&self) -> usize {
        self.length_offset
    }

    pub const fn frame_offset(&self) -> usize {
        self.frame_offset
    }

    pub const fn frame_len(&self) -> usize {
        self.frame_len
    }

    pub const fn frame(&self) -> &TraceFrame {
        &self.frame
    }
}

#[derive(Clone, Debug)]
pub struct TraceFrameStreamCursor<'a> {
    bytes: &'a [u8],
    cursor: usize,
    index: usize,
}

impl TraceFrameStream {
    pub fn new(frames: Vec<TraceFrame>) -> Result<Self, ProtoError> {
        if frames.is_empty() {
            return Err(ProtoError::EmptyFrameStream);
        }
        Ok(Self { frames })
    }

    pub fn frames(&self) -> &[TraceFrame] {
        &self.frames
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(FRAME_STREAM_MAGIC);
        bytes.extend_from_slice(&FRAME_STREAM_VERSION.to_le_bytes());
        for frame in &self.frames {
            let frame_bytes = frame.encode();
            let frame_len =
                u32::try_from(frame_bytes.len()).expect("trace frame exceeds stream length limit");
            write_varint32(frame_len, &mut bytes);
            bytes.extend_from_slice(&frame_bytes);
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtoError> {
        let mut cursor = TraceFrameStreamCursor::new(bytes)?;
        let mut frames = Vec::new();
        while let Some(record) = cursor.next_frame()? {
            frames.push(record.frame);
        }

        Self::new(frames)
    }
}

impl<'a> TraceFrameStreamCursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, ProtoError> {
        validate_stream_header(bytes)?;
        if bytes.len() == STREAM_HEADER_BYTES {
            return Err(ProtoError::EmptyFrameStream);
        }
        Ok(Self {
            bytes,
            cursor: STREAM_HEADER_BYTES,
            index: 0,
        })
    }

    pub const fn byte_position(&self) -> usize {
        self.cursor
    }

    pub const fn next_index(&self) -> usize {
        self.index
    }

    pub fn is_finished(&self) -> bool {
        self.cursor == self.bytes.len()
    }

    pub fn reset(&mut self) {
        self.cursor = STREAM_HEADER_BYTES;
        self.index = 0;
    }

    pub fn next_frame(&mut self) -> Result<Option<TraceFrameStreamRecord>, ProtoError> {
        if self.is_finished() {
            return Ok(None);
        }

        let length_offset = self.cursor;
        let (frame_len, frame_offset) = read_varint32(self.bytes, length_offset)?;
        let frame_len =
            usize::try_from(frame_len).map_err(|_| ProtoError::InvalidFrameStreamLength)?;
        if frame_len == 0 {
            return Err(ProtoError::InvalidFrameStreamLength);
        }
        let frame_end = frame_offset
            .checked_add(frame_len)
            .ok_or(ProtoError::TruncatedFrameStream)?;
        if frame_end > self.bytes.len() {
            return Err(ProtoError::TruncatedFrameStream);
        }
        let frame = TraceFrame::decode(&self.bytes[frame_offset..frame_end])?;
        let record = TraceFrameStreamRecord {
            index: self.index,
            length_offset,
            frame_offset,
            frame_len,
            frame,
        };
        self.cursor = frame_end;
        self.index += 1;
        Ok(Some(record))
    }
}

fn validate_stream_header(bytes: &[u8]) -> Result<(), ProtoError> {
    if bytes.len() < STREAM_HEADER_BYTES {
        return Err(ProtoError::TruncatedFrameStream);
    }
    if &bytes[..4] != FRAME_STREAM_MAGIC {
        return Err(ProtoError::InvalidFrameStreamMagic);
    }

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != FRAME_STREAM_VERSION {
        return Err(ProtoError::UnsupportedFrameStreamVersion { version });
    }
    Ok(())
}

fn write_varint32(mut value: u32, bytes: &mut Vec<u8>) {
    while value >= 0x80 {
        bytes.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

fn read_varint32(bytes: &[u8], offset: usize) -> Result<(u32, usize), ProtoError> {
    let mut value = 0u32;
    let mut shift = 0;
    let mut cursor = offset;
    for _ in 0..5 {
        if cursor == bytes.len() {
            return Err(ProtoError::TruncatedFrameStream);
        }
        let byte = bytes[cursor];
        cursor += 1;
        let chunk = u32::from(byte & 0x7f);
        if shift == 28 && chunk > 0x0f {
            return Err(ProtoError::InvalidFrameStreamLength);
        }
        value |= chunk << shift;
        if byte & 0x80 == 0 {
            return Ok((value, cursor));
        }
        shift += 7;
    }
    Err(ProtoError::InvalidFrameStreamLength)
}
