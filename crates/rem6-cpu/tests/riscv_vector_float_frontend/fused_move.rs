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
fn riscv_core_driver_traps_vfmul_vv_e64_without_destination_write() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_machine_trap_vector(0x9000);
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
fn riscv_core_driver_executes_vfmacc_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfmacc_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::ProductPlusAccumulator,
        },
        [0x3f80_0000, 0xc000_0000, 0x3f00_0000, 0x4110_0000],
        [0x4000_0000, 0x4080_0000, 0xc100_0000, 0x3f80_0000],
        [0x4120_0000, 0x3f80_0000, 0xc040_0000, 0x4140_0000],
        [0x4140_0000, 0xc0e0_0000, 0xc0e0_0000, 0x4140_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmacc_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes_bits(
        vfmacc_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::ProductPlusAccumulator,
        },
        0xc000_0000,
        [0x4040_0000, 0xc080_0000, 0x3e80_0000, 0x3f80_0000],
        [0x3f80_0000, 0xbf80_0000, 0x4100_0000, 0x4140_0000],
        [0xc0a0_0000, 0x40e0_0000, 0x40f0_0000, 0x4140_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmacc_vv_round_down_exact_cancellation_to_negative_zero() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(2));
    core.write_vector_register(
        vreg(1),
        lanes_f32_bits([0x3f80_0000, 0xbf80_0000, 0x4000_0000, 0x3f80_0000]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3f80_0000, 0x3f80_0000, 0x4040_0000, 0x3f80_0000]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0xbf80_0000, 0x3f80_0000, 0xc0c0_0000, 0x4110_0000]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmacc_vv_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::ProductPlusAccumulator,
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x8000_0000, 0x8000_0000, 0x8000_0000, 0x4110_0000])
    );
}

