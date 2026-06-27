extern crate std;

use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String};

use crate::{DataKey, Error, NormalNFT721, NormalNFT721Client};

fn jump_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|li| {
        li.sequence_number += delta;
    });
}

fn setup() -> (
    Env,
    NormalNFT721Client<'static>,
    Address, /*contract_id*/
    Address, /*creator*/
) {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    env.mock_all_auths();

    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    client.initialize(
        &creator,
        &String::from_str(&env, "Test Collection 721"),
        &String::from_str(&env, "T721"),
        &1_000u64,
        &500u32,
        &royalty_receiver,
    );

    (env, client, contract_id, creator)
}

// ── TTL tests (pre-existing) ──────────────────────────────────────────────────

#[test]
fn instance_ttl_is_extended_on_mint() {
    let (env, client, _contract_id, _creator) = setup();

    let alice = Address::generate(&env);

    // After init, instance TTL is bumped by the initializer.
    // Move past the threshold so missing "extend_instance_ttl" on mint would expire it.
    jump_ledger(&env, 60_000);
    let token_id_0 = client.mint(&alice, &String::from_str(&env, "uri-0"));

    jump_ledger(&env, 60_000);
    let token_id_1 = client.mint(&alice, &String::from_str(&env, "uri-1"));

    assert_eq!(token_id_0, 0u64);
    assert_eq!(token_id_1, 1u64);
}

#[test]
fn persistent_ttl_is_extended_on_transfer_keys() {
    let (env, client, contract_id, _creator) = setup();

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let token_id = client.mint(&alice, &String::from_str(&env, "uri"));

    client.transfer(&alice, &bob, &token_id);

    // Jump beyond TTL_THRESHOLD. If transfer() didn't extend TTL for the
    // updated keys, they'd disappear.
    jump_ledger(&env, 60_000);

    let (owner_has, alice_balance_has) = env.as_contract(&contract_id, || {
        let owner_has = env.storage().persistent().has(&DataKey::Owner(token_id));
        let alice_balance_has = env
            .storage()
            .persistent()
            .has(&DataKey::BalanceOf(alice.clone()));
        (owner_has, alice_balance_has)
    });

    assert!(owner_has);
    assert!(alice_balance_has);
    assert_eq!(client.owner_of(&token_id), bob);
}

#[test]
fn persistent_ttl_is_extended_on_burn_balance_key() {
    let (env, client, contract_id, _creator) = setup();

    let alice = Address::generate(&env);

    let token_id = client.mint(&alice, &String::from_str(&env, "uri"));
    // NormalNFT721's burn() path checks explicit approval (via Approved(token_id)),
    // so set a self-approval first to keep this test focused on TTL behavior.
    client.approve(&alice, &alice, &token_id);
    client.burn(&alice, &token_id);

    jump_ledger(&env, 60_000);

    let (owner_has, alice_balance_has) = env.as_contract(&contract_id, || {
        let owner_has = env.storage().persistent().has(&DataKey::Owner(token_id));
        let alice_balance_has = env
            .storage()
            .persistent()
            .has(&DataKey::BalanceOf(alice.clone()));
        (owner_has, alice_balance_has)
    });

    // burn() intentionally removes the token ownership key
    assert!(!owner_has);
    // but BalanceOf must still be kept alive.
    assert!(alice_balance_has);
}

// ── Query functions ───────────────────────────────────────────────────────────

#[test]
fn name_and_symbol_are_stored_correctly() {
    let (_, client, _, _) = setup();
    assert_eq!(
        client.name(),
        String::from_str(&client.env, "Test Collection 721")
    );
    assert_eq!(client.symbol(), String::from_str(&client.env, "T721"));
}

#[test]
fn total_supply_starts_at_zero() {
    let (_, client, _, _) = setup();
    assert_eq!(client.total_supply(), 0u64);
}

#[test]
fn max_supply_reflects_initialized_value() {
    let (_, client, _, _) = setup();
    assert_eq!(client.max_supply(), 1_000u64);
}

#[test]
fn balance_of_returns_zero_for_address_with_no_tokens() {
    let (env, client, _, _) = setup();
    let nobody = Address::generate(&env);
    assert_eq!(client.balance_of(&nobody), 0u64);
}

