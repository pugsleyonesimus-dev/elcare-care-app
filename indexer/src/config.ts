import { z } from 'zod';

// ── Generic helpers ──────────────────────────────────────────────────────────

function parsePositiveInt(name: string, raw: string | undefined, defaultVal: number): number {
  if (raw === undefined || raw === '') return defaultVal;
  const n = Number(raw);
  if (!Number.isInteger(n) || n <= 0) {
    throw new Error(`Config error: ${name} must be a positive integer, got "${raw}"`);
  }
  return n;
}

// ── TrackedContract definition ───────────────────────────────────────────────

export interface TrackedContractConfig {
  id: string;
  type: 'marketplace' | 'launchpad';
  label: string;
  startLedger: number;
}

const trackedContractSchema = z.object({
  id: z.string().min(1),
  type: z.enum(['marketplace', 'launchpad']),
  label: z.string().default(''),
  startLedger: z.number().int().min(0).default(0),
});

/**
 * Parses the TRACKED_CONTRACTS environment variable.
 *
 * TRACKED_CONTRACTS should be a JSON array:
 *   [{"id":"C...","type":"marketplace","label":"mainnet","startLedger":1000000}]
 *
 * Falls back to the legacy single-contract MARKETPLACE_CONTRACT_ID /
 * LAUNCHPAD_CONTRACT_ID variables so existing deployments keep working
 * without any config changes.
 */
export function parseTrackedContracts(): TrackedContractConfig[] {
  const raw = process.env.TRACKED_CONTRACTS;

  if (raw && raw.trim() !== '') {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new Error(
        '[indexer] TRACKED_CONTRACTS is not valid JSON. ' +
          'Expected a JSON array: [{"id":"C...","type":"marketplace","label":"...","startLedger":0}]'
      );
    }

    if (!Array.isArray(parsed)) {
      throw new Error('[indexer] TRACKED_CONTRACTS must be a JSON array.');
    }

    const contracts: TrackedContractConfig[] = [];
    for (const [i, item] of parsed.entries()) {
      const result = trackedContractSchema.safeParse(item);
      if (!result.success) {
        const msgs = result.error.errors.map((e) => `${e.path.join('.')}: ${e.message}`).join('; ');
        throw new Error(`[indexer] TRACKED_CONTRACTS[${i}] is invalid: ${msgs}`);
      }
      contracts.push(result.data as TrackedContractConfig);
    }

    if (contracts.length === 0) {
      throw new Error('[indexer] TRACKED_CONTRACTS must contain at least one entry.');
    }

    return contracts;
  }

  // ── Legacy fallback ────────────────────────────────────────────────────────
  const contracts: TrackedContractConfig[] = [];
  if (process.env.MARKETPLACE_CONTRACT_ID) {
    contracts.push({
      id: process.env.MARKETPLACE_CONTRACT_ID,
      type: 'marketplace',
      label: 'marketplace',
      startLedger: 0,
    });
  }
  if (process.env.LAUNCHPAD_CONTRACT_ID) {
    contracts.push({
      id: process.env.LAUNCHPAD_CONTRACT_ID,
      type: 'launchpad',
      label: 'launchpad',
      startLedger: 0,
    });
  }
  return contracts;
}

// ── Required env-var list (non-keeper) ──────────────────────────────────────

const REQUIRED_ENV_VARS = [
  'DATABASE_URL',
  'REDIS_URL',
  'STELLAR_RPC_URL',
  'STELLAR_NETWORK',
] as const;

// At least one of TRACKED_CONTRACTS or MARKETPLACE_CONTRACT_ID must be set.
const CONTRACT_ENV_VARS = ['TRACKED_CONTRACTS', 'MARKETPLACE_CONTRACT_ID'] as const;

/**
 * Validates that all required environment variables are present.
 * Throws a single aggregated error listing every missing variable so the
 * operator can fix all problems in one restart rather than discovering them
 * one-by-one.
 */
