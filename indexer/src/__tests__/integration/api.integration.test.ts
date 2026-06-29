import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import express from 'express';
import request from 'supertest';
import { PrismaClient } from '@prisma/client';
import { cacheMiddleware } from '../../api/cache-middleware.js';

const prisma = new PrismaClient();

function serialize(obj: unknown) {
  return JSON.parse(
    JSON.stringify(obj, (_key, value) => (typeof value === 'bigint' ? value.toString() : value))
  );
}

async function waitForRedisReady() {
  const { default: redis } = await import('../../redis.js');
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (redis.isReady) return redis;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('Redis did not become ready for integration tests');
}

describe('Indexer API integration (Postgres + Redis)', () => {
  let app: express.Express;
  let redis: Awaited<ReturnType<typeof waitForRedisReady>>;

  beforeAll(async () => {
    redis = await waitForRedisReady();
    await redis.flushDb();

    app = express();
    app.get('/listings', cacheMiddleware(30), async (_req, res) => {
      const listings = await prisma.listing.findMany({ orderBy: { listingId: 'asc' } });
      res.json({ listings: serialize(listings), total: listings.length });
    });

    app.get('/collections', cacheMiddleware(60), async (_req, res) => {
      const collections = await prisma.collection.findMany({ orderBy: { contractAddress: 'asc' } });
      res.json({ collections: serialize(collections), total: collections.length });
    });
  });

  afterAll(async () => {
    await prisma.$disconnect();
  });

  it('reads representative seeded listings from Postgres', async () => {
    const res = await request(app).get('/listings');
    expect(res.status).toBe(200);
    expect(res.body.total).toBeGreaterThanOrEqual(2);
    expect(res.body.listings.some((row: { listingId: string }) => row.listingId === '101')).toBe(
      true
    );
    expect(res.body.listings.some((row: { status: string }) => row.status === 'Sold')).toBe(true);
  });

  it('caches listing responses in Redis', async () => {
    const cacheKey = 'cache:/listings';
    await redis.del(cacheKey);

    const first = await request(app).get('/listings');
    expect(first.status).toBe(200);

    const cachedPayload = await redis.get(cacheKey);
    expect(cachedPayload).toBeTruthy();

    const second = await request(app).get('/listings');
    expect(second.body).toEqual(first.body);
  });

  it('reads seeded collections from Postgres', async () => {
    const res = await request(app).get('/collections');
    expect(res.status).toBe(200);
    expect(res.body.total).toBeGreaterThanOrEqual(2);
    expect(res.body.collections[0]).toHaveProperty('name');
  });
});
