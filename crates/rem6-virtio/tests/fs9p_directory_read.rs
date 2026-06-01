use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_DEFAULT_MSIZE, VIRTIO_9P_DTDIR, VIRTIO_9P_DTREG,
    VIRTIO_9P_EBADF, VIRTIO_9P_NOFID, VIRTIO_9P_QTDIR, VIRTIO_9P_QTFILE, VIRTIO_9P_RLERROR,
    VIRTIO_9P_RLOPEN, VIRTIO_9P_RREADDIR, VIRTIO_9P_TATTACH, VIRTIO_9P_TLOPEN, VIRTIO_9P_TREADDIR,
    VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_opens_and_reads_root_directory_entries() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("beta.txt", b"beta".to_vec())
        .unwrap()
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 2, p9_lopen_payload(1, 0));
    let open_completion = device.execute_at(11, open_root).unwrap();
    assert_eq!(open_completion.message_type(), VIRTIO_9P_RLOPEN);
    let (open_qtype, _, open_path) = read_qid(open_completion.payload(), 0);
    assert_eq!(open_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(open_path, 1);
    assert_eq!(
        open_completion.payload()[13..17],
        VIRTIO_9P_DEFAULT_MSIZE.to_le_bytes()
    );

    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 3, p9_readdir_payload(1, 0, 512));
    let completion = device.execute_at(12, readdir).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RREADDIR);
    let entries = read_dir_entries(completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".", "..", "alpha.txt", "beta.txt"]);
    assert_eq!(entries[0].qtype, VIRTIO_9P_QTDIR);
    assert_eq!(entries[0].qpath, 1);
    assert_eq!(entries[0].dtype, VIRTIO_9P_DTDIR);
    assert_eq!(entries[1].qtype, VIRTIO_9P_QTDIR);
    assert_eq!(entries[1].qpath, 1);
    assert_eq!(entries[1].dtype, VIRTIO_9P_DTDIR);
    assert_eq!(entries[2].qtype, VIRTIO_9P_QTFILE);
    assert_eq!(entries[2].dtype, VIRTIO_9P_DTREG);
    assert_eq!(entries[3].qtype, VIRTIO_9P_QTFILE);
    assert_eq!(entries[3].dtype, VIRTIO_9P_DTREG);
    assert!(entries
        .windows(2)
        .all(|pair| pair[0].next_offset < pair[1].next_offset));

    let resume = decoded_request(
        VIRTIO_9P_TREADDIR,
        4,
        p9_readdir_payload(1, entries[0].next_offset, 512),
    );
    let resumed_completion = device.execute_at(13, resume).unwrap();
    let resumed_entries = read_dir_entries(resumed_completion.payload());
    let resumed_names: Vec<_> = resumed_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(resumed_names, ["..", "alpha.txt", "beta.txt"]);

    let too_small = decoded_request(VIRTIO_9P_TREADDIR, 5, p9_readdir_payload(1, 0, 1));
    let too_small_completion = device.execute_at(14, too_small).unwrap();
    assert_eq!(too_small_completion.message_type(), VIRTIO_9P_RREADDIR);
    assert!(read_counted_data(too_small_completion.payload()).is_empty());
}

#[test]
fn virtio_9p_device_rejects_readdir_on_stale_unopened_and_file_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap();

    let stale = decoded_request(VIRTIO_9P_TREADDIR, 1, p9_readdir_payload(7, 0, 128));
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();

    let unopened = decoded_request(VIRTIO_9P_TREADDIR, 3, p9_readdir_payload(1, 0, 128));
    let unopened_completion = device.execute_at(12, unopened).unwrap();
    assert_eq!(unopened_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(unopened_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"hello.txt"]));
    device.execute_at(13, walk).unwrap();
    let open_file = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(2, 0));
    device.execute_at(14, open_file).unwrap();

    let file = decoded_request(VIRTIO_9P_TREADDIR, 6, p9_readdir_payload(2, 0, 128));
    let file_completion = device.execute_at(15, file).unwrap();
    assert_eq!(file_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(file_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}
