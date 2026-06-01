use rem6_virtio::{
    Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_AT_REMOVEDIR, VIRTIO_9P_EBADF, VIRTIO_9P_ENOENT,
    VIRTIO_9P_ENOTEMPTY, VIRTIO_9P_GETATTR_BASIC, VIRTIO_9P_NOFID, VIRTIO_9P_OPEN_READ_WRITE,
    VIRTIO_9P_RGETATTR, VIRTIO_9P_RLCREATE, VIRTIO_9P_RLERROR, VIRTIO_9P_RLINK, VIRTIO_9P_RMKDIR,
    VIRTIO_9P_RREAD, VIRTIO_9P_RREMOVE, VIRTIO_9P_RRENAME, VIRTIO_9P_RRENAMEAT,
    VIRTIO_9P_RUNLINKAT, VIRTIO_9P_RWALK, VIRTIO_9P_TATTACH, VIRTIO_9P_TGETATTR,
    VIRTIO_9P_TLCREATE, VIRTIO_9P_TLINK, VIRTIO_9P_TLOPEN, VIRTIO_9P_TMKDIR, VIRTIO_9P_TREAD,
    VIRTIO_9P_TREADDIR, VIRTIO_9P_TREMOVE, VIRTIO_9P_TRENAME, VIRTIO_9P_TRENAMEAT,
    VIRTIO_9P_TSTATFS, VIRTIO_9P_TUNLINKAT, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

#[test]
fn virtio_9p_device_unlinks_named_files_from_root_directory() {
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
    device.execute_at(11, open_root).unwrap();

    let initial = decoded_request(VIRTIO_9P_TREADDIR, 3, p9_readdir_payload(1, 0, 512));
    let initial_completion = device.execute_at(12, initial).unwrap();
    let initial_entries = read_dir_entries(initial_completion.payload());
    let initial_names: Vec<_> = initial_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(initial_names, [".", "..", "alpha.txt", "beta.txt"]);

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    device.execute_at(13, walk_alpha).unwrap();
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(2, 0));
    device.execute_at(14, open_alpha).unwrap();

    let unlink = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        6,
        p9_unlinkat_payload(1, b"alpha.txt", 0),
    );
    let unlink_completion = device.execute_at(15, unlink).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);
    assert!(unlink_completion.payload().is_empty());

    let read_deleted = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 8));
    let read_completion = device.execute_at(16, read_deleted).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(read_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let after = decoded_request(VIRTIO_9P_TREADDIR, 8, p9_readdir_payload(1, 0, 512));
    let after_completion = device.execute_at(17, after).unwrap();
    let after_entries = read_dir_entries(after_completion.payload());
    let after_names: Vec<_> = after_entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(after_names, [".", "..", "beta.txt"]);

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    let removed_completion = device.execute_at(18, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_removes_file_fids_and_rejects_deleted_access() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"hello.txt"]));
    device.execute_at(11, walk).unwrap();
    let open = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    device.execute_at(12, open).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 4, p9_remove_payload(2));
    let remove_completion = device.execute_at(13, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());
    assert_eq!(device.fid_count(), 1);

    let read_after_remove = decoded_request(VIRTIO_9P_TREAD, 5, p9_read_payload(2, 0, 8));
    let read_completion = device.execute_at(14, read_after_remove).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(read_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"hello.txt"]));
    let removed_completion = device.execute_at(15, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_remove_deletes_empty_directory_fids() {
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
        p9_mkdir_payload(1, b"empty", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let walk = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"empty"]));
    device.execute_at(12, walk).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 4, p9_remove_payload(2));
    let remove_completion = device.execute_at(13, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());
    assert_eq!(device.fid_count(), 1);

    let stat_removed = decoded_request(VIRTIO_9P_TSTATFS, 5, p9_statfs_payload(2));
    let stat_completion = device.execute_at(14, stat_removed).unwrap();
    assert_eq!(stat_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stat_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"empty"]));
    let removed_completion = device.execute_at(15, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_remove_rejects_non_empty_directories() {
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
        p9_mkdir_payload(1, b"parent", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let remove_target = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"parent"]));
    device.execute_at(12, remove_target).unwrap();
    let create_parent = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"parent"]));
    device.execute_at(13, create_parent).unwrap();
    let create_child = decoded_request(
        VIRTIO_9P_TLCREATE,
        5,
        p9_lcreate_payload(3, b"child.txt", 0, 0o100644, 0),
    );
    device.execute_at(14, create_child).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 6, p9_remove_payload(2));
    let remove_completion = device.execute_at(15, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        remove_completion.payload(),
        VIRTIO_9P_ENOTEMPTY.to_le_bytes()
    );
    assert_eq!(device.fid_count(), 2);

    let stat_removed_fid = decoded_request(VIRTIO_9P_TSTATFS, 7, p9_statfs_payload(2));
    let stat_completion = device.execute_at(16, stat_removed_fid).unwrap();
    assert_eq!(stat_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stat_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let walk_parent = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"parent"]));
    let walk_completion = device.execute_at(17, walk_parent).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
}

