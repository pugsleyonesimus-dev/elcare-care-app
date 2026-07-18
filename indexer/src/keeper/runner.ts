/**
 * keeper/runner.ts
 *
 * Orchestrates a single keeper sweep cycle:
 *
 *   1. Discover candidates (expired listings, ended auctions, expired offers).
 *   2. Resume any Submitted actions that survived a restart (re-poll them).
 *   3. For each new candidate:
 *       a. Check idempotency store — skip if already Succeeded/Skipped/Submitted.
 *       b. Create a Pending KeeperAction row.
 *       c. Execute the tx pipeline → update row to Submitted → Succeeded/Failed/Skipped.
 *   4. Enforce max-actions-per-cycle and daily-budget caps.
 *   5. Emit metrics and structured logs throughout.
 *
 * The runner is stateless across calls — all durable state lives in the DB.
 */

import { rpc, Keypair } from '@stellar/stellar-sdk';
import { logger } from '../logger.js';
import {
  keeperActionsTotal,
  keeperFeesSpentStroops,
  keeperBudgetExhaustedTotal,
  keeperBudgetExhaustedGauge,
  keeperCycleDurationSeconds,
} from '../metrics.js';
import { loadKeeperConfig } from '../config.js';
import { discoverAllCandidates } from './candidates.js';
import { executeTransaction } from './tx-pipeline.js';
import {
  findAction,
  findActionsByStatus,
  createOrSkipAction,
  markSubmitted,
  markSucceeded,
  markFailed,
  markSkipped,
} from './idempotency.js';
import { ENTRY_POINT_LABEL } from './types.js';
import type { KeeperCandidate, KeeperCycleStats } from './types.js';

// ── Daily budget tracker (in-process; resets on restart) ─────────────────────
// For production you'd persist this in Redis or the DB; for the scope of this
// implementation an in-process counter is reset each calendar day.

let dailyBudgetDate = new Date().toDateString();
let dailyFeesSpentStroops = 0n;

function checkAndAccumulateFee(feeStroops: bigint, maxDaily: number): boolean {
  const today = new Date().toDateString();
  if (today !== dailyBudgetDate) {
    dailyBudgetDate = today;
    dailyFeesSpentStroops = 0n;
    keeperBudgetExhaustedGauge.set(0);
  }
  if (dailyFeesSpentStroops + feeStroops > BigInt(maxDaily)) {
    return false; // budget exhausted
  }
  dailyFeesSpentStroops += feeStroops;
  return true;
}

// ── Resume Submitted actions from a prior (possibly crashed) cycle ────────────

async function resumeSubmittedActions(
  server: rpc.Server,
  cfg: ReturnType<typeof loadKeeperConfig>,
  stats: KeeperCycleStats,
): Promise<void> {
  const submitted = await findActionsByStatus('Submitted');
  if (submitted.length === 0) return;

  logger.info('keeper: resuming submitted actions from prior run', { count: submitted.length });

  for (const action of submitted) {
    if (!action.txHash) continue;

    const entryLabel = ENTRY_POINT_LABEL[action.targetType as keyof typeof ENTRY_POINT_LABEL];

    try {
      const result = await server.getTransaction(action.txHash);

      if (result.status === rpc.Api.GetTransactionStatus.SUCCESS) {
        await markSucceeded(action.id, action.txHash, 0n);
        keeperActionsTotal.inc({ entry_point: entryLabel, outcome: 'succeeded' });
        stats.actionsSucceeded++;
        logger.info('keeper: resumed action succeeded', {
          id: action.id, txHash: action.txHash, targetType: action.targetType,
        });
      } else if (result.status === rpc.Api.GetTransactionStatus.FAILED) {
        await markFailed(action.id, `on-chain failure during resume: ${JSON.stringify(result)}`);
        keeperActionsTotal.inc({ entry_point: entryLabel, outcome: 'failed' });
        stats.actionsFailed++;
      }
      // NOT_FOUND: tx may still be propagating; leave in Submitted for next cycle
    } catch (err) {
      logger.warn('keeper: failed to poll resumed action', {
        id: action.id,
        txHash: action.txHash,
        err: err instanceof Error ? err.message : String(err),
      });
    }
  }
}

