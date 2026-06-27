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
        &salt_a,
    );

    let deployed_b = client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "Creator 1155 B"),
        &500u32,
        &royalty_receiver,
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
        &salt_a,
    );

    let deployed_b = client.deploy_lazy_1155(
        &creator,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Lazy 1155 B"),
        &600u32,
        &royalty_receiver,
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
        &salt,
    );

    let addr_bob = client.deploy_normal_1155(
        &bob,
        &currency,
        &String::from_str(&env, "Bob 1155"),
        &500u32,
        &royalty_receiver,
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
        &salt,
    );

    let addr_bob = client.deploy_lazy_1155(
        &bob,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Bob L1155"),
        &400u32,
        &royalty_receiver,
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
        &salt,
    );

    let addr_alice = client.deploy_normal_1155(
        &alice,
        &currency,
        &String::from_str(&env, "Alice 1155"),
        &0u32,
        &royalty_receiver,
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
        &salt,
    );

    let addr_alice = client.deploy_lazy_1155(
        &alice,
        &currency,
        &creator_pubkey,
        &String::from_str(&env, "Alice L1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );

    assert_ne!(addr_alice, addr_bob);
    assert_eq!(client.collection_count(), 2u64);
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
    VoucherAlreadyUsed = 8,
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

// ─── Issue #47 — Name/symbol length validation ───────────────────────────────

#[test]
fn deploy_with_name_too_long_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    // 65-byte name exceeds MAX_NAME_LEN (64)
    let long_name   = String::from_str(&env, "AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDDEEEEEEEEEEFFFFFFFFFFFGGGG");
    let royalty_receiver = Address::generate(&env);
    let currency         = Address::generate(&env);
    let salt             = BytesN::from_array(&env, &[0xD1u8; 32]);

    let result = client.try_deploy_normal_721(
        &creator,
        &currency,
        &long_name,
        &String::from_str(&env, "OK"),
        &100u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );
    assert_eq!(result, Err(Ok(Error::InvalidCollectionMetadata)));
}

#[test]
fn deploy_with_symbol_too_long_fails() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    // 17-byte symbol exceeds MAX_SYMBOL_LEN (16)
    let long_sym         = String::from_str(&env, "TOOLONGSYMBOL1234X");
    let royalty_receiver = Address::generate(&env);
    let currency         = Address::generate(&env);
    let salt             = BytesN::from_array(&env, &[0xD2u8; 32]);

    let result = client.try_deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Valid Name"),
        &long_sym,
        &100u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );
    assert_eq!(result, Err(Ok(Error::InvalidCollectionMetadata)));
}

// ─── Issue #48 — Factory integration tests: deploy each kind and mint ─────────
//
// Each test deploys one collection kind through the factory, mints at least one
// token, and asserts both the registry record and the token ownership.

use soroban_sdk::Bytes;
use soroban_sdk::xdr::ToXdr;

// ── Cross-contract clients for minting ───────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Normal721Error {
    AlreadyInitialized  = 1,
    NotInitialized      = 2,
    NotOwner            = 3,
    NotApproved         = 4,
    TokenNotFound       = 5,
    MaxSupplyReached    = 6,
    NotCreator          = 7,
    InsufficientBalance = 8,
}