#[test]
fn royalty_info_matches_initialized_values() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    env.mock_all_auths();

    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    client.initialize(
        &creator,
        &String::from_str(&env, "Royalty Test"),
        &String::from_str(&env, "RT"),
        &100u64,
        &750u32,
        &royalty_receiver,
    );

    let (recv, bps) = client.royalty_info();
    assert_eq!(recv, royalty_receiver);
    assert_eq!(bps, 750u32);
}

// ── Minting ───────────────────────────────────────────────────────────────────

#[test]
fn mint_increments_total_supply_and_balance() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    assert_eq!(client.total_supply(), 0);
    let id0 = client.mint(&alice, &String::from_str(&env, "uri-0"));
    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.balance_of(&alice), 1);

    let id1 = client.mint(&alice, &String::from_str(&env, "uri-1"));
    assert_eq!(client.total_supply(), 2);
    assert_eq!(client.balance_of(&alice), 2);
    assert_eq!(id0, 0u64);
    assert_eq!(id1, 1u64);
}

#[test]
fn mint_sets_owner_and_token_uri() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "ipfs://Qm123"));
    assert_eq!(client.owner_of(&id), alice);
    assert_eq!(
        client.token_uri(&id),
        String::from_str(&env, "ipfs://Qm123")
    );
}

#[test]
fn mint_to_multiple_addresses_tracks_balances_independently() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint(&alice, &String::from_str(&env, "alice-uri"));
    client.mint(&bob, &String::from_str(&env, "bob-uri-1"));
    client.mint(&bob, &String::from_str(&env, "bob-uri-2"));

    assert_eq!(client.balance_of(&alice), 1);
    assert_eq!(client.balance_of(&bob), 2);
    assert_eq!(client.total_supply(), 3);
}

// ── Max supply enforcement ────────────────────────────────────────────────────

#[test]
fn mint_fails_when_max_supply_is_reached() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    env.mock_all_auths();

    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let receiver = Address::generate(&env);

    // Max supply = 2
    client.initialize(
        &creator,
        &String::from_str(&env, "Small Collection"),
        &String::from_str(&env, "SC"),
        &2u64,
        &0u32,
        &receiver,
    );

    let alice = Address::generate(&env);
    client.mint(&alice, &String::from_str(&env, "uri-0"));
    client.mint(&alice, &String::from_str(&env, "uri-1"));

    // Third mint should fail
    let result = client.try_mint(&alice, &String::from_str(&env, "uri-2"));
    assert_eq!(result, Err(Ok(Error::MaxSupplyReached)));
}

#[test]
fn cannot_initialize_twice() {
    let (env, client, _, creator) = setup();
    let receiver = Address::generate(&env);

    let result = client.try_initialize(
        &creator,
        &String::from_str(&env, "Again"),
        &String::from_str(&env, "AG"),
        &100u64,
        &0u32,
        &receiver,
    );
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

// ── Transfers ─────────────────────────────────────────────────────────────────

#[test]
fn transfer_moves_ownership_and_updates_balances() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    assert_eq!(client.owner_of(&id), alice);
    assert_eq!(client.balance_of(&alice), 1);
    assert_eq!(client.balance_of(&bob), 0);

    client.transfer(&alice, &bob, &id);

    assert_eq!(client.owner_of(&id), bob);
    assert_eq!(client.balance_of(&alice), 0);
    assert_eq!(client.balance_of(&bob), 1);
}

#[test]
fn transfer_fails_when_called_by_non_owner() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let eve = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    // Eve is not the owner and has no approval
    let result = client.try_transfer(&eve, &alice, &id);
    assert!(result.is_err());
}

#[test]
fn transfer_clears_single_token_approval() {
    let (env, client, contract_id, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    client.approve(&alice, &charlie, &id);

    // Approval is set before transfer
    let approved_before = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Approved(id))
    });
    assert!(approved_before.is_some());

    client.transfer(&alice, &bob, &id);

    // Approval must be cleared after transfer
    let approved_after = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Approved(id))
    });
    assert!(approved_after.is_none());
}

#[test]
fn transfer_from_by_approved_spender_succeeds() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let spender = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    client.approve(&alice, &spender, &id);

    client.transfer_from(&spender, &alice, &bob, &id);
    assert_eq!(client.owner_of(&id), bob);
}

#[test]
fn transfer_from_by_operator_succeeds() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let operator = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    client.set_approval_for_all(&alice, &operator, &true);

    client.transfer_from(&operator, &alice, &bob, &id);
    assert_eq!(client.owner_of(&id), bob);
}

