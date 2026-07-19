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
//! # Platform fee (#38)
//! `platform_fee_bps` is stored at initialization; priced redemptions split
//! payment between the platform receiver and the creator.
#![no_std]
#![allow(clippy::too_many_arguments, deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token::TokenClient,
    xdr::ToXdr, Address, Bytes, BytesN, Env, String, Vec,
};

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
        env.storage().instance().extend_ttl(50_000, 100_000);
        Ok(())
    }

    // ── Lazy Mint ─────────────────────────────────────────────────────────

    /// Issue #39: `voucher.nonce` is tracked in `RedeemedVoucher`; replaying
    /// the same voucher reverts with `VoucherAlreadyRedeemed`.
    /// Issue #38: applies platform fee split on priced redemptions.
    pub fn redeem(
        env: Env,
        buyer: Address,
        voucher: MintVoucher1155,
        amount: u128,
        signature: BytesN<64>,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        buyer.require_auth();

        // 1. Expiry
        if voucher.valid_until != 0 && env.ledger().sequence() > voucher.valid_until as u32 {
            return Err(Error::VoucherExpired);
        }

        // 2. Replay check by voucher nonce (#39)
        if env
            .storage()
            .persistent()
            .has(&DataKey::RedeemedVoucher(voucher.nonce))
        {
            return Err(Error::VoucherAlreadyRedeemed);
        }

        // 3. Global supply check — edition must be registered
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

        // 4. Per-buyer quota check
        let minted_key = DataKey::MintedPerBuyer(buyer.clone(), voucher.token_id);
        let already: u128 = env.storage().persistent().get(&minted_key).unwrap_or(0);
        if already + amount > voucher.buyer_quota {
            return Err(Error::ExceedsVoucherMax);
        }

        // 5. Signature verification (panics tx on bad sig)
        let pubkey: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::CreatorPubkey)
            .ok_or(Error::NotInitialized)?;
        let digest = Self::_voucher_digest(&env, &voucher);
        Self::verify_signature_or_panic(&env, &pubkey, &digest, &signature);

        // 6. Payment with platform fee split (#38)
        if voucher.price_per_unit > 0 {
            let total_price = voucher
                .price_per_unit
                .checked_mul(amount as i128)
                .unwrap_or(i128::MAX);
            let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
            let fee_bps: u32 = env
                .storage()
                .instance()
                .get(&DataKey::PlatformFeeBps)
                .unwrap_or(0);
            if fee_bps > 0 {
                let fee_receiver: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::PlatformFeeReceiver)
                    .unwrap();
                let fee_amount = (total_price * fee_bps as i128) / 10_000;
                let creator_amount = total_price - fee_amount;
                if fee_amount > 0 {
                    TokenClient::new(&env, &voucher.currency).transfer(
                        &buyer,
                        &fee_receiver,
                        &fee_amount,
                    );
                }
                if creator_amount > 0 {
                    TokenClient::new(&env, &voucher.currency).transfer(
                        &buyer,
                        &creator,
                        &creator_amount,
                    );
                }
            } else {
                TokenClient::new(&env, &voucher.currency).transfer(&buyer, &creator, &total_price);
            }
        }

        // 7. Mint
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
            50_000,
            100_000,
        );

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
                50_000,
                100_000,
            );
        }

        let supply: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(voucher.token_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(voucher.token_id), &(supply + amount));
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(voucher.token_id),
            50_000,
            100_000,
        );

        env.storage()
            .persistent()
            .set(&minted_key, &(already + amount));
        env.storage()
            .persistent()
            .extend_ttl(&minted_key, 50_000, 100_000);

        // Mark voucher nonce as redeemed (#39)
        env.storage()
            .persistent()
            .set(&DataKey::RedeemedVoucher(voucher.nonce), &true);
        env.storage().persistent().extend_ttl(
            &DataKey::RedeemedVoucher(voucher.nonce),
            50_000,
            100_000,
        );

        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("mint"), creator, buyer.clone()),
            (voucher.token_id, amount),
        );
        Ok(())
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

        #[allow(deprecated)]
        env.events().publish(
            (
                Symbol::new(&env, "TransferBatch"),
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
        env.storage().persistent().extend_ttl(&key, 50_000, 100_000);
        #[allow(deprecated)]
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
            50_000,
            100_000,
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
        env.storage().persistent().extend_ttl(&key, 50_000, 100_000);

        #[allow(deprecated)]
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

    fn extend_instance_ttl(env: &Env) {
        env.storage().instance().extend_ttl(50_000, 100_000);
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

    /// Signed digest: sha256(contract_addr ‖ token_id ‖ nonce ‖ buyer_quota ‖ price_per_unit ‖ valid_until ‖ uri_hash ‖ currency_xdr)
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
        operator: &Address,
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
            50_000,
            100_000,
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
            50_000,
            100_000,
        );
        #[allow(deprecated)]
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
            50_000,
            100_000,
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
            50_000,
            100_000,
        );
        #[allow(deprecated)]
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

mod test;
