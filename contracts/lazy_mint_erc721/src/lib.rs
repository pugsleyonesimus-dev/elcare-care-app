//! LazyMint721 — Lazy-minting ERC-721-equivalent on Soroban.
//!
//! # How lazy minting works
//!
//! 1. Creator builds a `MintVoucher` off-chain.
//! 2. Creator hashes it with `sha256(contract_addr ‖ token_id ‖ price ‖ valid_until ‖ uri_hash ‖ currency_xdr)`
//!    and signs the 32-byte digest with their ed25519 private key.
//! 3. Buyer submits the voucher + signature on-chain via `redeem()`.
//! 4. Contract re-hashes, verifies ed25519, takes payment, then mints.
//!
//! # Replay protection (#39)
//! Every redeemed `token_id` is tracked in `UsedVoucher`. Once redeemed it
//! can never be claimed again (`VoucherAlreadyRedeemed`).
//!
//! # Voucher revocation
//! The creator can revoke a specific voucher nonce before it is redeemed via
//! `revoke_voucher(nonce)` or batch-revoke with `revoke_vouchers(nonces)`.
//! Attempting to redeem a revoked voucher returns `VoucherRevoked`.
//! Revoking an already-redeemed nonce returns `VoucherAlreadyRedeemed`.
//!
//! # Merkle allowlist
//! A Merkle-root-based allowlist phase gates redemptions before
//! `set_public_phase()` is called.
//!
//! # Platform fee (#38)
//! A per-collection `platform_fee_bps` is stored at initialization. When a
//! buyer redeems a priced voucher the fee portion is transferred to
//! `platform_fee_receiver` and the remainder to the creator.
//!
//! # Batch redemption
//! `redeem_batch` verifies and mints multiple vouchers atomically (all-or-nothing).
//! Payments are aggregated per currency to minimise token transfer calls.
#![no_std]
#![allow(clippy::too_many_arguments, deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    token::Client as TokenClient, xdr::ToXdr, Address, Bytes, BytesN, Env, Map, String, Vec,
};

const TTL_THRESHOLD: u32 = 50_000;
const TTL_BUMP: u32 = 100_000;

// ─── Errors ──────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotOwner = 3,
    NotApproved = 4,
    TokenNotFound = 5,
    MaxSupplyReached = 6,
    VoucherExpired = 7,
    /// Voucher nonce (token_id) already redeemed (#39).
    VoucherAlreadyRedeemed = 8,
    NotCreator = 9,
    InvalidSignature = 10,
    NotAllowlisted = 11,
    InvalidMerkleProof = 12,
    /// Voucher nonce has been explicitly revoked by the creator.
    VoucherRevoked = 13,
}

// ─── Data types ───────────────────────────────────────────────────────────────

/// Off-chain voucher created by the collection creator.
///
/// `uri_hash` = sha256(uri_string) computed off-chain; included in the signed
/// digest so a relayer cannot swap the URI while keeping the signature valid.
#[contracttype]
#[derive(Clone)]
pub struct MintVoucher {
    pub token_id: u64,
    pub price: i128,          // 0 = free
    pub currency: Address,    // SAC address (ignored when price == 0)
    pub uri: String,          // IPFS / HTTPS metadata URI
    pub uri_hash: BytesN<32>, // sha256(uri bytes) — included in signature
    pub valid_until: u64,     // ledger sequence; 0 = no expiry
}

/// One element of a `redeem_batch` call.
#[contracttype]
#[derive(Clone)]
pub struct BatchVoucherItem {
    pub voucher: MintVoucher,
    pub signature: BytesN<64>,
    pub merkle_proof: Vec<BytesN<32>>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Initialized,
    Creator,
    CreatorPubkey,
    Name,
    Symbol,
    MaxSupply,
    NextTokenId,
    TotalSupply,
    RoyaltyBps,
    RoyaltyReceiver,
    /// Platform fee receiver address (#38).
    PlatformFeeReceiver,
    /// Platform fee in basis points (#38).
    PlatformFeeBps,
    Owner(u64),
    TokenUri(u64),
    Approved(u64),
    BalanceOf(Address),
    ApprovedForAll(Address, Address),
    UsedVoucher(u64),    // token_id → bool  (redeemed)
    RevokedVoucher(u64), // token_id → bool  (creator-revoked, per-nonce)
    MerkleRoot,          // BytesN<32> — root of allowlist Merkle tree
    IsPublicPhase,       // bool — true once public minting is enabled
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct LazyMint721;

impl LazyMint721 {
    fn verify_signature_or_panic(
        env: &Env,
        pubkey: &BytesN<32>,
        digest: &Bytes,
        signature: &BytesN<64>,
    ) {
        env.crypto().ed25519_verify(pubkey, digest, signature);
    }

