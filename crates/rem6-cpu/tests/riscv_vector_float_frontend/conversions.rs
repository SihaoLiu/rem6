use crate::common::*;

#[test]
fn riscv_core_driver_executes_vfcvt_f_xu_v_from_fetch_stream() {
    assert_int_to_float_fetch_stream_executes(
        vfcvt_f_xu_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0, 1, 16_777_216, 4_294_967_295],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [
            0.0f32.to_bits(),
            1.0f32.to_bits(),
            16_777_216.0f32.to_bits(),
            0x1122_3344,
        ],
        RiscvFloatStatus::new(0),
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_f_x_v_from_fetch_stream() {
    assert_int_to_float_fetch_stream_executes(
        vfcvt_f_x_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [0, -2_i32 as u32, i32::MIN as u32, 9],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [
            0.0f32.to_bits(),
            (-2.0f32).to_bits(),
            (i32::MIN as f32).to_bits(),
            0x1122_3344,
        ],
        RiscvFloatStatus::new(0),
        0,
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_xu_f_v_round_down_with_inexact_flag() {
    assert_float_to_int_fetch_stream_executes(
        vfcvt_xu_f_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [
            1.75f32.to_bits(),
            2.0f32.to_bits(),
            3.875f32.to_bits(),
            9.0f32.to_bits(),
        ],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [1, 2, 3, 0x1122_3344],
        RiscvFloatStatus::new(0).with_frm(2),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_x_f_v_round_toward_zero_with_inexact_flag() {
    assert_float_to_int_fetch_stream_executes(
        vfcvt_x_f_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertSignedIntFromFloatV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [
            1.75f32.to_bits(),
            (-2.75f32).to_bits(),
            (-0.5f32).to_bits(),
            9.0f32.to_bits(),
        ],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [1, (-2_i32) as u32, 0, 0x1122_3344],
        RiscvFloatStatus::new(0).with_frm(1),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_rtz_xu_f_v_with_reserved_frm() {
    assert_float_to_int_fetch_stream_executes(
        vfcvt_rtz_xu_f_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatTowardZeroV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [
            1.75f32.to_bits(),
            2.0f32.to_bits(),
            3.875f32.to_bits(),
            9.0f32.to_bits(),
        ],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [1, 2, 3, 0x1122_3344],
        RiscvFloatStatus::new(0).with_frm(5),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_rtz_x_f_v_with_reserved_frm() {
    assert_float_to_int_fetch_stream_executes(
        vfcvt_rtz_x_f_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertSignedIntFromFloatTowardZeroV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [
            1.75f32.to_bits(),
            (-2.75f32).to_bits(),
            (-0.5f32).to_bits(),
            9.0f32.to_bits(),
        ],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [1, (-2_i32) as u32, 0, 0x1122_3344],
        RiscvFloatStatus::new(0).with_frm(5),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_traps_vfcvt_x_f_v_with_reserved_frm() {
    assert_conversion_traps_without_destination_write(
        vfcvt_x_f_v_type(2, 3),
        0xd0,
        3,
        RiscvFloatStatus::new(0).with_frm(5),
        [
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            3.0f32.to_bits(),
            4.0f32.to_bits(),
        ],
    );
}

#[test]
fn riscv_core_driver_traps_vfcvt_xu_f_v_for_unsupported_element_width() {
    assert_conversion_traps_without_destination_write(
        vfcvt_xu_f_v_type(2, 3),
        0xd8,
        1,
        RiscvFloatStatus::new(0),
        [
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            3.0f32.to_bits(),
            4.0f32.to_bits(),
        ],
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_f_xu_v_round_down_with_inexact_flag() {
    assert_int_to_float_fetch_stream_executes(
        vfcvt_f_xu_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [16_777_217, 1, 2, 9],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [
            16_777_216.0f32.to_bits(),
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            0x1122_3344,
        ],
        RiscvFloatStatus::new(0).with_frm(2),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_executes_vfcvt_f_x_v_round_up_with_inexact_flag() {
    assert_int_to_float_fetch_stream_executes(
        vfcvt_f_x_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [16_777_217, (-16_777_217_i32) as u32, 2, 9],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [
            16_777_218.0f32.to_bits(),
            (-16_777_216.0f32).to_bits(),
            2.0f32.to_bits(),
            0x1122_3344,
        ],
        RiscvFloatStatus::new(0).with_frm(3),
        FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_vfcvt_f_xu_v_preserves_existing_float_flags() {
    assert_int_to_float_fetch_stream_executes(
        vfcvt_f_xu_v_type(2, 3),
        RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        },
        [16_777_217, 1, 2, 9],
        [0xdead_beef, 0xdead_beef, 0xdead_beef, 0x1122_3344],
        [
            16_777_216.0f32.to_bits(),
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            0x1122_3344,
        ],
        RiscvFloatStatus::new(0)
            .with_frm(2)
            .with_fflags(FLOAT_FLAG_INVALID),
        FLOAT_FLAG_INVALID | FLOAT_FLAG_INEXACT,
    );
}

#[test]
fn riscv_core_driver_traps_vfcvt_f_x_v_with_reserved_frm() {
    assert_conversion_traps_without_destination_write(
        vfcvt_f_x_v_type(2, 3),
        0xd0,
        3,
        RiscvFloatStatus::new(0).with_frm(5),
        [0, 1, 2, 3],
    );
}

#[test]
fn riscv_core_driver_traps_vfcvt_f_xu_v_for_unsupported_element_width() {
    assert_conversion_traps_without_destination_write(
        vfcvt_f_xu_v_type(2, 3),
        0xd8,
        1,
        RiscvFloatStatus::new(0),
        [0, 1, 2, 3],
    );
}

fn assert_conversion_traps_without_destination_write(
    instruction: u32,
    vtype: u32,
    vl: u64,
    initial_float_status: RiscvFloatStatus,
    source: [u32; 4],
) {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(10), vl);
    core.set_machine_trap_vector(0x9000);
    core.set_float_status(initial_float_status);
    core.write_vector_register(vreg(2), lanes_f32_bits(source));
    core.write_vector_register(vreg(3), lanes_f32_bits([9, 9, 9, 12]));
    let store = loaded_program_store(
        0x8000,
        &[vsetvli_type(vtype, 10, 5), instruction, 0x0010_0073],
    );

    assert_eq!(
        drive_until_instruction(&core, store.clone(), &mut scheduler, &transport),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: u64::from(vtype),
        }
    );

    assert_eq!(
        drive_until_trap_kind(&core, store, &mut scheduler, &transport),
        Some(RiscvTrapKind::IllegalInstruction)
    );
    assert_eq!(
        core.read_vector_register(vreg(3)),
        lanes_f32_bits([9, 9, 9, 12])
    );
}