#[test]
fn transfer_from_fails_without_approval() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let eve = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    let result = client.try_transfer_from(&eve, &alice, &bob, &id);
    assert_eq!(result, Err(Ok(Error::NotApproved)));
}

// ── Approvals ─────────────────────────────────────────────────────────────────

#[test]
fn approve_sets_single_token_approval() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    assert_eq!(client.get_approved(&id), None);

    client.approve(&alice, &bob, &id);
    assert_eq!(client.get_approved(&id), Some(bob));
}

#[test]
fn approve_by_non_owner_fails() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let eve = Address::generate(&env);
    let bob = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    let result = client.try_approve(&eve, &bob, &id);
    assert_eq!(result, Err(Ok(Error::NotApproved)));
}

#[test]
fn set_approval_for_all_and_is_approved_for_all() {
    let (env, client, _, _) = setup();
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);

    assert!(!client.is_approved_for_all(&owner, &operator));
    client.set_approval_for_all(&owner, &operator, &true);
    assert!(client.is_approved_for_all(&owner, &operator));

    client.set_approval_for_all(&owner, &operator, &false);
    assert!(!client.is_approved_for_all(&owner, &operator));
}

#[test]
fn operator_can_approve_on_behalf_of_owner() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let operator = Address::generate(&env);
    let charlie = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    client.set_approval_for_all(&alice, &operator, &true);

    // Operator should be able to call approve() for alice's token
    client.approve(&operator, &charlie, &id);
    assert_eq!(client.get_approved(&id), Some(charlie));
}

// ── Burns ─────────────────────────────────────────────────────────────────────

#[test]
fn burn_removes_token_and_decrements_supply_and_balance() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.balance_of(&alice), 1);

    client.approve(&alice, &alice, &id);
    client.burn(&alice, &id);

    assert_eq!(client.total_supply(), 0);
    assert_eq!(client.balance_of(&alice), 0);

    // ownerOf should now return TokenNotFound
    let result = client.try_owner_of(&id);
    assert_eq!(result, Err(Ok(Error::TokenNotFound)));
}

#[test]
fn burn_by_non_owner_without_approval_fails() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let eve = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    let result = client.try_burn(&eve, &id);
    assert_eq!(result, Err(Ok(Error::NotApproved)));
}

#[test]
fn burn_by_approved_spender_succeeds() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let spender = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    client.approve(&alice, &spender, &id);
    client.burn(&spender, &id);

    let result = client.try_owner_of(&id);
    assert_eq!(result, Err(Ok(Error::TokenNotFound)));
}

#[test]
fn burn_by_operator_succeeds() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let operator = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "uri"));
    client.set_approval_for_all(&alice, &operator, &true);
    client.burn(&operator, &id);

    let result = client.try_owner_of(&id);
    assert_eq!(result, Err(Ok(Error::TokenNotFound)));
}

#[test]
fn burn_nonexistent_token_fails() {
    let (_, client, _, _) = setup();
    let caller = soroban_sdk::Address::generate(&client.env);
    let result = client.try_burn(&caller, &999u64);
    assert_eq!(result, Err(Ok(Error::TokenNotFound)));
}

// ── Ownership management ──────────────────────────────────────────────────────

#[test]
fn transfer_ownership_updates_creator() {
    let (env, client, _, creator) = setup();
    let new_creator = Address::generate(&env);

    // original creator can transfer
    client.transfer_ownership(&new_creator);
    assert_eq!(client.creator(), new_creator);

    // new creator can mint
    let alice = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "new-uri"));
    assert_eq!(client.owner_of(&id), alice);
    let _ = creator; // suppress unused variable warning
}

#[test]
fn update_royalty_changes_receiver_and_bps() {
    let (env, client, _, _) = setup();
    let new_receiver = Address::generate(&env);

    client.update_royalty(&new_receiver, &250u32);
    let (recv, bps) = client.royalty_info();
    assert_eq!(recv, new_receiver);
    assert_eq!(bps, 250u32);
}

// ── Balance corruption fix test ───────────────────────────────────────────────

