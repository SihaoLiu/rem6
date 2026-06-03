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

#[test]
fn guest_fd_close_descriptor_releases_description_only_after_last_reference() {
    let mut table = GuestFdTable::new();
    let first = GuestFd::new(14).unwrap();
    let second = GuestFd::new(15).unwrap();
    let description = GuestFileDescriptionId::new(150);
    table
        .insert_description(GuestFileDescription::host_backed(
            description,
            GuestHostFd::new(42).unwrap(),
            GuestFileStatusFlags::new(0x80),
        ))
        .unwrap();
    table.insert(first, GuestFdEntry::new(description)).unwrap();
    table
        .insert(second, GuestFdEntry::new(description))
        .unwrap();

    let first_closed = table.close_descriptor(first).unwrap();

    assert_eq!(first_closed.fd(), first);
    assert_eq!(first_closed.entry().description(), description);
    assert_eq!(first_closed.released_description(), None);
    assert!(table.description(description).is_some());

    let second_closed = table.close_descriptor(second).unwrap();

    assert_eq!(second_closed.fd(), second);
    assert_eq!(second_closed.entry().description(), description);
    assert_eq!(
        second_closed
            .into_released_description()
            .unwrap()
            .host_fd()
            .unwrap()
            .get(),
        42
    );
    assert!(table.description(description).is_none());
}

#[test]
fn guest_fd_close_descriptor_allows_missing_description_cleanup() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(16).unwrap();
    let missing = GuestFileDescriptionId::new(160);
    table.insert(fd, GuestFdEntry::new(missing)).unwrap();

    let closed = table.close_descriptor(fd).unwrap();

    assert_eq!(closed.fd(), fd);
    assert_eq!(closed.entry().description(), missing);
    assert_eq!(closed.released_description(), None);
    assert!(table.entry(fd).is_none());
}

#[test]
fn guest_fd_entry_only_close_preserves_description_metadata() {
    let mut table = GuestFdTable::new();
    let fd = GuestFd::new(17).unwrap();
    let description = GuestFileDescriptionId::new(170);
    table
        .insert_description(GuestFileDescription::host_backed(
            description,
            GuestHostFd::new(43).unwrap(),
            GuestFileStatusFlags::new(0x01),
        ))
        .unwrap();
    table.insert(fd, GuestFdEntry::new(description)).unwrap();

    let closed = table.close(fd).unwrap();

    assert_eq!(closed.description(), description);
    assert!(table.entry(fd).is_none());
    assert_eq!(
        table
            .description(description)
            .unwrap()
            .host_fd()
            .unwrap()
            .get(),
        43
    );
}

#[test]
fn guest_fd_entry_only_dup2_preserves_replaced_description_metadata() {
    let mut table = GuestFdTable::new();
    let source = GuestFd::new(18).unwrap();
    let destination = GuestFd::new(19).unwrap();
    let source_description = GuestFileDescriptionId::new(180);
    let destination_description = GuestFileDescriptionId::new(190);
    table
        .insert_description(GuestFileDescription::guest_backed(
            source_description,
            GuestFileStatusFlags::new(0x02),
        ))
        .unwrap();
    table
        .insert_description(GuestFileDescription::host_backed(
            destination_description,
            GuestHostFd::new(44).unwrap(),
            GuestFileStatusFlags::new(0x04),
        ))
        .unwrap();
    table
        .insert(source, GuestFdEntry::new(source_description))
        .unwrap();
    table
        .insert(destination, GuestFdEntry::new(destination_description))
        .unwrap();

    assert_eq!(table.dup2(source, destination).unwrap(), destination);

    assert_eq!(
        table.entry(destination).unwrap().description(),
        source_description
    );
    assert_eq!(
        table
            .description(destination_description)
            .unwrap()
            .host_fd()
            .unwrap()
            .get(),
        44
    );
}

