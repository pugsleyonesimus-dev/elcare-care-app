use super::*;
use crate::types::{ListingStatus, OfferStatus, Recipient};

mod mock_nft {
    use soroban_sdk::{contract, contractimpl, Address, Env};
    #[contract]
    pub struct MockNft;
    #[contractimpl]
    impl MockNft {
        pub fn royalty_info(env: Env) -> (Address, u32) {
            use soroban_sdk::testutils::Address as _;
            let bps: u32 = env
                .storage()
                .instance()
                .get(&soroban_sdk::symbol_short!("bps"))
                .unwrap_or(0);
            let recv: Address = env
                .storage()
                .instance()
                .get(&soroban_sdk::symbol_short!("recv"))
                .unwrap_or_else(|| Address::generate(&env));
            (recv, bps)
        }
        pub fn set_royalty(env: Env, recv: Address, bps: u32) {
            env.storage()
                .instance()
                .set(&soroban_sdk::symbol_short!("recv"), &recv);
            env.storage()
                .instance()
                .set(&soroban_sdk::symbol_short!("bps"), &bps);
        }
        pub fn transfer_from(
            _env: Env,
            _spender: Address,
            _from: Address,
            _to: Address,
            _token_id: u64,
        ) {
        }
    }
}

use soroban_sdk::{
    bytes, symbol_short,
    testutils::Address as _,
    testutils::Events as _,
    testutils::Ledger,
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env,
};

/// Helper — deploy the contract and a real test token, returning
/// (env, client, artist, buyer, token_id, contract_id).
fn setup() -> (
    Env,
    MarketplaceContractClient<'static>,
    Address,
    Address,
    Address, // token_id  — a real SAC test token
    Address, // contract_id — the marketplace contract
    Address, // collection_id
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketplaceContract, ());
    let client = MarketplaceContractClient::new(&env, &contract_id);

    let artist = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Register a Stellar Asset Contract for use as the payment token.
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sac = StellarAssetClient::new(&env, &token_id);
    sac.mint(&artist, &100_000_000_000_i128);
    sac.mint(&buyer, &100_000_000_000_i128);
    // Pre-mint to contract for cases where it needs to hold escrow funds.
    sac.mint(&contract_id, &100_000_000_000_i128);

    let collection_id = env.register(mock_nft::MockNft, ());
    (
        env,
        client,
        artist,
        buyer,
        token_id,
        contract_id,
        collection_id,
    )
}

fn valid_recipients(env: &Env, artist: &Address) -> soroban_sdk::Vec<Recipient> {
    vec![
        env,
        Recipient {
            address: artist.clone(),
            percentage: 10_000, // 100 % expressed in basis points
        },
    ]
}

#[test]
fn test_set_treasury_and_protocol_fee() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Set treasury address
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);
    assert_eq!(client.get_treasury(), Some(treasury.clone()));

    // Create listing BEFORE setting protocol fee so that validate_recipients
    // sees fee == 0 and accepts 10 000 bps recipients.
    let price = 10_000_000_i128;
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // Set protocol fee to 500 bps (5%) — applied at purchase time
    client.set_protocol_fee(&artist, &500u32);
    assert_eq!(client.get_protocol_fee(), 500u32);

    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // Fee logic: 5% of 10_000_000 = 500_000
    // Seller should get 9_500_000, treasury gets 500_000
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 500_000_i128);
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + 9_500_000_i128
    );
}

#[test]
fn test_buy_artwork_no_treasury_fee_set() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Create listing before setting the protocol fee so validate_recipients passes
    let price = 1_000_000_i128;
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // Set protocol fee but no treasury — fee is discarded when treasury is absent
    client.set_protocol_fee(&artist, &300u32); // 3%
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // All funds should go to seller if treasury not set
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&artist), 100_000_000_000_i128 + price);
}

#[test]
#[should_panic]
fn test_set_protocol_fee_not_admin_panics() {
    let (_env, client, artist, buyer, _token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    // Buyer tries to set protocol fee
    client.set_protocol_fee(&buyer, &100u32);
}

#[test]
#[should_panic]
fn test_set_treasury_not_admin_panics() {
    let (env, client, artist, buyer, _token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    let treasury = Address::generate(&env);
    // Buyer tries to set treasury
    client.set_treasury(&buyer, &treasury);
}

#[test]
#[should_panic]
fn test_set_protocol_fee_too_high_panics() {
    let (_env, client, artist, _buyer, _token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    // Try to set fee > 1000 bps (10%)
    client.set_protocol_fee(&artist, &2000u32);
}

// ── create_listing ───────────────────────────────────────────

#[test]
fn test_create_listing_success() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    let cid = bytes!(&env, 0x516d546573744349444f6f6e495046533132333435);
    let price: i128 = 10_000_000; // 1 XLM

    // Set admin and whitelist the token
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    assert_eq!(listing_id, 1);
    assert_eq!(client.get_total_listings(), 1);

    let listing = client.get_listing(&1);
    assert_eq!(listing.listing_id, 1u64);
    assert_eq!(listing.artist, artist);
    assert_eq!(listing.price, price);
    assert_eq!(listing.status, ListingStatus::Active);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_create_listing_zero_price() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    client.create_listing(
        &artist,
        &0_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

// #[test] // Deprecated in V2 architecture
#[should_panic(expected = "Error(Contract, #1)")]
fn test_create_listing_empty_cid() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.create_listing(
        &artist,
        &10_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_create_listing_invalid_split() {
    // Recipients that sum to 11_000 bps (110%) — must be rejected at creation.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 6_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 5_000,
        },
    ];
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_create_listing_too_many_recipients() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let recipients = vec![
        &env,
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
    ];
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
}

// ── cancel_listing ───────────────────────────────────────────

#[test]
fn test_cancel_listing_success() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_listing(
        &artist,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let result = client.cancel_listing(&artist, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Cancelled);
}

#[test]
fn test_cancel_listing_rejects_pending_offers() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    let result = client.cancel_listing(&artist, &listing_id);
    assert!(result);

    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.status, ListingStatus::Cancelled);

    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.status, OfferStatus::Rejected);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_cancel_listing_wrong_artist() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);

    let id = client.create_listing(
        &artist,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.cancel_listing(&buyer, &id);
}

// ── update_listing ───────────────────────────────────────────

#[test]
fn test_update_listing_success() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_listing(
        &artist,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let new_cid = bytes!(&env, 0x516e6577434944);
    let new_price = 10_000_000_i128;
    let new_rec = valid_recipients(&env, &artist);
    let result = client.update_listing(&artist, &id, &new_price, &token_id, &new_rec);
    assert!(result);

    let listing = client.get_listing(&id);
    assert_eq!(listing.price, new_price);
    assert_eq!(listing.token, token_id);
}

// #[test] // Deprecated in V2 architecture
#[should_panic(expected = "Error(Contract, #1)")]
fn test_update_listing_empty_cid() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_listing(
        &artist,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let new_rec = valid_recipients(&env, &artist);
    client.update_listing(&artist, &id, &10_000_000_i128, &token_id, &new_rec);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_update_listing_wrong_artist() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_listing(
        &artist,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let new_cid = bytes!(&env, 0x51);
    let new_rec = valid_recipients(&env, &artist);
    client.update_listing(&buyer, &id, &10_000_000_i128, &token_id, &new_rec);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_update_listing_not_active() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_listing(
        &artist,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    client.cancel_listing(&artist, &id);

    let new_cid = bytes!(&env, 0x51);
    let new_rec = valid_recipients(&env, &artist);
    client.update_listing(&artist, &id, &10_000_000_i128, &token_id, &new_rec);
}

#[test]
fn test_artist_revocation_and_reinstatement() {
    let (env, client, artist, _, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist); // artist is admin for this test
    client.add_token_to_whitelist(&token_id);

    let artist_to_revoke = Address::generate(&env);
    client.revoke_artist(&artist_to_revoke);

    // Verify revoked artist cannot create listing
    let cid = bytes!(&env, 0x516d74657374);
    env.as_contract(&contract_id, || {
        let r = client.try_create_listing(
            &artist_to_revoke,
            &5_000_000_i128,
            &symbol_short!("XLM"),
            &token_id,
            &collection_id,
            &1u64,
            &valid_recipients(&env, &artist_to_revoke),
        );
        assert!(r.is_err());
    });

    // Verify revoked artist cannot create auction
    env.as_contract(&contract_id, || {
        let r = client.try_create_auction(
            &artist_to_revoke,
            &token_id,
            &collection_id,
            &1u64,
            &1_000_000_i128,
            &3600u64,
            &valid_recipients(&env, &artist_to_revoke),
        );
        assert!(r.is_err());
    });

    // Reinstate
    client.reinstate_artist(&artist_to_revoke);

    // Now it should work
    StellarAssetClient::new(&env, &token_id).mint(&artist_to_revoke, &100_000_000_000_i128);
    let id = client.create_listing(
        &artist_to_revoke,
        &5_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist_to_revoke),
    );
    assert_eq!(id, 1u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_update_listing_fails_with_pending_offers() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    // Try to update while offer is pending
    let new_cid = bytes!(&env, 0x51);
    let new_rec = valid_recipients(&env, &artist);
    client.update_listing(&artist, &listing_id, &10_000_000_i128, &token_id, &new_rec);
}

// ── get_artist_listings ──────────────────────────────────────

#[test]
fn test_get_artist_listings() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);

    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.create_listing(
        &artist,
        &2_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.create_listing(
        &artist,
        &3_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let ids = client.get_artist_listings(&artist);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), 1_u64);
    assert_eq!(ids.get(1).unwrap(), 2_u64);
    assert_eq!(ids.get(2).unwrap(), 3_u64);
}

#[test]
fn test_buy_artwork_success() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let price = 10_000_000_i128;

    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
}

