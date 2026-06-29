import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { loadConfig, validateRequiredEnv } from '../config';

// ── loadConfig ────────────────────────────────────────────────────────────────

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

// ── validateRequiredEnv ───────────────────────────────────────────────────────

describe('validateRequiredEnv', () => {
  const REQUIRED: Record<string, string> = {
    DATABASE_URL: 'postgresql://localhost/test',
    MARKETPLACE_CONTRACT_ID: 'C_MARKETPLACE',
    REDIS_URL: 'redis://localhost:6379',
    STELLAR_RPC_URL: 'https://soroban-testnet.stellar.org',
    STELLAR_NETWORK: 'testnet',
  };

  const ORIGINAL = { ...process.env };

  beforeEach(() => {
    Object.entries(REQUIRED).forEach(([k, v]) => { process.env[k] = v; });
  });

  afterEach(() => {
    process.env = { ...ORIGINAL };
  });

  it('does not throw when all required variables are set', () => {
    expect(() => validateRequiredEnv()).not.toThrow();
  });

  it('throws when DATABASE_URL is missing', () => {
    delete process.env.DATABASE_URL;
    expect(() => validateRequiredEnv()).toThrow('DATABASE_URL');
  });

  it('throws when MARKETPLACE_CONTRACT_ID is missing', () => {
    delete process.env.MARKETPLACE_CONTRACT_ID;
    expect(() => validateRequiredEnv()).toThrow('MARKETPLACE_CONTRACT_ID');
  });

  it('throws when REDIS_URL is missing', () => {
    delete process.env.REDIS_URL;
    expect(() => validateRequiredEnv()).toThrow('REDIS_URL');
  });

  it('throws when STELLAR_RPC_URL is missing', () => {
    delete process.env.STELLAR_RPC_URL;
    expect(() => validateRequiredEnv()).toThrow('STELLAR_RPC_URL');
  });

  it('throws when STELLAR_NETWORK is missing', () => {
    delete process.env.STELLAR_NETWORK;
    expect(() => validateRequiredEnv()).toThrow('STELLAR_NETWORK');
  });

  it('throws an aggregated error listing all missing variables', () => {
    delete process.env.DATABASE_URL;
    delete process.env.MARKETPLACE_CONTRACT_ID;
    let caught: Error | null = null;
    try {
      validateRequiredEnv();
    } catch (e) {
      caught = e as Error;
    }
    expect(caught).not.toBeNull();
    expect(caught!.message).toContain('DATABASE_URL');
    expect(caught!.message).toContain('MARKETPLACE_CONTRACT_ID');
  });
});
