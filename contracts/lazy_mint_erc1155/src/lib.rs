//! LazyMint1155 — Lazy-minting ERC-1155-equivalent on Soroban.
//!
//! Voucher model is the same as LazyMint721 but vouchers carry a
//! `buyer_quota` and `price_per_unit`. A buyer can call `redeem` multiple
//! times for the same token_id as long as their cumulative amount stays ≤
//! `buyer_quota`.  This mirrors edition-based lazy drops.
//!
//! # Voucher replay protection (#39)
//! Each voucher carries a `nonce: u64`. Once a voucher's nonce is redeemed
//! the contract stores it in `RedeemedVoucher(nonce)` and rejects any further
//! submission of that same nonce with `VoucherAlreadyRedeemed`.
//!
//! Signed digest:
//!   sha256(contract_addr ‖ token_id ‖ nonce ‖ buyer_quota ‖ price_per_unit ‖ valid_until ‖ uri_hash ‖ currency_xdr)
//!
//! # Voucher revocation
//! The creator can revoke a specific voucher nonce before it is redeemed via
//! `revoke_voucher(nonce)` or batch-revoke with `revoke_vouchers(nonces)`.
//! Revoking an already-redeemed nonce returns `VoucherAlreadyRedeemed`.
//!
//! # Merkle allowlist
//! A Merkle-root-based allowlist phase (identical scheme to lazy_mint_erc721)
//! gates `redeem` and `redeem_batch` before `set_public_phase()` is called.
//! Leaf = sha256(address XDR); siblings sorted at each level.
//!
//! # Platform fee (#38)
//! `platform_fee_bps` is stored at initialization; priced redemptions split
//! payment between the platform receiver and the creator.
//!
//! # Batch redemption
//! `redeem_batch` verifies and mints multiple vouchers atomically (all-or-nothing).
//! Payments are aggregated per currency.
#![no_std]
#![allow(clippy::too_many_arguments, deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token::TokenClient,
    xdr::ToXdr, Address, Bytes, BytesN, Env, Map, String, Vec,
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
    NotApproved = 3,
    InsufficientBalance = 4,
    LengthMismatch = 5,
    VoucherExpired = 6,
    ExceedsVoucherMax = 7,
    NotCreator = 8,
    EditionNotRegistered = 9,
    EditionAlreadyRegistered = 10,
    InvalidSignature = 11,
    MaxSupplyReached = 12,
    /// Voucher nonce already redeemed (#39).
    VoucherAlreadyRedeemed = 13,
    NotAllowlisted = 14,
    InvalidMerkleProof = 15,
    /// Voucher nonce has been explicitly revoked by the creator.
    VoucherRevoked = 16,
}

// ─── Data types ───────────────────────────────────────────────────────────────

/// Issue #39: `nonce` is the unique voucher identifier for replay protection.
#[contracttype]
#[derive(Clone)]
pub struct MintVoucher1155 {
    pub token_id: u64,
    pub nonce: u64,           // unique per voucher — prevents replay (#39)
    pub buyer_quota: u128,    // max per-buyer allocation
    pub price_per_unit: i128, // 0 = free
    pub currency: Address,
    pub uri: String,
    pub uri_hash: BytesN<32>,
    pub valid_until: u64,
}

/// One element of a `redeem_batch` call.
#[contracttype]
#[derive(Clone)]
pub struct BatchVoucherItem1155 {
    pub voucher: MintVoucher1155,
    pub amount: u128,
    pub signature: BytesN<64>,
    pub merkle_proof: Vec<BytesN<32>>,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Creator,
    CreatorPubkey,
    Name,
    RoyaltyBps,
    RoyaltyReceiver,
    /// Platform fee receiver (#38).
    PlatformFeeReceiver,
    /// Platform fee in basis points (#38).
    PlatformFeeBps,
    Balance(Address, u64),
    ApprovedForAll(Address, Address),
    TokenUri(u64),
    TotalSupply(u64),
    MintedPerBuyer(Address, u64),
    MaxAmount(u64),
    EditionMaxSupply(u64),
    /// Redeemed voucher nonce → bool (#39).
    RedeemedVoucher(u64),
    /// Revoked voucher nonce → bool (creator-revoked, per-nonce).
    RevokedVoucher(u64),
    MerkleRoot,      // BytesN<32>
    IsPublicPhase,   // bool
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct LazyMint1155;

impl LazyMint1155 {
    fn verify_signature_or_panic(
        env: &Env,
        pubkey: &BytesN<32>,
        digest: &Bytes,
        signature: &BytesN<64>,
    ) {
        env.crypto().ed25519_verify(pubkey, digest, signature);
    }

