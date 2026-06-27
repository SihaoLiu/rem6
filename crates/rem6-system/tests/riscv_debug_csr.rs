use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_debug::{GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{
    RiscvGdbXlen, RiscvHartState, RiscvStatusCsr, RiscvStatusWord, RiscvVectorConfig,
    RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_system::{
    handle_riscv_gdb_remote_core_packet, handle_riscv_gdb_remote_packet,
    riscv_gdb_remote_session_from_core, riscv_gdb_remote_session_from_hart,
};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

#[test]
fn riscv_gdb_remote_packet_handler_reads_and_writes_advertised_rv64_csr_registers() {
    let mut hart = RiscvHartState::with_hart_id(0x1000, 9);
    hart.set_status(RiscvStatusWord::new(0x0008_0000));
    hart.set_supervisor_trap_vector(0x0123_4567_89ab_cdef);
    hart.set_supervisor_scratch(0x0102_0304_0506_0708);
    hart.set_supervisor_exception_pc(0x1122_3344_5566_7788);
    hart.set_supervisor_trap_cause(0x8877_6655_4433_2211);
    hart.set_supervisor_trap_value(0xfedc_ba98_7654_3210);
    hart.set_translation_satp(0x1111_2222_3333_4444);
    hart.set_machine_interrupt_delegation(0x0000_0000_0000_0aaa);
    hart.set_machine_interrupt_enable(0x1010_2020_3030_4040);
    hart.set_machine_trap_vector(0x0204_0608_0a0c_0e10);
    hart.set_machine_scratch(0x0f0e_0d0c_0b0a_0908);
    hart.set_machine_interrupt_pending(0x1111_3333_5555_7777);
    hart.set_supervisor_environment_config(0x0101_2020_3030_4040);
    let mut vector_fixed_point =
        RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundToOdd);
    vector_fixed_point.write_vxsat_bit(true);
    hart.set_vector_fixed_point(vector_fixed_point);
    hart.set_vector_config(RiscvVectorConfig::new(12, 0xd0));
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p46".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000080000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p4b".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"1032547698badcfe",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p4c".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"4444333322221111",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p52".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"08090a0b0c0d0e0f",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p7f".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0900000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p80".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p81".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p82".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p83".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"2d11140000000080",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p84".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0c00000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p85".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"d000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p86".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"1000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p87".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"4040303020200101",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P83=0000000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p83".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"2d11140000000080",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P7f=aa00000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p7f".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0900000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p57".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0100000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p58".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0300000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p59".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0700000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p7a".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p7b".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"2202000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P47=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_trap_vector(), 0x1122_3344_5566_7788);
    assert_eq!(RiscvStatusCsr::Sstatus.read(hart.status()), 0x0008_0000);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P48=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_scratch(), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P4c=8899aabbccddeeff".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.translation_satp(), 0xffee_ddcc_bbaa_9988);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P52=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.machine_scratch(), 0x1122_3344_5566_7788);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P58=0200000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P57=0000000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert!(!hart.vector_fixed_point().vxsat());
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P59=0500000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown,
    );
    assert!(hart.vector_fixed_point().vxsat());
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P7a=8808000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.machine_interrupt_enable(), 0x1010_2020_3030_48c8);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P7b=aa0a000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.machine_interrupt_pending(), 0x1111_3333_5555_7fff);
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P84=0500000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(5, 0xd0));
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P85=c000000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(5, 0xc0));
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P86=2000000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(5, 0xc0));
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P87=8877665544332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(hart.supervisor_environment_config(), 0x1122_3344_5566_7788);

    let registers = packet_payload(
        handle_riscv_gdb_remote_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &mut hart,
            &GdbRemotePacket::new(b"g".to_vec()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(registers.len(), rv64_register_hex_offset(158));
    assert_eq!(&registers[rv64_register_hex_range(70)], b"0000080000000000");
    assert_eq!(&registers[rv64_register_hex_range(71)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(72)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(75)], b"1032547698badcfe");
    assert_eq!(&registers[rv64_register_hex_range(76)], b"8899aabbccddeeff");
    assert_eq!(&registers[rv64_register_hex_range(80)], b"c848303020201010");
    assert_eq!(&registers[rv64_register_hex_range(81)], b"100e0c0a08060402");
    assert_eq!(&registers[rv64_register_hex_range(82)], b"8877665544332211");
    assert_eq!(&registers[rv64_register_hex_range(86)], b"ff7f555533331111");
    assert_eq!(&registers[rv64_register_hex_range(87)], b"0100000000000000");
    assert_eq!(&registers[rv64_register_hex_range(88)], b"0200000000000000");
    assert_eq!(&registers[rv64_register_hex_range(89)], b"0500000000000000");
    assert_eq!(
        &registers[rv64_register_hex_range(122)],
        b"8808000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(123)],
        b"aa0a000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(124)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(125)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(126)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(127)],
        b"0900000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(128)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(129)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(130)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(131)],
        b"2d11140000000080"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(132)],
        b"0500000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(133)],
        b"c000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(134)],
        b"1000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(135)],
        b"8877665544332211"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(136)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(137)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(138)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(139)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(140)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(141)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(142)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(143)],
        b"0000000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(146)],
        b"0000000000000000"
    );
}

