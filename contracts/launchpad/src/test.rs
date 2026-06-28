extern crate std;

use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env, String};

use crate::{CollectionKind, Error, Launchpad, LaunchpadClient};

fn jump_ledger(env: &Env, delta: u32) {
    env.ledger().with_mut(|li| {
        li.sequence_number += delta;
    });
}

fn wasm_bytes(name: &str) -> std::vec::Vec<u8> {
    // In Cursor's sandbox, cargo builds into an isolated target dir (not `./target`).
    // Derive the target dir from the current test binary path:
    //   .../cargo-target/debug/deps/<test-binary>
    let exe = std::env::current_exe().unwrap();
    let target_dir = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();
    let path = target_dir
        .join("wasm32v1-none")
        .join("release")
        .join(std::format!("{name}.wasm"));

    std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "missing wasm at {}. build it first with: cargo build --target wasm32v1-none --release -p collection-nft-erc1155 -p lazy-mint-erc721 -p collection-nft-erc721 -p lazy-mint-erc1155",
            path.display()
        )
    })
}

fn setup_launchpad(env: &Env) -> (LaunchpadClient<'_>, Address, Address, Address) {
    env.mock_all_auths();

    let launchpad_id = env.register(Launchpad, ());
    let client = LaunchpadClient::new(env, &launchpad_id);

    let admin = Address::generate(env);
    let fee_receiver = Address::generate(env);
    let creator = Address::generate(env);

    client.initialize(&admin, &fee_receiver, &0u32);

    let wasm_normal_721_bytes = wasm_bytes("collection_nft_erc721");
    let wasm_normal_1155_bytes = wasm_bytes("collection_nft_erc1155");
    let wasm_lazy_721_bytes = wasm_bytes("lazy_mint_erc721");
    let wasm_lazy_1155_bytes = wasm_bytes("lazy_mint_erc1155");

    let wasm_normal_721 = env
        .deployer()
        .upload_contract_wasm(wasm_normal_721_bytes.as_slice());
    let wasm_normal_1155 = env
        .deployer()
        .upload_contract_wasm(wasm_normal_1155_bytes.as_slice());
    let wasm_lazy_721 = env
        .deployer()
        .upload_contract_wasm(wasm_lazy_721_bytes.as_slice());
    let wasm_lazy_1155 = env
        .deployer()
        .upload_contract_wasm(wasm_lazy_1155_bytes.as_slice());

    client.set_wasm_hashes(
        &wasm_normal_721,
        &wasm_normal_1155,
        &wasm_lazy_721,
        &wasm_lazy_1155,
    );

    (client, admin, fee_receiver, creator)
}

#[test]
fn deploys_normal_721_twice_with_unique_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt_a = BytesN::from_array(&env, &[10u8; 32]);
    let salt_b = BytesN::from_array(&env, &[11u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let deployed_a = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Creator 721 A"),
        &String::from_str(&env, "C721A"),
        &1_000u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt_a,
    );

    let deployed_b = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Creator 721 B"),
        &String::from_str(&env, "C721B"),
        &1_500u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt_b,
    );

    assert_ne!(deployed_a, deployed_b);
    assert_eq!(client.collection_count(), 2u64);

    let all = client.all_collections();
    assert_eq!(all.len(), 2);
    assert!(matches!(
        all.get(0).unwrap().kind,
        CollectionKind::Normal721
    ));
    assert!(matches!(
        all.get(1).unwrap().kind,
        CollectionKind::Normal721
    ));
}

#[test]
fn deploys_normal_1155_twice_with_unique_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt_a = BytesN::from_array(&env, &[20u8; 32]);
    let salt_b = BytesN::from_array(&env, &[21u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let deployed_a = client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "Creator 1155 A"),
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt_a,
    );

    let deployed_b = client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "Creator 1155 B"),
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt_b,
    );

    assert_ne!(deployed_a, deployed_b);
    assert_eq!(client.collection_count(), 2u64);

    let all = client.all_collections();
    assert_eq!(all.len(), 2);
    assert!(matches!(
        all.get(0).unwrap().kind,
        CollectionKind::Normal1155
    ));
    assert!(matches!(
        all.get(1).unwrap().kind,
        CollectionKind::Normal1155
    ));
}