#[test]
fn virtio_9p_device_remove_rejects_root_and_clunks_the_fid() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let remove = decoded_request(VIRTIO_9P_TREMOVE, 2, p9_remove_payload(1));
    let remove_completion = device.execute_at(11, remove).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(remove_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
    assert_eq!(device.fid_count(), 0);

    let stat_removed_fid = decoded_request(VIRTIO_9P_TSTATFS, 3, p9_statfs_payload(1));
    let stat_completion = device.execute_at(12, stat_removed_fid).unwrap();
    assert_eq!(stat_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stat_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_remove_and_unlinkat_on_missing_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("hello.txt", b"hello".to_vec())
        .unwrap();

    let remove_stale = decoded_request(VIRTIO_9P_TREMOVE, 1, p9_remove_payload(7));
    let remove_completion = device.execute_at(10, remove_stale).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(remove_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        2,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(11, attach).unwrap();

    let unlink_missing = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        3,
        p9_unlinkat_payload(1, b"missing.txt", 0),
    );
    let missing_completion = device.execute_at(12, unlink_missing).unwrap();
    assert_eq!(missing_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(missing_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let unlink_stale = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        4,
        p9_unlinkat_payload(7, b"hello.txt", 0),
    );
    let stale_completion = device.execute_at(13, unlink_stale).unwrap();
    assert_eq!(stale_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(stale_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());
}

#[test]
fn virtio_9p_device_unlinkat_removes_empty_directories_with_remove_dir_flag() {
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
        p9_mkdir_payload(1, b"empty", 0o040755, 0),
    );
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);

    let unlink = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        3,
        p9_unlinkat_payload(1, b"empty", VIRTIO_9P_AT_REMOVEDIR),
    );
    let unlink_completion = device.execute_at(12, unlink).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);
    assert!(unlink_completion.payload().is_empty());

    let walk_removed = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 2, &[b"empty"]));
    let removed_completion = device.execute_at(13, walk_removed).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_rejects_unlinkat_remove_dir_for_non_empty_directories() {
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
        p9_mkdir_payload(1, b"parent", 0o040755, 0),
    );
    device.execute_at(11, mkdir).unwrap();
    let walk_parent = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"parent"]));
    device.execute_at(12, walk_parent).unwrap();
    let create_child = decoded_request(
        VIRTIO_9P_TLCREATE,
        4,
        p9_lcreate_payload(2, b"child.txt", 0, 0o100644, 0),
    );
    device.execute_at(13, create_child).unwrap();

    let unlink = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        5,
        p9_unlinkat_payload(1, b"parent", VIRTIO_9P_AT_REMOVEDIR),
    );
    let unlink_completion = device.execute_at(14, unlink).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        unlink_completion.payload(),
        VIRTIO_9P_ENOTEMPTY.to_le_bytes()
    );

    let walk_parent = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"parent"]));
    let walk_completion = device.execute_at(15, walk_parent).unwrap();
    assert_eq!(walk_completion.message_type(), VIRTIO_9P_RWALK);
}

