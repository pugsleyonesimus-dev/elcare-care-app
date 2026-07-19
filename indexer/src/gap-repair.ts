/**
 * gap-repair.ts
 *
 * Background worker that discovers Open LedgerGap rows and triggers
 * BackfillJob runs against the archival RPC to cover each gap.
 *
 * Lifecycle per gap:
 *   Open → Repairing (job created + advisory lock acquired)
 *        → Repaired  (job Completed)
 *        → Failed    (all retries exhausted)
 *
 * Safety:
 *   - One worker claims one gap at a time via a compare-and-set update
 *     (status: Open → Repairing). Two concurrent workers will each try
 *     to claim different gaps because the update targets only Open rows.
 *   - The BackfillJob advisory lock (pg_try_advisory_lock) provides a
 *     second layer: even if two workers claim the same gap row (e.g. a
 *     race before the CAS lands), only one will hold the advisory lock
 *     and the other will abort.
 *   - Failures are retried up to GAP_REPAIR_MAX_RETRIES times with
 *     withRetry exponential back-off; after that the gap is marked Failed
 *     and the worker moves on so other gaps are not blocked.
 *
 * Standalone usage:
 *   npm run gaps:repair
 *
 * Embedded: imported in index.ts behind GAP_REPAIR_ENABLED=true.
 */

import { pathToFileURL } from 'node:url';
import dotenv from 'dotenv';
import prisma from './db.js';
import { runBackfill } from './backfill.js';
import { withRetry } from './retry.js';
import { logger } from './logger.js';
import {
  openGapsGauge,
  openGapLedgersTotalGauge,
} from './metrics.js';

dotenv.config();

// ── Config ────────────────────────────────────────────────────────────────────

const GAP_REPAIR_INTERVAL_MS = parseInt(
  process.env.GAP_REPAIR_INTERVAL_MS || '120000', // 2 min
);

const GAP_REPAIR_MAX_RETRIES = parseInt(
  process.env.GAP_REPAIR_MAX_RETRIES || '3',
);

const GAP_REPAIR_BATCH_SIZE = parseInt(
  process.env.GAP_REPAIR_BATCH_SIZE || process.env.BACKFILL_BATCH_SIZE || '5000',
);

// ── Types ─────────────────────────────────────────────────────────────────────

export interface GapRepairResult {
  gapId: number;
  fromLedger: number;
  toLedger: number;
  jobId: number | null;
  status: 'Repaired' | 'Failed' | 'Skipped';
  error?: string;
}

// ── Core repair logic ─────────────────────────────────────────────────────────

/**
 * Attempt to claim one Open gap (compare-and-set: Open → Repairing).
 * Returns the updated gap row, or null if there are no Open gaps or
 * the row was claimed by a concurrent worker before we could update it.
 */
async function claimNextOpenGap() {
  // Find oldest Open gap
  const candidate = await prisma.ledgerGap.findFirst({
    where: { status: 'Open' },
    orderBy: { createdAt: 'asc' },
  });
  if (!candidate) return null;

  // CAS: only update if still Open (concurrent worker may have claimed it first)
  const result = await prisma.ledgerGap.updateMany({
    where: { id: candidate.id, status: 'Open' },
    data:  { status: 'Repairing', error: null },
  });

  if (result.count === 0) return null; // lost the race — try again next cycle

  return prisma.ledgerGap.findUnique({ where: { id: candidate.id } });
}

/**
 * Repair a single gap: run a BackfillJob, then mark the gap Repaired or Failed.
 */