#[test]
fn deploys_lazy_721_twice_with_unique_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt_a = BytesN::from_array(&env, &[30u8; 32]);
    let salt_b = BytesN::from_array(&env, &[31u8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[7u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let deployed_a = client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Lazy 721 A"),
        &String::from_str(&env, "LZ7A"),
        &1_000u64,
        &750u32,
        &royalty_receiver,
        &0u32,
        &salt_a,
    );

    let deployed_b = client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Lazy 721 B"),
        &String::from_str(&env, "LZ7B"),
        &1_200u64,
        &750u32,
        &royalty_receiver,
        &0u32,
        &salt_b,
    );

    assert_ne!(deployed_a, deployed_b);
    assert_eq!(client.collection_count(), 2u64);

    let all = client.all_collections();
    assert_eq!(all.len(), 2);
    assert!(matches!(
        all.get(0).unwrap().kind,
        CollectionKind::LazyMint721
    ));
    assert!(matches!(
        all.get(1).unwrap().kind,
        CollectionKind::LazyMint721
    ));
}

#[test]
fn deploys_lazy_1155_twice_with_unique_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt_a = BytesN::from_array(&env, &[40u8; 32]);
    let salt_b = BytesN::from_array(&env, &[41u8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[9u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let deployed_a = client.deploy_lazy_1155(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Lazy 1155 A"),
        &600u32,
        &royalty_receiver,
        &0u32,
        &salt_a,
    );

    let deployed_b = client.deploy_lazy_1155(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Lazy 1155 B"),
        &600u32,
        &royalty_receiver,
        &0u32,
        &salt_b,
    );

    assert_ne!(deployed_a, deployed_b);
    assert_eq!(client.collection_count(), 2u64);

    let all = client.all_collections();
    assert_eq!(all.len(), 2);
    assert!(matches!(
        all.get(0).unwrap().kind,
        CollectionKind::LazyMint1155
    ));
    assert!(matches!(
        all.get(1).unwrap().kind,
        CollectionKind::LazyMint1155
    ));
}

#[test]
fn deploy_calls_extend_instance_ttl() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    // After initialize(), instance TTL is bumped to 100_000 ledgers.
    // Move forward so remaining TTL is below threshold (50_000),
    // then call deploy_* which should bump instance TTL again.
    jump_ledger(&env, 60_000);

    let salt_a = BytesN::from_array(&env, &[60u8; 32]);
    let _deployed_a = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "TTL A"),
        &String::from_str(&env, "TTLA"),
        &100u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt_a,
    );

    // Without TTL extension on deploy, instance storage would now be expired:
    // 60_000 + 60_000 > 100_000.
    jump_ledger(&env, 60_000);

    let salt_b = BytesN::from_array(&env, &[61u8; 32]);
    let _deployed_b = client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "TTL B"),
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt_b,
    );

    assert_eq!(client.collection_count(), 2u64);
}

#[test]
fn admin_calls_extend_instance_ttl() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, _creator) = setup_launchpad(&env);

    jump_ledger(&env, 60_000);

    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);

    jump_ledger(&env, 60_000);

    assert_eq!(client.admin(), new_admin);
}

// ─── Issue #53 — Salt front-running / griefing tests ─────────────────────────
//
// The fix: secure_salt = sha256(creator.to_xdr() ‖ raw_salt)
//
// Two categories of tests:
//   A. Same raw salt from two different creators → different deployed addresses.
//   B. Front-runner copies Alice's raw salt and transacts first → Alice's
//      subsequent transaction still succeeds (different address).

// ── Category A: Per-creator namespace isolation ──────────────────────────────

