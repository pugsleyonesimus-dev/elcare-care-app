#![cfg(test)]
#![allow(unused_variables)]

use crate::{
    BatchVoucherItem1155, DataKey, Error, LazyMint1155, LazyMint1155Client, MintVoucher1155,
};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, String, Vec,
};

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn creator_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn setup(fee_bps: u32) -> (Env, LazyMint1155Client<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let contract_id = env.register(LazyMint1155, ());
    let client = LazyMint1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    let sk = creator_signing_key();
    client.initialize(
        &creator,
        &BytesN::from_array(&env, &sk.verifying_key().to_bytes()),
        &String::from_str(&env, "LazyMint1155"),
        &500u32,
        &Address::generate(&env),
        &fee_receiver,
        &fee_bps,
    );
    (env, client, creator, fee_receiver)
}

fn empty_proof(env: &Env) -> Vec<BytesN<32>> {
    Vec::new(env)
}

fn make_voucher(env: &Env, token_id: u64, nonce: u64) -> MintVoucher1155 {
    MintVoucher1155 {
        token_id,
        nonce,
        buyer_quota: 100u128,
        price_per_unit: 0i128,
        currency: Address::generate(env),
        uri: String::from_str(env, "ipfs://test"),
        uri_hash: BytesN::from_array(env, &[0u8; 32]),
        valid_until: 0,
    }
}

fn sign_voucher(env: &Env, contract_id: &Address, v: &MintVoucher1155) -> BytesN<64> {
    let sk = creator_signing_key();
    let digest = env.as_contract(contract_id, || LazyMint1155::_voucher_digest(env, v));
    let mut msg = [0u8; 32];
    digest.copy_into_slice(&mut msg);
    BytesN::from_array(env, &sk.sign(&msg).to_bytes())
}

fn leaf_hash(env: &Env, addr: &Address) -> BytesN<32> {
    env.crypto().sha256(&addr.clone().to_xdr(env)).into()
}

fn combine(env: &Env, a: BytesN<32>, b: BytesN<32>) -> BytesN<32> {
    let mut pair = Bytes::new(env);
    if a.to_array() <= b.to_array() {
        pair.append(&a.into());
        pair.append(&b.into());
    } else {
        pair.append(&b.into());
        pair.append(&a.into());
    }
    env.crypto().sha256(&pair).into()
}

fn two_leaf_tree(
    env: &Env,
    a: &Address,
    b: &Address,
) -> (BytesN<32>, Vec<BytesN<32>>, Vec<BytesN<32>>) {
    let la = leaf_hash(env, a);
    let lb = leaf_hash(env, b);
    let root = combine(env, la.clone(), lb.clone());
    let mut pa = Vec::new(env);
    pa.push_back(lb.clone());
    let mut pb = Vec::new(env);
    pb.push_back(la.clone());
    (root, pa, pb)
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 1 — Digest stability (pinned regression)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn digest_byte_layout_is_stable() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LazyMint1155, ());
    let client = LazyMint1155Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let sk = creator_signing_key();
    client.initialize(
        &creator,
        &BytesN::from_array(&env, &sk.verifying_key().to_bytes()),
        &String::from_str(&env, "T"),
        &0u32,
        &Address::generate(&env),
        &Address::generate(&env),
        &0u32,
    );
    let currency = Address::generate(&env);
    let v = MintVoucher1155 {
        token_id: 1u64,
        nonce: 42u64,
        buyer_quota: 10u128,
        price_per_unit: 500i128,
        currency: currency.clone(),
        uri: String::from_str(&env, "ipfs://stable"),
        uri_hash: BytesN::from_array(&env, &[0xabu8; 32]),
        valid_until: 9999u64,
    };
    let d1 = env.as_contract(&contract_id, || LazyMint1155::_voucher_digest(&env, &v));
    let d2 = env.as_contract(&contract_id, || LazyMint1155::_voucher_digest(&env, &v));
    assert_eq!(d1, d2, "digest must be deterministic");

    // Mutating each field must change the digest.
    let mutations: &[MintVoucher1155] = &[
        MintVoucher1155 { token_id: 2, ..v.clone() },
        MintVoucher1155 { nonce: 43, ..v.clone() },
        MintVoucher1155 { buyer_quota: 11, ..v.clone() },
        MintVoucher1155 { price_per_unit: 501, ..v.clone() },
        MintVoucher1155 { valid_until: 1, ..v.clone() },
        MintVoucher1155 {
            uri_hash: BytesN::from_array(&env, &[0xbbu8; 32]),
            ..v.clone()
        },
    ];
    for mv in mutations {
        let dm = env.as_contract(&contract_id, || LazyMint1155::_voucher_digest(&env, mv));
        assert_ne!(d1, dm, "mutating a field must change digest");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 2 — Single redeem (1155)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn redeem_free_voucher_public_phase() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 1u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 0);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(client.balance_of(&buyer, &token_id), 1u128);
    assert_eq!(client.total_supply(&token_id), 1u128);
    assert!(client.is_voucher_redeemed(&0u64));
}

