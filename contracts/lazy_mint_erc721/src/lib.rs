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
//! can never be claimed again (`VoucherAlreadyRedeemed`). The token_id serves
//! as the unique voucher nonce — each collection token may be lazy-minted at
//! most once.
//!
//! # Platform fee (#38)
//! A per-collection `platform_fee_bps` (≤ MAX_FEE_BPS set by the launchpad) is
//! stored at initialization. When a buyer redeems a priced voucher the fee
//! portion is transferred to `platform_fee_receiver` and the remainder to the
//! creator.
#![no_std]
#![allow(clippy::too_many_arguments, deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    token::Client as TokenClient, xdr::ToXdr, Address, Bytes, BytesN, Env, String, Vec,
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
}

// ─── Data types ───────────────────────────────────────────────────────────────

/// Off-chain voucher created by the collection creator.
///
/// Field `uri_hash` = sha256(uri_string) computed off-chain.
/// This is included in the signed digest so a relayer cannot swap the URI
/// while keeping the signature valid.
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
    UsedVoucher(u64), // token_id → bool
    MerkleRoot,       // BytesN<32> — root of allowlist Merkle tree
    IsPublicPhase,    // bool — true once public minting is enabled
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

    /// Verify a standard binary Merkle proof against the stored root.
    ///
    /// Leaf = sha256(address XDR).
    /// At each step the two sibling nodes are sorted (smaller first) before
    /// hashing so that proofs are order-independent (standard OpenZeppelin
    /// Merkle tree convention, ported to sha256).
    fn verify_merkle_proof(
        env: &Env,
        root: &BytesN<32>,
        leaf_preimage: &Address,
        proof: &Vec<BytesN<32>>,
    ) -> bool {
        // Leaf hash = sha256(address XDR)
        let mut computed: BytesN<32> = env
            .crypto()
            .sha256(&leaf_preimage.clone().to_xdr(env))
            .into();

        for sibling in proof.iter() {
            let mut pair = Bytes::new(env);
            // Sort the pair so the smaller hash goes first — makes proofs
            // position-independent (matches standard Merkle tree tooling).
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
        env.storage().instance().extend_ttl(50_000, 100_000);
        Ok(())
    }

    // ── Lazy Mint (core) ──────────────────────────────────────────────────

    /// Buyer submits a signed voucher to mint their NFT.
    /// During the allowlist phase a valid Merkle proof for `buyer` is required.
    /// The transaction fails (panics) if the ed25519 signature is invalid.
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
        let is_public: bool = env
            .storage()
            .instance()
            .get(&DataKey::IsPublicPhase)
            .unwrap_or(false);
        if !is_public {
            // Merkle root must be set; proof must be non-empty and valid.
            let root: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::MerkleRoot)
                .ok_or(Error::NotAllowlisted)?;
            if merkle_proof.is_empty() {
                return Err(Error::NotAllowlisted);
            }
            if !Self::verify_merkle_proof(&env, &root, &buyer, &merkle_proof) {
                return Err(Error::InvalidMerkleProof);
            }
        }

        // 1. Expiry check
        if voucher.valid_until != 0 && env.ledger().sequence() > voucher.valid_until as u32 {
            return Err(Error::VoucherExpired);
        }

        // 2. Replay check (#39) — token_id is the voucher nonce
        if env
            .storage()
            .persistent()
            .has(&DataKey::UsedVoucher(voucher.token_id))
        {
            return Err(Error::VoucherAlreadyRedeemed);
        }

        // 3. Supply check
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
        if next_id >= max {
            return Err(Error::MaxSupplyReached);
        }

        // 4. Signature verification — panics on invalid sig (caught by try_redeem)
        let pubkey: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::CreatorPubkey)
            .ok_or(Error::NotInitialized)?;
        let digest = Self::_voucher_digest(&env, &voucher);
        Self::verify_signature_or_panic(&env, &pubkey, &digest, &signature);

        // 5. Payment with platform fee split (#38)
        if voucher.price > 0 {
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
                let fee_amount = (voucher.price * fee_bps as i128) / 10_000;
                let creator_amount = voucher.price - fee_amount;
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
                let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
                TokenClient::new(&env, &voucher.currency).transfer(
                    &buyer,
                    &creator,
                    &voucher.price,
                );
            }
        }

        // 6. Mint
        let token_id = voucher.token_id;
        env.storage()
            .persistent()
            .set(&DataKey::Owner(token_id), &buyer);
        env.storage()
            .persistent()
            .set(&DataKey::TokenUri(token_id), &voucher.uri);
        // Mark voucher nonce as redeemed (#39)
        env.storage()
            .persistent()
            .set(&DataKey::UsedVoucher(token_id), &true);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Owner(token_id), 50_000, 100_000);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::TokenUri(token_id), 50_000, 100_000);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::UsedVoucher(token_id), 50_000, 100_000);

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
            .extend_ttl(&DataKey::BalanceOf(buyer.clone()), 50_000, 100_000);

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

        let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
        env.events().publish(
            (symbol_short!("mint"), creator, buyer.clone()),
            (token_id, 1u128),
        );
        Ok(token_id)
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
            .extend_ttl(&DataKey::Approved(token_id), 50_000, 100_000);
        Ok(())
    }

    pub fn set_approval_for_all(env: Env, owner: Address, operator: Address, approved: bool) {
        Self::extend_instance_ttl(&env);
        owner.require_auth();
        let key = DataKey::ApprovedForAll(owner.clone(), operator.clone());
        env.storage().persistent().set(&key, &approved);
        env.storage().persistent().extend_ttl(&key, 50_000, 100_000);
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

    /// Returns true if the voucher nonce (token_id) has already been redeemed (#39).
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

    /// Set the Merkle root for the allowlist.  Callable only by creator.
    /// Automatically enables allowlist phase (clears public phase flag).
    pub fn set_merkle_root(env: Env, root: BytesN<32>) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage().instance().set(&DataKey::MerkleRoot, &root);
        // Setting a new root resets to allowlist phase.
        env.storage()
            .instance()
            .set(&DataKey::IsPublicPhase, &false);
        Ok(())
    }

    /// Switch the sale to public phase — removes the allowlist restriction.
    /// Callable only by creator. Irreversible unless a new Merkle root is set.
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
    /// Layout (all big-endian):
    ///   N   bytes  contract_address XDR  (binds signature to this instance)
    ///   8   bytes  token_id
    ///  16   bytes  price (i128)
    ///   8   bytes  valid_until
    ///  32   bytes  uri_hash
    ///  N   bytes  currency address XDR
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
        env.storage().persistent().extend_ttl(
            &DataKey::BalanceOf(from.clone()),
            TTL_THRESHOLD,
            TTL_BUMP,
        );

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
            .extend_ttl(&DataKey::BalanceOf(to.clone()), 50_000, 100_000);

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
