#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, BytesN, Env};

fn setup(env: &Env) -> (DepositRegistryClient<'_>, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let contract_id = env.register(DepositRegistry, ());
    let registry = DepositRegistryClient::new(env, &contract_id);
    registry.initialize(&admin);
    (registry, admin)
}

fn id(env: &Env, byte: u8) -> BytesN<16> {
    BytesN::from_array(env, &[byte; 16])
}

#[test]
fn record_then_get_returns_available_lot() {
    let env = Env::default();
    let (registry, _admin) = setup(&env);
    let dep = id(&env, 1);
    let user = id(&env, 9);

    registry.record_deposit(&dep, &user, &5_000_00);

    let stored = registry.get_deposit(&dep).unwrap();
    assert_eq!(stored.user_id, user);
    assert_eq!(stored.amount, 5_000_00);
    assert_eq!(stored.badge, Badge::Available);
    assert_eq!(stored.backing_loan, None);
}

#[test]
fn badge_moves_to_collateral_and_back() {
    let env = Env::default();
    let (registry, _admin) = setup(&env);
    let dep = id(&env, 1);
    let loan = id(&env, 2);
    registry.record_deposit(&dep, &id(&env, 9), &1_000_00);

    registry.set_badge(&dep, &Badge::Collateral, &Some(loan.clone()));
    let working = registry.get_deposit(&dep).unwrap();
    assert_eq!(working.badge, Badge::Collateral);
    assert_eq!(working.backing_loan, Some(loan));

    registry.set_badge(&dep, &Badge::Available, &None);
    assert_eq!(registry.get_deposit(&dep).unwrap().badge, Badge::Available);
}

#[test]
fn working_badge_requires_a_loan() {
    let env = Env::default();
    let (registry, _admin) = setup(&env);
    let dep = id(&env, 1);
    registry.record_deposit(&dep, &id(&env, 9), &1_000_00);

    // collateral with no backing loan is illegal
    assert_eq!(
        registry.try_set_badge(&dep, &Badge::Collateral, &None),
        Err(Ok(Error::IllegalBadgeState))
    );
    // available WITH a backing loan is equally illegal
    assert_eq!(
        registry.try_set_badge(&dep, &Badge::Available, &Some(id(&env, 2))),
        Err(Ok(Error::IllegalBadgeState))
    );
}

#[test]
fn only_available_lots_can_be_withdrawn() {
    let env = Env::default();
    let (registry, _admin) = setup(&env);
    let dep = id(&env, 1);
    registry.record_deposit(&dep, &id(&env, 9), &1_000_00);
    registry.set_badge(&dep, &Badge::Lent, &Some(id(&env, 2)));

    // a lent lot cannot leave the pool
    assert_eq!(
        registry.try_withdraw(&dep),
        Err(Ok(Error::NotAvailable))
    );

    // free it, then it withdraws
    registry.set_badge(&dep, &Badge::Available, &None);
    registry.withdraw(&dep);
    assert_eq!(registry.get_deposit(&dep).unwrap().badge, Badge::Withdrawn);

    // withdrawn is terminal
    assert_eq!(
        registry.try_withdraw(&dep),
        Err(Ok(Error::AlreadyWithdrawn))
    );
}

#[test]
fn duplicate_deposit_id_is_refused() {
    let env = Env::default();
    let (registry, _admin) = setup(&env);
    let dep = id(&env, 1);
    registry.record_deposit(&dep, &id(&env, 9), &1_000_00);
    assert_eq!(
        registry.try_record_deposit(&dep, &id(&env, 9), &2_000_00),
        Err(Ok(Error::DepositExists))
    );
}

#[test]
fn non_positive_amount_is_refused() {
    let env = Env::default();
    let (registry, _admin) = setup(&env);
    assert_eq!(
        registry.try_record_deposit(&id(&env, 1), &id(&env, 9), &0),
        Err(Ok(Error::InvalidAmount))
    );
}

#[test]
fn writes_require_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(DepositRegistry, ());
    let registry = DepositRegistryClient::new(&env, &contract_id);
    registry.initialize(&admin);

    // from here, no auths are allowed — an unsigned write must fail
    env.set_auths(&[]);
    assert!(registry
        .try_record_deposit(&id(&env, 1), &id(&env, 9), &1_000_00)
        .is_err());
}
