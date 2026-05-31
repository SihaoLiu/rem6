use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, LineMemorySnapshot, LineMemoryStore,
    MemoryAtomicOp, MemoryError, MemoryLineSnapshot, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(3), sequence)
}

type AtomicBinary = fn(u64, u64) -> u64;
type AtomicLogicalCase = (MemoryAtomicOp, AtomicBinary);

fn line_data(base: u8) -> Vec<u8> {
    (0..64).map(|offset| base.wrapping_add(offset)).collect()
}

fn read(sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(sequence: u64, address: u64, data: Vec<u8>, mask: ByteMask) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        size,
        data,
        mask,
        layout(),
    )
    .unwrap()
}

fn atomic(sequence: u64, address: u64, data: Vec<u8>, mask: ByteMask) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::atomic(
        request_id(sequence),
        Address::new(address),
        size,
        data,
        mask,
        layout(),
    )
    .unwrap()
}

fn atomic_with_op(
    sequence: u64,
    address: u64,
    op: MemoryAtomicOp,
    data: Vec<u8>,
    mask: ByteMask,
) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::atomic_with_op(
        request_id(sequence),
        Address::new(address),
        size,
        op,
        data,
        mask,
        layout(),
    )
    .unwrap()
}

#[test]
fn line_store_serves_reads_from_independent_lines() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x10))
        .unwrap();
    store
        .insert_line(Address::new(0x2000), line_data(0x80))
        .unwrap();

    let first = store.respond(&read(1, 0x1004, 4)).unwrap().unwrap();
    let second = store.respond(&read(2, 0x2002, 3)).unwrap().unwrap();

    assert_eq!(first.status(), ResponseStatus::Completed);
    assert_eq!(first.data(), Some(&[0x14, 0x15, 0x16, 0x17][..]));
    assert_eq!(second.data(), Some(&[0x82, 0x83, 0x84][..]));
    assert_eq!(store.line_count(), 2);
    assert_eq!(store.line_data(Address::new(0x1000)).unwrap()[0], 0x10);
    assert_eq!(store.line_data(Address::new(0x2000)).unwrap()[0], 0x80);
}

#[test]
fn line_store_applies_masked_writes_and_reports_completed_response() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x00))
        .unwrap();
    let request = write(
        3,
        0x1002,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::from_bits(vec![true, false, true, false]).unwrap(),
    );

    let response = store.respond(&request).unwrap().unwrap();

    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), None);
    assert_eq!(
        &store.line_data(Address::new(0x1000)).unwrap()[0..8],
        &[0, 1, 0xaa, 3, 0xcc, 5, 6, 7]
    );
}

#[test]
fn line_store_atomic_returns_old_bytes_before_applying_masked_write() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x00))
        .unwrap();
    let request = atomic(
        4,
        0x1002,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::from_bits(vec![true, false, true, false]).unwrap(),
    );

    let response = store.respond(&request).unwrap().unwrap();

    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), Some(&[2, 3, 4, 5][..]));
    assert_eq!(
        &store.line_data(Address::new(0x1000)).unwrap()[0..8],
        &[0, 1, 0xaa, 3, 0xcc, 5, 6, 7]
    );
}

#[test]
fn line_store_atomic_add_returns_old_bytes_and_writes_wrapped_sum() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x00))
        .unwrap();
    let request = atomic_with_op(
        5,
        0x1008,
        MemoryAtomicOp::Add,
        0x0102_0304_0506_0708u64.to_le_bytes().to_vec(),
        ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
    );

    let response = store.respond(&request).unwrap().unwrap();

    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), Some(&[8, 9, 10, 11, 12, 13, 14, 15][..]));
    assert_eq!(
        &store.line_data(Address::new(0x1000)).unwrap()[8..16],
        &0x1010_1010_1010_1010u64.to_le_bytes()
    );
}

#[test]
fn line_store_atomic_logical_ops_return_old_bytes_and_write_bitwise_result() {
    let cases: [AtomicLogicalCase; 3] = [
        (MemoryAtomicOp::Xor, |old: u64, operand: u64| old ^ operand),
        (MemoryAtomicOp::Or, |old: u64, operand: u64| old | operand),
        (MemoryAtomicOp::And, |old: u64, operand: u64| old & operand),
    ];

    for (index, (op, expected)) in cases.into_iter().enumerate() {
        let old = 0xf0f0_0f0f_aaaa_5555u64;
        let operand = 0x0ff0_f00f_5555_3333u64;
        let mut line = line_data(0x00);
        line[8..16].copy_from_slice(&old.to_le_bytes());
        let mut store = LineMemoryStore::new(layout());
        store.insert_line(Address::new(0x1000), line).unwrap();
        let request = atomic_with_op(
            6 + index as u64,
            0x1008,
            op,
            operand.to_le_bytes().to_vec(),
            ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
        );

        let response = store.respond(&request).unwrap().unwrap();

        assert_eq!(response.status(), ResponseStatus::Completed);
        assert_eq!(response.data(), Some(&old.to_le_bytes()[..]));
        assert_eq!(
            &store.line_data(Address::new(0x1000)).unwrap()[8..16],
            &expected(old, operand).to_le_bytes()
        );
    }
}

