pub(crate) const MAX_VECTOR_ALLOCATION_BYTES: usize = isize::MAX as usize;

pub(crate) const fn max_vector_len<T>() -> usize {
    let element_bytes = std::mem::size_of::<T>();
    if element_bytes == 0 {
        usize::MAX
    } else {
        MAX_VECTOR_ALLOCATION_BYTES / element_bytes
    }
}
