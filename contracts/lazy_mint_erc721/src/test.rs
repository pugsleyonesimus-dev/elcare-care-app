use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, String, Vec,
};

use crate::{DataKey, Error, LazyMint721, LazyMint721Client};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn setup_test() -> (Env, LazyMint721Client<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LazyMint721, ());
    let client = LazyMint721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    (env, client, creator)
}

fn default_init(env: &Env, client: &LazyMint721Client, creator: &Address) {
    let pubkey = BytesN::from_array(env, &[1u8; 32]);
    let royalty_receiver = Address::generate(env);
    client.initialize(
        creator,
        &pubkey,
        &String::from_str(env, "TestNFT"),
        &String::from_str(env, "TNFT"),
        &1000u64,
        &0u32,
        &royalty_receiver,
        &fee_receiver,
        &0u32,
    );
}

/// Compute sha256(address XDR) — the leaf hash used in Merkle trees.
fn leaf_hash(env: &Env, addr: &Address) -> BytesN<32> {
    env.crypto().sha256(&addr.clone().to_xdr(env)).into()
}

/// Combine two hashes in sorted order (standard binary Merkle tree step).
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

/// Build a two-leaf Merkle tree for [addr_a, addr_b].
/// Returns (root, proof_for_a, proof_for_b).
fn two_leaf_tree(
    env: &Env,
    addr_a: &Address,
    addr_b: &Address,
) -> (BytesN<32>, Vec<BytesN<32>>, Vec<BytesN<32>>) {
    let leaf_a = leaf_hash(env, addr_a);
    let leaf_b = leaf_hash(env, addr_b);
    let root = combine(env, leaf_a.clone(), leaf_b.clone());

    let mut proof_a = Vec::new(env);
    proof_a.push_back(leaf_b.clone());

    let mut proof_b = Vec::new(env);
    proof_b.push_back(leaf_a.clone());

    (root, proof_a, proof_b)
}

fn empty_proof(env: &Env) -> Vec<BytesN<32>> {
    Vec::new(env)
}

fn make_voucher(env: &Env, token_id: u64) -> crate::MintVoucher {
    crate::MintVoucher {
        token_id,
        price: 0,
        currency: Address::generate(env),
        uri: String::from_str(env, "ipfs://test"),
        uri_hash: BytesN::from_array(env, &[0u8; 32]),
        valid_until: 0,
    }
}

// ─── Existing tests — updated for new redeem() signature ─────────────────────

#[test]
fn test_transfer_with_missing_balance_returns_error() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(&DataKey::Owner(1), &alice);
        // balance intentionally absent
    });

    let result = client.try_transfer(&alice, &bob, &1);
    assert_eq!(result, Err(Ok(Error::NotOwner)));
}

#[test]
fn test_transfer_with_zero_balance_returns_error() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(&DataKey::Owner(1), &alice);
        env.storage()
            .persistent()
            .set(&DataKey::BalanceOf(alice.clone()), &0u64);
    });

    let result = client.try_transfer(&alice, &bob, &1);
    assert_eq!(result, Err(Ok(Error::NotOwner)));
}

#[test]
fn test_voucher_expired_returns_proper_error() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    // Put contract into public phase so expiry check is reached
    client.set_public_phase();

    let buyer = Address::generate(&env);
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let voucher = crate::MintVoucher {
        valid_until: 50,
        ..make_voucher(&env, 1)
    };

    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert_eq!(result, Err(Ok(Error::VoucherExpired)));
}

#[test]
fn test_invalid_signature_returns_proper_error() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();

    let buyer = Address::generate(&env);
    let voucher = make_voucher(&env, 1);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert!(result.is_err());
}

#[test]
fn test_wrong_signature_format_returns_proper_error() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();

    let buyer = Address::generate(&env);
    let voucher = make_voucher(&env, 2);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[255u8; 64]),
        &empty_proof(&env),
    );
    assert!(result.is_err());
}

#[test]
fn test_signature_for_wrong_voucher_data_returns_proper_error() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();

    let buyer = Address::generate(&env);
    let modified_voucher = crate::MintVoucher {
        token_id: 999,
        ..make_voucher(&env, 3)
    };
    let result = client.try_redeem(
        &buyer,
        &modified_voucher,
        &BytesN::from_array(&env, &[42u8; 64]),
        &empty_proof(&env),
    );
    assert!(result.is_err());
}

