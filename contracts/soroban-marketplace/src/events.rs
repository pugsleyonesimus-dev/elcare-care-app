// events.rs — Defines all contract event schemas for ELCARE-HUB Marketplace

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// Versioned event topics as Symbol constants
pub const LISTING_CREATED: Symbol = symbol_short!("lst_crtd");
pub const ARTWORK_SOLD: Symbol = symbol_short!("art_sold");
pub const LISTING_CANCELLED: Symbol = symbol_short!("lst_cncl");
pub const LISTING_UPDATED: Symbol = symbol_short!("lst_updt");
pub const BID_PLACED: Symbol = symbol_short!("bid_plcd");
pub const AUCTION_RESOLVED: Symbol = symbol_short!("auc_rslv");
pub const AUCTION_CREATED: Symbol = symbol_short!("auc_crtd");
pub const OFFER_MADE: Symbol = symbol_short!("ofr_made");
pub const OFFER_ACCEPTED: Symbol = symbol_short!("ofr_accp");
pub const OFFER_REJECTED: Symbol = symbol_short!("ofr_rjct");
pub const OFFER_WITHDRAWN: Symbol = symbol_short!("ofr_wdrn");
pub const ROYALTY_PAID: Symbol = symbol_short!("roy_paid");
pub const ADMIN_TRANSFER_PROPOSED: Symbol = symbol_short!("adm_prop");
pub const ADMIN_TRANSFERRED: Symbol = symbol_short!("adm_xfrd");
pub const ARTIST_REVOKED: Symbol = symbol_short!("art_rvkd");
pub const ARTIST_REINSTATED: Symbol = symbol_short!("art_rnst");
pub const CONTRACT_PAUSED: Symbol = symbol_short!("ctr_psd");
pub const CONTRACT_UNPAUSED: Symbol = symbol_short!("ctr_unpsd");
pub const LISTING_PRICE_UPDATED: Symbol = symbol_short!("lst_pru");
pub const LISTING_EXPIRED: Symbol = symbol_short!("lst_expd");
pub const AUCTION_EXTENDED: Symbol = symbol_short!("auc_ext");
pub const AUCTION_CANCELLED: Symbol = symbol_short!("auc_cncl");
pub const PROTOCOL_FEE_COLLECTED: Symbol = symbol_short!("fee_cltd");
pub const OFFER_RECLAIMED: Symbol = symbol_short!("ofr_rclm");

// Event data structs
// Event data structs
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCreatedEvent {
    pub listing_id: u64,
    pub artist: Address,
    pub price: i128,
    pub currency: Symbol,
    pub collection: Address,
    pub token_id: u64,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtworkSoldEvent {
    pub listing_id: u64,
    pub artist: Address,
    pub buyer: Address,
    pub price: i128,
    pub currency: Symbol,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCancelledEvent {
    pub listing_id: u64,
    /// The actor that triggered the cancellation (may be the artist, admin, or contract).
    pub cancelled_by: Address,
    /// Discriminant indicating the reason for cancellation (Owner, Expired, AdminRevoked).
    pub reason: crate::types::CancelReason,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingUpdatedEvent {
    pub listing_id: u64,
    pub artist: Address,
    pub new_price: i128,
    pub collection: Address,
    pub token_id: u64,
    pub ledger_sequence: u32,
}

// Add more event structs as needed for other actions
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionCreatedEvent {
    pub auction_id: u64,
    pub creator: Address,
    pub reserve_price: i128,
    pub token: Address,
    pub collection: Address,
    pub token_id: u64,
    pub end_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidPlacedEvent {
    pub auction_id: u64,
    pub bidder: Address,
    pub bid_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionFinalizedEvent {
    pub auction_id: u64,
    pub winner: Option<Address>,
    pub amount: i128,
}

impl ListingCreatedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((LISTING_CREATED,), self);
    }
}

impl ArtworkSoldEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((ARTWORK_SOLD,), self);
    }
}

impl ListingCancelledEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((LISTING_CANCELLED,), self);
    }
}

impl AuctionCreatedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((AUCTION_CREATED,), self);
    }
}

impl BidPlacedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((BID_PLACED,), self);
    }
}

