use crate::common::*;

#[test]
fn riscv_core_driver_executes_vfmul_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.5, -2.0, 0.5, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([2.0, 4.0, -8.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([0.0, 0.0, 0.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmul_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([3.0, -8.0, -4.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfmul_vv_e64_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.5f64.to_bits(), (-2.0f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([2.0f64.to_bits(), 4.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfmul_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([3.0f64.to_bits(), (-8.0f64).to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfmul_vv_e64_signed_zero_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([(-0.0f64).to_bits(), (-0.0f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([2.0f64.to_bits(), (-2.0f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfmul_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([(-0.0f64).to_bits(), 0.0f64.to_bits()])
    );
}

#[test]
fn riscv_core_driver_traps_vfmul_vv_e64_inexact_without_destination_write() {
    assert_vfmul_vv_e64_traps_without_destination_write(
        [(1.0f64 + f64::EPSILON).to_bits(), 2.0f64.to_bits()],
        [(1.0f64 + f64::EPSILON).to_bits(), 4.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfmul_vv_e64_late_inexact_without_partial_destination_write() {
    assert_vfmul_vv_e64_traps_without_destination_write(
        [2.0f64.to_bits(), (1.0f64 + f64::EPSILON).to_bits()],
        [4.0f64.to_bits(), (1.0f64 + f64::EPSILON).to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfmul_vv_e64_nonfinite_without_destination_write() {
    assert_vfmul_vv_e64_traps_without_destination_write(
        [f64::INFINITY.to_bits(), 2.0f64.to_bits()],
        [1.0f64.to_bits(), f64::NAN.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfmul_vv_e64_overflow_without_destination_write() {
    assert_vfmul_vv_e64_traps_without_destination_write(
        [f64::MAX.to_bits(), 2.0f64.to_bits()],
        [2.0f64.to_bits(), 4.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfmul_vv_e64_underflow_to_zero_without_destination_write() {
    assert_vfmul_vv_e64_traps_without_destination_write(
        [f64::MIN_POSITIVE.to_bits(), 2.0f64.to_bits()],
        [f64::MIN_POSITIVE.to_bits(), 4.0f64.to_bits()],
    );
}

fn assert_vfmul_vv_e64_traps_without_destination_write(lhs: [u64; 2], rhs: [u64; 2]) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(vreg(1), lanes_u64_bits(lhs));
    core.write_vector_register(vreg(2), lanes_u64_bits(rhs));
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfmul_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfmul_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfmul_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -2.0,
        [1.5, -2.0, 0.5, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [-3.0, 4.0, -1.0, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfmul_vf_e64_from_fetch_stream() {
    assert_vf_e64_fetch_stream_executes_bits(
        vfmul_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        (-2.0f64).to_bits(),
        [1.5f64.to_bits(), (-2.0f64).to_bits()],
        [0xdead_beef_dead_beef, 12.0f64.to_bits()],
        [(-3.0f64).to_bits(), 4.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfmul_vf_e64_nonfinite_scalar_without_destination_write() {
    assert_vf_e64_fetch_stream_traps_without_destination_write(
        vfmul_vf_type(2, 1, 3),
        f64::INFINITY.to_bits(),
        [1.5f64.to_bits(), (-2.0f64).to_bits()],
    );
}

#[test]
fn riscv_core_driver_executes_vfdiv_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfdiv_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::DivVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x4000_0000, 0xc080_0000, 0x3f00_0000, 0x4100_0000],
        [0x4100_0000, 0x4180_0000, 0xbf80_0000, 0x3f80_0000],
        [0, 0, 0, 0x4140_0000],
        [0x4080_0000, 0xc080_0000, 0xc000_0000, 0x4140_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfdiv_vv_e64_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([2.0f64.to_bits(), (-4.0f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([8.0f64.to_bits(), 8.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfdiv_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::DivVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([4.0f64.to_bits(), (-2.0f64).to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfdiv_vv_e64_signed_zero_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([2.0f64.to_bits(), (-2.0f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([(-0.0f64).to_bits(), 0.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfdiv_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::DivVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([(-0.0f64).to_bits(), (-0.0f64).to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfdiv_vv_e64_exact_subnormal_quotient_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([2.0f64.to_bits(), 4.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([
            f64::from_bits(0x0000_0000_0000_0002).to_bits(),
            f64::from_bits(0x0000_0000_0000_0004).to_bits(),
        ]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfdiv_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::DivVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([
            f64::from_bits(0x0000_0000_0000_0001).to_bits(),
            f64::from_bits(0x0000_0000_0000_0001).to_bits(),
        ])
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_inexact_without_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [3.0f64.to_bits(), 2.0f64.to_bits()],
        [1.0f64.to_bits(), 8.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_late_inexact_without_partial_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [2.0f64.to_bits(), 3.0f64.to_bits()],
        [8.0f64.to_bits(), 1.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_zero_divisor_without_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [0.0f64.to_bits(), 2.0f64.to_bits()],
        [1.0f64.to_bits(), 8.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_negative_zero_divisor_without_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [(-0.0f64).to_bits(), 2.0f64.to_bits()],
        [1.0f64.to_bits(), 8.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_nonfinite_without_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [1.0f64.to_bits(), 2.0f64.to_bits()],
        [f64::INFINITY.to_bits(), 8.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_overflow_without_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [0.5f64.to_bits(), 2.0f64.to_bits()],
        [f64::MAX.to_bits(), 8.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vv_e64_underflow_to_zero_without_destination_write() {
    assert_vfdiv_vv_e64_traps_without_destination_write(
        [f64::MAX.to_bits(), 2.0f64.to_bits()],
        [f64::MIN_POSITIVE.to_bits(), 8.0f64.to_bits()],
    );
}

fn assert_vfdiv_vv_e64_traps_without_destination_write(denominator: [u64; 2], numerator: [u64; 2]) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(vreg(1), lanes_u64_bits(denominator));
    core.write_vector_register(vreg(2), lanes_u64_bits(numerator));
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfdiv_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd8,
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfdiv_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfdiv_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::DivVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -2.0,
        [8.0, -4.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [-4.0, 2.0, -0.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfdiv_vf_e64_from_fetch_stream() {
    assert_vf_e64_fetch_stream_executes_bits(
        vfdiv_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::DivVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        (-2.0f64).to_bits(),
        [8.0f64.to_bits(), (-4.0f64).to_bits()],
        [0xdead_beef_dead_beef, 12.0f64.to_bits()],
        [(-4.0f64).to_bits(), 2.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_traps_vfdiv_vf_e64_zero_scalar_without_destination_write() {
    assert_vf_e64_fetch_stream_traps_without_destination_write(
        vfdiv_vf_type(2, 1, 3),
        0.0f64.to_bits(),
        [1.0f64.to_bits(), 8.0f64.to_bits()],
    );
}

#[test]
fn riscv_core_driver_executes_vfrdiv_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfrdiv_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::ReverseDivVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        8.0,
        [2.0, -4.0, 16.0, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [4.0, -2.0, 0.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfrdiv_vf_e64_from_fetch_stream() {
    assert_vf_e64_fetch_stream_executes_bits(
        vfrdiv_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::ReverseDivVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        8.0f64.to_bits(),
        [2.0f64.to_bits(), (-4.0f64).to_bits()],
        [0xdead_beef_dead_beef, 12.0f64.to_bits()],
        [4.0f64.to_bits(), (-2.0f64).to_bits()],
    );
}