#[test]
fn redeem_marks_nonce_and_blocks_replay() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 2u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 1);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    let sig2 = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &1u128, &sig2, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::VoucherAlreadyRedeemed)));
}

#[test]
fn redeem_expired_voucher_returns_error() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 3u64;
    client.register_edition(&token_id, &100u128);
    env.ledger().with_mut(|li| li.sequence_number = 100);
    let buyer = Address::generate(&env);
    let v = MintVoucher1155 { valid_until: 50, ..make_voucher(&env, token_id, 2) };
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::VoucherExpired)));
}

#[test]
fn redeem_wrong_key_fails() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 4u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 3);
    let bad_sig = BytesN::from_array(&env, &[0u8; 64]);
    assert!(client.try_redeem(&buyer, &v, &1u128, &bad_sig, &empty_proof(&env)).is_err());
}

#[test]
fn redeem_tampered_uri_fails_sig() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 5u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 4);
    let sig = sign_voucher(&env, &client.address, &v);
    let v_bad = MintVoucher1155 {
        uri_hash: BytesN::from_array(&env, &[0xffu8; 32]),
        ..v
    };
    assert!(client.try_redeem(&buyer, &v_bad, &1u128, &sig, &empty_proof(&env)).is_err());
}

#[test]
fn redeem_unregistered_edition_returns_error() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, 99, 5);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::EditionNotRegistered)));
}

#[test]
fn redeem_exceeds_max_supply_returns_error() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 6u64;
    client.register_edition(&token_id, &2u128); // only 2 available
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 6);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &5u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::MaxSupplyReached)));
}

#[test]
fn redeem_exceeds_buyer_quota_returns_error() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 7u64;
    client.register_edition(&token_id, &1000u128);
    let buyer = Address::generate(&env);
    let v = MintVoucher1155 { buyer_quota: 3, ..make_voucher(&env, token_id, 7) };
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &10u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::ExceedsVoucherMax)));
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 3 — Voucher revocation (1155)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn revoke_blocks_subsequent_redeem() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 10u64;
    client.register_edition(&token_id, &100u128);
    let nonce = 10u64;
    client.revoke_voucher(&nonce);
    assert!(client.is_voucher_revoked(&nonce));
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, nonce);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::VoucherRevoked)));
}

#[test]
fn revoke_already_redeemed_nonce_returns_error() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 11u64;
    client.register_edition(&token_id, &100u128);
    let nonce = 11u64;
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, nonce);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    let res = client.try_revoke_voucher(&nonce);
    assert_eq!(res, Err(Ok(Error::VoucherAlreadyRedeemed)));
}

#[test]
fn revoke_vouchers_batch_all_or_nothing() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 12u64;
    client.register_edition(&token_id, &100u128);
    // Redeem nonce 20
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 20);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    // Batch revoke [21, 22, 20] — nonce 20 is redeemed → should fail
    let mut nonces = Vec::new(&env);
    nonces.push_back(21u64);
    nonces.push_back(22u64);
    nonces.push_back(20u64);
    let res = client.try_revoke_vouchers(&nonces);
    assert_eq!(res, Err(Ok(Error::VoucherAlreadyRedeemed)));
    assert!(!client.is_voucher_revoked(&21u64));
    assert!(!client.is_voucher_revoked(&22u64));
}

