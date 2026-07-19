/**
 * deduplication.test.ts
 *
 * Verifies that processing the same on-chain event twice:
 *   1. Produces exactly one MarketplaceEvent row in the database
 *   2. Increments elcarehub_duplicate_events_total once
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import client from 'prom-client';

// ── Mock Prisma ───────────────────────────────────────────────────────────────

const storedEvents: Map<string, any> = new Map();

const mockTx = {
  marketplaceEvent: {
    findUnique: vi.fn(async ({ where }: { where: { eventHash: string } }) => {
      return storedEvents.get(where.eventHash) ?? null;
    }),
    create: vi.fn(async ({ data }: { data: any }) => {
      storedEvents.set(data.eventHash, data);
      return data;
    }),
  },
  listing: {
    upsert:      vi.fn().mockResolvedValue({}),
    updateMany:  vi.fn().mockResolvedValue({ count: 1 }),
    findMany:    vi.fn().mockResolvedValue([]),
  },
  auction: {
    upsert:      vi.fn().mockResolvedValue({}),
    updateMany:  vi.fn().mockResolvedValue({ count: 1 }),
  },
  offer: {
    upsert:  vi.fn().mockResolvedValue({}),
    update:  vi.fn().mockResolvedValue({}),
  },
  collection: {
    upsert: vi.fn().mockResolvedValue({}),
  },
};

const mockPrisma = vi.hoisted(() => ({
  marketplaceEvent: {
    findUnique: vi.fn(),
    create: vi.fn(),
    findMany: vi.fn().mockResolvedValue([]),
    count: vi.fn().mockResolvedValue(0),
  },
  listing: {
    upsert:     vi.fn().mockResolvedValue({}),
    updateMany: vi.fn().mockResolvedValue({ count: 1 }),
    findMany:   vi.fn().mockResolvedValue([]),
  },
  auction:    { upsert: vi.fn().mockResolvedValue({}), updateMany: vi.fn().mockResolvedValue({ count: 1 }) },
  offer:      { upsert: vi.fn().mockResolvedValue({}), update:     vi.fn().mockResolvedValue({}) },
  collection: { upsert: vi.fn().mockResolvedValue({}) },
  $transaction: vi.fn(),
}));

const mockRedis = vi.hoisted(() => ({
  isOpen:  false,
  isReady: false,
  get:     vi.fn().mockResolvedValue(null),
  set:     vi.fn().mockResolvedValue(undefined),
  setEx:   vi.fn().mockResolvedValue(undefined),
  on:      vi.fn(),
  connect: vi.fn().mockRejectedValue(new Error('no redis')),
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: mockRedis }));

import { applyDecodedEvents } from '../poller';
import { computeEventHash } from '../parser';
import { duplicateEventsCounter } from '../metrics';

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeEvent(overrides: Partial<any> = {}) {
  const contractId    = 'CONTRACT_A';
  const ledger        = 1000;
  const txHash        = 'txabc123';
  const eventIndex    = 0;

  return {
    eventType:      'LISTING_CREATED',
    listingId:      BigInt(42),
    actor:          'GARTIST',
    ledgerSequence: ledger,
    data:           { artist: 'GARTIST', price: '100', currency: 'XLM', collection: 'COL', token_id: 1, token: 'CTOKEN' },
    eventHash:      computeEventHash(contractId, ledger, txHash, eventIndex),
    contractId,
    txHash,
    eventIndex,
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Idempotent event processing — deduplication via eventHash', () => {
  beforeEach(() => {
    storedEvents.clear();
    vi.clearAllMocks();

    // Wire findUnique and create to the in-memory store
    mockTx.marketplaceEvent.findUnique.mockImplementation(
      async ({ where }: { where: { eventHash: string } }) =>
        storedEvents.get(where.eventHash) ?? null
    );
    mockTx.marketplaceEvent.create.mockImplementation(async ({ data }: { data: any }) => {
      storedEvents.set(data.eventHash, data);
      return data;
    });
  });

  it('inserts exactly one row when the same event is processed twice', async () => {
    const event = makeEvent();

    // First pass
    const first = await applyDecodedEvents([event], mockTx as any);
    // Second pass — same event, same hash
    const second = await applyDecodedEvents([event], mockTx as any);

    expect(first).toHaveLength(1);
    expect(second).toHaveLength(0);  // duplicate skipped

    // Only one create call total
    expect(mockTx.marketplaceEvent.create).toHaveBeenCalledTimes(1);
    expect(storedEvents.size).toBe(1);
  });

  it('increments duplicate_events_total counter exactly once on second pass', async () => {
    // Read the counter value before the test
    const registry = client.register;
    const getCount = async () => {
      const metrics = await registry.getMetricsAsJSON();
      const counter = metrics.find((m) => m.name === 'elcarehub_duplicate_events_total');
      if (!counter) return 0;
      const values = (counter as any).values as Array<{ value: number }>;
      return values.reduce((sum, v) => sum + v.value, 0);
    };

    const before = await getCount();
    const event = makeEvent();

    await applyDecodedEvents([event], mockTx as any);  // first — inserts
    await applyDecodedEvents([event], mockTx as any);  // second — duplicate

    const after = await getCount();
    expect(after - before).toBe(1);
  });

  it('inserts two distinct rows when events have different hashes', async () => {
    const e1 = makeEvent({ eventIndex: 0 });
    const e2 = makeEvent({
      eventIndex: 1,
      listingId: BigInt(99),
      eventHash: computeEventHash('CONTRACT_A', 1000, 'txabc123', 1),
    });

    await applyDecodedEvents([e1, e2], mockTx as any);

    expect(mockTx.marketplaceEvent.create).toHaveBeenCalledTimes(2);
    expect(storedEvents.size).toBe(2);
  });

  it('computeEventHash produces a 64-char hex string', () => {
    const hash = computeEventHash('C_ID', 500, 'txhash', 3);
    expect(typeof hash).toBe('string');
    expect(hash).toHaveLength(64);
    expect(/^[0-9a-f]+$/.test(hash)).toBe(true);
  });

  it('computeEventHash is deterministic for the same inputs', () => {
    const a = computeEventHash('C', 1, 'tx', 0);
    const b = computeEventHash('C', 1, 'tx', 0);
    expect(a).toBe(b);
  });

  it('computeEventHash differs when any input changes', () => {
    const base = computeEventHash('C', 1, 'tx', 0);
    expect(computeEventHash('X', 1, 'tx', 0)).not.toBe(base);  // contractId
    expect(computeEventHash('C', 2, 'tx', 0)).not.toBe(base);  // ledger
    expect(computeEventHash('C', 1, 'xy', 0)).not.toBe(base);  // txHash
    expect(computeEventHash('C', 1, 'tx', 1)).not.toBe(base);  // eventIndex
  });
});
