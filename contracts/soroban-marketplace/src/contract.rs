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
        add_to_active_listings, append_bid_record, clear_pending_admin_storage,
        get_active_listing_ids, get_artist_auction_ids, get_artist_listing_ids,
        get_auction_count, get_listing_count, get_pending_admin_storage,
        increment_auction_count, increment_listing_count, increment_offer_count,
        is_artist_revoked_storage, load_auction, load_auction_bids, load_listing,
        load_listing_offers, load_offer, load_offerer_offers, release_auction_lock,
        release_listing_lock, remove_artist_revocation_storage, remove_from_active_listings,
        save_auction, save_listing, save_listing_offers, save_offer, save_offerer_offers,
        set_artist_revocation_storage, set_pending_admin_storage,
        get_auction_extension_window_storage, get_auction_extension_trigger_storage,
        set_auction_extension_window_storage, set_auction_extension_trigger_storage,
        get_min_price_storage, get_max_price_storage,
        set_min_price_storage, set_max_price_storage,
        is_migration_done, set_migration_done,
    },
    types::{
        Auction, AuctionStatus, BidRecord, CancelReason, Listing, ListingStatus,
        MarketplaceError, Offer, OfferStatus, Recipient,
    },
};

/// Default minimum bid increment used when no global value has been configured.
/// A value of 1 preserves the invariant that a new bid must strictly exceed the
/// previous highest bid.
const DEFAULT_MIN_BID_INCREMENT: i128 = 1;

/// Default anti-sniping extension window: 10 minutes. New auctions inherit this
/// unless the admin has configured a different value before auction creation.
const DEFAULT_EXTENSION_WINDOW: u64 = 600;

/// Default anti-sniping trigger: a bid placed when fewer than 5 minutes remain
/// triggers the extension. Set to 0 to disable the feature by default.
const DEFAULT_EXTENSION_TRIGGER: u64 = 0;

/// Minimum auction duration in seconds (1 hour).
///
/// An auction whose computed `end_time = now + duration` would be less than
/// `MIN_AUCTION_DURATION` seconds in the future is rejected with
/// `InvalidAuctionDuration`.  This prevents meaningless or front-runnable
/// auctions that expire almost immediately.
const MIN_AUCTION_DURATION: u64 = 3_600; // 1 hour