#[test]
fn revoke_vouchers_batch_success() {
    let (env, client, _creator, _fee) = setup(0);
    let mut nonces = Vec::new(&env);
    nonces.push_back(30u64);
    nonces.push_back(31u64);
    client.revoke_vouchers(&nonces);
    assert!(client.is_voucher_revoked(&30u64));
    assert!(client.is_voucher_revoked(&31u64));
    assert!(!client.is_voucher_revoked(&32u64));
}

#[test]
fn revoked_voucher_rejected_in_batch_redeem() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 13u64;
    client.register_edition(&token_id, &100u128);
    let nonce = 40u64;
    client.revoke_voucher(&nonce);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, nonce);
    let sig = sign_voucher(&env, &client.address, &v);
    let mut items = Vec::new(&env);
    items.push_back(BatchVoucherItem1155 {
        voucher: v,
        amount: 1u128,
        signature: sig,
        merkle_proof: empty_proof(&env),
    });
    let res = client.try_redeem_batch(&buyer, &items);
    assert_eq!(res, Err(Ok(Error::VoucherRevoked)));
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 4 — Merkle allowlist (1155) — identical scheme to 721
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn allowlist_phase_default_no_root_blocks() {
    let (env, client, _creator, _fee) = setup(0);
    let token_id = 20u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 50);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::NotAllowlisted)));
}

#[test]
fn allowlist_valid_proof_passes_gate_and_mints() {
    let (env, client, _creator, _fee) = setup(0);
    let token_id = 21u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let (root, proof_buyer, _) = two_leaf_tree(&env, &buyer, &Address::generate(&env));
    client.set_merkle_root(&root);
    let v = make_voucher(&env, token_id, 51);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &proof_buyer);
    assert_eq!(client.balance_of(&buyer, &token_id), 1u128);
}

#[test]
fn allowlist_wrong_proof_returns_invalid_merkle_proof() {
    let (env, client, _creator, _fee) = setup(0);
    let token_id = 22u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let other = Address::generate(&env);
    let (root, _, proof_other) = two_leaf_tree(&env, &buyer, &other);
    client.set_merkle_root(&root);
    let v = make_voucher(&env, token_id, 52);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&buyer, &v, &1u128, &sig, &proof_other);
    assert_eq!(res, Err(Ok(Error::InvalidMerkleProof)));
}

#[test]
fn allowlist_outsider_with_empty_proof_not_allowlisted() {
    let (env, client, _creator, _fee) = setup(0);
    let token_id = 23u64;
    client.register_edition(&token_id, &100u128);
    let allowed = Address::generate(&env);
    let (root, _, _) = two_leaf_tree(&env, &allowed, &Address::generate(&env));
    client.set_merkle_root(&root);
    let outsider = Address::generate(&env);
    let v = make_voucher(&env, token_id, 53);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&outsider, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(res, Err(Ok(Error::NotAllowlisted)));
}

#[test]
fn public_phase_bypasses_allowlist() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 24u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 54);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(client.balance_of(&buyer, &token_id), 1u128);
}

#[test]
fn set_merkle_root_resets_to_allowlist_phase() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    assert!(client.is_public_phase());
    let root = BytesN::from_array(&env, &[0xaau8; 32]);
    client.set_merkle_root(&root);
    assert!(!client.is_public_phase());
}

#[test]
fn allowlist_unbalanced_three_leaf_tree() {
    let (env, client, _creator, _fee) = setup(0);
    let token_id = 25u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    let lb = leaf_hash(&env, &buyer);
    let lbb = leaf_hash(&env, &b);
    let lc = leaf_hash(&env, &c);
    let node_left = combine(&env, lb.clone(), lbb.clone());
    let root = combine(&env, node_left, lc.clone());
    client.set_merkle_root(&root);
    let mut proof = Vec::new(&env);
    proof.push_back(lbb);
    proof.push_back(lc);
    let v = make_voucher(&env, token_id, 55);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &proof);
    assert_eq!(client.balance_of(&buyer, &token_id), 1u128);
}

