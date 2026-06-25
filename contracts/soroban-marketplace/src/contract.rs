// ------------------------------------------------------------
// contract.rs — ELCARE-HUB Marketplace contract implementation
// ------------------------------------------------------------

#[allow(unused_imports)]
use soroban_sdk::{
    contract, contractimpl, log, panic_with_error, token::Client as TokenClient, Address, Bytes,
    Env, IntoVal, Symbol, Vec,
};

use crate::events::*;

use crate::{
    storage::{
        acquire_auction_lock, acquire_listing_lock, add_artist_auction_id, add_artist_listing_id,
        add_to_active_listings, clear_pending_admin_storage, get_active_listing_ids,
        get_artist_auction_ids, get_artist_listing_ids, get_auction_count, get_listing_count,
        get_pending_admin_storage, increment_auction_count, increment_listing_count,
        increment_offer_count, is_artist_revoked_storage, load_auction, load_listing,
        load_listing_offers, load_offer, load_offerer_offers, release_auction_lock,
        release_listing_lock, remove_artist_revocation_storage, remove_from_active_listings,
        save_auction, save_listing, save_listing_offers, save_offer, save_offerer_offers,
        set_artist_revocation_storage, set_pending_admin_storage,
    },
    types::{
        Auction, AuctionStatus, CancelReason, Listing, ListingStatus, MarketplaceError, Offer,
        OfferStatus, Recipient,
    },
};

#[contract]
pub struct MarketplaceContract;

#[contractimpl]
impl MarketplaceContract {
    // ── Admin & Global Configuration ───────────────────────

