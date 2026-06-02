use crate::hex::decode_hex_u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteDisconnectRequest {
    Detach { process_id: Option<u64> },
    Terminate,
    TerminateProcess { process_id: u64 },
}

pub(crate) fn parse_disconnect_request(payload: &[u8]) -> Option<GdbRemoteDisconnectRequest> {
    if payload == b"D" {
        return Some(GdbRemoteDisconnectRequest::Detach { process_id: None });
    }
    if let Some(process_id) = payload.strip_prefix(b"D;") {
        return Some(GdbRemoteDisconnectRequest::Detach {
            process_id: Some(parse_process_id(process_id)?),
        });
    }
    if payload == b"k" {
        return Some(GdbRemoteDisconnectRequest::Terminate);
    }
    if let Some(process_id) = payload.strip_prefix(b"vKill;") {
        return Some(GdbRemoteDisconnectRequest::TerminateProcess {
            process_id: parse_process_id(process_id)?,
        });
    }
    None
}

fn parse_process_id(process_id: &[u8]) -> Option<u64> {
    let process_id = decode_hex_u64(process_id)?;
    if process_id == 0 {
        return None;
    }
    Some(process_id)
}
