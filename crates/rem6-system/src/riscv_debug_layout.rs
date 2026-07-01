use rem6_isa_riscv::{RiscvGdbXlen, RISCV_VECTOR_REGISTER_BYTES};

pub(super) const RISCV_GDB_INTEGER_REGISTER_COUNT: u8 = 32;
pub(super) const RISCV_GDB_PC_REGISTER: u64 = 32;
pub(super) const RISCV_GDB_FLOAT_REGISTER_BASE: u64 = 33;
pub(super) const RISCV_GDB_FLOAT_REGISTER_COUNT: u8 = 32;
pub(super) const RISCV_GDB_FLOAT_CSR_REGISTER_BASE: u64 = 66;
pub(super) const RISCV_GDB_FLOAT_CSR_REGISTER_COUNT: u8 = 3;
pub(super) const RISCV_GDB_FLOAT_PLACEHOLDER_REGISTER: u64 = 69;
pub(super) const RISCV_GDB_RV32_CSR_REGISTER_BASE: u64 = 70;
pub(super) const RISCV_GDB_RV64_CSR_REGISTER_BASE: u64 = 70;
pub(super) const RISCV_GDB_CSR_REGISTER_COUNT: u8 = 20;
pub(super) const RISCV_GDB_VECTOR_REGISTER_BASE: u64 = 90;
pub(super) const RISCV_GDB_VECTOR_REGISTER_COUNT: u8 = 32;
pub(super) const RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER: u64 = 122;
pub(super) const RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER: u64 = 123;
pub(super) const RISCV_GDB_COUNTER_CYCLE_REGISTER: u64 = 124;
pub(super) const RISCV_GDB_COUNTER_INSTRET_REGISTER: u64 = 125;
pub(super) const RISCV_GDB_COUNTER_TIME_REGISTER: u64 = 126;
pub(super) const RISCV_GDB_MACHINE_HART_ID_REGISTER: u64 = 127;
pub(super) const RISCV_GDB_MACHINE_VENDOR_ID_REGISTER: u64 = 128;
pub(super) const RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER: u64 = 129;
pub(super) const RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER: u64 = 130;
pub(super) const RISCV_GDB_MACHINE_ISA_REGISTER: u64 = 131;
pub(super) const RISCV_GDB_VECTOR_LENGTH_REGISTER: u64 = 132;
pub(super) const RISCV_GDB_VECTOR_TYPE_REGISTER: u64 = 133;
pub(super) const RISCV_GDB_VECTOR_LENGTH_BYTES_REGISTER: u64 = 134;
pub(super) const RISCV_GDB_SUPERVISOR_ENVIRONMENT_CONFIG_REGISTER: u64 = 135;
pub(super) const RISCV_GDB_PMP_CONFIG0_REGISTER: u64 = 136;
pub(super) const RISCV_GDB_MACHINE_COUNTER_CYCLE_REGISTER: u64 = 138;
pub(super) const RISCV_GDB_MACHINE_COUNTER_INSTRET_REGISTER: u64 = 139;
pub(super) const RISCV_GDB_PMP_CONFIG2_REGISTER: u64 = 147;
pub(super) const RISCV_GDB_PMP_ADDR_REGISTERS: [u64; 16] = [
    137, 140, 141, 142, 143, 144, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155,
];
pub(super) const RISCV_GDB_RV32_PMP_CONFIG1_REGISTER: u64 = 156;
pub(super) const RISCV_GDB_RV32_PMP_CONFIG3_REGISTER: u64 = 157;
pub(super) const RISCV_GDB_RV64_SUPERVISOR_COUNTER_ENABLE_REGISTER: u64 = 156;
pub(super) const RISCV_GDB_RV64_MACHINE_COUNTER_ENABLE_REGISTER: u64 = 157;
pub(super) const RISCV_GDB_RV64_MACHINE_ENVIRONMENT_CONFIG_REGISTER: u64 = 158;
pub(super) const RISCV_GDB_RV64_MACHINE_CONFIG_POINTER_REGISTER: u64 = 159;
pub(super) const RISCV_GDB_RV64_MACHINE_COUNTER_INHIBIT_REGISTER: u64 = 160;
pub(super) const RISCV_GDB_RV32_SUPERVISOR_COUNTER_ENABLE_REGISTER: u64 = 158;
pub(super) const RISCV_GDB_RV32_MACHINE_COUNTER_ENABLE_REGISTER: u64 = 159;
pub(super) const RISCV_GDB_RV32_STATUS_HIGH_REGISTER: u64 = 160;
pub(super) const RISCV_GDB_RV32_COUNTER_CYCLE_HIGH_REGISTER: u64 = 161;
pub(super) const RISCV_GDB_RV32_COUNTER_TIME_HIGH_REGISTER: u64 = 162;
pub(super) const RISCV_GDB_RV32_COUNTER_INSTRET_HIGH_REGISTER: u64 = 163;
pub(super) const RISCV_GDB_RV32_MACHINE_COUNTER_CYCLE_HIGH_REGISTER: u64 = 164;
pub(super) const RISCV_GDB_RV32_MACHINE_COUNTER_INSTRET_HIGH_REGISTER: u64 = 165;
pub(super) const RISCV_GDB_RV32_MACHINE_CONFIG_POINTER_REGISTER: u64 = 166;
pub(super) const RISCV_GDB_RV32_MACHINE_COUNTER_INHIBIT_REGISTER: u64 = 167;
pub(super) fn riscv_gdb_register_numbers(xlen: RiscvGdbXlen) -> impl Iterator<Item = u64> {
    let float_count = u64::from(RISCV_GDB_FLOAT_REGISTER_COUNT);
    let float_csr_count = u64::from(RISCV_GDB_FLOAT_CSR_REGISTER_COUNT);
    let csr_count = u64::from(RISCV_GDB_CSR_REGISTER_COUNT);
    (0..RISCV_GDB_INTEGER_REGISTER_COUNT)
        .map(u64::from)
        .chain(std::iter::once(RISCV_GDB_PC_REGISTER))
        .chain(RISCV_GDB_FLOAT_REGISTER_BASE..RISCV_GDB_FLOAT_REGISTER_BASE + float_count)
        .chain(
            RISCV_GDB_FLOAT_CSR_REGISTER_BASE..RISCV_GDB_FLOAT_CSR_REGISTER_BASE + float_csr_count,
        )
        .chain(std::iter::once(RISCV_GDB_FLOAT_PLACEHOLDER_REGISTER))
        .chain(riscv_gdb_csr_register_base(xlen)..riscv_gdb_csr_register_base(xlen) + csr_count)
        .chain(
            RISCV_GDB_VECTOR_REGISTER_BASE
                ..RISCV_GDB_VECTOR_REGISTER_BASE + u64::from(RISCV_GDB_VECTOR_REGISTER_COUNT),
        )
        .chain([
            RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER,
            RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER,
            RISCV_GDB_COUNTER_CYCLE_REGISTER,
            RISCV_GDB_COUNTER_INSTRET_REGISTER,
            RISCV_GDB_COUNTER_TIME_REGISTER,
            RISCV_GDB_MACHINE_HART_ID_REGISTER,
            RISCV_GDB_MACHINE_VENDOR_ID_REGISTER,
            RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER,
            RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER,
            RISCV_GDB_MACHINE_ISA_REGISTER,
            RISCV_GDB_VECTOR_LENGTH_REGISTER,
            RISCV_GDB_VECTOR_TYPE_REGISTER,
            RISCV_GDB_VECTOR_LENGTH_BYTES_REGISTER,
            RISCV_GDB_SUPERVISOR_ENVIRONMENT_CONFIG_REGISTER,
            RISCV_GDB_PMP_CONFIG0_REGISTER,
            RISCV_GDB_PMP_ADDR_REGISTERS[0],
            RISCV_GDB_MACHINE_COUNTER_CYCLE_REGISTER,
            RISCV_GDB_MACHINE_COUNTER_INSTRET_REGISTER,
        ])
        .chain(RISCV_GDB_PMP_ADDR_REGISTERS[1..8].iter().copied())
        .chain(std::iter::once(RISCV_GDB_PMP_CONFIG2_REGISTER))
        .chain(RISCV_GDB_PMP_ADDR_REGISTERS[8..].iter().copied())
        .chain((xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_PMP_CONFIG1_REGISTER))
        .chain((xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_PMP_CONFIG3_REGISTER))
        .chain(std::iter::once(
            riscv_gdb_supervisor_counter_enable_register(xlen),
        ))
        .chain(std::iter::once(riscv_gdb_machine_counter_enable_register(
            xlen,
        )))
        .chain(
            (xlen == RiscvGdbXlen::Rv64)
                .then_some(RISCV_GDB_RV64_MACHINE_ENVIRONMENT_CONFIG_REGISTER),
        )
        .chain(
            (xlen == RiscvGdbXlen::Rv64).then_some(RISCV_GDB_RV64_MACHINE_CONFIG_POINTER_REGISTER),
        )
        .chain(
            (xlen == RiscvGdbXlen::Rv64).then_some(RISCV_GDB_RV64_MACHINE_COUNTER_INHIBIT_REGISTER),
        )
        .chain((xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_STATUS_HIGH_REGISTER))
        .chain((xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_COUNTER_CYCLE_HIGH_REGISTER))
        .chain((xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_COUNTER_TIME_HIGH_REGISTER))
        .chain((xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_COUNTER_INSTRET_HIGH_REGISTER))
        .chain(
            (xlen == RiscvGdbXlen::Rv32)
                .then_some(RISCV_GDB_RV32_MACHINE_COUNTER_CYCLE_HIGH_REGISTER),
        )
        .chain(
            (xlen == RiscvGdbXlen::Rv32)
                .then_some(RISCV_GDB_RV32_MACHINE_COUNTER_INSTRET_HIGH_REGISTER),
        )
        .chain(
            (xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_MACHINE_CONFIG_POINTER_REGISTER),
        )
        .chain(
            (xlen == RiscvGdbXlen::Rv32).then_some(RISCV_GDB_RV32_MACHINE_COUNTER_INHIBIT_REGISTER),
        )
}