/// deploy_normal_721: same raw salt, different creators ⟹ different addresses.
#[test]
fn same_salt_different_creators_normal_721_yields_different_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0xAAu8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_alice = client.deploy_normal_721(
        &alice,
        &currency,
        &String::from_str(&env, "Alice 721"),
        &String::from_str(&env, "AL7"),
        &100u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_bob = client.deploy_normal_721(
        &bob,
        &currency,
        &String::from_str(&env, "Bob 721"),
        &String::from_str(&env, "BO7"),
        &100u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt, // identical raw salt
    );

    // Because secure_salt = sha256(creator ‖ raw_salt) they must differ.
    assert_ne!(
        addr_alice, addr_bob,
        "same raw salt must not collide across creators"
    );
    assert_eq!(client.collection_count(), 2u64);
}

/// deploy_normal_1155: same raw salt, different creators ⟹ different addresses.
#[test]
fn same_salt_different_creators_normal_1155_yields_different_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0xBBu8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_alice = client.deploy_normal_1155(
        &alice,
        &currency,
        &String::from_str(&env, "Alice 1155"),
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_bob = client.deploy_normal_1155(
        &bob,
        &currency,
        &String::from_str(&env, "Bob 1155"),
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
}

/// deploy_lazy_721: same raw salt, different creators ⟹ different addresses.
#[test]
fn same_salt_different_creators_lazy_721_yields_different_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0xCCu8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[0x01u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_alice = client.deploy_lazy_721(
        &alice,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Alice L721"),
        &String::from_str(&env, "AL7L"),
        &500u64,
        &300u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_bob = client.deploy_lazy_721(
        &bob,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Bob L721"),
        &String::from_str(&env, "BO7L"),
        &500u64,
        &300u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
}

/// deploy_lazy_1155: same raw salt, different creators ⟹ different addresses.
#[test]
fn same_salt_different_creators_lazy_1155_yields_different_addresses() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0xDDu8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[0x02u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_alice = client.deploy_lazy_1155(
        &alice,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Alice L1155"),
        &400u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_bob = client.deploy_lazy_1155(
        &bob,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Bob L1155"),
        &400u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
}

// ── Category B: Front-runner cannot block the victim ─────────────────────────
//
// Bob front-runs with the same raw salt as Alice.  After the fix, Bob's
// deploy lands at sha256(Bob ‖ salt).  Alice's subsequent deploy lands at
// sha256(Alice ‖ salt) — a distinct address — so her tx must succeed.

/// deploy_normal_721: front-runner copies Alice's salt → Alice still succeeds.
#[test]
fn front_runner_cannot_grief_normal_721() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env); // malicious actor

    let salt = BytesN::from_array(&env, &[0x11u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    // Bob front-runs using Alice's raw salt.
    let addr_bob = client.deploy_normal_721(
        &bob,
        &currency,
        &String::from_str(&env, "Bob Grief 721"),
        &String::from_str(&env, "BG7"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    // Alice's transaction must still succeed (no panic / error).
    let addr_alice = client.deploy_normal_721(
        &alice,
        &currency,
        &String::from_str(&env, "Alice 721"),
        &String::from_str(&env, "AL7"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(
        addr_alice, addr_bob,
        "front-runner must not occupy Alice's slot"
    );
    assert_eq!(client.collection_count(), 2u64);
}

/// deploy_normal_1155: front-runner copies Alice's salt → Alice still succeeds.
#[test]
fn front_runner_cannot_grief_normal_1155() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0x22u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_bob = client.deploy_normal_1155(
        &bob,
        &currency,
        &String::from_str(&env, "Bob Grief 1155"),
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_alice = client.deploy_normal_1155(
        &alice,
        &currency,
        &String::from_str(&env, "Alice 1155"),
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
}

/// deploy_lazy_721: front-runner copies Alice's salt → Alice still succeeds.
#[test]
fn front_runner_cannot_grief_lazy_721() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0x33u8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[0x03u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_bob = client.deploy_lazy_721(
        &bob,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Bob Grief L721"),
        &String::from_str(&env, "BGL7"),
        &200u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_alice = client.deploy_lazy_721(
        &alice,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Alice L721"),
        &String::from_str(&env, "ALL7"),
        &200u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
}

/// deploy_lazy_1155: front-runner copies Alice's salt → Alice still succeeds.
#[test]
fn front_runner_cannot_grief_lazy_1155() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, alice) = setup_launchpad(&env);
    let bob = Address::generate(&env);

    let salt = BytesN::from_array(&env, &[0x44u8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[0x04u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    let addr_bob = client.deploy_lazy_1155(
        &bob,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Bob Grief L1155"),
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let addr_alice = client.deploy_lazy_1155(
        &alice,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Alice L1155"),
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
}

// ── Category C: Duplicate (creator, salt) deploy reverts cleanly ─────────────
//
// Deploying with the same creator AND same raw salt a second time must revert
// because the derived secure_salt (sha256(creator ‖ raw_salt)) is identical,
// so the factory would try to instantiate a contract at an already-occupied
// deterministic address — the Soroban VM rejects this with a host error.

/// deploy_normal_721: same creator, same salt → second deploy reverts.
#[test]
#[should_panic]
fn duplicate_creator_salt_normal_721_reverts() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);
    let salt = BytesN::from_array(&env, &[0xE1u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "First 721"),
        &String::from_str(&env, "F721"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );
    // Second call with identical creator + salt must panic.
    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Dupe 721"),
        &String::from_str(&env, "D721"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );
}

/// deploy_normal_1155: same creator, same salt → second deploy reverts.
#[test]
#[should_panic]
fn duplicate_creator_salt_normal_1155_reverts() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);
    let salt = BytesN::from_array(&env, &[0xE2u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "First 1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );
    client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "Dupe 1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );
}

/// deploy_lazy_721: same creator, same salt → second deploy reverts.
#[test]
#[should_panic]
fn duplicate_creator_salt_lazy_721_reverts() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);
    let salt = BytesN::from_array(&env, &[0xE3u8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[0x01u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "First L721"),
        &String::from_str(&env, "FL72"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );
    client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Dupe L721"),
        &String::from_str(&env, "DL72"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );
}

/// deploy_lazy_1155: same creator, same salt → second deploy reverts.
#[test]
#[should_panic]
fn duplicate_creator_salt_lazy_1155_reverts() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);
    let salt = BytesN::from_array(&env, &[0xE4u8; 32]);
    let creator_pubkey = BytesN::from_array(&env, &[0x02u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);

    client.deploy_lazy_1155(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "First L1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );
    client.deploy_lazy_1155(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Dupe L1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );
}

// ── Initialisation error tests ──────────────────────────────────

#[test]
fn initialize_twice_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let launchpad_id = env.register(Launchpad, ());
    let client = LaunchpadClient::new(&env, &launchpad_id);

    let admin = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    client.initialize(&admin, &fee_receiver, &0u32);

    let result = client.try_initialize(&admin, &fee_receiver, &0u32);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn deploy_without_wasm_hashes_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let launchpad_id = env.register(Launchpad, ());
    let client = LaunchpadClient::new(&env, &launchpad_id);

    let admin = Address::generate(&env);
    let fee_receiver = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin, &fee_receiver, &0u32);

    let salt = BytesN::from_array(&env, &[0x99u8; 32]);
    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    let result = client.try_deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "No Wasm"),
        &String::from_str(&env, "NOWASM"),
        &100u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );
    assert_eq!(result, Err(Ok(Error::WasmHashNotSet)));
}

// ── Admin function tests ────────────────────────────────────────

#[test]
fn admin_calls_before_init_fail() {
    let env = Env::default();
    env.mock_all_auths();

    let launchpad_id = env.register(Launchpad, ());
    let client = LaunchpadClient::new(&env, &launchpad_id);

    let new_admin = Address::generate(&env);
    let result = client.try_transfer_admin(&new_admin);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));

    let result = client.try_update_platform_fee(&Address::generate(&env), &100u32);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn transfer_admin_success() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, _creator) = setup_launchpad(&env);

    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);

    assert_eq!(client.admin(), new_admin);
}

#[test]
fn update_platform_fee_success() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, _creator) = setup_launchpad(&env);

    let new_receiver = Address::generate(&env);
    let new_fee_bps = 250u32;
    client.update_platform_fee(&new_receiver, &new_fee_bps);

    let (receiver, bps) = client.platform_fee();
    assert_eq!(receiver, new_receiver);
    assert_eq!(bps, new_fee_bps);
}

// ── View function tests ─────────────────────────────────────────

#[test]
fn view_functions_return_correct_values() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, admin, fee_receiver, _creator) = setup_launchpad(&env);

    assert_eq!(client.admin(), admin);

    let (receiver, bps) = client.platform_fee();
    assert_eq!(receiver, fee_receiver);
    assert_eq!(bps, 0u32);
}

// ── Collections view tests ──────────────────────────────────────

#[test]
fn collections_by_creator_returns_correct_collections() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let other = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[0x55u8; 32]);
    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Creator Coll"),
        &String::from_str(&env, "CRC"),
        &100u64,
        &500u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let creator_colls = client.collections_by_creator(&creator);
    assert_eq!(creator_colls.len(), 1);
    assert!(matches!(
        creator_colls.get(0).unwrap().kind,
        CollectionKind::Normal721
    ));

    let other_colls = client.collections_by_creator(&other);
    assert_eq!(other_colls.len(), 0);
}

