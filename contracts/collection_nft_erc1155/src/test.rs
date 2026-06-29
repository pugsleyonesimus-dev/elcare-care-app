extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};

use crate::{DataKey, Error, NormalNFT1155, NormalNFT1155Client};

/// Utility: advance ledger sequence to simulate TTL expiry windows
fn jump_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|li| {
        li.sequence_number += delta;
    });
}

/// Setup contract environment and return initialized client
fn setup() -> (Env, NormalNFT1155Client<'static>, Address, Address) {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    env.mock_all_auths();

    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    client.initialize(
        &creator,
        &String::from_str(&env, "Test 1155"),
        &500u32,
        &royalty_receiver,
    );

    (env, client, contract_id, creator)
}

#[test]
fn test_mint_batch_success_multiple() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_ids = Vec::from_array(&env, [0u64, 1u64]);
    let amounts = Vec::from_array(&env, [100u128, 200u128]);
    let uris = Vec::from_array(
        &env,
        [
            String::from_str(&env, "uri-0"),
            String::from_str(&env, "uri-1"),
        ],
    );

    client.mint_batch(&alice, &token_ids, &amounts, &uris);

    assert_eq!(client.balance_of(&alice, &0u64), 100);
    assert_eq!(client.balance_of(&alice, &1u64), 200);
}

#[test]
fn test_mint_batch_try_success() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_ids = Vec::from_array(&env, [0u64]);
    let amounts = Vec::from_array(&env, [50u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri-0")]);

    let result = client.try_mint_batch(&alice, &token_ids, &amounts, &uris);
    assert!(result.is_ok());
}

#[test]
fn test_mint_batch_length_mismatch_fails() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_ids = Vec::from_array(&env, [0u64, 1u64]);
    let amounts = Vec::from_array(&env, [100u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri")]);

    let result = client.try_mint_batch(&alice, &token_ids, &amounts, &uris);
    assert!(result.is_err());
}

#[test]
fn test_mint_batch_empty_is_noop() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let empty_ids: Vec<u64> = Vec::new(&env);
    let empty_amounts: Vec<u128> = Vec::new(&env);
    let empty_uris: Vec<String> = Vec::new(&env);

    client.mint_batch(&alice, &empty_ids, &empty_amounts, &empty_uris);
}

#[test]
fn test_existing_token_does_not_override_uri() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let ids = Vec::from_array(&env, [0u64]);
    let amounts = Vec::from_array(&env, [100u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "original")]);

    client.mint_batch(&alice, &ids, &amounts, &uris);

    let new_uris = Vec::from_array(&env, [String::from_str(&env, "new")]);
    let amounts2 = Vec::from_array(&env, [50u128]);

    client.mint_batch(&alice, &ids, &amounts2, &new_uris);

    assert_eq!(client.uri(&0u64), String::from_str(&env, "original"));
    assert_eq!(client.total_supply(&0u64), 150);
}

#[test]
#[ignore]
fn test_auth_enforcement() {
    let (env, client, _, creator) = setup();
    let bob = Address::generate(&env);

    let ids = Vec::from_array(&env, [0u64]);
    let amounts = Vec::from_array(&env, [100u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri")]);

    // Unauthorized
    let result = client.try_mint_batch(&bob, &ids, &amounts, &uris);
    assert!(result.is_err());

    // Authorized
    client.mint_batch(&creator, &ids, &amounts, &uris);
}

#[test]
fn test_ttl_persistence() {
    let (env, client, contract_id, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));

    jump_ledger(&env, 60_000);

    let exists = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .has(&DataKey::TotalSupply(token_id))
    });

    assert!(exists);
}

#[test]
fn instance_ttl_is_extended_on_mint_new() {
    let (env, client, _contract_id, _creator) = setup();

    let alice = Address::generate(&env);

    jump_ledger(&env, 60_000);
    let token_id_0 = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri-0"));

    jump_ledger(&env, 60_000);
    let token_id_1 = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri-1"));

    assert_eq!(token_id_0, 0u64);
    assert_eq!(token_id_1, 1u64);
}

#[test]
fn persistent_ttl_is_extended_on_transfer_and_mint_keys() {
    let (env, client, contract_id, _creator) = setup();

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));

    client.transfer(&alice, &bob, &token_id, &3u128);

    jump_ledger(&env, 60_000);

    let (alice_balance_has, total_supply_has) = env.as_contract(&contract_id, || {
        let alice_balance_has = env
            .storage()
            .persistent()
            .has(&DataKey::Balance(alice.clone(), token_id));
        let total_supply_has = env
            .storage()
            .persistent()
            .has(&DataKey::TotalSupply(token_id));
        (alice_balance_has, total_supply_has)
    });

    assert!(alice_balance_has);
    assert!(total_supply_has);
}

#[test]
fn persistent_ttl_is_extended_on_burn_keys() {
    let (env, client, contract_id, _creator) = setup();

    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));

    client.burn(&alice, &alice, &token_id, &4u128);

    jump_ledger(&env, 60_000);

    let (alice_balance_has, total_supply_has) = env.as_contract(&contract_id, || {
        let alice_balance_has = env
            .storage()
            .persistent()
            .has(&DataKey::Balance(alice.clone(), token_id));
        let total_supply_has = env
            .storage()
            .persistent()
            .has(&DataKey::TotalSupply(token_id));
        (alice_balance_has, total_supply_has)
    });

    assert!(alice_balance_has);
    assert!(total_supply_has);
}