#[test]
fn virtio_9p_device_renames_root_files_preserving_open_fids() {
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
    device.execute_at(11, open_root).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let walk_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(walk_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        5,
        p9_renameat_payload(1, b"alpha.txt", 1, b"gamma.txt"),
    );
    let rename_completion = device.execute_at(14, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    let read_open = decoded_request(VIRTIO_9P_TREAD, 6, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(15, read_open).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 3, &[b"alpha.txt"]));
    let old_completion = device.execute_at(16, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"gamma.txt"]));
    let new_completion = device.execute_at(17, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, gamma_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(gamma_path, alpha_path);

    let readdir = decoded_request(VIRTIO_9P_TREADDIR, 9, p9_readdir_payload(1, 0, 512));
    let readdir_completion = device.execute_at(18, readdir).unwrap();
    let entries = read_dir_entries(readdir_completion.payload());
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".", "..", "beta.txt", "gamma.txt"]);
}

#[test]
fn virtio_9p_device_renameat_replaces_existing_root_files() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap()
        .with_file("beta.txt", b"beta".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 3, p9_lopen_payload(2, 0));
    device.execute_at(12, open_alpha).unwrap();

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    device.execute_at(13, walk_beta).unwrap();
    let open_beta = decoded_request(VIRTIO_9P_TLOPEN, 5, p9_lopen_payload(3, 0));
    device.execute_at(14, open_beta).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"alpha.txt", 1, b"beta.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);

    let read_replaced = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(3, 0, 16));
    let replaced_completion = device.execute_at(16, read_replaced).unwrap();
    assert_eq!(replaced_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(replaced_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let read_alpha_fid = decoded_request(VIRTIO_9P_TREAD, 8, p9_read_payload(2, 0, 16));
    let alpha_fid_completion = device.execute_at(17, read_alpha_fid).unwrap();
    assert_eq!(alpha_fid_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(alpha_fid_completion.payload()), b"alpha");

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(1, 4, &[b"beta.txt"]));
    let new_completion = device.execute_at(18, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, new_beta_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(new_beta_path, alpha_path);

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 10, p9_walk_payload(1, 5, &[b"alpha.txt"]));
    let old_completion = device.execute_at(19, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_renameat_moves_open_file_between_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
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

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 3, &[b"tmp"]));
    device.execute_at(14, walk_tmp).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"alpha.txt", 3, b"moved.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    let read_open = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(16, read_open).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let old_completion = device.execute_at(17, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(3, 5, &[b"moved.txt"]));
    let new_completion = device.execute_at(18, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, moved_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(moved_path, alpha_path);
}

#[test]
fn virtio_9p_device_renameat_moves_same_name_between_directories() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
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

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 3, &[b"tmp"]));
    device.execute_at(14, walk_tmp).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"alpha.txt", 3, b"alpha.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    let read_open = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(16, read_open).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let old_completion = device.execute_at(17, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(3, 5, &[b"alpha.txt"]));
    let new_completion = device.execute_at(18, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, moved_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(moved_path, alpha_path);
}

