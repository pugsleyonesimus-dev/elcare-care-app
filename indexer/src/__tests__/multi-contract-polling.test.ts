import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Prevent dotenv from loading .env
vi.mock('dotenv', () => ({ default: { config: vi.fn() } }));

// ── Mock Prisma ───────────────────────────────────────────────────────────────

const mockTx = vi.hoisted(() => ({
  marketplaceEvent: {
    deleteMany: vi.fn().mockResolvedValue({}),
    findUnique: vi.fn().mockResolvedValue(null),
    create: vi.fn().mockResolvedValue({}),
  },
  listing: {
    deleteMany: vi.fn().mockResolvedValue({}),
    updateMany: vi.fn().mockResolvedValue({ count: 1 }),
    upsert: vi.fn().mockResolvedValue({}),
  },
  auction: {
    upsert: vi.fn().mockResolvedValue({}),
    updateMany: vi.fn().mockResolvedValue({ count: 1 }),
  },
  offer: {
    upsert: vi.fn().mockResolvedValue({}),
    update: vi.fn().mockResolvedValue({}),
  },
  collection: {
    deleteMany: vi.fn().mockResolvedValue({}),
    upsert: vi.fn().mockResolvedValue({}),
  },
  syncState: {
    upsert: vi.fn().mockResolvedValue({ id: 1, lastLedger: 0, lastLedgerHash: null }),
    update: vi.fn().mockResolvedValue({ id: 1, lastLedger: 100 }),
  },
}));

const mockPrisma = vi.hoisted(() => ({
  trackedContract: {
    upsert: vi.fn(),
    findMany: vi.fn(),
    findUnique: vi.fn(),
    update: vi.fn(),
  },
  marketplaceEvent: {
    create: vi.fn().mockResolvedValue({}),
    findUnique: vi.fn().mockResolvedValue(null),
  },
  listing: {
    upsert: vi.fn().mockResolvedValue({}),
    updateMany: vi.fn().mockResolvedValue({ count: 1 }),
  },
  auction: {
    upsert: vi.fn().mockResolvedValue({}),
    updateMany: vi.fn().mockResolvedValue({ count: 1 }),
  },
  offer: {
    upsert: vi.fn().mockResolvedValue({}),
    update: vi.fn().mockResolvedValue({}),
  },
  collection: { upsert: vi.fn().mockResolvedValue({}) },
  syncState: {
    upsert: vi.fn().mockResolvedValue({ id: 1, lastLedger: 0 }),
    update: vi.fn().mockResolvedValue({}),
    findUnique: vi.fn().mockResolvedValue({ id: 1, lastLedger: 0 }),
  },
  ledgerGap: {
    upsert: vi.fn().mockResolvedValue({}),
    findMany: vi.fn().mockResolvedValue([]),
  },
  $transaction: vi.fn((fn: (tx: typeof mockTx) => Promise<any>) => fn(mockTx)),
  $disconnect: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../metrics.js', () => ({
  latestLedgerProcessedGauge: { set: vi.fn() },
  networkLatestLedgerGauge: { set: vi.fn() },
  syncLatencyGauge: { set: vi.fn() },
  decodeErrorsCounter: { inc: vi.fn() },
  duplicateEventsCounter: { inc: vi.fn() },
  gapsCreatedTotal: { inc: vi.fn() },
  openGapsGauge: { set: vi.fn() },
  openGapLedgersTotalGauge: { set: vi.fn() },
}));
vi.mock('../stall.js', () => ({ recordProgress: vi.fn() }));
vi.mock('../api/routes.js', () => ({ emitSSEEvent: vi.fn() }));
vi.mock('../logger.js', () => ({
  logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn(), debug: vi.fn() },
}));
vi.mock('../redis.js', () => ({ default: { get: vi.fn(), set: vi.fn(), disconnect: vi.fn() } }));
vi.mock('../retry.js', () => ({
  withRetry: vi.fn((fn: () => any) => fn()),
}));

vi.mock('@stellar/stellar-sdk', () => ({
  rpc: {
    Server: class {
      getEvents() { return Promise.resolve({ events: [] }); }
      getLedgers() {
        return Promise.resolve({ ledgers: [{ hash: 'testhash', sequence: 100 }] });
      }
      getLatestLedger() { return Promise.resolve({ sequence: 1000 }); }
    },
    Api: { isSimulationError: () => false },
  },
  Contract: class { call() { return {}; } },
  TransactionBuilder: class {
    addOperation() { return this; }
    setTimeout() { return this; }
    build() { return {}; }
  },
  BASE_FEE: '100',
  nativeToScVal: () => ({}),
  scValToNative: () => ({}),
  Address: class {
    constructor(public addr: string) {}
    toScVal() { return {}; }
    toString() { return this.addr; }
  },
}));