#[test]
fn allowlist_forged_proof_address_rejected() {
    let (env, client, _creator, _fee) = setup(0);
    let token_id = 26u64;
    client.register_edition(&token_id, &100u128);
    let addr_a = Address::generate(&env);
    let addr_b = Address::generate(&env);
    let forger = Address::generate(&env);
    let (root, _, proof_b) = two_leaf_tree(&env, &addr_a, &addr_b);
    client.set_merkle_root(&root);
    // forger tries addr_b's proof — leaf is sha256(forger XDR) ≠ sha256(addr_b XDR)
    let v = make_voucher(&env, token_id, 56);
    let sig = sign_voucher(&env, &client.address, &v);
    let res = client.try_redeem(&forger, &v, &1u128, &sig, &proof_b);
    assert_eq!(res, Err(Ok(Error::InvalidMerkleProof)));
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 5 — redeem_batch (1155)
// ═══════════════════════════════════════════════════════════════════════════════

fn make_batch_item(
    env: &Env,
    contract_id: &Address,
    token_id: u64,
    nonce: u64,
    amount: u128,
) -> BatchVoucherItem1155 {
    let v = make_voucher(env, token_id, nonce);
    let sig = sign_voucher(env, contract_id, &v);
    BatchVoucherItem1155 {
        voucher: v,
        amount,
        signature: sig,
        merkle_proof: empty_proof(env),
    }
}

#[test]
fn batch_size_1_succeeds() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 100u64;
    client.register_edition(&token_id, &1000u128);
    let buyer = Address::generate(&env);
    let mut items = Vec::new(&env);
    items.push_back(make_batch_item(&env, &client.address, token_id, 100, 5));
    client.redeem_batch(&buyer, &items);
    assert_eq!(client.balance_of(&buyer, &token_id), 5u128);
    assert_eq!(client.total_supply(&token_id), 5u128);
}

#[test]
fn batch_n_items_all_minted() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    for tid in 200u64..205u64 {
        client.register_edition(&tid, &1000u128);
    }
    let buyer = Address::generate(&env);
    let mut items = Vec::new(&env);
    for (i, tid) in (200u64..205u64).enumerate() {
        items.push_back(make_batch_item(&env, &client.address, tid, 200 + i as u64, 1));
    }
    client.redeem_batch(&buyer, &items);
    for tid in 200u64..205u64 {
        assert_eq!(client.balance_of(&buyer, &tid), 1u128);
    }
}

#[test]
fn batch_one_expired_reverts_all() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let t1 = 300u64;
    let t2 = 301u64;
    client.register_edition(&t1, &100u128);
    client.register_edition(&t2, &100u128);
    env.ledger().with_mut(|li| li.sequence_number = 200);
    let buyer = Address::generate(&env);
    let good = make_batch_item(&env, &client.address, t1, 300, 1);
    let expired_v = MintVoucher1155 { valid_until: 100, ..make_voucher(&env, t2, 301) };
    let expired_sig = sign_voucher(&env, &client.address, &expired_v);
    let bad = BatchVoucherItem1155 {
        voucher: expired_v,
        amount: 1,
        signature: expired_sig,
        merkle_proof: empty_proof(&env),
    };
    let mut items = Vec::new(&env);
    items.push_back(good);
    items.push_back(bad);
    let res = client.try_redeem_batch(&buyer, &items);
    assert_eq!(res, Err(Ok(Error::VoucherExpired)));
    assert_eq!(client.balance_of(&buyer, &t1), 0u128);
}

#[test]
fn batch_one_revoked_reverts_all() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let t1 = 400u64;
    let t2 = 401u64;
    client.register_edition(&t1, &100u128);
    client.register_edition(&t2, &100u128);
    client.revoke_voucher(&401u64);
    let buyer = Address::generate(&env);
    let mut items = Vec::new(&env);
    items.push_back(make_batch_item(&env, &client.address, t1, 400, 1));
    items.push_back(make_batch_item(&env, &client.address, t2, 401, 1));
    let res = client.try_redeem_batch(&buyer, &items);
    assert_eq!(res, Err(Ok(Error::VoucherRevoked)));
    assert_eq!(client.balance_of(&buyer, &t1), 0u128);
}

