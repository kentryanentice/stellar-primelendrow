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

fn entry(env:&Env,reference:&str)->PaymentEntry {
    PaymentEntry{rail_ref:String::from_str(env,reference),loan_id:id(env,1),user_id:id(env,9),
        amount_received:353_33,interest_paid:20_00,principal_paid:333_33,excess:0,paid_at:1_700_000_000}
}

#[test]
fn batch_records_each_payment_and_its_financial_fields() {
    let env=Env::default();let reg=setup(&env);
    let mut payments=soroban_sdk::Vec::new(&env);
    payments.push_back(entry(&env,"BATCH-1"));payments.push_back(entry(&env,"BATCH-2"));
    reg.record_payments(&payments);
    for p in payments {
        let saved=reg.get_payment(&p.rail_ref).unwrap();
        assert_eq!(saved.loan_id,p.loan_id);assert_eq!(saved.user_id,p.user_id);
        assert_eq!(saved.amount_received,p.amount_received);assert_eq!(saved.interest_paid,p.interest_paid);
        assert_eq!(saved.principal_paid,p.principal_paid);assert_eq!(saved.excess,p.excess);assert_eq!(saved.paid_at,p.paid_at);
    }
}

#[test]
fn duplicate_in_a_batch_rolls_back_every_new_record() {
    let env=Env::default();let reg=setup(&env);
    let p=entry(&env,"DUPLICATE");
    let payments=soroban_sdk::vec![&env,p.clone(),p.clone()];
    assert_eq!(reg.try_record_payments(&payments),Err(Ok(Error::PaymentExists)));
    assert!(reg.get_payment(&p.rail_ref).is_none());
}

#[test]
fn an_invalid_later_entry_rolls_back_the_earlier_valid_one() {
    let env=Env::default();let reg=setup(&env);
    let p=entry(&env,"VALID");let mut invalid=entry(&env,"INVALID");invalid.principal_paid = -1;
    assert_eq!(reg.try_record_payments(&soroban_sdk::vec![&env,p.clone(),invalid]),Err(Ok(Error::InvalidAmount)));
    assert!(reg.get_payment(&p.rail_ref).is_none());
}

#[test]
fn batch_auth_and_size_limits_are_enforced() {
    let env=Env::default();let reg=setup(&env);
    assert_eq!(reg.try_record_payments(&soroban_sdk::Vec::new(&env)),Err(Ok(Error::InvalidBatch)));
    let mut oversized=soroban_sdk::Vec::new(&env);
    for _ in 0..=MAX_BATCH_SIZE {oversized.push_back(entry(&env,"CAP"));}
    assert_eq!(reg.try_record_payments(&oversized),Err(Ok(Error::InvalidBatch)));
    env.set_auths(&[]);
    assert!(reg.try_record_payments(&soroban_sdk::vec![&env,entry(&env,"AUTH")]).is_err());
}

#[test]
fn batch_reads_preserve_order_and_missing_entries() {
    let env=Env::default();let reg=setup(&env);let p=entry(&env,"PRESENT");
    reg.record_payments(&soroban_sdk::vec![&env,p.clone()]);
    let result=reg.get_payments(&soroban_sdk::vec![&env,String::from_str(&env,"ABSENT"),p.rail_ref]);
    assert!(result.get(0).unwrap().is_none());
    assert_eq!(result.get(1).unwrap().unwrap().principal_paid,333_33);
}