#[test]
fn line_store_atomic_min_max_ops_return_old_bytes_and_write_selected_value() {
    let negative = 0xffff_ffff_ffff_fff0u64;
    let positive = 7u64;
    let cases: [(MemoryAtomicOp, u64, u64, u64); 4] = [
        (MemoryAtomicOp::MinSigned, negative, positive, negative),
        (MemoryAtomicOp::MaxSigned, negative, positive, positive),
        (MemoryAtomicOp::MinUnsigned, negative, positive, positive),
        (MemoryAtomicOp::MaxUnsigned, negative, positive, negative),
    ];

    for (index, (op, old, operand, expected)) in cases.into_iter().enumerate() {
        let mut line = line_data(0x00);
        line[8..16].copy_from_slice(&old.to_le_bytes());
        let mut store = LineMemoryStore::new(layout());
        store.insert_line(Address::new(0x1000), line).unwrap();
        let request = atomic_with_op(
            9 + index as u64,
            0x1008,
            op,
            operand.to_le_bytes().to_vec(),
            ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
        );

        let response = store.respond(&request).unwrap().unwrap();

        assert_eq!(response.status(), ResponseStatus::Completed);
        assert_eq!(response.data(), Some(&old.to_le_bytes()[..]));
        assert_eq!(
            &store.line_data(Address::new(0x1000)).unwrap()[8..16],
            &expected.to_le_bytes()
        );
    }
}

#[test]
fn line_store_replaces_dirty_writebacks_without_response() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x00))
        .unwrap();
    let request = MemoryRequest::writeback_dirty(
        request_id(4),
        Address::new(0x1000),
        line_data(0x40),
        layout(),
    )
    .unwrap();

    let response = store.respond(&request).unwrap();

    assert_eq!(response, None);
    assert_eq!(
        store.line_data(Address::new(0x1000)).unwrap(),
        line_data(0x40)
    );
}

#[test]
fn line_store_rejects_unmapped_and_cross_line_accesses() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x00))
        .unwrap();

    let missing = store.respond(&read(5, 0x2000, 4)).unwrap_err();
    assert_eq!(
        missing,
        MemoryError::UnmappedLine {
            line: Address::new(0x2000),
        }
    );

    let crossing = store.respond(&read(6, 0x103e, 4)).unwrap_err();
    assert_eq!(
        crossing,
        MemoryError::CrossLineAccess {
            request: request_id(6),
            start: Address::new(0x103e),
            size: AccessSize::new(4).unwrap(),
            line_size: 64,
        }
    );
}

#[test]
fn line_store_validates_inserted_line_shape() {
    let mut store = LineMemoryStore::new(layout());

    assert_eq!(
        store
            .insert_line(Address::new(0x1004), line_data(0x00))
            .unwrap_err(),
        MemoryError::UnalignedLineAddress {
            address: Address::new(0x1004),
            line_size: 64,
        }
    );
    assert_eq!(
        store
            .insert_line(Address::new(0x1000), vec![0; 32])
            .unwrap_err(),
        MemoryError::PayloadSizeMismatch {
            expected: AccessSize::new(64).unwrap(),
            actual: 32,
        }
    );
}

#[test]
fn line_store_restore_rejects_duplicate_line_snapshots() {
    let duplicate_line = Address::new(0x1000);
    let snapshot = LineMemorySnapshot::new(
        layout(),
        vec![
            MemoryLineSnapshot::new(duplicate_line, line_data(0x10)),
            MemoryLineSnapshot::new(duplicate_line, line_data(0x20)),
        ],
    );

    let expected_error = MemoryError::DuplicateMemoryLine {
        line: duplicate_line,
    };
    assert_eq!(
        LineMemoryStore::from_snapshot(&snapshot),
        Err(expected_error.clone())
    );

    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x2000), line_data(0x80))
        .unwrap();
    let before_restore = store.snapshot();
    assert_eq!(store.restore(&snapshot), Err(expected_error));
    assert_eq!(store.snapshot(), before_restore);
}

#[test]
fn line_store_rejects_requests_with_different_line_layout() {
    let mut store = LineMemoryStore::new(layout());
    store
        .insert_line(Address::new(0x1000), line_data(0x10))
        .unwrap();

    let actual = CacheLineLayout::new(128).unwrap();
    let request = MemoryRequest::read_shared(
        request_id(6),
        Address::new(0x1008),
        AccessSize::new(8).unwrap(),
        actual,
    )
    .unwrap();
    assert_eq!(
        store.respond(&request).unwrap_err(),
        MemoryError::LineLayoutMismatch {
            request: request.id(),
            expected: layout(),
            actual,
        }
    );
}