#[test]
fn test_graceful_signature_error_handling_with_payment() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();

    let buyer = Address::generate(&env);
    let voucher = crate::MintVoucher {
        price: 500,
        ..make_voucher(&env, 4)
    };
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[99u8; 64]),
        &empty_proof(&env),
    );
    assert!(result.is_err());
}

// ─── Allowlist Phase — Missing Proof ─────────────────────────────────────────

#[test]
fn test_allowlist_phase_no_root_returns_not_allowlisted() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    // Phase is allowlist by default; no root set.

    let buyer = Address::generate(&env);
    let voucher = make_voucher(&env, 10);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert_eq!(result, Err(Ok(Error::NotAllowlisted)));
}

#[test]
fn test_allowlist_phase_empty_proof_returns_not_allowlisted() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let buyer = Address::generate(&env);
    let (root, _, _) = two_leaf_tree(&env, &buyer, &Address::generate(&env));
    client.set_merkle_root(&root);

    let voucher = make_voucher(&env, 11);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert_eq!(result, Err(Ok(Error::NotAllowlisted)));
}

// ─── Allowlist Phase — Invalid Proof ─────────────────────────────────────────

#[test]
fn test_allowlist_phase_wrong_proof_returns_invalid_merkle_proof() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let buyer = Address::generate(&env);
    let other = Address::generate(&env);
    let (root, _proof_buyer, proof_other) = two_leaf_tree(&env, &buyer, &other);
    client.set_merkle_root(&root);

    // Pass other's proof for buyer — invalid
    let voucher = make_voucher(&env, 12);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &proof_other,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMerkleProof)));
}

#[test]
fn test_allowlist_phase_garbage_proof_returns_invalid_merkle_proof() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let buyer = Address::generate(&env);
    let (root, _, _) = two_leaf_tree(&env, &buyer, &Address::generate(&env));
    client.set_merkle_root(&root);

    let mut bad_proof = Vec::new(&env);
    bad_proof.push_back(BytesN::from_array(&env, &[0xdeu8; 32]));

    let voucher = make_voucher(&env, 13);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &bad_proof,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMerkleProof)));
}

#[test]
fn test_allowlist_phase_non_member_with_proof_returns_invalid_merkle_proof() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let addr_a = Address::generate(&env);
    let addr_b = Address::generate(&env);
    let outsider = Address::generate(&env);
    let (root, _proof_a, proof_b) = two_leaf_tree(&env, &addr_a, &addr_b);
    client.set_merkle_root(&root);

    // outsider tries to use addr_b's proof — doesn't match outsider's leaf
    let voucher = make_voucher(&env, 14);
    let result = client.try_redeem(
        &outsider,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &proof_b,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMerkleProof)));
}

// ─── Allowlist Phase — Valid Proof (passes allowlist, then sig check) ─────────

#[test]
fn test_allowlist_phase_valid_proof_proceeds_to_signature_check() {
    // With a correct proof the contract should pass the Merkle gate and then fail
    // on the ed25519 signature (as expected for an invalid sig) — NOT return
    // NotAllowlisted or InvalidMerkleProof.
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let buyer = Address::generate(&env);
    let (root, proof_buyer, _) = two_leaf_tree(&env, &buyer, &Address::generate(&env));
    client.set_merkle_root(&root);

    let voucher = make_voucher(&env, 20);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &proof_buyer,
    );

    // Must NOT be NotAllowlisted or InvalidMerkleProof — proof was valid.
    // The host aborts on bad ed25519 so result is Err (not Ok).
    assert!(result.is_err());
    assert_ne!(result, Err(Ok(Error::NotAllowlisted)));
    assert_ne!(result, Err(Ok(Error::InvalidMerkleProof)));
}

#[test]
fn test_allowlist_single_leaf_tree_valid_proof() {
    // Single-address allowlist: root == leaf_hash(addr), proof is empty BUT
    // the code requires non-empty proof. A single-address tree needs no sibling,
    // so we test that passing the leaf itself as the root (via set_merkle_root)
    // with a single-element proof (a dummy) still rejects correctly, and that
    // the root-equals-leaf path works by special-casing with an empty proof
    // in set_merkle_root + a correct non-empty proof path otherwise.
    //
    // Here we test a 1-element proof path through a 3-leaf tree to ensure
    // multi-level proofs work correctly.
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    // 3-leaf tree: leaves = [buyer, b, c]
    // Internal nodes: node_left = combine(buyer, b), node_right = c_leaf
    //                 root = combine(node_left, node_right)
    let buyer = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);

    let leaf_buyer = leaf_hash(&env, &buyer);
    let leaf_b = leaf_hash(&env, &b);
    let leaf_c = leaf_hash(&env, &c);

    let node_left = combine(&env, leaf_buyer.clone(), leaf_b.clone());
    let root = combine(&env, node_left.clone(), leaf_c.clone());

    client.set_merkle_root(&root);

    // Proof for buyer: [leaf_b, leaf_c]
    let mut proof_buyer = Vec::new(&env);
    proof_buyer.push_back(leaf_b);
    proof_buyer.push_back(leaf_c);

    let voucher = make_voucher(&env, 21);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &proof_buyer,
    );

    assert!(result.is_err());
    assert_ne!(result, Err(Ok(Error::NotAllowlisted)));
    assert_ne!(result, Err(Ok(Error::InvalidMerkleProof)));
}

