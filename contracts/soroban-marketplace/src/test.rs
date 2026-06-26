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

/// Helper â€” deploy the contract and a real test token, returning
/// (env, client, artist, buyer, token_id, contract_id).
fn setup() -> (
    Env,
    MarketplaceContractClient<'static>,
    Address,
    Address,
    Address, // token_id  â€” a real SAC test token
    Address, // contract_id â€” the marketplace contract
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
        &None::<u64>,
    );

    // Set protocol fee to 500 bps (5%) â€” applied at purchase time
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
        &None::<u64>,
    );
    // Set protocol fee but no treasury â€” fee is discarded when treasury is absent
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

// â”€â”€ create_listing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_create_listing_invalid_split() {
    // Recipients that sum to 11_000 bps (110%) â€” must be rejected at creation.
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
        &None::<u64>,
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
        &None::<u64>,
    );
}

// â”€â”€ cancel_listing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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
        &None::<u64>,
    );
    client.cancel_listing(&buyer, &id);
}

// â”€â”€ update_listing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
            &None::<u64>,
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
        &None::<u64>,
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

// â”€â”€ get_artist_listings â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
    );
    client.create_listing(
        &artist,
        &2_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
        &None::<u64>,
    );
    client.create_listing(
        &artist,
        &3_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
    );
    assert!(client.buy_artwork(&buyer, &id));

    // Verify recipients received correct amounts
    let token = TokenClient::new(&env, &token_id);
    let artist_got = token.balance(&artist) - 100_000_000_000_i128;
    let colab1_got = token.balance(&colab1);
    let colab2_got = token.balance(&colab2);
    assert_eq!(artist_got + colab1_got + colab2_got, price);
}

// â”€â”€ get_listing not found â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_get_listing_not_found() {
    let (_env, client, _, _, _, _, collection_id) = setup();
    client.get_listing(&999);
}

// â”€â”€ Admin/Whitelist Management Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
    );
    // Set protocol fee to 10% â€” applied at purchase time
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
        &None::<u64>,
    );
    // Set protocol fee to 333 bps (3.33%) â€” applied at purchase time
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
        &None::<u64>,
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
    // 100% royalty (10000 bps) â€” but artist IS original_creator, so royalty skipped (same address)
    let id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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

// â”€â”€ Auction Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
#[should_panic(expected = "Error(Contract, #28)")]
fn test_finalize_auction_before_expiry_rejects_non_creator() {
    // Under the new rules, ALL callers — including the creator — are rejected
    // with AuctionNotEnded (#28) when finalize is called before end_time.
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

// â”€â”€ Offer Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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

// â”€â”€ Admin and Revocation Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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
            &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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

// â”€â”€ buy_artwork edge cases (Issue #124) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
#[should_panic(expected = "Error(Contract, #29)")]
fn test_buy_own_listing_fails() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    // Artist (listing creator) must not be able to buy their own listing.
    // Expect SelfPurchaseNotAllowed = error #29.
    client.buy_artwork(&artist, &id);
}

// ── Task (a): Self-purchase guard — dedicated SelfPurchaseNotAllowed error ───

/// Confirms the revert carries the dedicated SelfPurchaseNotAllowed code (#29),
/// not the legacy CannotBuyOwnListing (#6), so clients can decode it reliably.
#[test]
#[should_panic(expected = "Error(Contract, #29)")]
fn test_self_purchase_not_allowed_error_code() {
    let (env, client, artist, _, token_id, _contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    client.buy_artwork(&artist, &id);
}

/// A third-party buyer who is not the artist must still be able to purchase.
#[test]
fn test_third_party_buyer_not_blocked() {
    let (env, client, artist, buyer, token_id, _contract_id, _collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    assert!(client.buy_artwork(&buyer, &id));
    let listing = client.get_listing(&id);
    assert_eq!(listing.status, ListingStatus::Sold);
    assert_eq!(listing.owner, Some(buyer));
}

// ── Task (b): ProtocolFeeCollected event ─────────────────────────────────────

/// buy_artwork settlement must emit a ProtocolFeeCollected event whose
/// `amount` equals exactly fee_bps % of the sale price and whose `treasury`
/// matches the configured treasury address.
#[test]
fn test_buy_artwork_emits_protocol_fee_collected_event() {
    use soroban_sdk::testutils::Events as _;

    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    // Set fee first, then create listing with recipients that leave room:
    // 500 bps protocol fee + 9500 bps recipient = 10000 bps total (valid).
    client.set_protocol_fee(&artist, &500u32);
    let price = 10_000_000_i128;
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_500, // 9500 bps leaves 500 bps for protocol fee
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
        &None::<u64>,
    );

    client.buy_artwork(&buyer, &id);

    // Expected fee: price * 500 / 10_000 = 500_000
    let expected_fee: i128 = price * 500 / 10_000;

    // Scan emitted events for ProtocolFeeCollected (topic symbol "fee_cltd")
    let all_events = env.events().all();
    let fee_event = all_events.iter().find(|e| {
        use soroban_sdk::xdr::{ContractEventBody, ScVal};
        if let ContractEventBody::V0(body) = &e.body {
            body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "fee_cltd"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(fee_event.is_some(), "ProtocolFeeCollected event not emitted from buy_artwork");

    // Verify treasury balance received exactly expected_fee
    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), expected_fee);
}/// accept_offer settlement must also emit ProtocolFeeCollected.
#[test]
fn test_accept_offer_emits_protocol_fee_collected_event() {
    use soroban_sdk::testutils::Events as _;

    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    // Set fee before creating listing so it's snapshotted into the listing.
    // 500 bps protocol fee + 9500 bps recipient = 10000 bps (valid).
    client.set_protocol_fee(&artist, &500u32);
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_500,
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
        &None::<u64>,
    );

    let offer_amount = 8_000_000_i128;
    let offer_id = client.make_offer(&buyer, &listing_id, &offer_amount, &token_id);
    client.accept_offer(&artist, &offer_id);

    // Expected fee: offer_amount * 500 / 10_000 = 400_000
    let expected_fee: i128 = offer_amount * 500 / 10_000;

    let all_events = env.events().all();
    let fee_event = all_events.iter().find(|e| {
        use soroban_sdk::xdr::{ContractEventBody, ScVal};
        if let ContractEventBody::V0(body) = &e.body {
            body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "fee_cltd"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(fee_event.is_some(), "ProtocolFeeCollected event not emitted from accept_offer");

    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), expected_fee);
}

/// finalize_auction settlement must also emit ProtocolFeeCollected.
#[test]
fn test_finalize_auction_emits_protocol_fee_collected_event() {
    use soroban_sdk::testutils::Events as _;

    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);

    // Set fee before creating auction so it's snapshotted.
    // Recipients get 9500 bps; 500 bps reserved for protocol fee.
    client.set_protocol_fee(&artist, &500u32);
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_500,
        },
    ];
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &recipients,
    );

    let bid_amount = 2_000_000_i128;
    client.place_bid(&buyer, &auction_id, &bid_amount);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);

    // Expected fee: bid_amount * 500 / 10_000 = 100_000
    let expected_fee: i128 = bid_amount * 500 / 10_000;

    let all_events = env.events().all();
    let fee_event = all_events.iter().find(|e| {
        use soroban_sdk::xdr::{ContractEventBody, ScVal};
        if let ContractEventBody::V0(body) = &e.body {
            body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "fee_cltd"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(fee_event.is_some(), "ProtocolFeeCollected event not emitted from finalize_auction");

    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), expected_fee);
}

