// types.rs
use soroban_sdk::{contracterror, contracttype, Address, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MarketplaceError {
    InvalidCid = 1,
    InvalidPrice = 2,
    ListingNotFound = 3,
    ListingNotActive = 4,
    Unauthorized = 5,
    CannotBuyOwnListing = 6,
    InvalidSplit = 7,
    TooManyRecipients = 8,
    AuctionNotFound = 9,
    AuctionNotActive = 10,
    BidTooLow = 11,
    AuctionExpired = 12,
    AuctionNotExpired = 13,
    AuctionAlreadyFinalized = 14,
    ArtistRevoked = 15,
    OfferNotFound = 16,
    CannotOfferOwnListing = 17,
    OfferNotPending = 18,
    InsufficientOfferAmount = 19,
    ListingSold = 20,
    ListingCancelled = 21,
    ReentrancyGuard = 22,
    ContractPaused = 23,
    /// Royalty bps greater than 10000 (100%) — rejects create_listing/create_auction
    InvalidRoyalty = 24,
    /// Token attempted at purchase time but is no longer whitelisted
    TokenNotWhitelisted = 25,
    /// The sum of all Recipient basis-point values plus the protocol fee exceeds
    /// 10 000 bps (100%).  Rejected at listing creation and on any update that
    /// would mutate recipients, so an invalid split can never be persisted.
    RoyaltyExceedsLimit = 26,
    /// The listing has passed its `expires_at` ledger timestamp and can no
    /// longer be purchased or updated.
    ListingExpired = 27,
    /// `expire_listing` was called on a listing whose `expires_at` is still in
    /// the future (or the listing has no expiry).
    ListingNotExpired = 28,
    /// `finalize_auction` was called before `end_time` has passed.
    AuctionNotEnded = 29,
    /// `cancel_auction` was called on an auction that already has at least one
    /// bid — cancelling would strand the bidder's escrowed funds.
    AuctionHasBids = 30,
    /// `create_auction` was called with an `end_time` (or `duration`) that is in
    /// the past or shorter than `MIN_AUCTION_DURATION`.
    InvalidAuctionDuration = 31,
    /// `place_bid` was called by the auction creator — self-bidding (shill
    /// bidding) is not allowed.  The bidder address must differ from the
    /// auction's `creator` field.
    SelfBidNotAllowed = 32,
    /// An offer state transition was attempted from a terminal state (Accepted,
    /// Rejected, or Withdrawn), or from Pending with the wrong authorizer.
    InvalidOfferState = 33,
    /// `accept_offer` called after the offer's `expires_at` has passed; or
    /// `reclaim_offer` called before expiry / on a non-expiring offer.
    OfferExpired = 34,
    /// A new offer would exceed MAX_OFFERS_PER_LISTING active (Pending) offers
    /// for this listing.  A cap bounds per-listing storage growth and keeps the
    /// auto-reject sweep (ISSUE-031) economically viable.
    OfferLimitReached = 35,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
}

/// Discriminant carried in the ListingCancelledEvent to indicate why a listing
/// was cancelled. This improves provenance display and analytics for indexers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CancelReason {
    /// The listing owner (artist) explicitly cancelled the listing
    Owner = 1,
    /// The listing expired (time-based expiry, if implemented)
    Expired = 2,
    /// Admin revoked the artist's permission, causing automatic cancellation
    AdminRevoked = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recipient {
    pub address: Address,
    /// Share expressed in basis points (0 – 10 000).
    /// The sum of all recipient `percentage` values plus the protocol fee bps
    /// must not exceed 10 000 (100 %).
    pub percentage: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Listing {
    pub listing_id: u64,
    pub artist: Address,
    pub price: i128,
    pub currency: Symbol,
    pub token: Address,
    pub collection: Address,
    pub token_id: u64,
    pub recipients: soroban_sdk::Vec<Recipient>,
    pub status: ListingStatus,
    pub owner: Option<Address>,
    pub created_at: u32,
    /// Protocol fee in basis points (0-10000) snapshotted at listing creation.
    /// This ensures the fee applied at purchase matches what was displayed when
    /// the listing was created, regardless of subsequent admin fee changes.
    pub protocol_fee_bps: u32,
    /// Optional expiry as a Unix ledger timestamp (seconds since epoch).
    /// When `Some(t)` and `env.ledger().timestamp() >= t`, the listing is
    /// considered expired and cannot be purchased.  `None` means no expiry
    /// (the listing lives until cancelled or sold).  Listings created before
    /// this field was introduced will deserialise as `None` automatically.
    pub expires_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuctionStatus {
    Active,
    Finalized,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Auction {
    pub auction_id: u64,
    pub creator: Address,
    pub token: Address,
    pub collection: Address,
    pub token_id: u64,
    pub reserve_price: i128,
    pub highest_bid: i128,
    pub highest_bidder: Option<Address>,
    pub end_time: u64,
    pub status: AuctionStatus,
    pub recipients: soroban_sdk::Vec<Recipient>,
    /// Minimum amount by which a new bid must exceed the current highest bid,
    /// snapshotted from the global setting at auction creation. The first bid is
    /// instead gated by `reserve_price`.
    pub min_increment: i128,
    /// How many seconds to extend the auction when a qualifying late bid arrives.
    /// Snapshotted from the global setting at auction creation time.
    pub extension_window: u64,
    /// If `end_time - now < extension_trigger` seconds at bid time, the auction
    /// end is extended by `extension_window`. Snapshotted at creation time.
    pub extension_trigger: u64,
    /// Protocol fee in basis points snapshotted from the global setting at
    /// auction creation time. This ensures settlement math is fixed when the
    /// auction is created, giving bidders and the creator certainty about the
    /// net payout — consistent with how listings behave.
    pub protocol_fee_bps: u32,
}

/// A single entry in the per-auction bounded bid history.
///
/// The history is capped to `BID_HISTORY_CAP` entries (see `contract.rs`).
/// When the cap is reached the oldest entry is evicted so only the most
/// recent N bids are ever persisted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidRecord {
    /// The account that placed this bid.
    pub bidder: Address,
    /// The bid amount (in the auction's payment token stroops).
    pub amount: i128,
    /// The ledger sequence number at which this bid was recorded.
    pub ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OfferStatus {
    Pending,
    Accepted,
    Rejected,
    Withdrawn,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Offer {
    pub offer_id: u64,
    pub listing_id: u64,
    pub offerer: Address,
    pub amount: i128,
    pub token: Address,
    pub status: OfferStatus,
    pub created_at: u32,
    /// Optional expiry (Unix timestamp, seconds). When `Some(t)` and the
    /// ledger timestamp >= t: `accept_offer` reverts, anyone may call
    /// `reclaim_offer` to refund the offerer.  `None` = no expiry.
    pub expires_at: Option<u64>,
}
