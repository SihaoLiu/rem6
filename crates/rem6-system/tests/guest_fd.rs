use rem6_system::{GuestFd, GuestFdEntry, GuestFdError, GuestFdTable, GuestFileDescriptionId};

#[test]
fn guest_fd_dup2_respects_requested_destination_fd() {
    let mut table = GuestFdTable::new();
    let stdout = GuestFd::new(1).unwrap();
    let requested = GuestFd::new(100).unwrap();
    let stdout_description = GuestFileDescriptionId::new(77);
    table
        .insert(
            stdout,
            GuestFdEntry::new(stdout_description).with_close_on_exec(true),
        )
        .unwrap();

    let duplicated = table.dup2(stdout, requested).unwrap();

    assert_eq!(duplicated, requested);
    assert_eq!(
        table.entry(requested).unwrap().description(),
        stdout_description
    );
    assert!(!table.entry(requested).unwrap().close_on_exec());
    assert!(table.entry(GuestFd::new(0).unwrap()).is_none());
    assert!(table.entry(GuestFd::new(2).unwrap()).is_none());
}

#[test]
fn guest_fd_dup2_replaces_existing_destination_without_allocating_another_fd() {
    let mut table = GuestFdTable::new();
    let source = GuestFd::new(3).unwrap();
    let destination = GuestFd::new(4).unwrap();
    let source_description = GuestFileDescriptionId::new(30);
    let replaced_description = GuestFileDescriptionId::new(40);
    table
        .insert(source, GuestFdEntry::new(source_description))
        .unwrap();
    table
        .insert(destination, GuestFdEntry::new(replaced_description))
        .unwrap();

    let duplicated = table.dup2(source, destination).unwrap();

    assert_eq!(duplicated, destination);
    assert_eq!(
        table.entry(destination).unwrap().description(),
        source_description
    );
    assert_eq!(table.len(), 2);
    assert_eq!(table.dup(source).unwrap(), GuestFd::new(0).unwrap());
}

#[test]
fn guest_fd_dup2_same_fd_is_a_noop_after_source_validation() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(5).unwrap();
    table
        .insert(
            fd,
            GuestFdEntry::new(GuestFileDescriptionId::new(50)).with_close_on_exec(true),
        )
        .unwrap();

    assert_eq!(table.dup2(fd, fd).unwrap(), fd);
    assert!(table.entry(fd).unwrap().close_on_exec());
    assert_eq!(
        table.dup2(GuestFd::new(6).unwrap(), GuestFd::new(6).unwrap()),
        Err(GuestFdError::BadFd {
            fd: GuestFd::new(6).unwrap()
        })
    );
}

#[test]
fn guest_fd_close_on_exec_can_be_read_and_updated_by_fd() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(7).unwrap();
    table
        .insert(fd, GuestFdEntry::new(GuestFileDescriptionId::new(70)))
        .unwrap();

    assert!(!table.close_on_exec(fd).unwrap());

    table.set_close_on_exec(fd, true).unwrap();
    assert!(table.close_on_exec(fd).unwrap());

    table.set_close_on_exec(fd, false).unwrap();
    assert!(!table.close_on_exec(fd).unwrap());
}

#[test]
fn guest_fd_close_on_exec_rejects_bad_fd_without_mutating_other_entries() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(8).unwrap();
    let bad_fd = GuestFd::new(9).unwrap();
    table
        .insert(
            fd,
            GuestFdEntry::new(GuestFileDescriptionId::new(80)).with_close_on_exec(true),
        )
        .unwrap();

    assert_eq!(
        table.set_close_on_exec(bad_fd, false),
        Err(GuestFdError::BadFd { fd: bad_fd })
    );
    assert_eq!(
        table.close_on_exec(bad_fd),
        Err(GuestFdError::BadFd { fd: bad_fd })
    );
    assert!(table.close_on_exec(fd).unwrap());
}
