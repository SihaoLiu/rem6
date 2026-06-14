mod command;
mod control;
mod disconnect;
mod feature;
mod hex;
mod memory;
mod register;
mod resume;
mod stop;
mod stream;
mod thread;
mod trap;
mod xfer;

pub use control::{GdbRemoteControlState, GdbRemoteExecutionControl};
pub use disconnect::GdbRemoteDisconnectRequest;
use feature::encode_supported_features;
pub use feature::{GdbRemoteFeature, GdbRemoteFeatureValue};
use hex::{decode_checksum, encode_hex_bytes, encode_hex_nibble, encode_hex_u64, is_hex_digit};
use memory::memory_addresses;
pub use register::{GdbRemoteRegisterBytes, GdbRemoteRegisterValue};
pub use resume::{GdbRemoteResumeKind, GdbRemoteResumeRequest};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
pub use stop::GdbRemoteStopReply;
pub use stream::GdbRemoteByteStreamResult;
pub(crate) use thread::parse_thread_id;
pub use thread::{GdbRemoteThreadId, GdbRemoteThreadInfoQuery, GdbRemoteThreadOperation};
pub use trap::{
    GdbRemoteTrapKind, GdbRemoteTrapOperation, GdbRemoteTrapPoint, GdbRemoteTrapRequest,
};
pub use xfer::{GdbRemoteXferObject, GdbRemoteXferReadRequest};

pub const DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES: usize = 16 * 1024;
const PACKET_START_BYTE: u8 = b'$';
const CHECKSUM_SEPARATOR_BYTE: u8 = b'#';
const NOTIFICATION_START_BYTE: u8 = b'%';
const ACK_BYTE: u8 = b'+';
const NEGATIVE_ACK_BYTE: u8 = b'-';
const INTERRUPT_BYTE: u8 = 0x03;
const ESCAPE_BYTE: u8 = b'}';
const RUN_LENGTH_BYTE: u8 = b'*';
const ESCAPE_XOR: u8 = 0x20;
const RUN_LENGTH_COUNT_BIAS: u8 = 29;
const MIN_RUN_LENGTH_REPEAT_COUNT: u8 = 3;
const MAX_RUN_LENGTH_COUNT_BYTE: u8 = 126;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GdbRemotePacketPayloadMode {
    Command,
    Response,
}

impl GdbRemotePacketPayloadMode {
    const fn decodes_run_length(self) -> bool {
        matches!(self, Self::Response)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteError {
    MissingPacketStart,
    MissingChecksumSeparator,
    ShortChecksum,
    InvalidChecksumHex { byte: u8 },
    ChecksumMismatch { expected: u8, actual: u8 },
    TrailingBytes { count: usize },
    ZeroMaxPayloadBytes,
    PayloadTooLong { len: usize, max: usize },
    LegacySequenceIdUnsupported,
    RunLengthWithoutPreviousByte,
    MissingRunLengthCount,
    InvalidRunLengthCount { byte: u8 },
}

impl fmt::Display for GdbRemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPacketStart => write!(formatter, "GDB remote packet must start with '$'"),
            Self::MissingChecksumSeparator => {
                write!(formatter, "GDB remote packet is missing checksum separator")
            }
            Self::ShortChecksum => write!(formatter, "GDB remote packet checksum is incomplete"),
            Self::InvalidChecksumHex { byte } => write!(
                formatter,
                "GDB remote packet checksum contains invalid hex byte 0x{byte:02x}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "GDB remote packet checksum mismatch: expected 0x{expected:02x}, got 0x{actual:02x}"
            ),
            Self::TrailingBytes { count } => write!(
                formatter,
                "GDB remote packet has {count} trailing byte(s) after checksum"
            ),
            Self::ZeroMaxPayloadBytes => {
                write!(formatter, "GDB remote max payload bytes must be positive")
            }
            Self::PayloadTooLong { len, max } => write!(
                formatter,
                "GDB remote payload has {len} byte(s), exceeding configured maximum {max}"
            ),
            Self::LegacySequenceIdUnsupported => {
                write!(
                    formatter,
                    "GDB remote legacy sequence-id packets are unsupported"
                )
            }
            Self::RunLengthWithoutPreviousByte => {
                write!(
                    formatter,
                    "GDB remote run-length marker has no preceding payload byte"
                )
            }
            Self::MissingRunLengthCount => {
                write!(formatter, "GDB remote run-length marker has no count byte")
            }
            Self::InvalidRunLengthCount { byte } => write!(
                formatter,
                "GDB remote run-length count byte 0x{byte:02x} encodes fewer than 3 repeats"
            ),
        }
    }
}

impl Error for GdbRemoteError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GdbRemotePacketConfig {
    max_payload_bytes: usize,
}

impl GdbRemotePacketConfig {
    pub fn new(max_payload_bytes: usize) -> Result<Self, GdbRemoteError> {
        if max_payload_bytes == 0 {
            return Err(GdbRemoteError::ZeroMaxPayloadBytes);
        }
        Ok(Self { max_payload_bytes })
    }

    pub const fn max_payload_bytes(self) -> usize {
        self.max_payload_bytes
    }
}