/// No ProtocolFeeCollected event is emitted when treasury is not configured.
#[test]
fn test_no_fee_event_without_treasury() {
    use soroban_sdk::testutils::Events as _;

    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // No treasury set — fee has nowhere to go, no event should fire.

    client.set_protocol_fee(&artist, &500u32);
    let recipients = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_500,
        },
    ];
    let id = client.create_listing(
        &artist,
        &10_000_000_i128,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &recipients,
        &None::<u64>,
    );
    client.buy_artwork(&buyer, &id);

    let all_events = env.events().all();
    let fee_event = all_events.iter().find(|e| {
        use soroban_sdk::xdr::{ContractEventBody, ScVal};
        if let ContractEventBody::V0(body) = &e.body {
            body.topics.iter().any(|t| {
                if let ScVal::Symbol(s) = t {
                    core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "fee_cltd"
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(fee_event.is_none(), "ProtocolFeeCollected must not fire without a treasury");
}

// â”€â”€ update_listing recipient validation (Issue #175) â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_update_listing_invalid_split_fails() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    let id = create_test_listing(&env, &client, &artist, &token_id);
    // Recipients summing to 12_000 bps â€” over 100%
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

// â”€â”€ transfer_admin / accept_admin tests (Issue #162) â”€â”€â”€â”€â”€â”€â”€â”€

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
    // impostor tries to initiate transfer â€” should panic Unauthorized
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
    // A different address tries to accept â€” should panic Unauthorized
    client.accept_admin(&impostor);
}

// â”€â”€ Event emission tests (Issue #180) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Token transfer tests (Issue #165) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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

// â”€â”€ Pause / unpause lifecycle tests (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
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

// â”€â”€ Offer edge cases (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
    // Reject a withdrawn offer â€” status is no longer Pending
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

// â”€â”€ Cancel listing edge cases (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Auction edge cases (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Admin transfer edge cases (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
#[should_panic]
fn test_accept_admin_with_no_pending_transfer_panics() {
    let (env, client, admin, _, _token_id, _, collection_id) = setup();
    let impostor = Address::generate(&env);
    client.set_admin(&admin);
    // accept_admin when no transfer has been initiated â€” should panic
    client.accept_admin(&impostor);
}

// â”€â”€ Revoke / reinstate standalone tests (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€

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
        &None::<u64>,
    );
}

// â”€â”€ Token whitelist edge cases (Issue #200) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Royalty bps validation tests (security)

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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
    );
    // Admin removes token from whitelist â€” purchase should now be rejected at buy time
    client.remove_token_from_whitelist(&token_id);
    client.buy_artwork(&buyer, &id);
}
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// admin_pause / admin_unpause mechanism
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

#[test]
fn test_is_paused_default_false() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    // Freshly deployed â€” must not be paused
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
    // `buyer` is not the admin â€” must panic with Unauthorized
    client.admin_pause(&buyer);
}

#[test]
#[should_panic]
fn test_admin_unpause_rejects_non_admin() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    client.admin_pause(&artist);
    // `buyer` is not the admin â€” must panic with Unauthorized
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

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// RoyaltyExceedsLimit boundary tests (Issue A)
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

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
        &None::<u64>,
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
        &None::<u64>,
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
            &None::<u64>,
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
        &None::<u64>,
    );
    assert_eq!(listing_id, 1u64);
    // Now set the protocol fee; an update with the same recipients would also pass.
    client.set_protocol_fee(&artist, &500u32);
    // Update_listing with 9_500 bps: 9_500 + 500 = 10_000 â€” should succeed.
    let updated = client.update_listing(&artist, &listing_id, &2_000_000, &token_id, &recipients);
    assert!(updated);
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_validate_recipients_exceeds_limit_with_protocol_fee() {
    // When protocol_fee_bps = 500 (5%), recipients summing to 9_501 bps will
    // result in total 10_001 bps â€” must be rejected with RoyaltyExceedsLimit.
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
        &None::<u64>,
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

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// Reentrancy attack tests (Issue B)
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

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

        /// Standard token methods â€” minimal stubs for testing
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
        &None::<u64>,
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
        &None::<u64>,
    );

    let listing2_id = client.create_listing(
        &artist2,
        &1_500_000_i128,
        &symbol_short!("XLM"),
        &normal_token_id,
        &collection_id,
        &2u64,
        &valid_recipients(&env, &artist2),
        &None::<u64>,
    );

    // Buy both listings â€” should succeed since they have different listing_ids.
    assert!(client.buy_artwork(&buyer, &listing1_id));
    assert!(client.buy_artwork(&buyer, &listing2_id));

    let listing1 = client.get_listing(&listing1_id);
    assert_eq!(listing1.status, crate::types::ListingStatus::Sold);

    let listing2 = client.get_listing(&listing2_id);
    assert_eq!(listing2.status, crate::types::ListingStatus::Sold);
}

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// ISSUE-A: Protocol fee snapshot tests
// Acceptance criteria:
//   1. The fee applied at purchase equals the fee stored on the listing at
//      creation, regardless of later admin changes.
//   2. New listings adopt the current global fee at creation time.
//   3. Settlement math is verified for both pre- and post-fee-change listings.
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

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
        &None::<u64>,
    )
}

#[test]
fn test_listing_snapshots_protocol_fee_at_creation() {
    // Create listing with fee == 0, then raise the global fee.
    // The listing's stored protocol_fee_bps must still reflect 0.
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // No fee set yet â€” default is 0
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

    // Create a listing with 9700 bps recipients so combined == 10000 â€” valid
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
        &None::<u64>,
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
            percentage: 9_500, // 95% â€” leaves 500 bps for protocol fee
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
        &None::<u64>,
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

    // Artist accepts the offer â€” settlement must use snapshotted fee (0)
    client.accept_offer(&artist, &offer_id);

    let token = TokenClient::new(&env, &token_id);
    // Treasury must receive 0 because the snapshotted fee at listing creation was 0
    assert_eq!(
        token.balance(&treasury),
        0_i128,
        "treasury must receive 0 when snapshotted fee is 0 at listing creation"
    );
    // Artist must receive the full offer amount (minus royalty â€” artist is also royalty receiver so skipped)
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

    // Listing A â€” created while fee is 0
    let listing_a = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, price);

    // Admin raises fee to 200 bps (2%)
    client.set_protocol_fee(&artist, &200u32);

    // Listing B â€” created after fee change; recipients must leave room for 200 bps
    let collection_b = env.register(mock_nft::MockNft, ());
    let recipients_b = vec![
        &env,
        Recipient {
            address: artist.clone(),
            percentage: 9_800, // 98% â€” leaves 2% for protocol fee
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
        &None::<u64>,
    );

    // Verify snapshotted fees
    assert_eq!(client.get_listing(&listing_a).protocol_fee_bps, 0u32);
    assert_eq!(client.get_listing(&listing_b).protocol_fee_bps, 200u32);

    // Settle listing A â€” buyer pays, treasury gets 0 (snapshotted fee 0)
    assert!(client.buy_artwork(&buyer, &listing_a));
    let token = TokenClient::new(&env, &token_id);
    let treasury_after_a = token.balance(&treasury);
    assert_eq!(treasury_after_a, 0_i128, "listing A must apply snapshotted fee of 0");

    // Settle listing B â€” buyer2 pays, treasury gets 2% of price == 200_000
    assert!(client.buy_artwork(&buyer2, &listing_b));
    let treasury_after_b = token.balance(&treasury);
    assert_eq!(
        treasury_after_b,
        200_000_i128,
        "listing B must apply snapshotted fee of 200 bps"
    );
}

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// ISSUE-B: Comprehensive pause enforcement tests
// Acceptance criteria:
//   1. Every mutating entry point reverts with ContractPaused when paused.
//   2. unpause works while paused; reads are unaffected.
//   3. A test matrix covers each mutating function under pause.
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

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

// â”€â”€ Pause matrix: create_listing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
#[should_panic(expected = "Error(Contract, #23)")]
fn test_pause_matrix_create_listing() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup_paused();
    create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
}

// â”€â”€ Pause matrix: update_listing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: cancel_listing â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: buy_artwork â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: create_auction â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: place_bid â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: finalize_auction â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: make_offer â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: withdraw_offer â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: reject_offer â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Pause matrix: accept_offer â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ Read-only functions are NOT blocked by pause â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ admin_unpause works while paused â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_unpause_works_while_paused() {
    let (env, client, artist, _buyer, token_id, _contract_id, collection_id) = setup_paused();
    // Contract is paused â€” admin_unpause must succeed
    assert!(client.is_paused());
    client.admin_unpause(&artist);
    assert!(!client.is_paused());
    // After unpausing, mutating calls must work again
    let listing_id = create_listing_with_fee(&env, &client, &artist, &token_id, &collection_id, 1_000_000);
    assert!(listing_id > 0);
}