#[test]
fn virtio_9p_device_renames_open_file_fid_into_directory() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
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

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let walk_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(walk_completion.payload(), 2);
    let open_alpha = decoded_request(VIRTIO_9P_TLOPEN, 4, p9_lopen_payload(2, 0));
    device.execute_at(13, open_alpha).unwrap();
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 3, &[b"tmp"]));
    device.execute_at(14, walk_tmp).unwrap();

    let rename = decoded_request(
        VIRTIO_9P_TRENAME,
        6,
        p9_rename_payload(2, 3, b"renamed.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAME);
    assert!(rename_completion.payload().is_empty());

    let read_open = decoded_request(VIRTIO_9P_TREAD, 7, p9_read_payload(2, 0, 16));
    let read_completion = device.execute_at(16, read_open).unwrap();
    assert_eq!(read_completion.message_type(), VIRTIO_9P_RREAD);
    assert_eq!(read_counted_data(read_completion.payload()), b"alpha");

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let old_completion = device.execute_at(17, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(3, 5, &[b"renamed.txt"]));
    let new_completion = device.execute_at(18, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    let (_, _, renamed_path) = read_qid(new_completion.payload(), 2);
    assert_eq!(renamed_path, alpha_path);
}

#[test]
fn virtio_9p_device_renameat_renames_directory_and_updates_child_fid_paths() {
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
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);

    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp"]));
    device.execute_at(12, walk_tmp).unwrap();
    let create_child = decoded_request(
        VIRTIO_9P_TLCREATE,
        4,
        p9_lcreate_payload(
            2,
            b"child.txt",
            u32::from(VIRTIO_9P_OPEN_READ_WRITE),
            0o100644,
            0,
        ),
    );
    let create_completion = device.execute_at(13, create_child).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLCREATE);
    let (_, _, child_path) = read_qid(create_completion.payload(), 0);

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        5,
        p9_renameat_payload(1, b"tmp", 1, b"renamed"),
    );
    let rename_completion = device.execute_at(14, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 3, &[b"tmp"]));
    let old_completion = device.execute_at(15, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(
        VIRTIO_9P_TWALK,
        7,
        p9_walk_payload(1, 4, &[b"renamed", b"child.txt"]),
    );
    let new_completion = device.execute_at(16, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(new_completion.payload(), 15).2, child_path);

    let remove_child = decoded_request(VIRTIO_9P_TREMOVE, 8, p9_remove_payload(2));
    let remove_completion = device.execute_at(17, remove_child).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);

    let walk_renamed = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(1, 5, &[b"renamed"]));
    let renamed_completion = device.execute_at(18, walk_renamed).unwrap();
    assert_eq!(renamed_completion.message_type(), VIRTIO_9P_RWALK);

    let removed_walk = decoded_request(VIRTIO_9P_TWALK, 10, p9_walk_payload(5, 6, &[b"child.txt"]));
    let removed_completion = device.execute_at(19, removed_walk).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_trename_renames_directory_and_updates_child_fid_paths() {
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
    let mkdir_completion = device.execute_at(11, mkdir).unwrap();
    assert_eq!(mkdir_completion.message_type(), VIRTIO_9P_RMKDIR);

    let walk_tmp_for_create = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"tmp"]));
    device.execute_at(12, walk_tmp_for_create).unwrap();
    let create_child = decoded_request(
        VIRTIO_9P_TLCREATE,
        4,
        p9_lcreate_payload(
            2,
            b"child.txt",
            u32::from(VIRTIO_9P_OPEN_READ_WRITE),
            0o100644,
            0,
        ),
    );
    let create_completion = device.execute_at(13, create_child).unwrap();
    assert_eq!(create_completion.message_type(), VIRTIO_9P_RLCREATE);
    let (_, _, child_path) = read_qid(create_completion.payload(), 0);

    let walk_tmp_for_rename = decoded_request(VIRTIO_9P_TWALK, 5, p9_walk_payload(1, 3, &[b"tmp"]));
    device.execute_at(14, walk_tmp_for_rename).unwrap();
    let rename = decoded_request(VIRTIO_9P_TRENAME, 6, p9_rename_payload(3, 1, b"renamed"));
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAME);
    assert!(rename_completion.payload().is_empty());

    let old_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"tmp"]));
    let old_completion = device.execute_at(16, old_walk).unwrap();
    assert_eq!(old_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(old_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());

    let new_walk = decoded_request(
        VIRTIO_9P_TWALK,
        8,
        p9_walk_payload(1, 5, &[b"renamed", b"child.txt"]),
    );
    let new_completion = device.execute_at(17, new_walk).unwrap();
    assert_eq!(new_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(new_completion.payload(), 15).2, child_path);

    let remove_child = decoded_request(VIRTIO_9P_TREMOVE, 9, p9_remove_payload(2));
    let remove_completion = device.execute_at(18, remove_child).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);

    let walk_renamed = decoded_request(VIRTIO_9P_TWALK, 10, p9_walk_payload(1, 6, &[b"renamed"]));
    let renamed_completion = device.execute_at(19, walk_renamed).unwrap();
    assert_eq!(renamed_completion.message_type(), VIRTIO_9P_RWALK);

    let removed_walk = decoded_request(VIRTIO_9P_TWALK, 11, p9_walk_payload(6, 7, &[b"child.txt"]));
    let removed_completion = device.execute_at(20, removed_walk).unwrap();
    assert_eq!(removed_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(removed_completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_device_renameat_keeps_hardlinked_same_file_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
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

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 3, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(12, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let walk_tmp = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"tmp"]));
    device.execute_at(13, walk_tmp).unwrap();
    let link = decoded_request(VIRTIO_9P_TLINK, 5, p9_link_payload(3, 2, b"alpha.txt"));
    let link_completion = device.execute_at(14, link).unwrap();
    assert_eq!(link_completion.message_type(), VIRTIO_9P_RLINK);

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"alpha.txt", 3, b"alpha.txt"),
    );
    let rename_completion = device.execute_at(15, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);

    let root_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let root_completion = device.execute_at(16, root_walk).unwrap();
    assert_eq!(root_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(root_completion.payload(), 2).2, alpha_path);

    let tmp_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(3, 5, &[b"alpha.txt"]));
    let tmp_completion = device.execute_at(17, tmp_walk).unwrap();
    assert_eq!(tmp_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(tmp_completion.payload(), 2).2, alpha_path);

    let getattr = decoded_request(
        VIRTIO_9P_TGETATTR,
        9,
        p9_getattr_payload(2, VIRTIO_9P_GETATTR_BASIC),
    );
    let getattr_completion = device.execute_at(18, getattr).unwrap();
    assert_eq!(getattr_completion.message_type(), VIRTIO_9P_RGETATTR);
    assert_eq!(read_u64(getattr_completion.payload(), 33), 2);
}

