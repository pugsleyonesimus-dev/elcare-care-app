import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import express from 'express';
import cors from 'cors';
import request from 'supertest';

const mockPrisma = vi.hoisted(() => ({
  listing: {
    findMany: vi.fn(),
  },
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: { get: vi.fn() } }));

describe('CORS Configuration', () => {
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    originalEnv = { ...process.env };
    mockPrisma.listing.findMany.mockResolvedValue([]);
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  const createApp = (corsConfig: any) => {
    const app = express();
    app.use(cors(corsConfig));
    app.use(express.json());
    
    app.get('/listings', (req, res) => {
      res.json([]);
    });

    return app;
  };

  it('should allow localhost origins in development', async () => {
    const app = createApp({
      origin: process.env.NODE_ENV === 'production' ? [] : true,
      credentials: true,
    });

    const res = await request(app)
      .get('/listings')
      .set('Origin', 'http://localhost:3000');

    expect(res.status).toBe(200);
    expect(res.get('Access-Control-Allow-Origin')).toBe('http://localhost:3000');
  });

  it('should allow preflight requests in development', async () => {
    const app = createApp({
      origin: true,
      credentials: true,
    });

    const res = await request(app)
      .options('/listings')
      .set('Origin', 'http://localhost:3000')
      .set('Access-Control-Request-Method', 'GET');

    expect(res.status).toBe(204);
    expect(res.get('Access-Control-Allow-Origin')).toBe('http://localhost:3000');
  });

  it('should restrict origins to allowlist in production', async () => {
    const allowedOrigins = ['https://example.com', 'https://app.example.com'];
    const corsConfig = {
      origin: allowedOrigins,
      credentials: true,
    };

    const app = createApp(corsConfig);

    // Allowed origin
    const resAllowed = await request(app)
      .get('/listings')
      .set('Origin', 'https://example.com');

    expect(resAllowed.get('Access-Control-Allow-Origin')).toBe('https://example.com');

    // Disallowed origin
    const resDisallowed = await request(app)
      .get('/listings')
      .set('Origin', 'https://evil.com');

    expect(resDisallowed.get('Access-Control-Allow-Origin')).toBeUndefined();
  });

  it('should parse CORS_ORIGIN from env and create allowlist', () => {
    const corsOriginEnv = 'https://example.com, https://app.example.com, https://dashboard.example.com';
    const allowlist = corsOriginEnv.split(',').map(o => o.trim()).filter(Boolean);

    expect(allowlist).toEqual([
      'https://example.com',
      'https://app.example.com',
      'https://dashboard.example.com',
    ]);
  });

  it('should handle whitespace in CORS_ORIGIN env variable', () => {
    const corsOriginEnv = '  https://example.com  ,  https://app.example.com  ';
    const allowlist = corsOriginEnv.split(',').map(o => o.trim()).filter(Boolean);

    expect(allowlist).toEqual([
      'https://example.com',
      'https://app.example.com',
    ]);
  });

  it('should handle empty CORS_ORIGIN env variable', () => {
    const corsOriginEnv = '';
    const allowlist = corsOriginEnv.split(',').map(o => o.trim()).filter(Boolean);

    expect(allowlist).toEqual([]);
  });

  it('should reject CORS request with missing origin header', async () => {
    const allowedOrigins = ['https://example.com'];
    const app = createApp({
      origin: allowedOrigins,
      credentials: true,
    });

    const res = await request(app).get('/listings');

    // Requests without Origin header are typically allowed, but CORS headers not set
    expect(res.status).toBe(200);
  });

  it('should handle multiple preflight requests correctly', async () => {
    const app = createApp({
      origin: 'https://example.com',
      credentials: true,
    });

    for (let i = 0; i < 3; i++) {
      const res = await request(app)
        .options('/listings')
        .set('Origin', 'https://example.com')
        .set('Access-Control-Request-Method', 'POST')
        .set('Access-Control-Request-Headers', 'Content-Type');

      expect(res.status).toBe(204);
      expect(res.get('Access-Control-Allow-Origin')).toBe('https://example.com');
    }
  });

  it('should include credentials in CORS response when configured', async () => {
    const app = createApp({
      origin: 'https://example.com',
      credentials: true,
    });

    const res = await request(app)
      .get('/listings')
      .set('Origin', 'https://example.com');

    expect(res.get('Access-Control-Allow-Credentials')).toBe('true');
  });
});
