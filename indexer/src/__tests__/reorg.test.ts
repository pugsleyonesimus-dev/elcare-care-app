import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockPrisma: any = vi.hoisted(() => {
  const mPrisma: any = {
    marketplaceEvent: {
      deleteMany: vi.fn().mockResolvedValue({ count: 1 }),
    },
    listing: {
      deleteMany: vi.fn().mockResolvedValue({ count: 1 }),
      updateMany: vi.fn().mockResolvedValue({ count: 1 }),
    },
    syncState: {
      update: vi.fn().mockResolvedValue({ id: 1, lastLedger: 100, lastLedgerHash: null }),
    },
    collection: {
      deleteMany: vi.fn().mockResolvedValue({ count: 1 }),
    },
  };
  mPrisma.$transaction = vi.fn((callback: (tx: typeof mPrisma) => Promise<void>) => callback(mPrisma));
  return mPrisma;
});

vi.mock('../db', () => ({ default: mockPrisma }));

vi.mock('dotenv', () => ({ default: { config: vi.fn() } }));

vi.mock('../metrics.js', () => ({
  latestLedgerProcessedGauge: { set: vi.fn() },
  networkLatestLedgerGauge: { set: vi.fn() },
  syncLatencyGauge: { set: vi.fn() },
}));

vi.mock('@stellar/stellar-sdk', () => ({
  rpc: {
    Server: class {
      getLedgers = vi.fn();
      getLatestLedger = vi.fn();
      getEvents = vi.fn();
    },
  },
}));

import { revertLedgers, findReorgSafePoint, validateHashContinuity, MAX_REORG_DEPTH } from '../poller';

describe('Chain Re-organization Rollback', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPrisma.$transaction.mockImplementation(
      (callback: (tx: typeof mockPrisma) => Promise<void>) => callback(mockPrisma)
    );
  });

  it('deletes events and listings created after the safe ledger', async () => {
    await revertLedgers(100);

    expect(mockPrisma.$transaction).toHaveBeenCalledOnce();

    expect(mockPrisma.marketplaceEvent.deleteMany).toHaveBeenCalledWith({
      where: { ledgerSequence: { gt: 100 } },
    });

    expect(mockPrisma.listing.deleteMany).toHaveBeenCalledWith({
      where: { createdAtLedger: { gt: 100 } },
    });
  });

  it('reverts listing status for listings updated after the safe ledger', async () => {
    await revertLedgers(100);

    expect(mockPrisma.listing.updateMany).toHaveBeenCalledWith({
      where: { updatedAtLedger: { gt: 100 } },
      data: { status: 'Active', updatedAtLedger: 100 },
    });
  });

  it('deletes collections deployed after the safe ledger', async () => {
    await revertLedgers(100);

    expect(mockPrisma.collection.deleteMany).toHaveBeenCalledWith({
      where: { deployedAtLedger: { gt: 100 } },
    });
  });

  it('resets SyncState to the safe ledger with null hash', async () => {
    await revertLedgers(100);

    expect(mockPrisma.syncState.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: { lastLedger: 100, lastLedgerHash: null },
    });
  });

  it('wraps all operations in a single transaction', async () => {
    await revertLedgers(50);

    expect(mockPrisma.$transaction).toHaveBeenCalledOnce();
    // All DB calls happen inside the transaction callback
    expect(mockPrisma.marketplaceEvent.deleteMany).toHaveBeenCalledOnce();
    expect(mockPrisma.listing.deleteMany).toHaveBeenCalledOnce();
    expect(mockPrisma.listing.updateMany).toHaveBeenCalledOnce();
    expect(mockPrisma.collection.deleteMany).toHaveBeenCalledOnce();
    expect(mockPrisma.syncState.update).toHaveBeenCalledOnce();
  });
});

// ── findReorgSafePoint — bounded walk-back (#50) ──────────────────────────────

