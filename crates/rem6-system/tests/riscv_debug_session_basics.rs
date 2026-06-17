use rem6_debug::{GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::{Register, RiscvGdbXlen, RiscvHartState};
use rem6_memory::{
    Address, TranslationPageMap, TranslationPageMappingScope, TranslationPagePermissions,
    TranslationPageSize,
};
use rem6_system::{
    riscv_gdb_page_table_dump_from_translation_map, riscv_gdb_remote_session,
    riscv_gdb_remote_session_from_hart, riscv_gdb_remote_session_from_translation_map,
    riscv_gdb_remote_session_with_page_table_dump,
};

#[test]
fn riscv_gdb_remote_session_advertises_target_description_xfer() {
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        session
            .handle_packet(&GdbRemotePacket::new(b"qSupported".to_vec()).unwrap())
            .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(
                GdbRemotePacket::new(
                    b"PacketSize=4000;qXfer:features:read+;vContSupported+".to_vec(),
                )
                .unwrap(),
            ),
        ],
    );
}

#[test]
fn riscv_gdb_remote_session_serves_rv64_target_documents() {
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    let target = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,400".to_vec()).unwrap(),
            )
            .unwrap(),
    );
    assert!(target.starts_with(b"l<?xml version=\"1.0\"?>\n"));
    let target = std::str::from_utf8(&target[1..]).unwrap();
    assert!(target.contains("<architecture>riscv</architecture>"));
    assert!(target.contains("<xi:include href=\"riscv-64bit-cpu.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-64bit-fpu.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-64bit-csr.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-64bit-vector.xml\"/>"));
    assert!(!target.contains("riscv-32bit-cpu.xml"));

    let cpu = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:riscv-64bit-cpu.xml:0,2000".to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let cpu = std::str::from_utf8(&cpu[1..]).unwrap();
    assert!(cpu.contains("<reg name=\"zero\" bitsize=\"64\" type=\"int\" regnum=\"0\"/>"));
    assert!(cpu.contains("<reg name=\"pc\" bitsize=\"64\" type=\"code_ptr\"/>"));
    assert!(!cpu.contains("bitsize=\"32\""));

    let fpu = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:riscv-64bit-fpu.xml:0,2000".to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let fpu = std::str::from_utf8(&fpu[1..]).unwrap();
    assert!(fpu.contains("<feature name=\"org.gnu.gdb.riscv.fpu\">"));
    assert!(fpu.contains("<reg name=\"ft0\" bitsize=\"64\" type=\"riscv_double\" regnum=\"33\"/>"));
    assert!(fpu.contains("<reg name=\"ft11\" bitsize=\"64\" type=\"riscv_double\"/>"));
    assert!(fpu.contains("<reg name=\"fcsr\" bitsize=\"32\" type=\"int\" regnum=\"68\"/>"));

    let csr = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:riscv-64bit-csr.xml:0,2000".to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let csr = std::str::from_utf8(&csr[1..]).unwrap();
    assert!(csr.contains("<reg name=\"sstatus\" bitsize=\"64\" regnum=\"70\"/>"));
    assert!(csr.contains("<reg name=\"sscratch\" bitsize=\"64\"/>"));
    assert!(csr.contains("<reg name=\"stval\" bitsize=\"64\"/>"));
    assert!(csr.contains("<reg name=\"satp\" bitsize=\"64\"/>"));
    assert!(csr.contains("<reg name=\"mscratch\" bitsize=\"64\"/>"));

    let vector = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(
                    b"qXfer:features:read:riscv-64bit-vector.xml:0,2000".to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
    );
    let vector = std::str::from_utf8(&vector[1..]).unwrap();
    assert!(vector.contains("<feature name=\"org.gnu.gdb.riscv.vector\">"));
    assert!(vector.contains("<reg name=\"v0\" bitsize=\"128\" type=\"uint128\" regnum=\"90\"/>"));
    assert!(vector.contains("<reg name=\"v31\" bitsize=\"128\" type=\"uint128\"/>"));
}

