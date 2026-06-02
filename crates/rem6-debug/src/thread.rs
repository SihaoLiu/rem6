use crate::hex::decode_hex_u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteThreadOperation {
    Continue,
    General,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteThreadId {
    All,
    Any,
    Id(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteThreadInfoQuery {
    First,
    Subsequent,
}

pub(crate) fn parse_thread_selection(
    request: &[u8],
) -> Option<(GdbRemoteThreadOperation, GdbRemoteThreadId)> {
    let (operation, thread_id) = request.split_first()?;
    let operation = match operation {
        b'c' => GdbRemoteThreadOperation::Continue,
        b'g' => GdbRemoteThreadOperation::General,
        _ => return None,
    };
    Some((operation, parse_thread_id(thread_id)?))
}

pub(crate) fn parse_thread_id(thread_id: &[u8]) -> Option<GdbRemoteThreadId> {
    if thread_id == b"-1" {
        return Some(GdbRemoteThreadId::All);
    }
    if thread_id == b"0" {
        return Some(GdbRemoteThreadId::Any);
    }

    let id = decode_hex_u64(thread_id)?;
    if id == 0 {
        return None;
    }
    Some(GdbRemoteThreadId::Id(id))
}