pub(super) fn register_count(xlen: RiscvGdbXlen) -> usize {
    riscv_gdb_register_numbers(xlen).count()
}

pub(super) fn register_set_byte_len(xlen: RiscvGdbXlen) -> usize {
    riscv_gdb_register_numbers(xlen)
        .map(|number| register_byte_len(xlen, number))
        .sum()
}

pub(super) fn riscv_gdb_register_number_is_supported(xlen: RiscvGdbXlen, number: u64) -> bool {
    number <= RISCV_GDB_PC_REGISTER
        || is_riscv_gdb_float_register(number)
        || is_riscv_gdb_float_csr_register(number)
        || is_riscv_gdb_float_placeholder_register(number)
        || is_riscv_gdb_csr_register(xlen, number)
        || is_riscv_gdb_vector_register(number)
}

pub(super) const fn is_riscv_gdb_float_register(number: u64) -> bool {
    number >= RISCV_GDB_FLOAT_REGISTER_BASE
        && number < RISCV_GDB_FLOAT_REGISTER_BASE + RISCV_GDB_FLOAT_REGISTER_COUNT as u64
}

pub(super) const fn is_riscv_gdb_float_csr_register(number: u64) -> bool {
    number >= RISCV_GDB_FLOAT_CSR_REGISTER_BASE
        && number < RISCV_GDB_FLOAT_CSR_REGISTER_BASE + RISCV_GDB_FLOAT_CSR_REGISTER_COUNT as u64
}

