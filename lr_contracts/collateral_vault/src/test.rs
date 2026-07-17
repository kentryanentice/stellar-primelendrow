#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;

fn setup(env: &Env) -> (CollateralVaultClient<'_>, Address, Address, token::StellarAssetClient<'_>, token::TokenClient<'_>) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let depositor = Address::generate(env);

    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let asset = token::StellarAssetClient::new(env, &sac.address());
    let token_client = token::TokenClient::new(env, &sac.address());

    let contract_id = env.register(CollateralVault, ());
    let vault = CollateralVaultClient::new(env, &contract_id);
    vault.initialize(&admin, &sac.address());

    asset.mint(&depositor, &1_000_0000000);
    (vault, admin, depositor, asset, token_client)
}

fn loan_id(env: &Env) -> BytesN<16> {
    BytesN::from_array(env, &[7u8; 16])
}

#[test]
fn lock_then_release_returns_funds_to_depositor() {
    let env = Env::default();
    let (vault, _admin, depositor, _asset, token_client) = setup(&env);
    let id = loan_id(&env);

    vault.lock(&depositor, &id, &250_0000000);
    assert_eq!(token_client.balance(&depositor), 750_0000000);
    assert_eq!(vault.get_lock(&id).unwrap().amount, 250_0000000);

    vault.release(&id);
    assert_eq!(token_client.balance(&depositor), 1_000_0000000);
    assert!(vault.get_lock(&id).is_none());
}

#[test]
fn seize_sends_funds_to_treasury() {
    let env = Env::default();
    let (vault, _admin, depositor, _asset, token_client) = setup(&env);
    let treasury = Address::generate(&env);
    let id = loan_id(&env);

    vault.lock(&depositor, &id, &100_0000000);
    vault.seize(&id, &treasury);

    assert_eq!(token_client.balance(&treasury), 100_0000000);
    assert!(vault.get_lock(&id).is_none());
}

#[test]
fn duplicate_lock_for_same_loan_is_refused() {
    let env = Env::default();
    let (vault, _admin, depositor, _asset, _token_client) = setup(&env);
    let id = loan_id(&env);

    vault.lock(&depositor, &id, &10_0000000);
    assert_eq!(
        vault.try_lock(&depositor, &id, &10_0000000),
        Err(Ok(Error::LockExists))
    );
}

#[test]
fn release_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    token::StellarAssetClient::new(&env, &sac.address()).mint(&depositor, &50_0000000);

    let contract_id = env.register(CollateralVault, ());
    let vault = CollateralVaultClient::new(&env, &contract_id);
    vault.initialize(&admin, &sac.address());
    vault.lock(&depositor, &loan_id(&env), &50_0000000);

    // From here, only allow explicitly-listed auths: none. The release
    // must then fail auth, proving a non-admin (or unsigned) call cannot
    // move funds out.
    env.set_auths(&[]);
    assert!(vault.try_release(&loan_id(&env)).is_err());
}