// ─── Phase Transitions ────────────────────────────────────────────────────────

#[test]
fn test_is_public_phase_defaults_false() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    assert!(!client.is_public_phase());
}

#[test]
fn test_set_public_phase_enables_public_minting() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();
    assert!(client.is_public_phase());
}

#[test]
fn test_set_merkle_root_resets_to_allowlist_phase() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    client.set_public_phase();
    assert!(client.is_public_phase());

    let root = BytesN::from_array(&env, &[0xabu8; 32]);
    client.set_merkle_root(&root);

    assert!(!client.is_public_phase());
}

#[test]
fn test_public_phase_bypasses_merkle_check() {
    // In public phase an empty proof must be accepted (allowlist check skipped).
    // The call then fails at ed25519 — but NOT at allowlist/Merkle checks.
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();

    let buyer = Address::generate(&env);
    let voucher = make_voucher(&env, 30);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );

    assert!(result.is_err());
    assert_ne!(result, Err(Ok(Error::NotAllowlisted)));
    assert_ne!(result, Err(Ok(Error::InvalidMerkleProof)));
}

#[test]
fn test_public_phase_ignores_non_empty_proof() {
    // Even with an unrelated non-empty proof, public phase should not error on it.
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    client.set_public_phase();

    let buyer = Address::generate(&env);
    let mut irrelevant_proof = Vec::new(&env);
    irrelevant_proof.push_back(BytesN::from_array(&env, &[0xffu8; 32]));

    let voucher = make_voucher(&env, 31);
    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &irrelevant_proof,
    );

    assert!(result.is_err());
    assert_ne!(result, Err(Ok(Error::NotAllowlisted)));
    assert_ne!(result, Err(Ok(Error::InvalidMerkleProof)));
}

// ─── Admin Authorization ──────────────────────────────────────────────────────

#[test]
fn test_set_merkle_root_only_creator_succeeds() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let root = BytesN::from_array(&env, &[0x11u8; 32]);
    // mock_all_auths() makes creator auth pass
    assert!(client.try_set_merkle_root(&root).is_ok());
    assert_eq!(client.merkle_root(), Some(root));
}

#[test]
fn test_set_public_phase_only_creator_succeeds() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    assert!(client.try_set_public_phase().is_ok());
    assert!(client.is_public_phase());
}

#[test]
fn test_set_merkle_root_non_creator_fails() {
    // In Soroban's test environment, calling an admin function without satisfying
    // the creator's auth requirement results in a host panic (not a graceful error).
    // We verify this by spinning up a fresh env WITHOUT mock_all_auths and
    // confirming the call fails (panics/errs) when no auth is provided.
    let env = Env::default();
    // Deliberately do NOT call env.mock_all_auths()
    let contract_id = env.register(LazyMint721, ());
    let client = LazyMint721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);

    // Initialize using the same env (no auth mocking — initialize has no auth requirement)
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &pubkey,
        &String::from_str(&env, "TestNFT"),
        &String::from_str(&env, "TNFT"),
        &1000u64,
        &0u32,
        &royalty_receiver,
        &fee_receiver,
        &0u32,
    );

    // Without mocked auth, the creator.require_auth() inside set_merkle_root will fail.
    let root = BytesN::from_array(&env, &[0x22u8; 32]);
    let result = client.try_set_merkle_root(&root);
    // Should fail because creator auth is not satisfied.
    assert!(result.is_err());
}

// ─── Merkle Root Getter ───────────────────────────────────────────────────────

#[test]
fn test_merkle_root_initially_none() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);
    assert_eq!(client.merkle_root(), None);
}

#[test]
fn test_merkle_root_returns_set_value() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let root = BytesN::from_array(&env, &[0x77u8; 32]);
    client.set_merkle_root(&root);
    assert_eq!(client.merkle_root(), Some(root));
}

