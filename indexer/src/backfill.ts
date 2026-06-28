import { rpc } from '@stellar/stellar-sdk';
import dotenv from 'dotenv';
import { pathToFileURL } from 'node:url';
import prisma from './db.js';
import { applyDecodedEvents, buildSyncStateLedgerData } from './poller.js';
import { collectMarketplaceEvents } from './event-sync.js';

dotenv.config();

const BACKFILL_BATCH_SIZE = parseInt(process.env.BACKFILL_BATCH_SIZE || '5000');

type BackfillArgs = {
  rpcUrl: string;
  startLedger: number;
  endLedger?: number;
};

function getContractIds(): string[] {
  return [
    process.env.MARKETPLACE_CONTRACT_ID || '',
    process.env.LAUNCHPAD_CONTRACT_ID || '',
  ].filter(Boolean);
}

function readFlag(name: string): string | undefined {
  const prefix = `--${name}=`;
  return process.argv.slice(2).find((arg) => arg.startsWith(prefix))?.slice(prefix.length);
}

function parseLedger(value: string | undefined, label: string): number {
  if (!value) {
    throw new Error(`Missing required --${label} flag`);
  }

  const ledger = Number(value);
  if (!Number.isInteger(ledger) || ledger < 0) {
    throw new Error(`Invalid --${label} value "${value}": must be a non-negative integer`);
  }

  return ledger;
}

function parseArgs(): BackfillArgs {
  const endLedgerFlag = readFlag('end');
  const rpcUrl = readFlag('rpc') || process.env.ARCHIVAL_STELLAR_RPC_URL || process.env.STELLAR_RPC_URL || '';
  if (!rpcUrl) {
    throw new Error('Missing archival RPC URL. Set ARCHIVAL_STELLAR_RPC_URL or pass --rpc=<url>.');
  }

  const startLedger = parseLedger(readFlag('start'), 'start');
  const endLedger = endLedgerFlag ? parseLedger(endLedgerFlag, 'end') : undefined;

  if (endLedger !== undefined && startLedger > endLedger) {
    throw new Error(`Invalid range: --start=${startLedger} must be ≤ --end=${endLedger}`);
  }

  return { rpcUrl, startLedger, endLedger };
}

async function fetchLedgerHash(server: rpc.Server, ledger: number): Promise<string | null> {
  try {
    const ledgersRes = await server.getLedgers({
      startLedger: ledger,
      pagination: { limit: 1 },
    });

    return ledgersRes.ledgers?.[0]?.hash ?? null;
  } catch (err) {
    console.error({ msg: 'Failed to fetch ledger hash during backfill', ledger, err });
    return null;
  }
}

export async function runBackfill(overrides?: Partial<BackfillArgs & { rpcServer?: rpc.Server }>) {
  const parsed = parseArgs();
  const rpcUrl = overrides?.rpcUrl ?? parsed.rpcUrl;
  const startLedger = overrides?.startLedger ?? parsed.startLedger;
  const endLedgerOverride = overrides?.endLedger ?? parsed.endLedger;

  const contractIds = getContractIds();

  if (contractIds.length === 0) {
    throw new Error('At least one of MARKETPLACE_CONTRACT_ID or LAUNCHPAD_CONTRACT_ID must be set');
  }

  const server = overrides?.rpcServer ?? new rpc.Server(rpcUrl);

  // Fetch chain tip to validate the requested range.
  const chainTip = (await server.getLatestLedger()).sequence;
  const endLedger = endLedgerOverride ?? chainTip;

  if (endLedger > chainTip) {
    throw new Error(
      `Invalid range: --end=${endLedger} exceeds the current chain tip (${chainTip})`
    );
  }

  if (startLedger > endLedger) {
    throw new Error(`Invalid range: --start=${startLedger} must be ≤ --end=${endLedger}`);
  }

  const totalLedgers = endLedger - startLedger + 1;
  console.log({
    msg: 'Backfill starting',
    startLedger,
    endLedger,
    totalLedgers,
    batchSize: BACKFILL_BATCH_SIZE,
  });

  let totalInserted = 0;
  let processedLedger = startLedger - 1;

  for (let batchStart = startLedger; batchStart <= endLedger; batchStart += BACKFILL_BATCH_SIZE) {
    const batchEnd = Math.min(batchStart + BACKFILL_BATCH_SIZE - 1, endLedger);

    const decodedEvents = await collectMarketplaceEvents(
      server,
      contractIds,
      batchStart,
      batchEnd
    );

    const batchMaxLedger = decodedEvents.length > 0
      ? Math.max(...decodedEvents.map((e) => e.ledgerSequence))
      : batchEnd;

    const latestHash = await fetchLedgerHash(server, batchMaxLedger);

    const { insertedCount } = await prisma.$transaction(async (tx) => {
      const inserted = await applyDecodedEvents(decodedEvents, tx);
      const ledgerData = buildSyncStateLedgerData(batchMaxLedger, latestHash);
      await tx.syncState.upsert({
        where: { id: 1 },
        create: { id: 1, ...ledgerData },
        update: ledgerData,
      });
      return { insertedCount: inserted.length };
    });

    totalInserted += insertedCount;
    processedLedger = batchMaxLedger;

    const progressPct = (((batchEnd - startLedger + 1) / totalLedgers) * 100).toFixed(1);
    console.log({
      msg: `Backfill progress: ${progressPct}%`,
      batchStart,
      batchEnd,
      batchInserted: insertedCount,
      processedLedger,
    });
  }

  console.log({
    msg: 'Backfill complete',
    startLedger,
    endLedger,
    totalInserted,
    processedLedger,
  });

  return { startLedger, endLedger, totalInserted, processedLedger };
}

if (process.argv[1] && process.argv[1].includes('backfill')) {
  runBackfill().catch((err) => {
    console.error({ msg: 'Backfill failed', err: err instanceof Error ? err.message : String(err) });
    process.exit(1);
  });
}