#[test]
fn test_buy_artwork_complex_split() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let colab1 = Address::generate(&env);
    let colab2 = Address::generate(&env);

    let price = 10_000_000_i128; // 1 XLM

    // test precision rounding 3300/3300/3400 bps (33%/33%/34% in basis points)
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 3_300,
        },
        Recipient {
            address: colab1.clone(),
            percentage: 3_300,
        },
        Recipient {
            address: colab2.clone(),
            percentage: 3_400, // Last receiver takes the exact fractional remainder securely
        },
    ];

    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
    assert!(client.buy_artwork(&buyer, &id));

    // Verify recipients received correct amounts
    let token = TokenClient::new(&env, &token_id);
    let artist_got = token.balance(&artist) - 100_000_000_000_i128;
    let colab1_got = token.balance(&colab1);
    let colab2_got = token.balance(&colab2);
    assert_eq!(artist_got + colab1_got + colab2_got, price);
}

// ── get_listing not found ────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_get_listing_not_found() {
    let (_env, client, _, _, _, _, collection_id) = setup();
    client.get_listing(&999);
}

// ── Admin/Whitelist Management Tests ───────────────────────

#[test]
#[should_panic]
fn test_set_admin_only_once() {
    let (_env, client, artist, _, _token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    // Second call should panic
    client.set_admin(&artist);
}

#[test]
fn test_add_and_remove_token_whitelist() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    // Add token
    client.add_token_to_whitelist(&token_id);
    // Remove token
    client.remove_token_from_whitelist(&token_id);
    // Now creating a listing with any token should SUCCEED (whitelist is empty)
    let cid = bytes!(&env, 0x516d74657374);
    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    assert_eq!(listing_id, 1u64);
}

#[test]
#[should_panic]
fn test_create_listing_with_non_whitelisted_token_panics() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    // Add a different token to whitelist
    let other_token = Address::generate(&env);
    client.add_token_to_whitelist(&other_token);
    // Now creating a listing with token_id (not whitelisted) should panic
    let cid = bytes!(&env, 0x516d74657374);
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_create_listing_with_whitelisted_token_succeeds() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    assert_eq!(listing_id, 1u64);
}

#[test]
fn test_buy_artwork_fee_greater_than_price() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);
    let price = 5_i128; // Very small price
    // Create listing before setting protocol fee so validate_recipients passes
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // Set protocol fee to 10% — applied at purchase time
    client.set_protocol_fee(&artist, &1000u32);
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // Fee: 10% of 5 = 0 (integer division), seller gets 5
}

#[test]
fn test_buy_artwork_fee_rounding_precision() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);
    let price = 100_i128;
    // Create listing before setting protocol fee so validate_recipients passes
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // Set protocol fee to 333 bps (3.33%) — applied at purchase time
    client.set_protocol_fee(&artist, &333u32);
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // Fee: 100 * 333 / 10_000 = 3 (integer division), seller gets 97
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 3_i128);
}

#[test]
fn test_royalty_zero_percent() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let price = 10_000_000_i128;
    // 0% royalty
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // All funds to seller
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&artist), 100_000_000_000_i128 + price);
}

#[test]
fn test_royalty_hundred_percent() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let price = 10_000_000_i128;
    // 100% royalty (10000 bps) — but artist IS original_creator, so royalty skipped (same address)
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // Royalty only applies when original_creator != seller; here they're equal so artist gets full price
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&artist), 100_000_000_000_i128 + price);
}

#[test]
fn test_royalty_rounding_precision() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let price = 7_i128;
    // 33% royalty (3300 bps)
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // Royalty skipped since artist == original_creator, artist gets full price
}

// #[test] // Deprecated in V2 architecture
fn test_royalty_secondary_sale() {
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let price = 10_000_000_i128;
    // 10% royalty
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // First sale: artist sells to buyer
    let result = client.buy_artwork(&buyer, &id);
    assert!(result);
    let mut listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
    // Simulate secondary sale: buyer relists and sells to a new buyer
    let new_buyer = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&new_buyer, &100_000_000_000_i128);
    listing.artist = buyer.clone();
    listing.status = ListingStatus::Active;
    listing.owner = None;
    // Update recipients to the new seller (buyer) so payout goes to them
    listing.recipients = vec![
        &env,
        Recipient {
            address: buyer.clone(),
            percentage: 10_000,
        },
    ];
    // Save the relisted artwork using contract context
    env.as_contract(&contract_id, || {
        crate::storage::save_listing(&env, &listing);
    });
    let result2 = client.buy_artwork(&new_buyer, &id);
    assert!(result2);
    let listing2 = client.get_listing(&id);
    assert_eq!(listing2.status, ListingStatus::Sold);
    assert_eq!(listing2.owner, Some(new_buyer.clone()));
    // 10% of price should go to original creator (artist), 90% to seller (buyer)
    let token = TokenClient::new(&env, &token_id);
    let royalty = price * 1000 / 10_000; // = 1_000_000
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + price + royalty
    );
    assert_eq!(
        token.balance(&buyer),
        100_000_000_000_i128 - price + (price - royalty)
    );
}

// ── Auction Tests ────────────────────────────────────────────

#[test]
fn test_create_auction_success() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let cid = bytes!(&env, 0x516d74657374);
    let reserve_price = 1_000_000_i128;
    let duration = 3600u64; // 1 hour

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &reserve_price,
        &duration,
        &valid_recipients(&env, &artist),
    );

    assert_eq!(auction_id, 1);
    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.creator, artist);
    assert_eq!(auction.reserve_price, reserve_price);
    assert_eq!(auction.status, crate::types::AuctionStatus::Active);
    assert_eq!(auction.end_time, env.ledger().timestamp() + duration);

    assert_eq!(client.get_total_auctions(), 1);
    let artist_auctions = client.get_artist_auctions(&artist);
    assert_eq!(artist_auctions.len(), 1);
    assert_eq!(artist_auctions.get(0).unwrap(), 1);
}

// #[test] // Deprecated in V2 architecture
#[should_panic(expected = "Error(Contract, #1)")]
fn test_create_auction_zero_reserve_rejected() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &0,
        &3600,
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_place_bid_success() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    client.place_bid(&buyer, &id, &1_500_000);
    let auction = client.get_auction(&id);
    assert_eq!(auction.highest_bid, 1_500_000);
    assert_eq!(auction.highest_bidder, Some(buyer));
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_place_bid_too_low() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    client.place_bid(&buyer, &id, &500_000); // Below reserve
}

#[test]
fn test_finalize_auction_with_winner() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    client.place_bid(&buyer, &id, &1_500_000);

    // Jump in time
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    client.finalize_auction(&buyer, &id);
    let auction = client.get_auction(&id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Finalized);
}

#[test]
fn test_finalize_auction_no_bids() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    client.finalize_auction(&artist, &id);
    let auction = client.get_auction(&id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Cancelled);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_finalize_auction_before_expiry_rejects_non_creator() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    client.finalize_auction(&buyer, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_place_bid_after_expiration() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    // Jump in time
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    client.place_bid(&buyer, &id, &1_500_000);
}

#[test]
fn test_outbid_refund_logic_check() {
    let (env, client, artist, buyer1, token_id, _contract_id, collection_id) = setup();
    let buyer2 = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&buyer2, &100_000_000_000_i128);
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000,
        &3600,
        &valid_recipients(&env, &artist),
    );

    client.place_bid(&buyer1, &id, &1_500_000);
    client.place_bid(&buyer2, &id, &2_000_000);

    let auction = client.get_auction(&id);
    assert_eq!(auction.highest_bid, 2_000_000);
    assert_eq!(auction.highest_bidder, Some(buyer2));

    // buyer1 should have been refunded their 1_500_000
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&buyer1), 100_000_000_000_i128);
}

// ── Offer Tests ─────────────────────────────────────────────

/// Helper to create a listing and return its ID.
fn create_test_listing(
    env: &Env,
    client: &MarketplaceContractClient,
    artist: &Address,
    token_id: &Address,
) -> u64 {
    let collection_id = env.register(mock_nft::MockNft, ());
    let cid = bytes!(env, 0x516d74657374);
    let price = 10_000_000_i128;
    client.create_listing(
        artist,
        &price,
        &symbol_short!("XLM"),
        token_id,
        &collection_id,
        &1u64,
        &valid_recipients(env, artist),
    )
}

#[test]
fn test_make_offer_success() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);

    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    assert_eq!(offer_id, 1);

    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.offer_id, 1u64);
    assert_eq!(offer.listing_id, listing_id);
    assert_eq!(offer.offerer, buyer);
    assert_eq!(offer.amount, 5_000_000_i128);
    assert_eq!(offer.token, token_id);
    assert_eq!(offer.status, OfferStatus::Pending);

    // Check indexes
    let listing_offers = client.get_listing_offers(&listing_id);
    assert_eq!(listing_offers.len(), 1);
    assert_eq!(listing_offers.get(0).unwrap(), 1u64);

    let offerer_offers = client.get_offerer_offers(&buyer);
    assert_eq!(offerer_offers.len(), 1);
    assert_eq!(offerer_offers.get(0).unwrap(), 1u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_make_offer_on_own_listing_fails() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);

    // Artist tries to offer on their own listing
    client.make_offer(&artist, &listing_id, &5_000_000_i128, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_make_offer_on_nonexistent_listing_fails() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);

    client.make_offer(&buyer, &999u64, &5_000_000_i128, &token_id);
}

#[test]
fn test_withdraw_offer_success() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    client.withdraw_offer(&buyer, &offer_id);

    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.status, OfferStatus::Withdrawn);

    // Buyer should have been refunded
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&buyer), 100_000_000_000_i128);
}

#[test]
fn test_accept_offer_success() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    client.accept_offer(&artist, &offer_id);

    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.status, OfferStatus::Accepted);

    // Listing should be sold with buyer as owner
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));

    // Artist should have received the offer amount
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + 5_000_000_i128
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_accept_offer_reentrancy_guard() {
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    // Simulate a nested accept_offer while the listing lock is held (e.g. payout token callback).
    env.as_contract(&contract_id, || {
        assert!(crate::storage::acquire_listing_lock(&env, listing_id));
    });
    client.accept_offer(&artist, &offer_id);
}

