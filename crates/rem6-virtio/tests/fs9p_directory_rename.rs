use rem6_virtio::{
    Virtio9pCompletion, Virtio9pConfig, Virtio9pDevice, VIRTIO_9P_EBADF, VIRTIO_9P_EINVAL,
    VIRTIO_9P_ENOENT, VIRTIO_9P_ENOTEMPTY, VIRTIO_9P_NOFID, VIRTIO_9P_OPEN_READ_WRITE,
    VIRTIO_9P_RLCREATE, VIRTIO_9P_RLERROR, VIRTIO_9P_RMKDIR, VIRTIO_9P_RREMOVE, VIRTIO_9P_RRENAME,
    VIRTIO_9P_RRENAMEAT, VIRTIO_9P_RWALK, VIRTIO_9P_TATTACH, VIRTIO_9P_TLCREATE, VIRTIO_9P_TLOPEN,
    VIRTIO_9P_TMKDIR, VIRTIO_9P_TREMOVE, VIRTIO_9P_TRENAME, VIRTIO_9P_TRENAMEAT, VIRTIO_9P_TWALK,
};

mod support;

use support::fs9p::*;

fn attached_device() -> Virtio9pDevice {
    let device = Virtio9pDevice::new(Virtio9pConfig::new("rem6share").unwrap());
    let attach = decoded_request(
        VIRTIO_9P_TATTACH,
        1,
        p9_attach_payload(1, VIRTIO_9P_NOFID, b"root", b"", 0),
    );
    device.execute_at(10, attach).unwrap();
    device
}

fn mkdir(device: &Virtio9pDevice, tick: u64, tag: u16, dfid: u32, name: &[u8]) {
    let mkdir = decoded_request(
        VIRTIO_9P_TMKDIR,
        tag,
        p9_mkdir_payload(dfid, name, 0o040755, 0),
    );
    let completion = device.execute_at(tick, mkdir).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RMKDIR);
}

fn walk(
    device: &Virtio9pDevice,
    tick: u64,
    tag: u16,
    fid: u32,
    newfid: u32,
    names: &[&[u8]],
) -> Virtio9pCompletion {
    let walk = decoded_request(VIRTIO_9P_TWALK, tag, p9_walk_payload(fid, newfid, names));
    let completion = device.execute_at(tick, walk).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RWALK);
    completion
}

fn create_child(device: &Virtio9pDevice, tick: u64, tag: u16, dirfid: u32) -> u64 {
    let create = decoded_request(
        VIRTIO_9P_TLCREATE,
        tag,
        p9_lcreate_payload(
            dirfid,
            b"child.txt",
            u32::from(VIRTIO_9P_OPEN_READ_WRITE),
            0o100644,
            0,
        ),
    );
    let completion = device.execute_at(tick, create).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLCREATE);
    read_qid(completion.payload(), 0).2
}

fn assert_walk_missing(
    device: &Virtio9pDevice,
    tick: u64,
    tag: u16,
    fid: u32,
    newfid: u32,
    name: &[u8],
) {
    let walk = decoded_request(VIRTIO_9P_TWALK, tag, p9_walk_payload(fid, newfid, &[name]));
    let completion = device.execute_at(tick, walk).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOENT.to_le_bytes());
}

