//! NormalNFT1155 — ERC-1155-equivalent on Soroban.
//!
//! Supports multiple token types per contract. Each token type can have
//! fungible supply (edition sizes). The creator mints token IDs on demand
//! via `mint_new` (auto-increments ID) or `mint` (explicit ID for resupply).
//! Batch operations mirror ERC-1155 `safeBatchTransferFrom`.
//!
//! # New features (parity with collection_nft_erc721)
//!
//! - **Pausability**: creator-only `pause`/`unpause`/`is_paused`; gates
//!   `mint_new`, `mint`, `mint_batch`, `transfer`, `transfer_from`,
//!   `batch_transfer`, and `burn`.
//! - **Metadata management**: `set_base_uri`/`base_uri`,
//!   `freeze_metadata`/`is_metadata_frozen`.  `uri()` resolves in order:
//!   (1) explicit per-token URI stored at first mint, **unless** a base URI is
//!   set, in which case `base_uri + token_id` is returned instead.
//! - **Per-token royalties (ERC-2981 parity)**: `set_default_royalty`,
//!   `set_token_royalty`, `royalty_info_for(token_id, sale_price)`.
//!   `update_royalty`/`royalty_info` kept for backward compatibility as
//!   default-royalty view/setter.
#![no_std]
#![allow(clippy::too_many_arguments, deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env, String,
    Vec,
};

const TTL_THRESHOLD: u32 = 50_000;
const TTL_BUMP: u32 = 100_000;
const MAX_BPS: u32 = 10_000; // 100 % in basis points

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
    NotCreator = 6, // kept for ABI stability; not used internally
    /// Mint would exceed the per-token max supply.
    MaxSupplyReached = 7,
    /// Mint would exceed the per-wallet cap.
    WalletLimitReached = 8,
    /// Collection is paused; state-mutating calls are blocked.
    CollectionPaused = 9,
    /// base_uri cannot be updated after metadata is frozen.
    MetadataFrozen = 10,
    /// freeze_metadata called more than once.
    AlreadyFrozen = 11,
    /// basis points exceed MAX_BPS (10_000).
    InvalidBps = 12,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // Instance storage
    Initialized,
    Creator,
    Name,
    NextTokenId,
    RoyaltyBps,
    RoyaltyReceiver,
    /// Optional global per-wallet mint cap. Stored in instance storage.
    PerWalletLimit,
    /// bool — collection is paused when true.
    Paused,
    /// String — collection-level base URI (optional).
    BaseUri,
    /// bool — permanently frozen when true.
    MetadataFrozen,
    // Persistent storage
    Balance(Address, u64),            // (account, token_id) → u128
    ApprovedForAll(Address, Address), // (owner, operator) → bool
    TokenUri(u64),
    TotalSupply(u64), // per token_id
    /// Per-token maximum supply cap. 0 means no cap.
    MaxTokenSupply(u64),
    /// Cumulative amount minted per wallet per token_id.
    WalletMinted(Address, u64),
    /// Per-token royalty override — receiver address.
    TokenRoyaltyReceiver(u64),
    /// Per-token royalty override — bps.
    TokenRoyaltyBps(u64),
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Convert a u64 to its decimal ASCII representation as a Soroban `String`.
/// Used to build `base_uri + token_id` paths in `uri()`.
fn u64_to_string(env: &Env, mut n: u64) -> String {
    let mut buf = [0u8; 20]; // u64::MAX has 20 decimal digits
    let mut len = 0usize;
    if n == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while n > 0 {
            buf[len] = b'0' + (n % 10) as u8;
            n /= 10;
            len += 1;
        }
        buf[..len].reverse();
    }
    String::from_bytes(env, &buf[..len])
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct NormalNFT1155;

#[contractimpl]
impl NormalNFT1155 {
    // ── Initializer ───────────────────────────────────────────────────────