#[test]
fn riscv_gdb_remote_session_serves_rv32_target_documents() {
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv32);

    let target = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:target.xml:0,400".to_vec()).unwrap(),
            )
            .unwrap(),
    );
    let target = std::str::from_utf8(&target[1..]).unwrap();
    assert!(target.contains("<xi:include href=\"riscv-32bit-cpu.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-fpu.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-csr.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-vector.xml\"/>"));
    assert!(!target.contains("riscv-64bit-cpu.xml"));

    let cpu = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(b"qXfer:features:read:riscv-32bit-cpu.xml:0,2000".to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let cpu = std::str::from_utf8(&cpu[1..]).unwrap();
    assert!(cpu.contains("<reg name=\"zero\" bitsize=\"32\" type=\"int\" regnum=\"0\"/>"));
    assert!(cpu.contains("<reg name=\"pc\" bitsize=\"32\" type=\"code_ptr\"/>"));
    assert!(!cpu.contains("bitsize=\"64\""));

    let vector = packet_payload(
        session
            .handle_packet(
                &GdbRemotePacket::new(
                    b"qXfer:features:read:riscv-32bit-vector.xml:0,2000".to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
    );
    let vector = std::str::from_utf8(&vector[1..]).unwrap();
    assert!(vector.contains("<feature name=\"org.gnu.gdb.riscv.vector\">"));
    assert!(vector.contains("<reg name=\"v0\" bitsize=\"128\" type=\"uint128\" regnum=\"90\"/>"));
    assert!(vector.contains("<reg name=\"v31\" bitsize=\"128\" type=\"uint128\"/>"));
}

#[test]
fn riscv_gdb_remote_session_serves_page_table_dump_payload() {
    let mut session = riscv_gdb_remote_session_with_page_table_dump(
        RiscvGdbXlen::Rv64,
        b"vpn=0x1000 ppn=0x2000 rwx\n".to_vec(),
    );

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b".".to_vec()).unwrap())
                .unwrap(),
        ),
        b"vpn=0x1000 ppn=0x2000 rwx\n",
    );
}

#[test]
fn riscv_gdb_page_table_dump_formats_translation_map() {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        Address::new(0x4000),
        Address::new(0x8000),
        2,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();
    map.map(
        Address::new(0x1000),
        Address::new(0x5000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();

    assert_eq!(
        riscv_gdb_page_table_dump_from_translation_map(&map),
        b"page_size=0x1000\nvaddr=0x1000 paddr=0x5000 pages=1 flags=rw- scope=non-global\nvaddr=0x4000 paddr=0x8000 pages=2 flags=r-x scope=non-global\n",
    );
}

#[test]
fn riscv_gdb_page_table_dump_formats_translation_map_scope() {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map_with_scope(
        Address::new(0x6000),
        Address::new(0xe000),
        1,
        TranslationPagePermissions::read_execute(),
        TranslationPageMappingScope::Global,
    )
    .unwrap();

    assert_eq!(
        riscv_gdb_page_table_dump_from_translation_map(&map),
        b"page_size=0x1000\nvaddr=0x6000 paddr=0xe000 pages=1 flags=r-x scope=global\n",
    );
}

#[test]
fn riscv_gdb_remote_session_serves_translation_map_page_table_dump() {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        Address::new(0x2000),
        Address::new(0xa000),
        1,
        TranslationPagePermissions::read_only(),
    )
    .unwrap();

    let mut session = riscv_gdb_remote_session_from_translation_map(RiscvGdbXlen::Rv64, &map);

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b".".to_vec()).unwrap())
                .unwrap(),
        ),
        b"page_size=0x1000\nvaddr=0x2000 paddr=0xa000 pages=1 flags=r-- scope=non-global\n",
    );
}

#[test]
fn riscv_gdb_remote_session_reports_rv64_hart_register_snapshot() {
    let mut hart = RiscvHartState::with_hart_id(0x8877_6655_4433_2211, 0);
    hart.write(Register::new(1).unwrap(), 0x0123_4567_89ab_cdef);
    hart.write(Register::new(10).unwrap(), 0xfedc_ba98_7654_3210);

    let mut session = riscv_gdb_remote_session_from_hart(RiscvGdbXlen::Rv64, &hart);

    let registers = packet_payload(
        session
            .handle_packet(&GdbRemotePacket::new(b"g".to_vec()).unwrap())
            .unwrap(),
    );
    assert_eq!(registers.len(), rv64_register_hex_offset(124));
    assert_eq!(&registers[0..16], b"0000000000000000");
    assert_eq!(&registers[16..32], b"efcdab8967452301");
    assert_eq!(&registers[10 * 16..11 * 16], b"1032547698badcfe");
    assert_eq!(&registers[32 * 16..33 * 16], b"1122334455667788");

    assert_eq!(
        packet_payload(
            session
                .handle_packet(&GdbRemotePacket::new(b"p20".to_vec()).unwrap())
                .unwrap(),
        ),
        b"1122334455667788",
    );
}

fn rv64_register_hex_offset(number: u64) -> usize {
    let byte_offset = match number {
        0..=32 => number * 8,
        33..=65 => (33 * 8) + ((number - 33) * 8),
        66..=69 => (33 * 8) + (32 * 8) + ((number - 66) * 4),
        70..=89 => (33 * 8) + (32 * 8) + (4 * 4) + ((number - 70) * 8),
        90..=122 => (33 * 8) + (32 * 8) + (4 * 4) + (20 * 8) + ((number - 90) * 16),
        123..=124 => (33 * 8) + (32 * 8) + (4 * 4) + (20 * 8) + (32 * 16) + ((number - 122) * 8),
        _ => panic!("unsupported RV64 GDB register number"),
    };
    byte_offset as usize * 2
}

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