#[test]
fn virtio_9p_device_remove_deletes_walked_hardlink_entry() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    let link_completion = device.execute_at(12, link).unwrap();
    assert_eq!(link_completion.message_type(), VIRTIO_9P_RLINK);

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(read_qid(beta_completion.payload(), 2).2, alpha_path);

    let remove_beta = decoded_request(VIRTIO_9P_TREMOVE, 5, p9_remove_payload(3));
    let remove_completion = device.execute_at(14, remove_beta).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());

    let beta_walk = decoded_request(VIRTIO_9P_TWALK, 6, p9_walk_payload(1, 4, &[b"beta.txt"]));
    let beta_walk_completion = device.execute_at(15, beta_walk).unwrap();
    assert_eq!(beta_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        beta_walk_completion.payload(),
        VIRTIO_9P_ENOENT.to_le_bytes()
    );

    let alpha_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 5, &[b"alpha.txt"]));
    let alpha_walk_completion = device.execute_at(16, alpha_walk).unwrap();
    assert_eq!(alpha_walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(alpha_walk_completion.payload(), 2).2, alpha_path);
}

#[test]
fn virtio_9p_device_remove_rejects_unlinked_hardlink_fids_without_removing_survivors() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    assert_eq!(
        device.execute_at(12, link).unwrap().message_type(),
        VIRTIO_9P_RLINK
    );

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(read_qid(beta_completion.payload(), 2).2, alpha_path);

    let unlink_beta = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        5,
        p9_unlinkat_payload(1, b"beta.txt", 0),
    );
    let unlink_completion = device.execute_at(14, unlink_beta).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);

    let remove_beta_fid = decoded_request(VIRTIO_9P_TREMOVE, 6, p9_remove_payload(3));
    let remove_completion = device.execute_at(15, remove_beta_fid).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(remove_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let beta_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"beta.txt"]));
    let beta_walk_completion = device.execute_at(16, beta_walk).unwrap();
    assert_eq!(beta_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        beta_walk_completion.payload(),
        VIRTIO_9P_ENOENT.to_le_bytes()
    );

    let alpha_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 5, &[b"alpha.txt"]));
    let alpha_walk_completion = device.execute_at(17, alpha_walk).unwrap();
    assert_eq!(alpha_walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(alpha_walk_completion.payload(), 2).2, alpha_path);

    let removed_fid_read = decoded_request(VIRTIO_9P_TREAD, 9, p9_read_payload(3, 0, 16));
    let removed_fid_completion = device.execute_at(18, removed_fid_read).unwrap();
    assert_eq!(removed_fid_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        removed_fid_completion.payload(),
        VIRTIO_9P_EBADF.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_remove_tracks_renamed_hardlink_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    let link_completion = device.execute_at(12, link).unwrap();
    assert_eq!(link_completion.message_type(), VIRTIO_9P_RLINK);

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(read_qid(beta_completion.payload(), 2).2, alpha_path);

    let rename_beta = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        5,
        p9_renameat_payload(1, b"beta.txt", 1, b"gamma.txt"),
    );
    let rename_completion = device.execute_at(14, rename_beta).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);

    let remove_gamma_fid = decoded_request(VIRTIO_9P_TREMOVE, 6, p9_remove_payload(3));
    let remove_completion = device.execute_at(15, remove_gamma_fid).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());

    let gamma_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"gamma.txt"]));
    let gamma_walk_completion = device.execute_at(16, gamma_walk).unwrap();
    assert_eq!(gamma_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        gamma_walk_completion.payload(),
        VIRTIO_9P_ENOENT.to_le_bytes()
    );

    let alpha_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 5, &[b"alpha.txt"]));
    let alpha_walk_completion = device.execute_at(17, alpha_walk).unwrap();
    assert_eq!(alpha_walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(alpha_walk_completion.payload(), 2).2, alpha_path);
}