    /// Verify a standard binary Merkle proof against `root`.
    /// Leaf = sha256(address XDR).  Siblings sorted at each level.
    /// Identical scheme to lazy_mint_erc721 for tooling reuse.
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

    /// Validate a single voucher (expiry → replay → revocation → supply → quota → sig).
    /// Does NOT write state or transfer funds.
    fn check_voucher(
        env: &Env,
        voucher: &MintVoucher1155,
        amount: u128,
        signature: &BytesN<64>,
        pubkey: &BytesN<32>,
        buyer: &Address,
    ) -> Result<(), Error> {
        if voucher.valid_until != 0 && env.ledger().sequence() > voucher.valid_until as u32 {
            return Err(Error::VoucherExpired);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::RedeemedVoucher(voucher.nonce))
        {
            return Err(Error::VoucherAlreadyRedeemed);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::RevokedVoucher(voucher.nonce))
        {
            return Err(Error::VoucherRevoked);
        }
        // Edition must exist
        let edition_max: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::EditionMaxSupply(voucher.token_id))
            .ok_or(Error::EditionNotRegistered)?;
        let current_supply: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(voucher.token_id))
            .unwrap_or(0);
        if current_supply + amount > edition_max {
            return Err(Error::MaxSupplyReached);
        }
        // Per-buyer quota
        let already: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::MintedPerBuyer(buyer.clone(), voucher.token_id))
            .unwrap_or(0);
        if already + amount > voucher.buyer_quota {
            return Err(Error::ExceedsVoucherMax);
        }
        // Signature
        let digest = Self::_voucher_digest(env, voucher);
        Self::verify_signature_or_panic(env, pubkey, &digest, signature);
        Ok(())
    }

    /// Execute payment for a single voucher's total cost.
    fn pay(
        env: &Env,
        buyer: &Address,
        creator: &Address,
        currency: &Address,
        total_price: i128,
        fee_bps: u32,
        fee_receiver: &Address,
    ) {
        if total_price <= 0 {
            return;
        }
        if fee_bps > 0 {
            let fee_amount = (total_price * fee_bps as i128) / 10_000;
            let creator_amount = total_price - fee_amount;
            if fee_amount > 0 {
                TokenClient::new(env, currency).transfer(buyer, fee_receiver, &fee_amount);
            }
            if creator_amount > 0 {
                TokenClient::new(env, currency).transfer(buyer, creator, &creator_amount);
            }
        } else {
            TokenClient::new(env, currency).transfer(buyer, creator, &total_price);
        }
    }

    /// Mint tokens for one voucher item and update all storage.
    fn mint_item(
        env: &Env,
        buyer: &Address,
        voucher: &MintVoucher1155,
        amount: u128,
        creator: &Address,
    ) {
        // Balance
        let bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(buyer.clone(), voucher.token_id))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Balance(buyer.clone(), voucher.token_id),
            &(bal + amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(buyer.clone(), voucher.token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        // URI (set once)
        if !env
            .storage()
            .persistent()
            .has(&DataKey::TokenUri(voucher.token_id))
        {
            env.storage()
                .persistent()
                .set(&DataKey::TokenUri(voucher.token_id), &voucher.uri);
            env.storage().persistent().extend_ttl(
                &DataKey::TokenUri(voucher.token_id),
                TTL_THRESHOLD,
                TTL_BUMP,
            );
        }
        // Total supply
        let supply: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(voucher.token_id))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::TotalSupply(voucher.token_id),
            &(supply + amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(voucher.token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        // Minted per buyer
        let minted_key = DataKey::MintedPerBuyer(buyer.clone(), voucher.token_id);
        let already: u128 = env.storage().persistent().get(&minted_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&minted_key, &(already + amount));
        env.storage()
            .persistent()
            .extend_ttl(&minted_key, TTL_THRESHOLD, TTL_BUMP);
        // Mark nonce redeemed
        env.storage()
            .persistent()
            .set(&DataKey::RedeemedVoucher(voucher.nonce), &true);
        env.storage().persistent().extend_ttl(
            &DataKey::RedeemedVoucher(voucher.nonce),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        env.events().publish(
            (symbol_short!("mint"), creator.clone(), buyer.clone()),
            (voucher.token_id, amount),
        );
    }
}

#[contractimpl]
impl LazyMint1155 {
    // ── Initializer ───────────────────────────────────────────────────────

    /// Issue #38: accepts per-collection platform fee receiver and rate.
    pub fn initialize(
        env: Env,
        creator: Address,
        creator_pubkey: BytesN<32>,
        name: String,
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

    /// Redeem a single signed voucher to mint `amount` edition tokens.
    /// During the allowlist phase a valid Merkle proof for `buyer` is required.
    pub fn redeem(
        env: Env,
        buyer: Address,
        voucher: MintVoucher1155,
        amount: u128,
        signature: BytesN<64>,
        merkle_proof: Vec<BytesN<32>>,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        buyer.require_auth();

        // 0. Allowlist
        Self::check_allowlist(&env, &buyer, &merkle_proof)?;

        // 1–6. Validate
        let pubkey: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::CreatorPubkey)
            .ok_or(Error::NotInitialized)?;
        Self::check_voucher(&env, &voucher, amount, &signature, &pubkey, &buyer)?;

        // 7. Payment
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
        let total_price = voucher
            .price_per_unit
            .checked_mul(amount as i128)
            .unwrap_or(i128::MAX);
        Self::pay(
            &env,
            &buyer,
            &creator,
            &voucher.currency,
            total_price,
            fee_bps,
            &fee_receiver,
        );

        // 8. Mint
        Self::mint_item(&env, &buyer, &voucher, amount, &creator);
        Ok(())
    }

    // ── Lazy Mint (batch) ─────────────────────────────────────────────────

    /// Atomically redeem multiple vouchers.  All-or-nothing semantics.
    /// Payments aggregated per currency.
    pub fn redeem_batch(
        env: Env,
        buyer: Address,
        items: Vec<BatchVoucherItem1155>,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        buyer.require_auth();

        let pubkey: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::CreatorPubkey)
            .ok_or(Error::NotInitialized)?;
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

        // Phase 1: validate all items — no state changes yet.
        for item in items.iter() {
            Self::check_allowlist(&env, &buyer, &item.merkle_proof)?;
            Self::check_voucher(&env, &item.voucher, item.amount, &item.signature, &pubkey, &buyer)?;
        }

        // Phase 2: aggregate payments per currency.
        let mut fee_totals: Map<Address, i128> = Map::new(&env);
        let mut creator_totals: Map<Address, i128> = Map::new(&env);
        for item in items.iter() {
            let total_price = item
                .voucher
                .price_per_unit
                .checked_mul(item.amount as i128)
                .unwrap_or(i128::MAX);
            if total_price <= 0 {
                continue;
            }
            let cur = item.voucher.currency.clone();
            let fee_amount = if fee_bps > 0 {
                (total_price * fee_bps as i128) / 10_000
            } else {
                0i128
            };
            let creator_amount = total_price - fee_amount;
            let prev_fee: i128 = fee_totals.get(cur.clone()).unwrap_or(0);
            fee_totals.set(cur.clone(), prev_fee + fee_amount);
            let prev_creator: i128 = creator_totals.get(cur.clone()).unwrap_or(0);
            creator_totals.set(cur.clone(), prev_creator + creator_amount);
        }

        // Phase 3: transfer payments.
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

        // Phase 4: mint all.
        for item in items.iter() {
            Self::mint_item(&env, &buyer, &item.voucher, item.amount, &creator);
        }
        Ok(())
    }

    // ── Voucher Revocation ────────────────────────────────────────────────

    /// Revoke a single voucher nonce.  Creator-only.
    /// Returns `VoucherAlreadyRedeemed` if the nonce was already redeemed.
    pub fn revoke_voucher(env: Env, nonce: u64) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        let creator = Self::only_creator(&env)?;

        if env
            .storage()
            .persistent()
            .has(&DataKey::RedeemedVoucher(nonce))
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

    /// Batch-revoke nonces.  Creator-only.  All-or-nothing.
    pub fn revoke_vouchers(env: Env, nonces: Vec<u64>) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        let creator = Self::only_creator(&env)?;

        for nonce in nonces.iter() {
            if env
                .storage()
                .persistent()
                .has(&DataKey::RedeemedVoucher(nonce))
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

    /// Return `true` if the voucher nonce has been explicitly revoked.
    pub fn is_voucher_revoked(env: Env, nonce: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::RevokedVoucher(nonce))
    }

    // ── Merkle Allowlist ──────────────────────────────────────────────────

    /// Set the Merkle root for the allowlist.  Creator-only.
    /// Resets to allowlist phase.
    pub fn set_merkle_root(env: Env, root: BytesN<32>) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage().instance().set(&DataKey::MerkleRoot, &root);
        env.storage()
            .instance()
            .set(&DataKey::IsPublicPhase, &false);
        Ok(())
    }

    /// Switch sale to public phase.  Creator-only.
    pub fn set_public_phase(env: Env) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::IsPublicPhase, &true);
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
    // ── Transfers ─────────────────────────────────────────────────────────

    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        from.require_auth();
        Self::_transfer(&env, &from, &to, token_id, amount)
    }

    pub fn transfer_from(
        env: Env,
        operator: Address,
        from: Address,
        to: Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        operator.require_auth();
        if !Self::_is_approved_for_all(&env, &operator, &from) {
            return Err(Error::NotApproved);
        }
        Self::_transfer_with_operator(&env, &operator, &from, &to, token_id, amount)
    }

    pub fn batch_transfer(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        token_ids: Vec<u64>,
        amounts: Vec<u128>,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        spender.require_auth();
        if spender != from && !Self::_is_approved_for_all(&env, &spender, &from) {
            return Err(Error::NotApproved);
        }
        if token_ids.len() != amounts.len() {
            return Err(Error::LengthMismatch);
        }
        env.events().publish(
            (
                symbol_short!("trfbatch"),
                spender.clone(),
                from.clone(),
                to.clone(),
            ),
            (token_ids.clone(), amounts.clone()),
        );
        for (id, amount) in token_ids.iter().zip(amounts.iter()) {
            Self::_transfer(&env, &from, &to, id, amount)?;
        }
        Ok(())
    }

    // ── Approvals ─────────────────────────────────────────────────────────

    pub fn set_approval_for_all(env: Env, owner: Address, operator: Address, approved: bool) {
        Self::extend_instance_ttl(&env);
        owner.require_auth();
        let key = DataKey::ApprovedForAll(owner.clone(), operator.clone());
        env.storage().persistent().set(&key, &approved);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        env.events()
            .publish((symbol_short!("appr_all"), owner), (operator, approved));
    }

    // ── Burn ──────────────────────────────────────────────────────────────

    pub fn burn(
        env: Env,
        spender: Address,
        from: Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        spender.require_auth();
        if spender != from && !Self::_is_approved_for_all(&env, &spender, &from) {
            return Err(Error::NotApproved);
        }
        let bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone(), token_id))
            .unwrap_or(0);
        if bal < amount {
            return Err(Error::InsufficientBalance);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone(), token_id), &(bal - amount));
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone(), token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        let supply: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(token_id))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::TotalSupply(token_id),
            &(supply.saturating_sub(amount)),
        );
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::TotalSupply(token_id), 50_000, 100_000);
        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("burn"), from.clone()), (token_id, amount));
        Ok(())
    }

    // ── View functions ────────────────────────────────────────────────────

    pub fn balance_of(env: Env, account: Address, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(account, token_id))
            .unwrap_or(0)
    }

    pub fn balance_of_batch(env: Env, accounts: Vec<Address>, token_ids: Vec<u64>) -> Vec<u128> {
        if accounts.len() != token_ids.len() {
            return Vec::new(&env);
        }
        let mut out = Vec::new(&env);
        for (account, token_id) in accounts.iter().zip(token_ids.iter()) {
            let b: u128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(account, token_id))
                .unwrap_or(0);
            out.push_back(b);
        }
        out
    }

    pub fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool {
        Self::_is_approved_for_all(&env, &operator, &owner)
    }

    pub fn uri(env: Env, token_id: u64) -> String {
        env.storage()
            .persistent()
            .get(&DataKey::TokenUri(token_id))
            .unwrap()
    }

    pub fn total_supply(env: Env, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalSupply(token_id))
            .unwrap_or(0)
    }

    pub fn minted_by_buyer(env: Env, buyer: Address, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::MintedPerBuyer(buyer, token_id))
            .unwrap_or(0)
    }

    pub fn max_amount(env: Env, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxAmount(token_id))
            .unwrap_or(0)
    }

    /// Returns true if the voucher nonce has already been redeemed (#39).
    pub fn is_voucher_redeemed(env: Env, nonce: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::RedeemedVoucher(nonce))
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
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

    // ── Edition Management ────────────────────────────────────────────────

    pub fn register_edition(env: Env, token_id: u64, max_supply: u128) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        let key = DataKey::EditionMaxSupply(token_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::EditionAlreadyRegistered);
        }
        env.storage().persistent().set(&key, &max_supply);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        env.events()
            .publish((symbol_short!("register"), token_id), max_supply);
        Ok(())
    }

    pub fn edition_max_supply(env: Env, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::EditionMaxSupply(token_id))
            .unwrap_or(0)
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

    /// Signed digest:
    /// sha256(contract_addr ‖ token_id ‖ nonce ‖ buyer_quota ‖ price_per_unit ‖ valid_until ‖ uri_hash ‖ currency_xdr)
    ///
    /// ⚠ Byte layout is STABLE — do not reorder fields.
    #[allow(non_snake_case)]
    pub fn _voucher_digest(env: &Env, v: &MintVoucher1155) -> Bytes {
        let mut raw = Bytes::new(env);
        raw.append(&env.current_contract_address().to_xdr(env));
        raw.extend_from_array(&v.token_id.to_be_bytes());
        raw.extend_from_array(&v.nonce.to_be_bytes());
        raw.extend_from_array(&v.buyer_quota.to_be_bytes());
        raw.extend_from_array(&v.price_per_unit.to_be_bytes());
        raw.extend_from_array(&v.valid_until.to_be_bytes());
        raw.append(&v.uri_hash.clone().into());
        raw.append(&v.currency.clone().to_xdr(env));
        env.crypto().sha256(&raw).into()
    }

    fn _transfer_with_operator(
        env: &Env,
        _operator: &Address,
        from: &Address,
        to: &Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        let from_bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone(), token_id))
            .unwrap_or(0);
        if from_bal < amount {
            return Err(Error::InsufficientBalance);
        }
        env.storage().persistent().set(
            &DataKey::Balance(from.clone(), token_id),
            &(from_bal - amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone(), token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        let to_bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone(), token_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone(), token_id), &(to_bal + amount));
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone(), token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        env.events().publish(
            (symbol_short!("transfer"), from.clone(), to.clone()),
            (token_id, amount),
        );
        Ok(())
    }

    fn _transfer(
        env: &Env,
        from: &Address,
        to: &Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        let from_bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone(), token_id))
            .unwrap_or(0);
        if from_bal < amount {
            return Err(Error::InsufficientBalance);
        }
        env.storage().persistent().set(
            &DataKey::Balance(from.clone(), token_id),
            &(from_bal - amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone(), token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        let to_bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone(), token_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone(), token_id), &(to_bal + amount));
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone(), token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        env.events().publish(
            (symbol_short!("transfer"), from.clone(), to.clone()),
            (token_id, amount),
        );
        Ok(())
    }

    fn _is_approved_for_all(env: &Env, operator: &Address, owner: &Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ApprovedForAll(owner.clone(), operator.clone()))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod test;
