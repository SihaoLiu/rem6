use crate::common::*;

#[test]
fn riscv_core_driver_executes_vfadd_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.0, -2.5, 0.25, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([2.0, 4.0, -0.75, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([0.0, 0.0, 0.0, 12.0]));
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
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(3, 0xd0));

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([3.0, 1.5, -0.5, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vv_e64_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.0f64.to_bits(), (-2.5f64).to_bits()]),
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
            vfadd_vv_type(2, 1, 3),
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
    assert_eq!(core.vector_config(), RiscvVectorConfig::new(2, 0xd8));

    assert_eq!(
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([3.0f64.to_bits(), 1.5f64.to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vv_e64_zero_identity() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.0f64.to_bits(), (-2.5f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([0.0f64.to_bits(), (-0.0f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfadd_vv_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([1.0f64.to_bits(), (-2.5f64).to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vv_e64_exact_zero_cancellation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.0f64.to_bits(), (-2.5f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([(-1.0f64).to_bits(), 2.5f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfadd_vv_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_u64_bits([0.0f64.to_bits(), 0.0f64.to_bits()])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vv_e64_round_down_cancellation_to_negative_zero() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_float_status(RiscvFloatStatus::new(0).with_frm(2));
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.0f64.to_bits(), (-2.5f64).to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([(-1.0f64).to_bits(), 2.5f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfadd_vv_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
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
fn riscv_core_driver_traps_vfadd_vv_e64_inexact_without_destination_write() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.0f64.to_bits(), 2.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([(f64::EPSILON / 2.0).to_bits(), 1.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfadd_vv_type(2, 1, 3),
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
fn riscv_core_driver_executes_vfadd_vv_exact_subnormal_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    let min_subnormal = f32::from_bits(0x0000_0001);
    let two_min_subnormal = f32::from_bits(0x0000_0002);
    core.write_vector_register(vreg(1), lanes_f32([min_subnormal, min_subnormal, 1.0, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([min_subnormal, 0.0, 0.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([8.0, 8.0, 8.0, 12.0]));
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
        drive_until_instruction(&core, store, &mut scheduler, &transport),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::AddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32([two_min_subnormal, min_subnormal, 1.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfadd_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfadd_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::AddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        1.5,
        [2.0, -4.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [3.5, -2.5, 1.75, 12.0],
    );
}

#[test]
fn riscv_core_driver_traps_vfadd_vf_with_unboxed_scalar_source() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_float_register(freg(1), 1.5f32.to_bits().into());
    core.write_vector_register(vreg(2), lanes_f32([2.0, -4.0, 0.25, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([9.0, 9.0, 9.0, 12.0]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfadd_vf_type(2, 1, 3),
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
        lanes_f32([9.0, 9.0, 9.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_executes_vfsub_vv_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(vreg(1), lanes_f32([1.0, 2.0, -3.0, 9.0]));
    core.write_vector_register(vreg(2), lanes_f32([5.0, -2.0, 4.0, 1.0]));
    core.write_vector_register(vreg(3), lanes_f32([0.0, 0.0, 0.0, 12.0]));
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
        lanes_f32([4.0, -4.0, 7.0, 12.0])
    );
}

#[test]
fn riscv_core_driver_traps_vfsub_vv_e64_without_destination_write() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 2);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(
        vreg(1),
        lanes_u64_bits([1.0f64.to_bits(), 2.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_u64_bits([0.5f64.to_bits(), 1.0f64.to_bits()]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_u64_bits([0xdead_beef_dead_beef, 12.0f64.to_bits()]),
    );
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vfsub_vv_type(2, 1, 3),
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
fn riscv_core_driver_executes_vfsub_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfsub_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::SubVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        1.5,
        [5.0, -2.0, 4.0, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [3.5, -3.5, 2.5, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfrsub_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes(
        vfrsub_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::ReverseSubVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        10.0,
        [2.0, -4.0, 0.25, 1.0],
        [0.0, 0.0, 0.0, 12.0],
        [8.0, 14.0, 9.75, 12.0],
    );
}

#[test]
fn riscv_core_driver_executes_vfsqrt_v_from_fetch_stream() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x4080_0000, 0x4110_0000, 0x8000_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x4000_0000, 0x4040_0000, 0x8000_0000, 0x40a0_0000],
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vfsqrt_v_for_exact_fractional_lanes() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x3e80_0000, 0x4010_0000, 0x4080_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x3f00_0000, 0x3fc0_0000, 0x4000_0000, 0x40a0_0000],
        0,
    );
}

#[test]
fn riscv_core_driver_vfsqrt_accrues_invalid_for_negative_and_signaling_nan() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0xc080_0000, 0x7f80_0001, 0x3f80_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x7fc0_0000, 0x7fc0_0000, 0x3f80_0000, 0x40a0_0000],
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_vfsqrt_quiet_nan_does_not_accrue_invalid() {
    assert_unary_fetch_stream_executes_bits(
        vfsqrt_v_type(2, 3),
        RiscvVectorFloatInstruction::SqrtV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x7fc0_1234, 0x7f80_0000, 0x0000_0000, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x7fc0_0000, 0x7f80_0000, 0x0000_0000, 0x40a0_0000],
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vfclass_v_from_fetch_stream() {
    assert_unary_fetch_stream_executes_bits(
        vfclass_v_type(2, 3),
        RiscvVectorFloatInstruction::ClassV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0xff80_0000, 0x8000_0001, 0x0000_0000, 0x7f80_0000],
        [0, 0, 0, 0xdead_beef],
        [0x0000_0001, 0x0000_0004, 0x0000_0010, 0xdead_beef],
        0,
    );
}

#[test]
fn riscv_core_driver_vfclass_v_classifies_nan_lanes_without_invalid_flag() {
    assert_unary_fetch_stream_executes_bits(
        vfclass_v_type(2, 3),
        RiscvVectorFloatInstruction::ClassV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x7f80_0001, 0x7fc0_1234, 0x7f80_0000, 0xffc0_5678],
        [0, 0, 0, 0xcafe_beef],
        [0x0000_0100, 0x0000_0200, 0x0000_0080, 0xcafe_beef],
        0,
    );
}

#[test]
fn riscv_core_driver_vfclass_v_preserves_existing_float_flags() {
    assert_unary_fetch_stream_executes_bits_with_float_status(
        vfclass_v_type(2, 3),
        RiscvVectorFloatInstruction::ClassV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0x7f80_0001, 0x7fc0_1234, 0x7f80_0000, 0xffc0_5678],
        [0, 0, 0, 0xcafe_beef],
        [0x0000_0100, 0x0000_0200, 0x0000_0080, 0xcafe_beef],
        RiscvFloatStatus::new(0).with_fflags(FLOAT_FLAG_INEXACT),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_traps_vfsqrt_v_for_inexact_finite_lane() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x4040_0000, 0x4080_0000, 0x3f80_0000, 0x40c0_0000]),
    );
    core.write_vector_register(
        vreg(3),
        lanes_f32_bits([0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x40a0_0000]),
    );
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(0xd0, 10, 5), vfsqrt_v_type(2, 3), 0x0010_0073],
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
        lanes_f32_bits([0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x40a0_0000])
    );
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_executes_vmfeq_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmfeq_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x4000_0000, 0x7fc0_1234, 0x4080_0000],
        [0x3f80_0000, 0x4040_0000, 0x7fc0_1234, 0x4080_0000],
        0b1111_1000,
        0b1111_1001,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmfeq_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmfeq_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x4000_0000, 0xc000_0000, 0x0000_0000, 0x4080_0000],
        0b1010_1000,
        0b1010_1001,
        0,
    );
}

#[test]
fn riscv_core_driver_vmfeq_accrues_invalid_for_signaling_nan_only() {
    assert_vv_mask_fetch_stream_executes(
        vmfeq_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        0b0101_1000,
        0b0101_1001,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_traps_vmfeq_vv_for_unsupported_element_width() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 1);
    core.set_machine_trap_vector(0x9000);
    core.write_vector_register(vreg(1), lanes_f32_bits([0x3f80_0000, 0, 0, 0]));
    core.write_vector_register(vreg(2), lanes_f32_bits([0x3f80_0000, 0, 0, 0]));
    core.write_vector_register(vreg(3), mask_bytes(0b1010_1010));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd8, 10, 5),
            vmfeq_vv_type(2, 1, 3),
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
    assert_eq!(core.read_vector_register(vreg(3)), mask_bytes(0b1010_1010));
    assert_eq!(core.float_status().fflags(), 0);
}

#[test]
fn riscv_core_driver_executes_vmfne_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmfne_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskNotEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x4000_0000, 0x7fc0_1234, 0x4080_0000],
        [0x3f80_0000, 0x4040_0000, 0x7fc0_1234, 0x40a0_0000],
        0b1111_1000,
        0b1111_1110,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmfne_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmfne_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskNotEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x4000_0000, 0xc000_0000, 0x0000_0000, 0x4080_0000],
        0b1010_1000,
        0b1010_1110,
        0,
    );
}

#[test]
fn riscv_core_driver_vmfne_accrues_invalid_for_signaling_nan_only() {
    assert_vv_mask_fetch_stream_executes(
        vmfne_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskNotEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        [0x3f80_0000, 0x7fc0_1234, 0x7f80_0001, 0x4080_0000],
        0b0101_1000,
        0b0101_1110,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_executes_vmfle_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmfle_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessEqualVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x4000_0000, 0x4040_0000, 0xbf80_0000, 0x0000_0000],
        [0x3f80_0000, 0x4040_0000, 0x0000_0000, 0x4080_0000],
        0b1111_0000,
        0b1111_0011,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmfle_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmfle_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x3f80_0000, 0x4000_0000, 0x4040_0000, 0xbf80_0000],
        0b1010_1000,
        0b1010_1011,
        0,
    );
}

#[test]
fn riscv_core_driver_vmfle_vf_accrues_invalid_for_quiet_nan_scalar() {
    assert_vf_mask_fetch_stream_executes(
        vmfle_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessEqualVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x7fc0_1234,
        [0x3f80_0000, 0x4000_0000, 0xbf80_0000, 0x4080_0000],
        0b1010_1111,
        0b1010_1000,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_executes_vmflt_vv_from_fetch_stream() {
    assert_vv_mask_fetch_stream_executes(
        vmflt_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x4000_0000, 0x4040_0000, 0xbf80_0000, 0x0000_0000],
        [0x3f80_0000, 0x4040_0000, 0xc080_0000, 0x4080_0000],
        0b1111_0000,
        0b1111_0101,
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vmflt_vf_from_fetch_stream() {
    assert_vf_mask_fetch_stream_executes(
        vmflt_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x4000_0000,
        [0x3f80_0000, 0x4000_0000, 0x4040_0000, 0xbf80_0000],
        0b1010_0110,
        0b1010_0001,
        0,
    );
}

#[test]
fn riscv_core_driver_vmflt_vf_accrues_invalid_for_quiet_nan_scalar() {
    assert_vf_mask_fetch_stream_executes(
        vmflt_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x7fc0_1234,
        [0x3f80_0000, 0x4000_0000, 0xbf80_0000, 0x4080_0000],
        0b1010_1111,
        0b1010_1000,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_vmflt_accrues_invalid_for_any_nan() {
    assert_vv_mask_fetch_stream_executes(
        vmflt_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaskLessThanVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x0000_0000, 0x0000_0000, 0x4000_0000, 0x4000_0000],
        [0x7fc0_1234, 0x7f80_0001, 0x3f80_0000, 0x4080_0000],
        0b0101_1011,
        0b0101_1100,
        FLOAT_FLAG_INVALID,
    );
}

#[test]
fn riscv_core_driver_executes_vfmin_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfmin_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MinVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0x3f80_0000, 0x0000_0000, 0x40a0_0000, 0],
        [0x4000_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x3f80_0000, 0x8000_0000, 0x40a0_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmin_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes_bits(
        vfmin_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MinVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x0000_0000,
        [0x3f80_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0x0000_0000, 0x8000_0000, 0x0000_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_vfmin_accrues_invalid_for_signaling_nan() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), 3);
    core.write_vector_register(
        vreg(1),
        lanes_f32_bits([0x7f80_0001, 0x40a0_0000, 0x3f80_0000, 0]),
    );
    core.write_vector_register(
        vreg(2),
        lanes_f32_bits([0x4080_0000, 0x7fc0_1234, 0x7f80_0001, 0x40c0_0000]),
    );
    core.write_vector_register(vreg(3), lanes_f32_bits([0, 0, 0, 0x40a0_0000]));
    let store = loaded_program_store(
        0x8000,
        &[
            vsetvli_type(0xd0, 10, 5),
            vfmin_vv_type(2, 1, 3),
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
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MinVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        })
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([0x4080_0000, 0x40a0_0000, 0x3f80_0000, 0x40a0_0000])
    );
    assert_eq!(core.float_status().fflags(), FLOAT_FLAG_INVALID);
}

#[test]
fn riscv_core_driver_executes_vfmax_vv_from_fetch_stream() {
    assert_vv_fetch_stream_executes_bits(
        vfmax_vv_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaxVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
        },
        [0xc040_0000, 0x0000_0000, 0x7fc0_5678, 0],
        [0xc000_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xc000_0000, 0x0000_0000, 0x7fc0_0000, 0x40a0_0000],
    );
}

#[test]
fn riscv_core_driver_executes_vfmax_vf_from_fetch_stream() {
    assert_vf_fetch_stream_executes_bits(
        vfmax_vf_type(2, 1, 3),
        RiscvVectorFloatInstruction::MaxVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
        },
        0x7fc0_5678,
        [0xc000_0000, 0x8000_0000, 0x7fc0_1234, 0x40c0_0000],
        [0, 0, 0, 0x40a0_0000],
        [0xc000_0000, 0x8000_0000, 0x7fc0_0000, 0x40a0_0000],
    );
}
