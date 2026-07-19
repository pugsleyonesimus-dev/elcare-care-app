extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};

use crate::{DataKey, Error, NormalNFT1155, NormalNFT1155Client};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn jump_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|li| {
        li.sequence_number += delta;
    });
}

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

// ─── Pre-existing batch semantics regression ──────────────────────────────────

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

    assert!(client
        .try_mint_batch(&alice, &token_ids, &amounts, &uris)
        .is_ok());
}

#[test]
fn test_mint_batch_length_mismatch_fails() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_ids = Vec::from_array(&env, [0u64, 1u64]);
    let amounts = Vec::from_array(&env, [100u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri")]);

    assert!(client
        .try_mint_batch(&alice, &token_ids, &amounts, &uris)
        .is_err());
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

// ─── TTL persistence ──────────────────────────────────────────────────────────

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
    let (env, client, _, _) = setup();
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
    let (env, client, contract_id, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));
    client.transfer(&alice, &bob, &token_id, &3u128);

    jump_ledger(&env, 60_000);

    let (alice_bal_exists, supply_exists) = env.as_contract(&contract_id, || {
        (
            env.storage()
                .persistent()
                .has(&DataKey::Balance(alice.clone(), token_id)),
            env.storage()
                .persistent()
                .has(&DataKey::TotalSupply(token_id)),
        )
    });
    assert!(alice_bal_exists);
    assert!(supply_exists);
}

#[test]
fn persistent_ttl_is_extended_on_burn_keys() {
    let (env, client, contract_id, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));
    client.burn(&alice, &alice, &token_id, &4u128);

    jump_ledger(&env, 60_000);

    let (alice_bal_exists, supply_exists) = env.as_contract(&contract_id, || {
        (
            env.storage()
                .persistent()
                .has(&DataKey::Balance(alice.clone(), token_id)),
            env.storage()
                .persistent()
                .has(&DataKey::TotalSupply(token_id)),
        )
    });
    assert!(alice_bal_exists);
    assert!(supply_exists);
}

// ─── ERC-1155 event emission ──────────────────────────────────────────────────

#[test]
fn test_erc1155_events_emit_successfully() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let token_id = client.mint_new(&alice, &100u128, &String::from_str(&env, "uri"));
    client.transfer(&alice, &bob, &token_id, &30u128);
    client.burn(&bob, &bob, &token_id, &10u128);

    assert_eq!(client.balance_of(&alice, &token_id), 70u128);
    assert_eq!(client.balance_of(&bob, &token_id), 20u128);
}

// ─── Burn regression #273 ─────────────────────────────────────────────────────

#[test]
fn burn_with_missing_total_supply_key_returns_zero_not_amount() {
    let (env, client, contract_id, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri"));
    assert_eq!(client.total_supply(&token_id), 5u128);

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .remove(&DataKey::TotalSupply(token_id));
    });

    client.burn(&alice, &alice, &token_id, &3u128);
    assert_eq!(client.total_supply(&token_id), 0u128);
}

// ─── Supply cap and per-wallet limit (pre-existing #40) ───────────────────────

#[test]
fn max_supply_cap_enforced_on_mint() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri-0"));
    client.set_token_max_supply(&token_id, &10u128);

    client.mint(&alice, &token_id, &5u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.total_supply(&token_id), 10u128);

    let result = client.try_mint(&alice, &token_id, &1u128, &String::from_str(&env, "uri-0"));
    assert_eq!(result, Err(Ok(Error::MaxSupplyReached)));
}

#[test]
fn per_wallet_limit_enforced_on_mint() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.set_per_wallet_limit(&3u128);
    assert_eq!(client.per_wallet_limit(), 3u128);

    let token_id = client.mint_new(&alice, &3u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&alice, &token_id), 3u128);

    let result = client.try_mint(&alice, &token_id, &1u128, &String::from_str(&env, "uri-0"));
    assert_eq!(result, Err(Ok(Error::WalletLimitReached)));

    client.mint(&bob, &token_id, &3u128, &String::from_str(&env, "uri-0"));
    assert_eq!(client.wallet_minted(&bob, &token_id), 3u128);
}

