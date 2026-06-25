// storage.rs
use crate::types::{Auction, Listing, Offer};
use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    ListingCount,
    Listing(u64),
    ArtistListings(Address),
    Admin,
    TokenWhitelist,
    Treasury,
    ProtocolFeeBps,
    AuctionCount,
    Auction(u64),
    ArtistAuctions(Address),
    RevokedArtist(Address),
    OfferCount,
    Offer(u64),
    ListingOffers(u64),
    OffererOffers(Address),
    ListingLock(u64),
    AuctionLock(u64),
    IsPaused,
    PendingAdmin,
    ActiveListings,
    MinBidIncrement,
}

pub const LEDGER_TTL_BUMP: u32 = 432_000;
pub const LEDGER_TTL_THRESHOLD: u32 = 144_000;
pub const REENTRANCY_LOCK_TTL: u32 = 100;

// ── Centralized TTL helpers ──────────────────────────────────
//
// All persistent entries use the same LEDGER_TTL_THRESHOLD / LEDGER_TTL_BUMP
// constants so there is a single place to tune the eviction window.
// Callers should prefer `bump_entry_ttl` over open-coding extend_ttl so that
// a future change to the constants is reflected automatically everywhere.

/// Bump (extend) the TTL of any persistent DataKey to the standard window.
/// No-op if the entry does not exist.
pub fn bump_entry_ttl(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, LEDGER_TTL_THRESHOLD, LEDGER_TTL_BUMP);
}

/// Explicitly bump the ActiveListings index TTL.  Call this whenever the index
/// is read in a hot path (e.g. get_active_listing_ids) to prevent eviction of
/// a large, frequently accessed entry.
pub fn bump_active_listings_ttl(env: &Env) {
    bump_entry_ttl(env, &DataKey::ActiveListings);
}

// ── Counter helpers ──────────────────────────────────────────

pub fn get_listing_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::ListingCount)
        .unwrap_or(0)
}

pub fn increment_listing_count(env: &Env) -> u64 {
    let count = get_listing_count(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::ListingCount, &count);
    bump_entry_ttl(env, &DataKey::ListingCount);
    count
}

pub fn get_auction_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::AuctionCount)
        .unwrap_or(0)
}

pub fn increment_auction_count(env: &Env) -> u64 {
    let count = get_auction_count(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::AuctionCount, &count);
    bump_entry_ttl(env, &DataKey::AuctionCount);
    count
}

pub fn get_offer_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::OfferCount)
        .unwrap_or(0)
}

pub fn increment_offer_count(env: &Env) -> u64 {
    let count = get_offer_count(env) + 1;
    env.storage().persistent().set(&DataKey::OfferCount, &count);
    bump_entry_ttl(env, &DataKey::OfferCount);
    count
}

// ── CRUD methods ─────────────────────────────────────────────

pub fn save_listing(env: &Env, listing: &Listing) {
    let key = DataKey::Listing(listing.listing_id);
    env.storage().persistent().set(&key, listing);
    bump_entry_ttl(env, &key);
}

pub fn load_listing(env: &Env, listing_id: u64) -> Option<Listing> {
    let key = DataKey::Listing(listing_id);
    let res = env.storage().persistent().get::<DataKey, Listing>(&key);
    if res.is_some() {
        bump_entry_ttl(env, &key);
    }
    res
}

pub fn save_auction(env: &Env, auction: &Auction) {
    let key = DataKey::Auction(auction.auction_id);
    env.storage().persistent().set(&key, auction);
    bump_entry_ttl(env, &key);
}

pub fn load_auction(env: &Env, auction_id: u64) -> Option<Auction> {
    let key = DataKey::Auction(auction_id);
    let res = env.storage().persistent().get::<DataKey, Auction>(&key);
    if res.is_some() {
        bump_entry_ttl(env, &key);
    }
    res
}

pub fn save_offer(env: &Env, offer: &Offer) {
    let key = DataKey::Offer(offer.offer_id);
    env.storage().persistent().set(&key, offer);
    bump_entry_ttl(env, &key);
}

