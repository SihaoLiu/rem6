use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3ProducerForwardedControlTarget {
    identity: O3ProducerForwardedControlIdentity,
    ready_tick: u64,
}

impl O3ProducerForwardedControlTarget {
    pub(super) fn new(identity: O3ProducerForwardedControlIdentity, ready_tick: u64) -> Self {
        Self {
            identity,
            ready_tick,
        }
    }

    pub(super) fn same_control_identity(self, other: Self) -> bool {
        self.identity == other.identity
    }

    pub(crate) const fn data_access_fetch_request(self) -> MemoryRequestId {
        self.identity.data_access_fetch_request
    }

    pub(crate) const fn fetch_request(self) -> MemoryRequestId {
        self.identity.fetch_request
    }

    pub(crate) const fn last_fetch_request(self) -> MemoryRequestId {
        self.identity.last_fetch_request
    }

    pub(crate) const fn pc(self) -> Address {
        self.identity.pc
    }

    pub(crate) const fn sequential_pc(self) -> Address {
        self.identity.sequential_pc
    }

    pub(crate) const fn instruction(self) -> RiscvInstruction {
        self.identity.instruction
    }

    pub(crate) const fn consumer_sequence(self) -> u64 {
        self.identity.consumer_sequence
    }

    pub(crate) const fn producer_sequence(self) -> u64 {
        self.identity.producer_sequence
    }

    pub(crate) const fn ready_tick(self) -> u64 {
        self.ready_tick
    }

    pub(crate) const fn target_source(self) -> rem6_isa_riscv::Register {
        self.identity.target_source
    }

    pub(crate) const fn target(self) -> Address {
        self.identity.target
    }

    pub(crate) fn link_destination(self) -> Option<rem6_isa_riscv::Register> {
        let control = o3_live_control_operands(self.instruction())?;
        (control.kind() == BranchTargetKind::CallIndirect).then_some(control.destination())?
    }