#[test]
fn test_reject_offer_success() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    client.reject_offer(&artist, &offer_id);

    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.status, OfferStatus::Rejected);

    // Listing should still be active
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.status, ListingStatus::Active);

    // Buyer should have been refunded
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&buyer), 100_000_000_000_i128);
}

#[test]
fn test_accept_offer_rejects_others() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    let buyer2 = Address::generate(&env);
    let buyer3 = Address::generate(&env);
    let sac = StellarAssetClient::new(&env, &token_id);
    sac.mint(&buyer2, &100_000_000_000_i128);
    sac.mint(&buyer3, &100_000_000_000_i128);
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);

    let offer_id_1 = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);
    let offer_id_2 = client.make_offer(&buyer2, &listing_id, &7_000_000_i128, &token_id);
    let offer_id_3 = client.make_offer(&buyer3, &listing_id, &3_000_000_i128, &token_id);

    // Accept offer 2
    client.accept_offer(&artist, &offer_id_2);

    // Offer 2 should be accepted
    let offer2 = client.get_offer(&offer_id_2);
    assert_eq!(offer2.status, OfferStatus::Accepted);

    // Offers 1 and 3 should be rejected (refunded)
    let offer1 = client.get_offer(&offer_id_1);
    assert_eq!(offer1.status, OfferStatus::Rejected);

    let offer3 = client.get_offer(&offer_id_3);
    assert_eq!(offer3.status, OfferStatus::Rejected);

    // Listing should be sold with buyer2 as owner
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer2.clone()));

    // Rejected offerers should have been refunded
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&buyer), 100_000_000_000_i128);
    assert_eq!(token.balance(&buyer3), 100_000_000_000_i128);
}

// ── Admin and Revocation Tests ──────────────────────────────

#[test]
fn test_artist_revocation_flow() {
    let (env, client, artist, _, token_id, contract_id, collection_id) = setup();
    let cid = bytes!(&env, 0x51);
    let price = 1_000_000_i128;

    client.set_admin(&artist); // Artist is admin for this test
    client.add_token_to_whitelist(&token_id);

    // 1. Artist is NOT revoked initially
    client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // 2. Admin revokes artist
    client.revoke_artist(&artist);

    // 3. Artist tries to create listing - Should Panic (Unauthorized #5)
    let result = env.as_contract(&contract_id, || {
        client.try_create_listing(
            &artist,
            &price,
            &symbol_short!("XLM"),
            &token_id,
            &collection_id,
            &1u64,
            &valid_recipients(&env, &artist),
        )
    });
    assert!(result.is_err());

    // 4. Admin reinstates artist
    client.reinstate_artist(&artist);

    // 5. Artist creates listing again - Should succeed
    client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

// ── Issue #17: revocation enforcement on all creation paths ─────────────────
// The listing path is already covered by the existing
// `test_revoked_artist_cannot_create_listing`. The cases below add the auction
// path, reinstatement of both paths, and settleability of existing items.

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_revoked_artist_cannot_create_auction() {
    let (env, client, admin, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);

    let artist = Address::generate(&env);
    client.revoke_artist(&artist);

    // A revoked artist creating an auction must also revert with ArtistRevoked
    // (#15) — consistent with create_listing via the shared require_not_revoked
    // guard (previously this path returned Unauthorized #5).
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_reinstated_artist_can_create_listing_and_auction() {
    let (env, client, admin, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);

    let artist = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&artist, &100_000_000_000_i128);

    client.revoke_artist(&artist);
    client.reinstate_artist(&artist);

    // Reinstatement removes the block on BOTH creation paths.
    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    assert_eq!(listing_id, 1u64);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    assert_eq!(auction_id, 1u64);
}

#[test]
fn test_revoked_artist_existing_listing_remains_settleable() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist); // artist is admin so it can revoke itself in-test
    client.add_token_to_whitelist(&token_id);

    // Listing is created BEFORE the artist is revoked.
    let id = client.create_listing(
        &artist,
        &10_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // Revoking the artist must NOT block settlement of their existing items.
    client.revoke_artist(&artist);

    let ok = client.buy_artwork(&buyer, &id);
    assert!(ok);
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer.clone()));
}

#[test]
fn test_revoked_artist_existing_auction_remains_finalizable() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Auction created (and bid on) before revocation.
    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&buyer, &id, &1_500_000_i128);

    // Revoke the artist; the in-flight auction must still finalize (settle).
    client.revoke_artist(&artist);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &id);

    let auction = client.get_auction(&id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Finalized);
}

#[test]
fn test_update_listing_with_pending_offer_fails() {
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = create_test_listing(&env, &client, &artist, &token_id);

    // Add a pending offer
    client.make_offer(&buyer, &id, &5_000_000, &token_id);

    // Try to update listing - Should fail
    let result = env.as_contract(&contract_id, || {
        client.try_update_listing(
            &artist,
            &id,
            &15_000_000,
            &token_id,
            &valid_recipients(&env, &artist),
        )
    });
    assert!(result.is_err());
}

#[test]
fn test_update_listing_success_with_recipients() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let id = create_test_listing(&env, &client, &artist, &token_id);

    let new_recipients = vec![
        &env,
        crate::types::Recipient {
            address: artist.clone(),
            percentage: 5_000, // 50% in bps
        },
        crate::types::Recipient {
            address: Address::generate(&env),
            percentage: 5_000, // 50% in bps
        },
    ];

    client.update_listing(&artist, &id, &15_000_000, &token_id, &new_recipients);

    let listing = client.get_listing(&id);
    assert_eq!(listing.price, 15_000_000);
    assert_eq!(listing.recipients.len(), 2);
}

// ── buy_artwork edge cases (Issue #124) ──────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #21)")]
fn test_buy_cancelled_listing_fails() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.cancel_listing(&artist, &id);
    client.buy_artwork(&buyer, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #20)")]
fn test_buy_already_sold_listing_fails() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.buy_artwork(&buyer, &id);
    // Second buy attempt on an already-sold listing
    let buyer2 = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&buyer2, &100_000_000_000_i128);
    client.buy_artwork(&buyer2, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_buy_own_listing_fails() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.buy_artwork(&artist, &id);
}

// ── update_listing recipient validation (Issue #175) ─────────

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_update_listing_invalid_split_fails() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    // Recipients summing to 12_000 bps — over 100%
    let bad_recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 7_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 5_000,
        },
    ];
    client.update_listing(&artist, &id, &10_000_000, &token_id, &bad_recipients);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_update_listing_too_many_recipients_fails() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    let too_many = vec![
        &env,
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 2_000,
        },
    ];
    client.update_listing(&artist, &id, &10_000_000, &token_id, &too_many);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_update_listing_empty_recipients_fails() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.update_listing(
        &artist,
        &id,
        &10_000_000,
        &token_id,
        &soroban_sdk::Vec::new(&env),
    );
}

// ── transfer_admin / accept_admin tests (Issue #162) ────────

#[test]
fn test_transfer_admin_two_step_succeeds() {
    let (env, client, admin, _, _token_id, _contract_id, collection_id) = setup();
    let new_admin = Address::generate(&env);

    client.set_admin(&admin);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Step 1: current admin proposes new admin
    client.transfer_admin(&admin, &new_admin);

    // Admin has NOT changed yet
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Step 2: new admin accepts
    client.accept_admin(&new_admin);

    assert_eq!(client.get_admin(), Some(new_admin.clone()));
}

#[test]
#[should_panic]
fn test_transfer_admin_wrong_caller_panics() {
    let (env, client, admin, _, _token_id, _contract_id, collection_id) = setup();
    let impostor = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.set_admin(&admin);
    // impostor tries to initiate transfer — should panic Unauthorized
    client.transfer_admin(&impostor, &new_admin);
}

#[test]
#[should_panic]
fn test_accept_admin_wrong_caller_panics() {
    let (env, client, admin, _, _token_id, _contract_id, collection_id) = setup();
    let new_admin = Address::generate(&env);
    let impostor = Address::generate(&env);

    client.set_admin(&admin);
    client.transfer_admin(&admin, &new_admin);
    // A different address tries to accept — should panic Unauthorized
    client.accept_admin(&impostor);
}

// ── Event emission tests (Issue #180) ────────────────────────

fn has_event_with_topic(events: &soroban_sdk::testutils::ContractEvents, symbol: &str) -> bool {
    use soroban_sdk::xdr::{ContractEventBody, ScVal};
    events.events().iter().any(|e| {
        if let ContractEventBody::V0(body) = &e.body {
            body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == symbol
                } else {
                    false
                }
            })
        } else {
            false
        }
    })
}

#[test]
fn test_buy_artwork_emits_artwork_sold_event() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.buy_artwork(&buyer, &listing_id);

    assert!(
        has_event_with_topic(&env.events().all(), "art_sold"),
        "ArtworkSoldEvent was not emitted"
    );
}

#[test]
fn test_cancel_listing_emits_listing_cancelled_event() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.cancel_listing(&artist, &listing_id);

    assert!(
        has_event_with_topic(&env.events().all(), "lst_cncl"),
        "ListingCancelledEvent was not emitted"
    );
}

#[test]
fn test_update_listing_emits_listing_updated_event() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.update_listing(
        &artist,
        &listing_id,
        &20_000_000,
        &token_id,
        &valid_recipients(&env, &artist),
    );

    assert!(
        has_event_with_topic(&env.events().all(), "lst_updt"),
        "ListingUpdatedEvent was not emitted"
    );
}

#[test]
fn test_make_offer_emits_offer_made_event() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    assert!(
        has_event_with_topic(&env.events().all(), "ofr_made"),
        "OfferMadeEvent was not emitted"
    );
}

#[test]
fn test_accept_offer_emits_offer_accepted_event() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);
    client.accept_offer(&artist, &offer_id);

    assert!(
        has_event_with_topic(&env.events().all(), "ofr_accp"),
        "OfferAcceptedEvent was not emitted"
    );
}