#[test]
fn riscv_gdb_remote_packet_handler_maps_rv32_vector_config_csr_width() {
    let mut hart = RiscvHartState::with_hart_id(0x1000, 0);
    hart.set_vector_config(RiscvVectorConfig::invalid());
    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv32, &hart);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p85".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"00000080",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"P85=05000080".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        hart.vector_config(),
        RiscvVectorConfig::new(0, RiscvVectorConfig::VILL_BIT | 5),
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &mut hart,
                &GdbRemotePacket::new(b"p86".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"10000000",
    );
}

#[test]
fn riscv_gdb_remote_core_packet_handler_reads_and_writes_pmp_csrs() {
    let core = riscv_debug_test_core(CpuId::new(5), 0x1000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv64, &core);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p88".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p93".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8d".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P8d=0004000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    for (packet, expected) in [
        (b"p8f".as_slice(), b"0000000000000000".as_slice()),
        (b"P8f=0006000000000000".as_slice(), b"OK".as_slice()),
        (b"p90".as_slice(), b"0000000000000000".as_slice()),
        (b"P90=0007000000000000".as_slice(), b"OK".as_slice()),
        (b"p91".as_slice(), b"0000000000000000".as_slice()),
        (b"P91=0008000000000000".as_slice(), b"OK".as_slice()),
        (b"p92".as_slice(), b"0000000000000000".as_slice()),
        (b"P92=0009000000000000".as_slice(), b"OK".as_slice()),
        (b"p94".as_slice(), b"0000000000000000".as_slice()),
        (b"P94=000a000000000000".as_slice(), b"OK".as_slice()),
        (b"p95".as_slice(), b"0000000000000000".as_slice()),
        (b"P95=000b000000000000".as_slice(), b"OK".as_slice()),
        (b"p96".as_slice(), b"0000000000000000".as_slice()),
        (b"P96=000c000000000000".as_slice(), b"OK".as_slice()),
        (b"p97".as_slice(), b"0000000000000000".as_slice()),
        (b"P97=000d000000000000".as_slice(), b"OK".as_slice()),
        (b"p98".as_slice(), b"0000000000000000".as_slice()),
        (b"P98=000e000000000000".as_slice(), b"OK".as_slice()),
        (b"p99".as_slice(), b"0000000000000000".as_slice()),
        (b"P99=000f000000000000".as_slice(), b"OK".as_slice()),
        (b"p9a".as_slice(), b"0000000000000000".as_slice()),
        (b"P9a=0010000000000000".as_slice(), b"OK".as_slice()),
        (b"p9b".as_slice(), b"0000000000000000".as_slice()),
        (b"P9b=0011000000000000".as_slice(), b"OK".as_slice()),
    ] {
        assert_eq!(
            packet_payload(
                handle_riscv_gdb_remote_core_packet(
                    RiscvGdbXlen::Rv64,
                    &mut session,
                    &core,
                    &GdbRemotePacket::new(packet.to_vec()).unwrap(),
                )
                .unwrap(),
            ),
            expected,
        );
    }
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8e".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P8e=0005000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P93=0f88888888888888".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p89".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8c".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0000000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P8c=0003000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P89=0002000000000000".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P88=0f88888888888888".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p88".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0f88888888888888",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p89".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0002000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8c".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0003000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8d".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0004000000000000",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8e".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0005000000000000",
    );
    for (packet, expected) in [
        (b"p8f".as_slice(), b"0006000000000000".as_slice()),
        (b"p90".as_slice(), b"0007000000000000".as_slice()),
        (b"p91".as_slice(), b"0008000000000000".as_slice()),
        (b"p92".as_slice(), b"0009000000000000".as_slice()),
    ] {
        assert_eq!(
            packet_payload(
                handle_riscv_gdb_remote_core_packet(
                    RiscvGdbXlen::Rv64,
                    &mut session,
                    &core,
                    &GdbRemotePacket::new(packet.to_vec()).unwrap(),
                )
                .unwrap(),
            ),
            expected,
        );
    }
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv64,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p93".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"0f88888888888888",
    );
    for (packet, expected) in [
        (b"p94".as_slice(), b"000a000000000000".as_slice()),
        (b"p95".as_slice(), b"000b000000000000".as_slice()),
        (b"p96".as_slice(), b"000c000000000000".as_slice()),
        (b"p97".as_slice(), b"000d000000000000".as_slice()),
        (b"p98".as_slice(), b"000e000000000000".as_slice()),
        (b"p99".as_slice(), b"000f000000000000".as_slice()),
        (b"p9a".as_slice(), b"0010000000000000".as_slice()),
        (b"p9b".as_slice(), b"0011000000000000".as_slice()),
    ] {
        assert_eq!(
            packet_payload(
                handle_riscv_gdb_remote_core_packet(
                    RiscvGdbXlen::Rv64,
                    &mut session,
                    &core,
                    &GdbRemotePacket::new(packet.to_vec()).unwrap(),
                )
                .unwrap(),
            ),
            expected,
        );
    }
    let registers = packet_payload(
        handle_riscv_gdb_remote_core_packet(
            RiscvGdbXlen::Rv64,
            &mut session,
            &core,
            &GdbRemotePacket::new(b"g".to_vec()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(registers.len(), rv64_register_hex_offset(158));
    assert_eq!(
        &registers[rv64_register_hex_range(136)],
        b"0f88888888888888"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(137)],
        b"0002000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(140)],
        b"0003000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(141)],
        b"0004000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(142)],
        b"0005000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(143)],
        b"0006000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(144)],
        b"0007000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(145)],
        b"0008000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(146)],
        b"0009000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(147)],
        b"0f88888888888888"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(148)],
        b"000a000000000000"
    );
    assert_eq!(
        &registers[rv64_register_hex_range(155)],
        b"0011000000000000"
    );

    let snapshot = core.pmp_snapshot();
    let first = &snapshot.entries()[0];
    assert_eq!(first.config().bits(), 0x0f);
    assert_eq!(first.raw_addr(), 0x0200);
    let second = &snapshot.entries()[1];
    assert_eq!(second.config().bits(), 0x88);
    assert_eq!(second.raw_addr(), 0x0300);
    let third = &snapshot.entries()[2];
    assert_eq!(third.config().bits(), 0x88);
    assert_eq!(third.raw_addr(), 0x0400);
    let fourth = &snapshot.entries()[3];
    assert_eq!(fourth.config().bits(), 0x88);
    assert_eq!(fourth.raw_addr(), 0x0500);
    let fifth = &snapshot.entries()[4];
    assert_eq!(fifth.config().bits(), 0x88);
    assert_eq!(fifth.raw_addr(), 0x0600);
    let sixth = &snapshot.entries()[5];
    assert_eq!(sixth.config().bits(), 0x88);
    assert_eq!(sixth.raw_addr(), 0x0700);
    let seventh = &snapshot.entries()[6];
    assert_eq!(seventh.config().bits(), 0x88);
    assert_eq!(seventh.raw_addr(), 0x0800);
    let eighth = &snapshot.entries()[7];
    assert_eq!(eighth.config().bits(), 0x88);
    assert_eq!(eighth.raw_addr(), 0x0900);
    let ninth = &snapshot.entries()[8];
    assert_eq!(ninth.config().bits(), 0x0f);
    assert_eq!(ninth.raw_addr(), 0x0a00);
    let sixteenth = &snapshot.entries()[15];
    assert_eq!(sixteenth.config().bits(), 0x88);
    assert_eq!(sixteenth.raw_addr(), 0x1100);
}