pub(super) const fn is_riscv_gdb_float_placeholder_register(number: u64) -> bool {
    number == RISCV_GDB_FLOAT_PLACEHOLDER_REGISTER
}

pub(super) const fn is_riscv_gdb_vector_register(number: u64) -> bool {
    number >= RISCV_GDB_VECTOR_REGISTER_BASE
        && number < RISCV_GDB_VECTOR_REGISTER_BASE + RISCV_GDB_VECTOR_REGISTER_COUNT as u64
}

pub(super) const fn riscv_gdb_csr_register_base(xlen: RiscvGdbXlen) -> u64 {
    match xlen {
        RiscvGdbXlen::Rv32 => RISCV_GDB_RV32_CSR_REGISTER_BASE,
        RiscvGdbXlen::Rv64 => RISCV_GDB_RV64_CSR_REGISTER_BASE,
    }
}

pub(super) fn riscv_gdb_supervisor_counter_enable_register(xlen: RiscvGdbXlen) -> u64 {
    match xlen {
        RiscvGdbXlen::Rv32 => RISCV_GDB_RV32_SUPERVISOR_COUNTER_ENABLE_REGISTER,
        RiscvGdbXlen::Rv64 => RISCV_GDB_RV64_SUPERVISOR_COUNTER_ENABLE_REGISTER,
    }
}

pub(super) fn riscv_gdb_machine_counter_enable_register(xlen: RiscvGdbXlen) -> u64 {
    match xlen {
        RiscvGdbXlen::Rv32 => RISCV_GDB_RV32_MACHINE_COUNTER_ENABLE_REGISTER,
        RiscvGdbXlen::Rv64 => RISCV_GDB_RV64_MACHINE_COUNTER_ENABLE_REGISTER,
    }
}

