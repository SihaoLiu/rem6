use rem6_cpu::{CpuId, RiscvCluster, RiscvCore};
use rem6_debug::{
    GdbRemoteAckMode, GdbRemoteCommand, GdbRemoteError, GdbRemoteFeature, GdbRemoteFeatureValue,
    GdbRemoteFrame, GdbRemotePacket, GdbRemoteRegisterBytes, GdbRemoteSession,
    DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
};
use rem6_isa_riscv::{Register, RiscvGdbTargetDescription, RiscvGdbXlen, RiscvHartState};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, MemoryError, MemoryRequest, MemoryRequestId,
    PartitionedMemoryStore,
};
use std::error::Error;
use std::fmt;

const RISCV_GDB_INTEGER_REGISTER_COUNT: u8 = 32;
const RISCV_GDB_PC_REGISTER: u64 = 32;
const RISCV_GDB_MEMORY_AGENT: AgentId = AgentId::new(u32::MAX - 1);

pub fn riscv_gdb_remote_session(xlen: RiscvGdbXlen) -> GdbRemoteSession {
    let mut session = GdbRemoteSession::new(vec![
        GdbRemoteFeature::new(
            b"PacketSize".to_vec(),
            GdbRemoteFeatureValue::Value(
                format!("{DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES:x}").into_bytes(),
            ),
        ),
        GdbRemoteFeature::new(
            b"qXfer:features:read".to_vec(),
            GdbRemoteFeatureValue::Supported,
        ),
    ]);

    for document in RiscvGdbTargetDescription::new(xlen).into_documents() {
        let (annex, content) = document.into_parts();
        let registered = session.set_xfer_feature(annex.as_bytes().to_vec(), content);
        debug_assert!(registered);
    }

    session
}

pub fn riscv_gdb_remote_session_from_hart(
    xlen: RiscvGdbXlen,
    hart: &RiscvHartState,
) -> GdbRemoteSession {
    let mut session = riscv_gdb_remote_session(xlen);
    sync_riscv_gdb_remote_session_from_hart(xlen, &mut session, hart);
    session
}

pub fn riscv_gdb_remote_session_from_core(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
) -> GdbRemoteSession {
    riscv_gdb_remote_session_from_hart(xlen, &riscv_gdb_hart_snapshot_from_core(core))
}

pub fn riscv_gdb_remote_session_from_cluster(
    xlen: RiscvGdbXlen,
    cluster: &RiscvCluster,
) -> Option<GdbRemoteSession> {
    let first_cpu = cluster.core_ids().into_iter().next()?;
    let first_core = cluster.core(first_cpu).ok()?;
    let mut session = riscv_gdb_remote_session_from_core(xlen, &first_core);
    sync_riscv_gdb_remote_threads_from_cluster(&mut session, cluster);
    Some(session)
}

pub fn sync_riscv_gdb_remote_threads_from_cluster(
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
) -> bool {
    session.set_thread_ids(
        cluster
            .core_ids()
            .into_iter()
            .map(riscv_gdb_remote_thread_id)
            .collect(),
    )
}

pub fn riscv_gdb_remote_thread_id(cpu: CpuId) -> u64 {
    u64::from(cpu.get()) + 1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvGdbRegisterWriteError {
    InvalidRegisterBytes {
        number: u64,
        expected: usize,
        actual: usize,
    },
    InvalidRegisterSetBytes {
        expected: usize,
        actual: usize,
    },
    UnsupportedRegister {
        number: u64,
    },
}

impl fmt::Display for RiscvGdbRegisterWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegisterBytes {
                number,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V GDB register {number} write has {actual} byte(s), expected {expected}"
            ),
            Self::InvalidRegisterSetBytes { expected, actual } => write!(
                formatter,
                "RISC-V GDB all-register write has {actual} byte(s), expected {expected}"
            ),
            Self::UnsupportedRegister { number } => {
                write!(formatter, "RISC-V GDB register {number} is unsupported")
            }
        }
    }
}

impl Error for RiscvGdbRegisterWriteError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvGdbRemotePacketError {
    Protocol(GdbRemoteError),
    RegisterWrite(RiscvGdbRegisterWriteError),
}

impl fmt::Display for RiscvGdbRemotePacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::RegisterWrite(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvGdbRemotePacketError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::RegisterWrite(error) => Some(error),
        }
    }
}

