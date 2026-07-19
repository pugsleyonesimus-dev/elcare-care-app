/**
 * keeper/index.ts
 *
 * Public API of the keeper subsystem.  Two usage modes:
 *
 *   Embedded (default):
 *     import { startKeeper } from './keeper/index.js';
 *     startKeeper();  // called from src/index.ts when KEEPER_ENABLED=true
 *
 *   Standalone:
 *     tsx src/keeper/index.ts          # runs one cycle then exits
 *     KEEPER_DRY_RUN=false tsx src/keeper/index.ts   # live run
 *
 * The module exports `startKeeper` (long-running loop) and `runOnce`
 * (single cycle, useful for cron / Lambda invocations).
 */

import { rpc, Keypair } from '@stellar/stellar-sdk';
import dotenv from 'dotenv';
import { pathToFileURL } from 'node:url';
import { logger } from '../logger.js';
import { loadKeeperConfig } from '../config.js';
import { runKeeperCycle } from './runner.js';
import type { KeeperCycleStats } from './types.js';

dotenv.config();

// ── Shared state exposed to the status endpoint ───────────────────────────────

let lastCycleStats: KeeperCycleStats | null = null;
let keeperRunning = false;

export function getLastCycleStats(): KeeperCycleStats | null {
  return lastCycleStats;
}

export function isKeeperRunning(): boolean {
  return keeperRunning;
}

// ── Core setup ────────────────────────────────────────────────────────────────

function buildDependencies() {
  const cfg = loadKeeperConfig();

  if (!cfg.KEEPER_ENABLED && !cfg.KEEPER_DRY_RUN) {
    throw new Error('[keeper] Neither KEEPER_ENABLED nor KEEPER_DRY_RUN is active');
  }

  const rpcUrl         = process.env.STELLAR_RPC_URL!;
  const contractId     = process.env.MARKETPLACE_CONTRACT_ID!;
  const networkPassphrase = process.env.STELLAR_NETWORK_PASSPHRASE
    ?? (process.env.STELLAR_NETWORK === 'mainnet'
        ? 'Public Global Stellar Network ; September 2015'
        : 'Test SDF Network ; September 2015');

  const server = new rpc.Server(rpcUrl);

  // Keypair is required for live mode; in dry-run we still need it to build the
  // tx (the caller address must be a valid Stellar address) but we never sign.
  const secret = cfg.KEEPER_SECRET;
  if (!secret) {
    throw new Error('[keeper] KEEPER_SECRET must be set (even for dry-run, to derive the caller address)');
  }
  const keypair = Keypair.fromSecret(secret);

  return { cfg, server, contractId, networkPassphrase, keypair };
}

// ── Single-run export ─────────────────────────────────────────────────────────

export async function runOnce(): Promise<KeeperCycleStats> {
  const { cfg, server, contractId, networkPassphrase, keypair } = buildDependencies();
  const dryRun = cfg.KEEPER_DRY_RUN;

  logger.info('keeper: running single cycle', { dryRun });
  const stats = await runKeeperCycle(server, contractId, networkPassphrase, keypair, cfg, dryRun);
  lastCycleStats = stats;
  return stats;
}

// ── Long-running loop ─────────────────────────────────────────────────────────

export async function startKeeper(): Promise<void> {
  const { cfg, server, contractId, networkPassphrase, keypair } = buildDependencies();
  const dryRun = cfg.KEEPER_DRY_RUN;

  keeperRunning = true;
  logger.info('keeper: starting loop', {
    dryRun,
    intervalMs: cfg.KEEPER_INTERVAL_MS,
    maxActionsPerCycle: cfg.KEEPER_MAX_ACTIONS_PER_CYCLE,
    maxFeeStroops: cfg.KEEPER_MAX_FEE_STROOPS,
    dailyBudgetStroops: cfg.KEEPER_DAILY_FEE_BUDGET_STROOPS,
  });

  // Allow graceful shutdown via SIGTERM / SIGINT
  let shutdown = false;
  const onSignal = () => { shutdown = true; };
  process.once('SIGTERM', onSignal);
  process.once('SIGINT', onSignal);

  try {
    while (!shutdown) {
      const stats = await runKeeperCycle(
        server, contractId, networkPassphrase, keypair, cfg, dryRun,
      );
      lastCycleStats = stats;

      if (shutdown) break;
      await new Promise<void>((r) => setTimeout(r, cfg.KEEPER_INTERVAL_MS));
    }
  } finally {
    keeperRunning = false;
    logger.info('keeper: loop stopped');
  }
}

// ── Standalone entrypoint ─────────────────────────────────────────────────────
// When this file is run directly (tsx src/keeper/index.ts) execute one cycle
// and exit with an appropriate exit code.

const isMain = process.argv[1]
  ? pathToFileURL(process.argv[1]).href === import.meta.url
  : false;

if (isMain) {
  runOnce()
    .then((stats) => {
      logger.info('keeper: standalone run complete', {
        succeeded: stats.actionsSucceeded,
        failed: stats.actionsFailed,
        skipped: stats.actionsSkipped,
        dryRun: stats.dryRun,
      });
      process.exit(stats.actionsFailed > 0 ? 1 : 0);
    })
    .catch((err) => {
      logger.error('keeper: standalone run fatal error', {
        err: err instanceof Error ? err.message : String(err),
      });
      process.exit(1);
    });
}