#[test]
fn mint_batch_supply_cap_enforced() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri-0"));
    client.set_token_max_supply(&token_id, &10u128);

    let token_ids = Vec::from_array(&env, [token_id]);
    let amounts = Vec::from_array(&env, [6u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri-0")]);

    let result = client.try_mint_batch(&alice, &token_ids, &amounts, &uris);
    assert_eq!(result, Err(Ok(Error::MaxSupplyReached)));
}

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

#[test]
fn no_wallet_limit_allows_unlimited_mints() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    assert_eq!(client.per_wallet_limit(), 0u128);

    let token_id = client.mint_new(&alice, &1000u128, &String::from_str(&env, "uri-0"));
    client.mint(
        &alice,
        &token_id,
        &1000u128,
        &String::from_str(&env, "uri-0"),
    );
    assert_eq!(client.total_supply(&token_id), 2000u128);
}

// ─── Pause — full auth matrix ─────────────────────────────────────────────────

#[test]
fn is_paused_defaults_false() {
    let (_, client, _, _) = setup();
    assert!(!client.is_paused());
}

#[test]
fn pause_sets_flag() {
    let (_, client, _, _) = setup();
    client.pause();
    assert!(client.is_paused());
}

#[test]
fn unpause_clears_flag() {
    let (_, client, _, _) = setup();
    client.pause();
    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn pause_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    // No mock_all_auths — creator auth will not be satisfied.
    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);
    client.initialize(&creator, &String::from_str(&env, "Test"), &500u32, &recv);
    assert!(client.try_pause().is_err());
}

#[test]
fn unpause_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);
    client.initialize(&creator, &String::from_str(&env, "Test"), &500u32, &recv);
    assert!(client.try_unpause().is_err());
}

#[test]
fn pause_idempotent() {
    let (_, client, _, _) = setup();
    client.pause();
    client.pause(); // second call must not error
    assert!(client.is_paused());
}

#[test]
fn unpause_idempotent_never_paused() {
    let (_, client, _, _) = setup();
    client.unpause(); // never paused — must not error
    assert!(!client.is_paused());
}

// ─── Pause gates every state-mutating function ───────────────────────────────

#[test]
fn mint_new_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    client.pause();
    let result = client.try_mint_new(&alice, &1u128, &String::from_str(&env, "uri"));
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn mint_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));
    client.pause();
    let result = client.try_mint(&alice, &token_id, &1u128, &String::from_str(&env, "uri"));
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn mint_batch_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    client.pause();
    let ids = Vec::from_array(&env, [0u64]);
    let amts = Vec::from_array(&env, [1u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "uri")]);
    let result = client.try_mint_batch(&alice, &ids, &amts, &uris);
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn transfer_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));
    client.pause();
    let result = client.try_transfer(&alice, &bob, &token_id, &1u128);
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn transfer_from_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let operator = Address::generate(&env);
    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));
    client.set_approval_for_all(&alice, &operator, &true);
    client.pause();
    let result = client.try_transfer_from(&operator, &alice, &bob, &token_id, &1u128);
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn batch_transfer_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));
    client.pause();
    let ids = Vec::from_array(&env, [token_id]);
    let amts = Vec::from_array(&env, [1u128]);
    let result = client.try_batch_transfer(&alice, &alice, &bob, &ids, &amts);
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn burn_blocked_when_paused() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_id = client.mint_new(&alice, &10u128, &String::from_str(&env, "uri"));
    client.pause();
    let result = client.try_burn(&alice, &alice, &token_id, &1u128);
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));
}

#[test]
fn mint_new_succeeds_after_unpause() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    client.pause();
    client.unpause();
    let id = client.mint_new(&alice, &5u128, &String::from_str(&env, "uri"));
    assert_eq!(client.balance_of(&alice, &id), 5u128);
}

#[test]
fn multiple_pause_unpause_cycles_work() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.pause();
    assert!(client
        .try_mint_new(&alice, &1u128, &String::from_str(&env, "u"))
        .is_err());

    client.unpause();
    client.mint_new(&alice, &1u128, &String::from_str(&env, "u"));

    client.pause();
    assert!(client
        .try_mint_new(&alice, &1u128, &String::from_str(&env, "u"))
        .is_err());

    client.unpause();
    client.mint_new(&alice, &1u128, &String::from_str(&env, "u"));
}