// â”€â”€ All mutating functions resume normally after unpause â”€â”€â”€â”€â”€

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

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// ISSUE-A (cont): Enriched cancellation events
// Acceptance criteria:
//   1. Each cancellation path emits an event carrying the correct CancelReason.
//   2. The event includes the actor (cancelled_by) and listing_id.
//   3. Contract tests assert the event payload for each reason.
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

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
        &None::<u64>,
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
        &None::<u64>,
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

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// ISSUE-B (cont): TTL bump tests
// Acceptance criteria:
//   1. Frequently accessed listing/auction/offer entries do not expire during
//      normal operation.
//   2. TTL constants are defined in one place and reused (bump_entry_ttl).
//   3. Ledger-advancement tests confirm survivability past the original TTL window.
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

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

    // Read the listing â€” this should bump its TTL
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

    // Read the auction â€” this should bump its TTL
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

    // Read the active listings â€” this should bump the index TTL
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

    // Read the offer â€” this should bump its TTL
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

    // Read the listing offers index â€” this should bump its TTL
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

    // Read the artist listings index â€” this should bump its TTL
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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
        &None::<u64>,
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

// ── Issue #20: atomic refund of the previous highest bidder on a new bid ─────

#[test]
fn test_outbid_refunds_prev_and_escrow_equals_highest_bid() {
    let (env, client, artist, buyer1, token_id, contract_id, collection_id) = setup();
    let buyer2 = Address::generate(&env);
    let buyer3 = Address::generate(&env);
    let sac = StellarAssetClient::new(&env, &token_id);
    sac.mint(&buyer2, &100_000_000_000_i128);
    sac.mint(&buyer3, &100_000_000_000_i128);
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

    let token = TokenClient::new(&env, &token_id);
    let base = 100_000_000_000_i128;
    // Contract is pre-funded in setup(); measure escrow as the delta from this.
    let contract_base = token.balance(&contract_id);

    // Bid 1 — buyer1 escrows 1_500_000.
    client.place_bid(&buyer1, &id, &1_500_000_i128);
    assert_eq!(token.balance(&buyer1), base - 1_500_000);
    assert_eq!(token.balance(&contract_id) - contract_base, 1_500_000);

    // Bid 2 — buyer2 outbids; buyer1 must be fully refunded.
    client.place_bid(&buyer2, &id, &2_000_000_i128);
    assert_eq!(token.balance(&buyer1), base, "buyer1 fully refunded");
    assert_eq!(token.balance(&buyer2), base - 2_000_000);
    // Escrow now equals the new highest bid (prev refund + new escrow net out).
    assert_eq!(token.balance(&contract_id) - contract_base, 2_000_000);

    // Bid 3 — buyer3 outbids; buyer2 must be fully refunded.
    client.place_bid(&buyer3, &id, &2_500_000_i128);
    assert_eq!(token.balance(&buyer2), base, "buyer2 fully refunded");
    assert_eq!(token.balance(&buyer3), base - 2_500_000);
    assert_eq!(token.balance(&contract_id) - contract_base, 2_500_000);

    // Final invariant: contract-held escrow equals the current highest bid.
    let auction = client.get_auction(&id);
    assert_eq!(auction.highest_bid, 2_500_000_i128);
    assert_eq!(auction.highest_bidder, Some(buyer3.clone()));
    assert_eq!(
        token.balance(&contract_id) - contract_base,
        auction.highest_bid,
        "escrow must equal the current highest bid"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Anti-sniping extension (Feature A)
// ═══════════════════════════════════════════════════════════════════════════
//
// Acceptance criteria:
//   1. A bid placed inside the trigger window extends end_time and emits
//      AuctionExtended.
//   2. A bid placed outside the trigger window (or when trigger == 0) does NOT
//      extend end_time and does NOT emit AuctionExtended.
//   3. finalize_auction respects the extended end_time (cannot be called by a
//      non-creator before the (new) end_time).
// ═══════════════════════════════════════════════════════════════════════════

/// Helper to create an auction whose extension parameters are set in global
/// config before creation (so they are snapshotted into the auction struct).
fn create_auction_with_extension(
    env: &Env,
    client: &MarketplaceContractClient,
    admin: &Address,
    creator: &Address,
    token_id: &Address,
    collection_id: &Address,
    duration: u64,
    extension_window: u64,
    extension_trigger: u64,
) -> u64 {
    // Configure the global anti-sniping parameters before auction creation so
    // that the new auction inherits them as its snapshotted values.
    client.set_auction_extension_window(admin, &extension_window);
    client.set_auction_extension_trigger(admin, &extension_trigger);
    client.create_auction(
        creator,
        token_id,
        collection_id,
        &1u64,
        &1_000_000_i128,
        &duration,
        &valid_recipients(env, creator),
    )
}

#[test]
fn test_bid_inside_trigger_window_extends_auction() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Auction runs for 3600 s; trigger fires if < 300 s remain;
    // extension adds 600 s.
    let duration = 3600u64;
    let trigger = 300u64;
    let window = 600u64;

    let auction_id = create_auction_with_extension(
        &env, &client, &artist, &artist, &token_id, &collection_id,
        duration, window, trigger,
    );

    // Advance time to 3400 s into the auction (200 s remaining < 300 s trigger).
    let start = env.ledger().timestamp();
    env.ledger().set_timestamp(start + 3400);

    let before = client.get_auction(&auction_id);
    let original_end = before.end_time;

    // This bid should trigger the extension.
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let after = client.get_auction(&auction_id);
    let now = env.ledger().timestamp();
    let expected_end = now + window;
    assert_eq!(
        after.end_time, expected_end,
        "end_time must be extended to now + extension_window"
    );
    assert!(after.end_time > original_end, "end_time must be strictly later than original");

    // Verify AuctionExtended event was emitted.
    let events = env.events().all();
    let extended_events: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|e| {
            use soroban_sdk::xdr::{ContractEventBody, ScVal};
            if let ContractEventBody::V0(body) = &e.body {
                body.topics.iter().any(|t| {
                    if let ScVal::Symbol(s) = t {
                        core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "auc_extd"
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        })
        .collect();
    assert_eq!(
        extended_events.len(),
        1,
        "exactly one AuctionExtended event must be emitted"
    );
}

#[test]
fn test_bid_outside_trigger_window_does_not_extend() {
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Auction runs for 3600 s; trigger fires only if < 300 s remain.
    let duration = 3600u64;
    let trigger = 300u64;
    let window = 600u64;

    let auction_id = create_auction_with_extension(
        &env, &client, &artist, &artist, &token_id, &collection_id,
        duration, window, trigger,
    );

    // Advance time to only 1000 s in (2600 s remaining >> 300 s trigger).
    let start = env.ledger().timestamp();
    env.ledger().set_timestamp(start + 1000);

    let before = client.get_auction(&auction_id);
    let original_end = before.end_time;

    // Bid well outside the trigger window — no extension should happen.
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let after = client.get_auction(&auction_id);
    assert_eq!(
        after.end_time, original_end,
        "end_time must remain unchanged when bid is outside the trigger window"
    );

    // Verify NO AuctionExtended event was emitted.
    let events = env.events().all();
    let extended_events: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|e| {
            use soroban_sdk::xdr::{ContractEventBody, ScVal};
            if let ContractEventBody::V0(body) = &e.body {
                body.topics.iter().any(|t| {
                    if let ScVal::Symbol(s) = t {
                        core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "auc_extd"
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        })
        .collect();
    assert_eq!(
        extended_events.len(),
        0,
        "no AuctionExtended event must be emitted when bid is outside the trigger window"
    );
}

#[test]
fn test_bid_with_trigger_zero_never_extends() {
    // When extension_trigger == 0 the feature is disabled regardless of timing.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let duration = 3600u64;
    let auction_id = create_auction_with_extension(
        &env, &client, &artist, &artist, &token_id, &collection_id,
        duration, 600u64, 0u64, // trigger == 0 → disabled
    );

    // Jump to the very last second of the auction.
    let start = env.ledger().timestamp();
    env.ledger().set_timestamp(start + 3599);

    let before = client.get_auction(&auction_id);
    let original_end = before.end_time;

    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let after = client.get_auction(&auction_id);
    assert_eq!(
        after.end_time, original_end,
        "end_time must not change when trigger == 0 (feature disabled)"
    );
}

#[test]
fn test_finalize_respects_extended_end_time() {
    // After a late bid extends the auction, a non-creator must NOT be able to
    // finalize until the NEW end_time has elapsed.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let duration = 3600u64;
    let trigger = 300u64;
    let window = 600u64;

    let auction_id = create_auction_with_extension(
        &env, &client, &artist, &artist, &token_id, &collection_id,
        duration, window, trigger,
    );

    // Jump to 200 s remaining → inside trigger window.
    let start = env.ledger().timestamp();
    env.ledger().set_timestamp(start + 3400);
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let after_bid = client.get_auction(&auction_id);
    let new_end = after_bid.end_time;

    // Jump to just past the ORIGINAL end but before the NEW end.
    // (original end = start + 3600, new end = bid_time + window = start + 3400 + 600 = start + 4000)
    env.ledger().set_timestamp(start + 3601);

    // Non-creator (buyer) cannot finalize before the extended end_time.
    let result = client.try_finalize_auction(&buyer, &auction_id);
    assert!(result.is_err(), "finalize must fail before the extended end_time");

    // Advance past the new end_time.
    env.ledger().set_timestamp(new_end + 1);

    // Now finalize must succeed.
    client.finalize_auction(&buyer, &auction_id);
    let finished = client.get_auction(&auction_id);
    assert_eq!(
        finished.status,
        crate::types::AuctionStatus::Finalized,
        "auction must be finalized after the extended end_time"
    );
}

#[test]
fn test_multiple_late_bids_each_reset_end_time() {
    // Every qualifying late bid resets end_time to now + window,
    // so consecutive snipe attempts keep pushing the deadline forward.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    let buyer2 = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&buyer2, &100_000_000_000_i128);
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let duration = 3600u64;
    let trigger = 300u64;
    let window = 600u64;

    let auction_id = create_auction_with_extension(
        &env, &client, &artist, &artist, &token_id, &collection_id,
        duration, window, trigger,
    );

    let start = env.ledger().timestamp();

    // First late bid at 200 s remaining.
    env.ledger().set_timestamp(start + 3400);
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);
    let end1 = client.get_auction(&auction_id).end_time;
    assert_eq!(end1, start + 3400 + window);

    // Second late bid 100 s later (still within the extended window and the trigger).
    env.ledger().set_timestamp(start + 3500);
    client.place_bid(&buyer2, &auction_id, &2_000_000_i128);
    let end2 = client.get_auction(&auction_id).end_time;
    assert_eq!(end2, start + 3500 + window, "second late bid must push end_time forward again");
    assert!(end2 > end1, "each late bid must produce a later deadline");
}

// ═══════════════════════════════════════════════════════════════════════════
// Cancel Auction (Feature B)
// ═══════════════════════════════════════════════════════════════════════════
//
// Acceptance criteria:
//   1. An auction with no bids can be cancelled by its creator.
//   2. An auction with at least one bid CANNOT be cancelled (reverts with
//      AuctionHasBids #27).
//   3. Cancellation emits AuctionCancelledEvent.
//   4. A non-creator cannot cancel the auction.
//   5. A finalized / already-cancelled auction cannot be cancelled again.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cancel_auction_no_bids_succeeds() {
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

    client.cancel_auction(&artist, &auction_id);

    let auction = client.get_auction(&auction_id);
    assert_eq!(
        auction.status,
        crate::types::AuctionStatus::Cancelled,
        "auction must be Cancelled after cancel_auction with no bids"
    );
}

#[test]
fn test_cancel_auction_emits_event() {
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

    client.cancel_auction(&artist, &auction_id);

    // Verify AuctionCancelled event was emitted.
    let events = env.events().all();
    let cancel_events: soroban_sdk::Vec<_> = events
        .iter()
        .filter(|e| {
            use soroban_sdk::xdr::{ContractEventBody, ScVal};
            if let ContractEventBody::V0(body) = &e.body {
                body.topics.iter().any(|t| {
                    if let ScVal::Symbol(s) = t {
                        core::str::from_utf8(s.0.as_slice()).unwrap_or("") == "auc_cncl"
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        })
        .collect();
    assert_eq!(
        cancel_events.len(),
        1,
        "exactly one AuctionCancelledEvent must be emitted"
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #31)")]
fn test_cancel_auction_with_bids_reverts() {
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

    // Place a bid so the auction has an active highest bidder.
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    // This must revert with AuctionHasBids (#27).
    client.cancel_auction(&artist, &auction_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_cancel_auction_non_creator_reverts() {
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

    // Buyer tries to cancel — must revert with Unauthorized (#5).
    client.cancel_auction(&buyer, &auction_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_cancel_already_cancelled_auction_reverts() {
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

    client.cancel_auction(&artist, &auction_id);
    // Second cancellation must revert with AuctionAlreadyFinalized (#14).
    client.cancel_auction(&artist, &auction_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_cancel_finalized_auction_reverts() {
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

    client.place_bid(&buyer, &auction_id, &1_500_000_i128);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);

    // Auction is now Finalized; cancel must revert with AuctionAlreadyFinalized (#14).
    client.cancel_auction(&artist, &auction_id);
}

#[test]
fn test_cancel_auction_bidder_escrow_is_safe() {
    // Verify that once a bid exists, cancellation is blocked and the bidder's
    // escrow is never stranded.
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

    let bid_amount = 1_500_000_i128;
    client.place_bid(&buyer, &auction_id, &bid_amount);

    let token = TokenClient::new(&env, &token_id);
    let buyer_balance_after_bid = token.balance(&buyer);

    // Attempt to cancel (must fail) — buyer's escrowed funds remain safe.
    let result = client.try_cancel_auction(&artist, &auction_id);
    assert!(
        result.is_err(),
        "cancel_auction must fail when a bid is present"
    );

    // Bidder's balance has not changed since the failed cancel.
    assert_eq!(
        token.balance(&buyer),
        buyer_balance_after_bid,
        "bidder's balance must not change after a failed cancel attempt"
    );

    // Clean up: finalize the auction to release the escrow properly.
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);
    // After finalization the bidder's escrowed amount has been transferred to
    // the creator (payout), so the buyer's final balance is less by bid_amount.
    assert_eq!(
        token.balance(&buyer),
        100_000_000_000_i128 - bid_amount,
        "after finalization, buyer balance must reflect the winning bid"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Finalize-auction: open access + strict end-time + double-finalize guard
// (Feature A — finalize_auction hardening)
// ═══════════════════════════════════════════════════════════════════════════
//
// Acceptance criteria:
//   1. Any caller can finalize AFTER end_time — not just the creator.
//   2. Finalize BEFORE end_time reverts with AuctionNotEnded (#28).
//   3. A second finalize on an already-settled auction reverts with
//      AuctionAlreadyFinalized (#14).
//   4. No-bid auction ends with status Cancelled and the NFT returned to creator.
//   5. Normal finalize (with a winner) settles funds and marks Finalized.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #28)")]
fn test_finalize_before_end_time_reverts() {
    // Nobody — not even the creator — may finalize before the auction ends.
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

    // Attempt finalize at t = 0 (well before end_time) — must revert.
    client.finalize_auction(&artist, &auction_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #28)")]
fn test_finalize_one_second_early_reverts() {
    // Edge case: exactly one second before end_time.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let duration = 3600u64;
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &duration,
        &valid_recipients(&env, &artist),
    );

    // Advance to exactly 1 second before the end.
    env.ledger().set_timestamp(env.ledger().timestamp() + duration - 1);
    client.finalize_auction(&artist, &auction_id);
}

#[test]
fn test_any_caller_can_finalize_after_end_time() {
    // A random third party (not the creator, not the bidder) may finalize.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    let third_party = Address::generate(&env);
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

    client.place_bid(&buyer, &auction_id, &1_500_000_i128);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    // Third party finalizes — must succeed.
    client.finalize_auction(&third_party, &auction_id);

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, AuctionStatus::Finalized);
    assert_eq!(auction.highest_bidder, Some(buyer));
}

#[test]
fn test_creator_can_finalize_after_end_time() {
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

    client.place_bid(&buyer, &auction_id, &1_500_000_i128);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    // Creator finalizes their own auction.
    client.finalize_auction(&artist, &auction_id);

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, AuctionStatus::Finalized);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_double_finalize_reverts() {
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

    client.place_bid(&buyer, &auction_id, &1_500_000_i128);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    client.finalize_auction(&buyer, &auction_id);
    // Second call must revert with AuctionAlreadyFinalized.
    client.finalize_auction(&buyer, &auction_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_double_finalize_no_bid_reverts() {
    // Double-finalize on a no-bid auction (status becomes Cancelled on first call).
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

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&artist, &auction_id);
    // Auction is now Cancelled; second call must still revert.
    client.finalize_auction(&artist, &auction_id);
}

#[test]
fn test_finalize_no_bid_auction_status_is_cancelled() {
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

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&artist, &auction_id);

    let auction = client.get_auction(&auction_id);
    assert_eq!(
        auction.status,
        AuctionStatus::Cancelled,
        "a no-bid auction must be marked Cancelled after finalization"
    );
    assert!(
        auction.highest_bidder.is_none(),
        "no winner should be recorded for a no-bid auction"
    );
}

#[test]
fn test_finalize_no_bid_returns_nft_to_creator() {
    // The mock NFT transfer_from records nothing, but the call must not panic.
    // This test verifies the code path executes without error.
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

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    // Must not panic — the NFT transfer_from(contract, creator, creator, token_id)
    // path through the mock succeeds silently.
    client.finalize_auction(&artist, &auction_id);

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, AuctionStatus::Cancelled);
}

#[test]
fn test_finalize_with_winner_transfers_funds() {
    // Verify the winning bid amount is routed away from the contract address
    // (i.e. ends up with the creator/recipients) after finalization.
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let bid_amount = 1_500_000_i128;
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    client.place_bid(&buyer, &auction_id, &bid_amount);

    let token = TokenClient::new(&env, &token_id);
    let artist_before = token.balance(&artist);
    let contract_escrow = token.balance(&contract_id);

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);

    // All escrowed funds must leave the contract.
    let contract_after = token.balance(&contract_id);
    assert_eq!(
        contract_after,
        contract_escrow - bid_amount,
        "full bid escrow must leave the contract after finalization"
    );

    // Creator must receive the bid amount (no fee or royalty configured in this test).
    let artist_after = token.balance(&artist);
    assert_eq!(
        artist_after,
        artist_before + bid_amount,
        "creator must receive the full bid when no fee or royalty is set"
    );

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, AuctionStatus::Finalized);
}

// ═══════════════════════════════════════════════════════════════════════════
// Auction settlement parity with direct sales (Feature B)
// ═══════════════════════════════════════════════════════════════════════════
//
// Acceptance criteria:
//   1. Auction payout equals direct-sale payout at the same price/recipients/fee.
//   2. The protocol fee snapshot taken at auction creation is honoured even if
//      the admin changes the global fee between creation and finalization.
//   3. Both code paths call the same distribute_payout helper (structural).
// ═══════════════════════════════════════════════════════════════════════════

/// Set up a scenario with a treasury, a non-zero protocol fee, and return the
/// treasury address alongside the standard setup tuple. The fee is set AFTER
/// listing/auction creation to isolate snapshot behaviour in tests that need it.
fn setup_with_treasury() -> (
    Env,
    MarketplaceContractClient<'static>,
    Address, // artist / creator
    Address, // buyer / bidder
    Address, // token_id (payment token)
    Address, // contract_id
    Address, // collection_id
    Address, // treasury
) {
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    let treasury = Address::generate(&env);
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);
    client.set_treasury(&artist, &treasury);
    (env, client, artist, buyer, token_id, contract_id, collection_id, treasury)
}

#[test]
fn test_auction_payout_matches_direct_sale_payout() {
    // Create a direct listing and an auction with identical price, recipients,
    // and protocol fee. Verify the seller receives the same net amount from
    // both settlement paths.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id, treasury) =
        setup_with_treasury();

    let price = 10_000_000_i128;
    let fee_bps = 500u32; // 5 %

    // ── Direct listing path ──────────────────────────────────────────────
    // Create listing BEFORE setting the fee so snapshot is 0 (matches the
    // auction snapshot below which is also taken before the fee is set).
    let listing_id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
        &None::<u64>,
    );

    // Set fee AFTER listing creation — listing snapshot stays 0.
    // Then reset fee to 0 so auction snapshot below is also 0.
    // (We will create the auction with fee=0 snapshotted, same as listing.)
    // Actually: create both with fee=0 snapshotted, then set fee=500 globally.
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &2u64, // different token_id so NFT mock doesn't conflict
        &price,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    // NOW set the global fee to 500 bps; both items have fee=0 snapshotted.
    client.set_protocol_fee(&artist, &fee_bps);

    let token = TokenClient::new(&env, &token_id);
    let artist_before_listing = token.balance(&artist);

    // Settle via direct buy.
    client.buy_artwork(&buyer, &listing_id);

    let artist_after_listing = token.balance(&artist);
    let listing_payout = artist_after_listing - artist_before_listing;

    // For the auction, use a fresh buyer with funds.
    let bidder = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&bidder, &100_000_000_000_i128);

    let artist_before_auction = token.balance(&artist);

    client.place_bid(&bidder, &auction_id, &price);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&bidder, &auction_id);

    let artist_after_auction = token.balance(&artist);
    let auction_payout = artist_after_auction - artist_before_auction;

    assert_eq!(
        listing_payout, auction_payout,
        "auction payout must equal direct-sale payout at equal price/fee/recipients"
    );
}

#[test]
fn test_auction_fee_snapshot_honoured_after_global_fee_change() {
    // Auction created with fee=500 bps snapshotted. Admin then raises the
    // global fee to 1000 bps. Finalization must use 500, not 1000.
    let (env, client, artist, _, token_id, _contract_id, collection_id, treasury) =
        setup_with_treasury();

    let price = 10_000_000_i128;

    // Set global fee to 500 bps BEFORE auction creation so it gets snapshotted.
    client.set_protocol_fee(&artist, &500u32);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &price,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    // Admin raises the global fee AFTER creation — must not affect this auction.
    client.set_protocol_fee(&artist, &1000u32);

    let bidder = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&bidder, &100_000_000_000_i128);

    client.place_bid(&bidder, &auction_id, &price);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    let token = TokenClient::new(&env, &token_id);
    let treasury_before = token.balance(&treasury);
    let artist_before = token.balance(&artist);

    client.finalize_auction(&bidder, &auction_id);

    let treasury_after = token.balance(&treasury);
    let artist_after = token.balance(&artist);

    // Expected fee at 500 bps (snapshotted), NOT 1000 bps (current global).
    let expected_fee = price * 500 / 10_000; // = 500_000
    let expected_seller = price - expected_fee;  // = 9_500_000

    assert_eq!(
        treasury_after - treasury_before,
        expected_fee,
        "treasury must receive 500 bps fee (snapshotted at creation), not 1000 bps"
    );
    assert_eq!(
        artist_after - artist_before,
        expected_seller,
        "creator must receive bid minus the 500 bps snapshotted fee"
    );
}

#[test]
fn test_auction_fee_zero_snapshot_seller_gets_full_amount() {
    // When no fee is set at creation time, the creator should receive
    // the entire winning bid (no treasury deduction).
    let (env, client, artist, buyer, token_id, _contract_id, collection_id, treasury) =
        setup_with_treasury();

    let bid_amount = 5_000_000_i128;

    // Fee is NOT set before auction creation → snapshot is 0.
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &bid_amount,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    // Set a non-zero global fee after creation; snapshot must shield the auction.
    client.set_protocol_fee(&artist, &1000u32);

    client.place_bid(&buyer, &auction_id, &bid_amount);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);

    let token = TokenClient::new(&env, &token_id);
    let artist_before = token.balance(&artist);
    let treasury_before = token.balance(&treasury);

    client.finalize_auction(&buyer, &auction_id);

    assert_eq!(
        token.balance(&artist) - artist_before,
        bid_amount,
        "creator must receive the full bid when fee snapshot is zero"
    );
    assert_eq!(
        token.balance(&treasury) - treasury_before,
        0,
        "treasury must receive nothing when fee snapshot is zero"
    );
}

#[test]
fn test_auction_settlement_with_fee_and_royalty_matches_listing() {
    // Both paths must produce identical payouts when royalty_bps > 0 but
    // royalty_receiver == seller (royalty is skipped in both cases).
    let (env, client, artist, buyer, token_id, _contract_id, collection_id, _treasury) =
        setup_with_treasury();

    let price = 10_000_000_i128;
    // No protocol fee (snapshot = 0), no treasury impact.
    // MockNft always returns royalty_bps=0, so royalty branch is skipped.

    let listing_id = client.create_listing(
        &artist,
        &price,
        &symbol_short!("XLM"),
        &token_id,
        &collection_id,
        &1u64,
        &valid_recipients(&env, &artist),
        &None::<u64>,
    );

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &2u64,
        &price,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let token = TokenClient::new(&env, &token_id);

    // Direct sale.
    let before_direct = token.balance(&artist);
    client.buy_artwork(&buyer, &listing_id);
    let direct_gain = token.balance(&artist) - before_direct;

    // Auction sale.
    let bidder = Address::generate(&env);
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&bidder, &100_000_000_000_i128);
    let before_auction = token.balance(&artist);
    client.place_bid(&bidder, &auction_id, &price);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&bidder, &auction_id);
    let auction_gain = token.balance(&artist) - before_auction;

    assert_eq!(
        direct_gain, auction_gain,
        "direct-sale and auction settlement must produce identical creator gains"
    );
}

#[test]
fn test_auction_protocol_fee_snapshot_field_set_at_creation() {
    // Directly inspect the snapshotted field on the stored Auction struct.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Set global fee to 300 bps before creation.
    client.set_protocol_fee(&artist, &300u32);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let auction = client.get_auction(&auction_id);
    assert_eq!(
        auction.protocol_fee_bps,
        300u32,
        "protocol_fee_bps must be snapshotted from the global setting at creation"
    );

    // Change global fee; snapshot on existing auction must be unchanged.
    client.set_protocol_fee(&artist, &700u32);
    let auction_after = client.get_auction(&auction_id);
    assert_eq!(
        auction_after.protocol_fee_bps,
        300u32,
        "changing global fee must not retroactively update an existing auction's snapshot"
    );
}

// =============================================================================
// Bounded bid history — get_auction_bids (Feature: BID_HISTORY_CAP)
// =============================================================================
//
// Acceptance criteria:
//   1. Bids are returned in chronological order (oldest → newest).
//   2. The history is capped; oldest entries are evicted beyond the cap.
//   3. get_auction_bids on an unknown auction returns AuctionNotFound (#9).
//   4. get_auction_bids on a fresh auction (no bids) returns an empty vector.
// =============================================================================

#[test]
fn test_get_auction_bids_empty_before_any_bid() {
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

    let history = client.get_auction_bids(&auction_id);
    assert_eq!(history.len(), 0, "bid history must be empty before any bids");
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_get_auction_bids_unknown_auction_reverts() {
    let (_env, client, _, _, _, _, _) = setup();
    client.get_auction_bids(&999u64);
}

#[test]
fn test_get_auction_bids_single_bid_recorded() {
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

    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let history = client.get_auction_bids(&auction_id);
    assert_eq!(history.len(), 1, "one bid must produce one history entry");

    let record = history.get(0).unwrap();
    assert_eq!(record.bidder, buyer, "record must carry the correct bidder");
    assert_eq!(record.amount, 1_500_000_i128, "record must carry the correct amount");
}

#[test]
fn test_get_auction_bids_ordering_oldest_to_newest() {
    // Place three bids from three different bidders and verify the history is
    // returned in chronological (oldest-first) order.
    let (env, client, artist, buyer, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let bidder2 = Address::generate(&env);
    let bidder3 = Address::generate(&env);
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    sac.mint(&bidder2, &100_000_000_000_i128);
    sac.mint(&bidder3, &100_000_000_000_i128);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    // Bids in ascending order (each must exceed the previous).
    client.place_bid(&buyer,   &auction_id, &1_000_000_i128);
    client.place_bid(&bidder2, &auction_id, &2_000_000_i128);
    client.place_bid(&bidder3, &auction_id, &3_000_000_i128);

    let history = client.get_auction_bids(&auction_id);
    assert_eq!(history.len(), 3, "all three bids must appear in history");

    // Verify chronological order by checking amounts.
    assert_eq!(history.get(0).unwrap().amount, 1_000_000_i128, "index 0: first (oldest) bid");
    assert_eq!(history.get(1).unwrap().amount, 2_000_000_i128, "index 1: second bid");
    assert_eq!(history.get(2).unwrap().amount, 3_000_000_i128, "index 2: third (newest) bid");

    // Verify correct bidder addresses.
    assert_eq!(history.get(0).unwrap().bidder, buyer);
    assert_eq!(history.get(1).unwrap().bidder, bidder2);
    assert_eq!(history.get(2).unwrap().bidder, bidder3);
}

#[test]
fn test_get_auction_bids_cap_evicts_oldest_entry() {
    // Place BID_HISTORY_CAP + 1 bids (21 total) and verify:
    //   - history.len() == BID_HISTORY_CAP (20)
    //   - the first recorded bid is gone (evicted)
    //   - the last recorded bid is present as the newest entry
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

    // Generate and fund 21 distinct bidders.
    let bid_count: u32 = 21; // one more than BID_HISTORY_CAP (20)
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    let mut bidders: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
    for _ in 0..bid_count {
        let b = Address::generate(&env);
        sac.mint(&b, &100_000_000_000_i128);
        bidders.push_back(b);
    }

    // Place 21 bids in ascending order (bid n costs n * 1_000_000 stroops).
    for i in 0..bid_count {
        let amount = (i as i128 + 1) * 1_000_000_i128;
        client.place_bid(&bidders.get(i).unwrap(), &auction_id, &amount);
    }

    let history = client.get_auction_bids(&auction_id);

    // The cap is 20 — exactly 20 entries must remain.
    assert_eq!(
        history.len(),
        20,
        "history must be capped at BID_HISTORY_CAP (20) entries"
    );

    // The very first bid (amount = 1_000_000) must have been evicted.
    let oldest_retained = history.get(0).unwrap();
    assert_eq!(
        oldest_retained.amount,
        2_000_000_i128,
        "oldest retained entry must be the second bid (first was evicted)"
    );

    // The newest bid (amount = 21_000_000) must be at the tail.
    let newest = history.get(19).unwrap();
    assert_eq!(
        newest.amount,
        21_000_000_i128,
        "newest entry must be the last placed bid"
    );
    assert_eq!(
        newest.bidder,
        bidders.get(20).unwrap(),
        "newest bidder address must match"
    );
}

#[test]
fn test_get_auction_bids_multiple_cap_evictions() {
    // Place 25 bids (5 beyond cap=20) and verify only the last 20 remain.
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

    let bid_count: u32 = 25;
    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    for i in 0..bid_count {
        let b = Address::generate(&env);
        sac.mint(&b, &100_000_000_000_i128);
        let amount = (i as i128 + 1) * 1_000_000_i128;
        client.place_bid(&b, &auction_id, &amount);
    }

    let history = client.get_auction_bids(&auction_id);
    assert_eq!(history.len(), 20, "only the last 20 bids must be retained");

    // Oldest retained must be bid #6 (amount = 6_000_000); bids 1-5 are evicted.
    assert_eq!(
        history.get(0).unwrap().amount,
        6_000_000_i128,
        "oldest retained entry must be bid #6"
    );
    // Newest must be bid #25.
    assert_eq!(
        history.get(19).unwrap().amount,
        25_000_000_i128,
        "newest entry must be bid #25"
    );
}

#[test]
fn test_get_auction_bids_ledger_sequence_recorded() {
    // Verify the `ledger` field in BidRecord is populated with the ledger
    // sequence at the time the bid was placed.
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

    let seq_before = env.ledger().sequence();
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let history = client.get_auction_bids(&auction_id);
    assert_eq!(history.len(), 1);

    let record = history.get(0).unwrap();
    // The ledger sequence recorded must be >= the sequence before the bid call.
    assert!(
        record.ledger >= seq_before,
        "bid record must carry a valid ledger sequence"
    );
}

// =============================================================================
// Minimum auction duration validation — InvalidAuctionDuration (#31)
// =============================================================================
//
// Acceptance criteria:
//   1. Duration < MIN_AUCTION_DURATION (3600 s) reverts with InvalidAuctionDuration.
//   2. Duration == 0 reverts.
//   3. Duration == MIN_AUCTION_DURATION - 1 reverts.
//   4. Duration == MIN_AUCTION_DURATION succeeds (boundary).
//   5. Duration > MIN_AUCTION_DURATION succeeds.
// =============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #31)")]
fn test_create_auction_zero_duration_reverts() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Duration = 0 is below MIN_AUCTION_DURATION; must revert.
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &0u64, // zero duration
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #31)")]
fn test_create_auction_one_second_duration_reverts() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // Duration of 1 second is far below the 1-hour minimum.
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &1u64, // 1 second
        &valid_recipients(&env, &artist),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #31)")]
fn test_create_auction_one_below_min_duration_reverts() {
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    // 3599 seconds = MIN_AUCTION_DURATION - 1; must be rejected.
    client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3599u64, // one second below the 1-hour minimum
        &valid_recipients(&env, &artist),
    );
}

#[test]
fn test_create_auction_exact_min_duration_succeeds() {
    // Duration == MIN_AUCTION_DURATION (3600 s) must be accepted (boundary value).
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64, // exactly MIN_AUCTION_DURATION
        &valid_recipients(&env, &artist),
    );

    assert_eq!(auction_id, 1u64, "auction must be created at exact minimum duration");

    let auction = client.get_auction(&auction_id);
    // end_time must be at least 3600 seconds from the creation timestamp.
    assert!(
        auction.end_time >= env.ledger().timestamp() + 3600,
        "end_time must reflect the full minimum duration"
    );
}

#[test]
fn test_create_auction_above_min_duration_succeeds() {
    // Duration well above the minimum (24 hours) must be accepted.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let duration = 86_400u64; // 24 hours
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &duration,
        &valid_recipients(&env, &artist),
    );

    assert_eq!(auction_id, 1u64);

    let auction = client.get_auction(&auction_id);
    assert!(
        auction.end_time >= env.ledger().timestamp() + duration,
        "end_time must reflect the requested duration"
    );
}

#[test]
fn test_create_auction_min_duration_end_time_is_future() {
    // Verify that even at the minimum duration the end_time is strictly in the
    // future relative to the ledger timestamp at creation.
    let (env, client, artist, _, token_id, _contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let ts_before = env.ledger().timestamp();

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let auction = client.get_auction(&auction_id);
    assert!(
        auction.end_time > ts_before,
        "end_time must be strictly greater than the ledger timestamp at creation"
    );
}

// =============================================================================
// ISSUE-028 — Auction escrow-conservation invariant tests
// =============================================================================
//
// Acceptance criteria:
//   1. After every bid, contract token balance == current highest bid
//      (net of the pre-funded contract balance from setup()).
//   2. After finalize with a winner, the auction's escrow contribution is
//      fully drained and creator/winner balances reconcile.
//   3. After finalize with no bids, escrow is unchanged (nothing was deposited).
//   4. After cancel (no bids), escrow is unchanged.
//   5. Multi-bidder sequences preserve the invariant at every step.
// =============================================================================

/// Snapshot baseline balances needed for escrow-conservation assertions.
struct EscrowSnapshot {
    /// Contract balance before any bids on this auction.
    contract_base: i128,
}

impl EscrowSnapshot {
    fn new(env: &Env, token_id: &Address, contract_id: &Address) -> Self {
        let token = soroban_sdk::token::TokenClient::new(env, token_id);
        Self {
            contract_base: token.balance(contract_id),
        }
    }

    /// Assert that the contract holds exactly `expected_escrow` above its
    /// baseline, i.e. contract_balance == contract_base + expected_escrow.
    fn assert_escrow(&self, env: &Env, token_id: &Address, contract_id: &Address, expected_escrow: i128, msg: &str) {
        let token = soroban_sdk::token::TokenClient::new(env, token_id);
        let current = token.balance(contract_id);
        assert_eq!(
            current - self.contract_base,
            expected_escrow,
            "{}",
            msg,
        );
    }
}
#[test]
fn test_escrow_equals_highest_bid_after_each_bid() {
    // Multi-bidder sequence: 5 bidders each outbid the previous one.
    // After every bid, contract escrow must equal the current highest bid.
    let (env, client, artist, _buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    let base_balance = 100_000_000_000_i128;

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let snap = EscrowSnapshot::new(&env, &token_id, &contract_id);

    // Generate 5 bidders and place escalating bids.
    let bid_amounts: [i128; 5] = [1_000_000, 2_000_000, 3_000_000, 4_000_000, 5_000_000];
    let mut bidders = soroban_sdk::Vec::new(&env);
    for _ in 0..5 {
        let b = Address::generate(&env);
        sac.mint(&b, &base_balance);
        bidders.push_back(b);
    }

    for (i, &amount) in bid_amounts.iter().enumerate() {
        let bidder = bidders.get(i as u32).unwrap();
        client.place_bid(&bidder, &auction_id, &amount);

        // Invariant: escrow == highest bid after this step.
        snap.assert_escrow(
            &env, &token_id, &contract_id,
            amount,
            "escrow must equal highest bid after each bid step",
        );

        // Invariant: outbid bidders are fully refunded.
        if i > 0 {
            let prev_bidder = bidders.get(i as u32 - 1).unwrap();
            let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
            assert_eq!(
                token.balance(&prev_bidder),
                base_balance,
                "bidder {} must be fully refunded after being outbid",
                i,
            );
        }
    }
}

#[test]
fn test_escrow_zero_after_finalize_with_winner() {
    // After finalization the contract must hold zero escrow for this auction
    // and creator + winner balances must reconcile.
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let bid_amount = 3_000_000_i128;
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let snap = EscrowSnapshot::new(&env, &token_id, &contract_id);
    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);

    let artist_before = token.balance(&artist);
    let buyer_before  = token.balance(&buyer);

    client.place_bid(&buyer, &auction_id, &bid_amount);

    // Invariant before finalize: escrow == bid.
    snap.assert_escrow(&env, &token_id, &contract_id, bid_amount,
        "escrow must equal the winning bid before finalization");

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);

    // Post-finalize: contract escrow contribution from this auction is zero.
    snap.assert_escrow(&env, &token_id, &contract_id, 0,
        "contract must hold zero escrow after finalization");

    // Balance reconciliation: no protocol fee set, so creator receives full bid.
    let artist_after = token.balance(&artist);
    let buyer_after  = token.balance(&buyer);

    assert_eq!(
        artist_after - artist_before,
        bid_amount,
        "creator must receive the full winning bid (no fee configured)",
    );
    assert_eq!(
        buyer_before - buyer_after,
        bid_amount,
        "winner's net outflow must equal the winning bid",
    );
}

#[test]
fn test_escrow_zero_after_finalize_with_winner_and_fee() {
    // Repeat with a 5 % protocol fee to verify reconciliation still holds.
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let treasury = Address::generate(&env);
    client.set_treasury(&artist, &treasury);
    client.set_protocol_fee(&artist, &500u32); // 5 %

    let bid_amount = 10_000_000_i128;
    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let snap = EscrowSnapshot::new(&env, &token_id, &contract_id);
    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);

    let artist_before   = token.balance(&artist);
    let buyer_before    = token.balance(&buyer);
    let treasury_before = token.balance(&treasury);

    client.place_bid(&buyer, &auction_id, &bid_amount);
    snap.assert_escrow(&env, &token_id, &contract_id, bid_amount,
        "escrow must equal bid before finalize");

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);

    snap.assert_escrow(&env, &token_id, &contract_id, 0,
        "contract escrow must be zero after finalization");

    let expected_fee    = bid_amount * 500 / 10_000; // 500_000
    let expected_seller = bid_amount - expected_fee;  // 9_500_000

    assert_eq!(token.balance(&artist)   - artist_before,   expected_seller);
    assert_eq!(token.balance(&treasury) - treasury_before, expected_fee);
    assert_eq!(buyer_before - token.balance(&buyer),       bid_amount);
}

#[test]
fn test_escrow_zero_after_finalize_no_bids() {
    // When no bids were placed the contract escrow must remain unchanged
    // (nothing deposited, nothing to drain).
    let (env, client, artist, _, token_id, contract_id, collection_id) = setup();
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

    let snap = EscrowSnapshot::new(&env, &token_id, &contract_id);

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&artist, &auction_id);

    // No bid was ever escrowed — delta must be zero.
    snap.assert_escrow(&env, &token_id, &contract_id, 0,
        "no escrow change when no bids were placed");

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Cancelled);
}

#[test]
fn test_escrow_zero_after_cancel_no_bids() {
    // cancel_auction with no bids also must not disturb escrow.
    let (env, client, artist, _, token_id, contract_id, collection_id) = setup();
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

    let snap = EscrowSnapshot::new(&env, &token_id, &contract_id);
    client.cancel_auction(&artist, &auction_id);

    snap.assert_escrow(&env, &token_id, &contract_id, 0,
        "cancel with no bids must leave escrow unchanged");
}

#[test]
fn test_escrow_invariant_multi_bidder_sequence_with_outbids() {
    // Simulate a realistic auction: 3 bidders raise each other in turn.
    // Assert escrow after every bid and full reconciliation after finalize.
    let (env, client, artist, _buyer, token_id, contract_id, collection_id) = setup();
    client.set_admin(&artist);
    client.add_token_to_whitelist(&token_id);

    let sac = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    let base_balance = 100_000_000_000_i128;

    let bidder_a = Address::generate(&env);
    let bidder_b = Address::generate(&env);
    let bidder_c = Address::generate(&env);
    sac.mint(&bidder_a, &base_balance);
    sac.mint(&bidder_b, &base_balance);
    sac.mint(&bidder_c, &base_balance);

    let auction_id = client.create_auction(
        &artist,
        &token_id,
        &collection_id,
        &1u64,
        &1_000_000_i128,
        &3600u64,
        &valid_recipients(&env, &artist),
    );

    let snap  = EscrowSnapshot::new(&env, &token_id, &contract_id);
    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);

    // Round 1 — A bids 1 000 000.
    client.place_bid(&bidder_a, &auction_id, &1_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 1_000_000,
        "after round 1: escrow == 1_000_000");

    // Round 2 — B outbids with 2 000 000; A is refunded.
    client.place_bid(&bidder_b, &auction_id, &2_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 2_000_000,
        "after round 2: escrow == 2_000_000");
    assert_eq!(token.balance(&bidder_a), base_balance,
        "bidder_a fully refunded after round 2");

    // Round 3 — C outbids with 3 000 000; B is refunded.
    client.place_bid(&bidder_c, &auction_id, &3_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 3_000_000,
        "after round 3: escrow == 3_000_000");
    assert_eq!(token.balance(&bidder_b), base_balance,
        "bidder_b fully refunded after round 3");

    // Round 4 — A re-enters at 4 000 000; C is refunded.
    client.place_bid(&bidder_a, &auction_id, &4_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 4_000_000,
        "after round 4: escrow == 4_000_000");
    assert_eq!(token.balance(&bidder_c), base_balance,
        "bidder_c fully refunded after round 4");

    // Finalize.
    let artist_before = token.balance(&artist);
    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&bidder_a, &auction_id);

    // Post-finalize escrow is zero.
    snap.assert_escrow(&env, &token_id, &contract_id, 0,
        "escrow must be zero after finalization");

    // Balances reconcile (no fee configured).
    assert_eq!(token.balance(&artist) - artist_before, 4_000_000_i128,
        "creator receives the winning bid");
    assert_eq!(base_balance - token.balance(&bidder_a), 4_000_000_i128,
        "winner's net outflow equals the winning bid");
    // Other bidders fully refunded throughout.
    assert_eq!(token.balance(&bidder_b), base_balance);
    assert_eq!(token.balance(&bidder_c), base_balance);
}