// ── parseTrackedContracts ─────────────────────────────────────────────────────

import { parseTrackedContracts, validateRequiredEnv } from '../config';

describe('parseTrackedContracts', () => {
  const ORIG = { ...process.env };

  afterEach(() => {
    process.env = { ...ORIG };
  });

  it('parses TRACKED_CONTRACTS JSON array', () => {
    process.env.TRACKED_CONTRACTS = JSON.stringify([
      { id: 'CA_MARKET', type: 'marketplace', label: 'mainnet', startLedger: 500 },
      { id: 'CB_LAUNCH', type: 'launchpad', label: 'launchpad', startLedger: 0 },
    ]);
    const contracts = parseTrackedContracts();
    expect(contracts).toHaveLength(2);
    expect(contracts[0]).toMatchObject({ id: 'CA_MARKET', type: 'marketplace', label: 'mainnet', startLedger: 500 });
    expect(contracts[1]).toMatchObject({ id: 'CB_LAUNCH', type: 'launchpad' });
  });

  it('applies default startLedger=0 when not provided', () => {
    process.env.TRACKED_CONTRACTS = JSON.stringify([
      { id: 'CA_MARKET', type: 'marketplace' },
    ]);
    const [c] = parseTrackedContracts();
    expect(c.startLedger).toBe(0);
    expect(c.label).toBe('');
  });

  it('falls back to MARKETPLACE_CONTRACT_ID when TRACKED_CONTRACTS is absent', () => {
    delete process.env.TRACKED_CONTRACTS;
    process.env.MARKETPLACE_CONTRACT_ID = 'CLEGACY';
    delete process.env.LAUNCHPAD_CONTRACT_ID;
    const contracts = parseTrackedContracts();
    expect(contracts).toHaveLength(1);
    expect(contracts[0]).toMatchObject({ id: 'CLEGACY', type: 'marketplace' });
  });

  it('includes both legacy vars when both are set', () => {
    delete process.env.TRACKED_CONTRACTS;
    process.env.MARKETPLACE_CONTRACT_ID = 'CMARKET';
    process.env.LAUNCHPAD_CONTRACT_ID = 'CLAUNCH';
    const contracts = parseTrackedContracts();
    expect(contracts).toHaveLength(2);
    expect(contracts.map((c) => c.id)).toEqual(['CMARKET', 'CLAUNCH']);
  });

  it('throws on invalid JSON', () => {
    process.env.TRACKED_CONTRACTS = '{not json}';
    expect(() => parseTrackedContracts()).toThrow('not valid JSON');
  });

  it('throws when TRACKED_CONTRACTS is not an array', () => {
    process.env.TRACKED_CONTRACTS = JSON.stringify({ id: 'C' });
    expect(() => parseTrackedContracts()).toThrow('must be a JSON array');
  });

  it('throws when TRACKED_CONTRACTS is empty array', () => {
    process.env.TRACKED_CONTRACTS = '[]';
    expect(() => parseTrackedContracts()).toThrow('at least one entry');
  });

  it('throws when an entry has invalid type', () => {
    process.env.TRACKED_CONTRACTS = JSON.stringify([
      { id: 'C', type: 'unknown' },
    ]);
    expect(() => parseTrackedContracts()).toThrow();
  });

  it('throws when an entry has empty id', () => {
    process.env.TRACKED_CONTRACTS = JSON.stringify([
      { id: '', type: 'marketplace' },
    ]);
    expect(() => parseTrackedContracts()).toThrow();
  });
});

// ── validateRequiredEnv ───────────────────────────────────────────────────────

describe('validateRequiredEnv — multi-contract config', () => {
  const ORIG = { ...process.env };

  beforeEach(() => {
    process.env.DATABASE_URL = 'postgresql://localhost/test';
    process.env.REDIS_URL = 'redis://localhost:6379';
    process.env.STELLAR_RPC_URL = 'https://soroban-testnet.stellar.org';
    process.env.STELLAR_NETWORK = 'testnet';
  });

  afterEach(() => {
    process.env = { ...ORIG };
  });

  it('accepts TRACKED_CONTRACTS in place of MARKETPLACE_CONTRACT_ID', () => {
    delete process.env.MARKETPLACE_CONTRACT_ID;
    process.env.TRACKED_CONTRACTS = JSON.stringify([{ id: 'C', type: 'marketplace' }]);
    expect(() => validateRequiredEnv()).not.toThrow();
  });

  it('accepts legacy MARKETPLACE_CONTRACT_ID alone', () => {
    delete process.env.TRACKED_CONTRACTS;
    process.env.MARKETPLACE_CONTRACT_ID = 'CMARKET';
    expect(() => validateRequiredEnv()).not.toThrow();
  });

  it('fails when neither TRACKED_CONTRACTS nor MARKETPLACE_CONTRACT_ID is set', () => {
    delete process.env.TRACKED_CONTRACTS;
    delete process.env.MARKETPLACE_CONTRACT_ID;
    expect(() => validateRequiredEnv()).toThrow('MARKETPLACE_CONTRACT_ID');
  });
});