/// Maximum number of bid records retained per auction in the on-chain history.
///
/// When a new bid is placed and the history already holds `BID_HISTORY_CAP`
/// entries, the oldest entry is evicted so storage growth is strictly bounded.
/// Exposed via `get_auction_bids` for contract-side verification and frontend
/// fallback.
const BID_HISTORY_CAP: u32 = 20;

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

    // ── Contract versioning & migration ──────────────────────────

    /// Returns the semantic version of this contract deployment.
    ///
    /// Callers (off-chain indexers, upgrade scripts) can use this to decide
    /// whether a `migrate` call is necessary before using new storage keys.
    pub fn version(env: Env) -> soroban_sdk::String {
        soroban_sdk::String::from_str(&env, CONTRACT_VERSION)
    }

    /// Admin-guarded, idempotent storage migration entry point.
    ///
    /// # Idempotency
    /// The function records a per-version marker in persistent storage the
    /// first time it is called.  Subsequent calls for the *same* version
    /// revert with `AlreadyMigrated` so that accidental double-invocations
    /// during upgrade scripts are caught rather than silently re-executed.
    ///
    /// # Usage
    /// Upgrade scripts should:
    /// 1. Deploy the new WASM and invoke `migrate(admin)`.
    /// 2. Verify `version()` returns the expected string.
    /// 3. Any data back-fill should be performed inside this function body
    ///    for the specific version being migrated to.
    pub fn migrate(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }

        let version = soroban_sdk::String::from_str(&env, CONTRACT_VERSION);

        // Guard: revert if this migration has already been applied.
        if is_migration_done(&env, &version) {
            panic_with_error!(&env, MarketplaceError::AlreadyMigrated);
        }

        // ── Per-version migration logic ──────────────────────────────
        // Add version-specific storage shape changes here.  Each released
        // version gets its own `if version == "x.y.z" { ... }` block.
        // Example for a future 1.1.0 migration:
        //   if version_str == "1.1.0" {
        //       // back-fill new field on existing listings...
        //   }
        //
        // For 1.0.0 there is no storage shape change — the marker alone
        // establishes forward compatibility for subsequent upgrades.
        // ────────────────────────────────────────────────────────────

        // Record the migration marker so this version is not re-applied.
        set_migration_done(&env, &version);
    }

    // ── Global price bounds ───────────────────────────────────────

    /// Set global minimum and maximum price bounds for listings and auctions.
    ///
    /// Both `min` and `max` must be positive and `min <= max`.  Any subsequent
    /// `create_listing` or `create_auction` call whose price falls outside
    /// `[min, max]` will revert with `PriceOutOfBounds`.
    ///
    /// # Backward compatibility
    /// Existing listings and auctions are NOT retroactively affected.  Only new
    /// items created after the bounds are set are validated against them.
    ///
    /// # Disabling bounds
    /// Pass `min = 0` or call the setter with very large values to make a bound
    /// effectively permissive.  To fully remove bounds, `set_price_bounds` can
    /// be called with `min = 1` and `max = i128::MAX / 10_000` (the existing
    /// overflow-safety ceiling already applied in `update_listing_price`).
    pub fn set_price_bounds(env: Env, admin: Address, min: i128, max: i128) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        // Both bounds must be non-negative and min <= max.
        if min < 0 || max < 0 || min > max {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }
        set_min_price_storage(&env, min);
        set_max_price_storage(&env, max);
    }

    /// Returns `(min_price, max_price)` — the current global price bounds.
    ///
    /// A value of `None` means the corresponding bound has not been configured
    /// and is treated permissively (no limit in that direction).
    pub fn get_price_bounds(env: Env) -> (Option<i128>, Option<i128>) {
        (
            get_min_price_storage(&env),
            get_max_price_storage(&env),
        )
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

    /// Set the global minimum bid increment (in payment-token stroops). New
    /// auctions snapshot this value at creation. Admin-only; must be non-negative.
    pub fn set_min_bid_increment(env: Env, admin: Address, increment: i128) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if increment < 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }
        crate::storage::set_min_bid_increment_storage(&env, increment);
    }

    pub fn get_min_bid_increment(env: Env) -> i128 {
        crate::storage::get_min_bid_increment_storage(&env).unwrap_or(DEFAULT_MIN_BID_INCREMENT)
    }

    /// Set the global auction extension window in seconds (anti-sniping feature).
    /// When a qualifying bid arrives near the end of an auction, the end time is
    /// extended by this many seconds. Admin-only. New auctions snapshot this value.
    pub fn set_auction_extension_window(env: Env, admin: Address, window: u64) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        set_auction_extension_window_storage(&env, window);
    }

    pub fn get_auction_extension_window(env: Env) -> u64 {
        get_auction_extension_window_storage(&env).unwrap_or(DEFAULT_EXTENSION_WINDOW)
    }

    /// Set the global auction extension trigger threshold in seconds (anti-sniping).
    /// If `end_time - now < trigger` when a bid is placed, the anti-sniping rule fires.
    /// Admin-only. New auctions snapshot this value.
    pub fn set_auction_extension_trigger(env: Env, admin: Address, trigger: u64) {
        admin.require_auth();
        let stored_admin = Self::get_admin(env.clone()).expect("admin not set");
        if admin != stored_admin {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        set_auction_extension_trigger_storage(&env, trigger);
    }

    pub fn get_auction_extension_trigger(env: Env) -> u64 {
        get_auction_extension_trigger_storage(&env).unwrap_or(DEFAULT_EXTENSION_TRIGGER)
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
        expires_at: Option<u64>,
    ) -> u64 {
        Self::require_not_paused(&env);
        artist.require_auth();
        Self::require_not_revoked(&env, &artist);
        if price <= 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }

        // Enforce admin-configured global price bounds when set.
        Self::require_price_in_bounds(&env, price);

        // Validate expiry is strictly in the future if provided
        if let Some(exp) = expires_at {
            if exp <= env.ledger().timestamp() {
                panic_with_error!(&env, MarketplaceError::InvalidPrice);
            }
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
            panic_with_error!(&env, MarketplaceError::TokenNotWhitelisted);
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
            expires_at,
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
        // Reject self-purchase: buyer must not be the listing artist (original
        // creator) or the current NFT owner. Using a dedicated error code so
        // indexers and clients surface a clear reason rather than a generic 5.
        if listing.artist == buyer {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::SelfPurchaseNotAllowed);
        }
        if let Some(ref owner) = listing.owner {
            if *owner == buyer {
                release_listing_lock(&env, listing_id);
                panic_with_error!(&env, MarketplaceError::SelfPurchaseNotAllowed);
            }
        }
        if let Some(exp) = listing.expires_at {
            if env.ledger().timestamp() >= exp {
                release_listing_lock(&env, listing_id);
                panic_with_error!(&env, MarketplaceError::ListingExpired);
            }
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
        let fee_collected = Self::distribute_payout(
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

        // Emit ProtocolFeeCollected so treasury revenue is observable on-chain.
        if fee_collected > 0 {
            if let Some(treasury) = crate::storage::get_treasury_storage(&env) {
                ProtocolFeeCollectedEvent {
                    listing_id,
                    amount: fee_collected,
                    token: listing.token.clone(),
                    treasury,
                }
                .publish(&env);
            }
        }

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

    // ── update_listing_price ─────────────────────────────────────────────────
    //
    // Allows the listing owner to update the price of an active listing in
    // place without cancelling and re-creating it.  The listing id and
    // creation ledger are preserved.  A ListingPriceUpdated event is emitted
    // with both the old and new price so indexers can reconstruct a full price
    // history.

    pub fn update_listing_price(
        env: Env,
        seller: Address,
        listing_id: u64,
        new_price: i128,
    ) -> bool {
        Self::require_not_paused(&env);
        seller.require_auth();

        let mut listing = load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));

        // Only the listing owner (artist) may update the price.
        if listing.artist != seller {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }

        // Listing must still be active.
        if listing.status != ListingStatus::Active {
            panic_with_error!(&env, MarketplaceError::ListingNotActive);
        }

        // Validate the new price is positive.
        if new_price <= 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }

        // Upper-bound sanity: price must not exceed i128::MAX / 10_000 to avoid
        // overflow in the payout distribution math.
        let price_upper_bound: i128 = i128::MAX / 10_000;
        if new_price > price_upper_bound {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }

        let old_price = listing.price;
        listing.price = new_price;

        // Persist the updated listing and bump its TTL so the change is durable.
        save_listing(&env, &listing);

        crate::events::ListingPriceUpdatedEvent {
            listing_id,
            old_price,
            new_price,
            updated_by: seller.clone(),
        }
        .publish(&env);

        true
    }

    // ── expire_listing ───────────────────────────────────────────────────────
    //
    // Permissionless entry point: anyone may call this to move an expired
    // listing out of the active set.  The listing must have an `expires_at`
    // timestamp that has already passed.  Calling it before expiry reverts
    // with ListingNotExpired.

    pub fn expire_listing(env: Env, listing_id: u64) {
        let mut listing = load_listing(&env, listing_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::ListingNotFound));

        // Only Active listings can be expired.
        if listing.status != ListingStatus::Active {
            panic_with_error!(&env, MarketplaceError::ListingNotActive);
        }

        // The listing must actually have an expiry and it must have passed.
        let exp = match listing.expires_at {
            Some(t) => t,
            None => panic_with_error!(&env, MarketplaceError::ListingNotExpired),
        };
        if env.ledger().timestamp() < exp {
            panic_with_error!(&env, MarketplaceError::ListingNotExpired);
        }

        // ── Effects ──────────────────────────────────────────────────────────
        // Mark the listing Cancelled (semantically: expired).  We reuse the
        // Cancelled status so the indexer only has to handle one terminal state.
        listing.status = ListingStatus::Cancelled;
        save_listing(&env, &listing);
        remove_from_active_listings(&env, listing_id);

        crate::events::ListingExpiredEvent {
            listing_id,
            expired_at: exp,
            ledger_sequence: env.ledger().sequence(),
        }
        .publish(&env);
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
        Self::require_not_revoked(&env, &creator);
        if reserve_price <= 0 {
            panic_with_error!(&env, MarketplaceError::InvalidPrice);
        }
        // Enforce a minimum auction duration so auctions that would expire
        // nearly immediately (or in the past) are rejected up front.
        // MIN_AUCTION_DURATION is documented in the constant declaration above.
        if duration < MIN_AUCTION_DURATION {
            panic_with_error!(&env, MarketplaceError::InvalidAuctionDuration);
        }
        if !Self::is_token_whitelisted(&env, &token) {
            panic_with_error!(&env, MarketplaceError::TokenNotWhitelisted);
        }
        let auction_id = increment_auction_count(&env);
        let end_time = env.ledger().timestamp() + duration;
        // Snapshot the global minimum bid increment so the auction's bidding
        // rules are fixed at creation time, regardless of later admin changes.
        let min_increment = crate::storage::get_min_bid_increment_storage(&env)
            .unwrap_or(DEFAULT_MIN_BID_INCREMENT);
        // Snapshot the anti-sniping parameters so the auction's extension
        // behaviour is determined at creation, not by future admin changes.
        let extension_window = get_auction_extension_window_storage(&env)
            .unwrap_or(DEFAULT_EXTENSION_WINDOW);
        let extension_trigger = get_auction_extension_trigger_storage(&env)
            .unwrap_or(DEFAULT_EXTENSION_TRIGGER);
        // Snapshot the global protocol fee so settlement math is fixed at
        // creation time — consistent with how listings work (ISSUE-005 parity).
        let protocol_fee_bps =
            crate::storage::get_protocol_fee_bps_storage(&env).unwrap_or(0);
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
            min_increment,
            extension_window,
            extension_trigger,
            protocol_fee_bps,
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
        // Block shill bidding: the auction creator must not be able to bid on
        // their own auction.  This prevents artificial price inflation and
        // protects legitimate bidders from being outbid by the seller.
        if bidder == auction.creator {
            panic_with_error!(&env, MarketplaceError::SelfBidNotAllowed);
        }
        // Enforce the minimum acceptable bid on-chain:
        //   • first bid (no prior bidder): must be at least `reserve_price`.
        //   • subsequent bids: must exceed the current highest bid by at least
        //     `min_increment`, computed with checked arithmetic to avoid overflow.
        let required_min = if auction.highest_bid == 0 {
            auction.reserve_price
        } else {
            auction
                .highest_bid
                .checked_add(auction.min_increment)
                .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::BidTooLow))
        };
        if amount < required_min {
            panic_with_error!(&env, MarketplaceError::BidTooLow);
        }

        // Capture the previous highest bidder/amount before they are overwritten,
        // so the escrowed funds can be refunded after state is recorded.
        let previous_bidder = auction.highest_bidder.clone();
        let previous_bid = auction.highest_bid;

        // ── CHECKS-EFFECTS-INTERACTIONS ──────────────────────────────────────
        // Record the new highest bid BEFORE moving any tokens, so a reentrant
        // place_bid on the same auction observes the updated state. The whole
        // call is atomic: if any transfer below fails, all of these effects roll
        // back, so escrow can never be left inconsistent.
        auction.highest_bid = amount;
        auction.highest_bidder = Some(bidder.clone());

        // ── Anti-sniping: extend the auction when a bid arrives near the end ─
        // Only fires when extension_trigger > 0 (opt-in). If the time remaining
        // is strictly less than the trigger threshold, push the end time forward
        // by the configured extension window.
        let now = env.ledger().timestamp();
        let time_remaining = auction.end_time.saturating_sub(now);
        let mut extended = false;
        if auction.extension_trigger > 0 && time_remaining < auction.extension_trigger {
            auction.end_time = now
                .checked_add(auction.extension_window)
                .unwrap_or(auction.end_time);
            extended = true;
        }

        save_auction(&env, &auction);

        // ── Record bid in bounded history ────────────────────────────────────
        // The history is capped to BID_HISTORY_CAP entries; the oldest is
        // evicted when the cap is reached.  Chronological (oldest-to-newest)
        // order is preserved, so index 0 is always the earliest retained bid.
        append_bid_record(
            &env,
            auction_id,
            &BidRecord {
                bidder: bidder.clone(),
                amount,
                ledger: env.ledger().sequence(),
            },
            BID_HISTORY_CAP,
        );

        BidPlacedEvent {
            auction_id,
            bidder: bidder.clone(),
            bid_amount: amount,
        }
        .publish(&env);

        // Emit AuctionExtended after the bid event so indexers can correlate
        // a single ledger's events: bid → optional extension.
        if extended {
            AuctionExtendedEvent {
                auction_id,
                new_end_time: auction.end_time,
            }
            .publish(&env);
        }

        // ── INTERACTIONS ─────────────────────────────────────────────────────
        // Refund the previous bidder's escrow, then pull the new bid into escrow.
        // Net effect: contract-held escrow for this auction equals `amount`, the
        // new highest bid.
        let token_client = TokenClient::new(&env, &auction.token);
        if let Some(prev) = previous_bidder {
            token_client.transfer(&env.current_contract_address(), &prev, &previous_bid);
        }
        token_client.transfer(&bidder, &env.current_contract_address(), &amount);
    }

    pub fn finalize_auction(env: Env, caller: Address, auction_id: u64) {
        // ─────────────────────────────────────────────────────────────────────
        // CHECKS-EFFECTS-INTERACTIONS ordering:
        //   1. Acquire reentrancy lock.
        //   2. Load & validate auction (status + time).
        //   3. Mutate state to Finalized/Cancelled (Effects).
        //   4. Emit event (Effects).
        //   5. Execute all external calls — token payout and NFT transfer
        //      (Interactions).
        //   6. Release lock.
        //
        // Any caller may finalize once `now >= end_time`.  Early calls revert
        // with AuctionNotEnded so the auction cannot be settled prematurely.
        // A second call on an already-settled auction reverts with
        // AuctionAlreadyFinalized regardless of who calls.
        // ─────────────────────────────────────────────────────────────────────
        Self::require_not_paused(&env);
        caller.require_auth();

        // ── 1. Reentrancy guard ───────────────────────────────────────────────
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

        // ── 2. Checks ─────────────────────────────────────────────────────────
        // Status: reject any attempt on an already-settled auction.
        if auction.status != AuctionStatus::Active {
            release_auction_lock(&env, auction_id);
            panic_with_error!(&env, MarketplaceError::AuctionAlreadyFinalized);
        }

        // Time: anyone may finalize only after the auction has ended.
        if env.ledger().timestamp() < auction.end_time {
            release_auction_lock(&env, auction_id);
            panic_with_error!(&env, MarketplaceError::AuctionNotEnded);
        }

        // ── 3. Effects — mutate state before any interaction ──────────────────
        // Capture settlement data before overwriting auction fields.
        let winner = auction.highest_bidder.clone();
        let winning_bid = auction.highest_bid;
        let snapshotted_fee = auction.protocol_fee_bps;

        if winner.is_some() {
            auction.status = AuctionStatus::Finalized;
        } else {
            auction.status = AuctionStatus::Cancelled;
        }
        save_auction(&env, &auction);

        // ── 4. Event ──────────────────────────────────────────────────────────
        AuctionFinalizedEvent {
            auction_id,
            winner: winner.clone(),
            amount: winning_bid,
        }
        .publish(&env);

        // ── 5. Interactions — external calls after state is final ─────────────
        if let Some(ref w) = winner {
            // Distribute the winning bid to the creator and their recipients
            // using the fee rate snapshotted at auction creation — this gives
            // the creator and bidder certainty about the net settlement amount
            // from the moment the auction was created (parity with listings).
            let fee_collected = Self::distribute_payout(
                &env,
                &auction.token,
                &auction.collection,
                winning_bid,
                &auction.creator,
                &auction.recipients,
                w,
                false, // funds are already held in escrow; do not pull from winner
                snapshotted_fee,
            );

            // Emit ProtocolFeeCollected so treasury revenue is observable on-chain.
            if fee_collected > 0 {
                if let Some(treasury) = crate::storage::get_treasury_storage(&env) {
                    ProtocolFeeCollectedEvent {
                        listing_id: auction_id, // reuse field; auction_id identifies the trade
                        amount: fee_collected,
                        token: auction.token.clone(),
                        treasury,
                    }
                    .publish(&env);
                }
            }

            // Transfer the NFT from the creator to the winner.
            env.invoke_contract::<()>(
                &auction.collection,
                &soroban_sdk::Symbol::new(&env, "transfer_from"),
                soroban_sdk::vec![
                    &env,
                    env.current_contract_address().into_val(&env),
                    auction.creator.into_val(&env),
                    w.into_val(&env),
                    auction.token_id.into_val(&env)
                ],
            );
        } else {
            // No bids — return the NFT to the creator so they are not locked out.
            env.invoke_contract::<()>(
                &auction.collection,
                &soroban_sdk::Symbol::new(&env, "transfer_from"),
                soroban_sdk::vec![
                    &env,
                    env.current_contract_address().into_val(&env),
                    auction.creator.into_val(&env),
                    auction.creator.into_val(&env),
                    auction.token_id.into_val(&env)
                ],
            );
        }

        // ── 6. Release lock ───────────────────────────────────────────────────
        release_auction_lock(&env, auction_id);
    }

    /// Cancel an auction that has received **no bids**.
    ///
    /// Rules:
    /// - Only the auction creator may call this.
    /// - If `highest_bidder` is `Some(_)` the call reverts with `AuctionHasBids`
    ///   to protect the bidder's escrowed funds.
    /// - The auction must be `Active`; attempting to cancel an already-finalised
    ///   or already-cancelled auction reverts with `AuctionAlreadyFinalized`.
    ///
    /// On success:
    /// - Auction status is set to `Cancelled`.
    /// - `AuctionCancelledEvent` is emitted.
    pub fn cancel_auction(env: Env, creator: Address, auction_id: u64) {
        Self::require_not_paused(&env);
        creator.require_auth();

        let mut auction = load_auction(&env, auction_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::AuctionNotFound));

        // Only the original creator may cancel.
        if auction.creator != creator {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }

        // Must be active — finalized / already-cancelled auctions cannot be cancelled again.
        if auction.status != AuctionStatus::Active {
            panic_with_error!(&env, MarketplaceError::AuctionAlreadyFinalized);
        }

        // Refuse if any bid has been placed — the bidder's escrow must not be stranded.
        if auction.highest_bidder.is_some() {
            panic_with_error!(&env, MarketplaceError::AuctionHasBids);
        }

        // ── EFFECTS ──────────────────────────────────────────────────────────
        auction.status = AuctionStatus::Cancelled;
        save_auction(&env, &auction);

        AuctionCancelledEvent {
            auction_id,
            cancelled_by: creator.clone(),
        }
        .publish(&env);
    }

    pub fn make_offer(
        env: Env,
        offerer: Address,
        listing_id: u64,
        amount: i128,
        token: Address,
        expires_at: Option<u64>,
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
        // Reject at creation time if the offer token is not whitelisted,
        // giving the offerer immediate feedback instead of a failed purchase later.
        if !Self::is_token_whitelisted(&env, &token) {
            panic_with_error!(&env, MarketplaceError::TokenNotWhitelisted);
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
                expires_at, // #32: optional expiry stored with offer
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

    // #30: Require offerer auth; revert with InvalidOfferState on terminal offers.
    pub fn withdraw_offer(env: Env, offerer: Address, offer_id: u64) {
        Self::require_not_paused(&env);
        offerer.require_auth();
        let mut offer = load_offer(&env, offer_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::OfferNotFound));
        if offer.offerer != offerer {
            panic_with_error!(&env, MarketplaceError::Unauthorized);
        }
        if offer.status != OfferStatus::Pending {
            panic_with_error!(&env, MarketplaceError::InvalidOfferState);
        }
        // #29: Refund escrow on withdraw.
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

    // #30: Require seller (artist) auth; revert with InvalidOfferState on terminal offers.
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
            panic_with_error!(&env, MarketplaceError::InvalidOfferState);
        }
        // #29: Refund escrow on reject.
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

    // #30: Require seller (artist) auth; revert with InvalidOfferState on terminal offers.
    // #32: Revert with OfferExpired if the offer has passed its expires_at.
    // #31: Refund and emit OfferRejectedEvent for each competing pending offer.
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
        // #30: InvalidOfferState for any non-Pending offer.
        if offer.status != OfferStatus::Pending || listing.status != ListingStatus::Active {
            release_listing_lock(&env, listing_id);
            panic_with_error!(&env, MarketplaceError::InvalidOfferState);
        }

        // #32: Reject acceptance if offer has expired.
        if let Some(exp) = offer.expires_at {
            if env.ledger().timestamp() >= exp {
                release_listing_lock(&env, listing_id);
                panic_with_error!(&env, MarketplaceError::OfferExpired);
            }
        }

        // Reject acceptance if listing has expired.
        if let Some(exp) = listing.expires_at {
            if env.ledger().timestamp() >= exp {
                release_listing_lock(&env, listing_id);
                panic_with_error!(&env, MarketplaceError::ListingExpired);
            }
        }

        // ── CHECKS-EFFECTS-INTERACTIONS ──────────────────────────────────────
        let accepted_offerer = offer.offerer.clone();
        let accepted_amount = offer.amount;
        let accepted_listing_id = offer.listing_id;
        offer.status = OfferStatus::Accepted;
        save_offer(&env, &offer);
        listing.status = ListingStatus::Sold;
        listing.owner = Some(accepted_offerer.clone());
        save_listing(&env, &listing);
        remove_from_active_listings(&env, accepted_listing_id);

        // #31: Mark all other pending offers Rejected; collect refund data and
        // emit OfferRejectedEvent for each.
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
                        OfferRejectedEvent {
                            offer_id: oid,
                            listing_id: accepted_listing_id,
                            offerer: other.offerer.clone(),
                        }
                        .publish(&env);
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
        let fee_collected = Self::distribute_payout(
            &env,
            &offer.token,
            &listing.collection,
            offer.amount,
            &artist,
            &listing.recipients,
            &offer.offerer,
            false,
            listing.protocol_fee_bps,
        );

        if fee_collected > 0 {
            if let Some(treasury) = crate::storage::get_treasury_storage(&env) {
                ProtocolFeeCollectedEvent {
                    listing_id: accepted_listing_id,
                    amount: fee_collected,
                    token: offer.token.clone(),
                    treasury,
                }
                .publish(&env);
            }
        }

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

        // #29: Refund escrowed amounts for rejected competing offers.
        for i in 0..refund_offerers.len() {
            TokenClient::new(&env, &refund_tokens.get(i).unwrap()).transfer(
                &env.current_contract_address(),
                &refund_offerers.get(i).unwrap(),
                &refund_amounts.get(i).unwrap(),
            );
        }

        release_listing_lock(&env, listing_id);
    }

    /// #32: Permissionless reclaim of escrowed funds from an expired offer.
    /// Anyone may call after `offer.expires_at` has passed. Offer must be Pending.
    pub fn reclaim_offer(env: Env, offer_id: u64) {
        Self::require_not_paused(&env);
        let mut offer = load_offer(&env, offer_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::OfferNotFound));
        if offer.status != OfferStatus::Pending {
            panic_with_error!(&env, MarketplaceError::InvalidOfferState);
        }
        let exp = match offer.expires_at {
            Some(e) => e,
            None => panic_with_error!(&env, MarketplaceError::InvalidOfferState),
        };
        if env.ledger().timestamp() < exp {
            panic_with_error!(&env, MarketplaceError::OfferExpired);
        }
        TokenClient::new(&env, &offer.token).transfer(
            &env.current_contract_address(),
            &offer.offerer,
            &offer.amount,
        );
        offer.status = OfferStatus::Withdrawn;
        save_offer(&env, &offer);

        OfferReclaimedEvent {
            offer_id,
            listing_id: offer.listing_id,
            offerer: offer.offerer.clone(),
            amount: offer.amount,
        }
        .publish(&env);
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

    /// Maximum `limit` accepted by `get_listings_paginated`.
    pub const MAX_PAGE_LIMIT: u32 = 100;

    /// Paginated view over active listings.
    ///
    /// Returns up to `limit` resolved [`Listing`] structs starting at cursor
    /// `start` (a zero-based index into the active-listings index).  `limit`
    /// is clamped to [`MAX_PAGE_LIMIT`] so clients cannot request oversized
    /// responses.
    ///
    /// # Return value
    /// `(listings, next_cursor)` where:
    /// - `listings` contains the resolved `Listing` structs for the page.
    /// - `next_cursor` is the index to pass as `start` to fetch the next
    ///   page.  When the returned page is the last one (i.e. there are no
    ///   further entries), `next_cursor` equals the total number of active
    ///   listing IDs, which is ≥ the current active-listing count and can be
    ///   used as a sentinel by callers (`listings.len() < limit` also
    ///   signals exhaustion).
    ///
    /// An out-of-range `start` (≥ total active listings) returns an empty
    /// page and a `next_cursor` equal to `start` without panicking.
    pub fn get_listings_paginated(env: Env, start: u32, limit: u32) -> (Vec<Listing>, u32) {
        // Clamp limit to the hard maximum.
        let effective_limit = if limit > Self::MAX_PAGE_LIMIT {
            Self::MAX_PAGE_LIMIT
        } else if limit == 0 {
            // A zero limit is valid; return an empty page immediately.
            return (Vec::new(&env), start);
        } else {
            limit
        };

        let ids = get_active_listing_ids(&env);
        let total = ids.len();

        // Out-of-range start: return empty page + same cursor (no panic).
        if start >= total {
            return (Vec::new(&env), start);
        }

        let end = (start + effective_limit).min(total);
        let mut listings: Vec<Listing> = Vec::new(&env);
        for i in start..end {
            let listing_id = ids.get(i).unwrap();
            if let Some(listing) = load_listing(&env, listing_id) {
                listings.push_back(listing);
            }
        }

        let next_cursor = end;
        (listings, next_cursor)
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

    /// Return the bounded bid history for `auction_id` in chronological order
    /// (oldest bid first, newest last).
    ///
    /// The history is capped to `BID_HISTORY_CAP` entries on-chain; older bids
    /// beyond the cap are not available here (use the indexer for full history).
    /// Returns an empty vector when no bids have been placed.
    pub fn get_auction_bids(env: Env, auction_id: u64) -> Vec<BidRecord> {
        // Verify the auction exists before returning history so callers get a
        // clear AuctionNotFound error rather than an empty vec for a bad id.
        load_auction(&env, auction_id)
            .unwrap_or_else(|| panic_with_error!(&env, MarketplaceError::AuctionNotFound));
        load_auction_bids(&env, auction_id)
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

    /// Validates that `price` falls within the admin-configured
    /// `[min_price, max_price]` bounds.  Missing bounds are treated permissively:
    /// * no `min_price` stored → no lower bound enforced.
    /// * no `max_price` stored → no upper bound enforced.
    ///
    /// Reverts with `PriceOutOfBounds` when the price violates a set bound.
    fn require_price_in_bounds(env: &Env, price: i128) {
        if let Some(min) = get_min_price_storage(env) {
            if price < min {
                panic_with_error!(env, MarketplaceError::PriceOutOfBounds);
            }
        }
        if let Some(max) = get_max_price_storage(env) {
            if price > max {
                panic_with_error!(env, MarketplaceError::PriceOutOfBounds);
            }
        }
    }

    /// Guard function that reverts with ContractPaused if the contract is paused.
    /// Should be called at the beginning of every mutating entry point.
    /// Read-only functions should NOT call this guard.
    fn require_not_paused(env: &Env) {
        if crate::storage::is_paused(env) {
            panic_with_error!(env, MarketplaceError::ContractPaused);
        }
    }

    /// Guard that reverts with `ArtistRevoked` when `artist` has been revoked by
    /// an admin. Call this after authentication at the start of every creation
    /// path (listings, auctions) so a revoked artist can no longer create new
    /// items. It deliberately does NOT guard settlement paths (buy, accept_offer,
    /// finalize_auction), so existing items remain settleable after revocation.
    fn require_not_revoked(env: &Env, artist: &Address) {
        if is_artist_revoked_storage(env, artist) {
            panic_with_error!(env, MarketplaceError::ArtistRevoked);
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
    ) -> i128 /* returns the protocol fee actually transferred */ {
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
            let royalty = amount
                .checked_mul(royalty_bps as i128)
                .unwrap_or_else(|| panic_with_error!(env, MarketplaceError::ArithmeticOverflow))
                .checked_div(10_000)
                .unwrap_or_else(|| panic_with_error!(env, MarketplaceError::ArithmeticOverflow));
            token.transfer(&env.current_contract_address(), &royalty_receiver, &royalty);
            payout -= royalty;
        }

        let mut fee_collected: i128 = 0;
        if let Some(t) = crate::storage::get_treasury_storage(env) {
            let fee = payout * fee_bps as i128 / 10_000;
            if fee > 0 {
                token.transfer(&env.current_contract_address(), &t, &fee);
                fee_collected = fee;
            }
            payout -= fee;
        }
        let len = recipients.len();
        let mut ds = 0;
        for i in 0..len {
            let r = recipients.get(i).unwrap();
            let amt = if i == len - 1 {
                payout - ds
            } else {
                payout
                    .checked_mul(r.percentage as i128)
                    .unwrap_or_else(|| panic_with_error!(env, MarketplaceError::ArithmeticOverflow))
                    .checked_div(10_000)
                    .unwrap_or_else(|| panic_with_error!(env, MarketplaceError::ArithmeticOverflow))
            };
            token.transfer(&env.current_contract_address(), &r.address, &amt);
            ds += amt;
        }
        fee_collected
    }
}
