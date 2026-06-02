use crate::hex::decode_hex_u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteTrapOperation {
    Insert,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteTrapKind {
    SoftwareBreakpoint,
    HardwareBreakpoint,
    WriteWatchpoint,
    ReadWatchpoint,
    AccessWatchpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GdbRemoteTrapPoint {
    kind: GdbRemoteTrapKind,
    address: u64,
    size: u64,
}

impl GdbRemoteTrapPoint {
    pub const fn new(kind: GdbRemoteTrapKind, address: u64, size: u64) -> Self {
        Self {
            kind,
            address,
            size,
        }
    }

    pub const fn kind(&self) -> GdbRemoteTrapKind {
        self.kind
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn size(&self) -> u64 {
        self.size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GdbRemoteTrapRequest {
    operation: GdbRemoteTrapOperation,
    point: GdbRemoteTrapPoint,
}

impl GdbRemoteTrapRequest {
    pub const fn new(
        operation: GdbRemoteTrapOperation,
        kind: GdbRemoteTrapKind,
        address: u64,
        size: u64,
    ) -> Self {
        Self {
            operation,
            point: GdbRemoteTrapPoint::new(kind, address, size),
        }
    }

    pub const fn operation(&self) -> GdbRemoteTrapOperation {
        self.operation
    }

    pub const fn point(&self) -> GdbRemoteTrapPoint {
        self.point
    }
}

pub(crate) fn parse_trap_request(payload: &[u8]) -> Option<GdbRemoteTrapRequest> {
    let (operation, request) = if let Some(request) = payload.strip_prefix(b"Z") {
        (GdbRemoteTrapOperation::Insert, request)
    } else if let Some(request) = payload.strip_prefix(b"z") {
        (GdbRemoteTrapOperation::Remove, request)
    } else {
        return None;
    };

    let (&kind, request) = request.split_first()?;
    let request = request.strip_prefix(b",")?;
    let separator = request.iter().position(|byte| *byte == b',')?;
    let address = decode_hex_u64(&request[..separator])?;
    let size = &request[separator + 1..];
    if size.iter().any(|byte| matches!(byte, b',' | b';')) {
        return None;
    }
    Some(GdbRemoteTrapRequest::new(
        operation,
        parse_trap_kind(kind)?,
        address,
        decode_hex_u64(size)?,
    ))
}

fn parse_trap_kind(kind: u8) -> Option<GdbRemoteTrapKind> {
    match kind {
        b'0' => Some(GdbRemoteTrapKind::SoftwareBreakpoint),
        b'1' => Some(GdbRemoteTrapKind::HardwareBreakpoint),
        b'2' => Some(GdbRemoteTrapKind::WriteWatchpoint),
        b'3' => Some(GdbRemoteTrapKind::ReadWatchpoint),
        b'4' => Some(GdbRemoteTrapKind::AccessWatchpoint),
        _ => None,
    }
}