pub fn load_offer(env: &Env, offer_id: u64) -> Option<Offer> {
    let key = DataKey::Offer(offer_id);
    let res = env.storage().persistent().get::<DataKey, Offer>(&key);
    if res.is_some() {
        bump_entry_ttl(env, &key);
    }
    res
}

// ── Indices ──────────────────────────────────────────────────

pub fn add_artist_listing_id(env: &Env, artist: &Address, listing_id: u64) {
    let key = DataKey::ArtistListings(artist.clone());
    let mut ids = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(listing_id);
    env.storage().persistent().set(&key, &ids);
    bump_entry_ttl(env, &key);
}

pub fn get_artist_listing_ids(env: &Env, artist: &Address) -> Vec<u64> {
    let key = DataKey::ArtistListings(artist.clone());
    let value = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    if !value.is_empty() {
        bump_entry_ttl(env, &key);
    }
    value
}

// ── Active listings index ────────────────────────────────────

pub fn add_to_active_listings(env: &Env, listing_id: u64) {
    let key = DataKey::ActiveListings;
    let mut ids = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(listing_id);
    env.storage().persistent().set(&key, &ids);
    bump_entry_ttl(env, &key);
}

pub fn remove_from_active_listings(env: &Env, listing_id: u64) {
    let key = DataKey::ActiveListings;
    let ids = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    let mut updated = Vec::new(env);
    for id in ids.iter() {
        if id != listing_id {
            updated.push_back(id);
        }
    }
    env.storage().persistent().set(&key, &updated);
    bump_entry_ttl(env, &key);
}

pub fn get_active_listing_ids(env: &Env) -> Vec<u64> {
    let key = DataKey::ActiveListings;
    let value = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    // Always bump the active-listings index on read — it is a hot path accessed
    // every time get_active_listings / get_active_listings_page is called.
    bump_entry_ttl(env, &key);
    value
}

pub fn add_artist_auction_id(env: &Env, artist: &Address, auction_id: u64) {
    let key = DataKey::ArtistAuctions(artist.clone());
    let mut ids = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(auction_id);
    env.storage().persistent().set(&key, &ids);
    bump_entry_ttl(env, &key);
}

pub fn get_artist_auction_ids(env: &Env, artist: &Address) -> Vec<u64> {
    let key = DataKey::ArtistAuctions(artist.clone());
    let value = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    if !value.is_empty() {
        bump_entry_ttl(env, &key);
    }
    value
}

pub fn save_listing_offers(env: &Env, listing_id: u64, ids: &Vec<u64>) {
    let key = DataKey::ListingOffers(listing_id);
    env.storage().persistent().set(&key, ids);
    bump_entry_ttl(env, &key);
}

pub fn load_listing_offers(env: &Env, listing_id: u64) -> Vec<u64> {
    let key = DataKey::ListingOffers(listing_id);
    let value = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    // Bump on read — listing-offer indexes are accessed during every purchase and
    // cancellation path, making them hot entries prone to accidental eviction.
    if !value.is_empty() {
        bump_entry_ttl(env, &key);
    }
    value
}

pub fn save_offerer_offers(env: &Env, offerer: &Address, ids: &Vec<u64>) {
    let key = DataKey::OffererOffers(offerer.clone());
    env.storage().persistent().set(&key, ids);
    bump_entry_ttl(env, &key);
}

pub fn load_offerer_offers(env: &Env, offerer: &Address) -> Vec<u64> {
    let key = DataKey::OffererOffers(offerer.clone());
    let value = env
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    if !value.is_empty() {
        bump_entry_ttl(env, &key);
    }
    value
}

// ── Moderation & Configuration storage ────────────────────

pub fn set_artist_revocation_storage(env: &Env, artist: &Address) {
    let key = DataKey::RevokedArtist(artist.clone());
    env.storage().persistent().set(&key, &true);
    bump_entry_ttl(env, &key);
}