impl AuctionFinalizedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((AUCTION_RESOLVED,), self);
    }
}

/// Emitted when a qualifying late bid triggers the anti-sniping extension rule.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionExtendedEvent {
    pub auction_id: u64,
    /// The new end time after the extension has been applied.
    pub new_end_time: u64,
}

impl AuctionExtendedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((AUCTION_EXTENDED,), self);
    }
}

/// Emitted when a creator cancels an auction that has received no bids.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionCancelledEvent {
    pub auction_id: u64,
    pub cancelled_by: Address,
}

impl AuctionCancelledEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((AUCTION_CANCELLED,), self);
    }
}

impl ListingUpdatedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((LISTING_UPDATED,), self);
    }
}

/// Emitted when a seller updates the price of an active listing in-place via
/// `update_listing_price`.  Both the old and new price are recorded so that
/// indexers can reconstruct the full price history of every listing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingPriceUpdatedEvent {
    pub listing_id: u64,
    pub old_price: i128,
    pub new_price: i128,
    pub updated_by: Address,
}

/// Emitted when anyone calls `expire_listing` on a genuinely expired listing,
/// transitioning it from Active → Expired/Cancelled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingExpiredEvent {
    pub listing_id: u64,
    pub expired_at: u64,
    pub ledger_sequence: u32,
}

impl ListingPriceUpdatedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((LISTING_PRICE_UPDATED,), self);
    }
}

impl ListingExpiredEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((LISTING_EXPIRED,), self);
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferMadeEvent {
    pub offer_id: u64,
    pub listing_id: u64,
    pub offerer: Address,
    pub amount: i128,
    pub token: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferAcceptedEvent {
    pub offer_id: u64,
    pub listing_id: u64,
    pub offerer: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferRejectedEvent {
    pub offer_id: u64,
    pub listing_id: u64,
    pub offerer: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferWithdrawnEvent {
    pub offer_id: u64,
    pub listing_id: u64,
    pub offerer: Address,
}

impl OfferMadeEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((OFFER_MADE,), self);
    }
}

impl OfferAcceptedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((OFFER_ACCEPTED,), self);
    }
}

impl OfferRejectedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((OFFER_REJECTED,), self);
    }
}

impl OfferWithdrawnEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((OFFER_WITHDRAWN,), self);
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistRevokedEvent {
    pub artist: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistReinstatedEvent {
    pub artist: Address,
}

impl ArtistRevokedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((ARTIST_REVOKED,), self);
    }
}

impl ArtistReinstatedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((ARTIST_REINSTATED,), self);
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferProposedEvent {
    pub current_admin: Address,
    pub proposed_admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferredEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

impl AdminTransferProposedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((ADMIN_TRANSFER_PROPOSED,), self);
    }
}

impl AdminTransferredEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((ADMIN_TRANSFERRED,), self);
    }
}

// ── Protocol Fee Event ────────────────────────────────────────────────────────
//
// Emitted from every settlement path (buy_artwork, finalize_auction,
// accept_offer) so the treasury's revenue is independently observable
// on-chain without requiring indexer inference.

/// Emitted once per settlement with the exact protocol-fee amount transferred
/// to the treasury.  Carries enough context to identify the originating trade
/// and reconcile treasury balances in real time.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolFeeCollectedEvent {
    /// ID of the listing (for buy_artwork / accept_offer) or auction
    /// (for finalize_auction) that generated the fee.
    pub listing_id: u64,
    /// Raw token amount transferred to the treasury.  Zero when no treasury is
    /// configured or when the computed fee rounds down to zero.
    pub amount: i128,
    /// The payment token from which the fee was deducted.
    pub token: Address,
    /// The treasury address that received the fee.
    pub treasury: Address,
}

impl ProtocolFeeCollectedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((PROTOCOL_FEE_COLLECTED,), self);
    }
}

// End of events

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferReclaimedEvent {
    pub offer_id: u64,
    pub listing_id: u64,
    pub offerer: Address,
    pub amount: i128,
}

impl OfferReclaimedEvent {
    #[allow(deprecated)]
    pub fn publish(self, env: &Env) {
        env.events().publish((OFFER_RECLAIMED,), self);
    }
}
