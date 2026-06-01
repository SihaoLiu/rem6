use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_DTCHR, VIRTIO_9P_DTDIR, VIRTIO_9P_DTSYMLINK,
    VIRTIO_9P_EBADF, VIRTIO_9P_EEXIST, VIRTIO_9P_GETATTR_BASIC, VIRTIO_9P_NOFID,
    VIRTIO_9P_OPEN_READ_WRITE, VIRTIO_9P_QTDIR, VIRTIO_9P_QTFILE, VIRTIO_9P_QTSYMLINK,
    VIRTIO_9P_RGETATTR, VIRTIO_9P_RLCREATE, VIRTIO_9P_RLERROR, VIRTIO_9P_RMKDIR, VIRTIO_9P_RMKNOD,
    VIRTIO_9P_RREAD, VIRTIO_9P_RREADLINK, VIRTIO_9P_RSYMLINK, VIRTIO_9P_RWALK, VIRTIO_9P_RWRITE,
    VIRTIO_9P_TATTACH, VIRTIO_9P_TGETATTR, VIRTIO_9P_TLCREATE, VIRTIO_9P_TLOPEN, VIRTIO_9P_TMKDIR,
    VIRTIO_9P_TMKNOD, VIRTIO_9P_TREAD, VIRTIO_9P_TREADDIR, VIRTIO_9P_TREADLINK, VIRTIO_9P_TSYMLINK,
    VIRTIO_9P_TWALK, VIRTIO_9P_TWRITE,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_creates_walks_and_reads_symlinks() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("target.txt", b"target data".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let symlink = decoded_request(
        VIRTIO_9P_TSYMLINK,
        2,
        p9_symlink_payload(1, b"target.link", b"target.txt", 0),
    );
    let symlink_completion = device.execute_at(11, symlink).unwrap();
    assert_eq!(symlink_completion.message_type(), VIRTIO_9P_RSYMLINK);
    let (symlink_qtype, symlink_version, symlink_path) = read_qid(symlink_completion.payload(), 0);
    assert_eq!(symlink_qtype, VIRTIO_9P_QTSYMLINK);
    assert_eq!(symlink_version, 0);
    assert_ne!(symlink_path, 1);

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"target.link"]));
    let walk_completion = device.execute_at(12, walk).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (walk_qtype, _, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTSYMLINK);
    assert_eq!(walk_path, symlink_path);

    let readlink = decoded_request(VIRTIO_9P_TREADLINK, 4, p9_readlink_payload(2));
    let readlink_completion = device.execute_at(13, readlink).unwrap();
    assert_eq!(readlink_completion.message_type(), VIRTIO_9P_RREADLINK);
    assert_eq!(readlink_completion.payload(), p9_string(b"target.txt"));

    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(1, 0));
    device.execute_at(14, open_root).unwrap();
    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 6, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(15, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let link_entry = entries
        .iter()
        .find(|entry| entry.name == "target.link")
        .unwrap();
    assert_eq!(link_entry.qtype, VIRTIO_9P_QTSYMLINK);
    assert_eq!(link_entry.qpath, symlink_path);
    assert_eq!(link_entry.dtype, VIRTIO_9P_DTSYMLINK);
}