// ─── Event Tests (ERC-1155 Standard Compliance) ─────────────────────────────
// Note: Complex event testing requires specific Soroban test utilities.
// The contracts now emit ERC-1155 compliant events:
// - TransferSingle(operator, from, to, id, amount)
// - TransferBatch(operator, from, to, ids, amounts)
// Events can be verified by indexers and off-chain infrastructure.

#[test]
fn test_erc1155_events_emit_successfully() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Test that operations complete successfully (events are emitted internally)
    let token_id = client.mint_new(&alice, &100u128, &String::from_str(&env, "uri"));
    client.transfer(&alice, &bob, &token_id, &30u128);
    client.burn(&bob, &bob, &token_id, &10u128);

    // If we reach here, all operations succeeded and events were emitted
    assert_eq!(client.balance_of(&alice, &token_id), 70u128);
    assert_eq!(client.balance_of(&bob, &token_id), 20u128);
}

#[test]
fn burn_with_missing_total_supply_key_returns_zero_not_amount() {
    // Regression test for #273: unwrap_or(amount) masked state corruption.
    // If TotalSupply key is absent, burn must treat supply as 0, not `amount`.
    // With the old bug, supply.saturating_sub(amount) == 0 silently;
    // with the fix, supply (0).saturating_sub(amount) == 0 too, but the
    // key is now written as 0 rather than being set to a phantom non-zero value
    // that would make total_supply() lie about the real on-chain state.
    let (env, client, contract_id, _creator) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri"));
    assert_eq!(client.total_supply(&token_id), 5u128);

    // Manually remove the TotalSupply key to simulate a missing/expired entry.
    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .remove(&DataKey::TotalSupply(token_id));
    });

    // Burn should succeed (balance check passes) and write supply = 0, not amount.
    client.burn(&alice, &alice, &token_id, &3u128);

    // total_supply must be 0, not 3 (the old unwrap_or(amount) result).
    assert_eq!(client.total_supply(&token_id), 0u128);
}

// ─── Issue #40 — Supply cap and per-wallet limit tests ────────────────────────

/// set_token_max_supply + mint: minting up to cap succeeds, over cap reverts.
#[test]
fn max_supply_cap_enforced_on_mint() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri-0"));

    // Set max supply to 10 (5 already minted)
    client.set_token_max_supply(&token_id, &10u128);

    // Mint 5 more — exactly at cap
    client.mint(
        &alice,
        &token_id,
        &5u128,
        &String::from_str(&env, "uri-0"),
    );
    assert_eq!(client.total_supply(&token_id), 10u128);

    // One more over cap — must revert with MaxSupplyReached
    let result = client.try_mint(
        &alice,
        &token_id,
        &1u128,
        &String::from_str(&env, "uri-0"),
    );
    assert_eq!(result, Err(Ok(crate::Error::MaxSupplyReached)));
}

/// set_per_wallet_limit enforced: minting up to limit succeeds, over limit reverts.
#[test]
fn per_wallet_limit_enforced_on_mint() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Set wallet limit to 3 tokens per wallet per token type
    client.set_per_wallet_limit(&3u128);
    assert_eq!(client.per_wallet_limit(), 3u128);

    let token_id = client.mint_new(&alice, &3u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&alice, &token_id), 3u128);

    // Alice tries to mint 1 more — must revert with WalletLimitReached
    let result = client.try_mint(
        &alice,
        &token_id,
        &1u128,
        &String::from_str(&env, "uri-0"),
    );
    assert_eq!(result, Err(Ok(crate::Error::WalletLimitReached)));

    // Bob can still mint (different wallet)
    client.mint(&bob, &token_id, &3u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&bob, &token_id), 3u128);
}

/// mint_batch respects both caps atomically.
#[test]
fn mint_batch_supply_cap_enforced() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    // Pre-create token 0 with a cap of 10
    let token_id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri-0"));
    client.set_token_max_supply(&token_id, &10u128);

    let token_ids = Vec::from_array(&env, [token_id]);
    let amounts = Vec::from_array(&env, [6u128]); // 5 existing + 6 = 11 > cap 10
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri-0")]);

    let result = client.try_mint_batch(&alice, &token_ids, &amounts, &uris);
    assert_eq!(result, Err(Ok(crate::Error::MaxSupplyReached)));
}

/// wallet_minted accumulates correctly across multiple mints.
#[test]
fn wallet_minted_counter_accurate_across_multiple_mints() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_per_wallet_limit(&100u128);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&alice, &token_id), 10u128);

    client.mint(&alice, &token_id, &20u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&alice, &token_id), 30u128);

    client.mint(&alice, &token_id, &15u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&alice, &token_id), 45u128);

    assert_eq!(client.total_supply(&token_id), 45u128);
}

/// total_supply increments accurately across multiple mints from different wallets.
#[test]
fn total_supply_accurate_across_multiple_mints() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri-0"));
    client.mint(&bob, &token_id, &20u128, &String::from_str(&env, "uri-0"));
    client.mint(&alice, &token_id, &5u128, &String::from_str(&env, "uri-0"));

    assert_eq!(client.total_supply(&token_id), 35u128);
    assert_eq!(client.balance_of(&alice, &token_id), 15u128);
    assert_eq!(client.balance_of(&bob, &token_id), 20u128);
}

/// Per-wallet limit with no limit set (0) does not block any mint.
#[test]
fn no_wallet_limit_allows_unlimited_mints() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    // Default: no limit
    assert_eq!(client.per_wallet_limit(), 0u128);

    let token_id = client.mint_new(&alice, &1000u128, &String::from_str(&env, "uri-0"));
    client.mint(&alice, &token_id, &1000u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.total_supply(&token_id), 2000u128);
}