describe('findReorgSafePoint', () => {
  it('returns the immediate predecessor when it is accessible', async () => {
    const mockServer = {
      getLedgers: vi.fn().mockResolvedValue({ ledgers: [{ hash: 'h99', sequence: 99 }] }),
    };
    const safe = await findReorgSafePoint(100, mockServer as any);
    expect(safe).toBe(99);
    expect(mockServer.getLedgers).toHaveBeenCalledTimes(1);
  });

  it('walks back multiple depths when predecessors are inaccessible', async () => {
    const mockServer = {
      getLedgers: vi.fn()
        .mockResolvedValueOnce({ ledgers: [] })           // depth 1: ledger 99 — empty
        .mockResolvedValueOnce({ ledgers: [{ hash: 'h98', sequence: 98 }] }), // depth 2: ledger 98
    };
    const safe = await findReorgSafePoint(100, mockServer as any);
    expect(safe).toBe(98);
    expect(mockServer.getLedgers).toHaveBeenCalledTimes(2);
  });

  it('walks back multiple depths when predecessors throw', async () => {
    const mockServer = {
      getLedgers: vi.fn()
        .mockRejectedValueOnce(new Error('unavailable'))   // depth 1: ledger 99
        .mockResolvedValueOnce({ ledgers: [{ hash: 'h98', sequence: 98 }] }), // depth 2: ledger 98
    };
    const safe = await findReorgSafePoint(100, mockServer as any);
    expect(safe).toBe(98);
  });

  it('returns 0 when divergedAt - depth reaches zero', async () => {
    const mockServer = {
      getLedgers: vi.fn().mockResolvedValue({ ledgers: [] }),
    };
    const safe = await findReorgSafePoint(2, mockServer as any);
    expect(safe).toBe(0);
  });

  it(`falls back to divergedAt - MAX_REORG_DEPTH when no safe ledger found within bound`, async () => {
    const mockServer = {
      getLedgers: vi.fn().mockResolvedValue({ ledgers: [] }),
    };
    const divergedAt = MAX_REORG_DEPTH + 50;
    const safe = await findReorgSafePoint(divergedAt, mockServer as any);
    expect(safe).toBe(divergedAt - MAX_REORG_DEPTH);
    expect(mockServer.getLedgers).toHaveBeenCalledTimes(MAX_REORG_DEPTH);
  });
});

// ── validateHashContinuity with bounded walk-back (#50) ──────────────────────

describe('validateHashContinuity — re-org simulation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockPrisma.$transaction.mockImplementation(
      (callback: (tx: typeof mockPrisma) => Promise<void>) => callback(mockPrisma)
    );
  });

  it('triggers bounded walk-back and reverts to safe ledger on hash mismatch', async () => {
    const mockServer = {
      getLedgers: vi.fn()
        // First call: check lastLedger 100 — hash mismatch (re-org)
        .mockResolvedValueOnce({ ledgers: [{ hash: 'fork_hash', sequence: 100 }] })
        // Walk-back depth 1: ledger 99 — accessible
        .mockResolvedValueOnce({ ledgers: [{ hash: 'safe_hash', sequence: 99 }] }),
    };

    const result = await validateHashContinuity(
      { lastLedger: 100, lastLedgerHash: 'canonical_hash' },
      mockServer as any
    );

    expect(result).toBe(false);
    // Should have reverted to ledger 99 (first safe ledger found)
    expect(mockPrisma.syncState.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: { lastLedger: 99, lastLedgerHash: null },
    });
  });

  it('walks back two depths when the immediate predecessor is inaccessible', async () => {
    const mockServer = {
      getLedgers: vi.fn()
        .mockResolvedValueOnce({ ledgers: [{ hash: 'fork_hash', sequence: 100 }] }) // mismatch at 100
        .mockResolvedValueOnce({ ledgers: [] })                                      // 99 inaccessible
        .mockResolvedValueOnce({ ledgers: [{ hash: 'safe_hash', sequence: 98 }] }), // 98 accessible
    };

    const result = await validateHashContinuity(
      { lastLedger: 100, lastLedgerHash: 'canonical_hash' },
      mockServer as any
    );

    expect(result).toBe(false);
    expect(mockPrisma.syncState.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: { lastLedger: 98, lastLedgerHash: null },
    });
  });

  it('returns true and does not revert when hashes match', async () => {
    const mockServer = {
      getLedgers: vi.fn().mockResolvedValue({
        ledgers: [{ hash: 'matching_hash', sequence: 100 }],
      }),
    };

    const result = await validateHashContinuity(
      { lastLedger: 100, lastLedgerHash: 'matching_hash' },
      mockServer as any
    );

    expect(result).toBe(true);
    expect(mockPrisma.$transaction).not.toHaveBeenCalled();
  });
});