    pub fn initialize(
        env: Env,
        creator: Address,
        name: String,
        royalty_bps: u32,
        royalty_receiver: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Creator, &creator);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::NextTokenId, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyBps, &royalty_bps);
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyReceiver, &royalty_receiver);
        env.storage().instance().extend_ttl(50_000, 100_000);
        Ok(())
    }

    // ── Supply cap and per-wallet limit management ────────────────────────

    /// Set the maximum mintable supply for a specific token_id.
    /// Pass 0 to remove the cap.
    pub fn set_token_max_supply(env: Env, token_id: u64, max_supply: u128) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        if max_supply == 0 {
            env.storage()
                .persistent()
                .remove(&DataKey::MaxTokenSupply(token_id));
        } else {
            env.storage()
                .persistent()
                .set(&DataKey::MaxTokenSupply(token_id), &max_supply);
            env.storage().persistent().extend_ttl(
                &DataKey::MaxTokenSupply(token_id),
                TTL_THRESHOLD,
                TTL_BUMP,
            );
        }
        Ok(())
    }

    /// Set or clear the global per-wallet mint limit (0 = no limit).
    pub fn set_per_wallet_limit(env: Env, limit: u128) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::PerWalletLimit, &limit);
        Ok(())
    }

    /// Read the configured per-wallet limit (0 = no limit).
    pub fn per_wallet_limit(env: Env) -> u128 {
        env.storage()
            .instance()
            .get(&DataKey::PerWalletLimit)
            .unwrap_or(0)
    }

    /// Read the configured per-token max supply (0 = no cap).
    pub fn token_max_supply(env: Env, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxTokenSupply(token_id))
            .unwrap_or(0)
    }

    /// Read cumulative amount minted by `wallet` for `token_id`.
    pub fn wallet_minted(env: Env, wallet: Address, token_id: u64) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::WalletMinted(wallet, token_id))
            .unwrap_or(0)
    }

    // ── Pause mechanism ───────────────────────────────────────────────────

    /// Pause all state-mutating operations. Callable only by creator.
    pub fn pause(env: Env) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((symbol_short!("pause"),), true);
        Ok(())
    }

    /// Resume operations. Callable only by creator.
    pub fn unpause(env: Env) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((symbol_short!("pause"),), false);
        Ok(())
    }

    /// Returns `true` if the collection is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Paused)
            .unwrap_or(false)
    }

    // ── Metadata management ───────────────────────────────────────────────

    /// Update the collection-level base URI.  Reverts if metadata is frozen.
    /// Callable only by creator.
    pub fn set_base_uri(env: Env, base_uri: String) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::MetadataFrozen)
            .unwrap_or(false)
        {
            return Err(Error::MetadataFrozen);
        }
        env.storage().instance().set(&DataKey::BaseUri, &base_uri);
        Ok(())
    }

    /// Returns the stored base URI, or `None` if unset.
    pub fn base_uri(env: Env) -> Option<String> {
        env.storage().instance().get(&DataKey::BaseUri)
    }

    /// Permanently freeze metadata. Can only be called once.
    /// After this, `set_base_uri` reverts forever. Callable only by creator.
    pub fn freeze_metadata(env: Env) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::MetadataFrozen)
            .unwrap_or(false)
        {
            return Err(Error::AlreadyFrozen);
        }
        env.storage()
            .instance()
            .set(&DataKey::MetadataFrozen, &true);
        Ok(())
    }

    /// Returns `true` if metadata has been permanently frozen.
    pub fn is_metadata_frozen(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::MetadataFrozen)
            .unwrap_or(false)
    }

    // ── Royalty management ────────────────────────────────────────────────

    /// Set the collection-level default royalty with bps validation.
    /// Also updates the legacy `royalty_info` storage. Callable only by creator.
    pub fn set_default_royalty(env: Env, receiver: Address, bps: u32) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        if bps > MAX_BPS {
            return Err(Error::InvalidBps);
        }
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyReceiver, &receiver);
        env.storage().instance().set(&DataKey::RoyaltyBps, &bps);
        Ok(())
    }

    /// Set a per-token royalty override with bps validation.
    /// When set, `royalty_info_for(token_id, ..)` uses this instead of the default.
    /// Callable only by creator.
    pub fn set_token_royalty(
        env: Env,
        token_id: u64,
        receiver: Address,
        bps: u32,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        if bps > MAX_BPS {
            return Err(Error::InvalidBps);
        }
        env.storage()
            .persistent()
            .set(&DataKey::TokenRoyaltyReceiver(token_id), &receiver);
        env.storage()
            .persistent()
            .set(&DataKey::TokenRoyaltyBps(token_id), &bps);
        env.storage().persistent().extend_ttl(
            &DataKey::TokenRoyaltyReceiver(token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::TokenRoyaltyBps(token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        Ok(())
    }

    /// EIP-2981-style royalty query: returns (recipient, royalty_amount) for a
    /// given token and sale price.
    ///
    /// Resolution order:
    /// 1. Per-token override (`set_token_royalty`) if present.
    /// 2. Collection default (`set_default_royalty` / `update_royalty`).
    ///
    /// `royalty_amount = sale_price * bps / 10_000` (integer division, rounds down).
    pub fn royalty_info_for(
        env: Env,
        token_id: u64,
        sale_price: i128,
    ) -> Result<(Address, i128), Error> {
        let (receiver, bps) = if env
            .storage()
            .persistent()
            .has(&DataKey::TokenRoyaltyBps(token_id))
        {
            let r: Address = env
                .storage()
                .persistent()
                .get(&DataKey::TokenRoyaltyReceiver(token_id))
                .ok_or(Error::NotInitialized)?;
            let b: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::TokenRoyaltyBps(token_id))
                .unwrap_or(0);
            (r, b)
        } else {
            (
                env.storage()
                    .instance()
                    .get(&DataKey::RoyaltyReceiver)
                    .ok_or(Error::NotInitialized)?,
                env.storage()
                    .instance()
                    .get(&DataKey::RoyaltyBps)
                    .unwrap_or(0),
            )
        };

        let amount = sale_price
            .checked_mul(bps as i128)
            .unwrap_or(0)
            .checked_div(MAX_BPS as i128)
            .unwrap_or(0);

        Ok((receiver, amount))
    }

    // ── Minting ───────────────────────────────────────────────────────────

    /// Create a brand new token type, auto-assign the next ID.
    /// Returns the new token_id.
    ///
    /// Blocked while paused. Enforces per-wallet limit if set.
    pub fn mint_new(env: Env, to: Address, amount: u128, uri: String) -> Result<u64, Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        Self::require_not_paused(&env)?;
        let token_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);
        Self::_check_wallet_limit(&env, &to, token_id, amount)?;
        Self::_mint(&env, &to, token_id, amount, &uri);
        Self::_update_wallet_minted(&env, &to, token_id, amount);
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));
        Ok(token_id)
    }

    /// Mint additional supply of an existing token type (explicit token_id).
    ///
    /// Blocked while paused. Enforces supply cap and per-wallet limit if set.
    pub fn mint(
        env: Env,
        to: Address,
        token_id: u64,
        amount: u128,
        uri: String,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        Self::require_not_paused(&env)?;
        Self::_check_supply_cap(&env, token_id, amount)?;
        Self::_check_wallet_limit(&env, &to, token_id, amount)?;
        Self::_mint(&env, &to, token_id, amount, &uri);
        Self::_update_wallet_minted(&env, &to, token_id, amount);
        Ok(())
    }

    /// Batch-mint multiple token types in one call.
    ///
    /// Blocked while paused. Enforces supply cap and per-wallet limit per
    /// token_id, correctly handling duplicate IDs within the same batch by
    /// accumulating amounts before checking caps.
    pub fn mint_batch(
        env: Env,
        to: Address,
        token_ids: Vec<u64>,
        amounts: Vec<u128>,
        uris: Vec<String>,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        Self::require_not_paused(&env)?;

        let len = token_ids.len();
        if len == 0 {
            return Ok(());
        }
        if token_ids.len() != amounts.len() || token_ids.len() != uris.len() {
            return Err(Error::LengthMismatch);
        }

        // ── Invariant hardening: accumulate amounts per-id within the batch ──
        // This prevents duplicate token IDs from bypassing supply/wallet caps.
        //
        // We use a simple O(n²) dedup scan — no std HashMap in no_std Soroban.
        // For typical batch sizes (tens of items) this is fine.
        let mut checked_ids: Vec<u64> = Vec::new(&env);
        let mut checked_amounts: Vec<u128> = Vec::new(&env);

        for i in 0..len {
            let tid = token_ids.get(i).unwrap();
            let amt = amounts.get(i).unwrap();

            // Find whether `tid` was already seen in this batch.
            let mut found = false;
            for j in 0..checked_ids.len() {
                if checked_ids.get(j).unwrap() == tid {
                    let prev = checked_amounts.get(j).unwrap();
                    // Use saturating_add to handle u128 overflow safely.
                    let new_total = prev.saturating_add(amt);
                    checked_amounts.set(j, new_total);
                    found = true;
                    break;
                }
            }
            if !found {
                checked_ids.push_back(tid);
                checked_amounts.push_back(amt);
            }
        }

        // Pre-flight cap checks on the per-id totals before writing any state.
        for i in 0..checked_ids.len() {
            let tid = checked_ids.get(i).unwrap();
            let total_amt = checked_amounts.get(i).unwrap();
            Self::_check_supply_cap(&env, tid, total_amt)?;
            Self::_check_wallet_limit(&env, &to, tid, total_amt)?;
        }

        // Read NextTokenId once.
        let next_token_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);
        let mut max_token_id = next_token_id;

        // Process each entry in the original (possibly duplicate) list.
        for i in 0..len {
            let token_id = token_ids.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
            let uri = uris.get(i).unwrap();

            if token_id >= max_token_id {
                max_token_id = token_id + 1;
            }

            // URI set on first encounter only.
            if !env.storage().persistent().has(&DataKey::TokenUri(token_id)) {
                env.storage()
                    .persistent()
                    .set(&DataKey::TokenUri(token_id), &uri);
                env.storage().persistent().extend_ttl(
                    &DataKey::TokenUri(token_id),
                    TTL_THRESHOLD,
                    TTL_BUMP,
                );
            }

            // Update balance.
            let balance_key = DataKey::Balance(to.clone(), token_id);
            let cur_bal: u128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
            env.storage()
                .persistent()
                .set(&balance_key, &(cur_bal + amount));
            env.storage()
                .persistent()
                .extend_ttl(&balance_key, TTL_THRESHOLD, TTL_BUMP);

            // Update total supply.
            let supply_key = DataKey::TotalSupply(token_id);
            let cur_supply: u128 = env.storage().persistent().get(&supply_key).unwrap_or(0);
            env.storage()
                .persistent()
                .set(&supply_key, &(cur_supply + amount));
            env.storage()
                .persistent()
                .extend_ttl(&supply_key, TTL_THRESHOLD, TTL_BUMP);

            // Update wallet minted counter.
            let wm_key = DataKey::WalletMinted(to.clone(), token_id);
            let prev_minted: u128 = env.storage().persistent().get(&wm_key).unwrap_or(0);
            env.storage()
                .persistent()
                .set(&wm_key, &(prev_minted + amount));
            env.storage()
                .persistent()
                .extend_ttl(&wm_key, TTL_THRESHOLD, TTL_BUMP);

            let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
            env.events().publish(
                (symbol_short!("mint"), creator, to.clone()),
                (token_id, amount),
            );
        }

        if max_token_id > next_token_id {
            env.storage()
                .instance()
                .set(&DataKey::NextTokenId, &max_token_id);
        }

        Ok(())
    }

    // ── Transfers ─────────────────────────────────────────────────────────

    /// Blocked while paused.
    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        from.require_auth();
        Self::require_not_paused(&env)?;
        Self::_transfer(&env, &from, &to, token_id, amount)
    }

    /// Operator transfer on behalf of `from`. Blocked while paused.
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
        Self::require_not_paused(&env)?;
        if !Self::_is_approved_for_all(&env, &operator, &from) {
            return Err(Error::NotApproved);
        }
        Self::_transfer_with_operator(&env, &from, &to, token_id, amount)
    }

    /// Batch transfer — mirrors `safeBatchTransferFrom`. Blocked while paused.
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
        Self::require_not_paused(&env)?;

        // Allow owner or authorized operator.
        if spender != from && !Self::_is_approved_for_all(&env, &spender, &from) {
            return Err(Error::NotApproved);
        }

        if token_ids.len() != amounts.len() {
            return Err(Error::LengthMismatch);
        }

        for i in 0..token_ids.len() {
            let id = token_ids.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
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
        env.events()
            .publish((symbol_short!("appr_all"), owner), (operator, approved));
    }

    // ── Burn ──────────────────────────────────────────────────────────────

    /// Blocked while paused.
    pub fn burn(
        env: Env,
        spender: Address,
        from: Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        spender.require_auth();
        Self::require_not_paused(&env)?;

        // Allow owner or authorized operator.
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
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );

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

    /// Batch balance query — mirrors ERC-1155 `balanceOfBatch`.
    pub fn balance_of_batch(env: Env, accounts: Vec<Address>, token_ids: Vec<u64>) -> Vec<u128> {
        if accounts.len() != token_ids.len() {
            return Vec::new(&env);
        }
        let mut result = Vec::new(&env);
        for i in 0..accounts.len() {
            let account = accounts.get(i).unwrap();
            let token_id = token_ids.get(i).unwrap();
            let bal: u128 = env
                .storage()
                .persistent()
                .get(&DataKey::Balance(account.clone(), token_id))
                .unwrap_or(0);
            result.push_back(bal);
        }
        result
    }

    pub fn is_approved_for_all(env: Env, owner: Address, operator: Address) -> bool {
        Self::_is_approved_for_all(&env, &operator, &owner)
    }

    /// Resolve URI for `token_id`.
    ///
    /// Resolution order:
    /// 1. If a base URI is set, returns `base_uri + token_id` (decimal string).
    /// 2. Otherwise returns the per-token URI stored at mint time.
    pub fn uri(env: Env, token_id: u64) -> String {
        if let Some(base) = env
            .storage()
            .instance()
            .get::<DataKey, String>(&DataKey::BaseUri)
        {
            let id_str = u64_to_string(&env, token_id);
            let mut combined: Bytes = base.into();
            let id_bytes: Bytes = id_str.into();
            combined.append(&id_bytes);
            return String::from(&combined);
        }
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

    pub fn next_token_id(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0)
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    pub fn creator(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Creator).unwrap()
    }

    /// Legacy collection-level royalty view. Kept for backward compatibility.
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

    // ── Admin ─────────────────────────────────────────────────────────────

    pub fn transfer_ownership(env: Env, new_creator: Address) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::Creator, &new_creator);
        Ok(())
    }

    /// Legacy default-royalty setter. Alias for set_default_royalty without bps
    /// validation (preserved for backward compatibility). New callers should use
    /// `set_default_royalty` which validates bps ≤ MAX_BPS.
    pub fn update_royalty(env: Env, receiver: Address, bps: u32) -> Result<(), Error> {
        Self::extend_instance_ttl(&env);
        Self::only_creator(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyReceiver, &receiver);
        env.storage().instance().set(&DataKey::RoyaltyBps, &bps);
        Ok(())
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

    fn require_not_paused(env: &Env) -> Result<(), Error> {
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(Error::CollectionPaused);
        }
        Ok(())
    }

    fn _mint(env: &Env, to: &Address, token_id: u64, amount: u128, uri: &String) {
        let bal: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone(), token_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone(), token_id), &(bal + amount));
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone(), token_id),
            50_000,
            100_000,
        );

        // URI is set once; resupply mints don't overwrite it.
        if !env.storage().persistent().has(&DataKey::TokenUri(token_id)) {
            env.storage()
                .persistent()
                .set(&DataKey::TokenUri(token_id), uri);
            env.storage()
                .persistent()
                .extend_ttl(&DataKey::TokenUri(token_id), 50_000, 100_000);
        }

        let supply: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(token_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(token_id), &(supply + amount));
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(token_id),
            TTL_THRESHOLD,
            TTL_BUMP,
        );

        let creator: Address = env.storage().instance().get(&DataKey::Creator).unwrap();
        env.events().publish(
            (symbol_short!("mint"), creator, to.clone()),
            (token_id, amount),
        );
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
            50_000,
            100_000,
        );

        env.events().publish(
            (symbol_short!("transfer"), from.clone(), to.clone()),
            (token_id, amount),
        );
        Ok(())
    }

    fn _transfer_with_operator(
        env: &Env,
        from: &Address,
        to: &Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        Self::_transfer(env, from, to, token_id, amount)
    }

    fn _is_approved_for_all(env: &Env, operator: &Address, owner: &Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ApprovedForAll(owner.clone(), operator.clone()))
            .unwrap_or(false)
    }

    fn _check_supply_cap(env: &Env, token_id: u64, amount: u128) -> Result<(), Error> {
        if let Some(max_supply) = env
            .storage()
            .persistent()
            .get::<DataKey, u128>(&DataKey::MaxTokenSupply(token_id))
        {
            if max_supply > 0 {
                let current: u128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::TotalSupply(token_id))
                    .unwrap_or(0);
                if current.saturating_add(amount) > max_supply {
                    return Err(Error::MaxSupplyReached);
                }
            }
        }
        Ok(())
    }

    fn _check_wallet_limit(
        env: &Env,
        wallet: &Address,
        token_id: u64,
        amount: u128,
    ) -> Result<(), Error> {
        let limit: u128 = env
            .storage()
            .instance()
            .get(&DataKey::PerWalletLimit)
            .unwrap_or(0);
        if limit > 0 {
            let already: u128 = env
                .storage()
                .persistent()
                .get(&DataKey::WalletMinted(wallet.clone(), token_id))
                .unwrap_or(0);
            if already.saturating_add(amount) > limit {
                return Err(Error::WalletLimitReached);
            }
        }
        Ok(())
    }

    fn _update_wallet_minted(env: &Env, wallet: &Address, token_id: u64, amount: u128) {
        let key = DataKey::WalletMinted(wallet.clone(), token_id);
        let prev: u128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prev + amount));
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
    }
}

#[cfg(test)]
mod test;