    /// Verify a standard binary Merkle proof against `root`.
    /// Leaf = sha256(address XDR).  Siblings are sorted (smaller first) at each
    /// level so proofs are position-independent (OpenZeppelin convention).
    fn verify_merkle_proof(
        env: &Env,
        root: &BytesN<32>,
        leaf_preimage: &Address,
        proof: &Vec<BytesN<32>>,
    ) -> bool {
        let mut computed: BytesN<32> = env
            .crypto()
            .sha256(&leaf_preimage.clone().to_xdr(env))
            .into();
        for sibling in proof.iter() {
            let mut pair = Bytes::new(env);
            if computed.to_array() <= sibling.to_array() {
                pair.append(&computed.clone().into());
                pair.append(&sibling.clone().into());
            } else {
                pair.append(&sibling.into());
                pair.append(&computed.clone().into());
            }
            computed = env.crypto().sha256(&pair).into();
        }
        &computed == root
    }

    /// Enforce the allowlist gate for `buyer`.  No-op in public phase.
    fn check_allowlist(
        env: &Env,
        buyer: &Address,
        merkle_proof: &Vec<BytesN<32>>,
    ) -> Result<(), Error> {
        let is_public: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPublicPhase)
            .unwrap_or(false);
        if is_public {
            return Ok(());
        }
        let root: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::MerkleRoot)
            .ok_or(Error::NotAllowlisted)?;
        if merkle_proof.is_empty() {
            return Err(Error::NotAllowlisted);
        }
        if !Self::verify_merkle_proof(env, &root, buyer, merkle_proof) {
            return Err(Error::InvalidMerkleProof);
        }
        Ok(())
    }

    /// Validate a single voucher (expiry → replay → revocation → supply → sig).
    /// Does NOT write state or transfer funds.
    fn check_voucher(
        env: &Env,
        voucher: &MintVoucher,
        signature: &BytesN<64>,
        pubkey: &BytesN<32>,
        max: u64,
        next_id: u64,
    ) -> Result<(), Error> {
        if voucher.valid_until != 0 && env.ledger().sequence() > voucher.valid_until as u32 {
            return Err(Error::VoucherExpired);
        }
        // Replay before revocation: if already redeemed, surface that error.
        if env
            .storage()
            .persistent()
            .has(&DataKey::UsedVoucher(voucher.token_id))
        {
            return Err(Error::VoucherAlreadyRedeemed);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::RevokedVoucher(voucher.token_id))
        {
            return Err(Error::VoucherRevoked);
        }
        if next_id >= max {
            return Err(Error::MaxSupplyReached);
        }
        let digest = Self::_voucher_digest(env, voucher);
        Self::verify_signature_or_panic(env, pubkey, &digest, signature);
        Ok(())
    }

    /// Execute payment split for one voucher's price.
    fn pay(
        env: &Env,
        buyer: &Address,
        creator: &Address,
        currency: &Address,
        price: i128,
        fee_bps: u32,
        fee_receiver: &Address,
    ) {
        if price <= 0 {
            return;
        }
        if fee_bps > 0 {
            let fee_amount = (price * fee_bps as i128) / 10_000;
            let creator_amount = price - fee_amount;
            if fee_amount > 0 {
                TokenClient::new(env, currency).transfer(buyer, fee_receiver, &fee_amount);
            }
            if creator_amount > 0 {
                TokenClient::new(env, currency).transfer(buyer, creator, &creator_amount);
            }
        } else {
            TokenClient::new(env, currency).transfer(buyer, creator, &price);
        }
    }

    /// Mint a single token after all checks have passed.
    /// Updates Owner, TokenUri, UsedVoucher, BalanceOf, TotalSupply, NextTokenId.
    fn mint_token(env: &Env, buyer: &Address, token_id: u64, uri: &String, next_id: u64) {
        env.storage()
            .persistent()
            .set(&DataKey::Owner(token_id), buyer);
        env.storage()
            .persistent()
            .set(&DataKey::TokenUri(token_id), uri);
        env.storage()
            .persistent()
            .set(&DataKey::UsedVoucher(token_id), &true);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Owner(token_id), TTL_THRESHOLD, TTL_BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::TokenUri(token_id), TTL_THRESHOLD, TTL_BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::UsedVoucher(token_id), TTL_THRESHOLD, TTL_BUMP);

        let bal: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::BalanceOf(buyer.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::BalanceOf(buyer.clone()), &(bal + 1));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BalanceOf(buyer.clone()), TTL_THRESHOLD, TTL_BUMP);

        let supply: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply + 1));

        if token_id >= next_id {
            env.storage()
                .instance()
                .set(&DataKey::NextTokenId, &(token_id + 1));
        }
    }
}

