import { describe, it, expect, vi, beforeEach } from 'vitest';

// Hoist mock factories so they are available inside vi.mock() closures
const { mockScValToNative, mockFromXDR } = vi.hoisted(() => ({
  mockScValToNative: vi.fn(),
  mockFromXDR: vi.fn(() => ({})),
}));

vi.mock('@stellar/stellar-sdk', () => ({
  xdr: {
    ScVal: {
      fromXDR: mockFromXDR,
    },
  },
  Address: class {},
  scValToNative: mockScValToNative,
}));

import { parseMarketplaceEvent, DecodedEvent } from '../parser';

// ── helpers ───────────────────────────────────────────────────────────────────

/**
 * Sets up mocks for a single parseMarketplaceEvent call.
 * - topicSymbol: the symbol the XDR topic decodes to (e.g. 'lst_crtd')
 * - valueData:   the plain object returned by scValToNative for the value XDR
 */
function setupMocks(topicSymbol: string, valueData: Record<string, any>) {
  // First scValToNative call → topic symbol
  // Second scValToNative call → event value data
  mockScValToNative
    .mockReturnValueOnce(topicSymbol)
    .mockReturnValueOnce(valueData);
}

// ── topic → eventType mapping ─────────────────────────────────────────────────

describe('parseMarketplaceEvent — topic mapping', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  const cases: [string, string][] = [
    ['lst_crtd', 'LISTING_CREATED'],
    ['art_sold', 'ARTWORK_SOLD'],
    ['lst_cncl', 'LISTING_CANCELLED'],
    ['lst_updt', 'LISTING_UPDATED'],
    ['bid_plcd', 'BID_PLACED'],
    ['auc_rslv', 'AUCTION_RESOLVED'],
    ['auc_cncl', 'AUCTION_CANCELLED'],
    ['ofr_made', 'OFFER_MADE'],
    ['ofr_accp', 'OFFER_ACCEPTED'],
    ['ofr_rjct', 'OFFER_REJECTED'],
    ['ofr_wdrn', 'OFFER_WITHDRAWN'],
    ['auc_crtd', 'AUCTION_CREATED'],
  ];

  for (const [symbol, expectedType] of cases) {
    it(`maps '${symbol}' → '${expectedType}'`, () => {
      setupMocks(symbol, { listing_id: 1n, artist: 'GA1' });

      const result = parseMarketplaceEvent(['topic_xdr'], 'value_xdr', 42);

      expect(result).not.toBeNull();
      expect(result!.eventType).toBe(expectedType);
    });
  }

  it('returns null for an unknown topic symbol', () => {
    setupMocks('unknown_sym', {});
    expect(parseMarketplaceEvent(['topic_xdr'], 'value_xdr', 1)).toBeNull();
  });
});

// ── fallback path (raw string topic) ─────────────────────────────────────────

describe('parseMarketplaceEvent — XDR fallback', () => {
  beforeEach(() => vi.resetAllMocks());

  it('falls back to the raw topic string when XDR parsing throws', () => {
    // First fromXDR call (for topic) throws; second call (for value) succeeds.
    mockFromXDR
      .mockImplementationOnce(() => { throw new Error('bad XDR'); })
      .mockReturnValueOnce({});
    // Only one scValToNative call (for the value) because topic path errored
    mockScValToNative.mockReturnValueOnce({ listing_id: 99n, artist: 'GFALLBACK' });

    const result = parseMarketplaceEvent(['lst_crtd'], 'value_xdr', 10);

    expect(result).not.toBeNull();
    expect(result!.eventType).toBe('LISTING_CREATED');
    expect(result!.actor).toBe('GFALLBACK');
  });

  it('returns null when raw fallback topic is not in TOPIC_MAP', () => {
    mockFromXDR.mockImplementationOnce(() => { throw new Error('bad XDR'); });

    const result = parseMarketplaceEvent(['not_a_topic'], 'value_xdr', 10);
    expect(result).toBeNull();
  });
});

// ── listingId extraction ──────────────────────────────────────────────────────