#[test]
fn pause_interleaved_with_batch_ops() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Mint before pause
    let ids = Vec::from_array(&env, [0u64, 1u64]);
    let amts = Vec::from_array(&env, [10u128, 20u128]);
    let uris = Vec::from_array(
        &env,
        [String::from_str(&env, "u0"), String::from_str(&env, "u1")],
    );
    client.mint_batch(&alice, &ids, &amts, &uris);

    // Pause blocks batch_transfer
    client.pause();
    let result = client.try_batch_transfer(&alice, &alice, &bob, &ids, &amts);
    assert_eq!(result, Err(Ok(Error::CollectionPaused)));

    // Unpause allows it
    client.unpause();
    let sub_amts = Vec::from_array(&env, [5u128, 5u128]);
    client.batch_transfer(&alice, &alice, &bob, &ids, &sub_amts);
    assert_eq!(client.balance_of(&bob, &0u64), 5u128);
    assert_eq!(client.balance_of(&bob, &1u64), 5u128);
}

// ─── Metadata management ──────────────────────────────────────────────────────

#[test]
fn base_uri_initially_none() {
    let (_, client, _, _) = setup();
    assert_eq!(client.base_uri(), None);
}

#[test]
fn set_base_uri_stores_value() {
    let (env, client, _, _) = setup();
    client.set_base_uri(&String::from_str(&env, "ipfs://base/"));
    assert_eq!(
        client.base_uri(),
        Some(String::from_str(&env, "ipfs://base/"))
    );
}

#[test]
fn set_base_uri_can_be_updated_before_freeze() {
    let (env, client, _, _) = setup();
    client.set_base_uri(&String::from_str(&env, "ipfs://v1/"));
    client.set_base_uri(&String::from_str(&env, "ipfs://v2/"));
    assert_eq!(
        client.base_uri(),
        Some(String::from_str(&env, "ipfs://v2/"))
    );
}

#[test]
fn set_base_uri_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);
    client.initialize(&creator, &String::from_str(&env, "T"), &0u32, &recv);
    let result = client.try_set_base_uri(&String::from_str(&env, "ipfs://bad/"));
    assert!(result.is_err());
}

#[test]
fn is_metadata_frozen_defaults_false() {
    let (_, client, _, _) = setup();
    assert!(!client.is_metadata_frozen());
}

#[test]
fn freeze_metadata_sets_frozen_flag() {
    let (_, client, _, _) = setup();
    client.freeze_metadata();
    assert!(client.is_metadata_frozen());
}

#[test]
fn freeze_metadata_twice_returns_already_frozen() {
    let (_, client, _, _) = setup();
    client.freeze_metadata();
    let result = client.try_freeze_metadata();
    assert_eq!(result, Err(Ok(Error::AlreadyFrozen)));
}

#[test]
fn set_base_uri_after_freeze_returns_metadata_frozen() {
    let (env, client, _, _) = setup();
    client.freeze_metadata();
    let result = client.try_set_base_uri(&String::from_str(&env, "ipfs://late/"));
    assert_eq!(result, Err(Ok(Error::MetadataFrozen)));
}

#[test]
fn set_base_uri_before_freeze_then_frozen_rejects_update() {
    let (env, client, _, _) = setup();
    client.set_base_uri(&String::from_str(&env, "ipfs://ok/"));
    client.freeze_metadata();
    let result = client.try_set_base_uri(&String::from_str(&env, "ipfs://nope/"));
    assert_eq!(result, Err(Ok(Error::MetadataFrozen)));
    // Original base URI still intact
    assert_eq!(
        client.base_uri(),
        Some(String::from_str(&env, "ipfs://ok/"))
    );
}

#[test]
fn freeze_metadata_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);
    client.initialize(&creator, &String::from_str(&env, "T"), &0u32, &recv);
    let result = client.try_freeze_metadata();
    assert!(result.is_err());
}

