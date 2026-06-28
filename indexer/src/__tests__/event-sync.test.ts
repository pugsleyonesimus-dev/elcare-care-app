import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDecodeErrorsCounter = vi.hoisted(() => ({ inc: vi.fn() }));
const mockRpcRetryExhaustedCounter = vi.hoisted(() => ({ inc: vi.fn() }));

vi.mock('../metrics.js', () => ({
  decodeErrorsCounter: mockDecodeErrorsCounter,
  rpcRetryExhaustedCounter: mockRpcRetryExhaustedCounter,
  latestLedgerProcessedGauge: { set: vi.fn() },
  networkLatestLedgerGauge: { set: vi.fn() },
  syncLatencyGauge: { set: vi.fn() },
}));

vi.mock('../parser.js', () => ({
  parseMarketplaceEvent: vi.fn((topics: string[], _valueXdr: string, ledger: number) => ({
    eventType: topics[0],
    listingId: BigInt(ledger),
    actor: 'GTEST',
    ledgerSequence: ledger,
    data: { ledger },
  })),
}));

vi.mock('../retry.js', () => ({
  withRetry: vi.fn((fn: () => Promise<unknown>) => fn()),
}));

import { collectMarketplaceEvents, MAX_LEDGER_WINDOW, EVENT_PAGE_LIMIT } from '../event-sync';

describe('collectMarketplaceEvents', () => {
  it('follows pagination tokens until the page is exhausted', async () => {
    const getEvents = vi.fn()
      .mockResolvedValueOnce({
        events: [
          { topic: ['page-1'], value: 'value-1', ledger: 1 },
          { topic: ['page-1'], value: 'value-2', ledger: 2 },
        ],
        paginationToken: 'page-2',
      })
      .mockResolvedValueOnce({
        events: [
          { topic: ['page-2'], value: 'value-3', ledger: 3 },
        ],
        paginationToken: null,
      });

    const server = { getEvents } as any;
    const events = await collectMarketplaceEvents(server, ['C1'], 1, 10);

    expect(events).toHaveLength(3);
    expect(getEvents).toHaveBeenNthCalledWith(1, expect.objectContaining({
      startLedger: 1,
      endLedger: 10,
      limit: EVENT_PAGE_LIMIT,
    }));
    expect(getEvents).toHaveBeenNthCalledWith(2, expect.objectContaining({
      startLedger: 1,
      endLedger: 10,
      limit: EVENT_PAGE_LIMIT, cursor: 'page-2',
    }));
  });

  it('collects all events across three pages — old single-page code would lose pages 2 and 3', async () => {
    const getEvents = vi.fn()
      .mockResolvedValueOnce({ events: [{ topic: ['E'], value: 'v', ledger: 1 }], paginationToken: 'tok1' })
      .mockResolvedValueOnce({ events: [{ topic: ['E'], value: 'v', ledger: 2 }], paginationToken: 'tok2' })
      .mockResolvedValueOnce({ events: [{ topic: ['E'], value: 'v', ledger: 3 }], paginationToken: null });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 10);

    expect(events).toHaveLength(3);
    expect(getEvents).toHaveBeenCalledTimes(3);
    expect(getEvents).toHaveBeenNthCalledWith(2, expect.objectContaining({
      cursor: 'tok1',
    }));
    expect(getEvents).toHaveBeenNthCalledWith(3, expect.objectContaining({
      cursor: 'tok2',
    }));
  });

  it('stops immediately when the first page has no paginationToken', async () => {
    const getEvents = vi.fn().mockResolvedValue({
      events: [{ topic: ['E'], value: 'v', ledger: 5 }],
      paginationToken: null,
    });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 10);

    expect(events).toHaveLength(1);
    expect(getEvents).toHaveBeenCalledTimes(1);
  });

  it('handles empty events array on first page', async () => {
    const getEvents = vi.fn().mockResolvedValue({ events: [], paginationToken: null });
    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 5, 10);
    expect(events).toHaveLength(0);
    expect(getEvents).toHaveBeenCalledTimes(1);
  });

  it('handles undefined events field gracefully', async () => {
    const getEvents = vi.fn().mockResolvedValue({ paginationToken: null });
    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 5);
    expect(events).toHaveLength(0);
  });

  it('advances through multiple ledger windows', async () => {
    const getEvents = vi.fn().mockResolvedValue({ events: [], paginationToken: null });
    const server = { getEvents } as any;

    await collectMarketplaceEvents(server, ['C1'], 1, MAX_LEDGER_WINDOW + 5);

    expect(getEvents).toHaveBeenCalledTimes(2);
    expect(getEvents).toHaveBeenNthCalledWith(1, expect.objectContaining({ startLedger: 1 }));
    expect(getEvents).toHaveBeenNthCalledWith(2, expect.objectContaining({ startLedger: MAX_LEDGER_WINDOW + 1 }));
  });

  it('clamps the last window end to endLedger', async () => {
    const getEvents = vi.fn().mockResolvedValue({ events: [], paginationToken: null });
    await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, MAX_LEDGER_WINDOW + 50);
    expect(getEvents).toHaveBeenNthCalledWith(2, expect.objectContaining({
      startLedger: MAX_LEDGER_WINDOW + 1,
      endLedger: MAX_LEDGER_WINDOW + 50,
    }));
  });

  it('makes exactly one call when range fits in a single window', async () => {
    const getEvents = vi.fn().mockResolvedValue({ events: [], paginationToken: null });
    await collectMarketplaceEvents({ getEvents } as any, ['C1'], 100, 200);
    expect(getEvents).toHaveBeenCalledTimes(1);
    expect(getEvents).toHaveBeenCalledWith(expect.objectContaining({ startLedger: 100, endLedger: 200 }));
  });

  it('returns empty array when contractIds is empty', async () => {
    const getEvents = vi.fn();
    const events = await collectMarketplaceEvents({ getEvents } as any, [], 1, 100);
    expect(events).toHaveLength(0);
    expect(getEvents).not.toHaveBeenCalled();
  });

  it('returns empty array when startLedger > endLedger', async () => {
    const getEvents = vi.fn();
    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 200, 100);
    expect(events).toHaveLength(0);
    expect(getEvents).not.toHaveBeenCalled();
  });

  it('filters out events the parser returns null for', async () => {
    const { parseMarketplaceEvent } = await import('../parser.js');
    const mockParse = parseMarketplaceEvent as ReturnType<typeof vi.fn>;
    mockParse.mockReturnValueOnce({ eventType: 'OK', ledgerSequence: 1, actor: 'G', listingId: 1n, data: {} });
    mockParse.mockReturnValueOnce(null);

    const getEvents = vi.fn().mockResolvedValue({
      events: [
        { topic: ['OK'], value: 'v1', ledger: 1 },
        { topic: ['UNKNOWN'], value: 'v2', ledger: 2 },
      ],
      paginationToken: null,
    });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 10);
    expect(events).toHaveLength(1);
    expect(events[0].eventType).toBe('OK');
  });
});

