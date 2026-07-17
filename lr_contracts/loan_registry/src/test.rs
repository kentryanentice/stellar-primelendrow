#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, BytesN, Env};

fn setup(env: &Env) -> LoanRegistryClient<'_> {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let contract_id = env.register(LoanRegistry, ());
    let registry = LoanRegistryClient::new(env, &contract_id);
    registry.initialize(&admin);
    registry
}

fn id(env: &Env, byte: u8) -> BytesN<16> {
    BytesN::from_array(env, &[byte; 16])
}

// principal 5,000.00 · 2%/mo · 3 months · policy v1
fn record(reg: &LoanRegistryClient, loan: &BytesN<16>, borrower: &BytesN<16>) {
    reg.record_loan(loan, borrower, &Product::DepositBacked, &5_000_00, &200, &3, &1);
}

#[test]
fn record_then_activate_sets_outstanding() {
    let env = Env::default();
    let reg = setup(&env);
    let loan = id(&env, 1);
    let borrower = id(&env, 9);

    record(&reg, &loan, &borrower);
    let pending = reg.get_loan(&loan).unwrap();
    assert_eq!(pending.status, Status::Pending);
    assert_eq!(pending.principal_outstanding, 0);
    assert_eq!(reg.open_loan_of(&borrower), Some(loan.clone()));

    reg.activate(&loan, &1_700_000_000);
    let active = reg.get_loan(&loan).unwrap();
    assert_eq!(active.status, Status::Active);
    assert_eq!(active.principal_outstanding, 5_000_00);
    assert_eq!(active.disbursed_at, Some(1_700_000_000));
}

#[test]
fn repay_down_then_close_frees_the_borrower() {
    let env = Env::default();
    let reg = setup(&env);
    let loan = id(&env, 1);
    let borrower = id(&env, 9);
    record(&reg, &loan, &borrower);
    reg.activate(&loan, &1_700_000_000);

    reg.reduce_principal(&loan, &2_000_00);
    assert_eq!(reg.get_loan(&loan).unwrap().principal_outstanding, 3_000_00);

    // can't close while principal remains
    assert_eq!(
        reg.try_close(&loan, &1_700_100_000),
        Err(Ok(Error::OutstandingRemains))
    );

    reg.reduce_principal(&loan, &3_000_00);
    reg.close(&loan, &1_700_100_000);
    let closed = reg.get_loan(&loan).unwrap();
    assert_eq!(closed.status, Status::Closed);
    assert_eq!(closed.closed_at, Some(1_700_100_000));
    // slot released -> borrower may borrow again
    assert_eq!(reg.open_loan_of(&borrower), None);
}

#[test]
fn one_open_loan_per_borrower() {
    let env = Env::default();
    let reg = setup(&env);
    let borrower = id(&env, 9);
    record(&reg, &id(&env, 1), &borrower);

    // a second open loan for the same borrower is refused
    assert_eq!(
        reg.try_record_loan(&id(&env, 2), &borrower, &Product::Guarantor, &1_000_00, &600, &6, &1),
        Err(Ok(Error::BorrowerHasOpenLoan))
    );

    // once the first is cancelled, a new one is allowed
    reg.cancel(&id(&env, 1));
    reg.record_loan(&id(&env, 2), &borrower, &Product::Guarantor, &1_000_00, &600, &6, &1);
    assert_eq!(reg.open_loan_of(&borrower), Some(id(&env, 2)));
}

#[test]
fn reduce_cannot_exceed_outstanding() {
    let env = Env::default();
    let reg = setup(&env);
    let loan = id(&env, 1);
    record(&reg, &loan, &id(&env, 9));
    reg.activate(&loan, &1_700_000_000);
    assert_eq!(
        reg.try_reduce_principal(&loan, &5_000_01),
        Err(Ok(Error::ReduceExceedsOutstanding))
    );
}

#[test]
fn rejects_bad_rate_term_and_amount() {
    let env = Env::default();
    let reg = setup(&env);
    let b = id(&env, 9);
    assert_eq!(
        reg.try_record_loan(&id(&env, 1), &b, &Product::DepositBacked, &0, &200, &3, &1),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        reg.try_record_loan(&id(&env, 1), &b, &Product::DepositBacked, &1_000_00, &0, &3, &1),
        Err(Ok(Error::InvalidRate))
    );
    assert_eq!(
        reg.try_record_loan(&id(&env, 1), &b, &Product::DepositBacked, &1_000_00, &200, &24, &1),
        Err(Ok(Error::InvalidTerm))
    );
}

#[test]
fn defaulted_from_active_only() {
    let env = Env::default();
    let reg = setup(&env);
    let loan = id(&env, 1);
    let borrower = id(&env, 9);
    record(&reg, &loan, &borrower);

    // pending loan can't default
    assert_eq!(
        reg.try_mark_defaulted(&loan, &1_700_100_000),
        Err(Ok(Error::NotActive))
    );

    reg.activate(&loan, &1_700_000_000);
    reg.mark_defaulted(&loan, &1_700_100_000);
    assert_eq!(reg.get_loan(&loan).unwrap().status, Status::Defaulted);
    assert_eq!(reg.open_loan_of(&borrower), None);
}

#[test]
fn writes_require_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(LoanRegistry, ());
    let reg = LoanRegistryClient::new(&env, &contract_id);
    reg.initialize(&admin);

    env.set_auths(&[]);
    assert!(reg
        .try_record_loan(&id(&env, 1), &id(&env, 9), &Product::DepositBacked, &1_000_00, &200, &3, &1)
        .is_err());
}