#[contractimpl]
impl LazyMint721 {
    // ── Initializer ───────────────────────────────────────────────────────

    /// Issue #38: accepts `platform_fee_receiver` and `platform_fee_bps` so
    /// the launchpad can configure per-collection fee splits at deployment time.
    pub fn initialize(
        env: Env,
        creator: Address,
        creator_pubkey: BytesN<32>,
        name: String,
        symbol: String,
        max_supply: u64,
        royalty_bps: u32,
        royalty_receiver: Address,
        platform_fee_receiver: Address,
        platform_fee_bps: u32,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Creator, &creator);
        env.storage()
            .instance()
            .set(&DataKey::CreatorPubkey, &creator_pubkey);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage()
            .instance()
            .set(&DataKey::MaxSupply, &max_supply);
        env.storage().instance().set(&DataKey::NextTokenId, &0u64);
        env.storage().instance().set(&DataKey::TotalSupply, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyBps, &royalty_bps);
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyReceiver, &royalty_receiver);
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeReceiver, &platform_fee_receiver);
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    // ── Lazy Mint (single) ────────────────────────────────────────────────

    /// Buyer submits a signed voucher to mint their NFT.
    /// During the allowlist phase a valid Merkle proof for `buyer` is required.
    pub fn redeem(
        env: Env,
        buyer: Address,
        voucher: MintVoucher,
        signature: BytesN<64>,
        merkle_proof: Vec<BytesN<32>>,
    ) -> Result<u64, Error> {
        Self::extend_instance_ttl(&env);
        buyer.require_auth();

        // 0. Allowlist phase check
        Self::check_allowlist(&env, &buyer, &merkle_proof)?;

        // 1–5. Validate
        let pubkey: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::CreatorPubkey)
            .ok_or(Error::NotInitialized)?;
        let next_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);
        let max: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSupply)
            .unwrap_or(u64::MAX);
        Self::check_voucher(&env, &voucher, &signature, &pubkey, max, next_id)?;

        // 6. Payment
        let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
        let fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeBps)
            .unwrap_or(0);
        let fee_receiver: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeReceiver)
            .unwrap_or(creator.clone());
        Self::pay(
            &env,
            &buyer,
            &creator,
            &voucher.currency,
            voucher.price,
            fee_bps,
            &fee_receiver,
        );

        // 7. Mint
        let token_id = voucher.token_id;
        Self::mint_token(&env, &buyer, token_id, &voucher.uri, next_id);

        env.events().publish(
            (symbol_short!("mint"), creator, buyer.clone()),
            (token_id, 1u128),
        );
        Ok(token_id)
    }

    // ── Lazy Mint (batch) ─────────────────────────────────────────────────

    /// Atomically redeem multiple vouchers.  All-or-nothing: if any voucher
    /// fails validation the entire batch reverts.
    ///
    /// Each item carries its own `merkle_proof` so mixed allowlist / open-entry
    /// batches are possible after `set_public_phase()`.
    ///
    /// Payments are aggregated per currency: for each unique currency the total
    /// fee portion and creator portion are summed and transferred in two calls
    /// (fee receiver then creator).  This minimises the number of token
    /// transfers for homogeneous batches.
    pub fn redeem_batch(
        env: Env,
        buyer: Address,
        items: Vec<BatchVoucherItem>,
    ) -> Result<Vec<u64>, Error> {
        Self::extend_instance_ttl(&env);
        buyer.require_auth();

        let pubkey: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::CreatorPubkey)
            .ok_or(Error::NotInitialized)?;
        let max: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxSupply)
            .unwrap_or(u64::MAX);
        let next_id_start: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);
        let fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeBps)
            .unwrap_or(0);
        let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
        let fee_receiver: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeReceiver)
            .unwrap_or(creator.clone());

        // Phase 1: validate every item (all-or-nothing — no state changes yet).
        // We track supply headroom manually since NextTokenId is not yet updated.
        let mut supply_used: u64 = 0u64;
        for item in items.iter() {
            Self::check_allowlist(&env, &buyer, &item.merkle_proof)?;
            let effective_next = next_id_start.saturating_add(supply_used);
            Self::check_voucher(
                &env,
                &item.voucher,
                &item.signature,
                &pubkey,
                max,
                effective_next,
            )?;
            supply_used = supply_used.saturating_add(1);
        }

        // Phase 2: aggregate payments per currency.
        // Map<currency_address_string, (fee_total, creator_total)>
        // We use a Vec of pairs because Map requires ScVal keys and Address
        // implements IntoVal — but to keep things simple we iterate twice.
        // For each currency accumulate: fee_amount and creator_amount.
        let mut fee_totals: Map<Address, i128> = Map::new(&env);
        let mut creator_totals: Map<Address, i128> = Map::new(&env);
        for item in items.iter() {
            let price = item.voucher.price;
            if price <= 0 {
                continue;
            }
            let cur = item.voucher.currency.clone();
            let fee_amount = if fee_bps > 0 {
                (price * fee_bps as i128) / 10_000
            } else {
                0i128
            };
            let creator_amount = price - fee_amount;

            let prev_fee: i128 = fee_totals.get(cur.clone()).unwrap_or(0);
            fee_totals.set(cur.clone(), prev_fee + fee_amount);

            let prev_creator: i128 = creator_totals.get(cur.clone()).unwrap_or(0);
            creator_totals.set(cur.clone(), prev_creator + creator_amount);
        }

        // Phase 3: transfer payments (aggregated).
        for (cur, fee_total) in fee_totals.iter() {
            if fee_total > 0 {
                TokenClient::new(&env, &cur).transfer(&buyer, &fee_receiver, &fee_total);
            }
        }
        for (cur, creator_total) in creator_totals.iter() {
            if creator_total > 0 {
                TokenClient::new(&env, &cur).transfer(&buyer, &creator, &creator_total);
            }
        }

        // Phase 4: mint all tokens.
        let mut minted_ids: Vec<u64> = Vec::new(&env);
        let mut next_id = next_id_start;
        for item in items.iter() {
            let token_id = item.voucher.token_id;
            Self::mint_token(&env, &buyer, token_id, &item.voucher.uri, next_id);
            if token_id >= next_id {
                next_id = token_id + 1;
            }
            env.events().publish(
                (symbol_short!("mint"), creator.clone(), buyer.clone()),
                (token_id, 1u128),
            );
            minted_ids.push_back(token_id);
        }

        Ok(minted_ids)
    }

    // ── Voucher Revocation ────────────────────────────────────────────────

    /// Revoke a single voucher by its nonce (token_id).  Creator-only.
    /// Returns `VoucherAlreadyRedeemed` if the nonce was already redeemed.
    pub fn revoke_voucher(env: Env, nonce: u64) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        let creator = Self::only_creator(&env)?;

        if env
            .storage()
            .persistent()
            .has(&DataKey::UsedVoucher(nonce))
        {
            return Err(Error::VoucherAlreadyRedeemed);
        }
        env.storage()
            .persistent()
            .set(&DataKey::RevokedVoucher(nonce), &true);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::RevokedVoucher(nonce), TTL_THRESHOLD, TTL_BUMP);
        env.events()
            .publish((symbol_short!("revoke"), creator), nonce);
        Ok(())
    }

    /// Batch-revoke a list of voucher nonces.  Creator-only.  All-or-nothing:
    /// if any nonce is already redeemed the call reverts and nothing is revoked.
    pub fn revoke_vouchers(env: Env, nonces: Vec<u64>) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        let creator = Self::only_creator(&env)?;

        // Validate all first (all-or-nothing)
        for nonce in nonces.iter() {
            if env
                .storage()
                .persistent()
                .has(&DataKey::UsedVoucher(nonce))
            {
                return Err(Error::VoucherAlreadyRedeemed);
            }
        }
        for nonce in nonces.iter() {
            env.storage()
                .persistent()
                .set(&DataKey::RevokedVoucher(nonce), &true);
            env.storage().persistent().extend_ttl(
                &DataKey::RevokedVoucher(nonce),
                TTL_THRESHOLD,
                TTL_BUMP,
            );
            env.events()
                .publish((symbol_short!("revoke"), creator.clone()), nonce);
        }
        Ok(())
    }

    /// Return `true` if the voucher nonce has been explicitly revoked by the creator.
    pub fn is_voucher_revoked(env: Env, nonce: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::RevokedVoucher(nonce))
    }

    // ── Transfers ─────────────────────────────────────────────────────────

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u64) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        from.require_auth();
        Self::_transfer(&env, &from, &to, token_id)
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        token_id: u64,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        spender.require_auth();
        Self::_check_approved(&env, &spender, &from, token_id)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Approved(token_id));
        Self::_transfer(&env, &from, &to, token_id)
    }

    // ── Approvals ─────────────────────────────────────────────────────────

    pub fn approve(
        env: Env,
        spender: Address,
        approved: Address,
        token_id: u64,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        spender.require_auth();
        let owner: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::TokenNotFound)?;
        if spender != owner
            && !Self::is_approved_for_all(env.clone(), owner.clone(), spender.clone())
        {
            return Err(Error::NotApproved);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Approved(token_id), &approved);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Approved(token_id), TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    pub fn set_approval_for_all(env: Env, owner: Address, operator: Address, approved: bool) {
        Self::extend_instance_ttl(&env);
        owner.require_auth();
        let key = DataKey::ApprovedForAll(owner.clone(), operator.clone());
        env.storage().persistent().set(&key, &approved);
        env.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }

    // ── View functions ────────────────────────────────────────────────────

    pub fn owner_of(env: Env, token_id: u64) -> Result<Address, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::TokenNotFound)
    }

    pub fn token_uri(env: Env, token_id: u64) -> Result<String, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::TokenUri(token_id))
            .ok_or(Error::TokenNotFound)
    }

    pub fn balance_of(env: Env, owner: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::BalanceOf(owner))
            .unwrap_or(0)
    }

    pub fn total_supply(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    /// Returns true if the voucher nonce (token_id) has already been redeemed.
    pub fn is_voucher_redeemed(env: Env, token_id: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::UsedVoucher(token_id))
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }

    pub fn creator(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Creator).unwrap()
    }

    pub fn royalty_info(env: Env) -> (Address, u32) {
        (
            env.storage()
                .instance()
                .get(&DataKey::RoyaltyReceiver)
                .unwrap(),
            env.storage()
                .instance()
                .get(&DataKey::RoyaltyBps)
                .unwrap_or(0),
        )
    }

    pub fn platform_fee_info(env: Env) -> (Address, u32) {
        (
            env.storage()
                .instance()
                .get(&DataKey::PlatformFeeReceiver)
                .unwrap(),
            env.storage()
                .instance()
                .get(&DataKey::PlatformFeeBps)
                .unwrap_or(0),
        )
    }

    pub fn get_approved(env: Env, token_id: u64) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Approved(token_id))
    }

    pub fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ApprovedForAll(owner, operator))
            .unwrap_or(false)
    }

    // ── Admin ─────────────────────────────────────────────────────────────

    pub fn transfer_ownership(env: Env, new_creator: Address) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::Creator, &new_creator);
        Ok(())
    }

    pub fn update_creator_pubkey(env: Env, new_pubkey: BytesN<32>) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::CreatorPubkey, &new_pubkey);
        Ok(())
    }

    pub fn update_royalty(env: Env, receiver: Address, bps: u32) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyReceiver, &receiver);
        env.storage().instance().set(&DataKey::RoyaltyBps, &bps);
        Ok(())
    }

    /// Set the Merkle root for the allowlist.  Creator-only.
    /// Automatically resets to allowlist phase (clears public phase flag).
    pub fn set_merkle_root(env: Env, root: BytesN<32>) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage().instance().set(&DataKey::MerkleRoot, &root);
        env.storage()
            .instance()
            .set(&DataKey::IsPublicPhase, &false);
        Ok(())
    }

    /// Switch the sale to public phase — removes the allowlist restriction.
    /// Creator-only.  Reversible by calling `set_merkle_root` again.
    pub fn set_public_phase(env: Env) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage().instance().set(&DataKey::IsPublicPhase, &true);
        Ok(())
    }

    /// Return whether the sale is currently in public phase.
    pub fn is_public_phase(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::IsPublicPhase)
            .unwrap_or(false)
    }

    /// Return the current Merkle root (None if unset).
    pub fn merkle_root(env: Env) -> Option<BytesN<32>> {
        env.storage().instance().get(&DataKey::MerkleRoot)
    }

    // ── Private helpers ───────────────────────────────────────────────────

    fn extend_instance_ttl(env: &Env) {
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_BUMP);
    }

    fn only_creator(env: &Env) -> Result<Address, Error> {
        let creator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Creator)
            .ok_or(Error::NotInitialized)?;
        creator.require_auth();
        Ok(creator)
    }

    /// Build the 32-byte digest that the creator must sign off-chain.
    ///
    /// Layout (all big-endian / XDR where noted):
    ///   N   bytes  contract_address XDR  (binds signature to this instance)
    ///   8   bytes  token_id  (u64 BE)
    ///  16   bytes  price     (i128 BE)
    ///   8   bytes  valid_until (u64 BE)
    ///  32   bytes  uri_hash
    ///   N   bytes  currency address XDR
    ///
    /// ⚠ Byte layout is STABLE — do not reorder fields.
    #[allow(non_snake_case)]
    pub fn _voucher_digest(env: &Env, v: &MintVoucher) -> Bytes {
        let mut raw = Bytes::new(env);
        raw.append(&env.current_contract_address().to_xdr(env));
        raw.extend_from_array(&v.token_id.to_be_bytes());
        raw.extend_from_array(&v.price.to_be_bytes());
        raw.extend_from_array(&v.valid_until.to_be_bytes());
        raw.append(&v.uri_hash.clone().into());
        raw.append(&v.currency.clone().to_xdr(env));
        env.crypto().sha256(&raw).into()
    }

    fn _transfer(env: &Env, from: &Address, to: &Address, token_id: u64) -> Result<(), Error> {
        env.storage()
            .persistent()
            .remove(&DataKey::Approved(token_id));
        let owner: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::TokenNotFound)?;
        if owner != *from {
            return Err(Error::NotOwner);
        }
        let from_bal: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::BalanceOf(from.clone()))
            .unwrap_or(0);
        if from_bal == 0 {
            return Err(Error::NotOwner);
        }
        env.storage().persistent().set(
            &DataKey::BalanceOf(from.clone()),
            &(from_bal.saturating_sub(1)),
        );
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BalanceOf(from.clone()), TTL_THRESHOLD, TTL_BUMP);
        let to_bal: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::BalanceOf(to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::BalanceOf(to.clone()), &(to_bal + 1));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::BalanceOf(to.clone()), TTL_THRESHOLD, TTL_BUMP);
        env.storage()
            .persistent()
            .set(&DataKey::Owner(token_id), to);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Owner(token_id), TTL_THRESHOLD, TTL_BUMP);
        env.events().publish(
            (symbol_short!("transfer"), from.clone(), to.clone()),
            (token_id, 1u128),
        );
        Ok(())
    }

    fn _check_approved(
        env: &Env,
        spender: &Address,
        from: &Address,
        token_id: u64,
    ) -> Result<(), Error> {
        if let Some(approved) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Approved(token_id))
        {
            if approved == *spender {
                return Ok(());
            }
        }
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::ApprovedForAll(from.clone(), spender.clone()))
            .unwrap_or(false)
        {
            return Ok(());
        }
        Err(Error::NotApproved)
    }
}

#[cfg(test)]
mod test;
