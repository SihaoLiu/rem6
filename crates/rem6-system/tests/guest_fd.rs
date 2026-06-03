use rem6_system::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFdTable, GuestFileDescription,
    GuestFileDescriptionId, GuestFileStatusFlags, GuestHostFd,
};

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

#[test]
fn guest_fd_exec_closes_marked_descriptors_and_returns_closed_entries() {
    let mut table = GuestFdTable::new();
    let retained = GuestFd::new(2).unwrap();
    let first_closed = GuestFd::new(3).unwrap();
    let second_closed = GuestFd::new(10).unwrap();
    let retained_description = GuestFileDescriptionId::new(20);
    let first_closed_description = GuestFileDescriptionId::new(30);
    let second_closed_description = GuestFileDescriptionId::new(100);
    table
        .insert(retained, GuestFdEntry::new(retained_description))
        .unwrap();
    table
        .insert(
            first_closed,
            GuestFdEntry::new(first_closed_description).with_close_on_exec(true),
        )
        .unwrap();
    table
        .insert(
            second_closed,
            GuestFdEntry::new(second_closed_description).with_close_on_exec(true),
        )
        .unwrap();

    let closed = table.close_on_exec_descriptors();

    assert_eq!(closed.len(), 2);
    assert_eq!(closed[0].fd(), first_closed);
    assert_eq!(closed[0].entry().description(), first_closed_description);
    assert_eq!(closed[1].fd(), second_closed);
    assert_eq!(closed[1].entry().description(), second_closed_description);
    assert!(table.entry(first_closed).is_none());
    assert!(table.entry(second_closed).is_none());
    assert_eq!(
        table.entry(retained).unwrap().description(),
        retained_description
    );
    assert!(!table.close_on_exec(retained).unwrap());
    assert_eq!(table.dup(retained).unwrap(), GuestFd::new(0).unwrap());
}

#[test]
fn guest_fd_status_flags_are_shared_by_file_description_not_descriptor() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(11).unwrap();
    let description = GuestFileDescriptionId::new(110);
    table
        .insert_description(GuestFileDescription::host_backed(
            description,
            GuestHostFd::new(40).unwrap(),
            GuestFileStatusFlags::new(0x02),
        ))
        .unwrap();
    table
        .insert(fd, GuestFdEntry::new(description).with_close_on_exec(true))
        .unwrap();

    let duplicate = table.dup(fd).unwrap();

    assert_eq!(table.status_flags(fd).unwrap().bits(), 0x02);
    assert_eq!(table.status_flags(duplicate).unwrap().bits(), 0x02);
    assert!(table.close_on_exec(fd).unwrap());
    assert!(!table.close_on_exec(duplicate).unwrap());

    table
        .set_status_flags(duplicate, GuestFileStatusFlags::new(0x802))
        .unwrap();

    assert_eq!(table.status_flags(fd).unwrap().bits(), 0x802);
    assert_eq!(table.status_flags(duplicate).unwrap().bits(), 0x802);
    assert_eq!(
        table
            .description_for_fd(fd)
            .unwrap()
            .host_fd()
            .unwrap()
            .get(),
        40
    );
}

#[test]
fn guest_fd_status_flags_reject_missing_description_without_mutating_entries() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(12).unwrap();
    let missing = GuestFileDescriptionId::new(120);
    table
        .insert(fd, GuestFdEntry::new(missing).with_close_on_exec(true))
        .unwrap();

    assert_eq!(
        table.status_flags(fd),
        Err(GuestFdError::MissingFileDescription {
            description: missing
        })
    );
    assert_eq!(
        table.set_status_flags(fd, GuestFileStatusFlags::new(0x20)),
        Err(GuestFdError::MissingFileDescription {
            description: missing
        })
    );
    assert!(table.close_on_exec(fd).unwrap());
    assert_eq!(table.entry(fd).unwrap().description(), missing);
}

#[test]
fn guest_fd_rejects_duplicate_descriptions_and_negative_host_fd() {
    let mut table = GuestFdTable::new();
    let description = GuestFileDescriptionId::new(130);
    let file = GuestFileDescription::host_backed(
        description,
        GuestHostFd::new(41).unwrap(),
        GuestFileStatusFlags::new(0),
    );

    table.insert_description(file.clone()).unwrap();

    assert_eq!(
        table.insert_description(file),
        Err(GuestFdError::DuplicateFileDescription { description })
    );
    assert_eq!(
        GuestHostFd::new(-1).unwrap_err(),
        GuestFdError::NegativeHostFd { fd: -1 }
    );
}

#[test]
fn guest_fd_guest_backed_description_has_shared_status_without_host_fd() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(13).unwrap();
    let description = GuestFileDescriptionId::new(140);
    table
        .insert_description(GuestFileDescription::guest_backed(
            description,
            GuestFileStatusFlags::new(0x40),
        ))
        .unwrap();
    table.insert(fd, GuestFdEntry::new(description)).unwrap();

    let duplicate = table.dup(fd).unwrap();

    assert_eq!(table.description_for_fd(fd).unwrap().host_fd(), None);
    assert_eq!(table.status_flags(duplicate).unwrap().bits(), 0x40);

    table
        .set_status_flags(fd, GuestFileStatusFlags::new(0x240))
        .unwrap();

    assert_eq!(table.status_flags(fd).unwrap().bits(), 0x240);
    assert_eq!(table.status_flags(duplicate).unwrap().bits(), 0x240);
}
