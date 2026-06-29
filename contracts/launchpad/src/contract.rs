//! Launchpad — Factory contract that deploys the 4 NFT collection types.
//!
//! # Deployment flow
//!
//! 1. Admin deploys this contract and calls `initialize`.
//! 2. Admin uploads each of the 4 collection WASMs with:
//!    `stellar contract upload --wasm <file>.wasm --network testnet`
//!    and then calls `set_wasm_hashes` with the 4 resulting 32-byte hashes.
//! 3. Any user can now call one of the four `deploy_*` functions to launch
//!    their own collection.  The factory calls `initialize` on the freshly
//!    deployed contract in the same transaction — no second call needed.
//!
//! # Deterministic addresses (clone-equivalent)
//! `env.deployer().with_current_contract(salt)` gives a deterministic address
//! from `sha256(factory_address ‖ salt)`.  Clients can pre-compute the address
//! before the transaction confirms.  Pass a different `salt` for each collection.
//!
//! # Why this is Soroban's answer to EIP-1167 clones
//! The collection WASM is stored once on the network (identified by hash).
//! Every `deploy()` call shares that same WASM — no bytecode duplication.
//! Each instance gets completely isolated storage.

use soroban_sdk::{
    contract, contractimpl, symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, String, Vec,
};

use crate::{
    events, storage,
    types::{CollectionKind, CollectionRecord, Error},
};

/// Maximum allowed platform fee (20 %) — issue #38.
const MAX_FEE_BPS: u32 = 2000;

// ─── Cross-contract clients ───────────────────────────────────────────────────

mod iface {
    use soroban_sdk::{contractclient, Address, BytesN, Env, String};

    #[contractclient(name = "Normal721Client")]
    pub trait INormal721 {
        fn initialize(
            env: Env,
            creator: Address,
            name: String,
            symbol: String,
            max_supply: u64,
            royalty_bps: u32,
            royalty_receiver: Address,
        );
    }

    #[contractclient(name = "Normal1155Client")]
    pub trait INormal1155 {
        fn initialize(
            env: Env,
            creator: Address,
            name: String,
            royalty_bps: u32,
            royalty_receiver: Address,
        );
    }

    /// Issue #38: lazy mint contracts accept per-collection platform fee at init.
    #[contractclient(name = "Lazy721Client")]
    #[allow(clippy::too_many_arguments)]
    pub trait ILazy721 {
        fn initialize(
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
        );
    }

    #[contractclient(name = "Lazy1155Client")]
    #[allow(clippy::too_many_arguments)]
    pub trait ILazy1155 {
        fn initialize(
            env: Env,
            creator: Address,
            creator_pubkey: BytesN<32>,
            name: String,
            royalty_bps: u32,
            royalty_receiver: Address,
            platform_fee_receiver: Address,
            platform_fee_bps: u32,
        );
    }
}

use iface::{Lazy1155Client, Lazy721Client, Normal1155Client, Normal721Client};

// ─── Salt hardening ───────────────────────────────────────────────────────────
fn make_secure_salt(env: &Env, creator: &Address, raw_salt: &BytesN<32>) -> BytesN<32> {
    let mut raw = Bytes::new(env);
    raw.append(&creator.to_xdr(env));
    raw.extend_from_array(&raw_salt.to_array());
    env.crypto().sha256(&raw).into()
}

#[contract]
pub struct Launchpad;

#[contractimpl]
#[allow(clippy::too_many_arguments)]
impl Launchpad {
    pub fn initialize(
        env: Env,
        admin: Address,
        platform_fee_receiver: Address,
        platform_fee_bps: u32,
    ) -> Result<(), Error> {
        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        storage::set_initialized(&env);
        storage::set_admin(&env, &admin);
        storage::set_platform_fee(&env, &platform_fee_receiver, platform_fee_bps);
        Ok(())
    }

    pub fn set_wasm_hashes(
        env: Env,
        wasm_normal_721: BytesN<32>,
        wasm_normal_1155: BytesN<32>,
        wasm_lazy_721: BytesN<32>,
        wasm_lazy_1155: BytesN<32>,
    ) -> Result<(), Error> {
        storage::extend_instance_ttl(&env);
        storage::require_admin(&env)?;
        storage::set_wasm_hashes(
            &env,
            &wasm_normal_721,
            &wasm_normal_1155,
            &wasm_lazy_721,
            &wasm_lazy_1155,
        );
        Ok(())
    }

    // ── Deploy: Normal ERC-721 ────────────────────────────────────────────

