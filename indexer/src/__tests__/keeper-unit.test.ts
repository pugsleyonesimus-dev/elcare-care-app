/**
 * keeper-unit.test.ts
 *
 * Unit tests for the keeper subsystem covering:
 *   - error-classifier: error classification matrix (permanent vs transient)
 *   - tx-pipeline: fee escalation math (escalateFee)
 *   - idempotency: state-machine transition helpers (mocked Prisma)
 *   - config: loadKeeperConfig validation for every new env var
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ── Stub out metrics so we don't need prom-client in unit tests ───────────────
vi.mock('../metrics.js', () => ({
  rpcRetryExhaustedCounter:      { inc: vi.fn() },
  keeperActionsTotal:             { inc: vi.fn() },
  keeperFeesSpentStroops:         { inc: vi.fn() },
  keeperBudgetExhaustedTotal:     { inc: vi.fn() },
  keeperBudgetExhaustedGauge:     { set: vi.fn() },
  keeperSimulationFailuresTotal:  { inc: vi.fn() },
  keeperCycleDurationSeconds:     { startTimer: vi.fn(() => vi.fn()) },
  keeperCandidatesDiscovered:     { set: vi.fn() },
  keeperFeeBumpsTotal:            { inc: vi.fn() },
  decodeErrorsCounter:            { inc: vi.fn() },
  latestLedgerProcessedGauge:     { set: vi.fn() },
  networkLatestLedgerGauge:       { set: vi.fn() },
  syncLatencyGauge:               { set: vi.fn() },
}));

// ── Stub Prisma so idempotency tests don't need a real DB ────────────────────
const mockPrisma = {
  keeperAction: {
    findUnique:  vi.fn(),
    findMany:    vi.fn(),
    upsert:      vi.fn(),
    update:      vi.fn(),
    groupBy:     vi.fn(),
  },
};

vi.mock('../db.js', () => ({ default: mockPrisma }));

// ─────────────────────────────────────────────────────────────────────────────
// 1. Error classifier
// ─────────────────────────────────────────────────────────────────────────────

import {
  classifyError,
  extractContractErrorCode,
  isFeeError,
  isSeqError,
} from '../keeper/error-classifier.js';

describe('extractContractErrorCode', () => {
  it('extracts numeric code from "Error(Contract, #28)"', () => {
    expect(extractContractErrorCode('Error(Contract, #28)')).toBe(28);
  });

  it('extracts code from lowercase / spaced variant', () => {
    expect(extractContractErrorCode('error ( contract , #14 )')).toBe(14);
  });

  it('returns null when no code is present', () => {
    expect(extractContractErrorCode('timeout: connection refused')).toBeNull();
  });

  it('returns null for a plain number with no contract error pattern', () => {
    expect(extractContractErrorCode('status 503')).toBeNull();
  });
});

describe('classifyError — permanent cases', () => {
  const permanentCases: Array<[string, unknown]> = [
    ['ListingNotExpired contract code 28',    new Error('simulate failed Error(Contract, #28)')],
    ['AuctionNotEnded contract code 29',      new Error('Error(Contract, #29)')],
    ['AuctionAlreadyFinalized code 14',       new Error('Error(Contract, #14)')],
    ['ListingNotFound code 3',                new Error('Error(Contract, #3)')],
    ['AuctionNotFound code 9',                new Error('Error(Contract, #9)')],
    ['OfferNotFound code 16',                 new Error('Error(Contract, #16)')],
    ['OfferNotPending code 18',               new Error('Error(Contract, #18)')],
    ['InvalidOfferState code 33',             new Error('Error(Contract, #33)')],
    ['ListingNotExpired by pattern',          new Error('ListingNotExpired')],
    ['AuctionNotEnded by pattern',            new Error('AuctionNotEnded')],
    ['AuctionAlreadyFinalized by pattern',    new Error('AuctionAlreadyFinalized')],
    ['ContractPaused by pattern',             new Error('ContractPaused: marketplace is paused')],
    ['generic contract error string',         new Error('simulate returned contract error code: 28')],
    ['non-Error permanent string',            'ListingNotExpired: expiry not reached'],
  ];

  it.each(permanentCases)('%s → permanent', (_label, err) => {
    expect(classifyError(err)).toBe('permanent');
  });
});

describe('classifyError — transient cases', () => {
  const transientCases: Array<[string, unknown]> = [
    ['timeout',                new Error('request timeout after 30s')],
    ['ECONNREFUSED',           new Error('connect ECONNREFUSED 127.0.0.1:8000')],
    ['ENOTFOUND',              new Error('getaddrinfo ENOTFOUND soroban-rpc.example.com')],
    ['rate limit 429',         new Error('too many requests — rate limited')],
    ['503 gateway',            new Error('503 service temporarily unavailable')],
    ['502 bad gateway',        new Error('502 Bad Gateway')],
    ['insufficient fee',       new Error('insufficient resource fee: need 5000 got 100')],
    ['tx_bad_seq',             new Error('tx_bad_seq: sequence number mismatch')],
    ['tx_insufficient_fee',    new Error('tx_insufficient_fee')],
    ['socket hang up',         new Error('socket hang up')],
    ['unknown error',          new Error('some completely unknown transient issue')],
    ['non-Error unknown',      'unexpected undefined behaviour'],
  ];

  it.each(transientCases)('%s → transient', (_label, err) => {
    expect(classifyError(err)).toBe('transient');
  });
});

describe('isFeeError', () => {
  it('detects insufficient resource fee', () => {
    expect(isFeeError(new Error('insufficient resource fee: need 5000'))).toBe(true);
  });
  it('detects tx_insufficient_fee', () => {
    expect(isFeeError(new Error('tx_insufficient_fee'))).toBe(true);
  });
  it('returns false for an unrelated error', () => {
    expect(isFeeError(new Error('ListingNotExpired'))).toBe(false);
  });
});

describe('isSeqError', () => {
  it('detects tx_bad_seq', () => {
    expect(isSeqError(new Error('tx_bad_seq: sequence number out of order'))).toBe(true);
  });
  it('returns false for a non-sequence error', () => {
    expect(isSeqError(new Error('timeout'))).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. Fee escalation math
// ─────────────────────────────────────────────────────────────────────────────

import { escalateFee } from '../keeper/tx-pipeline.js';

describe('escalateFee', () => {
  const BASE   = 10_000;    // 10 000 stroops base
  const MULT   = 1.5;
  const MAX    = 1_000_000;

  it('bump attempt 1 = base × multiplier¹', () => {
    // ceil(10000 × 1.5^1) = ceil(15000) = 15000
    expect(escalateFee(BASE, 1, MULT, MAX)).toBe(15_000);
  });

  it('bump attempt 2 = base × multiplier²', () => {
    // ceil(10000 × 1.5^2) = ceil(22500) = 22500
    expect(escalateFee(BASE, 2, MULT, MAX)).toBe(22_500);
  });

  it('bump attempt 3 = base × multiplier³', () => {
    // ceil(10000 × 1.5^3) = ceil(33750) = 33750
    expect(escalateFee(BASE, 3, MULT, MAX)).toBe(33_750);
  });

  it('caps at maxFeeStroops', () => {
    expect(escalateFee(BASE, 20, MULT, MAX)).toBe(MAX);
  });

  it('does not exceed the cap even with a very high multiplier', () => {
    expect(escalateFee(500_000, 5, 10, 1_000_000)).toBe(1_000_000);
  });

  it('works with multiplier of exactly 1.01 (minimum useful escalation)', () => {
    // ceil(10000 × 1.01) = ceil(10100) = 10100
    expect(escalateFee(BASE, 1, 1.01, MAX)).toBe(10_100);
  });

  it('returns ceil (never floors a fractional stroop)', () => {
    // ceil(10000 × 1.5^1) is exactly 15000 — but let's use a case that fractions
    // ceil(7 × 1.5^1) = ceil(10.5) = 11
    expect(escalateFee(7, 1, 1.5, 1_000_000)).toBe(11);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. Idempotency state-machine transitions
// ─────────────────────────────────────────────────────────────────────────────

import {
  findAction,
  createOrSkipAction,
  markSubmitted,
  markSucceeded,
  markFailed,
  markSkipped,
  resetForRetry,
  getActionSummary,
} from '../keeper/idempotency.js';

const CANDIDATE = { targetType: 'ExpireListing' as const, targetId: 42n };

describe('findAction', () => {
  beforeEach(() => vi.clearAllMocks());

  it('calls prisma.keeperAction.findUnique with correct where clause', async () => {
    mockPrisma.keeperAction.findUnique.mockResolvedValue(null);
    await findAction(CANDIDATE);
    expect(mockPrisma.keeperAction.findUnique).toHaveBeenCalledWith({
      where: {
        targetType_targetId: {
          targetType: 'ExpireListing',
          targetId: 42n,
        },
      },
    });
  });

  it('returns null when no action exists', async () => {
    mockPrisma.keeperAction.findUnique.mockResolvedValue(null);
    expect(await findAction(CANDIDATE)).toBeNull();
  });
});

describe('createOrSkipAction', () => {
  beforeEach(() => vi.clearAllMocks());

  it('upserts with Pending status and 0 attempts', async () => {
    mockPrisma.keeperAction.upsert.mockResolvedValue({
      id: 1, status: 'Pending', attempts: 0,
    });
    await createOrSkipAction(CANDIDATE);
    expect(mockPrisma.keeperAction.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        create: expect.objectContaining({ status: 'Pending', attempts: 0 }),
        update: {},
      }),
    );
  });
});

describe('markSubmitted', () => {
  beforeEach(() => vi.clearAllMocks());

  it('sets status Submitted, records txHash, increments attempts', async () => {
    mockPrisma.keeperAction.update.mockResolvedValue({});
    await markSubmitted(1, 'abc123');
    expect(mockPrisma.keeperAction.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: expect.objectContaining({
        status: 'Submitted',
        txHash: 'abc123',
        attempts: { increment: 1 },
        lastError: null,
      }),
    });
  });
});

describe('markSucceeded', () => {
  beforeEach(() => vi.clearAllMocks());

  it('sets status Succeeded with txHash and feePaid', async () => {
    mockPrisma.keeperAction.update.mockResolvedValue({});
    await markSucceeded(1, 'txhash', 500n);
    expect(mockPrisma.keeperAction.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: expect.objectContaining({
        status: 'Succeeded',
        txHash: 'txhash',
        feePaid: 500n,
        lastError: null,
      }),
    });
  });
});

describe('markFailed', () => {
  beforeEach(() => vi.clearAllMocks());

  it('sets status Failed and records the error message', async () => {
    mockPrisma.keeperAction.update.mockResolvedValue({});
    await markFailed(1, 'rpc timeout');
    expect(mockPrisma.keeperAction.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: expect.objectContaining({ status: 'Failed', lastError: 'rpc timeout' }),
    });
  });

  it('truncates error messages longer than 4096 characters', async () => {
    mockPrisma.keeperAction.update.mockResolvedValue({});
    const longError = 'x'.repeat(5000);
    await markFailed(1, longError);
    const call = mockPrisma.keeperAction.update.mock.calls[0][0];
    expect(call.data.lastError.length).toBe(4096);
  });
});

describe('markSkipped', () => {
  beforeEach(() => vi.clearAllMocks());

  it('sets status Skipped and records the reason', async () => {
    mockPrisma.keeperAction.update.mockResolvedValue({});
    await markSkipped(1, 'ListingNotExpired');
    expect(mockPrisma.keeperAction.update).toHaveBeenCalledWith({
      where: { id: 1 },
      data: expect.objectContaining({ status: 'Skipped', lastError: 'ListingNotExpired' }),
    });
  });
});

describe('resetForRetry', () => {
  beforeEach(() => vi.clearAllMocks());

  it('only targets rows in Failed status', async () => {
    mockPrisma.keeperAction.update.mockResolvedValue({});
    await resetForRetry(7);
    expect(mockPrisma.keeperAction.update).toHaveBeenCalledWith({
      where: { id: 7, status: 'Failed' },
      data: expect.objectContaining({ status: 'Pending', txHash: null, lastError: null }),
    });
  });
});

describe('getActionSummary', () => {
  beforeEach(() => vi.clearAllMocks());

  it('returns zeroes for all statuses when the table is empty', async () => {
    mockPrisma.keeperAction.groupBy.mockResolvedValue([]);
    const summary = await getActionSummary();
    expect(summary).toEqual({
      Pending: 0, Submitted: 0, Succeeded: 0, Failed: 0, Skipped: 0,
    });
  });

  it('maps groupBy results to the correct status keys', async () => {
    mockPrisma.keeperAction.groupBy.mockResolvedValue([
      { status: 'Succeeded', _count: { id: 10 } },
      { status: 'Failed',    _count: { id: 3  } },
      { status: 'Skipped',   _count: { id: 1  } },
    ]);
    const summary = await getActionSummary();
    expect(summary.Succeeded).toBe(10);
    expect(summary.Failed).toBe(3);
    expect(summary.Skipped).toBe(1);
    expect(summary.Pending).toBe(0);
    expect(summary.Submitted).toBe(0);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. loadKeeperConfig validation
// ─────────────────────────────────────────────────────────────────────────────

import { loadKeeperConfig } from '../config.js';

describe('loadKeeperConfig', () => {
  const SAVED = { ...process.env };

  // Minimal valid state: all optional vars absent → defaults used
  beforeEach(() => {
    // Strip all KEEPER_* vars so each test starts clean
    for (const key of Object.keys(process.env)) {
      if (key.startsWith('KEEPER_')) delete process.env[key];
    }
  });

  afterEach(() => {
    process.env = { ...SAVED };
  });

  it('returns safe defaults when no KEEPER_ vars are set', () => {
    const cfg = loadKeeperConfig();
    expect(cfg.KEEPER_ENABLED).toBe(false);
    expect(cfg.KEEPER_DRY_RUN).toBe(true);     // safe default: dry-run on
    expect(cfg.KEEPER_INTERVAL_MS).toBe(60_000);
    expect(cfg.KEEPER_MAX_ACTIONS_PER_CYCLE).toBe(20);
    expect(cfg.KEEPER_MAX_FEE_STROOPS).toBe(1_000_000);
    expect(cfg.KEEPER_DAILY_FEE_BUDGET_STROOPS).toBe(10_000_000);
    expect(cfg.KEEPER_FEE_BUMP_MULTIPLIER).toBe(1.5);
    expect(cfg.KEEPER_FEE_BUMP_MAX_RETRIES).toBe(3);
    expect(cfg.KEEPER_SUBMIT_TIMEOUT_MS).toBe(30_000);
    expect(cfg.KEEPER_POLL_TIMEOUT_MS).toBe(60_000);
    expect(cfg.KEEPER_POLL_INTERVAL_MS).toBe(2_000);
  });

  it('parses KEEPER_ENABLED=true', () => {
    process.env.KEEPER_ENABLED = 'true';
    process.env.KEEPER_SECRET  = 'SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA3';
    expect(loadKeeperConfig().KEEPER_ENABLED).toBe(true);
  });

  it('treats KEEPER_DRY_RUN absent as true (safe default)', () => {
    expect(loadKeeperConfig().KEEPER_DRY_RUN).toBe(true);
  });

  it('treats KEEPER_DRY_RUN=false as false', () => {
    process.env.KEEPER_DRY_RUN = 'false';
    expect(loadKeeperConfig().KEEPER_DRY_RUN).toBe(false);
  });

  it('treats KEEPER_DRY_RUN=anything-else as true', () => {
    process.env.KEEPER_DRY_RUN = 'yes';
    expect(loadKeeperConfig().KEEPER_DRY_RUN).toBe(true);
  });

  it('parses KEEPER_INTERVAL_MS from env', () => {
    process.env.KEEPER_INTERVAL_MS = '30000';
    expect(loadKeeperConfig().KEEPER_INTERVAL_MS).toBe(30_000);
  });

  it('throws when KEEPER_INTERVAL_MS is not a positive integer', () => {
    process.env.KEEPER_INTERVAL_MS = '0';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_INTERVAL_MS is negative', () => {
    process.env.KEEPER_INTERVAL_MS = '-1000';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_MAX_FEE_STROOPS is zero', () => {
    process.env.KEEPER_MAX_FEE_STROOPS = '0';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_FEE_BUMP_MULTIPLIER is below 1.01', () => {
    process.env.KEEPER_FEE_BUMP_MULTIPLIER = '0.5';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_FEE_BUMP_MULTIPLIER exceeds 10', () => {
    process.env.KEEPER_FEE_BUMP_MULTIPLIER = '11';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_FEE_BUMP_MAX_RETRIES is negative', () => {
    process.env.KEEPER_FEE_BUMP_MAX_RETRIES = '-1';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_FEE_BUMP_MAX_RETRIES exceeds 10', () => {
    process.env.KEEPER_FEE_BUMP_MAX_RETRIES = '11';
    expect(() => loadKeeperConfig()).toThrow();
  });

  it('throws when KEEPER_SECRET does not start with S', () => {
    process.env.KEEPER_SECRET = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA3';
    expect(() => loadKeeperConfig()).toThrow('KEEPER_SECRET');
  });

  it('throws when KEEPER_ENABLED=true but KEEPER_SECRET is absent', () => {
    process.env.KEEPER_ENABLED = 'true';
    expect(() => loadKeeperConfig()).toThrow('KEEPER_SECRET');
  });

  it('does NOT throw when KEEPER_ENABLED=true and KEEPER_SECRET is a valid S-key', () => {
    process.env.KEEPER_ENABLED = 'true';
    process.env.KEEPER_SECRET  = 'SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA3';
    expect(() => loadKeeperConfig()).not.toThrow();
  });

  it('parses all numeric fields correctly when set', () => {
    process.env.KEEPER_MAX_ACTIONS_PER_CYCLE     = '5';
    process.env.KEEPER_MAX_FEE_STROOPS           = '500000';
    process.env.KEEPER_DAILY_FEE_BUDGET_STROOPS  = '2000000';
    process.env.KEEPER_FEE_BUMP_MULTIPLIER       = '2';
    process.env.KEEPER_FEE_BUMP_MAX_RETRIES      = '2';
    process.env.KEEPER_SUBMIT_TIMEOUT_MS         = '15000';
    process.env.KEEPER_POLL_TIMEOUT_MS           = '45000';
    process.env.KEEPER_POLL_INTERVAL_MS          = '1000';

    const cfg = loadKeeperConfig();
    expect(cfg.KEEPER_MAX_ACTIONS_PER_CYCLE).toBe(5);
    expect(cfg.KEEPER_MAX_FEE_STROOPS).toBe(500_000);
    expect(cfg.KEEPER_DAILY_FEE_BUDGET_STROOPS).toBe(2_000_000);
    expect(cfg.KEEPER_FEE_BUMP_MULTIPLIER).toBe(2);
    expect(cfg.KEEPER_FEE_BUMP_MAX_RETRIES).toBe(2);
    expect(cfg.KEEPER_SUBMIT_TIMEOUT_MS).toBe(15_000);
    expect(cfg.KEEPER_POLL_TIMEOUT_MS).toBe(45_000);
    expect(cfg.KEEPER_POLL_INTERVAL_MS).toBe(1_000);
  });
});