#[test]
fn riscv_core_driver_executes_vfnmacc_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfnmacc_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::NegativeProductMinusAccumulator,
        },
        [0x3f80_0000, 0xc000_0000, 0x3f00_0000, 0x4110_0000],
        [0x4000_0000, 0x4080_0000, 0xc100_0000, 0x3f80_0000],
        [0x4120_0000, 0x3f80_0000, 0xc040_0000, 0x4140_0000],
        [0xc140_0000, 0x40e0_0000, 0x40e0_0000, 0x4140_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfnmacc_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfnmacc_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::NegativeProductMinusAccumulator,
        },
        -2.0,
        [3.0, -4.0, 0.25, 1.0],
        [1.0, -1.0, 8.0, 12.0],
        [5.0, -7.0, -7.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfmsac_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfmsac_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::ProductMinusAccumulator,
        },
        [0x3f80_0000, 0xc000_0000, 0x3f00_0000, 0x4110_0000],
        [0x4000_0000, 0x4080_0000, 0xc100_0000, 0x3f80_0000],
        [0x4120_0000, 0x3f80_0000, 0xc040_0000, 0x4140_0000],
        [0xc100_0000, 0xc110_0000, 0xbf80_0000, 0x4140_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmsac_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfmsac_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::ProductMinusAccumulator,
        },
        -2.0,
        [3.0, -4.0, 0.25, 1.0],
        [1.0, -1.0, 8.0, 12.0],
        [-7.0, 9.0, -8.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfnmsac_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfnmsac_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::NegativeProductPlusAccumulator,
        },
        [0x3f80_0000, 0xc000_0000, 0x3f00_0000, 0x4110_0000],
        [0x4000_0000, 0x4080_0000, 0xc100_0000, 0x3f80_0000],
        [0x4120_0000, 0x3f80_0000, 0xc040_0000, 0x4140_0000],
        [0x4100_0000, 0x4110_0000, 0x3f80_0000, 0x4140_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfnmsac_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfnmsac_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MulAddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
            mode: RiscvVectorFloatMulAddMode::NegativeProductPlusAccumulator,
        },
        -2.0,
        [3.0, -4.0, 0.25, 1.0],
        [1.0, -1.0, 8.0, 12.0],
        [7.0, -9.0, 8.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_traps_vfnmsac_vf_with_unboxed_scalar_source() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_float_register(freg(1), u64::from(0x4000_0000_u32));
    core.write_vector_register(vreg(2), lanes_f32([3.0, -4.0, 0.25, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([1.0, -1.0, 8.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfnmsac_vf_type(2, 1, 3),
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([1.0, -1.0, 8.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_traps_vfmacc_vv_with_reserved_frm() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(5));
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(2), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 9.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmacc_vv_type(2, 1, 3),
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(core.machine_trap_cause(), 2);
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([9.0, 9.0, 9.0, 9.0])
    );
}

#[test]
fn riscv_core_driver_traps_vfmacc_vv_when_tiny_addend_result_is_not_exact_binary32() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(
        vreg(1),
        lanes_f32_bits([0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x4080_0000]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x0000_0001, 0x4000_0000, 0x4040_0000, 0x4080_0000]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x3f80_0000, 0x4110_0000, 0x4110_0000, 0x4110_0000]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmacc_vv_type(2, 1, 3),
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x3f80_0000, 0x4110_0000, 0x4110_0000, 0x4110_0000])
    );
}

#[test]
fn riscv_core_driver_traps_vfmacc_vv_when_result_is_not_exact_binary32() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(
        vreg(1),
        lanes_f32_bits([0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x4080_0000]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3380_0000, 0x4000_0000, 0x4040_0000, 0x4080_0000]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x3f80_0000, 0x4110_0000, 0x4110_0000, 0x4110_0000]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmacc_vv_type(2, 1, 3),
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x3f80_0000, 0x4110_0000, 0x4110_0000, 0x4110_0000])
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
fn riscv_core_driver_traps_vfdiv_vv_e64_without_destination_write() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_machine_trap_vector(0x9000);
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
fn riscv_core_driver_executes_vfsgnj_vf_with_reserved_frm_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(5));
    core.write_float_register(freg(1), f32_box_bits(0x8000_0000));
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3f80_0000, 0xc000_0000, 0x7fc0_1234, 0x4000_0000]),
    );
    core.write_vector_register(vreg(3), lanes_f32_bits([0, 0, 0, 0x40a0_0000]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsgnj_vf_type(2, 1, 3),
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
        drive_until_execution(&core, store, &mut scheduler, &transport),
        (
            RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::SignInjectVf {
                vd: vreg(3),
                fs1: freg(1),
                vs2: vreg(2),
            }),
            None,
        )
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0xbf80_0000, 0xc000_0000, 0xffc0_1234, 0x40a0_0000])
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnj_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfsgnj_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x8000_0000, 0x0000_0000, 0x8000_0000, 0],
        [0x3f80_0000, 0xc000_0000, 0x7fc0_1234, 0x4000_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xbf80_0000, 0x4000_0000, 0xffc0_1234, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjn_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfsgnjn_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectNegVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -0.0,
        [1.0, -2.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [1.0, 2.0, 0.25, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjn_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfsgnjn_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectNegVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x8000_0000, 0x0000_0000, 0x8000_0000, 0],
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x4000_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjx_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfsgnjx_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectXorVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        -0.0,
        [1.0, -2.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [-1.0, 2.0, -0.25, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsgnjx_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfsgnjx_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::SignInjectXorVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x8000_0000, 0x0000_0000, 0x8000_0000, 0],
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x4000_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xbf80_0000, 0xc000_0000, 0xbe80_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmv_v_f_from_fetch_stream() {
    assert_vf_fetch_stream_executes_bits(
        vfmv_v_f_type(1, 3),
        RiscvVectorFloatInstruction::MoveVf {
            vd: vreg(3),
            fs1: freg(1),
        },
        0x7fc0_1234,
        [0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x40c0_0000],
        [0, 0, 0, 0xdead_beef],
        [0x7fc0_1234, 0x7fc0_1234, 0x7fc0_1234, 0xdead_beef],
    );
}

#[test]
fn riscv_core_driver_vfmv_v_f_nan_boxes_scalar_source() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), u64::from(0x3f80_0000_u32));
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), vfmv_v_f_type(1, 3), 0x0010_0073],
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveVf {
            vd: vreg(3),
            fs1: freg(1),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x7fc0_0000, 0x7fc0_0000, 0x7fc0_0000, 0xdead_beef])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_executes_vfmerge_vfm_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box_bits(0x7fc0_1234));
    core.write_vector_register(vreg(0), mask_bytes(0b0000_0101));
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x40c0_0000]),
    );
    core.write_vector_register(vreg(3), lanes_f32_bits([0, 0, 0, 0xdead_beef]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmerge_vfm_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MergeVf {
            vd: vreg(3),
            vs2: vreg(2),
            fs1: freg(1),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x7fc0_1234, 0xc000_0000, 0x7fc0_1234, 0xdead_beef])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_vfmerge_vfm_nan_boxes_scalar_source() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), u64::from(0x3f80_0000_u32));
    core.write_vector_register(vreg(0), mask_bytes(0b0000_0111));
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x40c0_0000]),
    );
    core.write_vector_register(vreg(3), lanes_f32_bits([0, 0, 0, 0xdead_beef]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmerge_vfm_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MergeVf {
            vd: vreg(3),
            vs2: vreg(2),
            fs1: freg(1),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x7fc0_0000, 0x7fc0_0000, 0x7fc0_0000, 0xdead_beef])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_vfmerge_vfm_traps_when_mask_register_overlaps_destination() {
    assert_vfmerge_vfm_overlap_trap(vfmerge_vfm_type(2, 1, 0));
}

#[test]
fn riscv_core_driver_vfmerge_vfm_traps_when_mask_register_overlaps_source() {
    assert_vfmerge_vfm_overlap_trap(vfmerge_vfm_type(0, 1, 3));
}

fn assert_vfmerge_vfm_overlap_trap(instruction: u32) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box_bits(0x3f80_0000));
    core.write_vector_register(vreg(0), mask_bytes(0b0000_0101));
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x3f80_0000, 0xc000_0000, 0x3e80_0000, 0x40c0_0000]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), instruction, 0x0010_0073],
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(core.read_vector_register(vreg(0)), mask_bytes(0b0000_0101));
}

