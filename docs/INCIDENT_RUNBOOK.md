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
