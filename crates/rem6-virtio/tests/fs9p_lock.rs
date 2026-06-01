use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_LOCK_BLOCKED,
    VIRTIO_9P_LOCK_SUCCESS, VIRTIO_9P_LOCK_TYPE_RDLCK, VIRTIO_9P_LOCK_TYPE_UNLCK,
    VIRTIO_9P_LOCK_TYPE_WRLCK, VIRTIO_9P_NOFID, VIRTIO_9P_RGETLOCK, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLINK, VIRTIO_9P_RLOCK, VIRTIO_9P_RLOPEN, VIRTIO_9P_RREMOVE, VIRTIO_9P_RWALK,
    VIRTIO_9P_TATTACH, VIRTIO_9P_TCLUNK, VIRTIO_9P_TGETLOCK, VIRTIO_9P_TLINK, VIRTIO_9P_TLOCK,
    VIRTIO_9P_TLOPEN, VIRTIO_9P_TREMOVE, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

fn open_file_fid(device: &Virtio9pDevice, tag: u16, fid: u32, name: &[u8]) {
    let walk = decoded_request(VIRTIO_9P_TWALK, tag, p9_walk_payload(1, fid, &[name]));
    assert_eq!(
        device
            .execute_at(u64::from(tag), walk)
            .unwrap()
            .message_type(),
        VIRTIO_9P_RWALK
    );
    let open = decoded_request(VIRTIO_9P_TLOPEN, tag + 1, p9_lopen_payload(fid, 0));
    assert_eq!(
        device
            .execute_at(u64::from(tag + 1), open)
            .unwrap()
            .message_type(),
        VIRTIO_9P_RLOPEN
    );
}

#[test]
fn virtio_9p_device_reports_conflicting_byte_range_locks() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    open_file_fid(&device, 2, 2, b"alpha.txt");
    open_file_fid(&device, 4, 3, b"alpha.txt");

    let writer = decoded_request(
        VIRTIO_9P_TLOCK,
        6,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 10, 42, b"client-a"),
    );
    let writer_completion = device.execute_at(20, writer).unwrap();
    assert_eq!(writer_completion.message_type(), VIRTIO_9P_RLOCK);
    assert_eq!(writer_completion.payload(), [VIRTIO_9P_LOCK_SUCCESS]);

    let blocked_reader = decoded_request(
        VIRTIO_9P_TLOCK,
        7,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 5, 3, 77, b"client-b"),
    );
    let blocked_reader_completion = device.execute_at(21, blocked_reader).unwrap();
    assert_eq!(blocked_reader_completion.message_type(), VIRTIO_9P_RLOCK);
    assert_eq!(
        blocked_reader_completion.payload(),
        [VIRTIO_9P_LOCK_BLOCKED]
    );

    let getlock = decoded_request(
        VIRTIO_9P_TGETLOCK,
        8,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 5, 3, 77, b"client-b"),
    );
    let getlock_completion = device.execute_at(22, getlock).unwrap();
    assert_eq!(getlock_completion.message_type(), VIRTIO_9P_RGETLOCK);
    let payload = getlock_completion.payload();
    assert_eq!(payload[0], VIRTIO_9P_LOCK_TYPE_WRLCK);
    assert_eq!(read_u32(payload, 1), 0);
    assert_eq!(read_u64(payload, 5), 0);
    assert_eq!(read_u64(payload, 13), 10);
    assert_eq!(read_u32(payload, 21), 42);
    assert_eq!(read_string(payload, 25), b"client-a");
}

#[test]
fn virtio_9p_device_unlocks_matching_byte_ranges() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    open_file_fid(&device, 2, 2, b"alpha.txt");
    open_file_fid(&device, 4, 3, b"alpha.txt");

    let writer = decoded_request(
        VIRTIO_9P_TLOCK,
        6,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 10, 42, b"client-a"),
    );
    assert_eq!(
        device.execute_at(20, writer).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
    let unlock = decoded_request(
        VIRTIO_9P_TLOCK,
        7,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_UNLCK, 0, 0, 10, 42, b"client-a"),
    );
    assert_eq!(
        device.execute_at(21, unlock).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );

    let reader = decoded_request(
        VIRTIO_9P_TLOCK,
        8,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 5, 3, 77, b"client-b"),
    );
    assert_eq!(
        device.execute_at(22, reader).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
}

