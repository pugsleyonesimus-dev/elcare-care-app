# ElcareHub Threat Model

## Overview

ElcareHub is a non-custodial NFT marketplace on Stellar Soroban. Users hold their own keys
via Freighter or Magic.link wallets. The platform does not hold funds or private keys on behalf
of users. All value-bearing operations go on-chain and are mediated by the smart contract.

The attack surface is divided into three layers: smart contract, indexer, and frontend.

---

## 1. Smart contract layer

### Assets at risk
- Seller proceeds locked in active listings
- Royalty payments owed to creators
- Admin privileges (pause, fee config, token whitelist)

### Threat: Reentrancy
**Description:** A malicious token contract or NFT could call back into the marketplace during a
`buy` or `settle_auction` execution to drain funds or manipulate state.

**Mitigations:**
- Soroban's execution model is single-threaded and sequential; cross-contract calls cannot
  re-enter the calling contract mid-execution in the same transaction.
- All state mutations (clearing the listing, transferring funds) complete before external
  transfers in the settlement logic.

**Residual risk:** Low.

### Threat: Integer overflow / underflow
**Description:** Arithmetic on price, royalty basis points, or fee calculations could overflow,
producing incorrect token amounts.

**Mitigations:**
- `Cargo.toml` sets `overflow-checks = true` in the release profile, causing integer overflows
  to panic rather than wrap.
- Settlement math is covered by property-based tests (7 tests on randomised inputs, Issue-116).

**Residual risk:** Low.

### Threat: Unauthorised admin operations
**Description:** An attacker could invoke `pause`, `set_fee`, or `revoke_artist` without holding
the admin key.

**Mitigations:**
- All privileged functions verify `env.invoker()` against the stored admin address.
- Admin key transfer is a 2-step propose-then-accept flow; a compromised propose does not
  immediately transfer control.

**Residual risk:** Low. Depends on the security of the admin wallet (see runbook for key rotation).

### Threat: Front-running on collection creation (Launchpad)
**Description:** An attacker observing the mempool could submit a collection deployment with the
same parameters before the legitimate creator, stealing the intended contract address or
consuming the creator's salt.

**Mitigations:**
- Collection creation uses caller-address-qualified salts, making the resulting contract ID
  dependent on the creator's identity.

**Residual risk:** Low.

---

## 2. Indexer layer

### Assets at risk
- Database integrity (listing/auction/offer state)
- API correctness (stale or incorrect data served to the frontend)
- Availability of the REST/SSE API

### Threat: Blockchain re-org
**Description:** If a ledger is reorganised (hash changes for a previously accepted ledger),
the indexer's database may contain events from the orphaned branch.

**Mitigations:**
- The poller checks the ledger hash of the most recently indexed ledger on every cycle.
- On hash mismatch, the reconciler rolls back affected rows and re-indexes from the fork point.
- See runbook for manual recovery procedure.

**Residual risk:** Medium — deep re-orgs (> reconciler window) require manual backfill.

### Threat: Trusting RPC provider
**Description:** The indexer relies on a Stellar RPC node. A malicious or compromised RPC could
serve fabricated events, causing the indexer to record false listings or sales.

**Mitigations:**
- Production deployments should use a reputable or self-hosted RPC node.
- Event decoding validates XDR structure and rejects malformed payloads.
- Ledger hash continuity checks catch injected synthetic ledgers.

**Residual risk:** Medium — entirely mitigated only by running a trusted RPC.

### Threat: SQL injection via query parameters
**Description:** Indexer REST endpoints accept filter parameters (artist, search, status). An
attacker could inject SQL to read or mutate the database.

**Mitigations:**
- All database access is via Prisma ORM with parameterised queries.
- Query parameters are validated through Zod schemas before use.

**Residual risk:** Low.

### Threat: Denial of service via rate-limit bypass
**Description:** An attacker could flood the indexer API to exhaust connections or CPU.

**Mitigations:**
- Global rate limiter (`express-rate-limit`) applied before all routes.
- Per-endpoint tighter limits on expensive queries.
- Redis-backed response caching reduces repeated computation.

**Residual risk:** Medium — volumetric DDoS at the network layer is out of scope for app-level mitigations.

---

## 3. Frontend layer

### Threat: Cross-site scripting (XSS)
**Description:** Malicious content in NFT metadata (title, description, image URI) rendered as
raw HTML could inject scripts to steal wallet connections or sign transactions without user consent.

**Mitigations:**
- React's JSX escapes all string interpolation by default.
- NFT metadata is rendered as text nodes, not `dangerouslySetInnerHTML`.
- Content Security Policy headers should be configured in production (Next.js `headers()`).

**Residual risk:** Low for stored XSS; CSP should be added before mainnet.

### Threat: Wallet phishing / UI redress
**Description:** A user visiting a look-alike site could be tricked into signing a malicious
Soroban transaction disguised as a legitimate marketplace operation.

**Mitigations:**
- The app runs on a well-known, consistently branded domain.
- All transaction signing goes through Freighter or Magic.link, which show the raw operation
  details to the user before signing.
- Transaction simulation (`simulateTransaction`) is called before submission; unexpected
  auth entries are surfaced to the user.

**Residual risk:** Medium — dependent on user vigilance and browser extension security.

### Threat: Secret leakage via client bundle
**Description:** Server-side secrets (e.g., `PINATA_JWT`) bundled into the client-side JavaScript
would be exposed to any visitor.

**Mitigations:**
- Only `NEXT_PUBLIC_*` variables are inlined into the client bundle by Next.js.
- `PINATA_JWT` is server-side only and validated at startup only in Node.js runtime
  (see `src/lib/config.ts`).
- The Sentry DSN uses `NEXT_PUBLIC_SENTRY_DSN` — safe to expose; it is a write-only ingest key.

**Residual risk:** Low, provided the env var naming convention is maintained.

---

## Risk summary

| Threat | Likelihood | Impact | Residual Risk |
|--------|-----------|--------|---------------|
| Reentrancy (contract) | Low | High | Low |
| Integer overflow | Low | High | Low |
| Unauthorised admin | Low | High | Low |
| Blockchain re-org | Medium | Medium | Medium |
| Malicious RPC | Low | High | Medium |
| XSS | Low | High | Low |
| Wallet phishing | Medium | High | Medium |
| Secret leakage | Low | High | Low |
| DDoS | Medium | Medium | Medium |

---

## Out of scope

- Stellar protocol-level vulnerabilities
- Operating system / infrastructure compromise
- Supply chain attacks on npm or cargo packages (mitigated by CI dependency scanning)
