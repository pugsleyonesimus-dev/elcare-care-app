import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Capture stdout writes to assert structured JSON log output
const writes: string[] = [];
const originalWrite = process.stdout.write.bind(process.stdout);

function captureStdout() {
  vi.spyOn(process.stdout, 'write').mockImplementation((chunk: any) => {
    writes.push(typeof chunk === 'string' ? chunk : chunk.toString());
    return true;
  });
}

function lastLog(): Record<string, unknown> {
  const raw = writes[writes.length - 1]?.trim();
  if (!raw) throw new Error('No log output captured');
  return JSON.parse(raw);
}

describe('logger', () => {
  beforeEach(() => {
    writes.length = 0;
    delete process.env.LOG_LEVEL;
    vi.resetModules();
    captureStdout();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    delete process.env.LOG_LEVEL;
  });

  it('emits valid JSON with level, time, and msg fields', async () => {
    const { logger } = await import('../logger');
    logger.info('hello world');

    const log = lastLog();
    expect(log.level).toBe('info');
    expect(typeof log.time).toBe('number');
    expect(log.msg).toBe('hello world');
  });

  it('includes extra fields alongside standard fields', async () => {
    const { logger } = await import('../logger');
    logger.warn('stall detected', { ledger: 500, cycleId: 'abc' });

    const log = lastLog();
    expect(log.level).toBe('warn');
    expect(log.msg).toBe('stall detected');
    expect(log.ledger).toBe(500);
    expect(log.cycleId).toBe('abc');
  });

  it('emits error level with correct level field', async () => {
    const { logger } = await import('../logger');
    logger.error('something broke', { err: 'details' });

    const log = lastLog();
    expect(log.level).toBe('error');
    expect(log.err).toBe('details');
  });

  it('suppresses messages below the configured LOG_LEVEL', async () => {
    process.env.LOG_LEVEL = 'warn';
    const { logger } = await import('../logger');
    logger.info('should be suppressed');
    logger.debug('also suppressed');

    expect(writes.filter((w) => w.trim()).length).toBe(0);
  });

  it('emits messages at or above the configured LOG_LEVEL', async () => {
    process.env.LOG_LEVEL = 'warn';
    const { logger } = await import('../logger');
    logger.warn('visible warn');
    logger.error('visible error');

    const lines = writes.filter((w) => w.trim()).map((w) => JSON.parse(w));
    expect(lines).toHaveLength(2);
    expect(lines[0].level).toBe('warn');
    expect(lines[1].level).toBe('error');
  });

  it('each log line is a self-contained JSON object terminated by a newline', async () => {
    const { logger } = await import('../logger');
    logger.info('line one');
    logger.info('line two');

    const lines = writes.join('').split('\n').filter(Boolean);
    expect(lines).toHaveLength(2);
    for (const line of lines) {
      expect(() => JSON.parse(line)).not.toThrow();
    }
  });
});