// ── Single action execution ───────────────────────────────────────────────────

async function processCandidate(
  candidate: KeeperCandidate,
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
  keypair: Keypair,
  cfg: ReturnType<typeof loadKeeperConfig>,
  dryRun: boolean,
  stats: KeeperCycleStats,
): Promise<void> {
  const entryLabel = ENTRY_POINT_LABEL[candidate.targetType];
  const candidateKey = `${candidate.targetType}:${candidate.targetId}`;

  // ── Idempotency check ─────────────────────────────────────────────────────
  const existing = await findAction(candidate);
  if (existing) {
    if (existing.status === 'Succeeded' || existing.status === 'Skipped') {
      logger.debug('keeper: candidate already terminal, skipping', { candidateKey, status: existing.status });
      return;
    }
    if (existing.status === 'Submitted') {
      // Still awaiting confirmation from a previous run; leave it alone.
      logger.debug('keeper: candidate already submitted, skipping', { candidateKey });
      return;
    }
    // Failed: only retry if attempts is within the configured retry window.
    // We use KEEPER_FEE_BUMP_MAX_RETRIES as the overall action retry cap
    // (distinct from the fee-bump loop retries within a single submission).
    const maxRetryAttempts = cfg.KEEPER_FEE_BUMP_MAX_RETRIES + 1;
    if (existing.status === 'Failed' && existing.attempts >= maxRetryAttempts) {
      logger.warn('keeper: candidate exceeded retry cap, skipping', {
        candidateKey, attempts: existing.attempts, maxRetryAttempts,
      });
      return;
    }
  }

  // ── Create / ensure Pending row ───────────────────────────────────────────
  const action = await createOrSkipAction(candidate);

  // If the upsert returned an existing Submitted/Succeeded/Skipped row, bail.
  if (action.status === 'Submitted' || action.status === 'Succeeded' || action.status === 'Skipped') {
    return;
  }

  stats.actionsAttempted++;

  // ── Execute pipeline ──────────────────────────────────────────────────────
  logger.info('keeper: executing action', { candidateKey, actionId: action.id, dryRun });

  const outcome = await executeTransaction(candidate, {
    server,
    contractId,
    networkPassphrase,
    keypair,
    cfg,
    dryRun,
  });

  switch (outcome.kind) {
    case 'succeeded': {
      if (dryRun) {
        // Dry-run: don't persist a Submitted state; just count and log.
        keeperActionsTotal.inc({ entry_point: entryLabel, outcome: 'dry_run' });
        return;
      }

      // Budget check before recording fees
      const fee = outcome.feePaid;
      if (!checkAndAccumulateFee(fee, cfg.KEEPER_DAILY_FEE_BUDGET_STROOPS)) {
        keeperBudgetExhaustedTotal.inc();
        keeperBudgetExhaustedGauge.set(1);
        stats.budgetExhausted = true;
        logger.warn('keeper: daily fee budget exhausted — halting cycle');
        // Re-mark this action as Pending so it's picked up next cycle.
        // (It was already upserted as Pending; nothing to change.)
        throw new BudgetExhaustedError();
      }

      await markSubmitted(action.id, outcome.txHash);
      await markSucceeded(action.id, outcome.txHash, fee);

      keeperActionsTotal.inc({ entry_point: entryLabel, outcome: 'succeeded' });
      keeperFeesSpentStroops.inc({ entry_point: entryLabel }, Number(fee));
      stats.actionsSucceeded++;
      stats.feesSpentStroops += fee;

      logger.info('keeper: action succeeded', {
        candidateKey, actionId: action.id, txHash: outcome.txHash, feePaid: fee.toString(),
      });
      break;
    }

    case 'permanent_skip': {
      await markSkipped(action.id, outcome.reason);
      keeperActionsTotal.inc({ entry_point: entryLabel, outcome: 'skipped' });
      stats.actionsSkipped++;
      logger.info('keeper: action permanently skipped', {
        candidateKey, actionId: action.id, reason: outcome.reason,
      });
      break;
    }

    case 'fee_bump_needed':
    case 'transient_failure': {
      const errMsg = outcome.kind === 'transient_failure'
        ? outcome.error.message
        : `fee_bump_needed after hash ${outcome.txHash}`;
      await markFailed(action.id, errMsg);
      keeperActionsTotal.inc({ entry_point: entryLabel, outcome: 'failed' });
      stats.actionsFailed++;
      logger.warn('keeper: action failed (transient)', {
        candidateKey, actionId: action.id, error: errMsg,
      });
      break;
    }
  }
}