#[test]
fn virtio_9p_renameat_replaces_empty_directory_target_and_invalidates_replaced_fid() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    mkdir(&device, 12, 3, 1, b"dst");
    walk(&device, 13, 4, 1, 2, &[b"src"]);
    walk(&device, 14, 5, 1, 3, &[b"dst"]);
    let child_path = create_child(&device, 15, 6, 2);

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        7,
        p9_renameat_payload(1, b"src", 1, b"dst"),
    );
    let rename_completion = device.execute_at(16, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    let open_replaced = decoded_request(VIRTIO_9P_TLOPEN, 8, p9_lopen_payload(3, 0));
    let replaced_completion = device.execute_at(17, open_replaced).unwrap();
    assert_eq!(replaced_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(replaced_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    assert_walk_missing(&device, 18, 9, 1, 4, b"src");
    let moved_child = walk(&device, 19, 10, 1, 5, &[b"dst", b"child.txt"]);
    assert_eq!(read_qid(moved_child.payload(), 15).2, child_path);

    let remove_child = decoded_request(VIRTIO_9P_TREMOVE, 11, p9_remove_payload(2));
    let remove_completion = device.execute_at(20, remove_child).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    walk(&device, 21, 12, 1, 6, &[b"dst"]);
    assert_walk_missing(&device, 22, 13, 6, 7, b"child.txt");
}

#[test]
fn virtio_9p_trename_replaces_empty_directory_target_and_invalidates_replaced_fid() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    mkdir(&device, 12, 3, 1, b"dst");
    walk(&device, 13, 4, 1, 2, &[b"src"]);
    walk(&device, 14, 5, 1, 3, &[b"dst"]);
    let child_path = create_child(&device, 15, 6, 2);
    walk(&device, 16, 7, 1, 4, &[b"src"]);

    let rename = decoded_request(VIRTIO_9P_TRENAME, 8, p9_rename_payload(4, 1, b"dst"));
    let rename_completion = device.execute_at(17, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAME);
    assert!(rename_completion.payload().is_empty());

    let open_replaced = decoded_request(VIRTIO_9P_TLOPEN, 9, p9_lopen_payload(3, 0));
    let replaced_completion = device.execute_at(18, open_replaced).unwrap();
    assert_eq!(replaced_completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(replaced_completion.payload(), VIRTIO_9P_EBADF.to_le_bytes());

    assert_walk_missing(&device, 19, 10, 1, 5, b"src");
    let moved_child = walk(&device, 20, 11, 1, 6, &[b"dst", b"child.txt"]);
    assert_eq!(read_qid(moved_child.payload(), 15).2, child_path);

    let remove_child = decoded_request(VIRTIO_9P_TREMOVE, 12, p9_remove_payload(2));
    let remove_completion = device.execute_at(21, remove_child).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    walk(&device, 22, 13, 1, 7, &[b"dst"]);
    assert_walk_missing(&device, 23, 14, 7, 8, b"child.txt");
}

#[test]
fn virtio_9p_renameat_rejects_directory_rename_over_non_empty_target() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    mkdir(&device, 12, 3, 1, b"dst");
    walk(&device, 13, 4, 1, 2, &[b"dst"]);
    let child_path = create_child(&device, 14, 5, 2);

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"src", 1, b"dst"),
    );
    let completion = device.execute_at(15, rename).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTEMPTY.to_le_bytes());

    walk(&device, 16, 7, 1, 3, &[b"src"]);
    let target_child = walk(&device, 17, 8, 1, 4, &[b"dst", b"child.txt"]);
    assert_eq!(read_qid(target_child.payload(), 15).2, child_path);
}

#[test]
fn virtio_9p_trename_rejects_directory_rename_over_non_empty_target() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    mkdir(&device, 12, 3, 1, b"dst");
    walk(&device, 13, 4, 1, 2, &[b"dst"]);
    let child_path = create_child(&device, 14, 5, 2);
    walk(&device, 15, 6, 1, 3, &[b"src"]);

    let rename = decoded_request(VIRTIO_9P_TRENAME, 7, p9_rename_payload(3, 1, b"dst"));
    let completion = device.execute_at(16, rename).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_ENOTEMPTY.to_le_bytes());

    walk(&device, 17, 8, 1, 4, &[b"src"]);
    let target_child = walk(&device, 18, 9, 1, 5, &[b"dst", b"child.txt"]);
    assert_eq!(read_qid(target_child.payload(), 15).2, child_path);
}

