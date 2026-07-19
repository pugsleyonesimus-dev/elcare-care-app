import client from 'prom-client';
import express from 'express';
import { logger } from './logger.js';

// Enable default metrics (CPU, memory, etc.)
client.collectDefaultMetrics();

// Custom Metrics
export const latestLedgerProcessedGauge = new client.Gauge({
  name: 'indexer_latest_ledger_processed',
  help: 'The sequence number of the latest ledger processed by the indexer',
});

export const networkLatestLedgerGauge = new client.Gauge({
  name: 'indexer_network_latest_ledger',
  help: 'The sequence number of the latest ledger on the Stellar network',
});

export const syncLatencyGauge = new client.Gauge({
  name: 'indexer_sync_latency_ledgers',
  help: 'The difference between the latest network ledger and the processed ledger',
});

export const rpcRetryExhaustedCounter = new client.Counter({
  name: 'indexer_rpc_retry_exhausted_total',
  help: 'Total number of times RPC retries were exhausted, indicating sustained failures',
  labelNames: ['operation'],
});

export const decodeErrorsCounter = new client.Counter({
  name: 'indexer_decode_errors_total',
  help: 'Total number of XDR event decode errors encountered during sync',
});

export const duplicateEventsCounter = new client.Counter({
  name: 'elcarehub_duplicate_events_total',
  help: 'Total number of duplicate on-chain events skipped during idempotent processing',
});

export const httpRequestDurationMicroseconds = new client.Histogram({
  name: 'http_request_duration_seconds',
  help: 'Duration of HTTP requests in seconds',
  labelNames: ['method', 'route', 'status'],
  buckets: [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10],
});

// Request logging middleware
export function requestLogger(req: express.Request, res: express.Response, next: express.NextFunction) {
  const startTime = Date.now();

  res.on('finish', () => {
    const latency = Date.now() - startTime;
    const statusClass = res.statusCode < 400 ? '2xx/3xx' : res.statusCode < 500 ? '4xx' : '5xx';
    
    // Skip logging for health checks and metrics
    if (req.path !== '/health' && req.path !== '/metrics' && req.path !== '/readyz') {
      console.log(
        `${req.method} ${req.path} ${res.statusCode} ${latency}ms`
      );
    }
  });

  next();
}

// Middleware to track HTTP response times
export function metricsMiddleware(req: express.Request, res: express.Response, next: express.NextFunction) {
  const start = process.hrtime();
  
  res.on('finish', () => {
    const duration = process.hrtime(start);
    const durationInSeconds = duration[0] + duration[1] / 1e9;
    
    // Normalize route to avoid high-cardinality issues
    let route = req.baseUrl + (req.route ? req.route.path : req.path);
    if (!route || route === '') {
      route = req.path;
    }
    
    httpRequestDurationMicroseconds.labels(
      req.method,
      route,
      res.statusCode.toString()
    ).observe(durationInSeconds);
  });
  
  next();
}

// ── Keeper metrics ────────────────────────────────────────────────────────────
//
// entry_point label values: "expire_listing" | "finalize_auction" | "reclaim_offer"
// outcome      label values: "succeeded" | "failed" | "skipped" | "dry_run"

/** Total keeper action attempts, labelled by entry point and final outcome. */
export const keeperActionsTotal = new client.Counter({
  name: 'keeper_actions_total',
  help: 'Total number of keeper maintenance actions attempted, by entry point and outcome',
  labelNames: ['entry_point', 'outcome'],
});

/** Total XLM fees spent (in stroops) by the keeper, labelled by entry point. */
export const keeperFeesSpentStroops = new client.Counter({
  name: 'keeper_fees_spent_stroops_total',
  help: 'Cumulative transaction fees paid by the keeper in stroops, by entry point',
  labelNames: ['entry_point'],
});

/** Number of times the daily fee budget was exhausted, halting the cycle. */
export const keeperBudgetExhaustedTotal = new client.Counter({
  name: 'keeper_budget_exhausted_total',
  help: 'Number of times the keeper halted because the daily fee budget was exhausted',
});

