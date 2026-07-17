#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, BytesN, Env, String};

fn setup(env: &Env) -> PaymentRegistryClient<'_> {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let contract_id = env.register(PaymentRegistry, ());
    let registry = PaymentRegistryClient::new(env, &contract_id);
    registry.initialize(&admin);
    registry
}

fn id(env: &Env, byte: u8) -> BytesN<16> {
    BytesN::from_array(env, &[byte; 16])
}

#[test]
fn record_then_get_returns_the_split() {
    let env = Env::default();
    let reg = setup(&env);
    let rail = String::from_str(&env, "PAYPAL-CAPTURE-0001");
    let loan = id(&env, 1);
    let user = id(&env, 9);

    // ₱353.33 received = ₱333.33 principal + ₱20.00 interest, no excess
    reg.record_payment(&rail, &loan, &user, &353_33, &20_00, &333_33, &0, &1_700_000_000);

    let p = reg.get_payment(&rail).unwrap();
    assert_eq!(p.loan_id, loan);
    assert_eq!(p.user_id, user);
    assert_eq!(p.amount_received, 353_33);
    assert_eq!(p.interest_paid, 20_00);
    assert_eq!(p.principal_paid, 333_33);
    assert_eq!(p.excess, 0);
    assert_eq!(p.paid_at, 1_700_000_000);
}

#[test]
fn same_capture_id_cannot_double_log() {
    let env = Env::default();
    let reg = setup(&env);
    let rail = String::from_str(&env, "PAYPAL-CAPTURE-0001");
    reg.record_payment(&rail, &id(&env, 1), &id(&env, 9), &353_33, &20_00, &333_33, &0, &1_700_000_000);

    // a resent PayPal callback with the same capture id is refused
    assert_eq!(
        reg.try_record_payment(&rail, &id(&env, 1), &id(&env, 9), &353_33, &20_00, &333_33, &0, &1_700_000_000),
        Err(Ok(Error::PaymentExists))
    );
}

#[test]
fn distinct_captures_coexist() {
    let env = Env::default();
    let reg = setup(&env);
    let r1 = String::from_str(&env, "CAP-1");
    let r2 = String::from_str(&env, "CAP-2");
    reg.record_payment(&r1, &id(&env, 1), &id(&env, 9), &353_33, &20_00, &333_33, &0, &1_700_000_000);
    reg.record_payment(&r2, &id(&env, 1), &id(&env, 9), &346_66, &13_33, &333_33, &0, &1_700_100_000);

    assert_eq!(reg.get_payment(&r1).unwrap().interest_paid, 20_00);
    assert_eq!(reg.get_payment(&r2).unwrap().interest_paid, 13_33);
}

#[test]
fn rejects_non_positive_or_negative_amounts() {
    let env = Env::default();
    let reg = setup(&env);
    let rail = String::from_str(&env, "CAP-X");
    // amount_received must be > 0
    assert_eq!(
        reg.try_record_payment(&rail, &id(&env, 1), &id(&env, 9), &0, &0, &0, &0, &1),
        Err(Ok(Error::InvalidAmount))
    );
    // a negative split part is refused
    assert_eq!(
        reg.try_record_payment(&rail, &id(&env, 1), &id(&env, 9), &100_00, &-1, &100_00, &0, &1),
        Err(Ok(Error::InvalidAmount))
    );
}

#[test]
fn unknown_capture_reads_none() {
    let env = Env::default();
    let reg = setup(&env);
    assert_eq!(reg.get_payment(&String::from_str(&env, "nope")), None);
}

#[test]
fn writes_require_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(PaymentRegistry, ());
    let reg = PaymentRegistryClient::new(&env, &contract_id);
    reg.initialize(&admin);

    env.set_auths(&[]);
    assert!(reg
        .try_record_payment(
            &String::from_str(&env, "CAP-1"),
            &id(&env, 1),
            &id(&env, 9),
            &100_00,
            &0,
            &100_00,
            &0,
            &1
        )
        .is_err());
}