    pub fn set_admin(env: Env, admin: Address) {
        let key = crate::storage::DataKey::Admin;
        if env.storage().persistent().get::<_, Address>(&key).is_some() {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        admin.require_auth();
        env.storage().persistent().set(&key, &admin);
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        let key = crate::storage::DataKey::Admin;
        env.storage().persistent().get::<_, Address>(&key)
    }

    /// Step 1 of a 2-step admin transfer: the current admin proposes a successor.
    /// The successor must call `accept_admin` to complete the handover.
    pub fn transfer_admin(env: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        let stored_admin = Self::get_admin(env.clone())
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::Unauthorized));
        if current_admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        set_pending_admin_storage(&env, &new_admin);
        crate::events::AdminTransferProposedEvent {
            current_admin,
            proposed_admin: new_admin,
        }
        .publish(&env);
    }

    /// Step 2 of a 2-step admin transfer: the proposed new admin accepts the role.
    pub fn accept_admin(env: Env, new_admin: Address) {
        new_admin.require_auth();
        let pending = get_pending_admin_storage(&env)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::Unauthorized));
        if new_admin != pending {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        let old_admin = Self::get_admin(env.clone())
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::Unauthorized));
        let key = crate::storage::DataKey::Admin;
        env.storage().persistent().set(&key, &new_admin);
        env.storage().persistent().extend_ttl(
            &key,
            crate::storage::LEDGER_TTL_THRESHOLD,
            crate::storage::LEDGER_TTL_BUMP,
        );
        clear_pending_admin_storage(&env);
        crate::events::AdminTransferredEvent {
            old_admin,
            new_admin,
        }
        .publish(&env);
    }

    pub fn set_treasury(env: Env, admin: Address, treasury: Address) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        crate::storage::set_treasury_storage(&env, &treasury);
    }

    pub fn get_treasury(env: Env) -> Option<Address> {
        crate::storage::get_treasury_storage(&env)
    }

    pub fn set_protocol_fee(env: Env, admin: Address, bps: u32) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if bps > 1000 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }
        crate::storage::set_protocol_fee_bps_storage(&env, bps);
    }

    pub fn get_protocol_fee(env: Env) -> u32 {
        crate::storage::get_protocol_fee_bps_storage(&env).unwrap_or(0)
    }

    // ── Pause/Unpause Mechanism ────────────────────────────

    pub fn admin_pause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        crate::storage::set_paused(&env, true);
        #[allow(deprecated)]
        env.events().publish((crate::events::CONTRACT_PAUSED,), ());
    }

    pub fn admin_unpause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        crate::storage::set_paused(&env, false);
        #[allow(deprecated)]
        env.events()
            .publish((crate::events::CONTRACT_UNPAUSED,), ());
    }

    pub fn is_paused(env: Env) -> bool {
        crate::storage::is_paused(&env)
    }

    // ── Artist Moderation ───────────────────────────────────

    pub fn revoke_artist(env: Env, artist: Address) {
        Self::require_admin(&env);
        set_artist_revocation_storage(&env, &artist);
        #[allow(deprecated)]
        env.events()
            .publish((crate::events::ARTIST_REVOKED,), artist);
    }

    pub fn reinstate_artist(env: Env, artist: Address) {
        Self::require_admin(&env);
        remove_artist_revocation_storage(&env, &artist);
        #[allow(deprecated)]
        env.events()
            .publish((crate::events::ARTIST_REINSTATED,), artist);
    }

    pub fn is_artist_revoked(env: Env, artist: Address) -> bool {
        is_artist_revoked_storage(&env, &artist)
    }

    /// Cancel all active listings for a revoked artist.
    /// Called by admin after revoking an artist to clean up their active listings.
    /// Emits ListingCancelledEvent with reason=AdminRevoked for each cancelled listing.
    pub fn cancel_artist_listings(env: Env, admin: Address, artist: Address) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }

        // Only cancel listings if the artist is actually revoked
        if !is_artist_revoked_storage(&env, &artist) {
            return;
        }

        let listing_ids = get_artist_listing_ids(&env, &artist);
        for listing_id in listing_ids.iter() {
            if let Some(mut listing) = load_listing(&env, listing_id) {
                if listing.status == ListingStatus::Active {
                    // Refund all pending offers
                    let offers = load_listing_offers(&env, listing_id);
                    for offer_id in offers.iter() {
                        if let Some(mut offer) = load_offer(&env, offer_id) {
                            if offer.status == OfferStatus::Pending {
                                TokenClient::new(&env, &offer.token).transfer(
                                    &env.current_contract_address(),
                                    &offer.offerer,
                                    &offer.amount,
                                );
                                offer.status = OfferStatus::Rejected;
                                save_offer(&env, &offer);
                            }
                        }
                    }

                    listing.status = ListingStatus::Cancelled;
                    save_listing(&env, &listing);
                    remove_from_active_listings(&env, listing_id);

                    ListingCancelledEvent {
                        listing_id,
                        cancelled_by: admin.clone(),
                        reason: CancelReason::AdminRevoked,
                        ledger_sequence: env.ledger().sequence(),
                    }
                    .publish(&env);
                }
            }
        }
    }

    // ── Token Whitelist ─────────────────────────────────────

    pub fn add_token_to_whitelist(env: Env, token: Address) {
        Self::require_admin(&env);
        let key = crate::storage::DataKey::TokenWhitelist;
        let mut whitelist = env
            .storage()
            .persistent()
            .get::<_, Vec<Address>>(&key)
            .unwrap_or(Vec::new(&env));
        if !whitelist.contains(&token) {
            whitelist.push_back(token);
            env.storage().persistent().set(&key, &whitelist);
        }
    }

    pub fn remove_token_from_whitelist(env: Env, token: Address) {
        Self::require_admin(&env);
        let key = crate::storage::DataKey::TokenWhitelist;
        let whitelist = env
            .storage()
            .persistent()
            .get::<_, Vec<Address>>(&key)
            .unwrap_or(Vec::new(&env));
        let mut new_whitelist = Vec::new(&env);
        for t in whitelist.iter() {
            if t != token {
                new_whitelist.push_back(t.clone());
            }
        }
        env.storage().persistent().set(&key, &new_whitelist);
    }

    pub fn get_token_whitelist(env: Env) -> Vec<Address> {
        let key = crate::storage::DataKey::TokenWhitelist;
        env.storage()
            .persistent()
            .get::<_, Vec<Address>>(&key)
            .unwrap_or(Vec::new(&env))
    }

    // ── Listing methods ──────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn create_listing(
        env: Env,
        artist: Address,
        price: i128,
        currency: Symbol,
        token: Address,
        collection: Address,
        token_id: u64,
        recipients: Vec<Recipient>,
    ) -> u64 {
        Self::require_not_paused(&env);
        artist.require_auth();
        if Self::is_artist_revoked(env.clone(), artist.clone()) {
            panic_with_error!(&env, MarketplaceError::ArtistRevoked);
        }
        if price <= 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }

        let recipients_len = recipients.len();
        // Empty recipient arrays are an invalid split configuration; reject with InvalidSplit.
        if recipients_len == 0 {
            panic_with_error!(&env, MarketplaceError::InvalidSplit);
        }
        if recipients_len > 4 {
            panic_with_error!(&env, MarketplaceError::TooManyRecipients);
        }

        // Snapshot the current protocol fee so the combined bps can be validated
        // and the listing's economic terms are fixed at creation time.
        let protocol_fee_bps =
            crate::storage::get_protocol_fee_bps_storage(&env).unwrap_or(0);

        // Reject if sum(recipient bps) + protocol_fee_bps > 10 000.
        // This must happen before persisting the listing so an invalid split
        // is never observable in the indexer or UI.
        Self::validate_recipients(&env, &recipients, protocol_fee_bps);

        if !Self::is_token_whitelisted(&env, &token) {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }

        let listing_id = increment_listing_count(&env);
        let listing = Listing {
            listing_id,
            artist: artist.clone(),
            price,
            currency,
            token,
            collection,
            token_id,
            recipients,
            status: ListingStatus::Active,
            owner: None,
            created_at: env.ledger().sequence(),
            protocol_fee_bps, // Snapshot the fee at creation time
        };
        save_listing(&env, &listing);
        add_artist_listing_id(&env, &artist, listing_id);
        add_to_active_listings(&env, listing_id);

        ListingCreatedEvent {
            listing_id,
            artist: artist.clone(),
            price,
            currency: listing.currency.clone(),
            collection: listing.collection.clone(),
            token_id: listing.token_id,
            ledger_sequence: env.ledger().sequence(),
        }
        .publish(&env);
        listing_id
    }

    pub fn update_listing(
        env: Env,
        artist: Address,
        listing_id: u64,
        new_price: i128,
        new_token: Address,
        new_recipients: Vec<Recipient>,
    ) -> bool {
        Self::require_not_paused(&env);
        artist.require_auth();
        let mut listing = load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));
        if listing.artist != artist {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if listing.status != ListingStatus::Active {
            panic_with_error!(&env, MarketplaceError::ListingNotActive);
        }

        let offers = load_listing_offers(&env, listing_id);
        for offer_id in offers.iter() {
            if let Some(offer) = load_offer(&env, offer_id) {
                if offer.status == OfferStatus::Pending {
                    panic_with_error!(&env, MarketplaceError::Unauthorized);
                }
            }
        }

        if new_price <= 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }
        if !Self::is_token_whitelisted(&env, &new_token) {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }

        let new_recipients_len = new_recipients.len();
        if new_recipients_len == 0 {
            panic_with_error!(&env, MarketplaceError::InvalidSplit);
        }
        if new_recipients_len > 4 {
            panic_with_error!(&env, MarketplaceError::TooManyRecipients);
        }

        // Validate combined bps using the listing's snapshotted protocol fee
        // (not the current global fee) so the listing remains internally consistent.
        Self::validate_recipients(&env, &new_recipients, listing.protocol_fee_bps);

        listing.price = new_price;
        listing.token = new_token;
        listing.recipients = new_recipients;
        // NOTE: listing.protocol_fee_bps remains unchanged — it was snapshotted at creation

        save_listing(&env, &listing);

        ListingUpdatedEvent {
            listing_id,
            artist: artist.clone(),
            new_price,
            collection: listing.collection.clone(),
            token_id: listing.token_id,
            ledger_sequence: env.ledger().sequence(),
        }
        .publish(&env);

        true
    }

    pub fn buy_artwork(env: Env, buyer: Address, listing_id: u64) -> bool {
        // ─────────────────────────────────────────────────────────────────────
        // CHECKS-EFFECTS-INTERACTIONS ordering is strictly enforced here:
        //   1. Acquire reentrancy lock (earliest possible).
        //   2. Validate all inputs / listing state (Checks).
        //   3. Mutate storage — mark listing Sold, update owner, remove from
        //      active set, mark pending offers Rejected (Effects).
        //   4. Emit events (Effects — Soroban events are append-only and safe).
        //   5. Execute all external token transfers and NFT transfer (Interactions).
        //   6. Release lock only after all state is finalized.
        //
        // A malicious token that tries to re-enter buy_artwork for the same
        // listing_id will either find the lock already held (→ ReentrancyGuard)
        // or find the listing status already Sold (→ ListingSold), in both cases
        // reverting without double-spending.
        // ─────────────────────────────────────────────────────────────────────
        Self::require_not_paused(&env);
        buyer.require_auth();

        // Reentrancy guard
        if !acquire_listing_lock(&env, listing_id) {
            panic_with_error!(&env, MarketplaceError::ReentrancyGuard);
        }

        let mut listing = match load_listing(&env, listing_id) {
            Some(l) => l,
            None => {
                release_listing_lock(&env, listing_id);
                panic_with_error!(&env, MarketplaceError::ListingNotFound);
            }
        };

        // Status checks
        if listing.status == ListingStatus::Sold {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::ListingSold);
        }
        if listing.status == ListingStatus::Cancelled {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::ListingCancelled);
        }
        if listing.status != ListingStatus::Active {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::ListingNotActive);
        }
        if listing.artist == buyer {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::CannotBuyOwnListing);
        }

        // Ensure token is still whitelisted at purchase time. If it was removed after listing creation, block the purchase.
        if !Self::is_token_whitelisted(&env, &listing.token) {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::TokenNotWhitelisted);
        }

        // ── CHECKS-EFFECTS-INTERACTIONS ──────────────────────────────────────
        // All validation is complete.  Mutate state BEFORE any cross-contract
        // token call so that a reentrant buy_artwork on the same listing_id
        // finds the listing already marked Sold and is rejected by the lock or
        // by the status check, not by an inconsistent intermediate state.
        listing.status = ListingStatus::Sold;
        listing.owner = Some(buyer.clone());
        save_listing(&env, &listing);
        remove_from_active_listings(&env, listing_id);

        // Reject all pending offers (state mutation only — token refunds happen
        // in the interactions phase below).
        let offers = load_listing_offers(&env, listing_id);
        let mut pending_offerers: Vec<Address> = Vec::new(&env);
        let mut pending_amounts: Vec<i128> = Vec::new(&env);
        let mut pending_tokens: Vec<Address> = Vec::new(&env);
        for offer_id in offers.iter() {
            if let Some(mut offer) = load_offer(&env, offer_id) {
                if offer.status == OfferStatus::Pending {
                    offer.status = OfferStatus::Rejected;
                    save_offer(&env, &offer);
                    pending_offerers.push_back(offer.offerer.clone());
                    pending_amounts.push_back(offer.amount);
                    pending_tokens.push_back(offer.token.clone());
                }
            }
        }

        ArtworkSoldEvent {
            listing_id,
            artist: listing.artist.clone(),
            buyer: buyer.clone(),
            price: listing.price,
            currency: listing.currency.clone(),
            ledger_sequence: env.ledger().sequence(),
        }
        .publish(&env);

        // ── INTERACTIONS (external calls after all state is final) ───────────

        // Use the snapshotted protocol fee from the listing, not the current global fee
        Self::distribute_payout(
            &env,
            &listing.token,
            &listing.collection,
            listing.price,
            &listing.artist,
            &listing.recipients,
            &buyer,
            true,
            listing.protocol_fee_bps, // Use snapshotted fee
        );

        // Transfer the NFT
        env.invoke_contract::<()>(
            &listing.collection,
            &soroban_sdk::Symbol::new(&env, "transfer_from"),
            soroban_sdk::vec![
                &env,
                env.current_contract_address().into_val(&env),
                listing.artist.into_val(&env),
                buyer.into_val(&env),
                listing.token_id.into_val(&env)
            ],
        );

        // Refund rejected offer escrows
        for i in 0..pending_offerers.len() {
            TokenClient::new(&env, &pending_tokens.get(i).unwrap()).transfer(
                &env.current_contract_address(),
                &pending_offerers.get(i).unwrap(),
                &pending_amounts.get(i).unwrap(),
            );
        }

        release_listing_lock(&env, listing_id);
        true
    }

    pub fn cancel_listing(env: Env, artist: Address, listing_id: u64) -> bool {
        Self::require_not_paused(&env);
        artist.require_auth();
        let mut listing = load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));
        if listing.artist != artist {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if listing.status != ListingStatus::Active {
            panic_with_error!(&env, MarketplaceError::ListingNotActive);
        }

        let offers = load_listing_offers(&env, listing_id);
        for offer_id in offers.iter() {
            if let Some(mut offer) = load_offer(&env, offer_id) {
                if offer.status == OfferStatus::Pending {
                    TokenClient::new(&env, &offer.token).transfer(
                        &env.current_contract_address(),
                        &offer.offerer,
                        &offer.amount,
                    );
                    offer.status = OfferStatus::Rejected;
                    save_offer(&env, &offer);
                }
            }
        }

        listing.status = ListingStatus::Cancelled;
        save_listing(&env, &listing);
        remove_from_active_listings(&env, listing_id);

        ListingCancelledEvent {
            listing_id,
            cancelled_by: artist.clone(),
            reason: crate::types::CancelReason::Owner,
            ledger_sequence: env.ledger().sequence(),
        }
        .publish(&env);

        true
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_auction(
        env: Env,
        creator: Address,
        token: Address,
        collection: Address,
        token_id: u64,
        reserve_price: i128,
        duration: u64,
        recipients: Vec<Recipient>,
    ) -> u64 {
        Self::require_not_paused(&env);
        creator.require_auth();
        if Self::is_artist_revoked(env.clone(), creator.clone()) {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if reserve_price <= 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }
        if !Self::is_token_whitelisted(&env, &token) {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        let auction_id = increment_auction_count(&env);
        let end_time = env.ledger().timestamp() + duration;
        let auction = Auction {
            auction_id,
            creator: creator.clone(),
            token: token.clone(),
            collection: collection.clone(),
            token_id,
            reserve_price,
            highest_bid: 0,
            highest_bidder: None,
            end_time,
            status: AuctionStatus::Active,
            recipients,
        };
        save_auction(&env, &auction);
        add_artist_auction_id(&env, &creator, auction_id);

        AuctionCreatedEvent {
            auction_id,
            creator: creator.clone(),
            reserve_price,
            token,
            collection,
            token_id,
            end_time,
        }
        .publish(&env);

        auction_id
    }

    pub fn place_bid(env: Env, bidder: Address, auction_id: u64, amount: i128) {
        Self::require_not_paused(&env);
        bidder.require_auth();
        let mut auction = load_auction(&env, auction_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::AuctionNotFound));
        if auction.status != AuctionStatus::Active {
            panic_with_error!(&env, MarketplaceError::AuctionNotActive);
        }
        if env.ledger().timestamp() >= auction.end_time {
            panic_with_error!(&env, MarketplaceError::AuctionExpired);
        }
        if amount <= auction.highest_bid || amount < auction.reserve_price {
            panic_with_error!(&env, MarketplaceError::BidTooLow);
        }

        let token_client = TokenClient::new(&env, &auction.token);
        if let Some(prev) = auction.highest_bidder.clone() {
            token_client.transfer(&env.current_contract_address(), &prev, &auction.highest_bid);
        }
        token_client.transfer(&bidder, &env.current_contract_address(), &amount);
        auction.highest_bid = amount;
        auction.highest_bidder = Some(bidder.clone());
        save_auction(&env, &auction);

        BidPlacedEvent {
            auction_id,
            bidder: bidder.clone(),
            bid_amount: amount,
        }
        .publish(&env);
    }

    pub fn finalize_auction(env: Env, caller: Address, auction_id: u64) {
        Self::require_not_paused(&env);
        caller.require_auth();

        // Reentrancy guard
        if !acquire_auction_lock(&env, auction_id) {
            panic_with_error!(&env, MarketplaceError::ReentrancyGuard);
        }

        let mut auction = match load_auction(&env, auction_id) {
            Some(a) => a,
            None => {
                release_auction_lock(&env, auction_id);
                panic_with_error!(&env, MarketplaceError::AuctionNotFound);
            }
        };

        // Status check
        if auction.status != AuctionStatus::Active {
            release_auction_lock(&env, auction_id);
            panic_with_error!(&env, MarketplaceError::AuctionAlreadyFinalized);
        }

        // Time check
        if env.ledger().timestamp() < auction.end_time {
            if caller != auction.creator {
                release_auction_lock(&env, auction_id);
                panic_with_error!(&env, MarketplaceError::Unauthorized);
            }
        }

        let (finalized_winner, finalized_amount) =
            if let Some(ref winner) = auction.highest_bidder.clone() {
                // Auctions use the live global protocol fee at finalization time.
                // (Auctions are not listings and do not snapshot the fee at creation.)
                let auction_fee_bps =
                    crate::storage::get_protocol_fee_bps_storage(&env).unwrap_or(0);
                Self::distribute_payout(
                    &env,
                    &auction.token,
                    &auction.collection,
                    auction.highest_bid,
                    &auction.creator,
                    &auction.recipients,
                    winner,
                    false,
                    auction_fee_bps,
                );

                // Transfer the NFT
                env.invoke_contract::<()>(
                    &auction.collection,
                    &soroban_sdk::Symbol::new(&env, "transfer_from"),
                    soroban_sdk::vec![
                        &env,
                        env.current_contract_address().into_val(&env),
                        auction.creator.into_val(&env),
                        winner.into_val(&env),
                        auction.token_id.into_val(&env)
                    ],
                );

                auction.status = AuctionStatus::Finalized;
                (Some(winner.clone()), auction.highest_bid)
            } else {
                auction.status = AuctionStatus::Cancelled;
                (None, 0)
            };

        save_auction(&env, &auction);
        release_auction_lock(&env, auction_id);

        AuctionFinalizedEvent {
            auction_id,
            winner: finalized_winner,
            amount: finalized_amount,
        }
        .publish(&env);
    }

    pub fn make_offer(
        env: Env,
        offerer: Address,
        listing_id: u64,
        amount: i128,
        token: Address,
    ) -> u64 {
        Self::require_not_paused(&env);
        offerer.require_auth();
        let listing = load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));
        if listing.status != ListingStatus::Active {
            panic_with_error!(&env, MarketplaceError::ListingNotActive);
        }
        if listing.artist == offerer {
            panic_with_error!(&env, MarketplaceError::CannotOfferOwnListing);
        }
        if amount <= 0 {
            panic_with_error!(&env, MarketplaceError::InsufficientOfferAmount);
        }
        TokenClient::new(&env, &token).transfer(&offerer, &env.current_contract_address(), &amount);
        let offer_id = increment_offer_count(&env);
        save_offer(
            &env,
            &Offer {
                offer_id,
                listing_id,
                offerer: offerer.clone(),
                amount,
                token: token.clone(),
                status: OfferStatus::Pending,
                created_at: env.ledger().sequence(),
            },
        );
        let mut lo = load_listing_offers(&env, listing_id);
        lo.push_back(offer_id);
        save_listing_offers(&env, listing_id, &lo);
        let mut oo = load_offerer_offers(&env, &offerer);
        oo.push_back(offer_id);
        save_offerer_offers(&env, &offerer, &oo);

        OfferMadeEvent {
            offer_id,
            listing_id,
            offerer: offerer.clone(),
            amount,
            token,
        }
        .publish(&env);

        offer_id
    }

    pub fn withdraw_offer(env: Env, offerer: Address, offer_id: u64) {
        Self::require_not_paused(&env);
        offerer.require_auth();
        let mut offer = load_offer(&env, offer_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::OfferNotFound));
        if offer.offerer != offerer {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if offer.status != OfferStatus::Pending {
            panic_with_error!(&env, MarketplaceError::OfferNotPending);
        }
        TokenClient::new(&env, &offer.token).transfer(
            &env.current_contract_address(),
            &offerer,
            &offer.amount,
        );
        offer.status = OfferStatus::Withdrawn;
        save_offer(&env, &offer);

        OfferWithdrawnEvent {
            offer_id,
            listing_id: offer.listing_id,
            offerer: offerer.clone(),
        }
        .publish(&env);
    }

    pub fn reject_offer(env: Env, artist: Address, offer_id: u64) {
        Self::require_not_paused(&env);
        artist.require_auth();
        let mut offer = load_offer(&env, offer_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::OfferNotFound));
        let listing = load_listing(&env, offer.listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));
        if listing.artist != artist {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if offer.status != OfferStatus::Pending {
            panic_with_error!(&env, MarketplaceError::OfferNotPending);
        }
        TokenClient::new(&env, &offer.token).transfer(
            &env.current_contract_address(),
            &offer.offerer,
            &offer.amount,
        );
        offer.status = OfferStatus::Rejected;
        save_offer(&env, &offer);

        OfferRejectedEvent {
            offer_id,
            listing_id: offer.listing_id,
            offerer: offer.offerer.clone(),
        }
        .publish(&env);
    }

    pub fn accept_offer(env: Env, artist: Address, offer_id: u64) {
        Self::require_not_paused(&env);
        artist.require_auth();
        let mut offer = load_offer(&env, offer_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::OfferNotFound));
        let listing_id = offer.listing_id;

        // Reentrancy guard (same listing lock as buy_artwork)
        if !acquire_listing_lock(&env, listing_id) {
            panic_with_error!(&env, MarketplaceError::ReentrancyGuard);
        }

        let mut listing = match load_listing(&env, listing_id) {
            Some(l) => l,
            None => {
                release_listing_lock(&env, listing_id);
                panic_with_error!(&env, MarketplaceError::ListingNotFound);
            }
        };
        if listing.artist != artist {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if offer.status != OfferStatus::Pending || listing.status != ListingStatus::Active {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::OfferNotPending);
        }

        // ── CHECKS-EFFECTS-INTERACTIONS ──────────────────────────────────────
        // Persist all state mutations before any cross-contract call so that a
        // reentrant attempt on the same listing sees it already Sold.
        let accepted_offerer = offer.offerer.clone();
        let accepted_amount = offer.amount;
        let accepted_listing_id = offer.listing_id;
        offer.status = OfferStatus::Accepted;
        save_offer(&env, &offer);
        listing.status = ListingStatus::Sold;
        listing.owner = Some(accepted_offerer.clone());
        save_listing(&env, &listing);
        remove_from_active_listings(&env, accepted_listing_id);

        // Mark all other pending offers as rejected (state change only).
        let sibling_offers = load_listing_offers(&env, listing.listing_id);
        let mut refund_offerers: Vec<Address> = Vec::new(&env);
        let mut refund_amounts: Vec<i128> = Vec::new(&env);
        let mut refund_tokens: Vec<Address> = Vec::new(&env);
        for oid in sibling_offers.iter() {
            if oid != offer_id {
                if let Some(mut other) = load_offer(&env, oid) {
                    if other.status == OfferStatus::Pending {
                        other.status = OfferStatus::Rejected;
                        save_offer(&env, &other);
                        refund_offerers.push_back(other.offerer.clone());
                        refund_amounts.push_back(other.amount);
                        refund_tokens.push_back(other.token.clone());
                    }
                }
            }
        }

        OfferAcceptedEvent {
            offer_id,
            listing_id: accepted_listing_id,
            offerer: accepted_offerer.clone(),
            amount: accepted_amount,
        }
        .publish(&env);

        // ── INTERACTIONS ─────────────────────────────────────────────────────
        // Use the listing's snapshotted protocol fee so settlement matches what
        // was agreed at listing creation time.
        Self::distribute_payout(
            &env,
            &offer.token,
            &listing.collection,
            offer.amount,
            &artist,
            &listing.recipients,
            &offer.offerer,
            false,
            listing.protocol_fee_bps, // Use snapshotted fee
        );

        // Transfer the NFT
        env.invoke_contract::<()>(
            &listing.collection,
            &soroban_sdk::Symbol::new(&env, "transfer_from"),
            soroban_sdk::vec![
                &env,
                env.current_contract_address().into_val(&env),
                artist.into_val(&env),
                offer.offerer.into_val(&env),
                listing.token_id.into_val(&env)
            ],
        );

        // Refund rejected offer escrows
        for i in 0..refund_offerers.len() {
            TokenClient::new(&env, &refund_tokens.get(i).unwrap()).transfer(
                &env.current_contract_address(),
                &refund_offerers.get(i).unwrap(),
                &refund_amounts.get(i).unwrap(),
            );
        }

        release_listing_lock(&env, listing_id);
    }

    pub fn get_listing(env: Env, listing_id: u64) -> Listing {
        load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound))
    }
    pub fn get_total_listings(env: Env) -> u64 {
        get_listing_count(&env)
    }
    pub fn get_artist_listings(env: Env, artist: Address) -> Vec<u64> {
        get_artist_listing_ids(&env, &artist)
    }

    pub fn get_total_auctions(env: Env) -> u64 {
        get_auction_count(&env)
    }

    pub fn get_artist_auctions(env: Env, artist: Address) -> Vec<u64> {
        get_artist_auction_ids(&env, &artist)
    }

    pub fn get_active_listings(env: Env, limit: u32, offset: u32) -> Vec<u64> {
        let ids = get_active_listing_ids(&env);
        let start = offset as usize;
        let end = (start + limit as usize).min(ids.len() as usize);
        let mut page = Vec::new(&env);
        for i in start..end {
            page.push_back(ids.get(i as u32).unwrap());
        }
        page
    }

    pub fn get_active_listings_page(env: Env, start: u32, limit: u32) -> Vec<u64> {
        Self::get_active_listings(env, limit, start)
    }

    pub fn get_offers_by_listing(env: Env, listing_id: u64) -> Vec<Offer> {
        let offer_ids = load_listing_offers(&env, listing_id);
        let mut offers = Vec::new(&env);

        for offer_id in offer_ids.iter() {
            if let Some(offer) = load_offer(&env, offer_id) {
                offers.push_back(offer);
            }
        }
        offers
    }

    pub fn get_listing_status(env: Env, listing_id: u64) -> ListingStatus {
        let listing = load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));
        listing.status
    }

    pub fn get_auction(env: Env, auction_id: u64) -> Auction {
        load_auction(&env, auction_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::AuctionNotFound))
    }
    pub fn get_offer(env: Env, offer_id: u64) -> Offer {
        load_offer(&env, offer_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::OfferNotFound))
    }
    pub fn get_listing_offers(env: Env, listing_id: u64) -> Vec<u64> {
        load_listing_offers(&env, listing_id)
    }
    pub fn get_offerer_offers(env: Env, offerer: Address) -> Vec<u64> {
        load_offerer_offers(&env, &offerer)
    }

    fn require_admin(env: &Env) {
        let key = crate::storage::DataKey::Admin;
        let admin = env
            .storage()
            .persistent()
            .get::<_, Address>(&key)
            .expect("admin not set");
        admin.require_auth();
    }

    /// Guard function that reverts with ContractPaused if the contract is paused.
    /// Should be called at the beginning of every mutating entry point.
    /// Read-only functions should NOT call this guard.
    fn require_not_paused(env: &Env) {
        if crate::storage::is_paused(env) {
            panic_with_error!(env, MarketplaceError::ContractPaused);
        }
    }

    fn is_token_whitelisted(env: &Env, token: &Address) -> bool {
        let key = crate::storage::DataKey::TokenWhitelist;
        let whitelist = env
            .storage()
            .persistent()
            .get::<_, Vec<Address>>(&key)
            .unwrap_or(Vec::new(env));
        if whitelist.is_empty() {
            true
        } else {
            whitelist.contains(token)
        }
    }

    /// Validate that the sum of all `Recipient.percentage` values (each expressed
    /// in basis points, 0–10 000) plus the current protocol fee does not exceed
    /// 10 000 bps (100 %).
    ///
    /// Uses `checked_add` throughout to prevent integer overflow on malformed
    /// input.  Panics with `RoyaltyExceedsLimit` when the invariant is violated.
    ///
    /// # Invariants checked
    /// * `recipients` must be non-empty (caller responsibility to guard with `InvalidSplit`).
    /// * `sum(r.percentage) + protocol_fee_bps <= 10_000`
    fn validate_recipients(
        env: &Env,
        recipients: &Vec<Recipient>,
        protocol_fee_bps: u32,
    ) {
        let len = recipients.len();
        let mut total_bps: u32 = 0;
        for i in 0..len {
            let bps = recipients.get(i).unwrap().percentage;
            total_bps = total_bps
                .checked_add(bps)
                .unwrap_or_else(|| panic_with_error!(env, MarketplaceError::RoyaltyExceedsLimit));
        }
        let combined = total_bps
            .checked_add(protocol_fee_bps)
            .unwrap_or_else(|| panic_with_error!(env, MarketplaceError::RoyaltyExceedsLimit));
        if combined > 10_000 {
            panic_with_error!(env, MarketplaceError::RoyaltyExceedsLimit);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn distribute_payout(
        env: &Env,
        token_addr: &Address,
        collection_addr: &Address,
        amount: i128,
        seller: &Address,
        recipients: &Vec<Recipient>,
        buyer: &Address,
        transfer_from_buyer: bool,
        fee_bps: u32, // Protocol fee in bps — caller provides snapshotted or live value
    ) {
        let token = TokenClient::new(env, token_addr);
        if transfer_from_buyer {
            token.transfer(buyer, &env.current_contract_address(), &amount);
        }
        let mut payout = amount;

        let royalty_info: (Address, u32) = env.invoke_contract(
            collection_addr,
            &soroban_sdk::Symbol::new(env, "royalty_info"),
            soroban_sdk::vec![env],
        );
        let royalty_receiver = royalty_info.0;
        let royalty_bps = royalty_info.1;

        if royalty_bps > 0 && royalty_receiver != seller.clone() {
            let royalty = amount * royalty_bps as i128 / 10_000;
            token.transfer(&env.current_contract_address(), &royalty_receiver, &royalty);
            payout -= royalty;
        }
        if let Some(t) = crate::storage::get_treasury_storage(env) {
            let fee = payout * fee_bps as i128 / 10_000;
            token.transfer(&env.current_contract_address(), &t, &fee);
            payout -= fee;
        }
        let len = recipients.len();
        let mut ds = 0;
        for i in 0..len {
            let r = recipients.get(i).unwrap();
            let amt = if i == len - 1 {
                payout - ds
            } else {
                (payout * r.percentage as i128) / 10_000
            };
            token.transfer(&env.current_contract_address(), &r.address, &amt);
            ds += amt;
        }
    }
}