/** Gauge set to 1 when the daily fee budget is currently exhausted, 0 otherwise. */
export const keeperBudgetExhaustedGauge = new client.Gauge({
  name: 'keeper_budget_exhausted',
  help: '1 when the keeper daily fee budget is currently exhausted, 0 otherwise',
});

/** Number of simulation failures (RPC-level, not contract reverts), by entry point. */
export const keeperSimulationFailuresTotal = new client.Counter({
  name: 'keeper_simulation_failures_total',
  help: 'Number of simulateTransaction failures (RPC errors, not contract reverts)',
  labelNames: ['entry_point'],
});

/** Duration of each full keeper sweep cycle in seconds. */
export const keeperCycleDurationSeconds = new client.Histogram({
  name: 'keeper_cycle_duration_seconds',
  help: 'Duration of a complete keeper sweep cycle in seconds',
  buckets: [0.1, 0.5, 1, 5, 10, 30, 60, 120],
});

/** How many candidates were discovered in the last sweep, by type. */
export const keeperCandidatesDiscovered = new client.Gauge({
  name: 'keeper_candidates_discovered',
  help: 'Number of actionable candidates discovered in the most recent sweep, by target type',
  labelNames: ['target_type'],
});

/** Number of fee-bump escalations triggered, by entry point. */
export const keeperFeeBumpsTotal = new client.Counter({
  name: 'keeper_fee_bumps_total',
  help: 'Number of fee-bump resubmissions triggered due to timeout or fee errors',
  labelNames: ['entry_point'],
});

// ── Backfill / gap-repair metrics ─────────────────────────────────────────────

/** Number of Open LedgerGap rows currently in the DB (set each gap-repair cycle). */
export const openGapsGauge = new client.Gauge({
  name: 'indexer_open_ledger_gaps',
  help: 'Number of LedgerGap rows currently in Open status',
});

/** Total ledgers covered by open gaps (sum of toLedger - fromLedger + 1). */
export const openGapLedgersTotalGauge = new client.Gauge({
  name: 'indexer_open_ledger_gap_ledgers_total',
  help: 'Total number of ledgers covered by all Open LedgerGap rows',
});

/** Total gap rows created, labelled by source. */
export const gapsCreatedTotal = new client.Counter({
  name: 'indexer_ledger_gaps_created_total',
  help: 'Total LedgerGap rows created, by source (rpc_window_skip | reorg | manual)',
  labelNames: ['source'],
});

/** Total BackfillJob outcomes, labelled by terminal status. */
export const backfillJobsTotal = new client.Counter({
  name: 'indexer_backfill_jobs_total',
  help: 'Total BackfillJob completions, by final status (Completed | Failed | Cancelled)',
  labelNames: ['status'],
});

/** Duration of a complete BackfillJob run in seconds. */
export const backfillDurationSeconds = new client.Histogram({
  name: 'indexer_backfill_duration_seconds',
  help: 'Wall-clock duration of a BackfillJob from Running to terminal state',
  buckets: [1, 5, 15, 30, 60, 120, 300, 600, 1800],
});

/** Ledgers processed per backfill batch (useful for sizing BACKFILL_BATCH_SIZE). */
export const backfillBatchLedgers = new client.Histogram({
  name: 'indexer_backfill_batch_ledgers',
  help: 'Number of ledgers processed in each backfill batch',
  buckets: [100, 500, 1000, 2500, 5000, 10000],
});

/** Events inserted per backfill batch. */
export const backfillBatchInserted = new client.Histogram({
  name: 'indexer_backfill_batch_inserted_events',
  help: 'Number of events inserted in each backfill batch',
  buckets: [0, 1, 10, 50, 100, 500, 1000, 5000],
});

/** Number of concurrent advisory-lock contentions (two workers raced for same job). */
export const backfillLockContentions = new client.Counter({
  name: 'indexer_backfill_lock_contentions_total',
  help: 'Number of times a BackfillJob advisory lock was already held by another worker',
});

// ── Expose metrics handler ────────────────────────────────────────────────────

export async function handleMetrics(req: express.Request, res: express.Response) {
  try {
    res.set('Content-Type', client.register.contentType);
    res.end(await client.register.metrics());
  } catch (err) {
    logger.error('Failed to retrieve metrics', { err });
    res.status(500).end('Failed to retrieve metrics');
  }
}
