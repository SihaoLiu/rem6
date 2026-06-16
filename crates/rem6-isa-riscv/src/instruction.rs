use crate::{
    AtomicMemoryOp, FloatRegister, Immediate, MemoryWidth, Register, RiscvCounterCsr,
    RiscvFenceSet, RiscvFloatCsr, RiscvFloatRoundingMode, RiscvInterruptCsr, RiscvMachineTrapCsr,
    RiscvPrivilegeMode, RiscvPseudoOp, RiscvStatusCsr, RiscvSupervisorTrapCsr, RiscvTranslationCsr,
    VectorRegister,
};

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
    Mulw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Divw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Divuw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Remw {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Remuw {
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
    VectorSetVli {
        rd: Register,
        rs1: Register,
        vtype: u64,
    },
    VectorSetIvli {
        rd: Register,
        avl: u8,
        vtype: u64,
    },
    VectorSetVl {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    VectorAddVv {
        vd: VectorRegister,
        vs1: VectorRegister,
        vs2: VectorRegister,
    },
    VectorAddVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    VectorAddVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
    VectorSubVv {
        vd: VectorRegister,
        vs1: VectorRegister,
        vs2: VectorRegister,
    },
    VectorSubVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    VectorAndVv {
        vd: VectorRegister,
        vs1: VectorRegister,
        vs2: VectorRegister,
    },
    VectorAndVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    VectorAndVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
    VectorOrVv {
        vd: VectorRegister,
        vs1: VectorRegister,
        vs2: VectorRegister,
    },
    VectorOrVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    VectorOrVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
    VectorXorVv {
        vd: VectorRegister,
        vs1: VectorRegister,
        vs2: VectorRegister,
    },
    VectorXorVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    VectorXorVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
    Load {
        rd: Register,
        rs1: Register,
        offset: Immediate,
        width: MemoryWidth,
        signed: bool,
    },
    FloatLoad {
        rd: FloatRegister,
        rs1: Register,
        offset: Immediate,
        width: MemoryWidth,
    },
    Store {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
        width: MemoryWidth,
    },
    FloatStore {
        rs1: Register,
        rs2: FloatRegister,
        offset: Immediate,
        width: MemoryWidth,
    },
    FloatAddS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatSubS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatMulS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatDivS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatMultiplyAddS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatMultiplySubtractS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatNegativeMultiplySubtractS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatNegativeMultiplyAddS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatSqrtS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatSignInjectS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatSignInjectNegS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatSignInjectXorS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatMinS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatMaxS {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatLessOrEqualS {
        rd: Register,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatLessThanS {
        rd: Register,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatEqualS {
        rd: Register,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatClassS {
        rd: Register,
        rs1: FloatRegister,
    },
    FloatMoveXFromS {
        rd: Register,
        rs1: FloatRegister,
    },
    FloatMoveSFromX {
        rd: FloatRegister,
        rs1: Register,
    },
    FloatConvertSFromW {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertSFromWu {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertSFromL {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertSFromLu {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertWFromS {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertWuFromS {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertLFromS {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertLuFromS {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertSFromD {
        rd: FloatRegister,
        rs1: FloatRegister,
    },
    FloatAddD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatSubD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatMulD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatDivD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatMultiplyAddD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatMultiplySubtractD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatNegativeMultiplySubtractD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatNegativeMultiplyAddD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
        rs3: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatSqrtD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatSignInjectD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatSignInjectNegD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatSignInjectXorD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatMinD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatMaxD {
        rd: FloatRegister,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatLessOrEqualD {
        rd: Register,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatLessThanD {
        rd: Register,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatEqualD {
        rd: Register,
        rs1: FloatRegister,
        rs2: FloatRegister,
    },
    FloatClassD {
        rd: Register,
        rs1: FloatRegister,
    },
    FloatMoveXFromD {
        rd: Register,
        rs1: FloatRegister,
    },
    FloatMoveDFromX {
        rd: FloatRegister,
        rs1: Register,
    },
    FloatConvertDFromW {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertDFromWu {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertDFromL {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertDFromLu {
        rd: FloatRegister,
        rs1: Register,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertDFromS {
        rd: FloatRegister,
        rs1: FloatRegister,
    },
    FloatConvertWFromD {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertWuFromD {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertLFromD {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
    },
    FloatConvertLuFromD {
        rd: Register,
        rs1: FloatRegister,
        rounding_mode: RiscvFloatRoundingMode,
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
    WaitForInterrupt,
    SupervisorReturn,
    MachineReturn,
    SfenceVma {
        rs1: Register,
        rs2: Register,
    },
    Gem5PseudoOp {
        op: RiscvPseudoOp,
    },
    ReadMachineHartId {
        rd: Register,
    },
    ReadCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
    },
    ReadMachineCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
    },
    WriteCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
        rs1: Register,
    },
    SetCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
        rs1: Register,
    },
    ClearCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
        rs1: Register,
    },
    WriteCounterCsrImmediate {
        rd: Register,
        csr: RiscvCounterCsr,
        zimm: u8,
    },
    SetCounterCsrImmediate {
        rd: Register,
        csr: RiscvCounterCsr,
        zimm: u8,
    },
    ClearCounterCsrImmediate {
        rd: Register,
        csr: RiscvCounterCsr,
        zimm: u8,
    },
    ReadFloatCsr {
        rd: Register,
        csr: RiscvFloatCsr,
    },
    WriteFloatCsr {
        rd: Register,
        csr: RiscvFloatCsr,
        rs1: Register,
    },
    SetFloatCsr {
        rd: Register,
        csr: RiscvFloatCsr,
        rs1: Register,
    },
    ClearFloatCsr {
        rd: Register,
        csr: RiscvFloatCsr,
        rs1: Register,
    },
    WriteFloatCsrImmediate {
        rd: Register,
        csr: RiscvFloatCsr,
        zimm: u8,
    },
    SetFloatCsrImmediate {
        rd: Register,
        csr: RiscvFloatCsr,
        zimm: u8,
    },
    ClearFloatCsrImmediate {
        rd: Register,
        csr: RiscvFloatCsr,
        zimm: u8,
    },
    ReadStatusCsr {
        rd: Register,
        csr: RiscvStatusCsr,
    },
    WriteStatusCsr {
        rd: Register,
        csr: RiscvStatusCsr,
        rs1: Register,
    },
    SetStatusCsr {
        rd: Register,
        csr: RiscvStatusCsr,
        rs1: Register,
    },
    ClearStatusCsr {
        rd: Register,
        csr: RiscvStatusCsr,
        rs1: Register,
    },
    WriteStatusCsrImmediate {
        rd: Register,
        csr: RiscvStatusCsr,
        zimm: u8,
    },
    SetStatusCsrImmediate {
        rd: Register,
        csr: RiscvStatusCsr,
        zimm: u8,
    },
    ClearStatusCsrImmediate {
        rd: Register,
        csr: RiscvStatusCsr,
        zimm: u8,
    },
    ReadInterruptCsr {
        rd: Register,
        csr: RiscvInterruptCsr,
    },
    WriteInterruptCsr {
        rd: Register,
        csr: RiscvInterruptCsr,
        rs1: Register,
    },
    SetInterruptCsr {
        rd: Register,
        csr: RiscvInterruptCsr,
        rs1: Register,
    },
    ClearInterruptCsr {
        rd: Register,
        csr: RiscvInterruptCsr,
        rs1: Register,
    },
    WriteInterruptCsrImmediate {
        rd: Register,
        csr: RiscvInterruptCsr,
        zimm: u8,
    },
    SetInterruptCsrImmediate {
        rd: Register,
        csr: RiscvInterruptCsr,
        zimm: u8,
    },
    ClearInterruptCsrImmediate {
        rd: Register,
        csr: RiscvInterruptCsr,
        zimm: u8,
    },
    ReadMachineTrapCsr {
        rd: Register,
        csr: RiscvMachineTrapCsr,
    },
    WriteMachineTrapCsr {
        rd: Register,
        csr: RiscvMachineTrapCsr,
        rs1: Register,
    },
    SetMachineTrapCsr {
        rd: Register,
        csr: RiscvMachineTrapCsr,
        rs1: Register,
    },
    ClearMachineTrapCsr {
        rd: Register,
        csr: RiscvMachineTrapCsr,
        rs1: Register,
    },
    WriteMachineTrapCsrImmediate {
        rd: Register,
        csr: RiscvMachineTrapCsr,
        zimm: u8,
    },
    SetMachineTrapCsrImmediate {
        rd: Register,
        csr: RiscvMachineTrapCsr,
        zimm: u8,
    },
    ClearMachineTrapCsrImmediate {
        rd: Register,
        csr: RiscvMachineTrapCsr,
        zimm: u8,
    },
    ReadSupervisorTrapCsr {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
    },
    WriteSupervisorTrapCsr {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
        rs1: Register,
    },
    SetSupervisorTrapCsr {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
        rs1: Register,
    },
    ClearSupervisorTrapCsr {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
        rs1: Register,
    },
    WriteSupervisorTrapCsrImmediate {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
        zimm: u8,
    },
    SetSupervisorTrapCsrImmediate {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
        zimm: u8,
    },
    ClearSupervisorTrapCsrImmediate {
        rd: Register,
        csr: RiscvSupervisorTrapCsr,
        zimm: u8,
    },
    ReadTranslationCsr {
        rd: Register,
        csr: RiscvTranslationCsr,
    },
    WriteTranslationCsr {
        rd: Register,
        csr: RiscvTranslationCsr,
        rs1: Register,
    },
    SetTranslationCsr {
        rd: Register,
        csr: RiscvTranslationCsr,
        rs1: Register,
    },
    ClearTranslationCsr {
        rd: Register,
        csr: RiscvTranslationCsr,
        rs1: Register,
    },
    WriteTranslationCsrImmediate {
        rd: Register,
        csr: RiscvTranslationCsr,
        zimm: u8,
    },
    SetTranslationCsrImmediate {
        rd: Register,
        csr: RiscvTranslationCsr,
        zimm: u8,
    },
    ClearTranslationCsrImmediate {
        rd: Register,
        csr: RiscvTranslationCsr,
        zimm: u8,
    },
    Ecall,
    Ebreak,
}

impl RiscvInstruction {
    pub(crate) const fn required_csr_privilege(self) -> Option<RiscvPrivilegeMode> {
        match self {
            Self::ReadMachineHartId { .. } => Some(RiscvPrivilegeMode::Machine),
            Self::ReadCounterCsr { .. } => Some(RiscvPrivilegeMode::User),
            Self::ReadMachineCounterCsr { .. }
            | Self::WriteCounterCsr { .. }
            | Self::SetCounterCsr { .. }
            | Self::ClearCounterCsr { .. }
            | Self::WriteCounterCsrImmediate { .. }
            | Self::SetCounterCsrImmediate { .. }
            | Self::ClearCounterCsrImmediate { .. } => Some(RiscvPrivilegeMode::Machine),
            Self::ReadFloatCsr { csr, .. }
            | Self::WriteFloatCsr { csr, .. }
            | Self::SetFloatCsr { csr, .. }
            | Self::ClearFloatCsr { csr, .. }
            | Self::WriteFloatCsrImmediate { csr, .. }
            | Self::SetFloatCsrImmediate { csr, .. }
            | Self::ClearFloatCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadStatusCsr { csr, .. }
            | Self::WriteStatusCsr { csr, .. }
            | Self::SetStatusCsr { csr, .. }
            | Self::ClearStatusCsr { csr, .. }
            | Self::WriteStatusCsrImmediate { csr, .. }
            | Self::SetStatusCsrImmediate { csr, .. }
            | Self::ClearStatusCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadInterruptCsr { csr, .. }
            | Self::WriteInterruptCsr { csr, .. }
            | Self::SetInterruptCsr { csr, .. }
            | Self::ClearInterruptCsr { csr, .. }
            | Self::WriteInterruptCsrImmediate { csr, .. }
            | Self::SetInterruptCsrImmediate { csr, .. }
            | Self::ClearInterruptCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadMachineTrapCsr { csr, .. }
            | Self::WriteMachineTrapCsr { csr, .. }
            | Self::SetMachineTrapCsr { csr, .. }
            | Self::ClearMachineTrapCsr { csr, .. }
            | Self::WriteMachineTrapCsrImmediate { csr, .. }
            | Self::SetMachineTrapCsrImmediate { csr, .. }
            | Self::ClearMachineTrapCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadSupervisorTrapCsr { csr, .. }
            | Self::WriteSupervisorTrapCsr { csr, .. }
            | Self::SetSupervisorTrapCsr { csr, .. }
            | Self::ClearSupervisorTrapCsr { csr, .. }
            | Self::WriteSupervisorTrapCsrImmediate { csr, .. }
            | Self::SetSupervisorTrapCsrImmediate { csr, .. }
            | Self::ClearSupervisorTrapCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadTranslationCsr { csr, .. }
            | Self::WriteTranslationCsr { csr, .. }
            | Self::SetTranslationCsr { csr, .. }
            | Self::ClearTranslationCsr { csr, .. }
            | Self::WriteTranslationCsrImmediate { csr, .. }
            | Self::SetTranslationCsrImmediate { csr, .. }
            | Self::ClearTranslationCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            _ => None,
        }
    }
}

const fn required_csr_privilege(address: u16) -> RiscvPrivilegeMode {
    match (address >> 8) & 0b11 {
        0 => RiscvPrivilegeMode::User,
        1 => RiscvPrivilegeMode::Supervisor,
        _ => RiscvPrivilegeMode::Machine,
    }
}

pub(crate) fn csr_privilege_allowed(
    current: RiscvPrivilegeMode,
    required: RiscvPrivilegeMode,
) -> bool {
    privilege_rank(current) >= privilege_rank(required)
}

const fn privilege_rank(privilege: RiscvPrivilegeMode) -> u8 {
    match privilege {
        RiscvPrivilegeMode::User => 0,
        RiscvPrivilegeMode::Supervisor => 1,
        RiscvPrivilegeMode::Machine => 3,
    }
}
