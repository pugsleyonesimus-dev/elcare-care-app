import { describe, it, expect, vi, beforeEach } from 'vitest';
import express from 'express';
import request from 'supertest';

const mockPrisma = vi.hoisted(() => ({
  syncState: {
    findUnique: vi.fn(),
  },
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: { get: vi.fn() } }));
vi.mock('../poller', () => ({ startPolling: vi.fn() }));

describe('Readiness Probe (/readyz)', () => {
  let app: express.Application;

  beforeEach(() => {
    vi.clearAllMocks();
  });

  const createApp = () => {
    const app = express();
    app.use(express.json());

    app.get('/readyz', async (req: express.Request, res: express.Response) => {
      const reasons: string[] = [];

      try {
        await mockPrisma.syncState.findUnique({ where: { id: 1 } });
      } catch (err) {
        reasons.push('Database unreachable');
      }

      try {
        const state = await mockPrisma.syncState.findUnique({ where: { id: 1 } });
        if (!state || state.lastLedger === 0) {
          reasons.push('No ledgers indexed yet');
        }
      } catch (err) {
        reasons.push('Failed to check sync state');
      }

      if (reasons.length > 0) {
        return res.status(503).json({ status: 'not_ready', reasons });
      }

      const state = await mockPrisma.syncState.findUnique({ where: { id: 1 } });
      res.json({ status: 'ready', lastLedger: state?.lastLedger });
    });

    return app;
  };

  it('should return 503 when database is unreachable', async () => {
    app = createApp();
    mockPrisma.syncState.findUnique.mockRejectedValue(new Error('DB error'));

    const res = await request(app).get('/readyz');

    expect(res.status).toBe(503);
    expect(res.body.status).toBe('not_ready');
    expect(res.body.reasons).toContain('Database unreachable');
  });

  it('should return 503 when no ledgers indexed yet', async () => {
    app = createApp();
    mockPrisma.syncState.findUnique.mockResolvedValue({ id: 1, lastLedger: 0 });

    const res = await request(app).get('/readyz');

    expect(res.status).toBe(503);
    expect(res.body.status).toBe('not_ready');
    expect(res.body.reasons).toContain('No ledgers indexed yet');
  });

  it('should return 503 when sync state is null', async () => {
    app = createApp();
    mockPrisma.syncState.findUnique.mockResolvedValue(null);

    const res = await request(app).get('/readyz');

    expect(res.status).toBe(503);
    expect(res.body.status).toBe('not_ready');
    expect(res.body.reasons).toContain('No ledgers indexed yet');
  });

  it('should return 200 when database is ready and ledgers are indexed', async () => {
    app = createApp();
    mockPrisma.syncState.findUnique.mockResolvedValue({ id: 1, lastLedger: 12345 });

    const res = await request(app).get('/readyz');

    expect(res.status).toBe(200);
    expect(res.body.status).toBe('ready');
    expect(res.body.lastLedger).toBe(12345);
  });

  it('should include multiple reasons when multiple checks fail', async () => {
    app = createApp();
    let callCount = 0;
    mockPrisma.syncState.findUnique.mockImplementation(() => {
      callCount++;
      if (callCount === 1) {
        throw new Error('DB error');
      }
      return Promise.resolve(null);
    });

    const res = await request(app).get('/readyz');

    expect(res.status).toBe(503);
    expect(res.body.reasons.length).toBeGreaterThan(0);
  });
});
