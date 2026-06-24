#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorMaskMode {
    Masked,
    Unmasked,
}

impl RiscvVectorMaskMode {
    pub const fn from_vm_bit(unmasked: bool) -> Self {
        if unmasked {
            Self::Unmasked
        } else {
            Self::Masked
        }
    }

    pub const fn is_masked(self) -> bool {
        matches!(self, Self::Masked)
    }
}
