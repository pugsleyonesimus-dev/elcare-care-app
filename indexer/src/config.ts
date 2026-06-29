function parsePositiveInt(name: string, raw: string | undefined, defaultVal: number): number {
  if (raw === undefined || raw === '') return defaultVal;
  const n = Number(raw);
  if (!Number.isInteger(n) || n <= 0) {
    throw new Error(`Config error: ${name} must be a positive integer, got "${raw}"`);
  }
  return n;
}

const REQUIRED_ENV_VARS = [
  'DATABASE_URL',
  'MARKETPLACE_CONTRACT_ID',
  'REDIS_URL',
  'STELLAR_RPC_URL',
  'STELLAR_NETWORK',
] as const;

/**
 * Validates that all required environment variables are present.
 * Throws a single aggregated error listing every missing variable so the
 * operator can fix all problems in one restart rather than discovering them
 * one-by-one.
 */
export function validateRequiredEnv(): void {
  const missing = REQUIRED_ENV_VARS.filter((name) => !process.env[name]);
  if (missing.length > 0) {
    throw new Error(
      `[indexer] Missing required environment variables: ${missing.join(', ')}.\n` +
        'Check indexer/.env and ensure all required vars are set (see README for the full table).'
    );
  }
}

export function loadConfig() {
  return {
    pollIntervalMs: parsePositiveInt('POLL_INTERVAL_MS', process.env.POLL_INTERVAL_MS, 5000),
    maxLedgersPerCycle: parsePositiveInt('MAX_LEDGERS_PER_CYCLE', process.env.MAX_LEDGERS_PER_CYCLE, 1000),
  };
}

export type Config = ReturnType<typeof loadConfig>;
