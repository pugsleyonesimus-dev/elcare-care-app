/**
 * stats-scheduler.ts — schedules hourly refresh of the
 * daily_marketplace_stats materialized view.
 */

import { refreshDailyStats } from './stats.js';
import { logger } from './logger.js';

const REFRESH_INTERVAL_MS =
  parseInt(process.env.STATS_REFRESH_INTERVAL_MS || '3600000', 10); // 1 hour

let intervalHandle: ReturnType<typeof setInterval> | null = null;

async function runRefresh(): Promise<void> {
  try {
    await refreshDailyStats();
    logger.info('[StatsScheduler] daily_marketplace_stats refreshed');
  } catch (err) {
    logger.error('[StatsScheduler] Failed to refresh materialized view', { err });
  }
}

/**
 * Start the hourly materialized-view refresh job.
 * Returns a stop function so the caller can clean up during shutdown.
 */
export function startStatsScheduler(): () => void {
  // Run once immediately on start, then on the interval
  runRefresh();

  intervalHandle = setInterval(runRefresh, REFRESH_INTERVAL_MS);

  return () => {
    if (intervalHandle !== null) {
      clearInterval(intervalHandle);
      intervalHandle = null;
    }
  };
}
