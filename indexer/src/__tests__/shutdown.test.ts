import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('dotenv', () => ({ default: { config: vi.fn() } }));

// ── Mock Prisma ───────────────────────────────────────────────────────────────
vi.mock('../db', () => ({
  default: {
    $disconnect: vi.fn().mockResolvedValue(undefined),
    syncState: { upsert: vi.fn(), update: vi.fn() },
    $transaction: vi.fn(),
  },
}));

vi.mock('../metrics.js', () => ({
  rpcRetryExhaustedCounter: { inc: vi.fn() },
  decodeErrorsCounter: { inc: vi.fn() },
  latestLedgerProcessedGauge: { set: vi.fn() },
  networkLatestLedgerGauge: { set: vi.fn() },
  syncLatencyGauge: { set: vi.fn() },
}));

vi.mock('../redis.js', () => ({
  default: {
    disconnect: vi.fn().mockResolvedValue(undefined),
    on: vi.fn(),
    connect: vi.fn(),
  },
}));

vi.mock('../retry.js', () => ({
  withRetry: vi.fn((fn: () => Promise<unknown>) => fn()),
}));

vi.mock('@stellar/stellar-sdk', () => ({
  rpc: {
    Server: class {
      getLatestLedger() { return Promise.resolve({ sequence: 100 }); }
      getLedgers() { return Promise.resolve({ ledgers: [{ hash: 'h', sequence: 100 }] }); }
      getEvents() { return Promise.resolve({ events: [], paginationToken: null }); }
    },
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
}));

vi.mock('../event-sync.js', () => ({
  collectMarketplaceEvents: vi.fn().mockResolvedValue([]),
  MAX_LEDGER_WINDOW: 17_000,
}));

vi.mock('../api/routes.js', () => ({
  emitSSEEvent: vi.fn(),
  closeSSEClients: vi.fn(),
}));

describe('gracefulShutdown — registered hooks', () => {
  let exitSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.resetModules();
    // Prevent actual process exit
    exitSpy = vi.spyOn(process, 'exit').mockImplementation((() => {}) as any);
  });

  afterEach(() => {
    exitSpy.mockRestore();
    vi.clearAllMocks();
  });

  it('calls a hook registered via registerShutdownHook during graceful shutdown', async () => {
    const { registerShutdownHook, gracefulShutdown } = await import('../poller.js');

    const hook = vi.fn().mockResolvedValue(undefined);
    registerShutdownHook(hook);

    await gracefulShutdown();

    expect(hook).toHaveBeenCalledOnce();
  });

  it('calls process.exit(0) after cleanup completes', async () => {
    const { gracefulShutdown } = await import('../poller.js');
    await gracefulShutdown();
    expect(exitSpy).toHaveBeenCalledWith(0);
  });

  it('is idempotent — calling gracefulShutdown twice runs cleanup only once', async () => {
    const { registerShutdownHook, gracefulShutdown } = await import('../poller.js');
    const hook = vi.fn().mockResolvedValue(undefined);
    registerShutdownHook(hook);

    await gracefulShutdown();
    await gracefulShutdown();

    expect(hook).toHaveBeenCalledOnce();
  });
});

describe('closeSSEClients', () => {
  it('is exported and callable without throwing (empty registry at startup)', async () => {
    const { closeSSEClients } = await import('../api/routes.js');
    expect(typeof closeSSEClients).toBe('function');
    expect(() => closeSSEClients()).not.toThrow();
  });
});