    /// Issue #38: `platform_fee_bps` is validated (≤ MAX_FEE_BPS) and stored in the registry.
    pub fn deploy_normal_721(
        env: Env,
        creator: Address,
        currency: Address,
        name: String,
        symbol: String,
        max_supply: u64,
        royalty_bps: u32,
        royalty_receiver: Address,
        platform_fee_bps: u32,
        salt: BytesN<32>,
    ) -> Result<Address, Error> {
        storage::extend_instance_ttl(&env);
        creator.require_auth();

        if platform_fee_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFeeBps);
        }

        let (receiver, fee) = storage::get_platform_fee(&env);
        if fee > 0 {
            soroban_sdk::token::TokenClient::new(&env, &currency).transfer(
                &creator,
                &receiver,
                &(fee as i128),
            );
            events::publish_deployment_fee_collected(&env, &creator, &receiver, fee as i128, &currency);
        }

        let wasm = storage::get_wasm_normal_721(&env).ok_or(Error::WasmHashNotSet)?;

        let secure_salt = make_secure_salt(&env, &creator, &salt);
        let addr = env
            .deployer()
            .with_current_contract(secure_salt)
            .deploy_v2(wasm, ());

        Normal721Client::new(&env, &addr).initialize(
            &creator,
            &name,
            &symbol,
            &max_supply,
            &royalty_bps,
            &royalty_receiver,
        );

        storage::record_collection(
            &env,
            &creator,
            &addr,
            CollectionKind::Normal721,
            &name,
            &symbol,
            env.ledger().sequence(),
            platform_fee_bps,
        );
        events::publish_deploy(&env, symbol_short!("n721"), &creator, &addr);
        Ok(addr)
    }

    // ── Deploy: Normal ERC-1155 ──────────────────────────────────────────
    pub fn deploy_normal_1155(
        env: Env,
        creator: Address,
        currency: Address,
        name: String,
        royalty_bps: u32,
        royalty_receiver: Address,
        platform_fee_bps: u32,
        salt: BytesN<32>,
    ) -> Result<Address, Error> {
        storage::extend_instance_ttl(&env);
        creator.require_auth();

        if platform_fee_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFeeBps);
        }

        let (receiver, fee) = storage::get_platform_fee(&env);
        if fee > 0 {
            soroban_sdk::token::TokenClient::new(&env, &currency).transfer(
                &creator,
                &receiver,
                &(fee as i128),
            );
            events::publish_deployment_fee_collected(&env, &creator, &receiver, fee as i128, &currency);
        }

        let wasm = storage::get_wasm_normal_1155(&env).ok_or(Error::WasmHashNotSet)?;

        let secure_salt = make_secure_salt(&env, &creator, &salt);
        let addr = env
            .deployer()
            .with_current_contract(secure_salt)
            .deploy_v2(wasm, ());

        Normal1155Client::new(&env, &addr).initialize(
            &creator,
            &name,
            &royalty_bps,
            &royalty_receiver,
        );

        let empty_symbol = String::from_str(&env, "");
        storage::record_collection(
            &env,
            &creator,
            &addr,
            CollectionKind::Normal1155,
            &name,
            &empty_symbol,
            env.ledger().sequence(),
            platform_fee_bps,
        );
        events::publish_deploy(&env, symbol_short!("n1155"), &creator, &addr);
        Ok(addr)
    }

    // ── Deploy: LazyMint ERC-721 ──────────────────────────────────────────

    /// Issue #38: passes per-collection fee to the lazy mint contract so that
    /// fee splits are applied at voucher redemption time.
    pub fn deploy_lazy_721(
        env: Env,
        creator: Address,
        currency: Address,
        creator_pubkey: BytesN<32>,
        name: String,
        symbol: String,
        max_supply: u64,
        royalty_bps: u32,
        royalty_receiver: Address,
        platform_fee_bps: u32,
        salt: BytesN<32>,
    ) -> Result<Address, Error> {
        storage::extend_instance_ttl(&env);
        creator.require_auth();

        if platform_fee_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFeeBps);
        }

        let (platform_fee_receiver, fee) = storage::get_platform_fee(&env);
        if fee > 0 {
            soroban_sdk::token::TokenClient::new(&env, &currency).transfer(
                &creator,
                &platform_fee_receiver,
                &(fee as i128),
            );
            events::publish_deployment_fee_collected(&env, &creator, &receiver, fee as i128, &currency);
        }

        let wasm = storage::get_wasm_lazy_721(&env).ok_or(Error::WasmHashNotSet)?;

        let secure_salt = make_secure_salt(&env, &creator, &salt);
        let addr = env
            .deployer()
            .with_current_contract(secure_salt)
            .deploy_v2(wasm, ());

        Lazy721Client::new(&env, &addr).initialize(
            &creator,
            &creator_pubkey,
            &name,
            &symbol,
            &max_supply,
            &royalty_bps,
            &royalty_receiver,
            &platform_fee_receiver,
            &platform_fee_bps,
        );

        storage::record_collection(
            &env,
            &creator,
            &addr,
            CollectionKind::LazyMint721,
            &name,
            &symbol,
            env.ledger().sequence(),
            platform_fee_bps,
        );
        events::publish_deploy(&env, symbol_short!("l721"), &creator, &addr);
        Ok(addr)
    }

    // ── Deploy: LazyMint ERC-1155 ─────────────────────────────────────────
    pub fn deploy_lazy_1155(
        env: Env,
        creator: Address,
        currency: Address,
        creator_pubkey: BytesN<32>,
        name: String,
        royalty_bps: u32,
        royalty_receiver: Address,
        platform_fee_bps: u32,
        salt: BytesN<32>,
    ) -> Result<Address, Error> {
        storage::extend_instance_ttl(&env);
        creator.require_auth();

        if platform_fee_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFeeBps);
        }

        let (platform_fee_receiver, fee) = storage::get_platform_fee(&env);
        if fee > 0 {
            soroban_sdk::token::TokenClient::new(&env, &currency).transfer(
                &creator,
                &platform_fee_receiver,
                &(fee as i128),
            );
            events::publish_deployment_fee_collected(&env, &creator, &receiver, fee as i128, &currency);
        }

        let wasm = storage::get_wasm_lazy_1155(&env).ok_or(Error::WasmHashNotSet)?;

        let secure_salt = make_secure_salt(&env, &creator, &salt);
        let addr = env
            .deployer()
            .with_current_contract(secure_salt)
            .deploy_v2(wasm, ());

        Lazy1155Client::new(&env, &addr).initialize(
            &creator,
            &creator_pubkey,
            &name,
            &royalty_bps,
            &royalty_receiver,
            &platform_fee_receiver,
            &platform_fee_bps,
        );

        let empty_symbol = String::from_str(&env, "");
        storage::record_collection(
            &env,
            &creator,
            &addr,
            CollectionKind::LazyMint1155,
            &name,
            &empty_symbol,
            env.ledger().sequence(),
            platform_fee_bps,
        );
        events::publish_deploy(&env, symbol_short!("l1155"), &creator, &addr);
        Ok(addr)
    }

    // ── Admin management ──────────────────────────────────────────────────

    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        storage::extend_instance_ttl(&env);
        storage::require_admin(&env)?;
        storage::set_admin(&env, &new_admin);
        Ok(())
    }

    pub fn update_platform_fee(env: Env, receiver: Address, fee_bps: u32) -> Result<(), Error> {
        storage::extend_instance_ttl(&env);
        storage::require_admin(&env)?;
        storage::set_platform_fee(&env, &receiver, fee_bps);
        Ok(())
    }

    /// Set only the flat deploy fee (in stroops or token smallest unit).
    pub fn set_deploy_fee(env: Env, fee: u32) -> Result<(), Error> {
        storage::extend_instance_ttl(&env);
        storage::require_admin(&env)?;
        storage::set_deploy_fee_only(&env, fee);
        Ok(())
    }

    /// Set only the treasury address that receives deploy fees.
    pub fn set_treasury(env: Env, treasury: Address) -> Result<(), Error> {
        storage::extend_instance_ttl(&env);
        storage::require_admin(&env)?;
        storage::set_treasury_only(&env, &treasury);
        Ok(())
    }

    // ── View functions ────────────────────────────────────────────────────

    pub fn collections_by_creator(env: Env, creator: Address) -> Vec<CollectionRecord> {
        storage::collections_by_creator(&env, &creator)
    }

    pub fn all_collections(env: Env) -> Vec<CollectionRecord> {
        storage::all_collections(&env)
    }

    pub fn collection_count(env: Env) -> u64 {
        storage::collection_count(&env)
    }

    /// Direct O(1) lookup of a collection by its deployed address (#37).
    pub fn get_collection(env: Env, address: Address) -> Option<CollectionRecord> {
        storage::get_collection_by_address(&env, &address)
    }

    /// Paginated read of the global registry (#37).
    pub fn get_collections(env: Env, start: u64, limit: u32) -> Vec<CollectionRecord> {
        storage::get_collections_paginated(&env, start, limit)
    }

    pub fn admin(env: Env) -> Address {
        storage::get_admin(&env).unwrap()
    }

    pub fn platform_fee(env: Env) -> (Address, u32) {
        storage::get_platform_fee(&env)
    }
}