#[test]
fn guest_fd_dup2_with_replacement_reports_released_destination_description() {
    let mut table = GuestFdTable::new();
    let source = GuestFd::new(20).unwrap();
    let destination = GuestFd::new(21).unwrap();
    let source_description = GuestFileDescriptionId::new(200);
    let destination_description = GuestFileDescriptionId::new(210);
    table
        .insert_description(GuestFileDescription::guest_backed(
            source_description,
            GuestFileStatusFlags::new(0x01),
        ))
        .unwrap();
    table
        .insert_description(GuestFileDescription::host_backed(
            destination_description,
            GuestHostFd::new(43).unwrap(),
            GuestFileStatusFlags::new(0x02),
        ))
        .unwrap();
    table
        .insert(source, GuestFdEntry::new(source_description))
        .unwrap();
    table
        .insert(destination, GuestFdEntry::new(destination_description))
        .unwrap();

    let duplicated = table.dup2_with_replacement(source, destination).unwrap();

    assert_eq!(duplicated.fd(), destination);
    let replaced = duplicated.replaced().unwrap();
    assert_eq!(replaced.fd(), destination);
    assert_eq!(replaced.entry().description(), destination_description);
    assert_eq!(
        replaced.released_description().unwrap().id(),
        destination_description
    );
    assert_eq!(
        table.entry(destination).unwrap().description(),
        source_description
    );
    assert!(table.description(source_description).is_some());
    assert!(table.description(destination_description).is_none());
    assert_eq!(table.len(), 2);
}

#[test]
fn guest_fd_dup2_with_replacement_keeps_destination_description_until_last_alias() {
    let mut table = GuestFdTable::new();
    let source = GuestFd::new(22).unwrap();
    let destination = GuestFd::new(23).unwrap();
    let destination_alias = GuestFd::new(24).unwrap();
    let source_description = GuestFileDescriptionId::new(220);
    let destination_description = GuestFileDescriptionId::new(230);
    table
        .insert_description(GuestFileDescription::guest_backed(
            source_description,
            GuestFileStatusFlags::new(0x04),
        ))
        .unwrap();
    table
        .insert_description(GuestFileDescription::host_backed(
            destination_description,
            GuestHostFd::new(45).unwrap(),
            GuestFileStatusFlags::new(0x08),
        ))
        .unwrap();
    table
        .insert(source, GuestFdEntry::new(source_description))
        .unwrap();
    table
        .insert(destination, GuestFdEntry::new(destination_description))
        .unwrap();
    table
        .insert(
            destination_alias,
            GuestFdEntry::new(destination_description),
        )
        .unwrap();

    let duplicated = table.dup2_with_replacement(source, destination).unwrap();

    assert_eq!(duplicated.fd(), destination);
    let replaced = duplicated.replaced().unwrap();
    assert_eq!(replaced.fd(), destination);
    assert_eq!(replaced.entry().description(), destination_description);
    assert_eq!(replaced.released_description(), None);
    assert!(table.description(destination_description).is_some());

    let final_alias = table.close_descriptor(destination_alias).unwrap();

    assert_eq!(
        final_alias.released_description().unwrap().id(),
        destination_description
    );
    assert!(table.description(destination_description).is_none());
}

