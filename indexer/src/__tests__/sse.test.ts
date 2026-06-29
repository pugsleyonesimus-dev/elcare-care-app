import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import express from 'express';
import http from 'http';

// ── Mocks ────────────────────────────────────────────────────────────────────

const mockPrisma = vi.hoisted(() => ({
  listing: { findMany: vi.fn() },
  marketplaceEvent: { findMany: vi.fn(), count: vi.fn() },
}));

const mockRedis = vi.hoisted(() => ({
  isOpen: false, isReady: false,
  get: vi.fn().mockResolvedValue(null),
  set: vi.fn().mockResolvedValue(undefined),
  on: vi.fn(),
  connect: vi.fn().mockRejectedValue(new Error('No Redis')),
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: mockRedis }));

import router, { emitSSEEvent, _getSseBuffer, _getSseEventCounter, _resetSseState } from '../api/routes';
import { errorHandler } from '../api/errors';

let server: http.Server;
let baseUrl: string;

const app = express();
app.use(express.json());
app.use(router);
app.use(errorHandler);

beforeEach(async () => {
  _resetSseState();
  vi.clearAllMocks();
  await new Promise<void>((resolve) => {
    server = app.listen(0, resolve);
  });
  const addr = server.address() as { port: number };
  baseUrl = `http://127.0.0.1:${addr.port}`;
});

afterEach(async () => {
  await new Promise<void>((resolve) => server.close(() => resolve()));
});

// Collect SSE chunks from a GET /events request until `count` frames arrive or timeout
function collectSseFrames(url: string, headers: Record<string, string>, count: number, timeoutMs = 500): Promise<string[]> {
  return new Promise((resolve, reject) => {
    const req = http.get(url, { headers }, (res) => {
      const frames: string[] = [];
      let buf = '';
      const timer = setTimeout(() => {
        req.destroy();
        resolve(frames);
      }, timeoutMs);

      res.on('data', (chunk: Buffer) => {
        buf += chunk.toString();
        // Split on double newline (SSE frame boundary)
        const parts = buf.split('\n\n');
        buf = parts.pop() ?? '';
        for (const part of parts) {
          if (part.trim()) {
            frames.push(part);
            if (frames.length >= count) {
              clearTimeout(timer);
              req.destroy();
              resolve(frames);
              return;
            }
          }
        }
      });

      res.on('error', reject);
    });
    req.on('error', (err) => {
      // destroyed intentionally — resolve with what we have
      if ((err as any).code === 'ECONNRESET') return;
      reject(err);
    });
  });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('SSE — monotonic event IDs', () => {
  it('emitted events carry incrementing id fields', () => {
    _resetSseState();
    emitSSEEvent({ type: 'A' });
    emitSSEEvent({ type: 'B' });
    emitSSEEvent({ type: 'C' });

    const buf = _getSseBuffer();
    expect(buf).toHaveLength(3);
    expect(buf[0].id).toBe(1);
    expect(buf[1].id).toBe(2);
    expect(buf[2].id).toBe(3);
  });

  it('counter increments strictly across multiple calls', () => {
    emitSSEEvent({ x: 1 });
    emitSSEEvent({ x: 2 });
    expect(_getSseEventCounter()).toBe(2);
  });
});

describe('SSE — ring buffer bounded to SSE_BUFFER_SIZE', () => {
  it('evicts oldest events once the buffer exceeds 200 entries', () => {
    for (let i = 0; i < 205; i++) emitSSEEvent({ i });
    const buf = _getSseBuffer();
    expect(buf.length).toBe(200);
    // Oldest surviving id should be 6 (205 - 200 + 1)
    expect(buf[0].id).toBe(6);
  });
});

describe('SSE — reconnect replay via Last-Event-ID', () => {
  it('delivers no replay when Last-Event-ID is absent', async () => {
    emitSSEEvent({ type: 'X' });
    emitSSEEvent({ type: 'Y' });

    // Connect fresh — no last-event-id
    const frames = await collectSseFrames(`${baseUrl}/events`, {}, 0, 100);
    expect(frames).toHaveLength(0);
  });

  it('replays events with id > Last-Event-ID on reconnect', async () => {
    // Emit 3 events before reconnect
    emitSSEEvent({ type: 'first' });   // id 1
    emitSSEEvent({ type: 'second' });  // id 2
    emitSSEEvent({ type: 'third' });   // id 3

    // Reconnect as if we received up to id 1 — expect ids 2 and 3
    const frames = await collectSseFrames(
      `${baseUrl}/events`,
      { 'Last-Event-ID': '1' },
      2
    );

    expect(frames).toHaveLength(2);
    expect(frames[0]).toContain('id: 2');
    expect(frames[1]).toContain('id: 3');
  });

  it('replays all buffered events when Last-Event-ID is 0', async () => {
    emitSSEEvent({ type: 'A' }); // id 1
    emitSSEEvent({ type: 'B' }); // id 2

    const frames = await collectSseFrames(
      `${baseUrl}/events`,
      { 'Last-Event-ID': '0' },
      2
    );

    expect(frames).toHaveLength(2);
    expect(frames[0]).toContain('id: 1');
    expect(frames[1]).toContain('id: 2');
  });

  it('sends SSE headers on /events', async () => {
    const frames = await collectSseFrames(`${baseUrl}/events`, {}, 0, 80);
    // We can't easily check headers via collectSseFrames, but we can verify
    // the response by making a raw request
    await new Promise<void>((resolve, reject) => {
      const req = http.get(`${baseUrl}/events`, (res) => {
        expect(res.headers['content-type']).toContain('text/event-stream');
        req.destroy();
        resolve();
      });
      req.on('error', (err) => {
        if ((err as any).code === 'ECONNRESET') return resolve();
        reject(err);
      });
    });
  });
});
