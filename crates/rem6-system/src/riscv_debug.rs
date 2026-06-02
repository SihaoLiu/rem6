use rem6_cpu::RiscvCore;
use rem6_debug::{
    GdbRemoteCommand, GdbRemoteError, GdbRemoteFeature, GdbRemoteFeatureValue, GdbRemoteFrame,
    GdbRemotePacket, GdbRemoteRegisterBytes, GdbRemoteSession,
    DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
};
use rem6_isa_riscv::{Register, RiscvGdbTargetDescription, RiscvGdbXlen, RiscvHartState};
use rem6_memory::Address;
use std::error::Error;
use std::fmt;

const RISCV_GDB_INTEGER_REGISTER_COUNT: u8 = 32;
const RISCV_GDB_PC_REGISTER: u64 = 32;

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
    let register_bytes = riscv_gdb_register_bytes(xlen, hart);

    session.set_register_bytes(GdbRemoteRegisterBytes::new(register_bytes.clone()));
    for (number, bytes) in riscv_gdb_single_register_bytes(xlen, hart) {
        session.set_register_value(number, GdbRemoteRegisterBytes::new(bytes));
    }

    session
}

pub fn riscv_gdb_remote_session_from_core(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
) -> GdbRemoteSession {
    riscv_gdb_remote_session_from_hart(xlen, &riscv_gdb_hart_snapshot_from_core(core))
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
    let command = GdbRemoteCommand::parse(packet);
    let mut updated_hart = hart.clone();
    let applies_register_write =
        apply_riscv_gdb_remote_register_write(xlen, &mut updated_hart, &command)?;
    let frames = session.handle_packet(packet)?;
    if applies_register_write {
        *hart = updated_hart;
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
            let register_byte_len = byte_len(xlen);
            for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
                let number = u64::from(register);
                let start = number as usize * register_byte_len;
                let end = start + register_byte_len;
                write_core_register_value(
                    xlen,
                    core,
                    number,
                    decode_register_value(&bytes[start..end]),
                );
            }

            let pc_start = RISCV_GDB_PC_REGISTER as usize * register_byte_len;
            let pc_end = pc_start + register_byte_len;
            write_core_register_value(
                xlen,
                core,
                RISCV_GDB_PC_REGISTER,
                decode_register_value(&bytes[pc_start..pc_end]),
            );
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
    let command = GdbRemoteCommand::parse(packet);
    let applies_register_write = validate_riscv_gdb_remote_register_write(xlen, &command)?;
    let frames = session.handle_packet(packet)?;
    if applies_register_write {
        apply_riscv_gdb_remote_core_register_write(xlen, core, &command)?;
    }
    Ok(frames)
}

fn riscv_gdb_register_bytes(xlen: RiscvGdbXlen, hart: &RiscvHartState) -> Vec<u8> {
    let mut bytes =
        Vec::with_capacity((RISCV_GDB_INTEGER_REGISTER_COUNT as usize + 1) * byte_len(xlen));
    for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
        bytes.extend_from_slice(&encode_register_value(
            xlen,
            hart.read(Register::new(register).unwrap()),
        ));
    }
    bytes.extend_from_slice(&encode_register_value(xlen, hart.pc()));
    bytes
}

fn riscv_gdb_single_register_bytes(
    xlen: RiscvGdbXlen,
    hart: &RiscvHartState,
) -> Vec<(u64, Vec<u8>)> {
    let mut registers = Vec::with_capacity(RISCV_GDB_INTEGER_REGISTER_COUNT as usize + 1);
    for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
        registers.push((
            u64::from(register),
            encode_register_value(xlen, hart.read(Register::new(register).unwrap())),
        ));
    }
    registers.push((
        RISCV_GDB_PC_REGISTER,
        encode_register_value(xlen, hart.pc()),
    ));
    registers
}

fn riscv_gdb_hart_snapshot_from_core(core: &RiscvCore) -> RiscvHartState {
    let mut hart = RiscvHartState::with_hart_id(core.pc().get(), u64::from(core.id().get()));
    for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
        let register = Register::new(register).unwrap();
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
    let register_byte_len = byte_len(xlen);
    for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
        let number = u64::from(register);
        let start = number as usize * register_byte_len;
        let end = start + register_byte_len;
        write_register_value(
            xlen,
            hart,
            number,
            decode_register_value(&bytes[start..end]),
        );
    }

    let pc_start = RISCV_GDB_PC_REGISTER as usize * register_byte_len;
    let pc_end = pc_start + register_byte_len;
    write_register_value(
        xlen,
        hart,
        RISCV_GDB_PC_REGISTER,
        decode_register_value(&bytes[pc_start..pc_end]),
    );
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
    let expected = (RISCV_GDB_INTEGER_REGISTER_COUNT as usize + 1) * byte_len(xlen);
    if bytes.len() != expected {
        return Err(RiscvGdbRegisterWriteError::InvalidRegisterSetBytes {
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn write_register_value(xlen: RiscvGdbXlen, hart: &mut RiscvHartState, number: u64, value: u64) {
    let value = match xlen {
        RiscvGdbXlen::Rv32 => value & u64::from(u32::MAX),
        RiscvGdbXlen::Rv64 => value,
    };

    if number == RISCV_GDB_PC_REGISTER {
        hart.set_pc(value);
    } else {
        hart.write(Register::new(number as u8).unwrap(), value);
    }
}

fn write_core_register_value(xlen: RiscvGdbXlen, core: &RiscvCore, number: u64, value: u64) {
    let value = match xlen {
        RiscvGdbXlen::Rv32 => value & u64::from(u32::MAX),
        RiscvGdbXlen::Rv64 => value,
    };

    if number == RISCV_GDB_PC_REGISTER {
        core.redirect_pc(Address::new(value));
    } else {
        core.write_register(Register::new(number as u8).unwrap(), value);
    }
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
