#!/usr/bin/env bash
# ============================================================
# fund_account.sh
# Creates a new Stellar keypair and funds it on Testnet via
# Friendbot. Saves the keys to .env.deploy for use by
# deploy_contract.sh.
#
# Usage: ./fund_account.sh [--dry-run] [--help]
#
# Flags:
#   --dry-run   Print what would happen without making changes.
#   --help      Show this message and exit.
# ============================================================
set -euo pipefail

ENV_FILE="$(dirname "$0")/.env.deploy"
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
echo "  ELCARE-HUB — Fund Testnet Deployer Account"
if $DRY_RUN; then echo "  (DRY RUN — no changes will be made)"; fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── Check prerequisites ──────────────────────────────────────
for cmd in stellar curl jq; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "ERROR: '$cmd' is required but not installed."
    echo "  Install the Stellar CLI: https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli"
    exit 1
  fi
done

# ── Idempotency: skip if account already funded ───────────────
if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  if [[ -n "${STELLAR_PUBLIC:-}" ]]; then
    echo "Checking if $STELLAR_PUBLIC is already funded on Testnet..."
    HTTP_STATUS=$(curl -s -o /dev/null -w "%{http_code}" \
      "https://horizon-testnet.stellar.org/accounts/${STELLAR_PUBLIC}")
    if [[ "$HTTP_STATUS" == "200" ]]; then
      echo "  Account already exists and is funded — nothing to do."
      echo "  Delete $ENV_FILE to force regeneration."
      exit 0
    fi
  fi
fi

if $DRY_RUN; then
  echo "DRY RUN: Would generate a new keypair via 'stellar keys generate'."
  echo "DRY RUN: Would fund it via https://friendbot.stellar.org"
  echo "DRY RUN: Would write credentials to $ENV_FILE"
  exit 0
fi

# ── Generate keypair ──────────────────────────────────────────
echo "Generating new keypair..."
stellar keys generate ELCARE-HUB-deployer --fund --network testnet --overwrite >/dev/null 2>&1 || true

STELLAR_SECRET=$(stellar keys secret ELCARE-HUB-deployer 2>/dev/null || true)
STELLAR_PUBLIC=$(stellar keys public-key ELCARE-HUB-deployer 2>/dev/null || true)

if [[ -z "$STELLAR_SECRET" || -z "$STELLAR_PUBLIC" ]]; then
  echo "ERROR: Failed to generate keypair. Is the Stellar CLI installed and in your PATH?"
  exit 1
fi

echo "Public Key : $STELLAR_PUBLIC"
echo "Secret Key : (written to $ENV_FILE — keep this safe!)"

# ── Fund via Friendbot ────────────────────────────────────────
echo "Funding account via Friendbot..."
RESPONSE=$(curl -s "https://friendbot.stellar.org?addr=${STELLAR_PUBLIC}")
STATUS=$(echo "$RESPONSE" | jq -r '.successful // "false"')

if [[ "$STATUS" != "true" ]]; then
  ERR_DETAIL=$(echo "$RESPONSE" | jq -r '.detail // .title // "unknown"' 2>/dev/null || echo "unknown")
  echo "  WARNING: Friendbot response: $ERR_DETAIL"
  echo "  If the account already exists this is expected — continuing."
fi

# ── Save to .env.deploy ───────────────────────────────────────
cat > "$ENV_FILE" <<EOF
# ELCARE-HUB Deploy — Testnet
# Generated $(date -u +"%Y-%m-%dT%H:%M:%SZ")
# WARNING: Never commit this file to version control.
STELLAR_SECRET=$STELLAR_SECRET
STELLAR_PUBLIC=$STELLAR_PUBLIC
NETWORK=testnet
RPC_URL=https://soroban-testnet.stellar.org
HORIZON_URL=https://horizon-testnet.stellar.org
EOF

echo ""
echo "✓ Account funded. Credentials saved to $ENV_FILE"
echo "  Run ./deploy_contract.sh next."
