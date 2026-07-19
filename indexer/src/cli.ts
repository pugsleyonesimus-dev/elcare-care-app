/**
 * cli.ts
 *
 * Operator CLI for backfill and gap management.
 *
 * Usage:
 *   npm run cli -- backfill list
 *   npm run cli -- backfill resume <jobId>
 *   npm run cli -- backfill cancel <jobId>
 *   npm run cli -- gaps list
 *   npm run cli -- gaps repair [--gap=<id>]
 *   npm run cli -- gaps open          # alias for gaps list --status=Open
 */

import dotenv from 'dotenv';
import prisma from './db.js';
import {
  listBackfillJobs,
  getBackfillJob,
  cancelBackfillJob,
  runBackfill,
} from './backfill.js';
import {
  runRepairCycle,
  repairGap,
} from './gap-repair.js';

dotenv.config();

// ── Formatting helpers ────────────────────────────────────────────────────────

function fmt(obj: unknown): string {
  return JSON.stringify(obj, (_k, v) => (typeof v === 'bigint' ? v.toString() : v), 2);
}

function pad(s: string | number, n: number): string {
  return String(s).padEnd(n);
}

function printJobTable(jobs: Awaited<ReturnType<typeof listBackfillJobs>>): void {
  if (jobs.length === 0) {
    console.log('No backfill jobs found.');
    return;
  }
  console.log(
    `${pad('ID', 6)}${pad('STATUS', 12)}${pad('START', 12)}${pad('END', 12)}` +
    `${pad('CHECKPOINT', 12)}${pad('INSERTED', 10)}${pad('GAP_ID', 8)}CREATED`,
  );
  console.log('─'.repeat(90));
  for (const j of jobs) {
    console.log(
      `${pad(j.id, 6)}${pad(j.status, 12)}${pad(j.startLedger, 12)}${pad(j.endLedger, 12)}` +
      `${pad(j.checkpointLedger, 12)}${pad(j.totalInserted, 10)}${pad(j.gapId ?? '-', 8)}` +
      `${j.createdAt.toISOString()}`,
    );
    if (j.error) console.log(`  error: ${j.error}`);
  }
}

function printGapTable(gaps: Awaited<ReturnType<typeof listGaps>>): void {
  if (gaps.length === 0) {
    console.log('No gaps found.');
    return;
  }
  console.log(
    `${pad('ID', 6)}${pad('STATUS', 12)}${pad('SOURCE', 20)}` +
    `${pad('FROM', 12)}${pad('TO', 12)}${pad('LEDGERS', 10)}CREATED`,
  );
  console.log('─'.repeat(90));
  for (const g of gaps) {
    const count = g.toLedger - g.fromLedger + 1;
    console.log(
      `${pad(g.id, 6)}${pad(g.status, 12)}${pad(g.source, 20)}` +
      `${pad(g.fromLedger, 12)}${pad(g.toLedger, 12)}${pad(count, 10)}` +
      `${g.createdAt.toISOString()}`,
    );
    if (g.error) console.log(`  error: ${g.error}`);
  }
}

// ── DB helpers ────────────────────────────────────────────────────────────────

async function listGaps(status?: string) {
  const where: any = {};
  if (status) where.status = status;
  return prisma.ledgerGap.findMany({
    where,
    orderBy: { createdAt: 'asc' },
  });
}

// ── Command handlers ──────────────────────────────────────────────────────────

async function handleBackfill(args: string[]): Promise<void> {
  const sub = args[0];

  switch (sub) {
    case 'list': {
      const jobs = await listBackfillJobs();
      printJobTable(jobs);
      break;
    }

    case 'resume': {
      const rawId = args[1];
      if (!rawId || !/^\d+$/.test(rawId)) {
        console.error('Usage: backfill resume <jobId>');
        process.exit(1);
      }
      const jobId = parseInt(rawId, 10);
      console.log(`Resuming BackfillJob #${jobId}...`);
      const result = await runBackfill({ resumeJobId: jobId });
      console.log(`Done. Status=${result.status}, inserted=${result.totalInserted}`);
      break;
    }

    case 'cancel': {
      const rawId = args[1];
      if (!rawId || !/^\d+$/.test(rawId)) {
        console.error('Usage: backfill cancel <jobId>');
        process.exit(1);
      }
      const jobId = parseInt(rawId, 10);
      const job = await getBackfillJob(jobId);
      if (!job) {
        console.error(`BackfillJob #${jobId} not found`);
        process.exit(1);
      }
      if (job.status === 'Completed' || job.status === 'Cancelled') {
        console.error(`BackfillJob #${jobId} is already ${job.status}`);
        process.exit(1);
      }
      await cancelBackfillJob(jobId);
      console.log(`BackfillJob #${jobId} marked Cancelled.`);
      break;
    }

    default:
      console.error(`Unknown backfill subcommand: "${sub}"`);
      console.error('Available: list | resume <id> | cancel <id>');
      process.exit(1);
  }
}

async function handleGaps(args: string[]): Promise<void> {
  const sub = args[0];

  switch (sub) {
    case 'list':
    case 'open': {
      const status = sub === 'open' ? 'Open' : undefined;
      const gaps = await listGaps(status);
      printGapTable(gaps);
      break;
    }

    case 'repair': {
      // Optional --gap=<id> to repair a specific gap; otherwise run a full cycle
      const gapFlag = args.find((a) => a.startsWith('--gap='))?.split('=')[1];
      if (gapFlag) {
        const gapId = parseInt(gapFlag, 10);
        if (isNaN(gapId)) {
          console.error('--gap must be a valid integer gap ID');
          process.exit(1);
        }
        console.log(`Repairing gap #${gapId}...`);
        const result = await repairGap(gapId);
        console.log(fmt(result));
        process.exit(result.status === 'Repaired' ? 0 : 1);
      } else {
        console.log('Running full gap-repair cycle...');
        const results = await runRepairCycle();
        const repaired = results.filter((r) => r.status === 'Repaired').length;
        const failed   = results.filter((r) => r.status === 'Failed').length;
        console.log(`Cycle complete: ${repaired} repaired, ${failed} failed`);
        if (results.length > 0) console.log(fmt(results));
        process.exit(failed > 0 ? 1 : 0);
      }
    }

    default:
      console.error(`Unknown gaps subcommand: "${sub}"`);
      console.error('Available: list | open | repair [--gap=<id>]');
      process.exit(1);
  }
}

// ── Entrypoint ────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const [, , domain, ...rest] = process.argv;

  switch (domain) {
    case 'backfill':
      await handleBackfill(rest);
      break;
    case 'gaps':
      await handleGaps(rest);
      break;
    default:
      console.error(`Unknown command domain: "${domain}"`);
      console.error('Usage: cli <backfill|gaps> <subcommand> [args]');
      process.exit(1);
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error('CLI fatal error:', err instanceof Error ? err.message : String(err));
    process.exit(1);
  })
  .finally(() => prisma.$disconnect());