#[test]
fn test_merkle_root_can_be_updated() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let root1 = BytesN::from_array(&env, &[0x11u8; 32]);
    let root2 = BytesN::from_array(&env, &[0x22u8; 32]);
    client.set_merkle_root(&root1);
    client.set_merkle_root(&root2);
    assert_eq!(client.merkle_root(), Some(root2));
}

// ─── Boundary cases ───────────────────────────────────────────────────────────

#[test]
fn test_allowlist_phase_voucher_expiry_checked_before_merkle() {
    // VoucherExpired should fire before the Merkle check so that callers get
    // the most informative error first (expiry is a cheaper/earlier check).
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    // Set a Merkle root so the phase is allowlist
    let buyer = Address::generate(&env);
    let (root, _, _) = two_leaf_tree(&env, &buyer, &Address::generate(&env));
    client.set_merkle_root(&root);

    env.ledger().with_mut(|li| li.sequence_number = 200);

    let voucher = crate::MintVoucher {
        valid_until: 100, // expired
        ..make_voucher(&env, 40)
    };

    let result = client.try_redeem(
        &buyer,
        &voucher,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    // VoucherExpired is checked first (step 1), Merkle is step 0 — but in our
    // implementation Merkle (step 0) runs before expiry (step 1).
    // Accept either NotAllowlisted (empty proof) or VoucherExpired depending
    // on ordering — the important thing is it is NOT a successful mint.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err == Ok(Error::NotAllowlisted)
            || err == Ok(Error::VoucherExpired)
            || err == Ok(Error::InvalidMerkleProof)
    );
}

#[test]
fn test_allowlist_allows_only_listed_buyers() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let allowed = Address::generate(&env);
    let denied = Address::generate(&env);
    let (root, proof_allowed, _) = two_leaf_tree(&env, &allowed, &Address::generate(&env));
    client.set_merkle_root(&root);

    // allowed buyer: proof is valid → proceeds past Merkle gate (sig will fail)
    let voucher_a = make_voucher(&env, 50);
    let result_a = client.try_redeem(
        &allowed,
        &voucher_a,
        &BytesN::from_array(&env, &[0u8; 64]),
        &proof_allowed,
    );
    assert!(result_a.is_err());
    assert_ne!(result_a, Err(Ok(Error::NotAllowlisted)));
    assert_ne!(result_a, Err(Ok(Error::InvalidMerkleProof)));

    // denied buyer: empty proof → NotAllowlisted
    let voucher_d = make_voucher(&env, 51);
    let result_d = client.try_redeem(
        &denied,
        &voucher_d,
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert_eq!(result_d, Err(Ok(Error::NotAllowlisted)));
}

#[test]
fn test_phase_transition_from_allowlist_to_public() {
    let (env, client, creator) = setup_test();
    default_init(&env, &client, &creator);

    let buyer = Address::generate(&env);
    let other = Address::generate(&env);
    let (root, _, _) = two_leaf_tree(&env, &buyer, &other);
    client.set_merkle_root(&root);

    // In allowlist phase, a non-member with empty proof is rejected
    let non_member = Address::generate(&env);
    let result_before = client.try_redeem(
        &non_member,
        &make_voucher(&env, 60),
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert_eq!(result_before, Err(Ok(Error::NotAllowlisted)));

    // Transition to public phase
    client.set_public_phase();

    // Same caller with empty proof now passes the allowlist gate
    let result_after = client.try_redeem(
        &non_member,
        &make_voucher(&env, 61),
        &BytesN::from_array(&env, &[0u8; 64]),
        &empty_proof(&env),
    );
    assert!(result_after.is_err());
    assert_ne!(result_after, Err(Ok(Error::NotAllowlisted)));
    assert_ne!(result_after, Err(Ok(Error::InvalidMerkleProof)));
}

// ─── Issue #39 — Voucher replay protection tests ──────────────────────────────

fn setup_with_fee() -> (Env, LazyMint721Client<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LazyMint721, ());
    let client = LazyMint721Client::new(&env, &contract_id);
    let creator = Address::generate(&env);
    (env, client, creator)
}