    pub(crate) fn fetched_scalar_chain(
        self,
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<O3ProducerForwardedScalarChain> {
        let link = self.link_destination()?;
        let (destination, sources) = o3_predicted_scalar_descendant_operands(instruction)?;
        if destination.is_zero()
            || destination == link
            || !sources.contains(&link)
            || !valid_live_speculative_fetch_identity(consumed_requests)
        {
            return None;
        }
        let fetch_request = *consumed_requests.first()?;
        Some(O3ProducerForwardedScalarChain::from_descendant(
            O3ProducerForwardedScalarDescendant {
                parent: self,
                fetch_request,
                last_fetch_request: *consumed_requests.last()?,
                pc: self.target(),
                sequential_pc: Address::new(
                    self.target()
                        .get()
                        .wrapping_add(u64::from(instruction_bytes)),
                ),
                instruction,
                sequence: fetch_request.sequence(),
            },
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct O3ProducerForwardedScalarDescendant {
    parent: O3ProducerForwardedControlTarget,
    fetch_request: MemoryRequestId,
    last_fetch_request: MemoryRequestId,
    pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
    sequence: u64,
}

impl PartialEq for O3ProducerForwardedScalarDescendant {
    fn eq(&self, other: &Self) -> bool {
        self.parent.same_control_identity(other.parent)
            && self.fetch_request == other.fetch_request
            && self.last_fetch_request == other.last_fetch_request
            && self.pc == other.pc
            && self.sequential_pc == other.sequential_pc
            && self.instruction == other.instruction
            && self.sequence == other.sequence
    }
}

impl Eq for O3ProducerForwardedScalarDescendant {}

impl O3ProducerForwardedScalarDescendant {
    pub(super) fn new(
        parent: O3ProducerForwardedControlTarget,
        fetch_request: MemoryRequestId,
        last_fetch_request: MemoryRequestId,
        pc: Address,
        sequential_pc: Address,
        instruction: RiscvInstruction,
        sequence: u64,
    ) -> Self {
        Self {
            parent,
            fetch_request,
            last_fetch_request,
            pc,
            sequential_pc,
            instruction,
            sequence,
        }
    }

    pub(crate) const fn parent(self) -> O3ProducerForwardedControlTarget {
        self.parent
    }

    #[cfg(test)]
    pub(crate) const fn fetch_request(self) -> MemoryRequestId {
        self.fetch_request
    }

    pub(crate) const fn last_fetch_request(self) -> MemoryRequestId {
        self.last_fetch_request
    }

    #[cfg(test)]
    pub(crate) const fn pc(self) -> Address {
        self.pc
    }

    pub(crate) const fn sequential_pc(self) -> Address {
        self.sequential_pc
    }

    #[cfg(test)]
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum O3ProducerForwardedScalarDescendants {
    Empty,
    One(O3ProducerForwardedScalarDescendant),
    Many(Vec<O3ProducerForwardedScalarDescendant>),
}

impl O3ProducerForwardedScalarDescendants {
    fn push(&mut self, descendant: O3ProducerForwardedScalarDescendant) {
        match self {
            Self::Empty => *self = Self::One(descendant),
            Self::One(first) => *self = Self::Many(vec![*first, descendant]),
            Self::Many(descendants) => descendants.push(descendant),
        }
    }

    fn last(&self) -> Option<O3ProducerForwardedScalarDescendant> {
        match self {
            Self::Empty => None,
            Self::One(descendant) => Some(*descendant),
            Self::Many(descendants) => descendants.last().copied(),
        }
    }

    const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    const fn is_one(&self) -> bool {
        matches!(self, Self::One(_))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct O3ProducerForwardedScalarChain {
    parent: O3ProducerForwardedControlTarget,
    descendants: O3ProducerForwardedScalarDescendants,
}

impl PartialEq for O3ProducerForwardedScalarChain {
    fn eq(&self, other: &Self) -> bool {
        self.parent.same_control_identity(other.parent) && self.descendants == other.descendants
    }
}

impl Eq for O3ProducerForwardedScalarChain {}

impl O3ProducerForwardedScalarChain {
    pub(crate) fn empty(parent: O3ProducerForwardedControlTarget) -> Self {
        Self {
            parent,
            descendants: O3ProducerForwardedScalarDescendants::Empty,
        }
    }

    pub(super) fn from_descendant(descendant: O3ProducerForwardedScalarDescendant) -> Self {
        let mut chain = Self::empty(descendant.parent());
        chain.push(descendant);
        chain
    }

    fn push(&mut self, descendant: O3ProducerForwardedScalarDescendant) -> bool {
        if !self.parent.same_control_identity(descendant.parent()) {
            return false;
        }
        self.descendants.push(descendant);
        true
    }

    pub(crate) const fn parent(&self) -> O3ProducerForwardedControlTarget {
        self.parent
    }

    pub(crate) fn last(&self) -> Option<O3ProducerForwardedScalarDescendant> {
        self.descendants.last()
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.descendants.is_empty()
    }

    #[cfg(test)]
    pub(crate) const fn is_one_step(&self) -> bool {
        self.descendants.is_one()
    }

    #[cfg(test)]
    pub(crate) fn repeated_last_for_test(&self) -> Self {
        let mut repeated = self.clone();
        if let Some(descendant) = repeated.last() {
            assert!(repeated.push(descendant));
        }
        repeated
    }

    pub(crate) fn matches_retained_candidate(&self, candidate: &Self) -> bool {
        self == candidate
            || (self.parent.same_control_identity(candidate.parent)
                && self.is_empty()
                && candidate.descendants.is_one())
    }

    pub(crate) fn retained_return_descendant(
        &self,
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        let scalar = self.last()?;
        if o3_exact_link_return_source(instruction) != self.parent().link_destination()
            || !valid_live_speculative_fetch_identity(consumed_requests)
        {
            return None;
        }
        let fetch_request = *consumed_requests.first()?;
        Some(O3ProducerForwardedReturnDescendant {
            scalar_chain: self.clone(),
            fetch_request,
            last_fetch_request: *consumed_requests.last()?,
            pc: scalar.sequential_pc(),
            sequential_pc: Address::new(
                scalar
                    .sequential_pc()
                    .get()
                    .wrapping_add(u64::from(instruction_bytes)),
            ),
            instruction,
            sequence: fetch_request.sequence(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct O3ProducerForwardedReturnDescendant {
    scalar_chain: O3ProducerForwardedScalarChain,
    fetch_request: MemoryRequestId,
    last_fetch_request: MemoryRequestId,
    pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
    sequence: u64,
}

impl PartialEq for O3ProducerForwardedReturnDescendant {
    fn eq(&self, other: &Self) -> bool {
        self.scalar_chain == other.scalar_chain
            && self.fetch_request == other.fetch_request
            && self.last_fetch_request == other.last_fetch_request
            && self.pc == other.pc
            && self.sequential_pc == other.sequential_pc
            && self.instruction == other.instruction
            && self.sequence == other.sequence
    }
}

impl Eq for O3ProducerForwardedReturnDescendant {}

impl O3ProducerForwardedReturnDescendant {
    pub(super) fn new(
        scalar_chain: O3ProducerForwardedScalarChain,
        fetch_request: MemoryRequestId,
        last_fetch_request: MemoryRequestId,
        pc: Address,
        sequential_pc: Address,
        instruction: RiscvInstruction,
        sequence: u64,
    ) -> Self {
        Self {
            scalar_chain,
            fetch_request,
            last_fetch_request,
            pc,
            sequential_pc,
            instruction,
            sequence,
        }
    }

    pub(crate) const fn parent(&self) -> O3ProducerForwardedControlTarget {
        self.scalar_chain.parent()
    }

    pub(crate) const fn scalar_chain(&self) -> &O3ProducerForwardedScalarChain {
        &self.scalar_chain
    }

    pub(crate) const fn fetch_request(&self) -> MemoryRequestId {
        self.fetch_request
    }

    pub(crate) const fn last_fetch_request(&self) -> MemoryRequestId {
        self.last_fetch_request
    }

    pub(crate) const fn pc(&self) -> Address {
        self.pc
    }

    pub(crate) const fn sequential_pc(&self) -> Address {
        self.sequential_pc
    }

    pub(crate) const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn target(&self) -> Address {
        self.parent().sequential_pc()
    }
}