#[test]
fn freeze_persists_after_ownership_transfer() {
    let (env, client, _, _) = setup();
    let new_creator = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "ipfs://frozen/"));
    client.freeze_metadata();
    client.transfer_ownership(&new_creator);

    // After ownership transfer, metadata must still be frozen
    assert!(client.is_metadata_frozen());
    let result = client.try_set_base_uri(&String::from_str(&env, "ipfs://nope/"));
    assert_eq!(result, Err(Ok(Error::MetadataFrozen)));
}

// ─── URI resolution order ─────────────────────────────────────────────────────

#[test]
fn uri_returns_per_token_uri_when_no_base_uri() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "ipfs://Qm123"));
    assert_eq!(client.uri(&id), String::from_str(&env, "ipfs://Qm123"));
}

#[test]
fn uri_returns_base_uri_plus_token_id_when_base_set() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "https://api.example.com/metadata/"));
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "ignored"));

    assert_eq!(
        client.uri(&id),
        String::from_str(&env, "https://api.example.com/metadata/0")
    );
}

#[test]
fn uri_base_uri_with_multiple_tokens() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "ipfs://col/"));
    client.mint_new(&alice, &1u128, &String::from_str(&env, "u0"));
    client.mint_new(&alice, &1u128, &String::from_str(&env, "u1"));
    client.mint_new(&alice, &1u128, &String::from_str(&env, "u2"));

    assert_eq!(client.uri(&0u64), String::from_str(&env, "ipfs://col/0"));
    assert_eq!(client.uri(&1u64), String::from_str(&env, "ipfs://col/1"));
    assert_eq!(client.uri(&2u64), String::from_str(&env, "ipfs://col/2"));
}

#[test]
fn uri_frozen_base_uri_still_resolves_correctly() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "ipfs://frozen/"));
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "u"));
    client.freeze_metadata();

    assert_eq!(client.uri(&id), String::from_str(&env, "ipfs://frozen/0"));
    assert_eq!(
        client.base_uri(),
        Some(String::from_str(&env, "ipfs://frozen/"))
    );
}

#[test]
fn uri_base_uri_update_changes_resolution_for_all_tokens() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.mint_new(&alice, &1u128, &String::from_str(&env, "old-0"));
    client.mint_new(&alice, &1u128, &String::from_str(&env, "old-1"));

    // Before base URI: per-token URIs returned
    assert_eq!(client.uri(&0u64), String::from_str(&env, "old-0"));

    // Set base URI — overrides resolution for all tokens
    client.set_base_uri(&String::from_str(&env, "https://new/"));
    assert_eq!(client.uri(&0u64), String::from_str(&env, "https://new/0"));
    assert_eq!(client.uri(&1u64), String::from_str(&env, "https://new/1"));
}

#[test]
fn freeze_then_set_base_uri_reverts() {
    let (env, client, _, _) = setup();
    client.freeze_metadata();
    let result = client.try_set_base_uri(&String::from_str(&env, "ipfs://any/"));
    assert_eq!(result, Err(Ok(Error::MetadataFrozen)));
}

// ─── Per-token royalties (ERC-2981 parity) ────────────────────────────────────

#[test]
fn royalty_info_returns_initialized_defaults() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    env.mock_all_auths();

    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);

    client.initialize(&creator, &String::from_str(&env, "T"), &750u32, &recv);

    let (r, bps) = client.royalty_info();
    assert_eq!(r, recv);
    assert_eq!(bps, 750u32);
}

#[test]
fn royalty_info_for_uses_default_when_no_per_token_override() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    let (default_recv, _) = client.royalty_info();
    let (recv, amount) = client.royalty_info_for(&id, &10_000i128);

    assert_eq!(recv, default_recv);
    // 10_000 * 500 / 10_000 = 500
    assert_eq!(amount, 500i128);
}

#[test]
fn royalty_info_for_per_token_override_wins_over_default() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_token_royalty(&id, &token_recv, &1_000u32); // 10%

    let (recv, amount) = client.royalty_info_for(&id, &5_000i128);
    // 5_000 * 1_000 / 10_000 = 500
    assert_eq!(recv, token_recv);
    assert_eq!(amount, 500i128);
}