// ── Issue #201: Invalid ED25519 signature and expired voucher tests ───────────
//
// Deploy a lazy_721 via the launchpad, then verify the deployed collection
// rejects invalid ED25519 signatures and expired vouchers.
//
// We mirror the MintVoucher / Error types from lazy_mint_erc721 using the same
// #[contracttype] / #[contracterror] macros so the XDR encoding matches.

use soroban_sdk::{contractclient, contracterror, contracttype};

#[contracttype]
#[derive(Clone)]
pub struct MintVoucher {
    pub token_id: u64,
    pub price: i128,
    pub currency: Address,
    pub uri: String,
    pub uri_hash: BytesN<32>,
    pub valid_until: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LazyError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotOwner = 3,
    NotApproved = 4,
    TokenNotFound = 5,
    MaxSupplyReached = 6,
    VoucherExpired = 7,
    VoucherAlreadyRedeemed = 8,
    NotCreator = 9,
    InvalidSignature = 10,
}

#[contractclient(name = "Lazy721Client")]
pub trait ILazy721 {
    fn redeem(
        env: Env,
        buyer: Address,
        voucher: MintVoucher,
        signature: BytesN<64>,
    ) -> Result<u64, LazyError>;
}

/// After deploying a lazy_721 via the launchpad, redeeming with an invalid
/// ED25519 signature must be rejected by the deployed collection contract.
#[test]
fn deployed_lazy_721_rejects_invalid_ed25519_signature() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let creator_pubkey = BytesN::from_array(&env, &[1u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[0xA1u8; 32]);

    let collection_addr = client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Sig Test 721"),
        &String::from_str(&env, "ST7"),
        &1_000u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let lazy_client = Lazy721Client::new(&env, &collection_addr);
    let buyer = Address::generate(&env);
    let voucher = MintVoucher {
        token_id: 1,
        price: 0,
        currency: Address::generate(&env),
        uri: String::from_str(&env, "ipfs://test"),
        uri_hash: BytesN::from_array(&env, &[0u8; 32]),
        valid_until: 0,
    };

    // All-zeros is not a valid ed25519 signature — host will abort
    let bad_sig = BytesN::from_array(&env, &[0u8; 64]);
    let result = lazy_client.try_redeem(&buyer, &voucher, &bad_sig);
    assert!(result.is_err(), "invalid signature must be rejected");
}

