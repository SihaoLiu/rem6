use crate::hex::decode_hex_usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteXferObject {
    Features,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteXferReadRequest {
    object: GdbRemoteXferObject,
    annex: Vec<u8>,
    offset: usize,
    length: usize,
}

impl GdbRemoteXferReadRequest {
    pub const fn new(
        object: GdbRemoteXferObject,
        annex: Vec<u8>,
        offset: usize,
        length: usize,
    ) -> Self {
        Self {
            object,
            annex,
            offset,
            length,
        }
    }

    pub const fn object(&self) -> GdbRemoteXferObject {
        self.object
    }

    pub fn annex(&self) -> &[u8] {
        &self.annex
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn length(&self) -> usize {
        self.length
    }
}

pub(crate) fn parse_xfer_read(payload: &[u8]) -> Option<GdbRemoteXferReadRequest> {
    let request = payload.strip_prefix(b"qXfer:")?;
    let (object, request) = split_once(request, b':')?;
    let (operation, request) = split_once(request, b':')?;
    let (annex, range) = split_once(request, b':')?;
    let (offset, length) = split_once(range, b',')?;

    if object != b"features" || operation != b"read" || annex.is_empty() {
        return None;
    }

    let offset = decode_hex_usize(offset)?;
    let length = decode_hex_usize(length)?;
    if length == 0 {
        return None;
    }

    Some(GdbRemoteXferReadRequest::new(
        GdbRemoteXferObject::Features,
        annex.to_vec(),
        offset,
        length,
    ))
}

fn split_once(data: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    let position = data.iter().position(|byte| *byte == separator)?;
    Some((&data[..position], &data[position + 1..]))
}