describe('parseMarketplaceEvent — listingId', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('extracts listing_id as BigInt', () => {
    setupMocks('lst_crtd', { listing_id: 5n, artist: 'GA' });

    const result = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(result.listingId).toBe(5n);
  });

  it('extracts auction_id as listingId for auction events', () => {
    setupMocks('auc_crtd', { auction_id: 7n, creator: 'GA_CREATOR' });

    const result = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(result.listingId).toBe(7n);
  });

  it('sets listingId to null when neither listing_id nor auction_id present', () => {
    setupMocks('ofr_made', { offerer: 'GA_OFFERER' });

    const result = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(result.listingId).toBeNull();
  });
});

// ── actor extraction ──────────────────────────────────────────────────────────

describe('parseMarketplaceEvent — actor priority', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('picks artist when present', () => {
    setupMocks('lst_crtd', { listing_id: 1n, artist: 'GA_ARTIST' });
    expect(parseMarketplaceEvent(['t'], 'v', 1)!.actor).toBe('GA_ARTIST');
  });

  it('picks creator when artist is absent', () => {
    setupMocks('auc_crtd', { auction_id: 1n, creator: 'GA_CREATOR' });
    expect(parseMarketplaceEvent(['t'], 'v', 1)!.actor).toBe('GA_CREATOR');
  });

  it('picks offerer when artist and creator are absent', () => {
    setupMocks('ofr_made', { offerer: 'GA_OFFERER' });
    expect(parseMarketplaceEvent(['t'], 'v', 1)!.actor).toBe('GA_OFFERER');
  });

  it('picks bidder when others are absent', () => {
    setupMocks('bid_plcd', { bidder: 'GA_BIDDER' });
    expect(parseMarketplaceEvent(['t'], 'v', 1)!.actor).toBe('GA_BIDDER');
  });

  it('picks buyer when others are absent', () => {
    setupMocks('art_sold', { listing_id: 1n, buyer: 'GA_BUYER' });
    expect(parseMarketplaceEvent(['t'], 'v', 1)!.actor).toBe('GA_BUYER');
  });

  it('leaves actor as empty string when no known actor field present', () => {
    setupMocks('lst_updt', { listing_id: 1n, new_price: 500n });
    expect(parseMarketplaceEvent(['t'], 'v', 1)!.actor).toBe('');
  });
});

// ── ledgerSequence passthrough ────────────────────────────────────────────────

describe('parseMarketplaceEvent — ledgerSequence', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('preserves the supplied ledger sequence number', () => {
    setupMocks('lst_crtd', { listing_id: 1n, artist: 'GA' });
    expect(parseMarketplaceEvent(['t'], 'v', 12345)!.ledgerSequence).toBe(12345);
  });
});

// ── convertBigInts (via data field) ──────────────────────────────────────────

describe('parseMarketplaceEvent — BigInt serialisation in data', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('converts top-level BigInt values to strings in the data payload', () => {
    setupMocks('lst_crtd', { listing_id: 1n, price: 10_000_000n, artist: 'GA' });

    const result = parseMarketplaceEvent(['t'], 'v', 1)!;
    // BigInts in data must be strings (safe for JSON)
    expect(typeof result.data.listing_id).toBe('string');
    expect(result.data.listing_id).toBe('1');
    expect(result.data.price).toBe('10000000');
  });

  it('converts nested BigInt values to strings', () => {
    setupMocks('bid_plcd', {
      listing_id: 2n,
      nested: { amount: 999n },
    });

    const result = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(result.data.nested.amount).toBe('999');
  });

  it('converts BigInt values inside arrays to strings', () => {
    setupMocks('ofr_made', {
      amounts: [100n, 200n],
    });

    const result = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(result.data.amounts).toEqual(['100', '200']);
  });
});

// ── per-event-type fixtures ───────────────────────────────────────────────────
// Each fixture uses representative decoded data matching the on-chain structure.
// Assertions verify field extraction AND that BigInts are converted to strings
// in the data payload (safe for JSON storage).

