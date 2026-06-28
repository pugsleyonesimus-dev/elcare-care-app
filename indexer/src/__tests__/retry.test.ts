import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRpcRetryExhaustedCounter = vi.hoisted(() => ({ inc: vi.fn() }));

vi.mock('../metrics.js', () => ({
  rpcRetryExhaustedCounter: mockRpcRetryExhaustedCounter,
  decodeErrorsCounter: { inc: vi.fn() },
  latestLedgerProcessedGauge: { set: vi.fn() },
  networkLatestLedgerGauge: { set: vi.fn() },
  syncLatencyGauge: { set: vi.fn() },
}));

import { withRetry } from '../retry.js';

describe('withRetry', () => {
  beforeEach(() => vi.clearAllMocks());

  it('returns result immediately on first success', async () => {
    const fn = vi.fn().mockResolvedValue('success');
    const result = await withRetry(fn, { maxAttempts: 3, baseDelayMs: 0, jitterMs: 0 });
    expect(result).toBe('success');
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('retries after a transient failure and returns on second attempt', async () => {
    const fn = vi.fn()
      .mockRejectedValueOnce(new Error('transient'))
      .mockResolvedValue('recovered');

    const result = await withRetry(fn, { maxAttempts: 3, baseDelayMs: 0, jitterMs: 0 });
    expect(result).toBe('recovered');
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it('retries the configured number of times before giving up', async () => {
    const fn = vi.fn().mockRejectedValue(new Error('persistent'));

    await expect(
      withRetry(fn, { maxAttempts: 4, baseDelayMs: 0, jitterMs: 0 })
    ).rejects.toThrow('persistent');

    expect(fn).toHaveBeenCalledTimes(4);
  });

  it('increments rpcRetryExhaustedCounter with the operation label on exhaustion', async () => {
    const fn = vi.fn().mockRejectedValue(new Error('rpc down'));

    await withRetry(fn, {
      maxAttempts: 2,
      baseDelayMs: 0,
      jitterMs: 0,
      operation: 'getLatestLedger',
    }).catch(() => {});

    expect(mockRpcRetryExhaustedCounter.inc).toHaveBeenCalledOnce();
    expect(mockRpcRetryExhaustedCounter.inc).toHaveBeenCalledWith({ operation: 'getLatestLedger' });
  });

  it('does NOT increment the counter when the function eventually succeeds', async () => {
    const fn = vi.fn()
      .mockRejectedValueOnce(new Error('blip'))
      .mockResolvedValue('ok');

    await withRetry(fn, { maxAttempts: 3, baseDelayMs: 0, jitterMs: 0 });
    expect(mockRpcRetryExhaustedCounter.inc).not.toHaveBeenCalled();
  });

  it('re-throws the last error so the caller can handle it', async () => {
    const err = new Error('root cause');
    const fn = vi.fn().mockRejectedValue(err);

    await expect(
      withRetry(fn, { maxAttempts: 2, baseDelayMs: 0, jitterMs: 0 })
    ).rejects.toBe(err);
  });

  it('uses default operation label when none is provided', async () => {
    const fn = vi.fn().mockRejectedValue(new Error('err'));
    await withRetry(fn, { maxAttempts: 1, baseDelayMs: 0, jitterMs: 0 }).catch(() => {});
    expect(mockRpcRetryExhaustedCounter.inc).toHaveBeenCalledWith({ operation: 'rpc' });
  });
});