#[test]
fn royalty_info_for_another_token_uses_default() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);

    let id0 = client.mint_new(&alice, &1u128, &String::from_str(&env, "u0"));
    let id1 = client.mint_new(&alice, &1u128, &String::from_str(&env, "u1"));

    client.set_token_royalty(&id0, &token_recv, &200u32);

    let (default_recv, _) = client.royalty_info();
    let (r1, _) = client.royalty_info_for(&id1, &1_000i128);
    assert_eq!(r1, default_recv);
}

#[test]
fn set_default_royalty_updates_collection_royalty() {
    let (env, client, _, _) = setup();
    let new_recv = Address::generate(&env);

    client.set_default_royalty(&new_recv, &300u32);

    let (recv, bps) = client.royalty_info();
    assert_eq!(recv, new_recv);
    assert_eq!(bps, 300u32);
}

#[test]
fn set_default_royalty_reflected_in_royalty_info_for() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let new_recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_default_royalty(&new_recv, &250u32); // 2.5%

    let (recv, amount) = client.royalty_info_for(&id, &8_000i128);
    // 8_000 * 250 / 10_000 = 200
    assert_eq!(recv, new_recv);
    assert_eq!(amount, 200i128);
}

#[test]
fn set_default_royalty_zero_bps_returns_zero_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_default_royalty(&recv, &0u32);
    let (_, amount) = client.royalty_info_for(&id, &100_000i128);
    assert_eq!(amount, 0i128);
}

#[test]
fn set_default_royalty_max_bps_succeeds() {
    let (env, client, _, _) = setup();
    let recv = Address::generate(&env);
    assert!(client.try_set_default_royalty(&recv, &10_000u32).is_ok());
    let (_, bps) = client.royalty_info();
    assert_eq!(bps, 10_000u32);
}

#[test]
fn set_default_royalty_exceeds_max_bps_returns_invalid_bps() {
    let (env, client, _, _) = setup();
    let recv = Address::generate(&env);
    let result = client.try_set_default_royalty(&recv, &10_001u32);
    assert_eq!(result, Err(Ok(Error::InvalidBps)));
}

#[test]
fn set_default_royalty_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);
    client.initialize(&creator, &String::from_str(&env, "T"), &0u32, &recv);
    let result = client.try_set_default_royalty(&recv, &500u32);
    assert!(result.is_err());
}

#[test]
fn set_token_royalty_zero_bps_returns_zero_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_token_royalty(&id, &token_recv, &0u32);
    let (recv, amount) = client.royalty_info_for(&id, &999_999i128);
    assert_eq!(recv, token_recv);
    assert_eq!(amount, 0i128);
}

#[test]
fn set_token_royalty_max_bps_succeeds() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    assert!(client
        .try_set_token_royalty(&id, &token_recv, &10_000u32)
        .is_ok());
    let (recv, amount) = client.royalty_info_for(&id, &1_000i128);
    assert_eq!(recv, token_recv);
    // 1_000 * 10_000 / 10_000 = 1_000
    assert_eq!(amount, 1_000i128);
}

#[test]
fn set_token_royalty_exceeds_max_bps_returns_invalid_bps() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    let result = client.try_set_token_royalty(&id, &token_recv, &10_001u32);
    assert_eq!(result, Err(Ok(Error::InvalidBps)));
}

#[test]
fn set_token_royalty_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(NormalNFT1155, ());
    let client = NormalNFT1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let recv = Address::generate(&env);
    client.initialize(&creator, &String::from_str(&env, "T"), &0u32, &recv);
    let result = client.try_set_token_royalty(&0u64, &recv, &500u32);
    assert!(result.is_err());
}

#[test]
fn royalty_info_for_zero_sale_price_returns_zero_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    let (_, amount) = client.royalty_info_for(&id, &0i128);
    assert_eq!(amount, 0i128);
}

#[test]
fn royalty_info_for_rounds_down_fractional_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_default_royalty(&recv, &333u32); // 3.33%
                                                // 1_000 * 333 / 10_000 = 33.3 → rounds down to 33
    let (_, amount) = client.royalty_info_for(&id, &1_000i128);
    assert_eq!(amount, 33i128);
}