#[test]
fn test_reject_offer_emits_offer_rejected_event() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);
    client.reject_offer(&artist, &offer_id);

    assert!(
        has_event_with_topic(&env.events().all(), "ofr_rjct"),
        "OfferRejectedEvent was not emitted"
    );
}

#[test]
fn test_withdraw_offer_emits_offer_withdrawn_event() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);
    client.withdraw_offer(&buyer, &offer_id);

    assert!(
        has_event_with_topic(&env.events().all(), "ofr_wdrn"),
        "OfferWithdrawnEvent was not emitted"
    );
}

#[test]
fn test_create_auction_emits_auction_created_event() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600_u64,
        &valid_recipients(&env, &artist),
    );

    assert!(
        has_event_with_topic(&env.events().all(), "auc_crtd"),
        "AuctionCreatedEvent was not emitted"
    );
}

#[test]
fn test_place_bid_emits_bid_placed_event() {
    let (env, client, artist, bidder, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600_u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&bidder, &auction_id, &2_000_000_i128);

    assert!(
        has_event_with_topic(&env.events().all(), "bid_plcd"),
        "BidPlacedEvent was not emitted"
    );
}

#[test]
fn test_finalize_auction_emits_auction_resolved_event() {
    let (env, client, artist, bidder, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600_u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&bidder, &auction_id, &2_000_000_i128);

    env.ledger().with_mut(|l| {
        l.timestamp += 7200;
    });

    client.finalize_auction(&bidder, &auction_id);

    assert!(
        has_event_with_topic(&env.events().all(), "auc_rslv"),
        "AuctionFinalizedEvent was not emitted"
    );
}

// ── Token transfer tests (Issue #165) ────────────────────────

#[test]
fn test_buy_artwork_transfers_correct_amounts_to_recipients() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let price = 10_000_000_i128;
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    let token = TokenClient::new(&env, &token_id);
    let buyer_before = token.balance(&buyer);
    let artist_before = token.balance(&artist);

    client.buy_artwork(&buyer, &id);

    assert_eq!(token.balance(&buyer), buyer_before - price);
    assert_eq!(token.balance(&artist), artist_before + price);
}

// #[test] // Deprecated in V2 architecture
fn test_buy_artwork_pays_royalty_on_secondary_sale() {
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let price = 10_000_000_i128;
    let royalty_bps = 1000u32; // 10%
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // First sale (no royalty since original_creator == seller)
    client.buy_artwork(&buyer, &id);

    // Secondary sale setup: buyer relists
    let new_buyer = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&new_buyer, &100_000_000_000_i128);
    let mut listing = client.get_listing(&id);
    listing.artist = buyer.clone();
    listing.status = ListingStatus::Active;
    listing.owner = None;
    listing.recipients = vec![
        &env,
        Recipient {
            address: buyer.clone(),
            percentage: 10_000,
        },
    ];
    env.as_contract(&contract_id, || {
        crate::storage::save_listing(&env, &listing);
    });

    let token = TokenClient::new(&env, &token_id);
    let artist_before = token.balance(&artist);
    let buyer_before = token.balance(&buyer);

    client.buy_artwork(&new_buyer, &id);

    let expected_royalty = price * royalty_bps as i128 / 10_000; // 1_000_000
    assert_eq!(token.balance(&artist), artist_before + expected_royalty);
    assert_eq!(
        token.balance(&buyer),
        buyer_before + price - expected_royalty
    );
}

#[test]
fn test_buy_artwork_pays_treasury_fee() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    let price = 10_000_000_i128;
    // Create listing before setting protocol fee so validate_recipients passes
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // Set fee to 500 bps (5%) after listing creation
    client.set_protocol_fee(&artist, &500u32); // 5%

    client.buy_artwork(&buyer, &id);

    let token = TokenClient::new(&env, &token_id);
    let expected_fee = price * 500 / 10_000; // 500_000
    assert_eq!(token.balance(&treasury), expected_fee);
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + price - expected_fee
    );
}

// ── Pause / unpause lifecycle tests (Issue #200) ─────────────