#[test]
fn virtio_9p_device_rejects_stale_or_non_symlink_readlink_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let stale = decoded_request(VIRTIO_9P_TREADLINK, 1, p9_readlink_payload(7));
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let root = decoded_request(VIRTIO_9P_TREADLINK, 3, p9_readlink_payload(1));
    let root_completion = device.execute_at(12, root).unwrap();
    assert_eq!(root_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(root_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_makes_root_directories_and_walks_into_them() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 2, p9_lopen_payload(1, 0));
    device.execute_at(11, open_root).unwrap();

    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        3,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    let mkdir_completion = device.execute_at(12, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);
    let (mkdir_qtype, mkdir_version, mkdir_path) = read_qid(mkdir_completion.payload(), 0);
    assert_eq!(mkdir_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(mkdir_version, 0);
    assert_ne!(mkdir_path, 1);

    let root_readdir = decoded_request(VIRTIO_9P_TREADDIR, 4, p9_readdir_payload(1, 0, 512));
    let root_completion = device.execute_at(13, root_readdir).unwrap();
    let root_entries = read_dir_entries(root_completion.payload());
    let root_names: Vec<_> = root_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(root_names, [".", "..", "alpha.txt", "tmp"]);
    let tmp_entry = root_entries
        .iter()
        .find(|entry| entry.name == "tmp")
        .unwrap();
    assert_eq!(tmp_entry.qtype, VIRTIO_9P_QTDIR);
    assert_eq!(tmp_entry.qpath, mkdir_path);
    assert_eq!(tmp_entry.dtype, VIRTIO_9P_DTDIR);

    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 2, &[b"tmp"]));
    let walk_completion = device.execute_at(14, walk_tmp).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (walk_qtype, _, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(walk_path, mkdir_path);

    let getattr_tmp = decoded_request(
        VIRTIO_9P_TGETATTR,
        6,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(15, getattr_tmp).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    let (getattr_qtype, _, getattr_path) = read_qid(getattr_completion.payload(), 8);
    assert_eq!(getattr_qtype, VIRTIO_9P_QTDIR);
    assert_eq!(getattr_path, mkdir_path);
    assert_eq!(read_u32(getattr_completion.payload(), 21), 0o040755);

    let open_tmp = decoded_request(VIRTIO_9P_TLOPEN, 7, p9_lopen_payload(2, 0));
    device.execute_at(16, open_tmp).unwrap();
    let tmp_readdir = decoded_request(VIRTIO_9P_TREADDIR, 8, p9_readdir_payload(2, 0, 512));
    let tmp_completion = device.execute_at(17, tmp_readdir).unwrap();
    let tmp_entries = read_dir_entries(tmp_completion.payload());
    let tmp_names: Vec<_> = tmp_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(tmp_names, [".", ".."]);
}

#[test]
fn virtio_9p_device_mknod_creates_lists_and_reports_character_devices() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let mknod = decoded_request(
        VIRTIO_9P_TMKNOD,
        2,
        p9_mknod_payload(1, b"null", 0o020666, 1, 3, 0),
    );
    let mknod_completion = device.execute_at(11, mknod).unwrap();
    assert_eq!(mknod_completion.message_type(), VIRTIO_9P_RMKNOD);
    let (mknod_qtype, mknod_version, mknod_path) = read_qid(mknod_completion.payload(), 0);
    assert_eq!(mknod_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(mknod_version, 0);
    assert_ne!(mknod_path, 1);

    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"null"]));
    let walk_completion = device.execute_at(12, walk).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (walk_qtype, _, walk_path) = read_qid(walk_completion.payload(), 2);
    assert_eq!(walk_qtype, VIRTIO_9P_QTFILE);
    assert_eq!(walk_path, mknod_path);

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        4,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(13, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(read_u32(getattr_completion.payload(), 21), 0o020666);
    assert_eq!(read_u64(getattr_completion.payload(), 33), 1);
    assert_eq!(read_u64(getattr_completion.payload(), 41), 0x103);
    assert_eq!(read_u64(getattr_completion.payload(), 49), 0);

    let open_root = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(1, 0));
    device.execute_at(14, open_root).unwrap();
    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 6, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(15, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let null_entry = entries.iter().find(|entry| entry.name == "null").unwrap();
    assert_eq!(null_entry.qtype, VIRTIO_9P_QTFILE);
    assert_eq!(null_entry.qpath, mknod_path);
    assert_eq!(null_entry.dtype, VIRTIO_9P_DTCHR);

    let open_null = decoded_request(VIRTIO_9P_TLOPEN, 7, p9_lopen_payload(2, 0));
    device.execute_at(16, open_null).unwrap();
    let read_null = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(2, 0, 8));
    let read_completion = device.execute_at(17, read_null).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(read_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_mknod_on_invalid_parents_and_duplicates() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let stale = decoded_request(
        VIRTIO_9P_TMKNOD,
        1,
        p9_mknod_payload(7, b"null", 0o020666, 1, 3, 0),
    );
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let duplicate = decoded_request(
        VIRTIO_9P_TMKNOD,
        3,
        p9_mknod_payload(1, b"alpha.txt", 0o020666, 1, 3, 0),
    );
    let duplicate_completion = device.execute_at(12, duplicate).unwrap();
    assert_eq!(duplicate_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        duplicate_completion.payload(),
        VIRTIO_9P_EEXIST.to_le_bytes()
    );

    let walk_file = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(13, walk_file).unwrap();
    let file_parent = decoded_request(
        VIRTIO_9P_TMKNOD,
        5,
        p9_mknod_payload(2, b"null", 0o020666, 1, 3, 0),
    );
    let file_parent_completion = device.execute_at(14, file_parent).unwrap();
    assert_eq!(file_parent_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        file_parent_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_creates_reads_and_lists_files_inside_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        2,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp"]));
    device.execute_at(12, walk_tmp).unwrap();

    let create = decoded_request(
        VIRTIO_9P_TLCREATE,
        4,
        p9_lcreate_payload(
            2,
            b"note.txt",
            u32::from(VIRTIO_9P_OPEN_READ_WRITE),
            0o100644,
            0,
        ),
    );
    let create_completion = device.execute_at(13, create).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLCREATE);
    let (created_qtype, _, created_path) = read_qid(create_completion.payload(), 0);
    assert_eq!(created_qtype, VIRTIO_9P_QTFILE);

    let write = decoded_request(VIRTIO_9P_TWRITE, 5, p9_write_payload(2, 0, b"inside"));
    let write_completion = device.execute_at(14, write).unwrap();
    assert_eq!(write_completion.message_type(), VIRTIO_9P_RWRITE);
    assert_eq!(write_completion.payload(), 6_u32.to_le_bytes());

    let read = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(15, read).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"inside");

    let attach_root = decoded_request(
        VIRTIO_9P_TATTACH,
        7,
        p9_attach_payload(10, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(16, attach_root).unwrap();
    let walk_file = decoded_request(
        VIRTIO_9P_TWALK,
        8,
        p9_walk_payload(10, 3, &[b"tmp", b"note.txt"]),
    );
    let walk_completion = device.execute_at(17, walk_file).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, walked_path) = read_qid(walk_completion.payload(), 15);
    assert_eq!(walked_path, created_path);

    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(10, 4, &[b"tmp"]));
    device.execute_at(18, walk_tmp).unwrap();
    let open_tmp = decoded_request(VIRTIO_9P_TLOPEN, 10, p9_lopen_payload(4, 0));
    device.execute_at(19, open_tmp).unwrap();
    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 11, p9_readdir_payload(4, 0, 512));
    let readdir_completion = device.execute_at(20, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".", "..", "note.txt"]);
}

#[test]
fn virtio_9p_device_rejects_mkdir_on_stale_file_and_duplicate_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("plain.txt", b"plain".to_vec())
        .unwrap();

    let stale = decoded_request(
        VIRTIO_9P_TMKDIR,
        1,
        p9_mkdir_payload(7, b"tmp", 0o040755, 0),
    );
    let stale_completion = device.execute_at(10, stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        3,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    device.execute_at(12, mkdir).unwrap();
    let duplicate = decoded_request(
        VIRTIO_9P_TMKDIR,
        4,
        p9_mkdir_payload(1, b"tmp", 0o040755, 0),
    );
    let duplicate_completion = device.execute_at(13, duplicate).unwrap();
    assert_eq!(duplicate_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        duplicate_completion.payload(),
        VIRTIO_9P_EEXIST.to_le_bytes()
    );

    let walk_file = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 2, &[b"plain.txt"]));
    device.execute_at(14, walk_file).unwrap();
    let mkdir_under_file = decoded_request(
        VIRTIO_9P_TMKDIR,
        6,
        p9_mkdir_payload(2, b"child", 0o040755, 0),
    );
    let file_parent_completion = device.execute_at(15, mkdir_under_file).unwrap();
    assert_eq!(file_parent_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        file_parent_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}