#[test]
fn transfer_from_zero_balance_fails_correctly() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Alice has no tokens, balance should be 0
    assert_eq!(client.balance_of(&alice), 0u64);

    // Mint a token to bob
    client.mint(&bob, &String::from_str(&env, "uri-0"));
    assert_eq!(client.balance_of(&bob), 1u64);

    // Try to transfer token 0 from alice (who doesn't own it) to bob
    // This should fail with NotApproved since Alice is not approved to transfer Bob's token
    let result = client.try_transfer_from(&alice, &alice, &bob, &0u64);
    assert_eq!(result, Err(Ok(Error::NotApproved)));
}

// ── next_token_id ─────────────────────────────────────────────────────────────

#[test]
fn next_token_id_advances_with_each_mint() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    assert_eq!(client.next_token_id(), 0u64);
    client.mint(&alice, &String::from_str(&env, "uri-0"));
    assert_eq!(client.next_token_id(), 1u64);
    client.mint(&alice, &String::from_str(&env, "uri-1"));
    assert_eq!(client.next_token_id(), 2u64);
}

// ── Metadata management ───────────────────────────────────────────────────────

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
    // No mock_all_auths — creator auth will not be satisfied.
    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &String::from_str(&env, "Test"),
        &String::from_str(&env, "T"),
        &100u64,
        &0u32,
        &royalty_receiver,
    );
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
    // Attempt update after freeze
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
    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &String::from_str(&env, "Test"),
        &String::from_str(&env, "T"),
        &100u64,
        &0u32,
        &royalty_receiver,
    );
    let result = client.try_freeze_metadata();
    assert!(result.is_err());
}

#[test]
fn token_uri_returns_base_uri_plus_token_id_when_base_set() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "https://api.example.com/metadata/"));
    let id = client.mint(&alice, &String::from_str(&env, "ignored-uri"));

    assert_eq!(
        client.token_uri(&id),
        String::from_str(&env, "https://api.example.com/metadata/0")
    );
}

#[test]
fn token_uri_base_uri_with_multiple_tokens() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "ipfs://col/"));
    client.mint(&alice, &String::from_str(&env, "u0"));
    client.mint(&alice, &String::from_str(&env, "u1"));
    let id2 = client.mint(&alice, &String::from_str(&env, "u2"));

    assert_eq!(
        client.token_uri(&0u64),
        String::from_str(&env, "ipfs://col/0")
    );
    assert_eq!(
        client.token_uri(&1u64),
        String::from_str(&env, "ipfs://col/1")
    );
    assert_eq!(
        client.token_uri(&id2),
        String::from_str(&env, "ipfs://col/2")
    );
}

#[test]
fn token_uri_falls_back_to_per_token_uri_when_no_base_uri() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    let id = client.mint(&alice, &String::from_str(&env, "ipfs://Qmabc"));
    // No base URI set — should return per-token URI unchanged.
    assert_eq!(
        client.token_uri(&id),
        String::from_str(&env, "ipfs://Qmabc")
    );
}

#[test]
fn token_uri_returns_token_not_found_for_nonexistent_token() {
    let (_, client, _, _) = setup();
    let result = client.try_token_uri(&999u64);
    assert_eq!(result, Err(Ok(Error::TokenNotFound)));
}

#[test]
fn token_uri_with_base_uri_returns_not_found_for_nonexistent_token() {
    let (env, client, _, _) = setup();
    client.set_base_uri(&String::from_str(&env, "ipfs://col/"));
    let result = client.try_token_uri(&999u64);
    assert_eq!(result, Err(Ok(Error::TokenNotFound)));
}

#[test]
fn token_uri_boundary_token_id_zero() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    client.set_base_uri(&String::from_str(&env, "https://x/"));
    let id = client.mint(&alice, &String::from_str(&env, "u"));
    assert_eq!(id, 0u64);
    assert_eq!(
        client.token_uri(&id),
        String::from_str(&env, "https://x/0")
    );
}

#[test]
fn token_uri_frozen_base_uri_still_returns_correct_uri() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.set_base_uri(&String::from_str(&env, "ipfs://frozen/"));
    let id = client.mint(&alice, &String::from_str(&env, "u"));
    client.freeze_metadata();

    // After freeze, token_uri still works correctly.
    assert_eq!(
        client.token_uri(&id),
        String::from_str(&env, "ipfs://frozen/0")
    );
    // And base URI is unchanged.
    assert_eq!(
        client.base_uri(),
        Some(String::from_str(&env, "ipfs://frozen/"))
    );
}

