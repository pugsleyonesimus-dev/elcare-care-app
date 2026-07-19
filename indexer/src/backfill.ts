/**
 * backfill.ts
 *
 * Historical event backfill with:
 *   - BackfillJob checkpointing (per-batch, in the same DB transaction as
 *     applyDecodedEvents so crash = resume from last checkpoint, no duplicates)
 *   - No-clobber SyncState rule: only advance SyncState when the backfill
 *     range extends *ahead* of the live cursor; historical backfills leave
 *     the cursor untouched
 *   - Resumable via --resume=<jobId>
 *   - Advisory lock (pg_try_advisory_lock) so two workers cannot run the
 *     same job concurrently
 *
 * CLI usage:
 *   npm run backfill -- --start=X [--end=Y] [--rpc=URL]
 *   npm run backfill -- --resume=<jobId>
 */

import { rpc } from '@stellar/stellar-sdk';
import dotenv from 'dotenv';
import { pathToFileURL } from 'node:url';
import prisma from './db.js';
import { applyDecodedEvents, buildSyncStateLedgerData } from './poller.js';
import { collectMarketplaceEvents } from './event-sync.js';
import { logger } from './logger.js';
import {
  backfillJobsTotal,
  backfillDurationSeconds,
  backfillBatchLedgers,
  backfillBatchInserted,
  backfillLockContentions,
} from './metrics.js';

dotenv.config();

const BACKFILL_BATCH_SIZE = parseInt(process.env.BACKFILL_BATCH_SIZE || '5000');

// ── Types ────────────────────────────────────────────────────────────────────

export type BackfillJobStatus = 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Cancelled';

export interface BackfillJobRecord {
  id: number;
  startLedger: number;
  endLedger: number;
  checkpointLedger: number;
  status: BackfillJobStatus;
  rpcUrl: string;
  error: string | null;
  totalInserted: number;
  gapId: number | null;
  createdAt: Date;
  updatedAt: Date;
}

export interface RunBackfillOptions {
  rpcUrl?: string;
  startLedger?: number;
  endLedger?: number;
  resumeJobId?: number;
  gapId?: number | null;
  rpcServer?: rpc.Server;   // injected in tests
  batchSize?: number;
}

