# Incident Runbook

Procedures for responding to security incidents and operational failures on ElcareHub.

For the disclosure process and contacts, see [SECURITY.md](../SECURITY.md).  
For the threat surface these procedures address, see [THREAT_MODEL.md](THREAT_MODEL.md).

---

## Table of Contents

1. [Emergency contract pause](#1-emergency-contract-pause)
2. [Admin key rotation](#2-admin-key-rotation)
3. [Indexer recovery from re-org](#3-indexer-recovery-from-re-org)
4. [Compromised secret rotation](#4-compromised-secret-rotation)
5. [Keeper subsystem operations](#5-keeper-subsystem-operations)

---

## 1. Emergency contract pause

Use when: active exploit is detected on-chain, critical bug found in contract logic, or at
the direction of the security contact.

**Who can execute:** The current admin wallet (the key stored in `ADMIN_SECRET`).

### Steps

```bash
# 1. Confirm you hold the admin key
stellar keys public-key ELCARE-HUB-admin

# 2. Confirm the admin address matches what the contract has stored
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- get_admin

# 3. Pause the contract
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- pause

# 4. Verify paused state — all buy/bid/settle calls should now fail
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- is_paused
```

**Expected result:** `is_paused` returns `true`. Any user-facing transaction that goes through
the contract will now revert with a `Paused` error.

### Unpause procedure

Once the issue is resolved and a patched contract version is deployed (or after the investigation
concludes with no action required):

```bash
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- unpause
```

Post-unpause: monitor the indexer `/metrics` endpoint for anomalous event rates, and confirm
the frontend shows active listings correctly.

---

## 2. Admin key rotation

Use when: admin private key is suspected compromised, admin account is lost, or during
scheduled key rotation.

The marketplace uses a **2-step admin transfer** (`propose_admin` → `accept_admin`) to prevent
a single compromised propose from transferring control.

### Steps

```bash
# Step 1 — Current admin proposes the new admin address
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --source "$CURRENT_ADMIN_SECRET" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- propose_admin \
  --new_admin "$NEW_ADMIN_PUBLIC"

# Step 2 — New admin accepts (signs with the NEW key)
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --source "$NEW_ADMIN_SECRET" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- accept_admin

# Step 3 — Confirm the transfer
stellar contract invoke \
  --id "$MARKETPLACE_CONTRACT_ID" \
  --rpc-url "$STELLAR_RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  -- get_admin
# Should return NEW_ADMIN_PUBLIC
```

### If the current admin key is already compromised

If the attacker has not yet called `accept_admin`, the pending proposal can be cancelled by
calling `propose_admin` again from the current admin with a safe replacement address before the
attacker can accept. If the current admin key cannot sign (key lost), there is no on-chain
recovery — this is a consequence of the non-custodial design. Coordinate with the Stellar RPC
provider and affected users; consider deploying a new contract.

---

## 3. Indexer recovery from re-org

Use when: the indexer reports a stalled or inconsistent state, `/readyz` returns `stalled`,
or the Prometheus gauge `sync_last_ledger` stops advancing.

### Diagnosis

```bash
# Check current sync state
curl http://localhost:4000/readyz

# Check Prometheus metrics for stall/reorg signals
curl http://localhost:4000/metrics | grep -E 'sync_|reorg_'

# Check indexer logs for re-org or hash-mismatch messages
docker compose logs --tail=200 indexer | grep -i 'reorg\|mismatch\|rollback'
```

### Automatic recovery

The indexer detects ledger hash mismatches and triggers an automatic rollback of the affected
rows, then re-indexes from the fork point. This handles shallow re-orgs (< a few ledgers)
without intervention.

### Manual backfill after deep re-org

```bash
# 1. Stop the indexer to avoid concurrent writes
docker compose stop indexer

# 2. Identify the last good ledger from Horizon
LAST_GOOD_LEDGER=<ledger number before the fork>

# 3. Roll back database to that point using Prisma migration + manual SQL if needed
# (Delete events and sync state with lastLedger > LAST_GOOD_LEDGER)
psql "$DATABASE_URL" -c "
  DELETE FROM MarketplaceEvent WHERE ledger > $LAST_GOOD_LEDGER;
  UPDATE SyncState SET lastLedger = $LAST_GOOD_LEDGER, lastHash = '<hash>'
  WHERE id = 1;
"

# 4. Run the backfill CLI to replay missed ledgers
cd indexer
npx tsx src/backfill.ts --from $LAST_GOOD_LEDGER --to <current tip>

# 5. Restart the indexer
docker compose start indexer

# 6. Verify /readyz returns ready
curl http://localhost:4000/readyz
```

### Indexer data integrity check

After recovery, spot-check that key tables are consistent with on-chain state by querying
contract events directly via the Stellar RPC and comparing against the indexer's REST responses.

---

## 4. Compromised secret rotation

### PINATA_JWT (Pinata IPFS upload key)

1. Log in to [Pinata](https://app.pinata.cloud) → API Keys → revoke the compromised key.
2. Generate a new JWT.
3. Update the `PINATA_JWT` secret in your deployment environment (Railway, Vercel, Kubernetes secret, etc.).
4. Redeploy the frontend.
5. Verify new uploads succeed via the listing creation flow.

### STELLAR_SECRET (deployer / admin key)

See [Admin key rotation](#2-admin-key-rotation) for the on-chain transfer procedure.

For the deployer key (used only at deploy time):
1. Generate a new keypair: `./scripts/deploy/fund_account.sh`
2. Transfer any remaining XLM from the old account using `stellar payment`.
3. Revoke the old key from any stored secrets / CI variables.

### SENTRY_DSN / SENTRY_AUTH_TOKEN

1. Log in to [Sentry](https://sentry.io) → Settings → Auth Tokens → revoke the compromised token.
2. Issue a new DSN / token.
3. Update secrets in your CI/CD environment and redeploy.

### Database credentials (DATABASE_URL)

1. Rotate the PostgreSQL password via your database provider.
2. Update `DATABASE_URL` in all deployment environments.
3. Restart the indexer: `docker compose restart indexer`.
4. Confirm `/health` and `/readyz` return healthy responses.

### Redis credentials (REDIS_URL)

1. Rotate the Redis password or ACL entry.
2. Update `REDIS_URL` in all deployment environments.
3. Restart the indexer.

---

## Post-incident checklist

- [ ] Root cause identified and documented
- [ ] Affected users notified (if personal data or funds at risk)
- [ ] Fix deployed and verified in production
- [ ] Secrets rotated where applicable
- [ ] Public disclosure prepared (coordinated with reporter, if external)
- [ ] Runbook updated with any new learnings

---

## 5. Keeper subsystem operations

The keeper is a background process that calls three permissionless maintenance entry-points on
the marketplace contract on behalf of the platform:

| Entry point        | When called                                              | Auth required |
|--------------------|----------------------------------------------------------|---------------|
| `expire_listing`   | Listing `expires_at` ≤ current ledger timestamp          | None          |
| `finalize_auction` | Auction `end_time` ≤ current ledger timestamp            | `caller.require_auth()` (keeper account) |
| `reclaim_offer`    | Offer `expires_at` ≤ current ledger timestamp            | None          |

### 5.1 Key provisioning

Generate a dedicated Stellar keypair for the keeper. **Never reuse the admin key.**

```bash
# Generate a new keypair
stellar keys generate keeper-account --network testnet

# Show the public key (fund this address)
stellar keys public-key keeper-account

# Export the secret key (store in your secrets manager, not in .env files in git)
stellar keys secret-key keeper-account
```

Fund the keeper account with enough XLM to cover the configured daily budget plus a safety
margin.  At the default settings (max 1 XLM/day), a balance of 10–20 XLM gives comfortable
runway.  The account requires a minimum reserve of 1 XLM.

```bash
# On testnet, use Friendbot
curl "https://friendbot.stellar.org?addr=$(stellar keys public-key keeper-account)"

# On mainnet, send XLM from your operations wallet
stellar payment --source ops-account --destination <KEEPER_PUBLIC_KEY> --amount 20
```

### 5.2 Configuration

Set the following environment variables in your deployment (Railway / Kubernetes secret /
`.env`).  See `indexer/.env.example` for the full list with defaults.

| Variable | Required | Description |
|---|---|---|
| `KEEPER_ENABLED` | Yes (to activate) | Set `true` to start the keeper loop |
| `KEEPER_SECRET` | Yes | Stellar secret key (`S...`) for the keeper account |
| `KEEPER_DRY_RUN` | No (default `true`) | Set `false` to broadcast real transactions |
| `KEEPER_INTERVAL_MS` | No (60000) | Sweep interval in milliseconds |
| `KEEPER_MAX_ACTIONS_PER_CYCLE` | No (20) | Cap per cycle to bound fee exposure |
| `KEEPER_MAX_FEE_STROOPS` | No (1000000) | Per-tx fee hard cap (~0.1 XLM) |
| `KEEPER_DAILY_FEE_BUDGET_STROOPS` | No (10000000) | Daily halt budget (~1 XLM) |
| `KEEPER_FEE_BUMP_MULTIPLIER` | No (1.5) | Fee escalation factor per bump |
| `KEEPER_FEE_BUMP_MAX_RETRIES` | No (3) | Max fee-bump attempts before marking Failed |
| `KEEPER_POLL_TIMEOUT_MS` | No (60000) | Timeout before triggering a fee-bump |

**Always start with `KEEPER_DRY_RUN=true`** and confirm the keeper is discovering the right
candidates (check `/keeper/status` and the Prometheus metrics) before switching to live mode.

### 5.3 Enabling the keeper

**Embedded mode** (runs inside the main indexer process):

```bash
# .env
KEEPER_ENABLED=true
KEEPER_SECRET=<secret>
KEEPER_DRY_RUN=false

# Restart the indexer
docker compose restart indexer
```

**Standalone mode** (one-shot, useful for cron or Lambda):

```bash
KEEPER_ENABLED=true KEEPER_SECRET=<secret> KEEPER_DRY_RUN=false \
  npx tsx indexer/src/keeper/index.ts
```

The standalone entrypoint exits with code 0 if all actions succeeded or were skipped, and
code 1 if any actions failed.

### 5.4 Monitoring and alerting

**Health check:**

```bash
curl http://localhost:4000/keeper/status
```

Response fields:

```jsonc
{
  "running": true,          // keeper loop is active
  "dryRun": false,          // live mode
  "enabled": true,
  "actionCounts": {         // cumulative DB counts by status
    "Pending": 0,
    "Submitted": 2,
    "Succeeded": 41,
    "Failed": 1,
    "Skipped": 3
  },
  "lastCycle": {
    "startedAt": "...",
    "completedAt": "...",
    "candidatesDiscovered": 5,
    "actionsAttempted": 5,
    "actionsSucceeded": 4,
    "actionsFailed": 1,
    "actionsSkipped": 0,
    "feesSpentStroops": "4200",
    "budgetExhausted": false,
    "dryRun": false
  },
  "recentActions": [ ... ]
}
```

**Prometheus metrics** (scraped at `/metrics`):

| Metric | Alert threshold |
|---|---|
| `keeper_actions_total{outcome="failed"}` | Rate > 0 over 5 min |
| `keeper_budget_exhausted` | Gauge == 1 |
| `keeper_simulation_failures_total` | Rate > 5/min |
| `keeper_fee_bumps_total` | Rate > 2/min (fee pressure) |
| `keeper_cycle_duration_seconds` | p95 > 60s |

### 5.5 Failure triage

**Scenario: Actions stuck in `Failed` status**

```bash
# Inspect the lastError field for recent failures
curl http://localhost:4000/keeper/status | jq '.recentActions[] | select(.status=="Failed")'

# Or query the DB directly
psql "$DATABASE_URL" -c "
  SELECT id, \"targetType\", \"targetId\", attempts, \"lastError\", \"updatedAt\"
  FROM \"KeeperAction\"
  WHERE status = 'Failed'
  ORDER BY \"updatedAt\" DESC LIMIT 20;
"
```

Common causes and remedies:

| `lastError` pattern | Cause | Fix |
|---|---|---|
| `tx_bad_seq` | Sequence collision (concurrent keeper instance or restart race) | Ensure only one keeper instance runs; actions will auto-retry next cycle |
| `insufficient resource fee` | Soroban resource cost increased | Raise `KEEPER_MAX_FEE_STROOPS` |
| `ECONNREFUSED` / `timeout` | RPC node unreliable | Check `STELLAR_RPC_URL`; switch to a backup node |
| `fee-bump cap reached` | Persistent network congestion | Raise `KEEPER_FEE_BUMP_MAX_RETRIES` and `KEEPER_MAX_FEE_STROOPS` |

To allow Failed actions to retry, reset them to Pending:

```sql
-- Reset all Failed actions to Pending (they will be retried next cycle)
UPDATE "KeeperAction" SET status = 'Pending', "txHash" = NULL, "lastError" = NULL
WHERE status = 'Failed';
```

**Scenario: Daily fee budget exhausted (`keeper_budget_exhausted = 1`)**

The keeper halts automatically when the daily budget is spent.  Investigate before raising the
budget — a sudden spike in fees usually signals network congestion or a configuration error.

```bash
# Check how much was spent today
psql "$DATABASE_URL" -c "
  SELECT SUM(\"feePaid\") AS total_stroops
  FROM \"KeeperAction\"
  WHERE status = 'Succeeded'
    AND \"updatedAt\" >= CURRENT_DATE;
"
```

To restore operation today (after verifying the root cause):

```bash
# Raise the daily budget temporarily
KEEPER_DAILY_FEE_BUDGET_STROOPS=50000000 docker compose restart indexer
```

The in-process budget counter resets on restart, so restarting the indexer effectively resets
the daily budget.  Use this only after confirming fees are within expected bounds.

**Scenario: Actions stuck in `Submitted` status after a crash**

On the next cycle the keeper automatically polls `getTransaction` for any `Submitted` row and
advances it to `Succeeded` or `Failed`.  No manual intervention is needed unless the tx has
been in `Submitted` for more than ~10 minutes (indicating the tx was dropped by the network).

If a tx was dropped:

```sql
-- Force the action back to Pending so it is resubmitted
UPDATE "KeeperAction" SET status = 'Pending', "txHash" = NULL
WHERE status = 'Submitted' AND "updatedAt" < NOW() - INTERVAL '15 minutes';
```

**Scenario: `Skipped` actions you expected to succeed**

A `Skipped` status means the contract returned a permanent error (e.g. `ListingNotExpired`)
at simulation time — the keeper's assumption was wrong.  This is not a keeper bug; it means the
on-chain state changed between discovery and execution (e.g. the listing was cancelled by the
owner before the keeper could expire it).  `Skipped` rows are terminal and are never re-queued.

If a `Skipped` row is wrong (e.g. the listing really is expired), check:
1. That `KEEPER_DRY_RUN=false` — in dry-run mode actions are counted as Succeeded without a DB transition to Submitted.
2. That the keeper account's clock matches the network — ledger timestamps, not wall clock, govern expiry.

### 5.6 KEEPER_SECRET rotation

Rotate the keeper key if it is suspected compromised:

```bash
# 1. Generate replacement keypair and fund it
stellar keys generate keeper-account-v2 --network testnet
curl "https://friendbot.stellar.org?addr=$(stellar keys public-key keeper-account-v2)"

# 2. Update KEEPER_SECRET in your secrets manager to the new secret key

# 3. Restart the indexer / keeper process
docker compose restart indexer

# 4. Verify the new key is being used
curl http://localhost:4000/keeper/status
```

The old key does not need to be explicitly revoked on-chain because the keeper entry-points
(`expire_listing`, `reclaim_offer`) are permissionless, and `finalize_auction` only requires
`caller.require_auth()` — any funded keypair can call it.  Simply stop using the old key.

Transfer any remaining XLM from the old account to the new one:

```bash
stellar payment \
  --source keeper-account-old \
  --destination $(stellar keys public-key keeper-account-v2) \
  --amount <remaining_balance_minus_reserve>
```