#[test]
fn per_token_royalty_can_be_updated() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let recv1 = Address::generate(&env);
    let recv2 = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_token_royalty(&id, &recv1, &100u32);
    let (r1, _) = client.royalty_info_for(&id, &1_000i128);
    assert_eq!(r1, recv1);

    client.set_token_royalty(&id, &recv2, &200u32);
    let (r2, amount) = client.royalty_info_for(&id, &1_000i128);
    assert_eq!(r2, recv2);
    // 1_000 * 200 / 10_000 = 20
    assert_eq!(amount, 20i128);
}

#[test]
fn default_royalty_does_not_affect_per_token_override() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);
    let new_default = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.set_token_royalty(&id, &token_recv, &800u32);
    client.set_default_royalty(&new_default, &100u32);

    let (recv, amount) = client.royalty_info_for(&id, &10_000i128);
    // Per-token override must win: 10_000 * 800 / 10_000 = 800
    assert_eq!(recv, token_recv);
    assert_eq!(amount, 800i128);
}

/// Royalty math property checks: bps ∈ {0, 1, 250, 10_000} × sale prices
/// including 1 and a large value. Results must match 721 rounding exactly.
#[test]
fn royalty_math_property_checks_match_721_rounding() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    let cases: &[(u32, i128, i128)] = &[
        (0, 1, 0),
        (0, 1_000_000, 0),
        (1, 1, 0),                // 1 * 1 / 10_000 = 0
        (1, 10_000, 1),           // 10_000 * 1 / 10_000 = 1
        (250, 1, 0),              // 1 * 250 / 10_000 = 0
        (250, 10_000, 250),       // 10_000 * 250 / 10_000 = 250
        (250, 100_000, 2_500),    // 100_000 * 250 / 10_000 = 2500
        (10_000, 1, 1),           // 1 * 10_000 / 10_000 = 1
        (10_000, 10_000, 10_000), // 100%
        // Large safe value: i128::MAX / 10_000 avoids overflow
        (500, i128::MAX / 10_000, (i128::MAX / 10_000) * 500 / 10_000),
    ];

    for &(bps, sale_price, expected_amount) in cases {
        client.set_default_royalty(&recv, &bps);
        let (_, amount) = client.royalty_info_for(&id, &sale_price);
        assert_eq!(
            amount, expected_amount,
            "bps={bps} sale_price={sale_price}: expected {expected_amount}, got {amount}"
        );
    }
}

// ─── update_royalty backward compatibility ────────────────────────────────────

#[test]
fn update_royalty_still_works_as_legacy_setter() {
    let (env, client, _, _) = setup();
    let new_recv = Address::generate(&env);

    client.update_royalty(&new_recv, &250u32);
    let (recv, bps) = client.royalty_info();
    assert_eq!(recv, new_recv);
    assert_eq!(bps, 250u32);
}

#[test]
fn update_royalty_is_reflected_in_royalty_info_for() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let new_recv = Address::generate(&env);
    let id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));

    client.update_royalty(&new_recv, &100u32);
    let (recv, amount) = client.royalty_info_for(&id, &10_000i128);
    assert_eq!(recv, new_recv);
    assert_eq!(amount, 100i128);
}

// ─── mint_batch invariant hardening — duplicate IDs ───────────────────────────

/// Two entries for the same token_id in one batch must be accumulated before
/// checking the supply cap. Without accumulation the pre-flight check passes
/// twice on the stale supply, allowing the cap to be blown.
#[test]
fn mint_batch_duplicate_ids_cannot_exceed_max_supply() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    // Create token 0 with a max supply of 5.
    let token_id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));
    client.set_token_max_supply(&token_id, &5u128);

    // Batch asks for token_id twice: 3 + 3 = 6 > cap 5 (current supply = 1)
    let ids = Vec::from_array(&env, [token_id, token_id]);
    let amts = Vec::from_array(&env, [3u128, 3u128]);
    let uris = Vec::from_array(
        &env,
        [String::from_str(&env, "u"), String::from_str(&env, "u")],
    );

    let result = client.try_mint_batch(&alice, &ids, &amts, &uris);
    assert_eq!(result, Err(Ok(Error::MaxSupplyReached)));

    // Supply must not have changed.
    assert_eq!(client.total_supply(&token_id), 1u128);
}