describe('parseMarketplaceEvent — LISTING_CREATED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('extracts listingId, actor, ledger and serialises all BigInt fields', () => {
    setupMocks('lst_crtd', {
      listing_id: 1n,
      artist: 'GARTIST000',
      price: 10_000_000n,
      currency: 'USDC',
      collection: 'CCOLLECTION',
      token_id: 42n,
      token: 'CTOKEN',
    });

    const r = parseMarketplaceEvent(['topic_xdr'], 'value_xdr', 500)!;
    expect(r.eventType).toBe('LISTING_CREATED');
    expect(r.listingId).toBe(1n);
    expect(r.actor).toBe('GARTIST000');
    expect(r.ledgerSequence).toBe(500);
    expect(r.data.price).toBe('10000000');
    expect(r.data.token_id).toBe('42');
    expect(r.data.listing_id).toBe('1');
    expect(r.data.currency).toBe('USDC');
  });

  it('handles null/absent optional recipients field without throwing', () => {
    setupMocks('lst_crtd', {
      listing_id: 2n,
      artist: 'GARTIST000',
      price: 500n,
      collection: 'CC',
      token_id: 1n,
      // recipients intentionally omitted
    });

    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.listingId).toBe(2n);
    expect(r.data.recipients).toBeUndefined();
  });

  it('preserves recipient percentage as a string when recipients are nested objects with BigInts', () => {
    setupMocks('lst_crtd', {
      listing_id: 3n,
      artist: 'GA',
      price: 100n,
      collection: 'CC',
      token_id: 1n,
      recipients: [{ address: 'GRECIP', percentage: 500n }],
    });

    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.data.recipients[0].percentage).toBe('500');
    expect(r.data.recipients[0].address).toBe('GRECIP');
  });
});

describe('parseMarketplaceEvent — ARTWORK_SOLD fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('extracts buyer as actor and serialises price', () => {
    setupMocks('art_sold', {
      listing_id: 8n,
      buyer: 'GBUYER111',
      price: 25_000_000n,
    });

    const r = parseMarketplaceEvent(['t'], 'v', 800)!;
    expect(r.eventType).toBe('ARTWORK_SOLD');
    expect(r.listingId).toBe(8n);
    expect(r.actor).toBe('GBUYER111');
    expect(r.data.price).toBe('25000000');
    expect(r.data.buyer).toBe('GBUYER111');
  });
});

describe('parseMarketplaceEvent — LISTING_CANCELLED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('maps listing_id and leaves actor empty when no actor field present', () => {
    setupMocks('lst_cncl', { listing_id: 3n });

    const r = parseMarketplaceEvent(['t'], 'v', 300)!;
    expect(r.eventType).toBe('LISTING_CANCELLED');
    expect(r.listingId).toBe(3n);
    expect(r.actor).toBe('');
    expect(r.data.listing_id).toBe('3');
  });
});

describe('parseMarketplaceEvent — LISTING_UPDATED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('serialises new_price BigInt and carries token_id', () => {
    setupMocks('lst_updt', {
      listing_id: 5n,
      new_price: 20_000_000n,
      token_id: 7n,
    });

    const r = parseMarketplaceEvent(['t'], 'v', 350)!;
    expect(r.eventType).toBe('LISTING_UPDATED');
    expect(r.listingId).toBe(5n);
    expect(r.data.new_price).toBe('20000000');
    expect(r.data.token_id).toBe('7');
  });
});

describe('parseMarketplaceEvent — AUCTION_CREATED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('maps auction_id to listingId and serialises reserve_price and end_time', () => {
    setupMocks('auc_crtd', {
      auction_id: 11n,
      creator: 'GCREATOR',
      reserve_price: 50_000_000n,
      end_time: 1_800_000_000n,
      token: 'CTOKEN',
      collection: 'CAUC',
      token_id: 99n,
    });

    const r = parseMarketplaceEvent(['t'], 'v', 600)!;
    expect(r.eventType).toBe('AUCTION_CREATED');
    expect(r.listingId).toBe(11n);
    expect(r.actor).toBe('GCREATOR');
    expect(r.data.reserve_price).toBe('50000000');
    expect(r.data.end_time).toBe('1800000000');
    expect(r.data.token_id).toBe('99');
  });

  it('sets listingId to null when auction_id is absent', () => {
    setupMocks('auc_crtd', { creator: 'GCREATOR' });
    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.listingId).toBeNull();
  });
});

describe('parseMarketplaceEvent — BID_PLACED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('extracts bidder as actor and serialises bid_amount', () => {
    setupMocks('bid_plcd', {
      auction_id: 11n,
      bidder: 'GBIDDER',
      bid_amount: 55_000_000n,
    });

    const r = parseMarketplaceEvent(['t'], 'v', 610)!;
    expect(r.eventType).toBe('BID_PLACED');
    expect(r.actor).toBe('GBIDDER');
    expect(r.data.bid_amount).toBe('55000000');
  });
});

