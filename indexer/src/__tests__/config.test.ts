import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { loadConfig } from '../config';

describe('loadConfig', () => {
  const ORIGINAL = { ...process.env };

  beforeEach(() => {
    delete process.env.POLL_INTERVAL_MS;
    delete process.env.MAX_LEDGERS_PER_CYCLE;
  });

  afterEach(() => {
    process.env = { ...ORIGINAL };
  });

  it('returns defaults when env vars are absent', () => {
    const config = loadConfig();
    expect(config.pollIntervalMs).toBe(5000);
    expect(config.maxLedgersPerCycle).toBe(1000);
  });

  it('parses valid POLL_INTERVAL_MS from env', () => {
    process.env.POLL_INTERVAL_MS = '3000';
    expect(loadConfig().pollIntervalMs).toBe(3000);
  });

  it('parses valid MAX_LEDGERS_PER_CYCLE from env', () => {
    process.env.MAX_LEDGERS_PER_CYCLE = '500';
    expect(loadConfig().maxLedgersPerCycle).toBe(500);
  });

  it('throws a descriptive error for non-numeric POLL_INTERVAL_MS', () => {
    process.env.POLL_INTERVAL_MS = 'abc';
    expect(() => loadConfig()).toThrow('POLL_INTERVAL_MS');
  });

  it('throws a descriptive error for zero POLL_INTERVAL_MS', () => {
    process.env.POLL_INTERVAL_MS = '0';
    expect(() => loadConfig()).toThrow('POLL_INTERVAL_MS');
  });

  it('throws a descriptive error for negative POLL_INTERVAL_MS', () => {
    process.env.POLL_INTERVAL_MS = '-500';
    expect(() => loadConfig()).toThrow('POLL_INTERVAL_MS');
  });

  it('throws a descriptive error for fractional POLL_INTERVAL_MS', () => {
    process.env.POLL_INTERVAL_MS = '1.5';
    expect(() => loadConfig()).toThrow('POLL_INTERVAL_MS');
  });

  it('throws a descriptive error for non-numeric MAX_LEDGERS_PER_CYCLE', () => {
    process.env.MAX_LEDGERS_PER_CYCLE = 'lots';
    expect(() => loadConfig()).toThrow('MAX_LEDGERS_PER_CYCLE');
  });

  it('throws a descriptive error for zero MAX_LEDGERS_PER_CYCLE', () => {
    process.env.MAX_LEDGERS_PER_CYCLE = '0';
    expect(() => loadConfig()).toThrow('MAX_LEDGERS_PER_CYCLE');
  });

  it('throws a descriptive error for negative MAX_LEDGERS_PER_CYCLE', () => {
    process.env.MAX_LEDGERS_PER_CYCLE = '-1';
    expect(() => loadConfig()).toThrow('MAX_LEDGERS_PER_CYCLE');
  });

  it('throws a descriptive error for fractional MAX_LEDGERS_PER_CYCLE', () => {
    process.env.MAX_LEDGERS_PER_CYCLE = '2.5';
    expect(() => loadConfig()).toThrow('MAX_LEDGERS_PER_CYCLE');
  });
});