#[test]
fn mint_batch_duplicate_ids_exactly_at_max_supply_succeeds() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &0u128, &String::from_str(&env, "uri"));
    client.set_token_max_supply(&token_id, &6u128);

    // Batch: 3 + 3 = exactly 6 == cap
    let ids = Vec::from_array(&env, [token_id, token_id]);
    let amts = Vec::from_array(&env, [3u128, 3u128]);
    let uris = Vec::from_array(
        &env,
        [String::from_str(&env, "u"), String::from_str(&env, "u")],
    );

    client.mint_batch(&alice, &ids, &amts, &uris);
    assert_eq!(client.total_supply(&token_id), 6u128);

    // One more should fail
    let ids2 = Vec::from_array(&env, [token_id]);
    let amts2 = Vec::from_array(&env, [1u128]);
    let uris2 = Vec::from_array(&env, [String::from_str(&env, "u")]);
    let result = client.try_mint_batch(&alice, &ids2, &amts2, &uris2);
    assert_eq!(result, Err(Ok(Error::MaxSupplyReached)));
}

#[test]
fn mint_batch_duplicate_ids_cannot_exceed_wallet_limit() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_per_wallet_limit(&5u128);
    let token_id = client.mint_new(&alice, &1u128, &String::from_str(&env, "uri"));
    // alice has already minted 1 for token_id; limit = 5, so 4 remaining.
    // Batch: 3 + 3 = 6 total attempted for token_id > limit 5
    let ids = Vec::from_array(&env, [token_id, token_id]);
    let amts = Vec::from_array(&env, [3u128, 3u128]);
    let uris = Vec::from_array(
        &env,
        [String::from_str(&env, "u"), String::from_str(&env, "u")],
    );

    let result = client.try_mint_batch(&alice, &ids, &amts, &uris);
    assert_eq!(result, Err(Ok(Error::WalletLimitReached)));
    assert_eq!(client.wallet_minted(&alice, &token_id), 1u128);
}

#[test]
fn mint_batch_supply_boundary_mint_to_exactly_max_then_one_more_reverts() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let token_id = client.mint_new(&alice, &0u128, &String::from_str(&env, "uri"));
    client.set_token_max_supply(&token_id, &10u128);

    // Mint exactly 10 — must succeed
    let ids = Vec::from_array(&env, [token_id]);
    let amts = Vec::from_array(&env, [10u128]);
    let uris = Vec::from_array(&env, [String::from_str(&env, "u")]);
    client.mint_batch(&alice, &ids, &amts, &uris);
    assert_eq!(client.total_supply(&token_id), 10u128);

    // +1 must revert
    let ids2 = Vec::from_array(&env, [token_id]);
    let amts2 = Vec::from_array(&env, [1u128]);
    let uris2 = Vec::from_array(&env, [String::from_str(&env, "u")]);
    let result = client.try_mint_batch(&alice, &ids2, &amts2, &uris2);
    assert_eq!(result, Err(Ok(Error::MaxSupplyReached)));
}

// ─── balance_of_batch ────────────────────────────────────────────────────────

#[test]
fn balance_of_batch_returns_correct_values() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let t0 = client.mint_new(&alice, &10u128, &String::from_str(&env, "u0"));
    let t1 = client.mint_new(&bob, &20u128, &String::from_str(&env, "u1"));

    let accounts = Vec::from_array(&env, [alice.clone(), bob.clone()]);
    let ids = Vec::from_array(&env, [t0, t1]);
    let balances = client.balance_of_batch(&accounts, &ids);

    assert_eq!(balances.get(0).unwrap(), 10u128);
    assert_eq!(balances.get(1).unwrap(), 20u128);
}

#[test]
fn balance_of_batch_mismatched_lengths_returns_empty() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let accounts = Vec::from_array(&env, [alice]);
    let ids: Vec<u64> = Vec::from_array(&env, [0u64, 1u64]);
    let result = client.balance_of_batch(&accounts, &ids);
    assert_eq!(result.len(), 0);
}