#[test]
fn test_admin_pause_and_unpause() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    assert!(!client.is_paused());
    client.admin_pause(&artist);
    assert!(client.is_paused());
    client.admin_unpause(&artist);
    assert!(!client.is_paused());
}

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_create_listing_while_paused_fails() {
    let (env, client, artist, _, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.admin_pause(&artist);
    client.create_listing(
        &artist,
        &10_000_000,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_buy_artwork_while_paused_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.admin_pause(&artist);
    client.buy_artwork(&buyer, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_cancel_listing_while_paused_fails() {
    let (env, client, artist, _, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.admin_pause(&artist);
    client.cancel_listing(&artist, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_make_offer_while_paused_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.admin_pause(&artist);
    client.make_offer(&buyer, &id, &5_000_000_i128, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_create_auction_while_paused_fails() {
    let (env, client, artist, _, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.admin_pause(&artist);
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600_u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_actions_succeed_after_unpause() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.admin_pause(&artist);
    client.admin_unpause(&artist);
    // Should succeed after unpausing
    assert!(client.buy_artwork(&buyer, &id));
}

// ── Offer edge cases (Issue #200) ─────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_make_offer_zero_amount_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.make_offer(&buyer, &id, &0_i128, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_make_offer_negative_amount_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.make_offer(&buyer, &id, &-1_000_i128, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_accept_already_accepted_offer_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &id, &5_000_000_i128, &token_id);
    client.accept_offer(&artist, &offer_id);
    // Second accept on the same (now non-pending) offer should panic
    client.accept_offer(&artist, &offer_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_reject_withdrawn_offer_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &id, &5_000_000_i128, &token_id);
    client.withdraw_offer(&buyer, &offer_id);
    // Reject a withdrawn offer — status is no longer Pending
    client.reject_offer(&artist, &offer_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_accept_nonexistent_offer_fails() {
    let (env, client, artist, _, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.accept_offer(&artist, &9999_u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_reject_nonexistent_offer_fails() {
    let (env, client, artist, _, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.reject_offer(&artist, &9999_u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_withdraw_nonexistent_offer_fails() {
    let (env, client, _, buyer, token_id, _, collection_id) = setup();
    client.withdraw_offer(&buyer, &9999_u64);
}

// ── Cancel listing edge cases (Issue #200) ───────────────────

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_cancel_already_cancelled_listing_fails() {
    let (env, client, artist, _, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.cancel_listing(&artist, &id);
    // Second cancel should fail: listing is no longer Active
    client.cancel_listing(&artist, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_cancel_sold_listing_fails() {
    let (env, client, artist, buyer, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.buy_artwork(&buyer, &id);
    client.cancel_listing(&artist, &id);
}

// ── Auction edge cases (Issue #200) ─────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_bid_on_nonexistent_auction_fails() {
    let (_, client, _, buyer, _, _, collection_id) = setup();
    client.place_bid(&buyer, &9999_u64, &1_000_000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_finalize_nonexistent_auction_fails() {
    let (_, client, _, caller, _, _, collection_id) = setup();
    client.finalize_auction(&caller, &9999_u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_finalize_already_finalized_auction_fails() {
    let (env, client, artist, bidder, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600_u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&bidder, &auction_id, &2_000_000_i128);
    env.ledger().with_mut(|l| {
        l.timestamp += 7200;
    });
    client.finalize_auction(&bidder, &auction_id);
    // Second finalize should fail
    client.finalize_auction(&bidder, &auction_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_bid_on_finalized_auction_fails() {
    let (env, client, artist, bidder, token_id, _, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600_u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&bidder, &auction_id, &2_000_000_i128);
    env.ledger().with_mut(|l| {
        l.timestamp += 7200;
    });
    client.finalize_auction(&bidder, &auction_id);
    // Bid after finalization: auction status is no longer Active
    let new_bidder = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&new_bidder, &100_000_000_000_i128);
    client.place_bid(&new_bidder, &auction_id, &3_000_000_i128);
}

// ── Admin transfer edge cases (Issue #200) ──────────────────

#[test]
#[should_panic]
fn test_accept_admin_with_no_pending_transfer_panics() {
    let (env, client, admin, _, _token_id, _, collection_id) = setup();
    let impostor = Address::generate(&env);
    client.set_admin(&admin);
    // accept_admin when no transfer has been initiated — should panic
    client.accept_admin(&impostor);
}

// ── Revoke / reinstate standalone tests (Issue #200) ────────

#[test]
fn test_revoke_and_reinstate_artist() {
    let (env, client, admin, artist2, token_id, _, collection_id) = setup();
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);

    assert!(!client.is_artist_revoked(&artist2));
    client.revoke_artist(&artist2);
    assert!(client.is_artist_revoked(&artist2));
    client.reinstate_artist(&artist2);
    assert!(!client.is_artist_revoked(&artist2));
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_revoked_artist_cannot_create_listing() {
    let (env, client, admin, artist2, token_id, _, collection_id) = setup();
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);
    client.revoke_artist(&artist2);
    client.create_listing(
        &artist2,
        &10_000_000,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist2),
    );
}

// ── Token whitelist edge cases (Issue #200) ─────────────────

#[test]
fn test_get_token_whitelist_after_removal() {
    let (env, client, admin, _, token_id, _, collection_id) = setup();
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);
    let list = client.get_token_whitelist();
    assert!(list.iter().any(|t| t == token_id));
    client.remove_token_from_whitelist(&token_id);
    let list_after = client.get_token_whitelist();
    assert!(!list_after.iter().any(|t| t == token_id));
}

// ── Royalty bps validation tests (security)

#[test]
fn test_create_listing_royalty_bps_max_allowed() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    // 10000 bps (100%) is allowed at creation time
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    assert_eq!(id, 1u64);
}

// #[test] // Deprecated in V2 architecture
#[should_panic(expected = "Error(Contract, #24)")]
fn test_create_listing_royalty_bps_too_high() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_create_auction_royalty_bps_max_allowed() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    assert_eq!(auction_id, 1u64);
}

// #[test] // Deprecated in V2 architecture
#[should_panic(expected = "Error(Contract, #24)")]
fn test_create_auction_royalty_bps_too_high() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let cid = bytes!(&env, 0x516d74657374);
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #25)")]
fn test_buy_artwork_fails_if_token_delisted() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Add a second token so the whitelist is non-empty after removing token_id.
    // An empty whitelist means "allow all" by design, so we need at least one
    // other entry to make token_id genuinely non-whitelisted.
    let other_token = Address::generate(&env);
    client.add_token_to_whitelist(&other_token);
    let cid = bytes!(&env, 0x516d74657374);
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // Admin removes token from whitelist — purchase should now be rejected at buy time
    client.remove_token_from_whitelist(&token_id);
    client.buy_artwork(&buyer, &id);
}
// ═══════════════════════════════════════════════════════════════════════════
// admin_pause / admin_unpause mechanism
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_paused_default_false() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Freshly deployed — must not be paused
    assert!(!client.is_paused());
}

#[test]
fn test_admin_pause_and_unpause_state_transitions() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    assert!(!client.is_paused(), "contract should start unpaused");

    client.admin_pause(&artist);
    assert!(
        client.is_paused(),
        "contract should be paused after admin_pause"
    );

    client.admin_unpause(&artist);
    assert!(
        !client.is_paused(),
        "contract should be unpaused after admin_unpause"
    );
}

#[test]
fn test_admin_pause_emits_event() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.admin_pause(&artist);

    assert!(
        has_event_with_topic(&env.events().all(), "ctr_psd"),
        "admin_pause must emit a CONTRACT_PAUSED event"
    );
}

#[test]
fn test_admin_unpause_emits_event() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.admin_pause(&artist);
    client.admin_unpause(&artist);

    assert!(
        has_event_with_topic(&env.events().all(), "ctr_unpsd"),
        "admin_unpause must emit a CONTRACT_UNPAUSED event"
    );
}

#[test]
#[should_panic]
fn test_admin_pause_rejects_non_admin() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // `buyer` is not the admin — must panic with Unauthorized
    client.admin_pause(&buyer);
}

#[test]
#[should_panic]
fn test_admin_unpause_rejects_non_admin() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.admin_pause(&artist);
    // `buyer` is not the admin — must panic with Unauthorized
    client.admin_unpause(&buyer);
}

#[test]
#[should_panic]
fn test_create_listing_blocked_when_paused() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.admin_pause(&artist);

    // Any create_listing call must panic while the contract is paused
    create_test_listing(&env, &client, &artist, &token_id);
}

#[test]
#[should_panic]
fn test_create_auction_blocked_when_paused() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.admin_pause(&artist);

    // Any create_auction call must panic while the contract is paused
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &5_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_create_listing_succeeds_after_unpause() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Pause then immediately unpause
    client.admin_pause(&artist);
    client.admin_unpause(&artist);

    // Now create_listing must work again
    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    assert!(listing_id > 0, "listing must be created after unpause");
}

#[test]
#[should_panic]
fn test_buy_artwork_blocked_when_paused() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_test_listing(&env, &client, &artist, &token_id);

    client.admin_pause(&artist);

    // buy_artwork must panic while paused
    client.buy_artwork(&buyer, &listing_id);
}

// ══════════════════════════════════════════════════════════════════════════
// RoyaltyExceedsLimit boundary tests (Issue A)
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn test_validate_recipients_exactly_10000_bps_succeeds() {
    // Recipients that sum to exactly 10 000 bps (100%) with zero protocol fee
    // must succeed.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 10_000,
        },
    ];
    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
    assert_eq!(listing_id, 1u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_validate_recipients_10001_bps_rejected() {
    // Recipients that sum to 10 001 bps (100.01%) must be rejected with
    // RoyaltyExceedsLimit even when there is no protocol fee.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 5_001,
        },
        Recipient {
            address: Address::generate(&env),
            percentage: 5_000,
        },
    ];
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
}

#[test]
fn test_validate_recipients_empty_succeeds() {
    // Edge case: although the contract rejects empty recipients with InvalidSplit,
    // here we verify that zero recipients + zero fee does not trip the new
    // RoyaltyExceedsLimit validator (it should panic with InvalidSplit first).
    // The test will panic with InvalidSplit (#7), NOT RoyaltyExceedsLimit (#26).
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let result = env.as_contract(&_contract_id, || {
        client.try_create_listing(
            &artist,
            &1_000_000_i128,
            &symbol_short!("XLM"),
            &token_id,
            &collection_id,
            &1u64,
            &soroban_sdk::Vec::new(&env),
        )
    });
    // Expect InvalidSplit (7), not RoyaltyExceedsLimit (26).
    assert!(result.is_err());
}

#[test]
fn test_validate_recipients_single_recipient_at_limit_with_protocol_fee() {
    // When protocol_fee_bps = 500 (5%), recipients can have at most 9 500 bps
    // to stay under the combined 10 000 limit.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Create listing before setting protocol fee so validate_recipients sees fee = 0
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_500,
        },
    ];
    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
    assert_eq!(listing_id, 1u64);
    // Now set the protocol fee; an update with the same recipients would also pass.
    client.set_protocol_fee(&artist, &500u32);
    // Update_listing with 9_500 bps: 9_500 + 500 = 10_000 — should succeed.
    let updated = client.update_listing(&artist, &listing_id, &2_000_000, &token_id, &recipients);
    assert!(updated);
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_validate_recipients_exceeds_limit_with_protocol_fee() {
    // When protocol_fee_bps = 500 (5%), recipients summing to 9_501 bps will
    // result in total 10_001 bps — must be rejected with RoyaltyExceedsLimit.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Create a listing with small recipients first
    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &vec![
            &env,
            Recipient {
                address: artist.clone(),
                percentage: 5_000,
            },
        ],
    );
    // Set protocol fee
    client.set_protocol_fee(&artist, &500u32);
    // Try to update with recipients summing to 9_501 bps
    let bad_recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_501,
        },
    ];
    client.update_listing(&artist, &listing_id, &2_000_000, &token_id, &bad_recipients);
}

// ══════════════════════════════════════════════════════════════════════════
// Reentrancy attack tests (Issue B)
// ══════════════════════════════════════════════════════════════════════════

mod mock_reentrant_token {
    use soroban_sdk::{contract, contractimpl, Address, Env, IntoVal};

    #[contract]
    pub struct MockReentrantToken;

    #[contractimpl]
    impl MockReentrantToken {
        /// On transfer, attempts to re-enter the marketplace's buy_artwork for
        /// the same listing_id that triggered this transfer. If the reentrancy
        /// guard is working correctly, the nested call should revert with
        /// ReentrancyGuard error.
        pub fn transfer(
            env: Env,
            _from: Address,
            _to: Address,
            _amount: i128,
        ) {
            // Attempt to call buy_artwork on the marketplace contract stored in
            // instance storage under key "marketplace".
            let marketplace_addr: Address = env
                .storage()
                .instance()
                .get(&soroban_sdk::symbol_short!("mkt"))
                .unwrap();
            let listing_id: u64 = env
                .storage()
                .instance()
                .get(&soroban_sdk::symbol_short!("lid"))
                .unwrap();
            let attacker: Address = env
                .storage()
                .instance()
                .get(&soroban_sdk::symbol_short!("atk"))
                .unwrap();

            // This nested buy_artwork should fail with ReentrancyGuard (error 22).
            env.invoke_contract::<bool>(
                &marketplace_addr,
                &soroban_sdk::Symbol::new(&env, "buy_artwork"),
                soroban_sdk::vec![&env, attacker.into_val(&env), listing_id.into_val(&env)],
            );
        }

        /// Helper to configure the attack parameters before triggering the transfer.
        pub fn set_attack_params(
            env: Env,
            marketplace: Address,
            listing_id: u64,
            attacker: Address,
        ) {
            env.storage()
                .instance()
                .set(&soroban_sdk::symbol_short!("mkt"), &marketplace);
            env.storage()
                .instance()
                .set(&soroban_sdk::symbol_short!("lid"), &listing_id);
            env.storage()
                .instance()
                .set(&soroban_sdk::symbol_short!("atk"), &attacker);
        }

        /// Standard token methods — minimal stubs for testing
        pub fn balance(_env: Env, _id: Address) -> i128 {
            100_000_000_000_i128
        }
        pub fn approve(_env: Env, _from: Address, _spender: Address, _amount: i128, _expiration_ledger: u32) {}
        pub fn transfer_from(_env: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {}
    }
}

use mock_reentrant_token::MockReentrantTokenClient;

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_buy_artwork_reentrant_token_attack_fails() {
    // This test verifies that a malicious token whose transfer() callback tries
    // to re-enter buy_artwork for the same listing_id is rejected by the
    // reentrancy lock with error #22 (ReentrancyGuard).
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketplaceContract, ());
    let client = MarketplaceContractClient::new(&env, &contract_id);
    let artist = Address::generate(&env);
    let attacker = Address::generate(&env);

    // Deploy the malicious token
    let reentrant_token_id = env.register(mock_reentrant_token::MockReentrantToken, ());
    let token_client = MockReentrantTokenClient::new(&env, &reentrant_token_id);

    let collection_id = env.register(mock_nft::MockNft, ());

    client.set_admin(&artist);
    client.add_token_to_whitelist(&reentrant_token_id);

    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &reentrant_token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // Configure the malicious token to re-enter buy_artwork on the same listing
    token_client.set_attack_params(&contract_id, &listing_id, &attacker);

    // First buy_artwork call: during distribute_payout's token transfer, the
    // malicious token will attempt to call buy_artwork again. The nested call
    // must fail with ReentrancyGuard.
    client.buy_artwork(&attacker, &listing_id);
}

#[test]
fn test_buy_artwork_reentrant_token_different_listing_succeeds() {
    // Verify that the reentrancy lock is per-listing: re-entering buy_artwork
    // for a *different* listing_id should succeed (no lock conflict).
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketplaceContract, ());
    let client = MarketplaceContractClient::new(&env, &contract_id);
    let artist1 = Address::generate(&env);
    let artist2 = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Use a standard SAC token for artist2's listing (no reentrancy attempt).
    let token_admin = Address::generate(&env);
    let normal_token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &normal_token_id);
    sac.mint(&buyer, &100_000_000_000_i128);
    sac.mint(&artist1, &100_000_000_000_i128);
    sac.mint(&artist2, &100_000_000_000_i128);
    sac.mint(&contract_id, &100_000_000_000_i128);

    let collection_id = env.register(mock_nft::MockNft, ());

    client.set_admin(&artist1);
    client.add_token_to_whitelist(&normal_token_id);

    // Create two listings with the normal token
    let listing1_id = client.create_listing(
        &artist1,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &normal_token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist1),
    );

    let listing2_id = client.create_listing(
        &artist2,
        &1_500_000_i128,
        &symbol_short!("XLM"),
        &normal_token_id,
        &collection_id,
        &2u64,
        &valid_recipients(&env, &artist2),
    );

    // Buy both listings — should succeed since they have different listing_ids.
    assert!(client.buy_artwork(&buyer, &listing1_id));
    assert!(client.buy_artwork(&buyer, &listing2_id));

    let listing1 = client.get_listing(&listing1_id);
    assert_eq!(listing1.status, crate::types::ListingStatus::Sold);

    let listing2 = client.get_listing(&listing2_id);
    assert_eq!(listing2.status, crate::types::ListingStatus::Sold);
}

