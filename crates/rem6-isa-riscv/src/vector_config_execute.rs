use crate::{write_register, Register, RegisterWrite, RiscvHartState, RiscvVectorConfig};

pub(crate) fn execute_vsetvli(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    rd: Register,
    rs1: Register,
    vtype: u64,
) {
    let avl = vector_avl(hart, rd, rs1);
    write_vector_config(hart, writes, rd, vtype, avl);
}

pub(crate) fn execute_vsetivli(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    rd: Register,
    avl: u8,
    vtype: u64,
) {
    write_vector_config(hart, writes, rd, vtype, u64::from(avl));
}

pub(crate) fn execute_vsetvl(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    rd: Register,
    rs1: Register,
    rs2: Register,
) {
    let avl = vector_avl(hart, rd, rs1);
    write_vector_config(hart, writes, rd, hart.read(rs2), avl);
}

fn vector_avl(hart: &RiscvHartState, rd: Register, rs1: Register) -> u64 {
    if !rs1.is_zero() {
        return hart.read(rs1);
    }
    if rd.is_zero() {
        u64::from(hart.vector_config().vl())
    } else {
        u64::MAX
    }
}

fn write_vector_config(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    rd: Register,
    vtype: u64,
    avl: u64,
) {
    let vector_config = RiscvVectorConfig::from_avl(vtype, avl);
    hart.set_vector_config(vector_config);
    write_register(hart, writes, rd, u64::from(vector_config.vl()));
}