impl Default for GdbRemotePacketConfig {
    fn default() -> Self {
        Self {
            max_payload_bytes: DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemotePacket {
    payload: Vec<u8>,
    checksum: u8,
}

impl GdbRemotePacket {
    pub fn new(payload: Vec<u8>) -> Result<Self, GdbRemoteError> {
        Self::with_config(payload, GdbRemotePacketConfig::default())
    }

    pub fn with_config(
        payload: Vec<u8>,
        config: GdbRemotePacketConfig,
    ) -> Result<Self, GdbRemoteError> {
        validate_payload_len(payload.len(), config)?;
        let checksum = checksum(&encode_payload(&payload));
        Ok(Self { payload, checksum })
    }

    pub fn parse_frame(frame: &[u8]) -> Result<Self, GdbRemoteError> {
        Self::parse_frame_with_config(frame, GdbRemotePacketConfig::default())
    }

    pub fn parse_frame_with_config(
        frame: &[u8],
        config: GdbRemotePacketConfig,
    ) -> Result<Self, GdbRemoteError> {
        Self::parse_frame_inner(frame, config, GdbRemotePacketPayloadMode::Command)
    }

    pub fn parse_response_frame(frame: &[u8]) -> Result<Self, GdbRemoteError> {
        Self::parse_response_frame_with_config(frame, GdbRemotePacketConfig::default())
    }

    pub fn parse_response_frame_with_config(
        frame: &[u8],
        config: GdbRemotePacketConfig,
    ) -> Result<Self, GdbRemoteError> {
        Self::parse_frame_inner(frame, config, GdbRemotePacketPayloadMode::Response)
    }

    fn parse_frame_inner(
        frame: &[u8],
        config: GdbRemotePacketConfig,
        payload_mode: GdbRemotePacketPayloadMode,
    ) -> Result<Self, GdbRemoteError> {
        let (payload, expected, consumed) = parse_packet_parts(frame, config, payload_mode)?;
        if consumed != frame.len() {
            return Err(GdbRemoteError::TrailingBytes {
                count: frame.len() - consumed,
            });
        }
        Ok(Self {
            checksum: expected,
            payload,
        })
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub const fn checksum(&self) -> u8 {
        self.checksum
    }

    pub fn encode_frame(&self) -> Vec<u8> {
        let payload = encode_payload(&self.payload);
        let mut frame = Vec::with_capacity(payload.len() + 4);
        frame.push(PACKET_START_BYTE);
        frame.extend_from_slice(&payload);
        frame.push(CHECKSUM_SEPARATOR_BYTE);
        frame.push(encode_hex_nibble(self.checksum >> 4));
        frame.push(encode_hex_nibble(self.checksum & 0x0f));
        frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteCommand {
    DumpPageTable,
    QueryAttached {
        process_id: Option<u64>,
    },
    QueryCurrentThread,
    QuerySupported {
        features: Vec<GdbRemoteFeature>,
    },
    QuerySymbol,
    QueryMonitorCommand {
        command: Vec<u8>,
    },
    QueryStopReason,
    QueryResumeActions,
    QueryThreadInfo {
        query: GdbRemoteThreadInfoQuery,
    },
    QueryXferRead {
        request: GdbRemoteXferReadRequest,
    },
    Disconnect {
        request: GdbRemoteDisconnectRequest,
    },
    ReadMemory {
        address: u64,
        length: usize,
    },
    ReadRegisters,
    ReadRegister {
        number: u64,
    },
    Resume {
        kind: GdbRemoteResumeKind,
        signal: Option<u8>,
        address: Option<u64>,
    },
    ResumeActions {
        requests: Vec<GdbRemoteResumeRequest>,
    },
    SetThread {
        operation: GdbRemoteThreadOperation,
        thread: GdbRemoteThreadId,
    },
    StartNoAckMode,
    ThreadAlive {
        thread: GdbRemoteThreadId,
    },
    Trap {
        request: GdbRemoteTrapRequest,
    },
    WriteMemory {
        address: u64,
        bytes: Vec<u8>,
    },
    WriteRegisters {
        bytes: Vec<u8>,
    },
    WriteRegister {
        number: u64,
        bytes: Vec<u8>,
    },
    Unknown(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteAttachKind {
    Attached,
    Created,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GdbRemoteAckMode {
    Acknowledged,
    NoAck,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteSession {
    ack_mode: GdbRemoteAckMode,
    attach_kind: GdbRemoteAttachKind,
    response_config: GdbRemotePacketConfig,
    stub_features: Vec<GdbRemoteFeature>,
    gdb_features: Vec<GdbRemoteFeature>,
    stop_reply: GdbRemoteStopReply,
    register_bytes: GdbRemoteRegisterBytes,
    register_values: BTreeMap<u64, GdbRemoteRegisterValue>,
    memory_bytes: BTreeMap<u64, u8>,
    page_table_dump: Option<Vec<u8>>,
    xfer_features: BTreeMap<Vec<u8>, Vec<u8>>,
    continue_thread: GdbRemoteThreadId,
    current_thread_id: u64,
    general_thread: GdbRemoteThreadId,
    thread_ids: Vec<u64>,
    thread_info_index: usize,
    execution_control: GdbRemoteExecutionControl,
    last_resume_requests: Vec<GdbRemoteResumeRequest>,
    last_monitor_command: Option<Vec<u8>>,
    last_disconnect_request: Option<GdbRemoteDisconnectRequest>,
    last_trap_request: Option<GdbRemoteTrapRequest>,
    trap_patches: BTreeMap<GdbRemoteTrapPoint, Vec<u8>>,
    disconnected: bool,
    last_response: Option<GdbRemotePacket>,
    interrupt_requested: bool,
}

impl GdbRemoteSession {
    pub fn new(stub_features: Vec<GdbRemoteFeature>) -> Self {
        Self::with_response_config(stub_features, GdbRemotePacketConfig::default())
    }

    pub fn with_response_config(
        stub_features: Vec<GdbRemoteFeature>,
        response_config: GdbRemotePacketConfig,
    ) -> Self {
        Self {
            ack_mode: GdbRemoteAckMode::Acknowledged,
            attach_kind: GdbRemoteAttachKind::Attached,
            response_config,
            stub_features,
            gdb_features: Vec::new(),
            stop_reply: GdbRemoteStopReply::signal(0x05),
            register_bytes: GdbRemoteRegisterBytes::default(),
            register_values: BTreeMap::new(),
            memory_bytes: BTreeMap::new(),
            page_table_dump: None,
            xfer_features: BTreeMap::new(),
            continue_thread: GdbRemoteThreadId::Any,
            current_thread_id: 1,
            general_thread: GdbRemoteThreadId::Any,
            thread_ids: vec![1],
            thread_info_index: 0,
            execution_control: GdbRemoteExecutionControl::default(),
            last_resume_requests: Vec::new(),
            last_monitor_command: None,
            last_disconnect_request: None,
            last_trap_request: None,
            trap_patches: BTreeMap::new(),
            disconnected: false,
            last_response: None,
            interrupt_requested: false,
        }
    }

    pub const fn ack_mode(&self) -> GdbRemoteAckMode {
        self.ack_mode
    }

    pub const fn attach_kind(&self) -> GdbRemoteAttachKind {
        self.attach_kind
    }

    pub fn set_attach_kind(&mut self, attach_kind: GdbRemoteAttachKind) {
        self.attach_kind = attach_kind;
    }

    pub const fn response_config(&self) -> GdbRemotePacketConfig {
        self.response_config
    }

    pub const fn interrupt_requested(&self) -> bool {
        self.interrupt_requested
    }

    pub const fn is_disconnected(&self) -> bool {
        self.disconnected
    }

    pub const fn last_disconnect_request(&self) -> Option<&GdbRemoteDisconnectRequest> {
        self.last_disconnect_request.as_ref()
    }

    pub fn last_trap_request(&self) -> Option<&GdbRemoteTrapRequest> {
        self.last_trap_request.as_ref()
    }

    pub fn active_traps(&self) -> &[GdbRemoteTrapPoint] {
        self.execution_control.active_traps()
    }

    pub fn trap_patch(&self, point: GdbRemoteTrapPoint) -> Option<&[u8]> {
        self.trap_patches.get(&point).map(Vec::as_slice)
    }

    pub fn record_trap_patch(&mut self, point: GdbRemoteTrapPoint, bytes: Vec<u8>) {
        self.trap_patches.entry(point).or_insert(bytes);
    }

    pub fn remove_trap_patch(&mut self, point: GdbRemoteTrapPoint) -> Option<Vec<u8>> {
        self.trap_patches.remove(&point)
    }

    pub fn stub_features(&self) -> &[GdbRemoteFeature] {
        &self.stub_features
    }

    pub fn gdb_features(&self) -> &[GdbRemoteFeature] {
        &self.gdb_features
    }

    pub const fn stop_reply(&self) -> &GdbRemoteStopReply {
        &self.stop_reply
    }

    pub const fn continue_thread(&self) -> GdbRemoteThreadId {
        self.continue_thread
    }

    pub const fn current_thread_id(&self) -> u64 {
        self.current_thread_id
    }

    pub fn set_current_thread_id(&mut self, thread_id: u64) -> bool {
        if !self.thread_ids.contains(&thread_id) {
            return false;
        }
        self.current_thread_id = thread_id;
        true
    }

    pub const fn general_thread(&self) -> GdbRemoteThreadId {
        self.general_thread
    }

    pub fn thread_ids(&self) -> &[u64] {
        &self.thread_ids
    }

    pub const fn control_state(&self) -> &GdbRemoteControlState {
        self.execution_control.state()
    }

    pub const fn execution_control(&self) -> &GdbRemoteExecutionControl {
        &self.execution_control
    }

    pub fn last_resume_request(&self) -> Option<&GdbRemoteResumeRequest> {
        self.last_resume_requests.last()
    }

    pub fn last_resume_requests(&self) -> &[GdbRemoteResumeRequest] {
        &self.last_resume_requests
    }

    pub fn last_monitor_command(&self) -> Option<&[u8]> {
        self.last_monitor_command.as_deref()
    }

    pub fn set_thread_ids(&mut self, thread_ids: Vec<u64>) -> bool {
        if thread_ids.is_empty() || thread_ids.contains(&0) || has_duplicates(&thread_ids) {
            return false;
        }
        if !thread_ids.contains(&self.current_thread_id) {
            self.current_thread_id = thread_ids[0];
        }
        self.thread_ids = thread_ids;
        self.thread_info_index = 0;
        true
    }

    pub fn set_stop_reply(&mut self, stop_reply: GdbRemoteStopReply) {
        self.stop_reply = stop_reply;
        self.execution_control.set_stopped();
    }

    pub const fn register_bytes(&self) -> &GdbRemoteRegisterBytes {
        &self.register_bytes
    }

    pub fn set_register_bytes(&mut self, register_bytes: GdbRemoteRegisterBytes) {
        self.register_bytes = register_bytes;
    }

    pub fn register_value(&self, number: u64) -> Option<&GdbRemoteRegisterValue> {
        self.register_values.get(&number)
    }

    pub fn set_register_value(&mut self, number: u64, register_bytes: GdbRemoteRegisterBytes) {
        self.register_values
            .insert(number, GdbRemoteRegisterValue::Bytes(register_bytes));
    }

    pub fn cache_fixed_width_register_values(
        &mut self,
        first_number: u64,
        register_byte_len: usize,
        register_count: usize,
    ) -> bool {
        if register_byte_len == 0 || register_count == 0 {
            return false;
        }
        let Some(expected_len) = register_byte_len.checked_mul(register_count) else {
            return false;
        };
        if self.register_bytes.bytes().len() != expected_len {
            return false;
        }

        let mut values = Vec::with_capacity(register_count);
        for offset in 0..register_count {
            let Ok(register_offset) = u64::try_from(offset) else {
                return false;
            };
            let Some(number) = first_number.checked_add(register_offset) else {
                return false;
            };
            let start = offset * register_byte_len;
            let end = start + register_byte_len;
            values.push((
                number,
                GdbRemoteRegisterValue::Bytes(GdbRemoteRegisterBytes::new(
                    self.register_bytes.bytes()[start..end].to_vec(),
                )),
            ));
        }

        for (number, value) in values {
            self.register_values.insert(number, value);
        }
        true
    }

    pub fn set_register_unavailable(&mut self, number: u64, byte_len: usize) {
        self.register_values
            .insert(number, GdbRemoteRegisterValue::Unavailable { byte_len });
    }

    pub fn set_memory_bytes(&mut self, address: u64, bytes: Vec<u8>) {
        if let Some(addresses) = memory_addresses(address, bytes.len()) {
            for (address, byte) in addresses.into_iter().zip(bytes) {
                self.memory_bytes.insert(address, byte);
            }
        }
    }

    pub fn set_page_table_dump(&mut self, page_table_dump: Vec<u8>) {
        self.page_table_dump = Some(page_table_dump);
    }

    pub fn set_xfer_feature(&mut self, annex: Vec<u8>, content: Vec<u8>) -> bool {
        if annex.is_empty() {
            return false;
        }
        self.xfer_features.insert(annex, content);
        true
    }

    pub fn respond_with_payload(
        &mut self,
        payload: Vec<u8>,
    ) -> Result<Vec<GdbRemoteFrame>, GdbRemoteError> {
        self.packet_response(payload)
    }

    fn write_memory_bytes(&mut self, address: u64, bytes: &[u8]) -> bool {
        let Some(addresses) = memory_addresses(address, bytes.len()) else {
            return false;
        };

        for (address, byte) in addresses.into_iter().zip(bytes.iter().copied()) {
            self.memory_bytes.insert(address, byte);
        }
        true
    }

    pub fn handle_packet(
        &mut self,
        packet: &GdbRemotePacket,
    ) -> Result<Vec<GdbRemoteFrame>, GdbRemoteError> {
        if self.disconnected {
            return Ok(Vec::new());
        }

        let command = GdbRemoteCommand::parse(packet);

        match command {
            GdbRemoteCommand::DumpPageTable => self.packet_response(self.page_table_dump_payload()),
            GdbRemoteCommand::QueryAttached { .. } => {
                let payload = match self.attach_kind {
                    GdbRemoteAttachKind::Attached => b"1".to_vec(),
                    GdbRemoteAttachKind::Created => b"0".to_vec(),
                };
                self.packet_response(payload)
            }
            GdbRemoteCommand::QueryCurrentThread => {
                let mut payload = b"QC".to_vec();
                payload.extend_from_slice(&encode_hex_u64(self.current_thread_id));
                self.packet_response(payload)
            }
            GdbRemoteCommand::QuerySupported { features } => {
                self.gdb_features = features;
                self.packet_response(encode_supported_features(&self.stub_features))
            }
            GdbRemoteCommand::QuerySymbol => self.packet_response(b"OK".to_vec()),
            GdbRemoteCommand::QueryMonitorCommand { command } => {
                self.last_monitor_command = Some(command);
                Ok(self.ack_response())
            }
            GdbRemoteCommand::QueryStopReason => {
                self.packet_response(self.stop_reply.encode_payload())
            }
            GdbRemoteCommand::QueryResumeActions => {
                if self.supports_vcont() {
                    self.packet_response(b"vCont;c;C;s;S".to_vec())
                } else {
                    self.packet_response(Vec::new())
                }
            }
            GdbRemoteCommand::QueryThreadInfo { query } => {
                let payload = match query {
                    GdbRemoteThreadInfoQuery::First => {
                        self.thread_info_index = 0;
                        self.next_thread_info_payload()
                    }
                    GdbRemoteThreadInfoQuery::Subsequent => self.next_thread_info_payload(),
                };
                self.packet_response(payload)
            }
            GdbRemoteCommand::QueryXferRead { request } => {
                self.packet_response(self.xfer_read_payload(&request))
            }
            GdbRemoteCommand::Disconnect { request } => {
                self.last_disconnect_request = Some(request);
                self.disconnected = true;
                self.execution_control.set_disconnected();
                match request {
                    GdbRemoteDisconnectRequest::Terminate => Ok(self.ack_response()),
                    GdbRemoteDisconnectRequest::Detach { .. }
                    | GdbRemoteDisconnectRequest::TerminateProcess { .. } => {
                        self.packet_response(b"OK".to_vec())
                    }
                }
            }
            GdbRemoteCommand::ReadMemory { address, length } => {
                let payload = self
                    .read_memory_payload(address, length)
                    .unwrap_or_else(|| b"E01".to_vec());
                self.packet_response(payload)
            }
            GdbRemoteCommand::ReadRegisters => {
                self.packet_response(self.register_bytes.encode_payload())
            }
            GdbRemoteCommand::Resume {
                kind,
                signal,
                address,
            } => {
                self.last_resume_requests = vec![GdbRemoteResumeRequest::new(
                    kind,
                    signal,
                    address,
                    self.continue_thread,
                )];
                self.execution_control
                    .apply_resume_requests(self.last_resume_requests.clone());
                Ok(self.ack_response())
            }
            GdbRemoteCommand::ResumeActions { requests } => {
                if !self.supports_vcont() {
                    return self.packet_response(Vec::new());
                }
                self.last_resume_requests = requests;
                self.execution_control
                    .apply_resume_requests(self.last_resume_requests.clone());
                Ok(self.ack_response())
            }
            GdbRemoteCommand::SetThread { operation, thread } => {
                match operation {
                    GdbRemoteThreadOperation::Continue => self.continue_thread = thread,
                    GdbRemoteThreadOperation::General => self.general_thread = thread,
                }
                self.packet_response(b"OK".to_vec())
            }
            GdbRemoteCommand::ThreadAlive { thread } => {
                let payload = if self.is_thread_alive(thread) {
                    b"OK".to_vec()
                } else {
                    b"E01".to_vec()
                };
                self.packet_response(payload)
            }
            GdbRemoteCommand::Trap { request } => {
                self.apply_trap_request(request);
                self.packet_response(b"OK".to_vec())
            }
            GdbRemoteCommand::WriteRegisters { bytes } => {
                self.set_register_bytes(GdbRemoteRegisterBytes::new(bytes));
                self.packet_response(b"OK".to_vec())
            }
            GdbRemoteCommand::ReadRegister { number } => {
                let payload = self
                    .register_values
                    .get(&number)
                    .map(GdbRemoteRegisterValue::encode_payload)
                    .unwrap_or_else(|| b"E01".to_vec());
                self.packet_response(payload)
            }
            GdbRemoteCommand::WriteRegister { number, bytes } => {
                self.set_register_value(number, GdbRemoteRegisterBytes::new(bytes));
                self.packet_response(b"OK".to_vec())
            }
            GdbRemoteCommand::WriteMemory { address, bytes } => {
                let payload = if self.write_memory_bytes(address, &bytes) {
                    b"OK".to_vec()
                } else {
                    b"E01".to_vec()
                };
                self.packet_response(payload)
            }
            GdbRemoteCommand::StartNoAckMode => {
                if !self.supports_no_ack_mode() {
                    return self.packet_response(Vec::new());
                }
                let frames = self.packet_response(b"OK".to_vec())?;
                self.ack_mode = GdbRemoteAckMode::NoAck;
                Ok(frames)
            }
            GdbRemoteCommand::Unknown(_) => self.packet_response(Vec::new()),
        }
    }

    fn packet_response(&mut self, payload: Vec<u8>) -> Result<Vec<GdbRemoteFrame>, GdbRemoteError> {
        let packet = GdbRemotePacket::with_config(payload, self.response_config)?;
        self.last_response = Some(packet.clone());

        let mut frames = Vec::new();
        if self.ack_mode == GdbRemoteAckMode::Acknowledged {
            frames.push(GdbRemoteFrame::Ack);
        }
        frames.push(GdbRemoteFrame::Packet(packet));
        Ok(frames)
    }

    fn ack_response(&self) -> Vec<GdbRemoteFrame> {
        match self.ack_mode {
            GdbRemoteAckMode::Acknowledged => vec![GdbRemoteFrame::Ack],
            GdbRemoteAckMode::NoAck => Vec::new(),
        }
    }

    fn apply_trap_request(&mut self, request: GdbRemoteTrapRequest) {
        self.execution_control.apply_trap_request(request);
        self.last_trap_request = Some(request);
    }

    fn read_memory_payload(&self, address: u64, length: usize) -> Option<Vec<u8>> {
        if length.checked_mul(2)? > self.response_config.max_payload_bytes() {
            return None;
        }

        let mut bytes = Vec::with_capacity(length);
        for offset in 0..length {
            let address = address.checked_add(offset as u64)?;
            bytes.push(*self.memory_bytes.get(&address)?);
        }
        Some(encode_hex_bytes(&bytes))
    }

    fn page_table_dump_payload(&self) -> Vec<u8> {
        let Some(payload) = self.page_table_dump.as_ref() else {
            return b"E01".to_vec();
        };
        if payload.len() > self.response_config.max_payload_bytes() {
            return b"E01".to_vec();
        }
        payload.clone()
    }

    fn next_thread_info_payload(&mut self) -> Vec<u8> {
        if self.thread_info_index >= self.thread_ids.len() {
            self.thread_info_index = 0;
            return b"l".to_vec();
        }

        let mut payload = b"m".to_vec();
        let max_payload_bytes = self.response_config.max_payload_bytes();
        while self.thread_info_index < self.thread_ids.len() {
            let encoded = encode_hex_u64(self.thread_ids[self.thread_info_index]);
            let separator_len = usize::from(payload.len() > 1);
            if payload.len() + separator_len + encoded.len() > max_payload_bytes {
                if payload.len() == 1 {
                    payload.extend_from_slice(&encoded);
                    self.thread_info_index += 1;
                }
                break;
            }
            if separator_len > 0 {
                payload.push(b',');
            }
            payload.extend_from_slice(&encoded);
            self.thread_info_index += 1;
        }
        payload
    }

    fn xfer_read_payload(&self, request: &GdbRemoteXferReadRequest) -> Vec<u8> {
        if !self.supports_xfer_features() {
            return Vec::new();
        }

        let Some(content) = self.xfer_features.get(request.annex()) else {
            return b"E01".to_vec();
        };
        if request.offset() > content.len() {
            return b"E01".to_vec();
        }

        let available = content.len() - request.offset();
        let slice_len = request.length().min(available);
        let end = request.offset() + slice_len;
        let mut payload = Vec::with_capacity(slice_len + 1);
        if end < content.len() {
            payload.push(b'm');
        } else {
            payload.push(b'l');
        }
        payload.extend_from_slice(&content[request.offset()..end]);
        payload
    }

    fn supports_no_ack_mode(&self) -> bool {
        self.stub_features.iter().any(|feature| {
            feature.name() == b"QStartNoAckMode"
                && matches!(feature.value(), GdbRemoteFeatureValue::Supported)
        })
    }

    fn supports_vcont(&self) -> bool {
        self.stub_features.iter().any(|feature| {
            feature.name() == b"vContSupported"
                && matches!(feature.value(), GdbRemoteFeatureValue::Supported)
        })
    }

    fn supports_xfer_features(&self) -> bool {
        self.stub_features.iter().any(|feature| {
            feature.name() == b"qXfer:features:read"
                && matches!(feature.value(), GdbRemoteFeatureValue::Supported)
        })
    }

    fn is_thread_alive(&self, thread: GdbRemoteThreadId) -> bool {
        match thread {
            GdbRemoteThreadId::All | GdbRemoteThreadId::Any => !self.thread_ids.is_empty(),
            GdbRemoteThreadId::Id(thread_id) => self.thread_ids.contains(&thread_id),
        }
    }

    fn retransmit_last_response(&self) -> Vec<GdbRemoteFrame> {
        if self.ack_mode == GdbRemoteAckMode::NoAck {
            return Vec::new();
        }

        match &self.last_response {
            Some(packet) => vec![GdbRemoteFrame::Packet(packet.clone())],
            None => Vec::new(),
        }
    }

    pub fn handle_frame(
        &mut self,
        frame: &GdbRemoteFrame,
    ) -> Result<Vec<GdbRemoteFrame>, GdbRemoteError> {
        match frame {
            GdbRemoteFrame::Packet(packet) => self.handle_packet(packet),
            GdbRemoteFrame::Interrupt => {
                self.interrupt_requested = true;
                self.execution_control.set_interrupted();
                Ok(Vec::new())
            }
            GdbRemoteFrame::NegativeAck => Ok(self.retransmit_last_response()),
            GdbRemoteFrame::Ack | GdbRemoteFrame::Notification(_) => Ok(Vec::new()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteNotification {
    data: Vec<u8>,
    checksum: u8,
}

impl GdbRemoteNotification {
    pub fn new(data: Vec<u8>) -> Result<Self, GdbRemoteError> {
        Self::with_config(data, GdbRemotePacketConfig::default())
    }

    pub fn with_config(
        data: Vec<u8>,
        config: GdbRemotePacketConfig,
    ) -> Result<Self, GdbRemoteError> {
        validate_payload_len(data.len(), config)?;
        let checksum = checksum(&data);
        Ok(Self { data, checksum })
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub const fn checksum(&self) -> u8 {
        self.checksum
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteFrame {
    Ack,
    NegativeAck,
    Interrupt,
    Packet(GdbRemotePacket),
    Notification(GdbRemoteNotification),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteFrameParse {
    frame: GdbRemoteFrame,
    consumed_bytes: usize,
    skipped_bytes: Vec<u8>,
}

impl GdbRemoteFrameParse {
    fn new(frame: GdbRemoteFrame, consumed_bytes: usize, skipped_bytes: Vec<u8>) -> Self {
        Self {
            frame,
            consumed_bytes,
            skipped_bytes,
        }
    }

    pub const fn frame(&self) -> &GdbRemoteFrame {
        &self.frame
    }

    pub const fn consumed_bytes(&self) -> usize {
        self.consumed_bytes
    }

    pub fn skipped_bytes(&self) -> &[u8] {
        &self.skipped_bytes
    }
}

pub fn parse_gdb_remote_frame(input: &[u8]) -> Result<Option<GdbRemoteFrameParse>, GdbRemoteError> {
    parse_gdb_remote_frame_with_config(input, GdbRemotePacketConfig::default())
}

pub fn parse_gdb_remote_frame_with_config(
    input: &[u8],
    config: GdbRemotePacketConfig,
) -> Result<Option<GdbRemoteFrameParse>, GdbRemoteError> {
    let mut search_start = 0;
    loop {
        let Some(relative_position) = input[search_start..].iter().position(|byte| {
            matches!(
                *byte,
                ACK_BYTE
                    | NEGATIVE_ACK_BYTE
                    | INTERRUPT_BYTE
                    | PACKET_START_BYTE
                    | NOTIFICATION_START_BYTE
            )
        }) else {
            return Ok(None);
        };
        let position = search_start + relative_position;
        let skipped = input[..position].to_vec();
        match input[position] {
            ACK_BYTE => {
                return Ok(Some(GdbRemoteFrameParse::new(
                    GdbRemoteFrame::Ack,
                    position + 1,
                    skipped,
                )));
            }
            NEGATIVE_ACK_BYTE => {
                return Ok(Some(GdbRemoteFrameParse::new(
                    GdbRemoteFrame::NegativeAck,
                    position + 1,
                    skipped,
                )));
            }
            INTERRUPT_BYTE => {
                return Ok(Some(GdbRemoteFrameParse::new(
                    GdbRemoteFrame::Interrupt,
                    position + 1,
                    skipped,
                )));
            }
            PACKET_START_BYTE => {
                let (payload, expected, consumed) = parse_packet_parts(
                    &input[position..],
                    config,
                    GdbRemotePacketPayloadMode::Command,
                )?;
                let packet = GdbRemotePacket {
                    payload,
                    checksum: expected,
                };
                return Ok(Some(GdbRemoteFrameParse::new(
                    GdbRemoteFrame::Packet(packet),
                    position + consumed,
                    skipped,
                )));
            }
            NOTIFICATION_START_BYTE => match parse_notification_parts(&input[position..], config) {
                Ok((data, expected, consumed)) => {
                    let notification = GdbRemoteNotification {
                        data,
                        checksum: expected,
                    };
                    return Ok(Some(GdbRemoteFrameParse::new(
                        GdbRemoteFrame::Notification(notification),
                        position + consumed,
                        skipped,
                    )));
                }
                Err(error) => {
                    search_start = position + notification_recovery_len(&input[position..]);
                    if search_start >= input.len() {
                        return Ok(None);
                    }
                    if matches!(
                        error,
                        GdbRemoteError::MissingPacketStart
                            | GdbRemoteError::MissingChecksumSeparator
                            | GdbRemoteError::ShortChecksum
                            | GdbRemoteError::InvalidChecksumHex { .. }
                            | GdbRemoteError::ChecksumMismatch { .. }
                            | GdbRemoteError::PayloadTooLong { .. }
                    ) {
                        continue;
                    }
                    return Err(error);
                }
            },
            _ => unreachable!("frame marker predicate and match arms must agree"),
        }
    }
}

fn parse_packet_parts(
    frame: &[u8],
    config: GdbRemotePacketConfig,
    payload_mode: GdbRemotePacketPayloadMode,
) -> Result<(Vec<u8>, u8, usize), GdbRemoteError> {
    if frame.first() != Some(&PACKET_START_BYTE) {
        return Err(GdbRemoteError::MissingPacketStart);
    }

    let mut payload = Vec::new();
    let mut expected = 0u8;
    let mut escaped = false;
    let mut index = 1;

    while index < frame.len() {
        let byte = frame[index];
        if escaped {
            expected = expected.wrapping_add(byte);
            payload.push(byte ^ ESCAPE_XOR);
            validate_payload_len(payload.len(), config)?;
            escaped = false;
            index += 1;
            continue;
        }

        if byte == CHECKSUM_SEPARATOR_BYTE {
            if frame.len() < index + 3 {
                return Err(GdbRemoteError::ShortChecksum);
            }
            let actual = decode_checksum(frame[index + 1], frame[index + 2])?;
            if actual != expected {
                return Err(GdbRemoteError::ChecksumMismatch { expected, actual });
            }
            reject_legacy_sequence_id(&payload)?;
            let normalized_checksum = checksum(&encode_payload(&payload));
            return Ok((payload, normalized_checksum, index + 3));
        }

        if byte == RUN_LENGTH_BYTE && payload_mode.decodes_run_length() {
            if payload.is_empty() {
                return Err(GdbRemoteError::RunLengthWithoutPreviousByte);
            }
            expected = expected.wrapping_add(byte);
            index += 1;
            if index >= frame.len() || frame[index] == CHECKSUM_SEPARATOR_BYTE {
                return Err(GdbRemoteError::MissingRunLengthCount);
            }
            let repeat_byte = frame[index];
            let repeat_count = decode_run_length_count(repeat_byte)?;
            expected = expected.wrapping_add(repeat_byte);
            let previous = payload
                .last()
                .copied()
                .expect("run-length marker already checked preceding payload byte");
            validate_payload_len(payload.len() + repeat_count, config)?;
            payload.extend(std::iter::repeat_n(previous, repeat_count));
            index += 1;
            continue;
        }

        expected = expected.wrapping_add(byte);
        if byte == ESCAPE_BYTE {
            escaped = true;
        } else {
            payload.push(byte);
            validate_payload_len(payload.len(), config)?;
        }
        index += 1;
    }

    Err(GdbRemoteError::MissingChecksumSeparator)
}

fn parse_notification_parts(
    frame: &[u8],
    config: GdbRemotePacketConfig,
) -> Result<(Vec<u8>, u8, usize), GdbRemoteError> {
    if frame.first() != Some(&NOTIFICATION_START_BYTE) {
        return Err(GdbRemoteError::MissingPacketStart);
    }

    let mut data = Vec::new();
    let mut expected = 0u8;
    let mut index = 1;

    while index < frame.len() {
        let byte = frame[index];
        if byte == CHECKSUM_SEPARATOR_BYTE {
            if frame.len() < index + 3 {
                return Err(GdbRemoteError::ShortChecksum);
            }
            let actual = decode_checksum(frame[index + 1], frame[index + 2])?;
            if actual != expected {
                return Err(GdbRemoteError::ChecksumMismatch { expected, actual });
            }
            return Ok((data, expected, index + 3));
        }
        if matches!(byte, PACKET_START_BYTE | NOTIFICATION_START_BYTE) {
            return Err(GdbRemoteError::MissingChecksumSeparator);
        }

        expected = expected.wrapping_add(byte);
        data.push(byte);
        validate_payload_len(data.len(), config)?;
        index += 1;
    }

    Err(GdbRemoteError::MissingChecksumSeparator)
}

fn notification_recovery_len(frame: &[u8]) -> usize {
    for (index, byte) in frame.iter().enumerate().skip(1) {
        match *byte {
            PACKET_START_BYTE | NOTIFICATION_START_BYTE => return index,
            CHECKSUM_SEPARATOR_BYTE if frame.len() >= index + 3 => return index + 3,
            _ => {}
        }
    }
    1
}

fn decode_run_length_count(byte: u8) -> Result<usize, GdbRemoteError> {
    if byte < RUN_LENGTH_COUNT_BIAS + MIN_RUN_LENGTH_REPEAT_COUNT
        || byte == PACKET_START_BYTE
        || byte == CHECKSUM_SEPARATOR_BYTE
        || byte > MAX_RUN_LENGTH_COUNT_BYTE
    {
        return Err(GdbRemoteError::InvalidRunLengthCount { byte });
    }
    Ok((byte - RUN_LENGTH_COUNT_BIAS) as usize)
}

fn reject_legacy_sequence_id(payload: &[u8]) -> Result<(), GdbRemoteError> {
    if payload.len() >= 3
        && payload[2] == b':'
        && is_hex_digit(payload[0])
        && is_hex_digit(payload[1])
    {
        return Err(GdbRemoteError::LegacySequenceIdUnsupported);
    }
    Ok(())
}

fn validate_payload_len(len: usize, config: GdbRemotePacketConfig) -> Result<(), GdbRemoteError> {
    if len > config.max_payload_bytes() {
        return Err(GdbRemoteError::PayloadTooLong {
            len,
            max: config.max_payload_bytes(),
        });
    }
    Ok(())
}

fn checksum(payload: &[u8]) -> u8 {
    payload
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

fn encode_payload(payload: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(payload.len());
    for byte in payload {
        match *byte {
            PACKET_START_BYTE | CHECKSUM_SEPARATOR_BYTE | ESCAPE_BYTE | RUN_LENGTH_BYTE => {
                encoded.push(ESCAPE_BYTE);
                encoded.push(byte ^ ESCAPE_XOR);
            }
            _ => encoded.push(*byte),
        }
    }
    encoded
}

fn has_duplicates(values: &[u64]) -> bool {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).any(|pair| pair[0] == pair[1])
}