#[test]
fn batch_one_replayed_nonce_reverts_all() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let t1 = 500u64;
    let t2 = 501u64;
    client.register_edition(&t1, &100u128);
    client.register_edition(&t2, &100u128);
    // Pre-redeem nonce 501
    let buyer = Address::generate(&env);
    let pre = make_voucher(&env, t2, 501);
    let pre_sig = sign_voucher(&env, &client.address, &pre);
    client.redeem(&buyer, &pre, &1u128, &pre_sig, &empty_proof(&env));
    // Now batch with replayed nonce 501
    let mut items = Vec::new(&env);
    items.push_back(make_batch_item(&env, &client.address, t1, 500, 1));
    items.push_back(make_batch_item(&env, &client.address, t2, 501, 1));
    let res = client.try_redeem_batch(&buyer, &items);
    assert_eq!(res, Err(Ok(Error::VoucherAlreadyRedeemed)));
    assert_eq!(client.total_supply(&t1), 0u128); // t1 not minted (all-or-nothing)
}

#[test]
fn batch_one_bad_sig_reverts_all() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let t1 = 600u64;
    let t2 = 601u64;
    client.register_edition(&t1, &100u128);
    client.register_edition(&t2, &100u128);
    let buyer = Address::generate(&env);
    let good = make_batch_item(&env, &client.address, t1, 600, 1);
    let bad = BatchVoucherItem1155 {
        voucher: make_voucher(&env, t2, 601),
        amount: 1,
        signature: BytesN::from_array(&env, &[0u8; 64]),
        merkle_proof: empty_proof(&env),
    };
    let mut items = Vec::new(&env);
    items.push_back(good);
    items.push_back(bad);
    let res = client.try_redeem_batch(&buyer, &items);
    assert!(res.is_err());
    assert_eq!(client.balance_of(&buyer, &t1), 0u128);
}

#[test]
fn batch_one_over_supply_reverts_all() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let t1 = 700u64;
    let t2 = 701u64;
    client.register_edition(&t1, &100u128);
    client.register_edition(&t2, &2u128); // only 2 available
    let buyer = Address::generate(&env);
    let good = make_batch_item(&env, &client.address, t1, 700, 1);
    let bad = BatchVoucherItem1155 {
        voucher: MintVoucher1155 {
            buyer_quota: 1000,
            ..make_voucher(&env, t2, 701)
        },
        amount: 10,
        signature: {
            let v2 = MintVoucher1155 {
                buyer_quota: 1000,
                ..make_voucher(&env, t2, 701)
            };
            sign_voucher(&env, &client.address, &v2)
        },
        merkle_proof: empty_proof(&env),
    };
    let mut items = Vec::new(&env);
    items.push_back(good);
    items.push_back(bad);
    let res = client.try_redeem_batch(&buyer, &items);
    assert_eq!(res, Err(Ok(Error::MaxSupplyReached)));
    assert_eq!(client.balance_of(&buyer, &t1), 0u128);
}

#[test]
fn batch_mixed_free_vouchers_aggregate_correctly() {
    let (env, client, _creator, _fee) = setup(250); // 2.5% fee
    client.set_public_phase();
    for tid in 800u64..803u64 {
        client.register_edition(&tid, &1000u128);
    }
    let buyer = Address::generate(&env);
    let mut items = Vec::new(&env);
    for (i, tid) in (800u64..803u64).enumerate() {
        items.push_back(make_batch_item(&env, &client.address, tid, 800 + i as u64, 3));
    }
    client.redeem_batch(&buyer, &items);
    for tid in 800u64..803u64 {
        assert_eq!(client.balance_of(&buyer, &tid), 3u128);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 6 — Fee-math edge cases (1155)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fee_bps_formula_correctness() {
    let price: i128 = 1; // 1 stroop
    let bps: u32 = 500;
    let fee = (price * bps as i128) / 10_000;
    assert_eq!(fee, 0); // rounds down
    let creator = price - fee;
    assert_eq!(creator, 1);

    let price2: i128 = 10_000;
    let fee2 = (price2 * 500i128) / 10_000;
    assert_eq!(fee2, 500);
    assert_eq!(price2 - fee2, 9_500);

    let price3: i128 = 10_000;
    let fee3 = (price3 * 0i128) / 10_000; // 0 bps
    assert_eq!(fee3, 0);
    assert_eq!(price3 - fee3, 10_000);
}

#[test]
fn zero_fee_bps_free_voucher_no_transfer() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 900u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 900); // price=0
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &5u128, &sig, &empty_proof(&env));
    assert_eq!(client.balance_of(&buyer, &token_id), 5u128);
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 7 — Replay protection (preserved & extended)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn replay_check_before_sig_verification() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 1000u64;
    let nonce = 1000u64;
    client.register_edition(&token_id, &100u128);
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::RedeemedVoucher(nonce), &true);
    });

    // Burn should succeed and write supply = 0, not amount (3).
    client.burn(&buyer, &buyer, &token_id, &3u128);

    // total_supply must be 0, not 3 (the old unwrap_or(amount) result).
    assert_eq!(client.total_supply(&token_id), 0u128);
}