#[contractclient(name = "Normal721CollClient")]
pub trait INormal721Coll {
    fn mint(env: Env, to: Address, uri: String) -> Result<u64, Normal721Error>;
    fn balance_of(env: Env, owner: Address) -> u64;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Normal1155Error {
    AlreadyInitialized  = 1,
    NotInitialized      = 2,
    NotApproved         = 3,
    InsufficientBalance = 4,
    LengthMismatch      = 5,
    NotCreator          = 6,
}

#[contractclient(name = "Normal1155CollClient")]
pub trait INormal1155Coll {
    fn mint_new(env: Env, to: Address, amount: u128, uri: String) -> Result<u64, Normal1155Error>;
    fn balance_of(env: Env, account: Address, token_id: u64) -> u128;
}

// Reuse MintVoucher + LazyError already defined above for Lazy721.
#[contractclient(name = "Lazy721CollClient")]
pub trait ILazy721Coll {
    fn redeem(
        env: Env,
        buyer: Address,
        voucher: MintVoucher,
        signature: BytesN<64>,
    ) -> Result<u64, LazyError>;
    fn balance_of(env: Env, owner: Address) -> u64;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Lazy1155Error {
    AlreadyInitialized       = 1,
    NotInitialized           = 2,
    NotApproved              = 3,
    InsufficientBalance      = 4,
    LengthMismatch           = 5,
    VoucherExpired           = 6,
    ExceedsVoucherMax        = 7,
    NotCreator               = 8,
    EditionNotRegistered     = 9,
    EditionAlreadyRegistered = 10,
    InvalidSignature         = 11,
    MaxSupplyReached         = 12,
}

#[contracttype]
#[derive(Clone)]
pub struct MintVoucher1155 {
    pub token_id:       u64,
    pub buyer_quota:    u128,
    pub price_per_unit: i128,
    pub currency:       Address,
    pub uri:            String,
    pub uri_hash:       BytesN<32>,
    pub valid_until:    u64,
}

#[contractclient(name = "Lazy1155CollClient")]
pub trait ILazy1155Coll {
    fn register_edition(env: Env, token_id: u64, max_supply: u128) -> Result<(), Lazy1155Error>;
    fn redeem(
        env: Env,
        buyer: Address,
        voucher: MintVoucher1155,
        amount: u128,
        signature: BytesN<64>,
    ) -> Result<(), Lazy1155Error>;
    fn balance_of(env: Env, account: Address, token_id: u64) -> u128;
}

// ── Digest helpers (mirror contract logic so tests can sign real vouchers) ────

fn lazy721_digest(
    env: &Env,
    coll: &Address,
    token_id: u64,
    price: i128,
    valid_until: u64,
    uri_hash: &BytesN<32>,
    currency: &Address,
) -> BytesN<32> {
    let mut raw = Bytes::new(env);
    raw.append(&coll.to_xdr(env));
    raw.extend_from_array(&token_id.to_be_bytes());
    raw.extend_from_array(&price.to_be_bytes());
    raw.extend_from_array(&valid_until.to_be_bytes());
    let uri_hash_bytes: Bytes = uri_hash.clone().into();
    raw.append(&uri_hash_bytes);
    raw.append(&currency.to_xdr(env));
    env.crypto().sha256(&raw)
}

fn lazy1155_digest(
    env: &Env,
    coll: &Address,
    token_id: u64,
    buyer_quota: u128,
    price_per_unit: i128,
    valid_until: u64,
    uri_hash: &BytesN<32>,
    currency: &Address,
) -> BytesN<32> {
    let mut raw = Bytes::new(env);
    raw.append(&coll.to_xdr(env));
    raw.extend_from_array(&token_id.to_be_bytes());
    raw.extend_from_array(&buyer_quota.to_be_bytes());
    raw.extend_from_array(&price_per_unit.to_be_bytes());
    raw.extend_from_array(&valid_until.to_be_bytes());
    let uri_hash_bytes: Bytes = uri_hash.clone().into();
    raw.append(&uri_hash_bytes);
    raw.append(&currency.to_xdr(env));
    env.crypto().sha256(&raw)
}

// ── Normal-721: deploy via factory + mint one token ──────────────────────────

#[test]
fn integration_normal_721_deploy_and_mint() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let royalty_receiver = Address::generate(&env);
    let currency         = Address::generate(&env);
    let salt             = BytesN::from_array(&env, &[0xC1u8; 32]);

    let coll_addr = client.deploy_normal_721(
        &creator,
        &currency,
        &String::from_str(&env, "Integ721"),
        &String::from_str(&env, "I721"),
        &1_000u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );

    // Registry
    let all = client.all_collections();
    assert_eq!(all.len(), 1);
    assert!(matches!(all.get(0).unwrap().kind, CollectionKind::Normal721));
    assert_eq!(all.get(0).unwrap().address, coll_addr);

    // Mint
    let n721  = Normal721CollClient::new(&env, &coll_addr);
    let buyer = Address::generate(&env);
    let tid   = n721.mint(&buyer, &String::from_str(&env, "ipfs://n721/0"));
    assert_eq!(tid, 0u64);
    assert_eq!(n721.balance_of(&buyer), 1u64);
}

// ── Normal-1155: deploy via factory + mint one token type ────────────────────

#[test]
fn integration_normal_1155_deploy_and_mint() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let royalty_receiver = Address::generate(&env);
    let currency         = Address::generate(&env);
    let salt             = BytesN::from_array(&env, &[0xC2u8; 32]);