pub(super) fn is_riscv_gdb_csr_register(xlen: RiscvGdbXlen, number: u64) -> bool {
    (number >= riscv_gdb_csr_register_base(xlen)
        && number < riscv_gdb_csr_register_base(xlen) + RISCV_GDB_CSR_REGISTER_COUNT as u64)
        || number == RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER
        || number == RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER
        || number == RISCV_GDB_COUNTER_CYCLE_REGISTER
        || number == RISCV_GDB_COUNTER_INSTRET_REGISTER
        || number == RISCV_GDB_COUNTER_TIME_REGISTER
        || number == RISCV_GDB_MACHINE_HART_ID_REGISTER
        || number == RISCV_GDB_MACHINE_VENDOR_ID_REGISTER
        || number == RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER
        || number == RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER
        || number == RISCV_GDB_MACHINE_ISA_REGISTER
        || number == RISCV_GDB_VECTOR_LENGTH_REGISTER
        || number == RISCV_GDB_VECTOR_TYPE_REGISTER
        || number == RISCV_GDB_VECTOR_LENGTH_BYTES_REGISTER
        || number == RISCV_GDB_SUPERVISOR_ENVIRONMENT_CONFIG_REGISTER
        || number == RISCV_GDB_PMP_CONFIG0_REGISTER
        || number == RISCV_GDB_PMP_ADDR_REGISTERS[0]
        || (number >= RISCV_GDB_PMP_ADDR_REGISTERS[1] && number <= RISCV_GDB_PMP_ADDR_REGISTERS[7])
        || number == RISCV_GDB_PMP_CONFIG2_REGISTER
        || (number >= RISCV_GDB_PMP_ADDR_REGISTERS[8] && number <= RISCV_GDB_PMP_ADDR_REGISTERS[15])
        || (xlen == RiscvGdbXlen::Rv32
            && (number == RISCV_GDB_RV32_PMP_CONFIG1_REGISTER
                || number == RISCV_GDB_RV32_PMP_CONFIG3_REGISTER))
        || number == riscv_gdb_supervisor_counter_enable_register(xlen)
        || number == riscv_gdb_machine_counter_enable_register(xlen)
        || (xlen == RiscvGdbXlen::Rv64
            && number == RISCV_GDB_RV64_MACHINE_ENVIRONMENT_CONFIG_REGISTER)
        || (xlen == RiscvGdbXlen::Rv64 && number == RISCV_GDB_RV64_MACHINE_CONFIG_POINTER_REGISTER)
        || (xlen == RiscvGdbXlen::Rv64 && number == RISCV_GDB_RV64_MACHINE_COUNTER_INHIBIT_REGISTER)
        || (xlen == RiscvGdbXlen::Rv32 && number == RISCV_GDB_RV32_STATUS_HIGH_REGISTER)
        || (xlen == RiscvGdbXlen::Rv32
            && (number == RISCV_GDB_RV32_COUNTER_CYCLE_HIGH_REGISTER
                || number == RISCV_GDB_RV32_COUNTER_TIME_HIGH_REGISTER
                || number == RISCV_GDB_RV32_COUNTER_INSTRET_HIGH_REGISTER
                || number == RISCV_GDB_RV32_MACHINE_COUNTER_CYCLE_HIGH_REGISTER
                || number == RISCV_GDB_RV32_MACHINE_COUNTER_INSTRET_HIGH_REGISTER
                || number == RISCV_GDB_RV32_MACHINE_CONFIG_POINTER_REGISTER
                || number == RISCV_GDB_RV32_MACHINE_COUNTER_INHIBIT_REGISTER))
        || number == RISCV_GDB_MACHINE_COUNTER_CYCLE_REGISTER
        || number == RISCV_GDB_MACHINE_COUNTER_INSTRET_REGISTER
}

pub(super) fn encode_register_value(xlen: RiscvGdbXlen, number: u64, value: u64) -> Vec<u8> {
    if register_byte_len(xlen, number) == 4 {
        (value as u32).to_le_bytes().to_vec()
    } else {
        value.to_le_bytes().to_vec()
    }
}

pub(super) fn decode_register_value(bytes: &[u8]) -> u64 {
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

pub(super) const fn riscv_gdb_xlen_bits(xlen: RiscvGdbXlen) -> u8 {
    match xlen {
        RiscvGdbXlen::Rv32 => 32,
        RiscvGdbXlen::Rv64 => 64,
    }
}

pub(super) const fn register_byte_len(xlen: RiscvGdbXlen, number: u64) -> usize {
    if is_riscv_gdb_float_register(number) {
        8
    } else if is_riscv_gdb_vector_register(number) {
        RISCV_VECTOR_REGISTER_BYTES
    } else if is_riscv_gdb_float_csr_register(number)
        || is_riscv_gdb_float_placeholder_register(number)
    {
        4
    } else {
        byte_len(xlen)
    }
}
