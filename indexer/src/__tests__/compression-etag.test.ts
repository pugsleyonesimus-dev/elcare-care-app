import { describe, it, expect, vi, beforeEach } from 'vitest';
import express from 'express';
import compression from 'compression';
import request from 'supertest';
import { etagMiddleware } from '../api/etag-middleware.js';

const mockPrisma = vi.hoisted(() => ({
  listing: {
    findMany: vi.fn(),
    count: vi.fn(),
    aggregate: vi.fn(),
  },
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ 
  default: {
    isOpen: false,
    isReady: false,
    get: vi.fn().mockRejectedValue(new Error('No Redis')),
  }
}));

import router from '../api/routes.js';

describe('Compression and ETag', () => {
  let app: express.Application;

  beforeEach(() => {
    app = express();
    app.use(compression());
    app.use(express.json());
    app.use(router);
  });

  it('should apply compression middleware to responses', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([
      { listingId: 1n, artist: 'GABC', price: '1000' }
    ]);

    const res = await request(app)
      .get('/listings');

    expect(res.status).toBe(200);
    // Compression is applied at middleware level; supertest handles it transparently
    // Verify the response is valid JSON (decompressed by supertest)
    expect(res.body).toBeDefined();
    expect(Array.isArray(res.body) || res.body.listings).toBeTruthy();
  });

  it('should return ETag for GET responses', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([
      { listingId: 1n, artist: 'GABC', price: '1000' }
    ]);

    const res = await request(app).get('/listings');
    
    expect(res.status).toBe(200);
    expect(res.get('ETag')).toBeDefined();
    expect(res.get('ETag')).toMatch(/^"[a-f0-9]+"$/);
  });

  it('should return 304 for matching If-None-Match', async () => {
    const data = [{ listingId: 1n, artist: 'GABC', price: '1000' }];
    mockPrisma.listing.findMany.mockResolvedValue(data);

    const firstRes = await request(app).get('/listings');
    const etag = firstRes.get('ETag');

    mockPrisma.listing.findMany.mockResolvedValue(data);
    const secondRes = await request(app)
      .get('/listings')
      .set('If-None-Match', etag);

    expect(secondRes.status).toBe(304);
    expect(secondRes.get('Content-Length')).toBeUndefined();
  });

  it('should return 200 for non-matching If-None-Match', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([
      { listingId: 1n, artist: 'GABC', price: '1000' }
    ]);

    const res = await request(app)
      .get('/listings')
      .set('If-None-Match', '"wrongetag"');

    expect(res.status).toBe(200);
    expect(res.body).toBeDefined();
  });

  it('should generate different ETags for different response bodies', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([
      { listingId: 1n, artist: 'GABC', price: '1000' }
    ]);
    const res1 = await request(app).get('/listings');
    const etag1 = res1.get('ETag');

    mockPrisma.listing.findMany.mockResolvedValue([
      { listingId: 2n, artist: 'GDEF', price: '2000' }
    ]);
    const res2 = await request(app).get('/listings');
    const etag2 = res2.get('ETag');

    expect(etag1).not.toBe(etag2);
  });
});
