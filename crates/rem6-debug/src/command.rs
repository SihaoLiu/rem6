use crate::disconnect::parse_disconnect_request;
use crate::feature::parse_supported_features;
use crate::hex::{decode_hex_bytes, decode_hex_u64, decode_hex_usize};
use crate::resume::{parse_resume_request, parse_vcont_requests};
use crate::thread::parse_thread_selection;
use crate::trap::parse_trap_request;
use crate::xfer::parse_xfer_read;
use crate::{GdbRemoteCommand, GdbRemotePacket, GdbRemoteResumeKind, GdbRemoteThreadInfoQuery};

impl GdbRemoteCommand {
    pub fn parse(packet: &GdbRemotePacket) -> Self {
        parse_command_payload(packet.payload())
    }
}

fn parse_command_payload(payload: &[u8]) -> GdbRemoteCommand {
    const QUERY_ATTACHED: &[u8] = b"qAttached";
    const QUERY_CURRENT_THREAD: &[u8] = b"qC";
    const QUERY_FIRST_THREAD_INFO: &[u8] = b"qfThreadInfo";
    const QUERY_MONITOR_COMMAND: &[u8] = b"qRcmd";
    const QUERY_SUBSEQUENT_THREAD_INFO: &[u8] = b"qsThreadInfo";
    const QUERY_SYMBOL: &[u8] = b"qSymbol";
    const READ_REGISTERS: &[u8] = b"g";
    const QUERY_SUPPORTED: &[u8] = b"qSupported";
    const QUERY_STOP_REASON: &[u8] = b"?";
    const START_NO_ACK_MODE: &[u8] = b"QStartNoAckMode";
    const QUERY_RESUME_ACTIONS: &[u8] = b"vCont?";

    if payload == READ_REGISTERS {
        return GdbRemoteCommand::ReadRegisters;
    }

    if let Some(request) = parse_disconnect_request(payload) {
        return GdbRemoteCommand::Disconnect { request };
    }

    if let Some(request) = parse_trap_request(payload) {
        return GdbRemoteCommand::Trap { request };
    }

    if let Some(register_data) = payload.strip_prefix(b"G") {
        if let Some(bytes) = parse_registers_write(register_data) {
            return GdbRemoteCommand::WriteRegisters { bytes };
        }
    }

    if let Some(thread_request) = payload.strip_prefix(b"H") {
        if let Some((operation, thread)) = parse_thread_selection(thread_request) {
            return GdbRemoteCommand::SetThread { operation, thread };
        }
    }

    if let Some(thread_id) = payload.strip_prefix(b"T") {
        if let Some(thread) = crate::parse_thread_id(thread_id) {
            return GdbRemoteCommand::ThreadAlive { thread };
        }
    }

    if let Some(request) = payload.strip_prefix(b"c") {
        if let Some((signal, address)) = parse_resume_request(request, false) {
            return GdbRemoteCommand::Resume {
                kind: GdbRemoteResumeKind::Continue,
                signal,
                address,
            };
        }
    }

    if let Some(request) = payload.strip_prefix(b"C") {
        if let Some((signal, address)) = parse_resume_request(request, true) {
            return GdbRemoteCommand::Resume {
                kind: GdbRemoteResumeKind::Continue,
                signal,
                address,
            };
        }
    }

    if let Some(request) = payload.strip_prefix(b"s") {
        if let Some((signal, address)) = parse_resume_request(request, false) {
            return GdbRemoteCommand::Resume {
                kind: GdbRemoteResumeKind::SingleInstruction,
                signal,
                address,
            };
        }
    }

    if let Some(request) = payload.strip_prefix(b"S") {
        if let Some((signal, address)) = parse_resume_request(request, true) {
            return GdbRemoteCommand::Resume {
                kind: GdbRemoteResumeKind::SingleInstruction,
                signal,
                address,
            };
        }
    }

    if let Some(memory_request) = payload.strip_prefix(b"m") {
        if let Some((address, length)) = parse_memory_read(memory_request) {
            return GdbRemoteCommand::ReadMemory { address, length };
        }
    }

    if let Some(memory_request) = payload.strip_prefix(b"M") {
        if let Some((address, bytes)) = parse_memory_write(memory_request) {
            return GdbRemoteCommand::WriteMemory { address, bytes };
        }
    }

    if let Some(register_number) = payload.strip_prefix(b"p") {
        if let Some(number) = decode_hex_u64(register_number) {
            return GdbRemoteCommand::ReadRegister { number };
        }
    }

    if let Some(register_request) = payload.strip_prefix(b"P") {
        if let Some((number, bytes)) = parse_register_write(register_request) {
            return GdbRemoteCommand::WriteRegister { number, bytes };
        }
    }

    if payload == QUERY_ATTACHED {
        return GdbRemoteCommand::QueryAttached { process_id: None };
    }

    if let Some(process_id) = payload.strip_prefix(b"qAttached:") {
        if let Some(process_id) = parse_process_id(process_id) {
            return GdbRemoteCommand::QueryAttached {
                process_id: Some(process_id),
            };
        }
    }

    if payload == QUERY_CURRENT_THREAD {
        return GdbRemoteCommand::QueryCurrentThread;
    }

    if payload == QUERY_SYMBOL || payload.starts_with(b"qSymbol:") {
        return GdbRemoteCommand::QuerySymbol;
    }

    if let Some(command) = parse_monitor_command(payload, QUERY_MONITOR_COMMAND) {
        return GdbRemoteCommand::QueryMonitorCommand { command };
    }

    if payload == QUERY_FIRST_THREAD_INFO {
        return GdbRemoteCommand::QueryThreadInfo {
            query: GdbRemoteThreadInfoQuery::First,
        };
    }

    if payload == QUERY_SUBSEQUENT_THREAD_INFO {
        return GdbRemoteCommand::QueryThreadInfo {
            query: GdbRemoteThreadInfoQuery::Subsequent,
        };
    }

    if payload == QUERY_STOP_REASON {
        return GdbRemoteCommand::QueryStopReason;
    }

    if let Some(request) = parse_xfer_read(payload) {
        return GdbRemoteCommand::QueryXferRead { request };
    }

    if payload == QUERY_RESUME_ACTIONS {
        return GdbRemoteCommand::QueryResumeActions;
    }

    if let Some(requests) = parse_vcont_requests(payload) {
        return GdbRemoteCommand::ResumeActions { requests };
    }

    if payload == START_NO_ACK_MODE {
        return GdbRemoteCommand::StartNoAckMode;
    }

    if payload == QUERY_SUPPORTED {
        return GdbRemoteCommand::QuerySupported {
            features: Vec::new(),
        };
    }

    if let Some(features) = payload.strip_prefix(b"qSupported:") {
        return GdbRemoteCommand::QuerySupported {
            features: parse_supported_features(features, false),
        };
    }

    GdbRemoteCommand::Unknown(payload.to_vec())
}