#[test]
fn guest_fd_dup2_with_replacement_handles_noop_bad_source_and_absent_destination() {
    let mut table = GuestFdTable::new();
    let source = GuestFd::new(25).unwrap();
    let absent_destination = GuestFd::new(26).unwrap();
    let source_description = GuestFileDescriptionId::new(250);
    table
        .insert_description(GuestFileDescription::guest_backed(
            source_description,
            GuestFileStatusFlags::new(0x10),
        ))
        .unwrap();
    table
        .insert(
            source,
            GuestFdEntry::new(source_description).with_close_on_exec(true),
        )
        .unwrap();

    let same_fd = table.dup2_with_replacement(source, source).unwrap();

    assert_eq!(same_fd.fd(), source);
    assert_eq!(same_fd.replaced(), None);
    assert!(table.close_on_exec(source).unwrap());

    let duplicated = table
        .dup2_with_replacement(source, absent_destination)
        .unwrap();

    assert_eq!(duplicated.fd(), absent_destination);
    assert_eq!(duplicated.replaced(), None);
    assert_eq!(
        table.entry(absent_destination).unwrap().description(),
        source_description
    );
    assert!(!table.close_on_exec(absent_destination).unwrap());

    let bad_source = GuestFd::new(27).unwrap();
    assert_eq!(
        table.dup2_with_replacement(bad_source, absent_destination),
        Err(GuestFdError::BadFd { fd: bad_source })
    );
    assert_eq!(
        table.entry(absent_destination).unwrap().description(),
        source_description
    );
}

#[test]
fn guest_fd_exec_close_releases_shared_description_after_all_marked_references() {
    let mut table = GuestFdTable::new();
    let retained = GuestFd::new(28).unwrap();
    let first_closed = GuestFd::new(29).unwrap();
    let second_closed = GuestFd::new(30).unwrap();
    let retained_description = GuestFileDescriptionId::new(280);
    let closed_description = GuestFileDescriptionId::new(290);
    table
        .insert_description(GuestFileDescription::guest_backed(
            retained_description,
            GuestFileStatusFlags::new(0x10),
        ))
        .unwrap();
    table
        .insert_description(GuestFileDescription::host_backed(
            closed_description,
            GuestHostFd::new(44).unwrap(),
            GuestFileStatusFlags::new(0x20),
        ))
        .unwrap();
    table
        .insert(retained, GuestFdEntry::new(retained_description))
        .unwrap();
    table
        .insert(
            first_closed,
            GuestFdEntry::new(closed_description).with_close_on_exec(true),
        )
        .unwrap();
    table
        .insert(
            second_closed,
            GuestFdEntry::new(closed_description).with_close_on_exec(true),
        )
        .unwrap();

    let closed = table.close_on_exec_descriptors();

    assert_eq!(closed.len(), 2);
    assert_eq!(closed[0].fd(), first_closed);
    assert_eq!(closed[0].released_description(), None);
    assert_eq!(closed[1].fd(), second_closed);
    assert_eq!(closed[1].entry().description(), closed_description);
    assert_eq!(
        closed[1].released_description().unwrap().id(),
        closed_description
    );
    assert!(table.description(closed_description).is_none());
    assert!(table.description(retained_description).is_some());
    assert_eq!(
        table.entry(retained).unwrap().description(),
        retained_description
    );
}

#[test]
fn guest_fd_exec_close_keeps_description_referenced_by_retained_alias() {
    let mut table = GuestFdTable::new();
    let retained = GuestFd::new(31).unwrap();
    let closed = GuestFd::new(32).unwrap();
    let description = GuestFileDescriptionId::new(310);
    table
        .insert_description(GuestFileDescription::host_backed(
            description,
            GuestHostFd::new(46).unwrap(),
            GuestFileStatusFlags::new(0x40),
        ))
        .unwrap();
    table
        .insert(retained, GuestFdEntry::new(description))
        .unwrap();
    table
        .insert(
            closed,
            GuestFdEntry::new(description).with_close_on_exec(true),
        )
        .unwrap();

    let closed_records = table.close_on_exec_descriptors();

    assert_eq!(closed_records.len(), 1);
    assert_eq!(closed_records[0].fd(), closed);
    assert_eq!(closed_records[0].entry().description(), description);
    assert_eq!(closed_records[0].released_description(), None);
    assert!(table.entry(closed).is_none());
    assert_eq!(table.entry(retained).unwrap().description(), description);
    assert_eq!(
        table
            .description(description)
            .unwrap()
            .host_fd()
            .unwrap()
            .get(),
        46
    );
}