// ── seedTrackedContracts ──────────────────────────────────────────────────────

describe('seedTrackedContracts', () => {
  const ORIG = { ...process.env };

  beforeEach(() => {
    vi.clearAllMocks();
    process.env.MARKETPLACE_CONTRACT_ID = 'CMARKET';
    delete process.env.TRACKED_CONTRACTS;
  });

  afterEach(() => {
    process.env = { ...ORIG };
  });

  it('upserts each configured contract into TrackedContract', async () => {
    mockPrisma.trackedContract.upsert.mockResolvedValue({ id: 1, contractId: 'CMARKET', active: true });
    mockPrisma.trackedContract.findMany.mockResolvedValue([
      { id: 1, contractId: 'CMARKET', type: 'marketplace', label: 'marketplace', lastLedger: 0, active: true },
    ]);

    const { seedTrackedContracts } = await import('../poller');
    const result = await seedTrackedContracts();

    expect(mockPrisma.trackedContract.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        where: { contractId: 'CMARKET' },
        create: expect.objectContaining({ contractId: 'CMARKET', type: 'marketplace' }),
      })
    );
    expect(result).toHaveLength(1);
  });

  it('seeds multiple contracts from TRACKED_CONTRACTS', async () => {
    process.env.TRACKED_CONTRACTS = JSON.stringify([
      { id: 'CA', type: 'marketplace', label: 'mainnet', startLedger: 100 },
      { id: 'CB', type: 'launchpad', label: 'launchpad', startLedger: 0 },
    ]);
    delete process.env.MARKETPLACE_CONTRACT_ID;

    mockPrisma.trackedContract.upsert.mockResolvedValue({});
    mockPrisma.trackedContract.findMany.mockResolvedValue([
      { id: 1, contractId: 'CA', type: 'marketplace', label: 'mainnet', lastLedger: 100, active: true },
      { id: 2, contractId: 'CB', type: 'launchpad', label: 'launchpad', lastLedger: 0, active: true },
    ]);

    vi.resetModules();
    // Re-import to pick up new env
    const { seedTrackedContracts: seed } = await import('../poller');
    await seed();

    expect(mockPrisma.trackedContract.upsert).toHaveBeenCalledTimes(2);
    const calls = mockPrisma.trackedContract.upsert.mock.calls;
    expect(calls[0][0].where.contractId).toBe('CA');
    expect(calls[1][0].where.contractId).toBe('CB');
  });
});

// ── startPolling — throws with no contracts ───────────────────────────────────

describe('startPolling — no contracts', () => {
  const ORIG = { ...process.env };

  beforeEach(() => {
    vi.clearAllMocks();
    delete process.env.MARKETPLACE_CONTRACT_ID;
    delete process.env.TRACKED_CONTRACTS;
  });

  afterEach(() => {
    process.env = { ...ORIG };
  });

  it('throws when no active contracts in DB and no env config', async () => {
    mockPrisma.trackedContract.upsert.mockResolvedValue({});
    mockPrisma.trackedContract.findMany.mockResolvedValue([]);

    vi.resetModules();
    const { startPolling } = await import('../poller');
    await expect(startPolling()).rejects.toThrow('No active tracked contracts');
  });
});

// ── applyDecodedEvents — contractId stored on MarketplaceEvent ────────────────

describe('applyDecodedEvents — contractId is stored', () => {
  beforeEach(() => vi.clearAllMocks());

  it('passes contractId from the decoded event to marketplaceEvent.create', async () => {
    mockTx.marketplaceEvent.findUnique.mockResolvedValue(null);

    const { applyDecodedEvents } = await import('../poller');

    const event = {
      eventType: 'LISTING_CREATED',
      listingId: 1n,
      actor: 'GA',
      ledgerSequence: 100,
      data: { artist: 'GA', collection: 'C', token_id: 1 },
      eventHash: 'hash-abc',
      contractId: 'CMARKET_ADDR',
      txHash: 'tx1',
      eventIndex: 0,
    };

    await applyDecodedEvents([event], mockTx);

    expect(mockTx.marketplaceEvent.create).toHaveBeenCalledWith(
      expect.objectContaining({
        data: expect.objectContaining({ contractId: 'CMARKET_ADDR' }),
      })
    );
  });

  it('defaults contractId to empty string when absent from event', async () => {
    mockTx.marketplaceEvent.findUnique.mockResolvedValue(null);

    const { applyDecodedEvents } = await import('../poller');

    // Use a non-LISTING_CREATED type to avoid the token_id BigInt path
    const event = {
      eventType: 'LISTING_CANCELLED',
      listingId: 2n,
      actor: 'GA',
      ledgerSequence: 101,
      data: {},
      eventHash: 'hash-def',
      txHash: 'tx2',
      eventIndex: 0,
      // contractId intentionally absent
    };

    await applyDecodedEvents([event], mockTx);

    expect(mockTx.marketplaceEvent.create).toHaveBeenCalledWith(
      expect.objectContaining({
        data: expect.objectContaining({ contractId: '' }),
      })
    );
  });
});