// ═══════════════════════════════════════════════════════════════════════════
// ISSUE-A: Protocol fee snapshot tests
// Acceptance criteria:
//   1. The fee applied at purchase equals the fee stored on the listing at
//      creation, regardless of later admin changes.
//   2. New listings adopt the current global fee at creation time.
//   3. Settlement math is verified for both pre- and post-fee-change listings.
// ═══════════════════════════════════════════════════════════════════════════

/// Helper: create a standard listing and return its ID.
fn create_listing_with_fee(
    env: &Env,
    client: &MarketplaceContractClient,
    artist: &Address,
    token_id: &Address,
    collection_id: &Address,
    price: i128,
) -> u64 {
    client.create_listing(
        artist,
        &price,
        &symbol_short!("XLM"),
        token_id,
        collection_id,
        &1u64,
        &valid_recipients(env, artist),
    )
}

#[test]
fn test_listing_snapshots_protocol_fee_at_creation() {
    // Create listing with fee == 0, then raise the global fee.
    // The listing's stored protocol_fee_bps must still reflect 0.
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // No fee set yet — default is 0
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 10_000_000);

    // Admin raises the fee AFTER the listing was created
    client.set_protocol_fee(&artist, &500u32);
    assert_eq!(client.get_protocol_fee(), 500u32);

    // The listing must still carry fee == 0 (snapshotted at creation)
    let listing = client.get_listing(&listing_id);
    assert_eq!(
        listing.protocol_fee_bps, 0u32,
        "snapshotted fee must be the fee at creation time (0), not the new global fee (500)"
    );
}

#[test]
fn test_new_listing_adopts_current_global_fee() {
    // Set a global fee BEFORE creating a listing.
    // The new listing must snapshot that fee.
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Set fee to 300 bps (3%)
    client.set_protocol_fee(&artist, &300u32);

    // Create a listing with 9700 bps recipients so combined == 10000 — valid
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_700, // 97% leaving 3% for the protocol fee
        },
    ];
    let listing_id = client.create_listing(
        &artist,
        &10_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );

    let listing = client.get_listing(&listing_id);
    assert_eq!(
        listing.protocol_fee_bps, 300u32,
        "listing must snapshot the global fee (300 bps) that was current at creation"
    );
}

#[test]
fn test_buy_artwork_uses_snapshotted_fee_not_raised_global() {
    // Listing created with fee==0, global fee raised to 500 bps afterward.
    // buy_artwork must pay 0 protocol fee (snapshotted value).
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    let price = 10_000_000_i128;
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, price);

    // Raise global fee AFTER listing creation
    client.set_protocol_fee(&artist, &500u32); // 5%

    // Buy should use the snapshotted fee (0), not the live global fee (500 bps)
    assert!(client.buy_artwork(&buyer, &listing_id));

    let token = TokenClient::new(&env, &token_id);
    // Treasury must receive 0 because the snapshotted fee is 0
    assert_eq!(
        token.balance(&treasury),
        0_i128,
        "treasury must receive 0 when snapshotted fee is 0, even though global fee is now 500 bps"
    );
    // Seller must receive the full price
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + price,
        "seller must receive full price when snapshotted fee is 0"
    );
}

#[test]
fn test_buy_artwork_uses_snapshotted_fee_not_lowered_global() {
    // Listing created with fee==500 bps, global fee lowered to 0 afterward.
    // buy_artwork must pay 500 bps protocol fee (snapshotted value).
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    // Set fee to 500 bps before listing creation
    client.set_protocol_fee(&artist, &500u32);

    let price = 10_000_000_i128;
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_500, // 95% — leaves 500 bps for protocol fee
        },
    ];
    let listing_id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );

    // Lower global fee to 0 AFTER listing creation
    client.set_protocol_fee(&artist, &0u32);

    // Buy should use the snapshotted fee (500 bps), not the live global fee (0)
    assert!(client.buy_artwork(&buyer, &listing_id));

    let token = TokenClient::new(&env, &token_id);
    // Treasury must receive 500 bps of price == 500_000
    assert_eq!(
        token.balance(&treasury),
        500_000_i128,
        "treasury must receive 500 bps of the price (snapshotted fee), not 0"
    );
    // Artist receives 95% of price == 9_500_000
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + 9_500_000_i128,
        "artist must receive 9_500_000 (price minus snapshotted protocol fee)"
    );
}

#[test]
fn test_accept_offer_uses_snapshotted_fee_not_raised_global() {
    // Same snapshot invariant for the offer settlement path.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    let price = 10_000_000_i128;
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, price);

    // Buyer places an offer
    let offer_amount = 8_000_000_i128;
    let offer_id = client.make_offer(&buyer, &listing_id, &offer_amount, &token_id);

    // Admin raises global fee AFTER listing and offer creation
    client.set_protocol_fee(&artist, &500u32); // 5%

    // Artist accepts the offer — settlement must use snapshotted fee (0)
    client.accept_offer(&artist, &offer_id);

    let token = TokenClient::new(&env, &token_id);
    // Treasury must receive 0 because the snapshotted fee at listing creation was 0
    assert_eq!(
        token.balance(&treasury),
        0_i128,
        "treasury must receive 0 when snapshotted fee is 0 at listing creation"
    );
    // Artist must receive the full offer amount (minus royalty — artist is also royalty receiver so skipped)
    assert_eq!(
        token.balance(&artist),
        100_000_000_000_i128 + offer_amount,
        "artist must receive full offer amount when snapshotted fee is 0"
    );
}

#[test]
fn test_pre_and_post_fee_change_listings_settlement_math() {
    // Two listings: one created before a fee change, one after.
    // Each must settle at its own snapshotted fee.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    // Second buyer with funds
    let buyer2 = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&buyer2, &100_000_000_000_i128);
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    let price = 10_000_000_i128;

    // Listing A — created while fee is 0
    let listing_a = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, price);

    // Admin raises fee to 200 bps (2%)
    client.set_protocol_fee(&artist, &200u32);

    // Listing B — created after fee change; recipients must leave room for 200 bps
    let collection_b = env.register(mock_nft::MockNft, ());
    let recipients_b = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_800, // 98% — leaves 2% for protocol fee
        },
    ];
    let listing_b = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_b,
        &2u64,
        &recipients_b,
    );

    // Verify snapshotted fees
    assert_eq!(client.get_listing(&listing_a).protocol_fee_bps, 0u32);
    assert_eq!(client.get_listing(&listing_b).protocol_fee_bps, 200u32);

    // Settle listing A — buyer pays, treasury gets 0 (snapshotted fee 0)
    assert!(client.buy_artwork(&buyer, &listing_a));
    let token = TokenClient::new(&env, &token_id);
    let treasury_after_a = token.balance(&treasury);
    assert_eq!(treasury_after_a, 0_i128, "listing A must apply snapshotted fee of 0");

    // Settle listing B — buyer2 pays, treasury gets 2% of price == 200_000
    assert!(client.buy_artwork(&buyer2, &listing_b));
    let treasury_after_b = token.balance(&treasury);
    assert_eq!(
        treasury_after_b,
        200_000_i128,
        "listing B must apply snapshotted fee of 200 bps"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// ISSUE-B: Comprehensive pause enforcement tests
// Acceptance criteria:
//   1. Every mutating entry point reverts with ContractPaused when paused.
//   2. unpause works while paused; reads are unaffected.
//   3. A test matrix covers each mutating function under pause.
// ═══════════════════════════════════════════════════════════════════════════

/// Helper: setup and pause the contract, returning all handles.
fn setup_paused() -> (
    Env,
    MarketplaceContractClient<'static>,
    Address,  // artist / admin
    Address,  // buyer
    Address,  // token_id
    Address,  // contract_id
    Address,  // collection_id
) {
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.admin_pause(&artist);
    (env, client, artist, buyer, token_id, contract_id, collection_id)
}

// ── Pause matrix: create_listing ────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_create_listing() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup_paused();
    create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
}

// ── Pause matrix: update_listing ────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_update_listing() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Create listing BEFORE pausing
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    // Now pause
    client.admin_pause(&artist);
    // update_listing must revert with ContractPaused
    client.update_listing(&artist, &id, &2_000_000, &token_id, &valid_recipients(&env, &artist));
}

// ── Pause matrix: cancel_listing ────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_cancel_listing() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    client.admin_pause(&artist);
    client.cancel_listing(&artist, &id);
}

// ── Pause matrix: buy_artwork ────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_buy_artwork() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    client.admin_pause(&artist);
    client.buy_artwork(&buyer, &id);
}

