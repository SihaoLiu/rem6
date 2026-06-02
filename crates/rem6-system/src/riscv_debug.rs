use rem6_debug::{
    GdbRemoteFeature, GdbRemoteFeatureValue, GdbRemoteRegisterBytes, GdbRemoteSession,
    DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
};
use rem6_isa_riscv::{Register, RiscvGdbTargetDescription, RiscvGdbXlen, RiscvHartState};

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

fn encode_register_value(xlen: RiscvGdbXlen, value: u64) -> Vec<u8> {
    match xlen {
        RiscvGdbXlen::Rv32 => (value as u32).to_le_bytes().to_vec(),
        RiscvGdbXlen::Rv64 => value.to_le_bytes().to_vec(),
    }
}

const fn byte_len(xlen: RiscvGdbXlen) -> usize {
    match xlen {
        RiscvGdbXlen::Rv32 => 4,
        RiscvGdbXlen::Rv64 => 8,
    }
}
