use rem6_debug::{GdbRemoteFrame, GdbRemotePacket};
use rem6_isa_riscv::RiscvGdbXlen;
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore};
use rem6_system::{handle_riscv_gdb_remote_memory_packet, riscv_gdb_remote_session};

#[test]
fn riscv_gdb_remote_memory_packet_handler_reads_partitioned_store_across_lines() {
    let mut store = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_memory_packet(
                &mut session,
                &mut store,
                &GdbRemotePacket::new(b"m100e,4".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"eeff1122",
    );
}

#[test]
fn riscv_gdb_remote_memory_packet_handler_writes_partitioned_store_across_lines() {
    let mut store = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        handle_riscv_gdb_remote_memory_packet(
            &mut session,
            &mut store,
            &GdbRemotePacket::new(b"M100e,4:aabbccdd".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"OK".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_memory_packet(
                &mut session,
                &mut store,
                &GdbRemotePacket::new(b"m100c,8".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"ccddaabbccdd3344",
    );
}

#[test]
fn riscv_gdb_remote_memory_packet_handler_rejects_invalid_write_without_partial_update() {
    let mut store = debug_memory_store();
    let mut session = riscv_gdb_remote_session(RiscvGdbXlen::Rv64);

    assert_eq!(
        handle_riscv_gdb_remote_memory_packet(
            &mut session,
            &mut store,
            &GdbRemotePacket::new(b"M101f,2:aabb".to_vec()).unwrap(),
        )
        .unwrap(),
        vec![
            GdbRemoteFrame::Ack,
            GdbRemoteFrame::Packet(GdbRemotePacket::new(b"E01".to_vec()).unwrap()),
        ],
    );
    assert_eq!(
        packet_payload(
            handle_riscv_gdb_remote_memory_packet(
                &mut session,
                &mut store,
                &GdbRemotePacket::new(b"m100c,8".to_vec()).unwrap(),
            )
            .unwrap(),
        ),
        b"ccddeeff11223344",
    );
}

fn debug_memory_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(0);
    let layout = CacheLineLayout::new(16).unwrap();
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout).unwrap();
    store
        .map_region(target, Address::new(0x1000), AccessSize::new(0x20).unwrap())
        .unwrap();
    store
        .insert_line(
            target,
            Address::new(0x1000),
            vec![
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ],
        )
        .unwrap();
    store
        .insert_line(
            target,
            Address::new(0x1010),
            vec![
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
                0xff, 0x00,
            ],
        )
        .unwrap();
    store
}

fn packet_payload(frames: Vec<GdbRemoteFrame>) -> Vec<u8> {
    let [GdbRemoteFrame::Ack, GdbRemoteFrame::Packet(packet)] = frames.as_slice() else {
        panic!("expected acknowledged packet response, got {frames:?}");
    };
    packet.payload().to_vec()
}
