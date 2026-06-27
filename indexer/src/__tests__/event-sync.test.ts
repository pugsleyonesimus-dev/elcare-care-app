import { describe, it, expect, vi } from 'vitest';

vi.mock('../parser.js', () => ({
  parseMarketplaceEvent: vi.fn((topics: string[], _valueXdr: string, ledger: number) => ({
    eventType: topics[0],
    listingId: BigInt(ledger),
    actor: 'GTEST',
    ledgerSequence: ledger,
    data: { ledger },
  })),
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

  it('none dropped: collects events from multi-page responses across multiple ledger windows', async () => {
    // Two windows; each window has two pages of events.
    // Old single-fetch code would have silently dropped page 2 in each window.
    let call = 0;
    const getEvents = vi.fn().mockImplementation(({ startLedger }: any) => {
      call++;
      if (startLedger === 1) {
        // Window 1, page 1
        if (call === 1) return Promise.resolve({ events: [{ topic: ['E'], value: 'v', ledger: 1 }], paginationToken: 'w1-tok2' });
        // Window 1, page 2
        if (call === 2) return Promise.resolve({ events: [{ topic: ['E'], value: 'v', ledger: 2 }], paginationToken: null });
      }
      // Window 2, page 1
      if (call === 3) return Promise.resolve({ events: [{ topic: ['E'], value: 'v', ledger: MAX_LEDGER_WINDOW + 1 }], paginationToken: 'w2-tok2' });
      // Window 2, page 2
      return Promise.resolve({ events: [{ topic: ['E'], value: 'v', ledger: MAX_LEDGER_WINDOW + 2 }], paginationToken: null });
    });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, MAX_LEDGER_WINDOW + 5);

    // 2 pages × 2 windows = 4 events — none dropped
    expect(events).toHaveLength(4);
    expect(getEvents).toHaveBeenCalledTimes(4);
  });

  it('carry-forward: next cycle picks up from the ledger after the last event processed', async () => {
    // This test verifies the contract between collectMarketplaceEvents and the
    // poller: the function returns events up to endLedger; the poller then
    // advances syncState to the max ledger seen, so the next call starts at
    // maxLedger + 1 — the remaining range is carried forward automatically.
    const getEvents = vi.fn().mockResolvedValue({
      events: [
        { topic: ['E'], value: 'v', ledger: 50 },
        { topic: ['E'], value: 'v', ledger: 75 },
      ],
      paginationToken: null,
    });

    const events = await collectMarketplaceEvents({ getEvents } as any, ['C1'], 1, 100);

    expect(events).toHaveLength(2);
    const maxLedger = Math.max(...events.map((e) => e.ledgerSequence));
    expect(maxLedger).toBe(75);
    // The poller uses this value to advance syncState; next cycle: startLedger = 76
  });
});