impl From<GdbRemoteError> for RiscvGdbRemotePacketError {
    fn from(error: GdbRemoteError) -> Self {
        Self::Protocol(error)
    }
}

impl From<RiscvGdbRegisterWriteError> for RiscvGdbRemotePacketError {
    fn from(error: RiscvGdbRegisterWriteError) -> Self {
        Self::RegisterWrite(error)
    }
}

pub fn apply_riscv_gdb_remote_register_write(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    command: &GdbRemoteCommand,
) -> Result<bool, RiscvGdbRegisterWriteError> {
    let applies = validate_riscv_gdb_remote_register_write(xlen, command)?;
    match command {
        GdbRemoteCommand::WriteRegister { number, bytes } => {
            apply_single_register_write(xlen, hart, *number, bytes)?;
        }
        GdbRemoteCommand::WriteRegisters { bytes } => {
            apply_all_register_write(xlen, hart, bytes)?;
        }
        _ => {}
    }
    Ok(applies)
}

pub fn handle_riscv_gdb_remote_packet(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    hart: &mut RiscvHartState,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    let command = GdbRemoteCommand::parse(packet);
    if reads_riscv_gdb_remote_registers(&command) {
        sync_riscv_gdb_remote_session_from_hart(xlen, session, hart);
    }

    let mut updated_hart = hart.clone();
    let applies_register_write =
        apply_riscv_gdb_remote_register_write(xlen, &mut updated_hart, &command)?;
    let frames = session.handle_packet(packet)?;
    if applies_register_write {
        *hart = updated_hart;
        sync_riscv_gdb_remote_session_from_hart(xlen, session, hart);
    }
    Ok(frames)
}

pub fn apply_riscv_gdb_remote_core_register_write(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
    command: &GdbRemoteCommand,
) -> Result<bool, RiscvGdbRegisterWriteError> {
    let applies = validate_riscv_gdb_remote_register_write(xlen, command)?;
    match command {
        GdbRemoteCommand::WriteRegister { number, bytes } => {
            write_core_register_value(xlen, core, *number, decode_register_value(bytes));
        }
        GdbRemoteCommand::WriteRegisters { bytes } => {
            for_each_decoded_register_value(xlen, bytes, |number, value| {
                write_core_register_value(xlen, core, number, value);
            });
        }
        _ => {}
    }
    Ok(applies)
}

pub fn handle_riscv_gdb_remote_core_packet(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    core: &RiscvCore,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    let command = GdbRemoteCommand::parse(packet);
    if reads_riscv_gdb_remote_registers(&command) {
        sync_riscv_gdb_remote_session_from_core(xlen, session, core);
    }

    let applies_register_write = validate_riscv_gdb_remote_register_write(xlen, &command)?;
    let frames = session.handle_packet(packet)?;
    if applies_register_write {
        apply_riscv_gdb_remote_core_register_write(xlen, core, &command)?;
        sync_riscv_gdb_remote_session_from_core(xlen, session, core);
    }
    Ok(frames)
}

pub fn handle_riscv_gdb_remote_memory_packet(
    session: &mut GdbRemoteSession,
    memory: &mut PartitionedMemoryStore,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    let command = GdbRemoteCommand::parse(packet);
    match &command {
        GdbRemoteCommand::ReadMemory { address, length } => {
            let Some(max_hex_len) = length.checked_mul(2) else {
                return gdb_remote_error_response(session);
            };
            if max_hex_len > session.response_config().max_payload_bytes() {
                return gdb_remote_error_response(session);
            }

            let bytes = match read_partitioned_memory_bytes(memory, *address, *length) {
                Ok(bytes) => bytes,
                Err(_) => return gdb_remote_error_response(session),
            };
            session.set_memory_bytes(*address, bytes);
            Ok(session.handle_packet(packet)?)
        }
        GdbRemoteCommand::WriteMemory { address, bytes } => {
            let mut updated_memory = memory.clone();
            if write_partitioned_memory_bytes(&mut updated_memory, *address, bytes).is_err() {
                return gdb_remote_error_response(session);
            }

            let frames = session.handle_packet(packet)?;
            *memory = updated_memory;
            Ok(frames)
        }
        _ => Ok(session.handle_packet(packet)?),
    }
}

