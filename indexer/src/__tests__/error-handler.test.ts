import { describe, it, expect, vi, beforeEach } from 'vitest';
import express, { Request, Response, NextFunction } from 'express';
import request from 'supertest';
import { errorHandler, ApiError, badRequest, notFound, internalError, ErrorCode } from '../api/errors';

function buildApp(handler: (req: Request, res: Response, next: NextFunction) => void) {
  const app = express();
  app.use(express.json());
  app.get('/test', handler);
  app.use(errorHandler);
  return app;
}

describe('error shape', () => {
  it('400 — wraps badRequest in standard envelope', async () => {
    const app = buildApp((_req, _res, next) => next(badRequest('Param x is required')));
    const res = await request(app).get('/test');
    expect(res.status).toBe(400);
    expect(res.body).toEqual({
      error: { code: ErrorCode.BAD_REQUEST, message: 'Param x is required' },
    });
  });

  it('404 — wraps notFound in standard envelope', async () => {
    const app = buildApp((_req, _res, next) => next(notFound('Resource not found')));
    const res = await request(app).get('/test');
    expect(res.status).toBe(404);
    expect(res.body).toEqual({
      error: { code: ErrorCode.NOT_FOUND, message: 'Resource not found' },
    });
  });

  it('500 — wraps unknown error in standard envelope without leaking internals', async () => {
    const app = buildApp((_req, _res, next) => next(new Error('secret db password')));
    const res = await request(app).get('/test');
    expect(res.status).toBe(500);
    expect(res.body.error.code).toBe(ErrorCode.INTERNAL);
    // Must not expose internal details
    expect(JSON.stringify(res.body)).not.toContain('secret db password');
    expect(JSON.stringify(res.body)).not.toContain('stack');
  });

  it('500 via internalError helper — standard envelope', async () => {
    const app = buildApp((_req, _res, next) => next(internalError('Failed to fetch data')));
    const res = await request(app).get('/test');
    expect(res.status).toBe(500);
    expect(res.body).toEqual({
      error: { code: ErrorCode.INTERNAL, message: 'Failed to fetch data' },
    });
  });

  it('ApiError preserves statusCode and code', () => {
    const err = new ApiError(422, ErrorCode.BAD_REQUEST, 'Unprocessable');
    expect(err.statusCode).toBe(422);
    expect(err.code).toBe(ErrorCode.BAD_REQUEST);
    expect(err.message).toBe('Unprocessable');
  });

  it('all error responses share { error: { code, message } } shape', async () => {
    const cases: Array<[string, () => ApiError]> = [
      ['400', () => badRequest('bad')],
      ['404', () => notFound('not found')],
      ['500', () => internalError()],
    ];
    for (const [label, factory] of cases) {
      const app = buildApp((_req, _res, next) => next(factory()));
      const res = await request(app).get('/test');
      expect(res.body, `shape for ${label}`).toHaveProperty('error');
      expect(res.body.error, `error.code for ${label}`).toHaveProperty('code');
      expect(res.body.error, `error.message for ${label}`).toHaveProperty('message');
    }
  });
});