export function validateRequiredEnv(): void {
  const missing = REQUIRED_ENV_VARS.filter((name) => !process.env[name]);

  // Must have either TRACKED_CONTRACTS or legacy MARKETPLACE_CONTRACT_ID
  const hasContractConfig = CONTRACT_ENV_VARS.some((name) => process.env[name]);
  if (!hasContractConfig) {
    missing.push('MARKETPLACE_CONTRACT_ID (or TRACKED_CONTRACTS)' as any);
  }

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

// ── Keeper configuration ─────────────────────────────────────────────────────
//
// All keeper env vars are optional at process start so that the main indexer
// can boot without them.  loadKeeperConfig() throws at keeper-startup time if
// KEEPER_ENABLED=true but required vars are missing.

/** Zod schema for the raw env vars consumed by the keeper. */
const keeperEnvSchema = z.object({
  // Whether the keeper loop should run at all (default: false → dry-run safe).
  KEEPER_ENABLED: z
    .string()
    .optional()
    .transform((v) => v === 'true'),

  // Stellar secret key for the keeper account (required when KEEPER_ENABLED=true).
  KEEPER_SECRET: z
    .string()
    .optional()
    .refine((v) => v === undefined || v.startsWith('S'), {
      message: 'KEEPER_SECRET must be a Stellar secret key starting with "S"',
    }),

  // Whether to simulate only and never broadcast (default: true — safe default).
  KEEPER_DRY_RUN: z
    .string()
    .optional()
    .transform((v) => v !== 'false'),   // anything other than explicit "false" = dry-run

  // How often to run the keeper sweep cycle (milliseconds).
  KEEPER_INTERVAL_MS: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 60_000))
    .pipe(z.number().int().positive()),

  // Maximum number of actions the keeper will attempt in a single cycle.
  KEEPER_MAX_ACTIONS_PER_CYCLE: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 20))
    .pipe(z.number().int().positive()),

  // Maximum fee in stroops allowed for a single transaction.
  KEEPER_MAX_FEE_STROOPS: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 1_000_000))   // ~0.1 XLM
    .pipe(z.number().int().positive()),

  // Daily fee budget in stroops; keeper halts cycle when exhausted.
  KEEPER_DAILY_FEE_BUDGET_STROOPS: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 10_000_000))  // ~1 XLM / day
    .pipe(z.number().int().positive()),

  // Fee-bump multiplier applied on each escalation step (e.g. 1.5 = +50%).
  KEEPER_FEE_BUMP_MULTIPLIER: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 1.5))
    .pipe(z.number().min(1.01).max(10)),

  // Maximum number of fee-bump retries before marking action Failed.
  KEEPER_FEE_BUMP_MAX_RETRIES: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 3))
    .pipe(z.number().int().min(0).max(10)),

  // How long to wait for a submitted tx to appear before triggering a fee-bump (ms).
  KEEPER_SUBMIT_TIMEOUT_MS: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 30_000))
    .pipe(z.number().int().positive()),

  // How long to poll getTransaction after submit before giving up (ms).
  KEEPER_POLL_TIMEOUT_MS: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 60_000))
    .pipe(z.number().int().positive()),

  // Interval between getTransaction polls (ms).
  KEEPER_POLL_INTERVAL_MS: z
    .string()
    .optional()
    .transform((v) => (v ? Number(v) : 2_000))
    .pipe(z.number().int().positive()),
});

export type KeeperConfig = z.infer<typeof keeperEnvSchema>;

/**
 * Parse and validate all keeper-related environment variables.
 *
 * Throws a descriptive ZodError if any value fails validation.
 * Also throws if KEEPER_ENABLED=true but KEEPER_SECRET is missing.
 */
export function loadKeeperConfig(): KeeperConfig {
  const result = keeperEnvSchema.safeParse(process.env);
  if (!result.success) {
    const messages = result.error.errors.map((e) => `  ${e.path.join('.')}: ${e.message}`).join('\n');
    throw new Error(`[keeper] Invalid configuration:\n${messages}`);
  }

  const cfg = result.data;

  if (cfg.KEEPER_ENABLED && !cfg.KEEPER_SECRET) {
    throw new Error(
      '[keeper] KEEPER_ENABLED=true requires KEEPER_SECRET to be set.\n' +
        'Generate a funded Stellar keypair and export its secret as KEEPER_SECRET.'
    );
  }

  return cfg;
}