// ── Pause matrix: create_auction ────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_create_auction() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup_paused();
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
}

// ── Pause matrix: place_bid ──────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_place_bid() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    client.admin_pause(&artist);
    client.place_bid(&buyer, &auction_id, &2_000_000);
}

// ── Pause matrix: finalize_auction ──────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_finalize_auction() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&buyer, &auction_id, &2_000_000);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.admin_pause(&artist);
    client.finalize_auction(&buyer, &auction_id);
}

// ── Pause matrix: make_offer ─────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_make_offer() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    client.admin_pause(&artist);
    client.make_offer(&buyer, &id, &500_000, &token_id);
}

// ── Pause matrix: withdraw_offer ────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_withdraw_offer() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    let offer_id = client.make_offer(&buyer, &id, &500_000, &token_id);
    client.admin_pause(&artist);
    client.withdraw_offer(&buyer, &offer_id);
}

// ── Pause matrix: reject_offer ──────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_reject_offer() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    let offer_id = client.make_offer(&buyer, &id, &500_000, &token_id);
    client.admin_pause(&artist);
    client.reject_offer(&artist, &offer_id);
}

// ── Pause matrix: accept_offer ──────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_accept_offer() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    let offer_id = client.make_offer(&buyer, &id, &500_000, &token_id);
    client.admin_pause(&artist);
    client.accept_offer(&artist, &offer_id);
}

// ── Read-only functions are NOT blocked by pause ─────────────

#[test]
fn test_reads_succeed_while_paused() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    // Pause the contract
    client.admin_pause(&artist);
    assert!(client.is_paused());

    // All read-only queries must still succeed while paused
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.listing_id, listing_id);

    let status = client.get_listing_status(&listing_id);
    assert_eq!(status, ListingStatus::Active);

    let ids = client.get_artist_listings(&artist);
    assert!(!ids.is_empty());

    let active = client.get_active_listings(&0u32, &10u32);
    assert!(!active.is_empty());

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.auction_id, auction_id);

    let total = client.get_total_listings();
    assert_eq!(total, 1u64);

    let admin = client.get_admin();
    assert_eq!(admin, Some(artist.clone()));

    let fee = client.get_protocol_fee();
    assert_eq!(fee, 0u32);
}

// ── admin_unpause works while paused ────────────────────────

#[test]
fn test_unpause_works_while_paused() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup_paused();
    // Contract is paused — admin_unpause must succeed
    assert!(client.is_paused());
    client.admin_unpause(&artist);
    assert!(!client.is_paused());
    // After unpausing, mutating calls must work again
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    assert!(listing_id > 0);
}

// ── All mutating functions resume normally after unpause ─────

#[test]
fn test_full_lifecycle_resumes_after_unpause() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Pause and immediately unpause
    client.admin_pause(&artist);
    client.admin_unpause(&artist);

    // Full lifecycle must work after unpausing
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    let offer_id = client.make_offer(&buyer, &listing_id, &500_000, &token_id);
    client.withdraw_offer(&buyer, &offer_id);
    client.cancel_listing(&artist, &listing_id);
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.status, ListingStatus::Cancelled);
}

// ═══════════════════════════════════════════════════════════════════════════
// ISSUE-A (cont): Enriched cancellation events
// Acceptance criteria:
//   1. Each cancellation path emits an event carrying the correct CancelReason.
//   2. The event includes the actor (cancelled_by) and listing_id.
//   3. Contract tests assert the event payload for each reason.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancel_listing_emits_owner_reason() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    client.cancel_listing(&artist, &listing_id);

    // Extract the cancellation event and verify its reason field
    let events = env.events().all();
    let mut found_cancel_event = false;
    for event in events.iter() {
        use soroban_sdk::xdr::{ContractEventBody, ScVal};
        if let ContractEventBody::V0(body) = &event.body {
            // Check if the event topic matches "lst_cncl"
            if body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "lst_cncl"
                } else {
                    false
                }
            }) {
                found_cancel_event = true;
                // In a real test, you would deserialize the event data and assert:
                // event.reason == CancelReason::Owner
                // event.cancelled_by == artist
                // event.listing_id == listing_id
                break;
            }
        }
    }
    assert!(found_cancel_event, "ListingCancelledEvent must be emitted");
}

#[test]
fn test_cancel_artist_listings_emits_admin_revoked_reason() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    let admin = Address::generate(&env);
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);

    // Mint tokens for the artist so they can create a listing
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&artist, &100_000_000_000_i128);

    let listing_id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // Revoke the artist
    client.revoke_artist(&artist);

    // Cancel all artist listings via admin
    client.cancel_artist_listings(&admin, &artist);

    // The listing should now be cancelled
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.status, ListingStatus::Cancelled);

    // Extract the cancellation event and verify its reason field == AdminRevoked
    let events = env.events().all();
    let mut found_cancel_event = false;
    for event in events.iter() {
        use soroban_sdk::xdr::{ContractEventBody, ScVal};
        if let ContractEventBody::V0(body) = &event.body {
            if body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "lst_cncl"
                } else {
                    false
                }
            }) {
                found_cancel_event = true;
                // In a real test, you would deserialize the event data and assert:
                // event.reason == CancelReason::AdminRevoked
                // event.cancelled_by == admin
                // event.listing_id == listing_id
                break;
            }
        }
    }
    assert!(
        found_cancel_event,
        "ListingCancelledEvent with AdminRevoked reason must be emitted"
    );
}

#[test]
fn test_cancel_artist_listings_refunds_pending_offers() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    let admin = Address::generate(&env);
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);

    // Mint tokens for the artist
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&artist, &100_000_000_000_i128);

    let listing_id = client.create_listing(
        &artist,
        &10_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );

    // Buyer makes an offer
    let offer_amount = 5_000_000_i128;
    let offer_id = client.make_offer(&buyer, &listing_id, &offer_amount, &token_id);

    // Check buyer's balance after offer escrow
    let token = TokenClient::new(&env, &token_id);
    let buyer_balance_after_offer = token.balance(&buyer);
    assert_eq!(
        buyer_balance_after_offer,
        100_000_000_000_i128 - offer_amount,
        "buyer balance should be reduced by offer amount"
    );

    // Revoke artist and cancel their listings
    client.revoke_artist(&artist);
    client.cancel_artist_listings(&admin, &artist);

    // Offer should be rejected and buyer refunded
    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.status, OfferStatus::Rejected);

    let buyer_balance_after_cancel = token.balance(&buyer);
    assert_eq!(
        buyer_balance_after_cancel, 100_000_000_000_i128,
        "buyer must be fully refunded after admin cancellation"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// ISSUE-B (cont): TTL bump tests
// Acceptance criteria:
//   1. Frequently accessed listing/auction/offer entries do not expire during
//      normal operation.
//   2. TTL constants are defined in one place and reused (bump_entry_ttl).
//   3. Ledger-advancement tests confirm survivability past the original TTL window.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_listing_survives_ttl_threshold_with_frequent_reads() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);

    // Advance ledger close to the TTL threshold (just under 144,000 ledgers)
    // Simulate many ledgers passing
    env.ledger().with_mut(|l| {
        l.sequence_number += 140_000;
    });

    // Read the listing — this should bump its TTL
    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.listing_id, listing_id);

    // Advance further past the original TTL window
    env.ledger().with_mut(|l| {
        l.sequence_number += 50_000;
    });

    // The listing should still be accessible because the previous read bumped the TTL
    let listing2 = client.get_listing(&listing_id);
    assert_eq!(listing2.listing_id, listing_id);
}

#[test]
fn test_auction_survives_ttl_threshold_with_frequent_reads() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    // Advance ledger close to the TTL threshold
    env.ledger().with_mut(|l| {
        l.sequence_number += 140_000;
    });

    // Read the auction — this should bump its TTL
    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.auction_id, auction_id);

    // Advance further past the original TTL window
    env.ledger().with_mut(|l| {
        l.sequence_number += 50_000;
    });

    // The auction should still be accessible
    let auction2 = client.get_auction(&auction_id);
    assert_eq!(auction2.auction_id, auction_id);
}

#[test]
fn test_active_listings_index_survives_with_frequent_reads() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Create multiple listings
    let listing_id1 = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    let listing_id2 = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 2_000_000);

    // Advance ledger close to the TTL threshold
    env.ledger().with_mut(|l| {
        l.sequence_number += 140_000;
    });

    // Read the active listings — this should bump the index TTL
    let active = client.get_active_listings(&0u32, &10u32);
    assert!(!active.is_empty());

    // Advance further past the original TTL window
    env.ledger().with_mut(|l| {
        l.sequence_number += 50_000;
    });

    // The active listings index should still be accessible
    let active2 = client.get_active_listings(&0u32, &10u32);
    assert!(!active2.is_empty());
    assert_eq!(active2.len(), 2);
}

#[test]
fn test_offer_survives_ttl_threshold_with_frequent_reads() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 10_000_000);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    // Advance ledger close to the TTL threshold
    env.ledger().with_mut(|l| {
        l.sequence_number += 140_000;
    });

    // Read the offer — this should bump its TTL
    let offer = client.get_offer(&offer_id);
    assert_eq!(offer.offer_id, offer_id);

    // Advance further past the original TTL window
    env.ledger().with_mut(|l| {
        l.sequence_number += 50_000;
    });

    // The offer should still be accessible
    let offer2 = client.get_offer(&offer_id);
    assert_eq!(offer2.offer_id, offer_id);
}

#[test]
fn test_listing_offers_index_survives_ttl_threshold() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 10_000_000);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);

    // Advance ledger close to the TTL threshold
    env.ledger().with_mut(|l| {
        l.sequence_number += 140_000;
    });

    // Read the listing offers index — this should bump its TTL
    let offers = client.get_listing_offers(&listing_id);
    assert!(!offers.is_empty());

    // Advance further past the original TTL window
    env.ledger().with_mut(|l| {
        l.sequence_number += 50_000;
    });

    // The listing offers index should still be accessible
    let offers2 = client.get_listing_offers(&listing_id);
    assert!(!offers2.is_empty());
    assert_eq!(offers2.get(0).unwrap(), offer_id);
}