fn sync_riscv_gdb_remote_session_from_hart(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    hart: &RiscvHartState,
) {
    session.set_register_bytes(GdbRemoteRegisterBytes::new(riscv_gdb_register_bytes(
        xlen, hart,
    )));
    for (number, bytes) in riscv_gdb_single_register_bytes(xlen, hart) {
        session.set_register_value(number, GdbRemoteRegisterBytes::new(bytes));
    }
}

fn sync_riscv_gdb_remote_session_from_core(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    core: &RiscvCore,
) {
    sync_riscv_gdb_remote_session_from_hart(
        xlen,
        session,
        &riscv_gdb_hart_snapshot_from_core(core),
    );
}

const fn reads_riscv_gdb_remote_registers(command: &GdbRemoteCommand) -> bool {
    matches!(
        command,
        GdbRemoteCommand::ReadRegisters | GdbRemoteCommand::ReadRegister { .. }
    )
}

fn read_partitioned_memory_bytes(
    memory: &PartitionedMemoryStore,
    address: u64,
    length: usize,
) -> Result<Vec<u8>, MemoryError> {
    if length == 0 {
        return Ok(Vec::new());
    }

    let start = Address::new(address);
    memory.validate_access_range(start, AccessSize::new(length as u64)?)?;

    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        let current = Address::new(address + bytes.len() as u64);
        let decode = memory.decode_detail(current)?;
        let layout = memory.partition_layout(decode.target())?;
        let line = layout.line_address(current);
        let line_offset = layout.line_offset(current) as usize;
        let chunk_len = (layout.bytes() as usize - line_offset).min(length - bytes.len());
        let line_data = memory.line_data(decode.target(), line)?;
        bytes.extend_from_slice(&line_data[line_offset..line_offset + chunk_len]);
    }
    Ok(bytes)
}

fn write_partitioned_memory_bytes(
    memory: &mut PartitionedMemoryStore,
    address: u64,
    bytes: &[u8],
) -> Result<(), MemoryError> {
    if bytes.is_empty() {
        return Ok(());
    }

    let start = Address::new(address);
    memory.validate_access_range(start, AccessSize::new(bytes.len() as u64)?)?;

    let mut offset = 0;
    while offset < bytes.len() {
        let current = Address::new(address + offset as u64);
        let decode = memory.decode_detail(current)?;
        let layout = memory.partition_layout(decode.target())?;
        let line_offset = layout.line_offset(current) as usize;
        let chunk_len = (layout.bytes() as usize - line_offset).min(bytes.len() - offset);
        let size = AccessSize::new(chunk_len as u64)?;
        let request = MemoryRequest::write(
            MemoryRequestId::new(RISCV_GDB_MEMORY_AGENT, offset as u64),
            current,
            size,
            bytes[offset..offset + chunk_len].to_vec(),
            ByteMask::full(size)?,
            layout,
        )?;
        memory.respond(&request)?;
        offset += chunk_len;
    }
    Ok(())
}

fn gdb_remote_error_response(
    session: &GdbRemoteSession,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    let mut frames = Vec::new();
    if session.ack_mode() == GdbRemoteAckMode::Acknowledged {
        frames.push(GdbRemoteFrame::Ack);
    }
    frames.push(GdbRemoteFrame::Packet(GdbRemotePacket::with_config(
        b"E01".to_vec(),
        session.response_config(),
    )?));
    Ok(frames)
}

fn riscv_gdb_register_bytes(xlen: RiscvGdbXlen, hart: &RiscvHartState) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(register_set_byte_len(xlen));
    for number in riscv_gdb_register_numbers() {
        bytes.extend_from_slice(&encode_register_value(
            xlen,
            read_hart_register_value(hart, number),
        ));
    }
    bytes
}

fn riscv_gdb_single_register_bytes(
    xlen: RiscvGdbXlen,
    hart: &RiscvHartState,
) -> Vec<(u64, Vec<u8>)> {
    let mut registers = Vec::with_capacity(register_count());
    for number in riscv_gdb_register_numbers() {
        registers.push((
            number,
            encode_register_value(xlen, read_hart_register_value(hart, number)),
        ));
    }
    registers
}

fn riscv_gdb_hart_snapshot_from_core(core: &RiscvCore) -> RiscvHartState {
    let mut hart = RiscvHartState::with_hart_id(core.pc().get(), u64::from(core.id().get()));
    for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
        let register = riscv_register(u64::from(register));
        hart.write(register, core.read_register(register));
    }
    hart
}