#[test]
fn virtio_9p_device_remove_tracks_trename_hardlink_fids() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    let link_completion = device.execute_at(12, link).unwrap();
    assert_eq!(link_completion.message_type(), VIRTIO_9P_RLINK);

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(read_qid(beta_completion.payload(), 2).2, alpha_path);

    let rename_beta = decoded_request(VIRTIO_9P_TRENAME, 5, p9_rename_payload(3, 1, b"gamma.txt"));
    let rename_completion = device.execute_at(14, rename_beta).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAME);

    let remove_gamma_fid = decoded_request(VIRTIO_9P_TREMOVE, 6, p9_remove_payload(3));
    let remove_completion = device.execute_at(15, remove_gamma_fid).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    assert!(remove_completion.payload().is_empty());

    let beta_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"beta.txt"]));
    let beta_walk_completion = device.execute_at(16, beta_walk).unwrap();
    assert_eq!(beta_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        beta_walk_completion.payload(),
        VIRTIO_9P_ENOENT.to_le_bytes()
    );

    let gamma_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 5, &[b"gamma.txt"]));
    let gamma_walk_completion = device.execute_at(17, gamma_walk).unwrap();
    assert_eq!(gamma_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        gamma_walk_completion.payload(),
        VIRTIO_9P_ENOENT.to_le_bytes()
    );

    let alpha_walk = decoded_request(VIRTIO_9P_TWALK, 9, p9_walk_payload(1, 6, &[b"alpha.txt"]));
    let alpha_walk_completion = device.execute_at(18, alpha_walk).unwrap();
    assert_eq!(alpha_walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(alpha_walk_completion.payload(), 2).2, alpha_path);
}