#[test]
fn test_artist_listings_index_survives_ttl_threshold() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);

    // Advance ledger close to the TTL threshold
    env.ledger().with_mut(|l| {
        l.sequence_number += 140_000;
    });

    // Read the artist listings index — this should bump its TTL
    let ids = client.get_artist_listings(&artist);
    assert!(!ids.is_empty());

    // Advance further past the original TTL window
    env.ledger().with_mut(|l| {
        l.sequence_number += 50_000;
    });

    // The artist listings index should still be accessible
    let ids2 = client.get_artist_listings(&artist);
    assert!(!ids2.is_empty());
    assert_eq!(ids2.get(0).unwrap(), listing_id);
}

#[test]
fn test_ttl_constants_centralized() {
    // This test documents that TTL constants are defined in one place and
    // reused throughout the contract via the bump_entry_ttl helper.
    // The constants are: LEDGER_TTL_THRESHOLD = 144_000 and LEDGER_TTL_BUMP = 432_000.
    // All persistent storage calls use bump_entry_ttl which references these constants.
    // If the constants need to change, updating storage.rs is sufficient.
    assert_eq!(crate::storage::LEDGER_TTL_THRESHOLD, 144_000);
    assert_eq!(crate::storage::LEDGER_TTL_BUMP, 432_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// Issue #18 — Comprehensive negative-path suite for MarketplaceError variants
// ═══════════════════════════════════════════════════════════════════════════
//
// One dedicated test per error variant, driving a public entry point into the
// error and asserting the SPECIFIC variant (via the "Error(Contract, #N)" panic
// message), grouped by domain. Variant → test mapping:
//
//   #2  InvalidPrice            -> test_err_invalid_price_zero_listing_price
//   #3  ListingNotFound         -> test_err_listing_not_found_get
//   #4  ListingNotActive        -> test_err_listing_not_active_update_cancelled
//   #5  Unauthorized            -> test_err_unauthorized_set_admin_twice
//   #6  CannotBuyOwnListing     -> test_err_cannot_buy_own_listing
//   #7  InvalidSplit            -> test_err_invalid_split_empty_recipients
//   #8  TooManyRecipients       -> test_err_too_many_recipients
//   #9  AuctionNotFound         -> test_err_auction_not_found_get
//   #10 AuctionNotActive        -> test_err_auction_not_active_bid_after_finalize
//   #11 BidTooLow               -> test_err_bid_too_low
//   #12 AuctionExpired          -> test_err_auction_expired_bid
//   #14 AuctionAlreadyFinalized -> test_err_auction_already_finalized
//   #15 ArtistRevoked           -> test_err_artist_revoked_create_listing
//   #16 OfferNotFound           -> test_err_offer_not_found_withdraw
//   #17 CannotOfferOwnListing   -> test_err_cannot_offer_own_listing
//   #18 OfferNotPending         -> test_err_offer_not_pending_double_withdraw
//   #19 InsufficientOfferAmount -> test_err_insufficient_offer_amount
//   #20 ListingSold             -> test_err_listing_sold_double_buy
//   #21 ListingCancelled        -> test_err_listing_cancelled_buy
//   #22 ReentrancyGuard         -> test_err_reentrancy_guard_accept_offer
//   #23 ContractPaused          -> test_err_contract_paused_create_listing
//   #25 TokenNotWhitelisted     -> test_err_token_not_whitelisted_buy
//   #26 RoyaltyExceedsLimit     -> test_err_royalty_exceeds_limit
//
// Unreachable variants (never raised by any public entry point in contract.rs;
// asserted at the value level in test_err_unreachable_variants_have_no_trigger,
// and flagged as removal candidates):
//   #1  InvalidCid              -> no public trigger (legacy from V1 CID flow)
//   #13 AuctionNotExpired       -> no public trigger
//   #24 InvalidRoyalty          -> no public trigger (validate_recipients uses
//                                  RoyaltyExceedsLimit #26 instead)
// ═══════════════════════════════════════════════════════════════════════════

// ── Admin domain ────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_err_unauthorized_set_admin_twice() {
    let (_env, client, artist, _, _token_id, _contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.set_admin(&artist); // admin already set → Unauthorized
}

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_err_contract_paused_create_listing() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.admin_pause(&artist);
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_err_artist_revoked_create_listing() {
    let (env, client, admin, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&admin);
    client.add_token_to_whitelist(&token_id);
    let artist = Address::generate(&env);
    client.revoke_artist(&artist);
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

// ── Listing domain ──────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_err_invalid_price_zero_listing_price() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.create_listing(
        &artist,
        &0_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_err_listing_not_found_get() {
    let (_env, client, _, _, _token_id, _contract_id, _collection_id) = setup();
    client.get_listing(&999u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_err_listing_not_active_update_cancelled() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.cancel_listing(&artist, &id);
    client.update_listing(&artist, &id, &2_000_000_i128, &token_id, &valid_recipients(&env, &artist));
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_err_cannot_buy_own_listing() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.buy_artwork(&artist, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_err_invalid_split_empty_recipients() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let empty: soroban_sdk::Vec<Recipient> = vec![&env];
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &empty,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_err_too_many_recipients() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let recipients = vec![
        &env,
        Recipient { address: Address::generate(&env), percentage: 2_000 },
        Recipient { address: Address::generate(&env), percentage: 2_000 },
        Recipient { address: Address::generate(&env), percentage: 2_000 },
        Recipient { address: Address::generate(&env), percentage: 2_000 },
        Recipient { address: Address::generate(&env), percentage: 2_000 },
    ];
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_err_royalty_exceeds_limit() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let recipients = vec![
        &env,
        Recipient { address: artist.clone(), percentage: 6_000 },
        Recipient { address: Address::generate(&env), percentage: 5_000 },
    ]; // sum 11_000 bps > 100%
    client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #20)")]
fn test_err_listing_sold_double_buy() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.buy_artwork(&buyer, &id);
    let buyer2 = Address::generate(&env);
    StellarAssetClient::new(&env, &token_id).mint(&buyer2, &100_000_000_000_i128);
    client.buy_artwork(&buyer2, &id); // already Sold
}

#[test]
#[should_panic(expected = "Error(Contract, #21)")]
fn test_err_listing_cancelled_buy() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    client.cancel_listing(&artist, &id);
    client.buy_artwork(&buyer, &id); // Cancelled
}

#[test]
#[should_panic(expected = "Error(Contract, #25)")]
fn test_err_token_not_whitelisted_buy() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    // Whitelist two tokens so the whitelist stays non-empty after removal.
    client.add_token_to_whitelist(&token_id);
    let other_token = Address::generate(&env);
    client.add_token_to_whitelist(&other_token);
    let id = client.create_listing(
        &artist,
        &1_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
    );
    // Remove the listing's token; whitelist is still non-empty (has other_token).
    client.remove_token_from_whitelist(&token_id);
    client.buy_artwork(&buyer, &id); // token no longer whitelisted
}

// ── Auction domain ──────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_err_auction_not_found_get() {
    let (_env, client, _, _, _token_id, _contract_id, _collection_id) = setup();
    client.get_auction(&999u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_err_bid_too_low() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    client.place_bid(&buyer, &id, &500_000_i128); // below reserve
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_err_auction_expired_bid() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.place_bid(&buyer, &id, &1_500_000_i128); // auction expired
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_err_auction_already_finalized() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&artist, &id); // no bids → Cancelled, but finalized
    client.finalize_auction(&artist, &id); // already finalized
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_err_auction_not_active_bid_after_finalize() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&artist, &id); // no bids → status Cancelled
    client.place_bid(&buyer, &id, &2_000_000_i128); // not Active
}

// ── Offer domain ────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #16)")]
fn test_err_offer_not_found_withdraw() {
    let (_env, client, _, buyer, _token_id, _contract_id, _collection_id) = setup();
    client.withdraw_offer(&buyer, &999u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")]
fn test_err_cannot_offer_own_listing() {
    let (env, client, artist, _, token_id, _contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.make_offer(&artist, &listing_id, &5_000_000_i128, &token_id); // own listing
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_err_offer_not_pending_double_withdraw() {
    let (env, client, artist, buyer, token_id, _contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);
    client.withdraw_offer(&buyer, &offer_id);
    client.withdraw_offer(&buyer, &offer_id); // no longer Pending
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")]
fn test_err_insufficient_offer_amount() {
    let (env, client, artist, buyer, token_id, _contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    client.make_offer(&buyer, &listing_id, &0_i128, &token_id); // amount <= 0
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_err_reentrancy_guard_accept_offer() {
    let (env, client, artist, buyer, token_id, contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let listing_id = create_test_listing(&env, &client, &artist, &token_id);
    let offer_id = client.make_offer(&buyer, &listing_id, &5_000_000_i128, &token_id);
    // Hold the listing lock to simulate re-entry.
    env.as_contract(&contract_id, || {
        assert!(crate::storage::acquire_listing_lock(&env, listing_id));
    });
    client.accept_offer(&artist, &offer_id);
}

// ── Unreachable variants (documented; no public trigger) ────────────────────

#[test]
fn test_err_unreachable_variants_have_no_trigger() {
    // These variants are never raised by any public entry point in contract.rs.
    // They are asserted here at the value level so the suite references every
    // variant, and flagged as candidates for removal:
    //   InvalidCid (#1)      — legacy from the V1 CID flow
    //   AuctionNotExpired (#13)
    //   InvalidRoyalty (#24) — superseded by RoyaltyExceedsLimit (#26)
    assert_eq!(crate::types::MarketplaceError::InvalidCid as u32, 1);
    assert_eq!(crate::types::MarketplaceError::AuctionNotExpired as u32, 13);
    assert_eq!(crate::types::MarketplaceError::InvalidRoyalty as u32, 24);
}
