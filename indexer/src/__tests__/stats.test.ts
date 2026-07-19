/**
 * stats.test.ts
 *
 * Verifies that the aggregation functions in stats.ts produce mathematically
 * correct results given known fixture data. Prisma and Redis are fully mocked
 * so no database connection is required.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import express from 'express';
import request from 'supertest';

// ── Mock Prisma ───────────────────────────────────────────────────────────────

const mockPrisma = vi.hoisted(() => ({
  listing: {
    count:    vi.fn(),
    aggregate: vi.fn(),
    groupBy:  vi.fn(),
    findMany: vi.fn(),
  },
  marketplaceEvent: {
    count:    vi.fn(),
    findMany: vi.fn(),
    groupBy:  vi.fn(),
  },
  collection: {
    count: vi.fn(),
  },
  $queryRaw: vi.fn(),
  $executeRaw: vi.fn(),
}));

const mockRedis = vi.hoisted(() => ({
  isOpen:  false,
  isReady: false,
  get:     vi.fn().mockResolvedValue(null),
  setEx:   vi.fn().mockResolvedValue(undefined),
  set:     vi.fn().mockResolvedValue(undefined),
  on:      vi.fn(),
  connect: vi.fn().mockRejectedValue(new Error('No Redis in tests')),
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: mockRedis }));

import router from '../api/routes';
import { errorHandler } from '../api/errors';

const app = express();
app.use(express.json());
app.use(router);
app.use(errorHandler);

// ── Helpers ───────────────────────────────────────────────────────────────────

function clearMocks() {
  vi.clearAllMocks();
  // Redis always misses so we always hit the DB aggregation functions
  mockRedis.get.mockResolvedValue(null);
}

// ── GET /stats/overview ───────────────────────────────────────────────────────

describe('GET /stats/overview', () => {
  beforeEach(clearMocks);

  it('returns accurate all-time totals', async () => {
    mockPrisma.listing.count
      .mockResolvedValueOnce(50)   // totalListings
      .mockResolvedValueOnce(undefined); // (not called again in overview)

    mockPrisma.marketplaceEvent.count.mockResolvedValue(12); // totalSales

    mockPrisma.listing.aggregate.mockResolvedValue({
      _sum: { price: '9500.0000000' },
    });

    // groupBy for distinct artists: 7 entries
    mockPrisma.listing.groupBy.mockResolvedValue(
      Array.from({ length: 7 }, (_, i) => ({ artist: `G${i}` }))
    );

    mockPrisma.collection.count.mockResolvedValue(3);

    const res = await request(app).get('/stats/overview');

    expect(res.status).toBe(200);
    expect(res.body.totalListings).toBe(50);
    expect(res.body.totalSales).toBe(12);
    expect(res.body.totalVolume).toBe('9500.0000000');
    expect(res.body.totalCreators).toBe(7);
    expect(res.body.totalCollections).toBe(3);
  });

  it('returns zero volume when nothing has been sold', async () => {
    mockPrisma.listing.count.mockResolvedValue(0);
    mockPrisma.marketplaceEvent.count.mockResolvedValue(0);
    mockPrisma.listing.aggregate.mockResolvedValue({ _sum: { price: null } });
    mockPrisma.listing.groupBy.mockResolvedValue([]);
    mockPrisma.collection.count.mockResolvedValue(0);

    const res = await request(app).get('/stats/overview');

    expect(res.status).toBe(200);
    expect(res.body.totalVolume).toBe('0');
    expect(res.body.totalCreators).toBe(0);
  });

  it('returns 500 when the DB throws', async () => {
    mockPrisma.listing.count.mockRejectedValue(new Error('DB error'));

    const res = await request(app).get('/stats/overview');
    expect(res.status).toBe(500);
  });
});

// ── GET /stats/daily ──────────────────────────────────────────────────────────

describe('GET /stats/daily', () => {
  beforeEach(clearMocks);

  const FROM = '2024-01-01';
  const TO   = '2024-01-03';

  // Simulate 2 rows returned from the materialized view
  const VIEW_ROWS = [
    {
      day:            new Date('2024-01-01T00:00:00.000Z'),
      sales_count:    BigInt(3),
      sales_volume:   '300.0000000',
      unique_buyers:  BigInt(2),
      unique_sellers: BigInt(2),
      new_listings:   BigInt(5),
      avg_sale_price: '100.0000000',
    },
    {
      day:            new Date('2024-01-02T00:00:00.000Z'),
      sales_count:    BigInt(1),
      sales_volume:   '150.0000000',
      unique_buyers:  BigInt(1),
      unique_sellers: BigInt(1),
      new_listings:   BigInt(2),
      avg_sale_price: '150.0000000',
    },
  ];

  it('returns per-day rows within the requested range', async () => {
    mockPrisma.$queryRaw.mockResolvedValue(VIEW_ROWS);

    const res = await request(app)
      .get(`/stats/daily?from=${FROM}&to=${TO}`);

    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
    expect(res.body).toHaveLength(2);

    const day1 = res.body[0];
    expect(day1.day).toBe('2024-01-01');
    expect(day1.salesCount).toBe(3);
    expect(day1.salesVolume).toBe('300.0000000');
    expect(day1.uniqueBuyers).toBe(2);
    expect(day1.newListings).toBe(5);
    expect(day1.avgSalePrice).toBe('100.0000000');

    const day2 = res.body[1];
    expect(day2.salesCount).toBe(1);
    expect(day2.salesVolume).toBe('150.0000000');
  });

  it('rejects a range exceeding 365 days with 400', async () => {
    const res = await request(app)
      .get('/stats/daily?from=2023-01-01&to=2024-12-31');

    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
    expect(res.body.error.message).toMatch(/365/);
  });

  it('rejects missing from/to params with 400', async () => {
    const res = await request(app).get('/stats/daily?from=2024-01-01');
    // `to` is required by statsDailyQuerySchema
    expect(res.status).toBe(400);
  });

  it('rejects an invalid ISO date with 400', async () => {
    const res = await request(app)
      .get('/stats/daily?from=not-a-date&to=2024-01-31');
    expect(res.status).toBe(400);
  });

  it('rejects from > to with 400', async () => {
    const res = await request(app)
      .get('/stats/daily?from=2024-06-01&to=2024-01-01');
    expect(res.status).toBe(400);
  });

  it('returns empty array when no data in range', async () => {
    mockPrisma.$queryRaw.mockResolvedValue([]);

    const res = await request(app)
      .get(`/stats/daily?from=${FROM}&to=${TO}`);

    expect(res.status).toBe(200);
    expect(res.body).toEqual([]);
  });

  it('verifies mathematical correctness: total volume = sum of daily volumes', async () => {
    mockPrisma.$queryRaw.mockResolvedValue(VIEW_ROWS);

    const res = await request(app)
      .get(`/stats/daily?from=${FROM}&to=${TO}`);

    const totalVolume = res.body.reduce(
      (acc: number, row: { salesVolume: string }) => acc + parseFloat(row.salesVolume),
      0
    );
    expect(totalVolume).toBeCloseTo(450, 5);

    const totalSales = res.body.reduce(
      (acc: number, row: { salesCount: number }) => acc + row.salesCount,
      0
    );
    expect(totalSales).toBe(4);
  });

  it('returns 500 when the DB throws', async () => {
    mockPrisma.$queryRaw.mockRejectedValue(new Error('DB gone'));

    const res = await request(app)
      .get(`/stats/daily?from=${FROM}&to=${TO}`);
    expect(res.status).toBe(500);
  });
});

// ── GET /stats/top-collections ────────────────────────────────────────────────

describe('GET /stats/top-collections', () => {
  beforeEach(clearMocks);

  const TOP_COLLECTIONS = [
    { collection: 'COLL_A', _count: { listingId: 8 }, _sum: { price: '800.0000000' } },
    { collection: 'COLL_B', _count: { listingId: 5 }, _sum: { price: '500.0000000' } },
    { collection: 'COLL_C', _count: { listingId: 2 }, _sum: { price: '200.0000000' } },
  ];

  it('returns top collections ordered by volume', async () => {
    mockPrisma.listing.groupBy.mockResolvedValue(TOP_COLLECTIONS);

    const res = await request(app).get('/stats/top-collections?limit=3');

    expect(res.status).toBe(200);
    expect(res.body).toHaveLength(3);
    expect(res.body[0].collection).toBe('COLL_A');
    expect(res.body[0].salesCount).toBe(8);
    expect(res.body[0].salesVolume).toBe('800.0000000');
    expect(res.body[1].salesVolume).toBe('500.0000000');
  });

  it('defaults limit to 10 when not provided', async () => {
    mockPrisma.listing.groupBy.mockResolvedValue([]);

    await request(app).get('/stats/top-collections');

    expect(mockPrisma.listing.groupBy).toHaveBeenCalledWith(
      expect.objectContaining({ take: 10 })
    );
  });

  it('caps limit at 100', async () => {
    // The schema rejects limit > 100 with a 400 — the DB is never queried
    const res = await request(app).get('/stats/top-collections?limit=999');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects a non-integer limit with 400', async () => {
    const res = await request(app).get('/stats/top-collections?limit=abc');
    expect(res.status).toBe(400);
  });

  it('returns 500 when the DB throws', async () => {
    mockPrisma.listing.groupBy.mockRejectedValue(new Error('DB error'));

    const res = await request(app).get('/stats/top-collections');
    expect(res.status).toBe(500);
  });
});

// ── GET /stats/top-artists ────────────────────────────────────────────────────

describe('GET /stats/top-artists', () => {
  beforeEach(clearMocks);

  const TOP_ARTISTS = [
    { artist: 'G_ARTIST_1', _count: { listingId: 10 }, _sum: { price: '1000.0000000' } },
    { artist: 'G_ARTIST_2', _count: { listingId: 4 },  _sum: { price: '400.0000000'  } },
  ];

  it('returns top artists ordered by earnings', async () => {
    mockPrisma.listing.groupBy.mockResolvedValue(TOP_ARTISTS);

    const res = await request(app).get('/stats/top-artists?limit=2');

    expect(res.status).toBe(200);
    expect(res.body).toHaveLength(2);
    expect(res.body[0].artist).toBe('G_ARTIST_1');
    expect(res.body[0].earnings).toBe('1000.0000000');
    expect(res.body[0].salesCount).toBe(10);
    expect(res.body[1].earnings).toBe('400.0000000');
  });

  it('verifies earnings sum matches expected total', async () => {
    mockPrisma.listing.groupBy.mockResolvedValue(TOP_ARTISTS);

    const res = await request(app).get('/stats/top-artists');

    const total = res.body.reduce(
      (acc: number, row: { earnings: string }) => acc + parseFloat(row.earnings),
      0
    );
    expect(total).toBeCloseTo(1400, 5);
  });

  it('returns 500 when the DB throws', async () => {
    mockPrisma.listing.groupBy.mockRejectedValue(new Error('DB error'));

    const res = await request(app).get('/stats/top-artists');
    expect(res.status).toBe(500);
  });
});
