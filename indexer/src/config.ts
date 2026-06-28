function parsePositiveInt(name: string, raw: string | undefined, defaultVal: number): number {
  if (raw === undefined || raw === '') return defaultVal;
  const n = Number(raw);
  if (!Number.isInteger(n) || n <= 0) {
    throw new Error(`Config error: ${name} must be a positive integer, got "${raw}"`);
  }
  return n;
}

export function loadConfig() {
  return {
    pollIntervalMs: parsePositiveInt('POLL_INTERVAL_MS', process.env.POLL_INTERVAL_MS, 5000),
    maxLedgersPerCycle: parsePositiveInt('MAX_LEDGERS_PER_CYCLE', process.env.MAX_LEDGERS_PER_CYCLE, 1000),
  };
}

export type Config = ReturnType<typeof loadConfig>;