#[test]
fn base_uri_update_changes_token_uri_for_all_tokens() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);

    client.mint(&alice, &String::from_str(&env, "old-uri-0"));
    client.mint(&alice, &String::from_str(&env, "old-uri-1"));

    // Before base URI is set, per-token URIs are returned.
    assert_eq!(
        client.token_uri(&0u64),
        String::from_str(&env, "old-uri-0")
    );

    // Set base URI — overrides all tokens.
    client.set_base_uri(&String::from_str(&env, "https://new/"));
    assert_eq!(
        client.token_uri(&0u64),
        String::from_str(&env, "https://new/0")
    );
    assert_eq!(
        client.token_uri(&1u64),
        String::from_str(&env, "https://new/1")
    );
}

// ── Collection royalty support ────────────────────────────────────────────────

#[test]
fn royalty_info_for_uses_default_when_no_per_token_override() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    // setup() initialises with royalty_receiver and 500 bps (5%)
    let (recv, bps) = client.royalty_info();
    let (recv2, amount) = client.royalty_info_for(&id, &10_000i128);

    assert_eq!(recv, recv2);
    // 10_000 * 500 / 10_000 = 500
    assert_eq!(amount, 500i128);
    let _ = bps;
}

#[test]
fn royalty_info_for_per_token_override_wins_over_default() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_receiver = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    client.set_token_royalty(&id, &token_receiver, &1_000u32); // 10%

    let (recv, amount) = client.royalty_info_for(&id, &5_000i128);
    // 5_000 * 1_000 / 10_000 = 500
    assert_eq!(recv, token_receiver);
    assert_eq!(amount, 500i128);
}

#[test]
fn royalty_info_for_another_token_still_uses_default() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_receiver = Address::generate(&env);

    let id0 = client.mint(&alice, &String::from_str(&env, "uri0"));
    let id1 = client.mint(&alice, &String::from_str(&env, "uri1"));

    client.set_token_royalty(&id0, &token_receiver, &200u32);

    // id1 has no override — should use collection default
    let (recv1, _) = client.royalty_info_for(&id1, &1_000i128);
    let (default_recv, _) = client.royalty_info();
    assert_eq!(recv1, default_recv);
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
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

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
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    client.set_default_royalty(&recv, &0u32);

    let (_, amount) = client.royalty_info_for(&id, &100_000i128);
    assert_eq!(amount, 0i128);
}

#[test]
fn set_default_royalty_max_bps_succeeds() {
    let (env, client, _, _) = setup();
    let recv = Address::generate(&env);
    // 10_000 bps = 100% — boundary, must succeed
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
    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &String::from_str(&env, "T"),
        &String::from_str(&env, "T"),
        &100u64,
        &0u32,
        &royalty_receiver,
    );
    let result = client.try_set_default_royalty(&royalty_receiver, &500u32);
    assert!(result.is_err());
}

#[test]
fn set_token_royalty_zero_bps_returns_zero_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let token_recv = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

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
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    assert!(client.try_set_token_royalty(&id, &token_recv, &10_000u32).is_ok());

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
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    let result = client.try_set_token_royalty(&id, &token_recv, &10_001u32);
    assert_eq!(result, Err(Ok(Error::InvalidBps)));
}

#[test]
fn set_token_royalty_non_creator_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(NormalNFT721, ());
    let client = NormalNFT721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &String::from_str(&env, "T"),
        &String::from_str(&env, "T"),
        &100u64,
        &0u32,
        &royalty_receiver,
    );
    let result = client.try_set_token_royalty(&0u64, &royalty_receiver, &500u32);
    assert!(result.is_err());
}

#[test]
fn royalty_info_for_zero_sale_price_returns_zero_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    let (_, amount) = client.royalty_info_for(&id, &0i128);
    assert_eq!(amount, 0i128);
}

#[test]
fn royalty_info_for_rounds_down_fractional_amount() {
    let (env, client, _, _) = setup();
    let alice = Address::generate(&env);
    let recv = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

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
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

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
    let new_default_recv = Address::generate(&env);
    let id = client.mint(&alice, &String::from_str(&env, "uri"));

    client.set_token_royalty(&id, &token_recv, &800u32);
    // Change the default — should not affect the per-token override
    client.set_default_royalty(&new_default_recv, &100u32);

    let (recv, amount) = client.royalty_info_for(&id, &10_000i128);
    // Per-token override must still win: 10_000 * 800 / 10_000 = 800
    assert_eq!(recv, token_recv);
    assert_eq!(amount, 800i128);
}