#[test]
fn virtio_9p_device_partial_unlock_keeps_unreleased_byte_ranges() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    open_file_fid(&device, 2, 2, b"alpha.txt");
    open_file_fid(&device, 4, 3, b"alpha.txt");

    let writer = decoded_request(
        VIRTIO_9P_TLOCK,
        6,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 10, 42, b"client-a"),
    );
    assert_eq!(
        device.execute_at(20, writer).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
    let partial_unlock = decoded_request(
        VIRTIO_9P_TLOCK,
        7,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_UNLCK, 0, 2, 4, 42, b"client-a"),
    );
    assert_eq!(
        device.execute_at(21, partial_unlock).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );

    let blocked_left = decoded_request(
        VIRTIO_9P_TLOCK,
        8,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 0, 1, 77, b"client-b"),
    );
    assert_eq!(
        device.execute_at(22, blocked_left).unwrap().payload(),
        [VIRTIO_9P_LOCK_BLOCKED]
    );
    let released_middle = decoded_request(
        VIRTIO_9P_TLOCK,
        9,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 3, 1, 77, b"client-b"),
    );
    assert_eq!(
        device.execute_at(23, released_middle).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
    let blocked_right = decoded_request(
        VIRTIO_9P_TLOCK,
        10,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 8, 1, 77, b"client-b"),
    );
    assert_eq!(
        device.execute_at(24, blocked_right).unwrap().payload(),
        [VIRTIO_9P_LOCK_BLOCKED]
    );
}

#[test]
fn virtio_9p_device_releases_byte_range_locks_when_fid_is_clunked() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    open_file_fid(&device, 2, 2, b"alpha.txt");
    open_file_fid(&device, 4, 3, b"alpha.txt");

    let writer = decoded_request(
        VIRTIO_9P_TLOCK,
        6,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 10, 42, b"client-a"),
    );
    assert_eq!(
        device.execute_at(20, writer).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
    let clunk = decoded_request(VIRTIO_9P_TCLUNK, 7, p9_clunk_payload(2));
    device.execute_at(21, clunk).unwrap();

    let reader = decoded_request(
        VIRTIO_9P_TLOCK,
        8,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 5, 3, 77, b"client-b"),
    );
    assert_eq!(
        device.execute_at(22, reader).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
}

#[test]
fn virtio_9p_device_releases_locks_when_removed_fid_has_surviving_links() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    open_file_fid(&device, 2, 2, b"alpha.txt");
    let link = decoded_request(VIRTIO_9P_TLINK, 4, p9_link_payload(1, 2, b"beta.txt"));
    assert_eq!(
        device.execute_at(12, link).unwrap().message_type(),
        VIRTIO_9P_RLINK
    );
    open_file_fid(&device, 5, 3, b"beta.txt");

    let writer = decoded_request(
        VIRTIO_9P_TLOCK,
        7,
        p9_lock_payload(2, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 10, 42, b"client-a"),
    );
    assert_eq!(
        device.execute_at(20, writer).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
    let remove = decoded_request(VIRTIO_9P_TREMOVE, 8, p9_remove_payload(2));
    assert_eq!(
        device.execute_at(21, remove).unwrap().message_type(),
        VIRTIO_9P_RREMOVE
    );

    let reader = decoded_request(
        VIRTIO_9P_TLOCK,
        9,
        p9_lock_payload(3, VIRTIO_9P_LOCK_TYPE_RDLCK, 0, 5, 3, 77, b"client-b"),
    );
    assert_eq!(
        device.execute_at(22, reader).unwrap().payload(),
        [VIRTIO_9P_LOCK_SUCCESS]
    );
}

#[test]
fn virtio_9p_device_keeps_stale_lock_rejections_as_lerrors() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let lock = decoded_request(
        VIRTIO_9P_TLOCK,
        1,
        p9_lock_payload(7, VIRTIO_9P_LOCK_TYPE_WRLCK, 0, 0, 5, 42, b"client-a"),
    );
    let lock_completion = device.execute_at(10, lock).unwrap();
    assert_eq!(lock_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(lock_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}