fn parse_process_id(process_id: &[u8]) -> Option<u64> {
    parse_positive_hex_id(process_id)
}

fn parse_positive_hex_id(id: &[u8]) -> Option<u64> {
    let id = decode_hex_u64(id)?;
    if id == 0 {
        return None;
    }
    Some(id)
}

fn parse_monitor_command(payload: &[u8], query: &[u8]) -> Option<Vec<u8>> {
    let rest = payload.strip_prefix(query)?;
    if rest.is_empty() {
        return Some(Vec::new());
    }
    let encoded = rest
        .strip_prefix(b",")
        .or_else(|| rest.strip_prefix(b":"))?;
    decode_hex_bytes(encoded)
}

fn parse_memory_read(request: &[u8]) -> Option<(u64, usize)> {
    let separator = request.iter().position(|byte| *byte == b',')?;
    let address = decode_hex_u64(&request[..separator])?;
    let length = decode_hex_usize(&request[separator + 1..])?;
    Some((address, length))
}

fn parse_memory_write(request: &[u8]) -> Option<(u64, Vec<u8>)> {
    let separator = request.iter().position(|byte| *byte == b':')?;
    let (address, length) = parse_memory_read(&request[..separator])?;
    let bytes = decode_hex_bytes(&request[separator + 1..])?;
    if bytes.len() != length {
        return None;
    }
    Some((address, bytes))
}

fn parse_registers_write(request: &[u8]) -> Option<Vec<u8>> {
    let bytes = decode_hex_bytes(request)?;
    if bytes.is_empty() {
        return None;
    }
    Some(bytes)
}

fn parse_register_write(request: &[u8]) -> Option<(u64, Vec<u8>)> {
    let separator = request.iter().position(|byte| *byte == b'=')?;
    let number = decode_hex_u64(&request[..separator])?;
    let bytes = decode_hex_bytes(&request[separator + 1..])?;
    if bytes.is_empty() {
        return None;
    }
    Some((number, bytes))
}