#[test]
fn riscv_gdb_remote_core_packet_handler_packs_rv32_pmpcfg0_entries() {
    let core = riscv_debug_test_core(CpuId::new(6), 0x1000);
    let mut session = riscv_gdb_remote_session_from_core(RiscvGdbXlen::Rv32, &core);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P8c=78563412".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P9c=88776655".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P9d=ccbbaa99".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"P88=44332211".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"OK",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p88".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"44332211",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p9c".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"88776655",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p9d".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"ccbbaa99",
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_core_packet(
                RiscvGdbXlen::Rv32,
                &mut session,
                &core,
                &GdbRemotePacket::new(b"p8c".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"78563412",
    );

    let snapshot = core.pmp_snapshot();
    assert_eq!(snapshot.entries()[0].config().bits(), 0x44);
    assert_eq!(snapshot.entries()[1].config().bits(), 0x33);
    assert_eq!(snapshot.entries()[2].config().bits(), 0x22);
    assert_eq!(snapshot.entries()[3].config().bits(), 0x11);
    assert_eq!(snapshot.entries()[4].config().bits(), 0x88);
    assert_eq!(snapshot.entries()[7].config().bits(), 0x55);
    assert_eq!(snapshot.entries()[12].config().bits(), 0xcc);
    assert_eq!(snapshot.entries()[15].config().bits(), 0x99);
    assert_eq!(snapshot.entries()[1].raw_addr(), 0x1234_5678);
}

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}

fn rv64_register_hex_range(number: u64) -> std::ops::Range<usize> {
    let start = rv64_register_hex_offset(number);
    let end = rv64_register_hex_offset(number + 1);
    start..end
}

fn rv64_register_hex_offset(number: u64) -> usize {
    let byte_offset = match number {
        0..=32 => number * 8,
        33..=65 => (33 * 8) + ((number - 33) * 8),
        66..=69 => (33 * 8) + (32 * 8) + ((number - 66) * 4),
        70..=89 => (33 * 8) + (32 * 8) + (4 * 4) + ((number - 70) * 8),
        90..=121 => (33 * 8) + (32 * 8) + (4 * 4) + (20 * 8) + ((number - 90) * 16),
        122..=158 => (33 * 8) + (32 * 8) + (4 * 4) + (20 * 8) + (32 * 16) + ((number - 122) * 8),
        _ => panic!("unexpected RV64 GDB register number {number}"),
    };
    byte_offset as usize * 2
}

fn riscv_debug_test_core(cpu: CpuId, entry: u64) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                cpu,
                PartitionId::new(0),
                AgentId::new(11),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("test.ifetch").unwrap(),
                MemoryRouteId::new(3),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}