describe('parseMarketplaceEvent — AUCTION_RESOLVED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('serialises final amount and preserves winner address', () => {
    setupMocks('auc_rslv', {
      auction_id: 11n,
      winner: 'GWINNER',
      amount: 55_000_000n,
    });

    const r = parseMarketplaceEvent(['t'], 'v', 620)!;
    expect(r.eventType).toBe('AUCTION_RESOLVED');
    expect(r.data.amount).toBe('55000000');
    expect(r.data.winner).toBe('GWINNER');
  });

  it('handles null winner (no-bid resolution) without throwing', () => {
    setupMocks('auc_rslv', { auction_id: 12n, amount: 0n, winner: null });
    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.data.winner).toBeNull();
    expect(r.data.amount).toBe('0');
  });
});

describe('parseMarketplaceEvent — AUCTION_CANCELLED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('maps auction_id to listingId for AUCTION_CANCELLED', () => {
    setupMocks('auc_cncl', { auction_id: 13n });
    const r = parseMarketplaceEvent(['t'], 'v', 615)!;
    expect(r.eventType).toBe('AUCTION_CANCELLED');
    expect(r.listingId).toBe(13n);
  });
});

describe('parseMarketplaceEvent — OFFER_MADE fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('extracts offerer as actor and serialises offer_id, listing_id and amount', () => {
    setupMocks('ofr_made', {
      offer_id: 1n,
      listing_id: 42n,
      offerer: 'GOFFERER',
      amount: 30_000_000n,
      token: 'CTOKEN',
    });

    const r = parseMarketplaceEvent(['t'], 'v', 630)!;
    expect(r.eventType).toBe('OFFER_MADE');
    expect(r.actor).toBe('GOFFERER');
    expect(r.data.offer_id).toBe('1');
    expect(r.data.listing_id).toBe('42');
    expect(r.data.amount).toBe('30000000');
  });
});

describe('parseMarketplaceEvent — OFFER_ACCEPTED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('serialises offer_id and listing_id', () => {
    setupMocks('ofr_accp', {
      offer_id: 1n,
      listing_id: 42n,
      offerer: 'GOFFERER',
    });

    const r = parseMarketplaceEvent(['t'], 'v', 640)!;
    expect(r.eventType).toBe('OFFER_ACCEPTED');
    expect(r.data.offer_id).toBe('1');
    expect(r.data.listing_id).toBe('42');
  });
});

describe('parseMarketplaceEvent — OFFER_REJECTED fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('maps offer_id to data', () => {
    setupMocks('ofr_rjct', { offer_id: 2n, listing_id: 5n });
    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.eventType).toBe('OFFER_REJECTED');
    expect(r.data.offer_id).toBe('2');
  });
});

describe('parseMarketplaceEvent — OFFER_WITHDRAWN fixture', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  it('maps offer_id to data', () => {
    setupMocks('ofr_wdrn', { offer_id: 3n, listing_id: 7n });
    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.eventType).toBe('OFFER_WITHDRAWN');
    expect(r.data.offer_id).toBe('3');
  });
});

describe('parseMarketplaceEvent — deploy event fixtures', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFromXDR.mockReturnValue({});
  });

  const deployTypes: [string, string][] = [
    ['dep_n721',  'DEPLOY_NORMAL_721'],
    ['dep_n1155', 'DEPLOY_NORMAL_1155'],
    ['dep_l721',  'DEPLOY_LAZY_721'],
    ['dep_l1155', 'DEPLOY_LAZY_1155'],
  ];

  for (const [symbol, expectedType] of deployTypes) {
    it(`${expectedType}: extracts creator from tuple index 0`, () => {
      // scValToNative returns a 2-tuple [creator, contractAddress] for deploy events
      setupMocks(symbol, ['GCREATOR', 'CCONTRACT']);

      const r = parseMarketplaceEvent(['t'], 'v', 700)!;
      expect(r.eventType).toBe(expectedType);
      expect(r.actor).toBe('GCREATOR');
      expect(r.listingId).toBeNull();
    });
  }

  it('DEPLOY_NORMAL_721: sets listingId to null (no listing_id in deploy data)', () => {
    setupMocks('dep_n721', ['GCREATOR', 'CCONTRACT']);
    const r = parseMarketplaceEvent(['t'], 'v', 1)!;
    expect(r.listingId).toBeNull();
  });
});
