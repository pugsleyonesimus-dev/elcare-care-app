#!/usr/bin/env bash
# ============================================================
# deploy_launchpad.sh
# Builds, optimises, and deploys the Soroban Launchpad factory
# plus all four NFT implementation contracts to Stellar Testnet.
# Requires: fund_account.sh and deploy_contract.sh run first.
#
# Usage: ./deploy_launchpad.sh [--dry-run] [--help]
#
# Flags:
#   --dry-run   Validate prerequisites and print what would
#               happen, but make no on-chain changes.
#   --help      Show this message and exit.
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ENV_FILE="$SCRIPT_DIR/.env.deploy"
DEPLOYED_IDS="$SCRIPT_DIR/deployed_ids.env"
FRONTEND_ENV="$REPO_ROOT/frontend/elcarehub-app/.env.local"
DRY_RUN=false

usage() {
  grep '^#' "$0" | grep -v '^#!/' | sed 's/^# \{0,1\}//'
}

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: Unknown flag: $arg"; usage; exit 1 ;;
  esac
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ELCARE-HUB — Deploy Launchpad to Testnet"
if $DRY_RUN; then echo "  (DRY RUN — no on-chain changes will be made)"; fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── Check prerequisites ──────────────────────────────────────
for cmd in stellar cargo jq; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "ERROR: '$cmd' is required but not installed."
    exit 1
  fi
done

if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: $ENV_FILE not found. Run ./fund_account.sh first."
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

for var in STELLAR_SECRET STELLAR_PUBLIC RPC_URL NETWORK; do
  if [[ -z "${!var:-}" ]]; then
    echo "ERROR: $var is not set in $ENV_FILE. Run ./fund_account.sh to regenerate."
    exit 1
  fi
done

if $DRY_RUN; then
  echo "DRY RUN: Prerequisites OK."
  echo "DRY RUN: Would build soroban-launchpad + 4 NFT contracts."
  echo "DRY RUN: Would upload WASMs and deploy Launchpad to $NETWORK via $RPC_URL"
  echo "DRY RUN: Would write IDs to:"
  echo "         $DEPLOYED_IDS"
  echo "         $FRONTEND_ENV"
  exit 0
fi

# ── 1. Build everything ───────────────────────────────────────
echo "Step 1/6  Building contracts..."
cd "$REPO_ROOT"
cargo build --target wasm32v1-none --release \
  -p soroban-launchpad \
  -p collection-nft-erc721 \
  -p collection-nft-erc1155 \
  -p lazy-mint-erc721 \
  -p lazy-mint-erc1155

# ── 2. Optimize ───────────────────────────────────────────────
echo "Step 2/6  Optimizing WASM..."
TARGET_DIR="$REPO_ROOT/target/wasm32v1-none/release"
for WASM in soroban_launchpad collection_nft_erc721 collection_nft_erc1155 lazy_mint_erc721 lazy_mint_erc1155; do
  stellar contract optimize --wasm "$TARGET_DIR/$WASM.wasm" --wasm-out "$TARGET_DIR/$WASM.wasm" || true
done

# ── 3. Upload NFT WASMs ───────────────────────────────────────
echo "Step 3/6  Uploading NFT WASMs..."
upload_wasm() {
  stellar contract install \
    --wasm "$TARGET_DIR/$1.wasm" \
    --source "$STELLAR_SECRET" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "Test SDF Network ; September 2015" \
    --ignore-checks 2>&1 | tail -1
}

HASH_N721=$(upload_wasm collection_nft_erc721)
echo "  Normal 721 Hash:   $HASH_N721"
HASH_N1155=$(upload_wasm collection_nft_erc1155)
echo "  Normal 1155 Hash:  $HASH_N1155"
HASH_L721=$(upload_wasm lazy_mint_erc721)
echo "  Lazy 721 Hash:     $HASH_L721"
HASH_L1155=$(upload_wasm lazy_mint_erc1155)
echo "  Lazy 1155 Hash:    $HASH_L1155"

# ── 4. Deploy Launchpad ───────────────────────────────────────
echo "Step 4/6  Deploying Launchpad..."
LAUNCHPAD_ID=$(stellar contract deploy \
  --wasm "$TARGET_DIR/soroban_launchpad.wasm" \
  --source "$STELLAR_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  --ignore-checks 2>&1 | tail -1)
echo "  Launchpad ID: $LAUNCHPAD_ID"

# ── 5. Initialize Launchpad ───────────────────────────────────
echo "Step 5/6  Initializing Launchpad..."
stellar contract invoke \
  --id "$LAUNCHPAD_ID" \
  --source "$STELLAR_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- initialize \
  --admin "$STELLAR_PUBLIC" \
  --platform_fee_receiver "$STELLAR_PUBLIC" \
  --platform_fee_bps 0

stellar contract invoke \
  --id "$LAUNCHPAD_ID" \
  --source "$STELLAR_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- set_wasm_hashes \
  --normal_721 "$HASH_N721" \
  --normal_1155 "$HASH_N1155" \
  --lazy_721 "$HASH_L721" \
  --lazy_1155 "$HASH_L1155"

# ── 6. Write machine-readable deployed IDs ────────────────────
echo "Step 6/6  Writing deployed IDs and updating frontend .env.local..."

update_deployed_id() {
  local key=$1
  local val=$2
  if [[ -f "$DEPLOYED_IDS" ]] && grep -q "^${key}=" "$DEPLOYED_IDS"; then
    sed "s|^${key}=.*|${key}=${val}|" "$DEPLOYED_IDS" > "$DEPLOYED_IDS.tmp"
    mv "$DEPLOYED_IDS.tmp" "$DEPLOYED_IDS"
  else
    echo "${key}=${val}" >> "$DEPLOYED_IDS"
  fi
}

touch "$DEPLOYED_IDS"
update_deployed_id "LAUNCHPAD_CONTRACT_ID" "$LAUNCHPAD_ID"
update_deployed_id "HASH_NORMAL_721" "$HASH_N721"
update_deployed_id "HASH_NORMAL_1155" "$HASH_N1155"
update_deployed_id "HASH_LAZY_721" "$HASH_L721"
update_deployed_id "HASH_LAZY_1155" "$HASH_L1155"
echo "  Deployed IDs written to $DEPLOYED_IDS"

update_env() {
  local key=$1
  local val=$2
  if [[ -f "$FRONTEND_ENV" ]] && grep -q "^${key}=" "$FRONTEND_ENV"; then
    sed "s|^${key}=.*|${key}=${val}|" "$FRONTEND_ENV" > "$FRONTEND_ENV.tmp"
    mv "$FRONTEND_ENV.tmp" "$FRONTEND_ENV"
  else
    echo "${key}=${val}" >> "$FRONTEND_ENV"
  fi
}

mkdir -p "$(dirname "$FRONTEND_ENV")"
update_env "NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID" "$LAUNCHPAD_ID"
echo "  Updated $FRONTEND_ENV"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ✓ Launchpad deployment complete!"
echo ""
echo "  Launchpad ID         : $LAUNCHPAD_ID"
echo "  Machine-readable IDs : $DEPLOYED_IDS"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