// ── BudgetExhaustedError (signals early cycle halt) ───────────────────────────

export class BudgetExhaustedError extends Error {
  constructor() {
    super('keeper daily fee budget exhausted');
    this.name = 'BudgetExhaustedError';
  }
}

// ── Main cycle ────────────────────────────────────────────────────────────────

/**
 * Run one complete keeper sweep cycle.
 * Returns a KeeperCycleStats summary; never throws (errors are caught and
 * surfaced in the stats / logs).
 */
export async function runKeeperCycle(
  server: rpc.Server,
  contractId: string,
  networkPassphrase: string,
  keypair: Keypair,
  cfg: ReturnType<typeof loadKeeperConfig>,
  dryRun: boolean,
): Promise<KeeperCycleStats> {
  const startedAt = new Date();
  const stats: KeeperCycleStats = {
    startedAt,
    completedAt: null,
    candidatesDiscovered: 0,
    actionsAttempted: 0,
    actionsSucceeded: 0,
    actionsFailed: 0,
    actionsSkipped: 0,
    feesSpentStroops: 0n,
    budgetExhausted: false,
    dryRun,
  };

  const cycleTimer = keeperCycleDurationSeconds.startTimer();

  try {
    logger.info('keeper: sweep cycle starting', { dryRun });

    // Resume any in-flight actions from a prior crashed run first.
    await resumeSubmittedActions(server, cfg, stats);

    // Discover new candidates.
    const candidates = await discoverAllCandidates(server, contractId, networkPassphrase);
    stats.candidatesDiscovered = candidates.length;
    logger.info('keeper: candidates discovered', { count: candidates.length });

    // Apply the per-cycle action cap.
    const capped = candidates.slice(0, cfg.KEEPER_MAX_ACTIONS_PER_CYCLE);
    if (capped.length < candidates.length) {
      logger.warn('keeper: candidate list capped by KEEPER_MAX_ACTIONS_PER_CYCLE', {
        discovered: candidates.length,
        capped: capped.length,
      });
    }

    // Process each candidate sequentially to avoid competing writes and to
    // respect the daily budget exit path cleanly.
    for (const candidate of capped) {
      try {
        await processCandidate(
          candidate, server, contractId, networkPassphrase, keypair, cfg, dryRun, stats,
        );
      } catch (err) {
        if (err instanceof BudgetExhaustedError) {
          stats.budgetExhausted = true;
          break; // halt the entire cycle
        }
        // Any other unexpected error: log and continue to the next candidate
        logger.error('keeper: unexpected error processing candidate', {
          candidate: `${candidate.targetType}:${candidate.targetId}`,
          err: err instanceof Error ? err.message : String(err),
        });
      }
    }
  } catch (err) {
    logger.error('keeper: cycle error', {
      err: err instanceof Error ? err.message : String(err),
    });
  } finally {
    cycleTimer();
    stats.completedAt = new Date();
    logger.info('keeper: sweep cycle complete', {
      durationMs: stats.completedAt.getTime() - startedAt.getTime(),
      ...stats,
      feesSpentStroops: stats.feesSpentStroops.toString(),
    });
  }

  return stats;
}