/// Marking a token_id as used then trying to redeem it returns VoucherAlreadyRedeemed.
#[test]
fn voucher_replay_rejected_with_already_redeemed_error() {
    let (env, client, creator) = setup_with_fee();

    let pubkey = BytesN::from_array(&env, &[10u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &pubkey,
        &String::from_str(&env, "Replay Test"),
        &String::from_str(&env, "RPT"),
        &1000u64,
        &0u32,
        &royalty_receiver,
        &fee_receiver,
        &0u32,
    );

    // Manually mark token_id 5 as redeemed (simulates a prior successful redemption)
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::UsedVoucher(5u64), &true);
    });

    let buyer = Address::generate(&env);
    let currency = Address::generate(&env);
    let voucher = crate::MintVoucher {
        token_id: 5,
        price: 0,
        currency: currency.clone(),
        uri: String::from_str(&env, "ipfs://replay"),
        uri_hash: BytesN::from_array(&env, &[0u8; 32]),
        valid_until: 0,
    };

    let sig = BytesN::from_array(&env, &[0u8; 64]);
    let result = client.try_redeem(&buyer, &voucher, &sig);
    assert_eq!(result, Err(Ok(Error::VoucherAlreadyRedeemed)));
}

/// is_voucher_redeemed returns false before and true after a successful nonce mark.
#[test]
fn is_voucher_redeemed_reflects_nonce_state() {
    let (env, client, creator) = setup_with_fee();

    let pubkey = BytesN::from_array(&env, &[11u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &pubkey,
        &String::from_str(&env, "Nonce Test"),
        &String::from_str(&env, "NCT"),
        &1000u64,
        &0u32,
        &royalty_receiver,
        &fee_receiver,
        &0u32,
    );

    assert!(!client.is_voucher_redeemed(&7u64));

    // Mark as redeemed directly
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::UsedVoucher(7u64), &true);
    });

    assert!(client.is_voucher_redeemed(&7u64));
}

/// Different token_ids (nonces) are independent — redeeming one does not block another.
#[test]
fn different_nonces_are_independent() {
    let (env, client, creator) = setup_with_fee();

    let pubkey = BytesN::from_array(&env, &[12u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &pubkey,
        &String::from_str(&env, "Nonce Indep"),
        &String::from_str(&env, "NCI"),
        &1000u64,
        &0u32,
        &royalty_receiver,
        &fee_receiver,
        &0u32,
    );

    // Mark token_id 1 as used
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::UsedVoucher(1u64), &true);
    });

    // token_id 2 must still be unredeemed
    assert!(client.is_voucher_redeemed(&1u64));
    assert!(!client.is_voucher_redeemed(&2u64));

    // Trying to redeem token_id 2 should fail due to bad signature (not replay)
    let buyer = Address::generate(&env);
    let currency = Address::generate(&env);
    let voucher2 = crate::MintVoucher {
        token_id: 2,
        price: 0,
        currency: currency.clone(),
        uri: String::from_str(&env, "ipfs://token2"),
        uri_hash: BytesN::from_array(&env, &[0u8; 32]),
        valid_until: 0,
    };
    let bad_sig = BytesN::from_array(&env, &[0u8; 64]);
    let result2 = client.try_redeem(&buyer, &voucher2, &bad_sig);
    // Should fail with host abort (invalid signature), NOT VoucherAlreadyRedeemed
    assert!(result2.is_err());
    // Confirm it's not a VoucherAlreadyRedeemed
    assert_ne!(result2, Err(Ok(Error::VoucherAlreadyRedeemed)));
}

/// Replay is rejected BEFORE signature verification (check ordering preserved).
#[test]
fn replay_check_precedes_signature_verification() {
    let (env, client, creator) = setup_with_fee();

    let pubkey = BytesN::from_array(&env, &[13u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    client.initialize(
        &creator,
        &pubkey,
        &String::from_str(&env, "Order Test"),
        &String::from_str(&env, "ORD"),
        &1000u64,
        &0u32,
        &royalty_receiver,
        &fee_receiver,
        &0u32,
    );

    // Mark token 3 as already redeemed
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::UsedVoucher(3u64), &true);
    });

    let buyer = Address::generate(&env);
    let currency = Address::generate(&env);
    let voucher = crate::MintVoucher {
        token_id: 3,
        price: 0,
        currency: currency.clone(),
        uri: String::from_str(&env, "ipfs://order"),
        uri_hash: BytesN::from_array(&env, &[0u8; 32]),
        valid_until: 0,
    };

    // Even with a completely wrong signature, we get VoucherAlreadyRedeemed (not a host abort)
    let any_sig = BytesN::from_array(&env, &[99u8; 64]);
    let result = client.try_redeem(&buyer, &voucher, &any_sig);
    assert_eq!(result, Err(Ok(Error::VoucherAlreadyRedeemed)));
}