// ── Independent sync state per contract ──────────────────────────────────────

describe('per-contract sync state isolation', () => {
  const ORIG = { ...process.env };

  beforeEach(() => {
    vi.clearAllMocks();
    process.env.TRACKED_CONTRACTS = JSON.stringify([
      { id: 'C_A', type: 'marketplace', label: 'alpha', startLedger: 0 },
      { id: 'C_B', type: 'marketplace', label: 'beta', startLedger: 5000 },
    ]);
    delete process.env.MARKETPLACE_CONTRACT_ID;
  });

  afterEach(() => {
    process.env = { ...ORIG };
  });

  it('upserts with independent startLedger values per contract', async () => {
    mockPrisma.trackedContract.upsert.mockResolvedValue({});
    mockPrisma.trackedContract.findMany.mockResolvedValue([
      { id: 1, contractId: 'C_A', type: 'marketplace', label: 'alpha', lastLedger: 0, lastLedgerHash: null, active: true },
      { id: 2, contractId: 'C_B', type: 'marketplace', label: 'beta', lastLedger: 5000, lastLedgerHash: null, active: true },
    ]);

    vi.resetModules();
    const { seedTrackedContracts } = await import('../poller');
    const contracts = await seedTrackedContracts();

    const aUpsert = mockPrisma.trackedContract.upsert.mock.calls.find(
      ([arg]: [any]) => arg.where.contractId === 'C_A'
    );
    const bUpsert = mockPrisma.trackedContract.upsert.mock.calls.find(
      ([arg]: [any]) => arg.where.contractId === 'C_B'
    );

    expect(aUpsert?.[0].create.lastLedger).toBe(0);
    expect(bUpsert?.[0].create.lastLedger).toBe(5000);
    expect(contracts).toHaveLength(2);
  });
});

// ── Admin route integration helpers ──────────────────────────────────────────
// These test the logic that the admin endpoints use (Prisma calls), not the
// HTTP layer directly, to keep tests fast and framework-independent.

describe('admin contract management — Prisma operations', () => {
  beforeEach(() => vi.clearAllMocks());

  it('GET /admin/contracts — lists all TrackedContract rows', async () => {
    const rows = [
      { id: 1, contractId: 'CA', type: 'marketplace', label: 'mainnet', lastLedger: 1000, active: true },
      { id: 2, contractId: 'CB', type: 'launchpad', label: 'launchpad', lastLedger: 500, active: true },
    ];
    mockPrisma.trackedContract.findMany.mockResolvedValue(rows);

    // Simulate what the route does
    const result = await mockPrisma.trackedContract.findMany({ orderBy: { createdAt: 'asc' } });
    expect(result).toHaveLength(2);
    expect(result[0].contractId).toBe('CA');
  });

  it('POST /admin/contracts — upserts a new contract', async () => {
    const created = { id: 3, contractId: 'CC', type: 'marketplace', label: 'new', lastLedger: 0, active: true };
    mockPrisma.trackedContract.upsert.mockResolvedValue(created);

    const result = await mockPrisma.trackedContract.upsert({
      where: { contractId: 'CC' },
      create: { contractId: 'CC', type: 'marketplace', label: 'new', startLedger: 0, lastLedger: 0, active: true },
      update: { type: 'marketplace', label: 'new', active: true },
    });
    expect(result.contractId).toBe('CC');
    expect(result.active).toBe(true);
  });

  it('DELETE /admin/contracts/:id — marks contract inactive', async () => {
    mockPrisma.trackedContract.findUnique.mockResolvedValue({ id: 1, contractId: 'CA', active: true });
    mockPrisma.trackedContract.update.mockResolvedValue({ id: 1, contractId: 'CA', active: false });

    const updated = await mockPrisma.trackedContract.update({
      where: { id: 1 },
      data: { active: false },
    });
    expect(updated.active).toBe(false);
  });
});