/// After deploying a lazy_721 via the launchpad, redeeming an expired voucher
/// (valid_until < current ledger sequence) must return VoucherExpired.
#[test]
fn deployed_lazy_721_rejects_expired_voucher() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let creator_pubkey = BytesN::from_array(&env, &[2u8; 32]);
    let royalty_receiver = Address::generate(&env);
    let currency = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[0xA2u8; 32]);

    let collection_addr = client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Expiry Test 721"),
        &String::from_str(&env, "ET7"),
        &1_000u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let lazy_client = Lazy721Client::new(&env, &collection_addr);

    // Advance ledger past the voucher's valid_until
    env.ledger().with_mut(|li| li.sequence_number = 200);

    let buyer = Address::generate(&env);
    let voucher = MintVoucher {
        token_id: 1,
        price: 0,
        currency: Address::generate(&env),
        uri: String::from_str(&env, "ipfs://expired"),
        uri_hash: BytesN::from_array(&env, &[0u8; 32]),
        valid_until: 50, // expired: 50 < 200
    };

    let sig = BytesN::from_array(&env, &[0u8; 64]);
    let result = lazy_client.try_redeem(&buyer, &voucher, &sig);
    assert_eq!(
        result,
        Err(Ok(LazyError::VoucherExpired)),
        "expired voucher must return VoucherExpired"
    );
}

