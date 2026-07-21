use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3ScopedIssuePlan {
    issue_width: usize,
    reserved_width: usize,
    issued: Vec<O3ScopedReadyInstruction>,
    resource_blocked: Vec<O3ScopedReadyInstruction>,
    dependency_blocked: Vec<O3ScopedReadyInstruction>,
}

impl O3ScopedIssuePlan {
    pub const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub const fn reserved_width(&self) -> usize {
        self.reserved_width
    }

    pub const fn available_width(&self) -> usize {
        self.issue_width - self.reserved_width
    }

    pub fn issued(&self) -> &[O3ScopedReadyInstruction] {
        &self.issued
    }

    pub fn resource_blocked(&self) -> &[O3ScopedReadyInstruction] {
        &self.resource_blocked
    }

    pub fn dependency_blocked(&self) -> &[O3ScopedReadyInstruction] {
        &self.dependency_blocked
    }

    pub fn issued_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.issued.iter().map(|instruction| instruction.sequence())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3ScopedIssueScheduler {
    issue_width: usize,
    capacities: BTreeMap<(O3IssueQueueId, O3IssueOpClass), usize>,
}

impl O3ScopedIssueScheduler {
    pub fn new<I>(issue_width: usize, capacities: I) -> Result<Self, O3PipelineError>
    where
        I: IntoIterator<Item = O3IssueQueueCapacity>,
    {
        if issue_width == 0 {
            return Err(O3PipelineError::ZeroIssueWidth);
        }

        Ok(Self {
            issue_width,
            capacities: capacities
                .into_iter()
                .map(|capacity| ((capacity.queue(), capacity.op_class()), capacity.slots()))
                .collect(),
        })
    }

    pub const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub fn plan<R, I>(&self, resolved_scopes: R, ready: I) -> O3ScopedIssuePlan
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        self.plan_with_reserved_width(0, resolved_scopes, ready)
    }

    pub fn plan_with_reserved_width<R, I>(
        &self,
        reserved_width: usize,
        resolved_scopes: R,
        ready: I,
    ) -> O3ScopedIssuePlan
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        self.try_plan_with_reserved_width(reserved_width, resolved_scopes, ready)
            .expect(
                "scoped issue plan must have a valid reservation and unique dependency producers",
            )
    }

    pub fn try_plan<R, I>(
        &self,
        resolved_scopes: R,
        ready: I,
    ) -> Result<O3ScopedIssuePlan, O3PipelineError>
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        self.try_plan_with_reserved_width(0, resolved_scopes, ready)
    }

    pub fn try_plan_with_reserved_width<R, I>(
        &self,
        reserved_width: usize,
        resolved_scopes: R,
        ready: I,
    ) -> Result<O3ScopedIssuePlan, O3PipelineError>
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        if reserved_width > self.issue_width {
            return Err(O3PipelineError::ReservedIssueWidthExceedsConfigured {
                reserved_width,
                issue_width: self.issue_width,
            });
        }
        let available_width = self.issue_width - reserved_width;
        let resolved_scopes = resolved_scopes.into_iter().collect::<BTreeSet<_>>();
        let mut pending = ready.into_iter().collect::<Vec<_>>();
        pending.sort_by_key(|instruction| instruction.sequence());
        validate_unique_dependency_producers(&pending)?;

        let mut remaining_capacity = self.capacities.clone();
        let mut issued = Vec::new();
        while issued.len() < available_width {
            let Some(index) = pending.iter().position(|instruction| {
                dependency_ready(&resolved_scopes, instruction)
                    && scoped_issue_slots(&remaining_capacity, instruction) != 0
            }) else {
                break;
            };

            let instruction = pending.remove(index);
            if let Some(slots) =
                remaining_capacity.get_mut(&(instruction.queue(), instruction.op_class()))
            {
                *slots -= 1;
            }
            issued.push(instruction);
        }

        let mut resource_blocked = Vec::new();
        let mut dependency_blocked = Vec::new();
        for instruction in pending {
            if dependency_ready(&resolved_scopes, &instruction) {
                resource_blocked.push(instruction);
            } else {
                dependency_blocked.push(instruction);
            }
        }

        Ok(O3ScopedIssuePlan {
            issue_width: self.issue_width,
            reserved_width,
            issued,
            resource_blocked,
            dependency_blocked,
        })
    }
}