fn validate_riscv_gdb_remote_register_write(
    xlen: RiscvGdbXlen,
    command: &GdbRemoteCommand,
) -> Result<bool, RiscvGdbRegisterWriteError> {
    match command {
        GdbRemoteCommand::WriteRegister { number, bytes } => {
            validate_single_register_write(xlen, *number, bytes)?;
            Ok(true)
        }
        GdbRemoteCommand::WriteRegisters { bytes } => {
            validate_all_register_write(xlen, bytes)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_single_register_write(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    number: u64,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    validate_single_register_write(xlen, number, bytes)?;
    write_register_value(xlen, hart, number, decode_register_value(bytes));
    Ok(())
}

fn apply_all_register_write(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    validate_all_register_write(xlen, bytes)?;
    for_each_decoded_register_value(xlen, bytes, |number, value| {
        write_register_value(xlen, hart, number, value);
    });
    Ok(())
}

fn validate_single_register_write(
    xlen: RiscvGdbXlen,
    number: u64,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    if number > RISCV_GDB_PC_REGISTER {
        return Err(RiscvGdbRegisterWriteError::UnsupportedRegister { number });
    }

    let expected = byte_len(xlen);
    if bytes.len() != expected {
        return Err(RiscvGdbRegisterWriteError::InvalidRegisterBytes {
            number,
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn validate_all_register_write(
    xlen: RiscvGdbXlen,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    let expected = register_set_byte_len(xlen);
    if bytes.len() != expected {
        return Err(RiscvGdbRegisterWriteError::InvalidRegisterSetBytes {
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn write_register_value(xlen: RiscvGdbXlen, hart: &mut RiscvHartState, number: u64, value: u64) {
    let value = normalize_register_value(xlen, value);

    if number == RISCV_GDB_PC_REGISTER {
        hart.set_pc(value);
    } else {
        hart.write(riscv_register(number), value);
    }
}

fn write_core_register_value(xlen: RiscvGdbXlen, core: &RiscvCore, number: u64, value: u64) {
    let value = normalize_register_value(xlen, value);

    if number == RISCV_GDB_PC_REGISTER {
        core.redirect_pc(Address::new(value));
    } else {
        core.write_register(riscv_register(number), value);
    }
}

fn read_hart_register_value(hart: &RiscvHartState, number: u64) -> u64 {
    if number == RISCV_GDB_PC_REGISTER {
        hart.pc()
    } else {
        hart.read(riscv_register(number))
    }
}

fn for_each_decoded_register_value(
    xlen: RiscvGdbXlen,
    bytes: &[u8],
    mut visit: impl FnMut(u64, u64),
) {
    let register_byte_len = byte_len(xlen);
    for (index, number) in riscv_gdb_register_numbers().enumerate() {
        let start = index * register_byte_len;
        let end = start + register_byte_len;
        visit(number, decode_register_value(&bytes[start..end]));
    }
}

fn normalize_register_value(xlen: RiscvGdbXlen, value: u64) -> u64 {
    match xlen {
        RiscvGdbXlen::Rv32 => value & u64::from(u32::MAX),
        RiscvGdbXlen::Rv64 => value,
    }
}

fn riscv_register(number: u64) -> Register {
    Register::new(number as u8).unwrap()
}

fn riscv_gdb_register_numbers() -> impl Iterator<Item = u64> {
    (0..RISCV_GDB_INTEGER_REGISTER_COUNT)
        .map(u64::from)
        .chain(std::iter::once(RISCV_GDB_PC_REGISTER))
}

const fn register_count() -> usize {
    RISCV_GDB_INTEGER_REGISTER_COUNT as usize + 1
}

const fn register_set_byte_len(xlen: RiscvGdbXlen) -> usize {
    register_count() * byte_len(xlen)
}

fn encode_register_value(xlen: RiscvGdbXlen, value: u64) -> Vec<u8> {
    match xlen {
        RiscvGdbXlen::Rv32 => (value as u32).to_le_bytes().to_vec(),
        RiscvGdbXlen::Rv64 => value.to_le_bytes().to_vec(),
    }
}

fn decode_register_value(bytes: &[u8]) -> u64 {
    let mut raw = [0; 8];
    raw[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(raw)
}

const fn byte_len(xlen: RiscvGdbXlen) -> usize {
    match xlen {
        RiscvGdbXlen::Rv32 => 4,
        RiscvGdbXlen::Rv64 => 8,
    }
}