// ─── Issue #37 — Registry metadata tests ─────────────────────────────────────

/// deploy_normal_721 stores name, symbol, and ledger in the collection record.
#[test]
fn registry_stores_full_metadata_normal_721() {
    let env = Env::default();
    env.ledger().with_mut(|li| {
        li.sequence_number = 42;
    });
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt = BytesN::from_array(&env, &[0xF1u8; 32]);
    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    let addr = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "My Collection"),
        &String::from_str(&env, "MYC"),
        &500u64,
        &0u32,
        &royalty_receiver,
        &0u32, // platform_fee_bps
        &salt,
    );

    let record = client.get_collection(&addr).unwrap();
    assert_eq!(record.name, String::from_str(&env, "My Collection"));
    assert_eq!(record.symbol, String::from_str(&env, "MYC"));
    assert_eq!(record.ledger, 42u32);
    assert_eq!(record.platform_fee_bps, 0u32);
    assert_eq!(record.creator, creator);
}

/// get_collection returns the same record as all_collections for the same address.
#[test]
fn get_collection_matches_all_collections() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt = BytesN::from_array(&env, &[0xF2u8; 32]);
    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    let addr = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Alpha"),
        &String::from_str(&env, "ALP"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let by_addr = client.get_collection(&addr).unwrap();
    let all = client.all_collections();
    let from_all = all.get(0).unwrap();
    assert_eq!(by_addr.address, from_all.address);
    assert_eq!(by_addr.name, from_all.name);
}

/// get_collections with start=0, limit=2 returns first two records in order.
#[test]
fn get_collections_paginated_returns_correct_slice() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    let salt_a = BytesN::from_array(&env, &[0xF3u8; 32]);
    let salt_b = BytesN::from_array(&env, &[0xF4u8; 32]);
    let salt_c = BytesN::from_array(&env, &[0xF5u8; 32]);

    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Alpha"),
        &String::from_str(&env, "ALP"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt_a,
    );
    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Beta"),
        &String::from_str(&env, "BET"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt_b,
    );
    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Gamma"),
        &String::from_str(&env, "GAM"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt_c,
    );

    assert_eq!(client.collection_count(), 3u64);

    let page0 = client.get_collections(&0u64, &2u32);
    assert_eq!(page0.len(), 2);
    assert_eq!(page0.get(0).unwrap().name, String::from_str(&env, "Alpha"));
    assert_eq!(page0.get(1).unwrap().name, String::from_str(&env, "Beta"));

    let page1 = client.get_collections(&2u64, &2u32);
    assert_eq!(page1.len(), 1);
    assert_eq!(page1.get(0).unwrap().name, String::from_str(&env, "Gamma"));
}

/// get_collections beyond range returns empty vec.
#[test]
fn get_collections_out_of_range_returns_empty() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[0xF6u8; 32]);

    client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Only"),
        &String::from_str(&env, "ONL"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &0u32,
        &salt,
    );

    let empty = client.get_collections(&10u64, &5u32);
    assert_eq!(empty.len(), 0);
}

// ─── Issue #38 — Per-collection platform fee tests ────────────────────────────

