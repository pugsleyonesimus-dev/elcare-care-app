import { stalledGauge } from './metrics.js';

export const STALL_THRESHOLD_MS = parseInt(process.env.STALL_THRESHOLD_MS || '60000');

let lastProgressAt = 0;
let stallTimer: ReturnType<typeof setTimeout> | null = null;

export function recordProgress(): void {
  lastProgressAt = Date.now();
  stalledGauge.set(0);
  if (stallTimer) clearTimeout(stallTimer);
  stallTimer = setTimeout(() => stalledGauge.set(1), STALL_THRESHOLD_MS);
}

export function isStalled(): boolean {
  if (lastProgressAt === 0) return false;
  return Date.now() - lastProgressAt > STALL_THRESHOLD_MS;
}

export function resetStallStateForTest(): void {
  lastProgressAt = 0;
  if (stallTimer) { clearTimeout(stallTimer); stallTimer = null; }
}