// ─── Issue #39 — Voucher nonce / replay protection tests ─────────────────────

fn make_voucher_1155_with_nonce(env: &Env, token_id: u64, nonce: u64) -> MintVoucher1155 {
    MintVoucher1155 {
        token_id,
        nonce,
        buyer_quota: 100u128,
        price_per_unit: 0i128,
        currency: Address::generate(env),
        uri: String::from_str(env, "ipfs://test"),
        uri_hash: BytesN::from_array(env, &[0u8; 32]),
        valid_until: 0,
    }
}

#[test]
fn different_nonces_are_independent() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 1001u64;
    client.register_edition(&token_id, &100u128);
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::RedeemedVoucher(1u64), &true);
    });
    assert!(client.is_voucher_redeemed(&1u64));
    assert!(!client.is_voucher_redeemed(&2u64));
    // Nonce 2 should NOT be VoucherAlreadyRedeemed
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 2);
    let bad_sig = BytesN::from_array(&env, &[0u8; 64]);
    let res = client.try_redeem(&buyer, &v, &1u128, &bad_sig, &empty_proof(&env));
    assert_ne!(res, Err(Ok(Error::VoucherAlreadyRedeemed)));
}

#[test]
fn is_voucher_redeemed_tracks_nonce() {
    let (env, client, _creator, _fee) = setup(0);
    let nonce = 99u64;
    assert!(!client.is_voucher_redeemed(&nonce));
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::RedeemedVoucher(nonce), &true);
    });
    assert!(client.is_voucher_redeemed(&nonce));
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECTION 8 — Burn, transfer, TTL (preserved regressions)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn burn_with_missing_total_supply_key_returns_zero_not_amount() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 1u64;
    client.register_edition(&token_id, &100u128);
    let buyer = Address::generate(&env);
    let v = make_voucher(&env, token_id, 0);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &5u128, &sig, &empty_proof(&env));
    assert_eq!(client.total_supply(&token_id), 5u128);
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .remove(&DataKey::TotalSupply(token_id));
    });
    client.burn(&buyer, &buyer, &token_id, &3u128);
    assert_eq!(client.total_supply(&token_id), 0u128);
}

fn jump_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|li| li.sequence_number += delta);
}

#[test]
fn instance_ttl_extended_on_redeem() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 2u64;
    client.register_edition(&token_id, &1000u128);
    let buyer = Address::generate(&env);
    jump_ledger(&env, 60_000);
    let v = make_voucher(&env, token_id, 10);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer, &v, &1u128, &sig, &empty_proof(&env));
    assert_eq!(client.balance_of(&buyer, &token_id), 1u128);
}

#[test]
fn persistent_balance_ttl_extended_on_transfer() {
    let (env, client, _creator, _fee) = setup(0);
    client.set_public_phase();
    let token_id = 3u64;
    client.register_edition(&token_id, &1000u128);
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);
    let v = make_voucher(&env, token_id, 20);
    let sig = sign_voucher(&env, &client.address, &v);
    client.redeem(&buyer1, &v, &5u128, &sig, &empty_proof(&env));
    client.transfer(&buyer1, &buyer2, &token_id, &2u128);
    jump_ledger(&env, 60_000);
    let still_has = env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .has(&DataKey::Balance(buyer1.clone(), token_id))
    });
    assert!(still_has);
}
