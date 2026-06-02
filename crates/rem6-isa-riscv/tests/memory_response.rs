use rem6_isa_riscv::{
    AtomicMemoryOp, MemoryAccessKind, MemoryResponseError, MemoryResponseWriteback, MemoryWidth,
    Register,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn load(width: MemoryWidth, signed: bool) -> MemoryAccessKind {
    MemoryAccessKind::Load {
        rd: reg(5),
        address: 0x8000,
        width,
        signed,
    }
}

#[test]
fn memory_width_reports_byte_counts() {
    assert_eq!(MemoryWidth::Byte.bytes(), 1);
    assert_eq!(MemoryWidth::Halfword.bytes(), 2);
    assert_eq!(MemoryWidth::Word.bytes(), 4);
    assert_eq!(MemoryWidth::Doubleword.bytes(), 8);
}

#[test]
fn load_response_writeback_decodes_width_and_signedness() {
    let cases = [
        (
            load(MemoryWidth::Byte, true),
            &[0x80, 0xaa][..],
            0xffff_ffff_ffff_ff80,
        ),
        (load(MemoryWidth::Byte, false), &[0x80, 0xaa][..], 0x80),
        (
            load(MemoryWidth::Halfword, true),
            &[0x00, 0x80, 0xaa][..],
            0xffff_ffff_ffff_8000,
        ),
        (
            load(MemoryWidth::Halfword, false),
            &[0x00, 0x80, 0xaa][..],
            0x8000,
        ),
        (
            load(MemoryWidth::Word, true),
            &[0x00, 0x00, 0x00, 0x80, 0xaa][..],
            0xffff_ffff_8000_0000,
        ),
        (
            load(MemoryWidth::Word, false),
            &[0x00, 0x00, 0x00, 0x80, 0xaa][..],
            0x8000_0000,
        ),
        (
            load(MemoryWidth::Doubleword, true),
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80][..],
            0x8000_0000_0000_0000,
        ),
    ];

    for (access, data, expected) in cases {
        assert_eq!(
            access.read_response_writeback(data),
            Ok(Some(MemoryResponseWriteback::new(reg(5), expected)))
        );
    }
}

#[test]
fn read_modify_write_response_writeback_returns_old_memory_value() {
    let load_reserved_word = MemoryAccessKind::LoadReserved {
        rd: reg(6),
        address: 0x9000,
        width: MemoryWidth::Word,
        acquire: true,
        release: false,
    };
    let atomic_doubleword = MemoryAccessKind::AtomicMemory {
        rd: reg(7),
        address: 0x9008,
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Swap,
        value: 0x1234,
        acquire: false,
        release: true,
    };

    assert_eq!(
        load_reserved_word.read_response_writeback(&[0x00, 0x00, 0x00, 0x80]),
        Ok(Some(MemoryResponseWriteback::new(
            reg(6),
            0xffff_ffff_8000_0000
        )))
    );
    assert_eq!(
        atomic_doubleword
            .read_response_writeback(&[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]),
        Ok(Some(MemoryResponseWriteback::new(
            reg(7),
            0x1122_3344_5566_7788
        )))
    );
}

#[test]
fn stores_have_no_read_response_writeback() {
    let store = MemoryAccessKind::Store {
        address: 0x8000,
        width: MemoryWidth::Doubleword,
        value: 0x55,
    };
    let store_conditional = MemoryAccessKind::StoreConditional {
        rd: reg(8),
        address: 0x8000,
        width: MemoryWidth::Doubleword,
        value: 0x55,
        acquire: false,
        release: false,
    };

    assert_eq!(store.read_response_writeback(&[]), Ok(None));
    assert_eq!(store_conditional.read_response_writeback(&[]), Ok(None));
}

#[test]
fn read_response_writeback_rejects_short_data() {
    assert_eq!(
        load(MemoryWidth::Word, true).read_response_writeback(&[0x00, 0x80]),
        Err(MemoryResponseError::ShortData {
            expected: 4,
            actual: 2
        })
    );
}