export interface BackfillResult {
  jobId: number;
  startLedger: number;
  endLedger: number;
  totalInserted: number;
  processedLedger: number;
  status: BackfillJobStatus;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function getContractIds(): string[] {
  return [
    process.env.MARKETPLACE_CONTRACT_ID || '',
    process.env.LAUNCHPAD_CONTRACT_ID   || '',
  ].filter(Boolean);
}

function readFlag(name: string): string | undefined {
  const prefix = `--${name}=`;
  return process.argv.slice(2).find((a) => a.startsWith(prefix))?.slice(prefix.length);
}

function parseLedger(value: string | undefined, label: string): number {
  if (!value) throw new Error(`Missing required --${label} flag`);
  const n = Number(value);
  if (!Number.isInteger(n) || n < 0)
    throw new Error(`Invalid --${label} value "${value}": must be a non-negative integer`);
  return n;
}

async function fetchLedgerHash(server: rpc.Server, ledger: number): Promise<string | null> {
  try {
    const res = await server.getLedgers({ startLedger: ledger, pagination: { limit: 1 } });
    return res.ledgers?.[0]?.hash ?? null;
  } catch (err) {
    logger.warn('backfill: failed to fetch ledger hash', {
      ledger,
      err: err instanceof Error ? err.message : String(err),
    });
    return null;
  }
}

/**
 * Postgres advisory lock key derived from BackfillJob id.
 * pg_try_advisory_lock takes a single bigint; we use a namespace prefix
 * (0xBACF = 47823) shifted 32 bits + the job id.
 */
function advisoryLockKey(jobId: number): bigint {
  return (BigInt(0xbacf) << 32n) | BigInt(jobId);
}

/**
 * Attempt to acquire a Postgres session-level advisory lock for the job.
 * Returns true if acquired, false if already held (another worker is running it).
 */
async function tryAcquireAdvisoryLock(jobId: number): Promise<boolean> {
  const key = advisoryLockKey(jobId);
  const result = await prisma.$queryRaw<[{ acquired: boolean }]>`
    SELECT pg_try_advisory_lock(${key}::bigint) AS acquired
  `;
  return result[0]?.acquired === true;
}

async function releaseAdvisoryLock(jobId: number): Promise<void> {
  const key = advisoryLockKey(jobId);
  await prisma.$queryRaw`SELECT pg_advisory_unlock(${key}::bigint)`;
}

// ── Cursor-interaction rule ────────────────────────────────────────────────────
//
// A backfill run that covers a range entirely *below* the live cursor must NOT
// write to SyncState — doing so would rewind the cursor and cause the poller to
// re-scan or lose its hash continuity checkpoint.
//
// A backfill run whose end extends *ahead* of the live cursor (initial-sync
// bootstrap use-case) SHOULD advance SyncState so the poller picks up from
// the right place.

export type CursorInteraction =
  | 'below'      // entire range < liveCursor   → do NOT touch SyncState
  | 'overlapping'// range straddles liveCursor   → do NOT touch SyncState (safe)
  | 'ahead';     // entire range > liveCursor    → DO advance SyncState

export function determineCursorInteraction(
  startLedger: number,
  endLedger: number,
  liveCursor: number,
): CursorInteraction {
  if (endLedger <= liveCursor)  return 'below';
  if (startLedger <= liveCursor) return 'overlapping';
  return 'ahead';
}

// ── Job lifecycle ─────────────────────────────────────────────────────────────

export async function createBackfillJob(
  startLedger: number,
  endLedger: number,
  rpcUrl: string,
  gapId?: number | null,
): Promise<BackfillJobRecord> {
  return prisma.backfillJob.create({
    data: {
      startLedger,
      endLedger,
      checkpointLedger: 0,
      status: 'Pending',
      rpcUrl,
      totalInserted: 0,
      gapId: gapId ?? null,
    },
  }) as unknown as BackfillJobRecord;
}

export async function getBackfillJob(id: number): Promise<BackfillJobRecord | null> {
  return prisma.backfillJob.findUnique({ where: { id } }) as unknown as BackfillJobRecord | null;
}

export async function listBackfillJobs(): Promise<BackfillJobRecord[]> {
  return prisma.backfillJob.findMany({
    orderBy: { createdAt: 'desc' },
  }) as unknown as BackfillJobRecord[];
}

export async function cancelBackfillJob(id: number): Promise<void> {
  await prisma.backfillJob.update({
    where: { id },
    data: { status: 'Cancelled' },
  });
}

// ── Core backfill executor ────────────────────────────────────────────────────

/**
 * Run (or resume) a single BackfillJob.
 *
 * Rules:
 *  - If opts.resumeJobId is given, loads that job and continues from
 *    its checkpointLedger.
 *  - Otherwise creates a new job, then runs it.
 *  - Advisory lock prevents two workers from running the same job.
 *  - SyncState is only updated when the job range extends ahead of the
 *    live cursor (determineCursorInteraction === 'ahead').
 */
export async function runBackfill(opts: RunBackfillOptions = {}): Promise<BackfillResult> {
  const contractIds = getContractIds();
  if (contractIds.length === 0) {
    throw new Error(
      'At least one of MARKETPLACE_CONTRACT_ID or LAUNCHPAD_CONTRACT_ID must be set',
    );
  }

  // ── Resolve job ───────────────────────────────────────────────────────────
  let job: BackfillJobRecord;

  if (opts.resumeJobId != null) {
    const existing = await getBackfillJob(opts.resumeJobId);
    if (!existing) throw new Error(`BackfillJob #${opts.resumeJobId} not found`);
    if (existing.status === 'Running') {
      throw new Error(
        `BackfillJob #${opts.resumeJobId} is already Running — may be held by another worker`,
      );
    }
    if (existing.status === 'Completed' || existing.status === 'Cancelled') {
      throw new Error(
        `BackfillJob #${opts.resumeJobId} is ${existing.status} — nothing to resume`,
      );
    }
    job = existing;
    logger.info('backfill: resuming existing job', {
      jobId: job.id, checkpoint: job.checkpointLedger,
    });
  } else {
    // Parse CLI args if not fully supplied via options
    const rpcUrl =
      opts.rpcUrl ??
      readFlag('rpc') ??
      process.env.ARCHIVAL_STELLAR_RPC_URL ??
      process.env.STELLAR_RPC_URL ??
      '';
    if (!rpcUrl) {
      throw new Error(
        'Missing archival RPC URL. Set ARCHIVAL_STELLAR_RPC_URL or pass --rpc=<url>.',
      );
    }

    const startLedger = opts.startLedger ?? parseLedger(readFlag('start'), 'start');
    const rawEnd      = opts.endLedger   ?? (readFlag('end') ? parseLedger(readFlag('end'), 'end') : undefined);

    const server      = opts.rpcServer ?? new rpc.Server(rpcUrl);
    const chainTip    = (await server.getLatestLedger()).sequence;
    const endLedger   = rawEnd ?? chainTip;

    if (endLedger > chainTip)
      throw new Error(`Invalid range: --end=${endLedger} exceeds chain tip (${chainTip})`);
    if (startLedger > endLedger)
      throw new Error(`Invalid range: --start=${startLedger} must be ≤ --end=${endLedger}`);

    job = await createBackfillJob(startLedger, endLedger, rpcUrl, opts.gapId ?? null);
    logger.info('backfill: created new job', { jobId: job.id, startLedger, endLedger });
  }

  // ── Advisory lock ─────────────────────────────────────────────────────────
  const locked = await tryAcquireAdvisoryLock(job.id);
  if (!locked) {
    backfillLockContentions.inc();
    throw new Error(
      `BackfillJob #${job.id} advisory lock is held by another worker — aborting`,
    );
  }

  // Mark Running
  await prisma.backfillJob.update({
    where: { id: job.id },
    data:  { status: 'Running', error: null },
  });

  const server =
    opts.rpcServer ??
    new rpc.Server(job.rpcUrl || process.env.STELLAR_RPC_URL || '');

  const batchSize  = opts.batchSize ?? BACKFILL_BATCH_SIZE;
  const startTimer = backfillDurationSeconds.startTimer();

  // Resume from checkpoint if this is a retry
  const resumeFrom =
    job.checkpointLedger > 0
      ? job.checkpointLedger + 1
      : job.startLedger;

  const { startLedger, endLedger } = job;
  const totalLedgers = endLedger - startLedger + 1;

  logger.info('backfill: starting run', {
    jobId: job.id, startLedger, endLedger, resumeFrom, batchSize,
  });

  let totalInserted   = job.totalInserted; // carry over from prior partial run
  let processedLedger = job.checkpointLedger > 0 ? job.checkpointLedger : startLedger - 1;

  try {
    // ── Determine SyncState cursor policy before scanning ──────────────────
    const syncState = await prisma.syncState.findUnique({ where: { id: 1 } });
    const liveCursor = syncState?.lastLedger ?? 0;
    const cursorInteraction = determineCursorInteraction(startLedger, endLedger, liveCursor);

    logger.info('backfill: cursor interaction', {
      jobId: job.id, cursorInteraction, liveCursor, startLedger, endLedger,
    });

    // ── Batch loop ─────────────────────────────────────────────────────────
    for (
      let batchStart = resumeFrom;
      batchStart <= endLedger;
      batchStart += batchSize
    ) {
      const batchEnd = Math.min(batchStart + batchSize - 1, endLedger);

      const decodedEvents = await collectMarketplaceEvents(
        server, contractIds, batchStart, batchEnd,
      );

      const batchMaxLedger =
        decodedEvents.length > 0
          ? Math.max(...decodedEvents.map((e) => e.ledgerSequence))
          : batchEnd;

      const latestHash = await fetchLedgerHash(server, batchMaxLedger);

      const batchInsertCount = await prisma.$transaction(async (tx) => {
        // 1. Insert events (existing dedupe semantics in applyDecodedEvents untouched)
        const inserted = await applyDecodedEvents(decodedEvents, tx);

        // 2. Checkpoint the job — always safe to update
        await tx.backfillJob.update({
          where: { id: job.id },
          data: {
            checkpointLedger: batchMaxLedger,
            totalInserted:    totalInserted + inserted.length,
          },
        });

        // 3. Advance SyncState ONLY when the backfill is running ahead of the
        //    live cursor (initial-sync bootstrap).  Historical backfills must
        //    leave the cursor untouched to protect the poller.
        if (cursorInteraction === 'ahead') {
          await tx.syncState.upsert({
            where:  { id: 1 },
            create: { id: 1, ...buildSyncStateLedgerData(batchMaxLedger, latestHash) },
            update: buildSyncStateLedgerData(batchMaxLedger, latestHash),
          });
        }

        return inserted.length;
      });

      totalInserted   += batchInsertCount;
      processedLedger  = batchMaxLedger;

      const progressPct = (((batchEnd - startLedger + 1) / totalLedgers) * 100).toFixed(1);
      logger.info(`backfill: progress ${progressPct}%`, {
        jobId: job.id, batchStart, batchEnd, batchInserted: batchInsertCount, processedLedger,
      });

      backfillBatchLedgers.observe(batchEnd - batchStart + 1);
      backfillBatchInserted.observe(batchInsertCount);
    }

    // ── Mark Completed ─────────────────────────────────────────────────────
    await prisma.backfillJob.update({
      where: { id: job.id },
      data:  { status: 'Completed', checkpointLedger: endLedger, totalInserted },
    });

    backfillJobsTotal.inc({ status: 'Completed' });
    startTimer();

    logger.info('backfill: job completed', { jobId: job.id, totalInserted, endLedger });

    return { jobId: job.id, startLedger, endLedger, totalInserted, processedLedger, status: 'Completed' };

  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);
    await prisma.backfillJob.update({
      where: { id: job.id },
      data:  { status: 'Failed', error: errMsg.slice(0, 4096) },
    });

    backfillJobsTotal.inc({ status: 'Failed' });
    startTimer();

    logger.error('backfill: job failed', { jobId: job.id, err: errMsg });
    throw err;

  } finally {
    await releaseAdvisoryLock(job.id).catch(() => {/* no-op on disconnect */});
  }
}

// ── Standalone entrypoint ─────────────────────────────────────────────────────

const isMain = process.argv[1]
  ? pathToFileURL(process.argv[1]).href === import.meta.url ||
    process.argv[1].includes('backfill')
  : false;

if (isMain) {
  const resumeFlag = readFlag('resume');
  const opts: RunBackfillOptions = resumeFlag
    ? { resumeJobId: parseInt(resumeFlag, 10) }
    : {};

  runBackfill(opts)
    .then((r) => {
      logger.info('backfill: standalone run complete', {
        jobId:    r.jobId,
        inserted: r.totalInserted,
        status:   r.status,
      });
      process.exit(0);
    })
    .catch((err) => {
      logger.error('backfill: standalone run fatal', {
        err: err instanceof Error ? err.message : String(err),
      });
      process.exit(1);
    });
}
