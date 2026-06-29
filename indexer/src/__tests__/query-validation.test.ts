import { describe, it, expect, vi, beforeEach } from 'vitest';
import express from 'express';
import request from 'supertest';

// ── Mocks ────────────────────────────────────────────────────────────────────

const mockPrisma = vi.hoisted(() => ({
  listing: { findMany: vi.fn(), count: vi.fn(), aggregate: vi.fn() },
  auction: { findMany: vi.fn(), findUnique: vi.fn() },
  offer:   { findMany: vi.fn() },
  marketplaceEvent: { findMany: vi.fn(), count: vi.fn() },
  collection: { findMany: vi.fn() },
}));

const mockRedis = vi.hoisted(() => ({
  isOpen: false,
  isReady: false,
  get: vi.fn().mockResolvedValue(null),
  set: vi.fn().mockResolvedValue(undefined),
  on: vi.fn(),
  connect: vi.fn().mockRejectedValue(new Error('No Redis')),
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: mockRedis }));

import router from '../api/routes';
import { errorHandler } from '../api/errors';

const app = express();
app.use(express.json());
app.use(router);
app.use(errorHandler);

beforeEach(() => {
  vi.clearAllMocks();
  mockPrisma.listing.findMany.mockResolvedValue([]);
  mockPrisma.listing.count.mockResolvedValue(0);
  mockPrisma.listing.aggregate.mockResolvedValue({ _sum: { price: null } });
  mockPrisma.auction.findMany.mockResolvedValue([]);
  mockPrisma.offer.findMany.mockResolvedValue([]);
  mockPrisma.marketplaceEvent.findMany.mockResolvedValue([]);
  mockPrisma.marketplaceEvent.count.mockResolvedValue(0);
  mockPrisma.collection.findMany.mockResolvedValue([]);
});

// ── /listings ─────────────────────────────────────────────────────────────────

describe('GET /listings — query validation', () => {
  it('accepts valid limit and offset', async () => {
    const res = await request(app).get('/listings?limit=10&offset=5');
    expect(res.status).toBe(200);
  });

  it('rejects limit above 1000 with 400', async () => {
    const res = await request(app).get('/listings?limit=9999');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects offset above 10000 with 400', async () => {
    const res = await request(app).get('/listings?offset=99999');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects non-numeric limit with 400', async () => {
    const res = await request(app).get('/listings?limit=abc');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects negative offset with 400', async () => {
    const res = await request(app).get('/listings?offset=-1');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects negative minPrice with 400', async () => {
    const res = await request(app).get('/listings?minPrice=-5');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('accepts valid minPrice and maxPrice', async () => {
    const res = await request(app).get('/listings?minPrice=0&maxPrice=1000');
    expect(res.status).toBe(200);
  });

  it('caps limit at 1000 — boundary value is accepted', async () => {
    const res = await request(app).get('/listings?limit=1000');
    expect(res.status).toBe(200);
  });
});

// ── /auctions ─────────────────────────────────────────────────────────────────

describe('GET /auctions — query validation', () => {
  it('accepts valid query params', async () => {
    const res = await request(app).get('/auctions?status=Active');
    expect(res.status).toBe(200);
  });

  it('accepts no query params', async () => {
    const res = await request(app).get('/auctions');
    expect(res.status).toBe(200);
  });
});

// ── /offers ───────────────────────────────────────────────────────────────────

describe('GET /offers — query validation', () => {
  it('accepts a numeric listing_id', async () => {
    const res = await request(app).get('/offers?listing_id=42');
    expect(res.status).toBe(200);
  });

  it('rejects a non-numeric listing_id with 400', async () => {
    const res = await request(app).get('/offers?listing_id=abc');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects a negative listing_id with 400', async () => {
    const res = await request(app).get('/offers?listing_id=-1');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });
});

// ── /wallets/:address/activity ────────────────────────────────────────────────

describe('GET /wallets/:address/activity — query validation', () => {
  it('accepts a valid limit', async () => {
    const res = await request(app).get('/wallets/GTEST/activity?limit=50');
    expect(res.status).toBe(200);
  });

  it('rejects limit above 200 with 400', async () => {
    const res = await request(app).get('/wallets/GTEST/activity?limit=500');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('rejects non-numeric limit with 400', async () => {
    const res = await request(app).get('/wallets/GTEST/activity?limit=xyz');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });
});

// ── /stats ────────────────────────────────────────────────────────────────────

describe('GET /stats — query validation', () => {
  it('accepts valid range values', async () => {
    for (const range of ['day', 'week', 'month']) {
      mockPrisma.marketplaceEvent.findMany.mockResolvedValue([]);
      mockPrisma.marketplaceEvent.count.mockResolvedValue(0);
      const res = await request(app).get(`/stats?range=${range}`);
      expect(res.status, `range=${range}`).toBe(200);
    }
  });

  it('rejects an invalid range value with 400', async () => {
    const res = await request(app).get('/stats?range=year');
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe('BAD_REQUEST');
  });

  it('accepts no params', async () => {
    mockPrisma.marketplaceEvent.findMany.mockResolvedValue([]);
    mockPrisma.marketplaceEvent.count.mockResolvedValue(0);
    const res = await request(app).get('/stats');
    expect(res.status).toBe(200);
  });
});
