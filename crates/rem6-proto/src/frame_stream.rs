use crate::{ProtoError, TraceFrame};

const FRAME_STREAM_MAGIC: &[u8; 4] = b"RM6S";
const FRAME_STREAM_VERSION: u16 = 1;
const STREAM_HEADER_BYTES: usize = 4 + 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrameStream {
    frames: Vec<TraceFrame>,
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

        let mut frames = Vec::new();
        let mut cursor = STREAM_HEADER_BYTES;
        while cursor < bytes.len() {
            let (frame_len, next_cursor) = read_varint32(bytes, cursor)?;
            cursor = next_cursor;
            let frame_len =
                usize::try_from(frame_len).map_err(|_| ProtoError::InvalidFrameStreamLength)?;
            if frame_len == 0 {
                return Err(ProtoError::InvalidFrameStreamLength);
            }
            let frame_end = cursor
                .checked_add(frame_len)
                .ok_or(ProtoError::TruncatedFrameStream)?;
            if frame_end > bytes.len() {
                return Err(ProtoError::TruncatedFrameStream);
            }
            frames.push(TraceFrame::decode(&bytes[cursor..frame_end])?);
            cursor = frame_end;
        }

        Self::new(frames)
    }
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