pub fn remove_artist_revocation_storage(env: &Env, artist: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::RevokedArtist(artist.clone()));
}

pub fn is_artist_revoked_storage(env: &Env, artist: &Address) -> bool {
    let key = DataKey::RevokedArtist(artist.clone());
    let revoked = env
        .storage()
        .persistent()
        .get::<_, bool>(&key)
        .unwrap_or(false);
    if revoked {
        bump_entry_ttl(env, &key);
    }
    revoked
}

pub fn set_treasury_storage(env: &Env, addr: &Address) {
    env.storage().persistent().set(&DataKey::Treasury, addr);
    bump_entry_ttl(env, &DataKey::Treasury);
}

pub fn get_treasury_storage(env: &Env) -> Option<Address> {
    let value = env.storage().persistent().get(&DataKey::Treasury);
    if value.is_some() {
        bump_entry_ttl(env, &DataKey::Treasury);
    }
    value
}

pub fn set_protocol_fee_bps_storage(env: &Env, bps: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::ProtocolFeeBps, &bps);
    bump_entry_ttl(env, &DataKey::ProtocolFeeBps);
}

pub fn get_protocol_fee_bps_storage(env: &Env) -> Option<u32> {
    let value = env.storage().persistent().get(&DataKey::ProtocolFeeBps);
    if value.is_some() {
        bump_entry_ttl(env, &DataKey::ProtocolFeeBps);
    }
    value
}

pub fn set_min_bid_increment_storage(env: &Env, increment: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::MinBidIncrement, &increment);
    bump_entry_ttl(env, &DataKey::MinBidIncrement);
}

pub fn get_min_bid_increment_storage(env: &Env) -> Option<i128> {
    let value = env.storage().persistent().get(&DataKey::MinBidIncrement);
    if value.is_some() {
        bump_entry_ttl(env, &DataKey::MinBidIncrement);
    }
    value
}

// ── Reentrancy Guards ────────────────────────────────────────

pub fn acquire_listing_lock(env: &Env, listing_id: u64) -> bool {
    let key = DataKey::ListingLock(listing_id);
    if env.storage().temporary().has(&key) {
        return false;
    }
    env.storage().temporary().set(&key, &true);
    env.storage()
        .temporary()
        .extend_ttl(&key, REENTRANCY_LOCK_TTL, REENTRANCY_LOCK_TTL);
    true
}

pub fn release_listing_lock(env: &Env, listing_id: u64) {
    let key = DataKey::ListingLock(listing_id);
    env.storage().temporary().remove(&key);
}

pub fn acquire_auction_lock(env: &Env, auction_id: u64) -> bool {
    let key = DataKey::AuctionLock(auction_id);
    if env.storage().temporary().has(&key) {
        return false;
    }
    env.storage().temporary().set(&key, &true);
    env.storage()
        .temporary()
        .extend_ttl(&key, REENTRANCY_LOCK_TTL, REENTRANCY_LOCK_TTL);
    true
}

pub fn release_auction_lock(env: &Env, auction_id: u64) {
    let key = DataKey::AuctionLock(auction_id);
    env.storage().temporary().remove(&key);
}

// ── Admin transfer helpers ───────────────────────────────────

pub fn set_pending_admin_storage(env: &Env, pending: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::PendingAdmin, pending);
    bump_entry_ttl(env, &DataKey::PendingAdmin);
}

pub fn get_pending_admin_storage(env: &Env) -> Option<Address> {
    let value = env.storage().persistent().get(&DataKey::PendingAdmin);
    if value.is_some() {
        bump_entry_ttl(env, &DataKey::PendingAdmin);
    }
    value
}

pub fn clear_pending_admin_storage(env: &Env) {
    env.storage().persistent().remove(&DataKey::PendingAdmin);
}

// ── Pause/Unpause Mechanism ──────────────────────────────────

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().persistent().set(&DataKey::IsPaused, &paused);
    bump_entry_ttl(env, &DataKey::IsPaused);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::IsPaused)
        .unwrap_or(false)
}