#[test]
fn virtio_9p_device_trename_rejects_unlinked_hardlink_fids_without_renaming_survivors() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    assert_eq!(
        device.execute_at(12, link).unwrap().message_type(),
        VIRTIO_9P_RLINK
    );

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    let beta_completion = device.execute_at(13, walk_beta).unwrap();
    assert_eq!(read_qid(beta_completion.payload(), 2).2, alpha_path);

    let unlink_beta = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        5,
        p9_unlinkat_payload(1, b"beta.txt", 0),
    );
    let unlink_completion = device.execute_at(14, unlink_beta).unwrap();
    assert_eq!(unlink_completion.message_type(), VIRTIO_9P_RUNLINKAT);

    let rename_beta_fid =
        decoded_request(VIRTIO_9P_TRENAME, 6, p9_rename_payload(3, 1, b"gamma.txt"));
    let rename_completion = device.execute_at(15, rename_beta_fid).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(rename_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let alpha_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let alpha_walk_completion = device.execute_at(16, alpha_walk).unwrap();
    assert_eq!(alpha_walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(alpha_walk_completion.payload(), 2).2, alpha_path);

    let gamma_walk = decoded_request(VIRTIO_9P_TWALK, 8, p9_walk_payload(1, 5, &[b"gamma.txt"]));
    let gamma_walk_completion = device.execute_at(17, gamma_walk).unwrap();
    assert_eq!(gamma_walk_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(
        gamma_walk_completion.payload(),
        VIRTIO_9P_ENOENT.to_le_bytes()
    );
}

#[test]
fn virtio_9p_device_trename_rejects_unlinked_hardlink_fids_to_surviving_targets() {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap())
        .with_file("alpha.txt", b"alpha".to_vec())
        .unwrap();
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();

    let walk_alpha = decoded_request(VIRTIO_9P_TWALK, 2, p9_walk_payload(1, 2, &[b"alpha.txt"]));
    let alpha_completion = device.execute_at(11, walk_alpha).unwrap();
    let (_, _, alpha_path) = read_qid(alpha_completion.payload(), 2);
    let link = decoded_request(VIRTIO_9P_TLINK, 3, p9_link_payload(1, 2, b"beta.txt"));
    assert_eq!(
        device.execute_at(12, link).unwrap().message_type(),
        VIRTIO_9P_RLINK
    );

    let walk_beta = decoded_request(VIRTIO_9P_TWALK, 4, p9_walk_payload(1, 3, &[b"beta.txt"]));
    assert_eq!(
        read_qid(device.execute_at(13, walk_beta).unwrap().payload(), 2).2,
        alpha_path
    );

    let unlink_beta = decoded_request(
        VIRTIO_9P_TUNLINKAT,
        5,
        p9_unlinkat_payload(1, b"beta.txt", 0),
    );
    assert_eq!(
        device.execute_at(14, unlink_beta).unwrap().message_type(),
        VIRTIO_9P_RUNLINKAT
    );

    let rename_beta_to_alpha =
        decoded_request(VIRTIO_9P_TRENAME, 6, p9_rename_payload(3, 1, b"alpha.txt"));
    let rename_completion = device.execute_at(15, rename_beta_to_alpha).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(rename_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    let alpha_walk = decoded_request(VIRTIO_9P_TWALK, 7, p9_walk_payload(1, 4, &[b"alpha.txt"]));
    let alpha_walk_completion = device.execute_at(16, alpha_walk).unwrap();
    assert_eq!(alpha_walk_completion.message_type(), VIRTIO_9P_RWALK);
    assert_eq!(read_qid(alpha_walk_completion.payload(), 2).2, alpha_path);
}