export async function repairGap(gapId: number): Promise<GapRepairResult> {
  const gap = await prisma.ledgerGap.findUnique({ where: { id: gapId } });
  if (!gap) {
    return { gapId, fromLedger: 0, toLedger: 0, jobId: null, status: 'Failed', error: 'Gap not found' };
  }

  const rpcUrl =
    process.env.ARCHIVAL_STELLAR_RPC_URL ||
    process.env.STELLAR_RPC_URL ||
    '';

  if (!rpcUrl) {
    await prisma.ledgerGap.update({
      where: { id: gapId },
      data:  { status: 'Failed', error: 'ARCHIVAL_STELLAR_RPC_URL not set' },
    });
    return {
      gapId, fromLedger: gap.fromLedger, toLedger: gap.toLedger, jobId: null,
      status: 'Failed', error: 'ARCHIVAL_STELLAR_RPC_URL not set',
    };
  }

  logger.info('gap-repair: starting repair', {
    gapId, fromLedger: gap.fromLedger, toLedger: gap.toLedger,
  });

  let jobId: number | null = null;

  try {
    const result = await withRetry(
      () => runBackfill({
        rpcUrl,
        startLedger: gap.fromLedger,
        endLedger:   gap.toLedger,
        gapId:       gapId,
        batchSize:   GAP_REPAIR_BATCH_SIZE,
      }),
      {
        operation:   `gap-repair-${gapId}`,
        maxAttempts: GAP_REPAIR_MAX_RETRIES,
        baseDelayMs: 2_000,
        maxDelayMs:  60_000,
        jitterMs:    1_000,
      },
    );

    jobId = result.jobId;

    await prisma.ledgerGap.update({
      where: { id: gapId },
      data:  { status: 'Repaired', error: null },
    });

    logger.info('gap-repair: gap repaired', {
      gapId, jobId, totalInserted: result.totalInserted,
    });

    return {
      gapId, fromLedger: gap.fromLedger, toLedger: gap.toLedger,
      jobId, status: 'Repaired',
    };

  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);

    await prisma.ledgerGap.update({
      where: { id: gapId },
      data:  { status: 'Failed', error: errMsg.slice(0, 4096) },
    });

    logger.error('gap-repair: gap repair failed', { gapId, jobId, err: errMsg });

    return {
      gapId, fromLedger: gap.fromLedger, toLedger: gap.toLedger,
      jobId, status: 'Failed', error: errMsg,
    };
  }
}

/**
 * Run one complete repair cycle: claim and repair all Open gaps one by one.
 * Returns the list of repair results for the cycle.
 */
export async function runRepairCycle(): Promise<GapRepairResult[]> {
  const results: GapRepairResult[] = [];

  // Refresh open-gap gauges at the start of each cycle
  const openGaps = await prisma.ledgerGap.findMany({
    where: { status: 'Open' },
    select: { fromLedger: true, toLedger: true },
  });
  openGapsGauge.set(openGaps.length);
  const totalLedgers = openGaps.reduce((a, g) => a + (g.toLedger - g.fromLedger + 1), 0);
  openGapLedgersTotalGauge.set(totalLedgers);

  if (openGaps.length === 0) {
    logger.debug('gap-repair: no open gaps this cycle');
    return results;
  }

  logger.info('gap-repair: cycle starting', { openGaps: openGaps.length, totalLedgers });

  // Claim and repair gaps one at a time; a concurrent worker may claim others
  // in parallel — that is intentional and safe due to the CAS + advisory lock.
  while (true) {
    const gap = await claimNextOpenGap();
    if (!gap) break; // no more Open gaps this cycle

    const result = await repairGap(gap.id);
    results.push(result);

    // Pause briefly between gaps to avoid hammering the archival RPC
    await new Promise((r) => setTimeout(r, 500));
  }

  logger.info('gap-repair: cycle complete', {
    repaired: results.filter((r) => r.status === 'Repaired').length,
    failed:   results.filter((r) => r.status === 'Failed').length,
  });

  return results;
}

// ── Long-running loop ─────────────────────────────────────────────────────────

let workerRunning = false;

export function isGapRepairRunning(): boolean {
  return workerRunning;
}

export async function startGapRepairWorker(): Promise<void> {
  if (workerRunning) {
    logger.warn('gap-repair: worker already running — ignoring duplicate start');
    return;
  }

  workerRunning = true;
  logger.info('gap-repair: worker starting', { intervalMs: GAP_REPAIR_INTERVAL_MS });

  let shutdown = false;
  const onSignal = () => { shutdown = true; };
  process.once('SIGTERM', onSignal);
  process.once('SIGINT', onSignal);

  try {
    while (!shutdown) {
      try {
        await runRepairCycle();
      } catch (err) {
        logger.error('gap-repair: cycle error', {
          err: err instanceof Error ? err.message : String(err),
        });
      }

      if (shutdown) break;
      await new Promise<void>((r) => setTimeout(r, GAP_REPAIR_INTERVAL_MS));
    }
  } finally {
    workerRunning = false;
    logger.info('gap-repair: worker stopped');
  }
}

// ── Standalone entrypoint ─────────────────────────────────────────────────────

const isMain = process.argv[1]
  ? pathToFileURL(process.argv[1]).href === import.meta.url ||
    process.argv[1].includes('gap-repair')
  : false;

if (isMain) {
  runRepairCycle()
    .then((results) => {
      const repaired = results.filter((r) => r.status === 'Repaired').length;
      const failed   = results.filter((r) => r.status === 'Failed').length;
      logger.info('gap-repair: standalone run complete', { repaired, failed });
      process.exit(failed > 0 ? 1 : 0);
    })
    .catch((err) => {
      logger.error('gap-repair: standalone run fatal', {
        err: err instanceof Error ? err.message : String(err),
      });
      process.exit(1);
    });
}