#[test]
fn riscv_core_driver_executes_vfmv_s_f_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box_bits(0x7fc0_1234));
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), vfmv_s_f_type(1, 3), 0x0010_0073],
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveSv {
            vd: vreg(3),
            fs1: freg(1),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x7fc0_1234, 0x1112_1314, 0x2122_2324, 0xdead_beef])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_vfmv_s_f_leaves_destination_when_vl_is_zero() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 0);
    core.write_float_register(freg(1), f32_box_bits(0x3f80_0000));
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), vfmv_s_f_type(1, 3), 0x0010_0073],
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveSv {
            vd: vreg(3),
            fs1: freg(1),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_executes_vfmv_f_s_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x7fc0_1234, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    core.write_float_register(freg(3), f32_box_bits(0x3f80_0000));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), vfmv_f_s_type(2, 3), 0x0010_0073],
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveFv {
            fd: freg(3),
            vs2: vreg(2),
        })
    );
    assert_eq!(core.read_float_register(freg(3)), f32_box_bits(0x7fc0_1234));
    assert_eq!(
        core.read_vector_register(vreg(2)),
        lanes_f32_bits([0x7fc0_1234, 0x1112_1314, 0x2122_2324, 0xdead_beef])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_vfmv_f_s_accepts_non_group_base_source_in_lmul2() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x7fc0_1234, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    core.write_float_register(freg(5), f32_box_bits(0x3f80_0000));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd1, 10, 5), vfmv_f_s_type(3, 5), 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd1,
        }
    );

    assert_eq!(
        drive_until_execution(&core, store, &mut scheduler, &transport),
        (
            RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveFv {
                fd: freg(5),
                vs2: vreg(3),
            }),
            None
        )
    );
    assert_eq!(core.read_float_register(freg(5)), f32_box_bits(0x7fc0_1234));
}

#[test]
fn riscv_core_driver_vfmv_s_f_accepts_non_group_base_destination_in_lmul2() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_float_register(freg(1), f32_box_bits(0x7fc0_1234));
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd1, 10, 5), vfmv_s_f_type(1, 3), 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd1,
        }
    );

    assert_eq!(
        drive_until_execution(&core, store, &mut scheduler, &transport),
        (
            RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveSv {
                vd: vreg(3),
                fs1: freg(1),
            }),
            None
        )
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x7fc0_1234, 0x1112_1314, 0x2122_2324, 0xdead_beef])
    );
}

#[test]
fn riscv_core_driver_vfmv_s_f_vl_zero_accepts_non_group_base_destination_in_lmul2() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 0);
    core.write_float_register(freg(1), f32_box_bits(0x3f80_0000));
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd1, 10, 5), vfmv_s_f_type(1, 3), 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd1,
        }
    );

    assert_eq!(
        drive_until_execution(&core, store, &mut scheduler, &transport),
        (
            RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveSv {
                vd: vreg(3),
                fs1: freg(1),
            }),
            None
        )
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x0102_0304, 0x1112_1314, 0x2122_2324, 0xdead_beef])
    );
}

#[test]
fn riscv_core_driver_traps_vfadd_vv_with_reserved_frm() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(5));
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(2), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 9.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vv_type(2, 1, 3),
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(core.machine_trap_cause(), 2);
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([9.0, 9.0, 9.0, 9.0])
    );
}

#[test]
fn riscv_core_driver_traps_vfsub_vv_when_add_sub_result_is_not_exact_binary32() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(2), lanes_f32([f32::MIN_POSITIVE, 2.0, 3.0, 4.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 9.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsub_vv_type(2, 1, 3),
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
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([9.0, 9.0, 9.0, 9.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfsub_vv_round_down_exact_cancellation_to_negative_zero() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(2));
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, 3.0, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([1.0, 2.0, 3.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([8.0, 8.0, 8.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfsub_vv_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::SubVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([-0.0, -0.0, -0.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_fetches_ahead_for_vfadd_vv_instruction() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.0, -2.5, 0.25, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([2.0, 4.0, -0.75, 1.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vv_type(2, 1, 3),
            0x0010_0073,
        ],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(vsetvli) = action else {
        panic!("expected vsetvli execution after vfadd.vv fetch-ahead");
    };
    assert_eq!(
        vsetvli.instruction(),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::FetchIssued { .. } = action else {
        panic!("expected ebreak fetch before retiring vfadd.vv");
    };
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(vfadd) = action else {
        panic!("expected vfadd.vv instruction to retire after successor fetch");
    };
    assert_eq!(
        vfadd.instruction(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
}
