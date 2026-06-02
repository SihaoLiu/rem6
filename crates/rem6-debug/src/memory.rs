pub(crate) fn memory_addresses(address: u64, len: usize) -> Option<Vec<u64>> {
    let mut addresses = Vec::with_capacity(len);
    for offset in 0..len {
        let offset = u64::try_from(offset).ok()?;
        addresses.push(address.checked_add(offset)?);
    }
    Some(addresses)
}
