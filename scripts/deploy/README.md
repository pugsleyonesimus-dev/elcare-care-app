# Deploy Scripts

Shell scripts for deploying ElcareHub Soroban contracts to Stellar Testnet.

## Prerequisites

| Tool | Install |
|------|---------|
| [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli) | `cargo install --locked stellar-cli --features opt` |
| Rust + `wasm32v1-none` target | `rustup target add wasm32v1-none` |
| `jq` | `apt install jq` / `brew install jq` |
| `curl` | pre-installed on most systems |

## Usage

Run the scripts in order:

```bash
cd scripts/deploy

# 1. Generate and fund a new testnet deployer keypair.
./fund_account.sh

# 2. Build and deploy the marketplace contract.
./deploy_contract.sh

# 3. Build and deploy the launchpad + NFT contracts.
./deploy_launchpad.sh
```

All three scripts support `--dry-run` to validate prerequisites and print what
would happen without touching the network or writing files:

```bash
./fund_account.sh    --dry-run
./deploy_contract.sh --dry-run
./deploy_launchpad.sh --dry-run
```

Pass `--help` to any script for its full usage text.

## Output files

| File | Description |
|------|-------------|
| `.env.deploy` | Deployer keypair and network config (never commit!) |
| `deployed_ids.env` | Machine-readable contract IDs — source this to configure the indexer or CI |
| `../../frontend/elcarehub-app/.env.local` | Frontend env — updated automatically with `NEXT_PUBLIC_CONTRACT_ID` and `NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID` |

### Using `deployed_ids.env` downstream

```bash
source scripts/deploy/deployed_ids.env

# Wire up the indexer:
echo "MARKETPLACE_CONTRACT_ID=$MARKETPLACE_CONTRACT_ID" >> indexer/.env
echo "LAUNCHPAD_CONTRACT_ID=$LAUNCHPAD_CONTRACT_ID"     >> indexer/.env
```

## Idempotency

| Script | Behaviour on re-run |
|--------|---------------------|
| `fund_account.sh` | Skips if the saved account is already funded on Horizon. Delete `.env.deploy` to force regeneration. |
| `deploy_contract.sh` | Always deploys a fresh instance (contracts are immutable once deployed). |
| `deploy_launchpad.sh` | Always deploys a fresh instance. |

## Security notes

- `.env.deploy` contains a **plaintext secret key** — it is listed in `.gitignore` and must never be committed.
- The deployer account should be ephemeral (testnet only). For mainnet, use a hardware wallet or a secrets manager.
