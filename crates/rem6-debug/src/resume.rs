use crate::hex::decode_hex_u64;
use crate::GdbRemoteThreadId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteResumeKind {
    Continue,
    SingleInstruction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GdbRemoteResumeRequest {
    kind: GdbRemoteResumeKind,
    signal: Option<u8>,
    address: Option<u64>,
    thread: GdbRemoteThreadId,
}

impl GdbRemoteResumeRequest {
    pub const fn new(
        kind: GdbRemoteResumeKind,
        signal: Option<u8>,
        address: Option<u64>,
        thread: GdbRemoteThreadId,
    ) -> Self {
        Self {
            kind,
            signal,
            address,
            thread,
        }
    }

    pub const fn kind(&self) -> GdbRemoteResumeKind {
        self.kind
    }

    pub const fn signal(&self) -> Option<u8> {
        self.signal
    }

    pub const fn address(&self) -> Option<u64> {
        self.address
    }

    pub const fn thread(&self) -> GdbRemoteThreadId {
        self.thread
    }
}

pub(crate) fn parse_resume_request(
    request: &[u8],
    accepts_signal: bool,
) -> Option<(Option<u8>, Option<u64>)> {
    if accepts_signal {
        return parse_signaled_resume_request(request);
    }
    parse_resume_address(request).map(|address| (None, address))
}

fn parse_signaled_resume_request(request: &[u8]) -> Option<(Option<u8>, Option<u64>)> {
    if request.len() < 2 {
        return None;
    }

    let signal = parse_signal(&request[..2])?;
    let rest = &request[2..];
    if rest.is_empty() {
        return Some((Some(signal), None));
    }

    let address = rest.strip_prefix(b";")?;
    if address.is_empty() {
        return None;
    }
    Some((Some(signal), Some(decode_hex_u64(address)?)))
}

fn parse_resume_address(request: &[u8]) -> Option<Option<u64>> {
    if request.is_empty() {
        return Some(None);
    }
    Some(Some(decode_hex_u64(request)?))
}

fn parse_signal(signal: &[u8]) -> Option<u8> {
    if signal.len() != 2 {
        return None;
    }
    u8::try_from(decode_hex_u64(signal)?).ok()
}