#[test]
fn virtio_9p_renameat_moves_directory_between_parents_and_updates_child_fid_path() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    mkdir(&device, 12, 3, 1, b"parent");
    walk(&device, 13, 4, 1, 2, &[b"src"]);
    walk(&device, 14, 5, 1, 3, &[b"parent"]);
    let child_path = create_child(&device, 15, 6, 2);

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        7,
        p9_renameat_payload(1, b"src", 3, b"moved"),
    );
    let rename_completion = device.execute_at(16, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAMEAT);
    assert!(rename_completion.payload().is_empty());

    assert_walk_missing(&device, 17, 8, 1, 4, b"src");
    let moved_child = walk(&device, 18, 9, 1, 5, &[b"parent", b"moved", b"child.txt"]);
    assert_eq!(read_qid(moved_child.payload(), 28).2, child_path);

    let remove_child = decoded_request(VIRTIO_9P_TREMOVE, 10, p9_remove_payload(2));
    let remove_completion = device.execute_at(19, remove_child).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    walk(&device, 20, 11, 1, 6, &[b"parent", b"moved"]);
    assert_walk_missing(&device, 21, 12, 6, 7, b"child.txt");
}

#[test]
fn virtio_9p_trename_moves_directory_between_parents_and_updates_child_fid_path() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    mkdir(&device, 12, 3, 1, b"parent");
    walk(&device, 13, 4, 1, 2, &[b"src"]);
    walk(&device, 14, 5, 1, 3, &[b"parent"]);
    let child_path = create_child(&device, 15, 6, 2);
    walk(&device, 16, 7, 1, 4, &[b"src"]);

    let rename = decoded_request(VIRTIO_9P_TRENAME, 8, p9_rename_payload(4, 3, b"moved"));
    let rename_completion = device.execute_at(17, rename).unwrap();
    assert_eq!(rename_completion.message_type(), VIRTIO_9P_RRENAME);
    assert!(rename_completion.payload().is_empty());

    assert_walk_missing(&device, 18, 9, 1, 5, b"src");
    let moved_child = walk(&device, 19, 10, 1, 6, &[b"parent", b"moved", b"child.txt"]);
    assert_eq!(read_qid(moved_child.payload(), 28).2, child_path);

    let remove_child = decoded_request(VIRTIO_9P_TREMOVE, 11, p9_remove_payload(2));
    let remove_completion = device.execute_at(20, remove_child).unwrap();
    assert_eq!(remove_completion.message_type(), VIRTIO_9P_RREMOVE);
    walk(&device, 21, 12, 1, 7, &[b"parent", b"moved"]);
    assert_walk_missing(&device, 22, 13, 7, 8, b"child.txt");
}

#[test]
fn virtio_9p_renameat_rejects_directory_move_into_descendant() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    walk(&device, 12, 3, 1, 2, &[b"src"]);
    mkdir(&device, 13, 4, 2, b"child");
    walk(&device, 14, 5, 1, 3, &[b"src", b"child"]);

    let rename = decoded_request(
        VIRTIO_9P_TRENAMEAT,
        6,
        p9_renameat_payload(1, b"src", 3, b"moved"),
    );
    let completion = device.execute_at(15, rename).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_EINVAL.to_le_bytes());

    walk(&device, 16, 7, 1, 4, &[b"src"]);
    walk(&device, 17, 8, 1, 5, &[b"src", b"child"]);
    assert_walk_missing(&device, 18, 9, 3, 6, b"moved");
}

#[test]
fn virtio_9p_trename_rejects_directory_move_into_descendant() {
    let device = attached_device();
    mkdir(&device, 11, 2, 1, b"src");
    walk(&device, 12, 3, 1, 2, &[b"src"]);
    mkdir(&device, 13, 4, 2, b"child");
    walk(&device, 14, 5, 1, 3, &[b"src", b"child"]);

    let rename = decoded_request(VIRTIO_9P_TRENAME, 6, p9_rename_payload(2, 3, b"moved"));
    let completion = device.execute_at(15, rename).unwrap();
    assert_eq!(completion.message_type(), VIRTIO_9P_RLERROR);
    assert_eq!(completion.payload(), VIRTIO_9P_EINVAL.to_le_bytes());

    walk(&device, 16, 7, 1, 4, &[b"src"]);
    walk(&device, 17, 8, 1, 5, &[b"src", b"child"]);
    assert_walk_missing(&device, 18, 9, 3, 6, b"moved");
}