// ── Issue #54: malformed event isolation ─────────────────────────────────────

describe('collectMarketplaceEvents — malformed event isolation', () => {
  beforeEach(() => vi.clearAllMocks());

  it('skips a malformed event and continues processing the remaining valid events', async () => {
    const { parseMarketplaceEvent } = await import('../parser.js');
    const mockParse = parseMarketplaceEvent as ReturnType<typeof vi.fn>;

    mockParse
      .mockReturnValueOnce({ eventType: 'VALID_A', ledgerSequence: 1, actor: 'G', listingId: 1n, data: {} })
      .mockImplementationOnce(() => { throw new Error('malformed XDR'); })
      .mockReturnValueOnce({ eventType: 'VALID_B', ledgerSequence: 3, actor: 'G', listingId: 3n, data: {} });

    const getEvents = vi.fn().mockResolvedValue({
      events: [
        { topic: ['t1'], value: 'ok-xdr', ledger: 1 },
        { topic: ['t2'], value: 'bad-xdr', ledger: 2 },
        { topic: ['t3'], value: 'ok-xdr', ledger: 3 },
      ],
      paginationToken: null,
    });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 10);

    expect(events).toHaveLength(2);
    expect(events.map((e) => e.eventType)).toEqual(['VALID_A', 'VALID_B']);
  });

  it('increments decodeErrorsCounter for each malformed event', async () => {
    const { parseMarketplaceEvent } = await import('../parser.js');
    const mockParse = parseMarketplaceEvent as ReturnType<typeof vi.fn>;

    mockParse
      .mockImplementationOnce(() => { throw new Error('bad 1'); })
      .mockImplementationOnce(() => { throw new Error('bad 2'); });

    const getEvents = vi.fn().mockResolvedValue({
      events: [
        { topic: ['t1'], value: 'v1', ledger: 10 },
        { topic: ['t2'], value: 'v2', ledger: 11 },
      ],
      paginationToken: null,
    });

    await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 20);

    expect(mockDecodeErrorsCounter.inc).toHaveBeenCalledTimes(2);
  });

  it('does not abort the cycle — returns all successfully decoded events', async () => {
    const { parseMarketplaceEvent } = await import('../parser.js');
    const mockParse = parseMarketplaceEvent as ReturnType<typeof vi.fn>;

    mockParse
      .mockImplementationOnce(() => { throw new Error('decode failure'); })
      .mockReturnValueOnce({ eventType: 'GOOD', ledgerSequence: 5, actor: 'G', listingId: null, data: {} });

    const getEvents = vi.fn().mockResolvedValue({
      events: [
        { topic: ['bad'], value: 'corrupt', ledger: 4 },
        { topic: ['good'], value: 'valid', ledger: 5 },
      ],
      paginationToken: null,
    });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 10);

    expect(events).toHaveLength(1);
    expect(events[0].eventType).toBe('GOOD');
    expect(mockDecodeErrorsCounter.inc).toHaveBeenCalledTimes(1);
  });
});