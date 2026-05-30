use rem6_protocol_moesi::{
    MoesiProtocolEventId, MoesiProtocolResourceId, MoesiProtocolStateId, MoesiResourceEffect,
    MoesiTransitionResourceContract, MoesiTransitionResourceError, MoesiTransitionResourceRule,
};

fn state(name: &str) -> MoesiProtocolStateId {
    MoesiProtocolStateId::new(name).unwrap()
}

fn event(name: &str) -> MoesiProtocolEventId {
    MoesiProtocolEventId::new(name).unwrap()
}

fn resource(name: &str) -> MoesiProtocolResourceId {
    MoesiProtocolResourceId::new(name).unwrap()
}

#[test]
fn transition_resource_contract_rejects_missing_tbe_allocation() {
    let tbe = resource("TBE");
    let contract = MoesiTransitionResourceContract::new()
        .with_state_resource(state("MM"), tbe.clone())
        .with_rule(MoesiTransitionResourceRule::new(
            state("M"),
            event("L1_GETX"),
            state("MM"),
        ));

    assert_eq!(
        contract.validate().unwrap_err(),
        MoesiTransitionResourceError::MissingResourceAllocation {
            state: state("M"),
            event: event("L1_GETX"),
            target: state("MM"),
            resource: tbe,
        }
    );
}

#[test]
fn transition_resource_contract_accepts_allocate_then_release_pair() {
    let tbe = resource("TBE");
    let contract = MoesiTransitionResourceContract::new()
        .with_state_resource(state("MM"), tbe.clone())
        .with_rule(
            MoesiTransitionResourceRule::new(state("M"), event("L1_GETX"), state("MM"))
                .with_effect(MoesiResourceEffect::Allocate(tbe.clone())),
        )
        .with_rule(
            MoesiTransitionResourceRule::new(state("MM"), event("Exclusive_Unblock"), state("M"))
                .with_effect(MoesiResourceEffect::Release(tbe.clone())),
        );

    let report = contract.validate().unwrap();

    assert_eq!(report.transition_count(), 2);
    assert_eq!(report.resource_count(), 1);
    assert_eq!(report.state_resource_count(), 1);
}

#[test]
fn transition_resource_contract_rejects_missing_tbe_release() {
    let tbe = resource("TBE");
    let contract = MoesiTransitionResourceContract::new()
        .with_state_resource(state("MM"), tbe.clone())
        .with_rule(
            MoesiTransitionResourceRule::new(state("M"), event("L1_GETX"), state("MM"))
                .with_effect(MoesiResourceEffect::Allocate(tbe.clone())),
        )
        .with_rule(MoesiTransitionResourceRule::new(
            state("MM"),
            event("Exclusive_Unblock"),
            state("M"),
        ));

    assert_eq!(
        contract.validate().unwrap_err(),
        MoesiTransitionResourceError::MissingResourceRelease {
            state: state("MM"),
            event: event("Exclusive_Unblock"),
            target: state("M"),
            resource: tbe,
        }
    );
}

#[test]
fn transition_resource_contract_rejects_release_without_ownership() {
    let tbe = resource("TBE");
    let contract = MoesiTransitionResourceContract::new().with_rule(
        MoesiTransitionResourceRule::new(state("OS"), event("Unblock"), state("O"))
            .with_effect(MoesiResourceEffect::Release(tbe.clone())),
    );

    assert_eq!(
        contract.validate().unwrap_err(),
        MoesiTransitionResourceError::ReleaseWithoutResource {
            state: state("OS"),
            event: event("Unblock"),
            target: state("O"),
            resource: tbe,
        }
    );
}

#[test]
fn transition_resource_contract_rejects_duplicate_transition_keys() {
    let contract = MoesiTransitionResourceContract::new()
        .with_rule(MoesiTransitionResourceRule::new(
            state("M"),
            event("L1_GETX"),
            state("MM"),
        ))
        .with_rule(MoesiTransitionResourceRule::new(
            state("M"),
            event("L1_GETX"),
            state("M_W"),
        ));

    assert_eq!(
        contract.validate().unwrap_err(),
        MoesiTransitionResourceError::DuplicateTransition {
            state: state("M"),
            event: event("L1_GETX"),
        }
    );
}
