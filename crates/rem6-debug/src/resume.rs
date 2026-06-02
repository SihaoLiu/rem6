use crate::hex::decode_hex_u64;
use crate::{parse_thread_id, GdbRemoteThreadId};

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

pub(crate) fn parse_vcont_requests(request: &[u8]) -> Option<Vec<GdbRemoteResumeRequest>> {
    let actions = request.strip_prefix(b"vCont;")?;
    if actions.is_empty() {
        return None;
    }

    let mut requests = Vec::new();
    for action in actions.split(|byte| *byte == b';') {
        if action.is_empty() {
            return None;
        }
        requests.push(parse_vcont_action(action)?);
    }
    Some(requests)
}

fn parse_vcont_action(action: &[u8]) -> Option<GdbRemoteResumeRequest> {
    match action.first()? {
        b'c' => parse_vcont_unary_action(GdbRemoteResumeKind::Continue, None, &action[1..]),
        b'C' => {
            let (signal, rest) = parse_vcont_signal_action(&action[1..])?;
            parse_vcont_unary_action(GdbRemoteResumeKind::Continue, Some(signal), rest)
        }
        b's' => {
            parse_vcont_unary_action(GdbRemoteResumeKind::SingleInstruction, None, &action[1..])
        }
        b'S' => {
            let (signal, rest) = parse_vcont_signal_action(&action[1..])?;
            parse_vcont_unary_action(GdbRemoteResumeKind::SingleInstruction, Some(signal), rest)
        }
        _ => None,
    }
}

fn parse_vcont_signal_action(action: &[u8]) -> Option<(u8, &[u8])> {
    if action.len() < 2 {
        return None;
    }
    Some((parse_signal(&action[..2])?, &action[2..]))
}

fn parse_vcont_unary_action(
    kind: GdbRemoteResumeKind,
    signal: Option<u8>,
    action: &[u8],
) -> Option<GdbRemoteResumeRequest> {
    let thread = if action.is_empty() {
        GdbRemoteThreadId::All
    } else {
        parse_thread_id(action.strip_prefix(b":")?)?
    };
    Some(GdbRemoteResumeRequest::new(kind, signal, None, thread))
}