/// Deploying with platform_fee_bps > MAX_FEE_BPS (2000) is rejected.
#[test]
fn invalid_fee_bps_rejected_at_deploy() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt = BytesN::from_array(&env, &[0xE1u8; 32]);
    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    let result = client.try_deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Fee Test"),
        &String::from_str(&env, "FEE"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &2001u32, // exceeds MAX_FEE_BPS
        &salt,
    );
    assert_eq!(result, Err(Ok(Error::InvalidFeeBps)));
}

/// Deploying with platform_fee_bps = MAX_FEE_BPS (2000) succeeds.
#[test]
fn valid_fee_bps_at_max_boundary_succeeds() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let salt = BytesN::from_array(&env, &[0xE2u8; 32]);
    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);

    let addr = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Max Fee"),
        &String::from_str(&env, "MXF"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &2000u32, // exactly MAX_FEE_BPS
        &salt,
    );

    let record = client.get_collection(&addr).unwrap();
    assert_eq!(record.platform_fee_bps, 2000u32);
}

/// Configured fee is persisted in the collection record for all 4 deploy types.
#[test]
fn fee_stored_in_collection_record_for_all_types() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    let creator_pubkey = BytesN::from_array(&env, &[0xAAu8; 32]);

    let addr_721 = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "N721"),
        &String::from_str(&env, "N721"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &500u32,
        &BytesN::from_array(&env, &[0xE3u8; 32]),
    );
    assert_eq!(client.get_collection(&addr_721).unwrap().platform_fee_bps, 500u32);

    let addr_1155 = client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "N1155"),
        &0u32,
        &royalty_receiver,
        &750u32,
        &BytesN::from_array(&env, &[0xE4u8; 32]),
    );
    assert_eq!(client.get_collection(&addr_1155).unwrap().platform_fee_bps, 750u32);

    let addr_l721 = client.deploy_lazy_721(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "L721"),
        &String::from_str(&env, "L721"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &100u32,
        &BytesN::from_array(&env, &[0xE5u8; 32]),
    );
    assert_eq!(client.get_collection(&addr_l721).unwrap().platform_fee_bps, 100u32);

    let addr_l1155 = client.deploy_lazy_1155(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "L1155"),
        &0u32,
        &royalty_receiver,
        &200u32,
        &BytesN::from_array(&env, &[0xE6u8; 32]),
    );
    assert_eq!(client.get_collection(&addr_l1155).unwrap().platform_fee_bps, 200u32);
}

/// Invalid fee rejected for all 4 deploy function variants.
#[test]
fn invalid_fee_rejected_for_all_deploy_variants() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let currency = Address::generate(&env);
    let royalty_receiver = Address::generate(&env);
    let creator_pubkey = BytesN::from_array(&env, &[0xBBu8; 32]);

    assert_eq!(
        client.try_deploy_normal_1155(
            &creator,
            &currency,
            &String::from_str(&env, "Bad Fee 1155"),
            &0u32,
            &royalty_receiver,
            &9999u32,
            &BytesN::from_array(&env, &[0xF7u8; 32]),
        ),
        Err(Ok(Error::InvalidFeeBps))
    );

    assert_eq!(
        client.try_deploy_lazy_721(
            &creator,
            &currency,
            &creator_pubkey,
            &String::from_str(&env, "Bad Fee L721"),
            &String::from_str(&env, "BFL"),
            &100u64,
            &0u32,
            &royalty_receiver,
            &5000u32,
            &BytesN::from_array(&env, &[0xF8u8; 32]),
        ),
        Err(Ok(Error::InvalidFeeBps))
    );

    assert_eq!(
        client.try_deploy_lazy_1155(
            &creator,
            &currency,
            &creator_pubkey,
            &String::from_str(&env, "Bad Fee L1155"),
            &0u32,
            &royalty_receiver,
            &3000u32,
            &BytesN::from_array(&env, &[0xF9u8; 32]),
        ),
        Err(Ok(Error::InvalidFeeBps))
    );
}
