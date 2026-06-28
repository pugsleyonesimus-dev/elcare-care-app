import { rpcRetryExhaustedCounter } from './metrics.js';

export interface RetryOptions {
  maxAttempts?: number;
  baseDelayMs?: number;
  maxDelayMs?: number;
  jitterMs?: number;
  operation?: string;
}

/**
 * Retries `fn` up to `maxAttempts` times with exponential backoff and optional jitter.
 * On exhaustion, increments `rpcRetryExhaustedCounter` and re-throws the last error.
 * Set `jitterMs: 0` or `baseDelayMs: 0` in tests to avoid real waits.
 */
export async function withRetry<T>(fn: () => Promise<T>, options: RetryOptions = {}): Promise<T> {
  const {
    maxAttempts = 5,
    baseDelayMs = 500,
    maxDelayMs = 30_000,
    jitterMs = 500,
    operation = 'rpc',
  } = options;

  let lastErr: unknown;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (err) {
      lastErr = err;
      if (attempt === maxAttempts) break;
      const jitter = jitterMs > 0 ? Math.floor(Math.random() * jitterMs) : 0;
      const delay = Math.min(baseDelayMs * Math.pow(2, attempt - 1) + jitter, maxDelayMs);
      console.warn({
        msg: `[withRetry] ${operation} failed — attempt ${attempt}/${maxAttempts}, retrying in ${delay}ms`,
        error: err instanceof Error ? err.message : String(err),
      });
      await new Promise((r) => setTimeout(r, delay));
    }
  }

  rpcRetryExhaustedCounter.inc({ operation });
  console.error({
    msg: `[withRetry] ${operation} exhausted all ${maxAttempts} attempts — sustained failure`,
    error: lastErr instanceof Error ? lastErr.message : String(lastErr),
    stack: lastErr instanceof Error ? lastErr.stack : undefined,
  });
  throw lastErr;
}