    let coll_addr = client.deploy_normal_1155(
        &creator,
        &currency,
        &String::from_str(&env, "Integ1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );

    // Registry
    let all = client.all_collections();
    assert_eq!(all.len(), 1);
    assert!(matches!(all.get(0).unwrap().kind, CollectionKind::Normal1155));
    assert_eq!(all.get(0).unwrap().address, coll_addr);

    // Mint
    let n1155 = Normal1155CollClient::new(&env, &coll_addr);
    let buyer = Address::generate(&env);
    let tid   = n1155.mint_new(&buyer, &10u128, &String::from_str(&env, "ipfs://n1155/0"));
    assert_eq!(tid, 0u64);
    assert_eq!(n1155.balance_of(&buyer, &0u64), 10u128);
}

// ── LazyMint-721: deploy via factory + redeem with real ed25519 signature ────

#[test]
fn integration_lazy_721_deploy_and_mint() {
    use ed25519_dalek::{Signer, SigningKey};

    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let secret_bytes  = [7u8; 32];
    let signing_key   = SigningKey::from_bytes(&secret_bytes);
    let creator_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());

    let royalty_receiver = Address::generate(&env);
    let fee_currency     = Address::generate(&env);
    let salt             = BytesN::from_array(&env, &[0xC3u8; 32]);

    let coll_addr = client.deploy_lazy_721(
        &creator,
        &fee_currency,
        &creator_pubkey,
        &String::from_str(&env, "IntegL721"),
        &String::from_str(&env, "IL7"),
        &1_000u64,
        &0u32,
        &royalty_receiver,
        &salt,
    );

    // Registry
    let all = client.all_collections();
    assert_eq!(all.len(), 1);
    assert!(matches!(all.get(0).unwrap().kind, CollectionKind::LazyMint721));
    assert_eq!(all.get(0).unwrap().address, coll_addr);

    // Build + sign voucher
    let voucher_currency = Address::generate(&env);
    let uri_hash         = BytesN::from_array(&env, &[0u8; 32]);
    let token_id: u64    = 0;
    let price: i128      = 0;
    let valid_until: u64 = 0;

    let digest = lazy721_digest(
        &env, &coll_addr, token_id, price, valid_until, &uri_hash, &voucher_currency,
    );
    let sig      = signing_key.sign(&digest.to_array());
    let sig_bn   = BytesN::<64>::from_array(&env, &sig.to_bytes());

    let voucher = MintVoucher {
        token_id,
        price,
        currency: voucher_currency,
        uri: String::from_str(&env, "ipfs://l721/0"),
        uri_hash,
        valid_until,
    };

    // Redeem
    let l721  = Lazy721CollClient::new(&env, &coll_addr);
    let buyer = Address::generate(&env);
    let tid   = l721.redeem(&buyer, &voucher, &sig_bn);
    assert_eq!(tid, 0u64);
    assert_eq!(l721.balance_of(&buyer), 1u64);
}

// ── LazyMint-1155: deploy via factory + register edition + redeem ─────────────

#[test]
fn integration_lazy_1155_deploy_and_mint() {
    use ed25519_dalek::{Signer, SigningKey};

    let env = Env::default();
    env.ledger().with_mut(|li| li.sequence_number = 1);
    let (client, _admin, _fee_receiver, creator) = setup_launchpad(&env);

    let secret_bytes  = [8u8; 32];
    let signing_key   = SigningKey::from_bytes(&secret_bytes);
    let creator_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());

    let royalty_receiver = Address::generate(&env);
    let fee_currency     = Address::generate(&env);
    let salt             = BytesN::from_array(&env, &[0xC4u8; 32]);

    let coll_addr = client.deploy_lazy_1155(
        &creator,
        &fee_currency,
        &creator_pubkey,
        &String::from_str(&env, "IntegL1155"),
        &0u32,
        &royalty_receiver,
        &salt,
    );

    // Registry
    let all = client.all_collections();
    assert_eq!(all.len(), 1);
    assert!(matches!(all.get(0).unwrap().kind, CollectionKind::LazyMint1155));
    assert_eq!(all.get(0).unwrap().address, coll_addr);

    let l1155      = Lazy1155CollClient::new(&env, &coll_addr);
    let token_id: u64   = 0;
    let edition_max: u128 = 100;

    // Register edition before minting
    l1155.register_edition(&token_id, &edition_max);

    // Build + sign voucher
    let voucher_currency  = Address::generate(&env);
    let uri_hash          = BytesN::from_array(&env, &[0u8; 32]);
    let buyer_quota: u128 = 10;
    let price_per_unit: i128 = 0;
    let valid_until: u64  = 0;
    let amount: u128      = 3;

    let digest = lazy1155_digest(
        &env, &coll_addr, token_id, buyer_quota, price_per_unit, valid_until,
        &uri_hash, &voucher_currency,
    );
    let sig    = signing_key.sign(&digest.to_array());
    let sig_bn = BytesN::<64>::from_array(&env, &sig.to_bytes());

    let voucher = MintVoucher1155 {
        token_id,
        buyer_quota,
        price_per_unit,
        currency: voucher_currency,
        uri: String::from_str(&env, "ipfs://l1155/0"),
        uri_hash,
        valid_until,
    };

    // Redeem
    let buyer = Address::generate(&env);
    l1155.redeem(&buyer, &voucher, &amount, &sig_bn);
    assert_eq!(l1155.balance_of(&buyer, &token_id), amount);
}
