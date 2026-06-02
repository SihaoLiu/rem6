use crate::{AtomicMemoryOp, Immediate, MemoryWidth, Register, RiscvCounterCsr, RiscvFenceSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvInstruction {
    Lui {
        rd: Register,
        imm: Immediate,
    },
    Auipc {
        rd: Register,
        imm: Immediate,
    },
    Addi {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Slti {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Sltiu {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Xori {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Ori {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Andi {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Slli {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Srli {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Srai {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Addiw {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Slliw {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Srliw {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Sraiw {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Add {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sub {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sll {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Slt {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sltu {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Xor {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Srl {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sra {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Or {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    And {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Mul {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Mulh {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Mulhsu {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Mulhu {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Div {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Divu {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Rem {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Remu {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Addw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Subw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sllw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Srlw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sraw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Beq {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bne {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Blt {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bge {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bltu {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bgeu {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Jal {
        rd: Register,
        offset: Immediate,
    },
    Jalr {
        rd: Register,
        rs1: Register,
        offset: Immediate,
    },
    Load {
        rd: Register,
        rs1: Register,
        offset: Immediate,
        width: MemoryWidth,
        signed: bool,
    },
    Store {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
        width: MemoryWidth,
    },
    LoadReserved {
        rd: Register,
        rs1: Register,
        width: MemoryWidth,
        acquire: bool,
        release: bool,
    },
    StoreConditional {
        rd: Register,
        rs1: Register,
        rs2: Register,
        width: MemoryWidth,
        acquire: bool,
        release: bool,
    },
    AtomicMemory {
        rd: Register,
        rs1: Register,
        rs2: Register,
        width: MemoryWidth,
        op: AtomicMemoryOp,
        acquire: bool,
        release: bool,
    },
    Fence {
        predecessor: RiscvFenceSet,
        successor: RiscvFenceSet,
        mode: u8,
    },
    FenceI,
    ReadMachineHartId {
        rd: Register,
    },
    ReadCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
    },
    Ecall,
    Ebreak,
}