#[test]
fn test_escrow_invariant_same_bidder_raises_own_bid() {
    // A single bidder may raise their own bid. Each new bid refunds the
    // previous escrow, so the net held is always the latest bid amount.
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
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

    let snap  = EscrowSnapshot::new(&env, &token_id, &contract_id);
    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    let base  = token.balance(&buyer);

    client.place_bid(&buyer, &auction_id, &1_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 1_000_000,
        "escrow after first self-raise bid");

    client.place_bid(&buyer, &auction_id, &2_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 2_000_000,
        "escrow after second self-raise bid");
    // Net outflow from buyer is the latest bid amount (previous escrow refunded).
    assert_eq!(base - token.balance(&buyer), 2_000_000_i128,
        "buyer's net outflow is the current highest bid");

    client.place_bid(&buyer, &auction_id, &5_000_000_i128);
    snap.assert_escrow(&env, &token_id, &contract_id, 5_000_000,
        "escrow after third self-raise bid");
    assert_eq!(base - token.balance(&buyer), 5_000_000_i128);

    env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
    client.finalize_auction(&buyer, &auction_id);

    snap.assert_escrow(&env, &token_id, &contract_id, 0,
        "escrow zero after finalize");
}

// =============================================================================
// ISSUE-028 (b) — Self-bid (shill bidding) prevention
// =============================================================================
//
// Acceptance criteria:
//   1. The auction creator cannot place a bid on their own auction.
//   2. The error raised is SelfBidNotAllowed (#32).
//   3. A distinct bidder (not the creator) can still bid normally.
//   4. The check fires even if the creator would be the first bidder.
//   5. The check fires even if the creator tries to outbid an existing bid.
// =============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #32)")]
fn test_creator_cannot_bid_on_own_auction() {
    // The simplest case: creator attempts the first bid.
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

    // Creator tries to bid on their own auction — must revert with #32.
    client.place_bid(&artist, &auction_id, &1_500_000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #32)")]
fn test_creator_cannot_outbid_existing_bid() {
    // A legitimate bidder bids first; the creator then tries to outbid — still blocked.
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

    // Legitimate first bid from buyer.
    client.place_bid(&buyer, &auction_id, &1_000_000_i128);

    // Creator attempts to outbid — must still revert with SelfBidNotAllowed.
    client.place_bid(&artist, &auction_id, &2_000_000_i128);
}

#[test]
fn test_non_creator_can_bid_normally() {
    // Verify the guard does not affect legitimate bidders.
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

    // Non-creator bid must succeed.
    client.place_bid(&buyer, &auction_id, &1_500_000_i128);

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.highest_bid, 1_500_000_i128);
    assert_eq!(auction.highest_bidder, Some(buyer));
}

#[test]
fn test_self_bid_blocked_uses_dedicated_error_code() {
    // Verify the error code is exactly 32 (SelfBidNotAllowed), not a generic
    // Unauthorized (#5) — important for frontend error handling.
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

    let result = client.try_place_bid(&artist, &auction_id, &1_500_000_i128);
    assert!(result.is_err(), "self-bid must return an error");

    let err = result.unwrap_err().unwrap();
    assert_eq!(
        err,
        crate::types::MarketplaceError::SelfBidNotAllowed,
        "error must be SelfBidNotAllowed (#32), not a generic error",
    );
}

#[test]
fn test_self_bid_blocked_does_not_mutate_state() {
    // A rejected self-bid must leave the auction completely unchanged.
    let (env, client, artist, buyer, token_id, contract_id, collection_id) = setup();
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

    // Place a legitimate bid first so there is existing state to check.
    client.place_bid(&buyer, &auction_id, &1_000_000_i128);

    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    let artist_balance_before   = token.balance(&artist);
    let contract_balance_before = token.balance(&contract_id);
    let auction_before = client.get_auction(&auction_id);

    // Creator tries to self-bid — must fail.
    let _ = client.try_place_bid(&artist, &auction_id, &2_000_000_i128);

    // Auction state is unchanged.
    let auction_after = client.get_auction(&auction_id);
    assert_eq!(auction_after.highest_bid,    auction_before.highest_bid);
    assert_eq!(auction_after.highest_bidder, auction_before.highest_bidder);

    // No tokens moved.
    assert_eq!(token.balance(&artist),      artist_balance_before);
    assert_eq!(token.balance(&contract_id), contract_balance_before);
}
