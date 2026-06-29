// ─────────────────────────────────────────────────────────────
// lib/config.ts — centralised runtime configuration
// ─────────────────────────────────────────────────────────────

export const config = {
  contractId: process.env.NEXT_PUBLIC_CONTRACT_ID ?? "",
  launchpadContractId: process.env.NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID ?? "",
  /** Base URL for the ELCARE-HUB indexer HTTP API (no trailing slash). */
  indexerUrl: (process.env.NEXT_PUBLIC_INDEXER_URL ?? "http://localhost:4000").replace(
    /\/$/,
    ""
  ),
  /** Base URL for the application (no trailing slash). */
  baseUrl: (process.env.NEXT_PUBLIC_BASE_URL ?? "http://localhost:3000").replace(
    /\/$/,
    ""
  ),
  network: process.env.NEXT_PUBLIC_STELLAR_NETWORK ?? "testnet",
  rpcUrl:
    process.env.NEXT_PUBLIC_STELLAR_RPC_URL ??
    "https://soroban-testnet.stellar.org",
  horizonUrl:
    process.env.NEXT_PUBLIC_STELLAR_HORIZON_URL ??
    "https://horizon-testnet.stellar.org",
  networkPassphrase:
    process.env.NEXT_PUBLIC_STELLAR_NETWORK_PASSPHRASE ??
    "Test SDF Network ; September 2015",
  pinataGateway:
    process.env.NEXT_PUBLIC_PINATA_GATEWAY ?? "https://gateway.pinata.cloud",
  isDevelopment: process.env.NODE_ENV === "development",
  /** True when targeting Stellar mainnet — gates mainnet-only guards in the UI. */
  isMainnet: (process.env.NEXT_PUBLIC_STELLAR_NETWORK ?? "testnet") === "mainnet",
} as const;

// Required on both client and server.
const PUBLIC_REQUIRED = [
  "NEXT_PUBLIC_CONTRACT_ID",
  "NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID",
] as const;

// Required server-side only — never validated on the client to avoid
// accidentally surfacing secrets that are not part of the client bundle.
const SERVER_REQUIRED = ["PINATA_JWT"] as const;

export function assertConfig(): void {
  const missing: string[] = [];

  for (const name of PUBLIC_REQUIRED) {
    if (!process.env[name]) missing.push(name);
  }

  // typeof window === "undefined" is true in Node.js (server / build) only.
  if (typeof window === "undefined") {
    for (const name of SERVER_REQUIRED) {
      if (!process.env[name]) missing.push(name);
    }
  }

  if (missing.length > 0) {
    throw new Error(
      `[ELCARE-HUB] Missing required environment variables: ${missing.join(", ")}.\n` +
        "Copy .env.example to .env.local and fill in the required values."
    );
  }
}

// Run at module load so missing vars surface in server logs on boot,
// not silently at the moment a user first tries to interact with the contract.
// Skipped in test environments to avoid breaking unit tests that don't set every var.
if (process.env.NODE_ENV !== "test") {
  assertConfig();
}
