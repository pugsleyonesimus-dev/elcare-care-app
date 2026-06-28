import { describe, it, expect, vi } from 'vitest';
import express from 'express';
import request from 'supertest';
import {
  latestLedgerProcessedGauge,
  networkLatestLedgerGauge,
  syncLatencyGauge,
  metricsMiddleware,
  handleMetrics,
} from '../metrics';

// We can construct a minimal Express app to verify the middleware and handler
const app = express();
app.use(metricsMiddleware);
app.get('/metrics', handleMetrics);
app.get('/test', (req, res) => {
  res.status(200).json({ test: 'ok' });
});

describe('Prometheus Metrics API & Middleware', () => {
  it('exposes a valid /metrics endpoint', async () => {
    const res = await request(app)
      .get('/metrics')
      .expect(200);

    expect(res.headers['content-type']).toContain('text/plain');
    expect(res.text).toContain('indexer_latest_ledger_processed');
    expect(res.text).toContain('indexer_network_latest_ledger');
    expect(res.text).toContain('indexer_sync_latency_ledgers');
    expect(res.text).toContain('http_request_duration_seconds');
  });

  it('records metrics for standard HTTP calls', async () => {
    // Send a request to a standard endpoint to trigger metrics collection
    await request(app)
      .get('/test')
      .expect(200);

    const res = await request(app)
      .get('/metrics')
      .expect(200);

    expect(res.text).toContain('method="GET"');
    expect(res.text).toContain('route="/test"');
    expect(res.text).toContain('status="200"');
  });

  it('exports the latest ledger gauges with their current values', async () => {
    latestLedgerProcessedGauge.set(321);
    networkLatestLedgerGauge.set(654);
    syncLatencyGauge.set(333);

    const res = await request(app)
      .get('/metrics')
      .expect(200);

    expect(res.text).toContain('indexer_latest_ledger_processed 321');
    expect(res.text).toContain('indexer_network_latest_ledger 654');
    expect(res.text).toContain('indexer_sync_latency_ledgers 333');
  });

  it('reflects updated gauge values after each simulated poll cycle', async () => {
    // Cycle 1: far behind
    latestLedgerProcessedGauge.set(1000);
    networkLatestLedgerGauge.set(5000);
    syncLatencyGauge.set(4000); // 5000 - 1000

    let res = await request(app).get('/metrics').expect(200);
    expect(res.text).toContain('indexer_latest_ledger_processed 1000');
    expect(res.text).toContain('indexer_network_latest_ledger 5000');
    expect(res.text).toContain('indexer_sync_latency_ledgers 4000');

    // Cycle 2: caught up partially
    latestLedgerProcessedGauge.set(3000);
    networkLatestLedgerGauge.set(5100);
    syncLatencyGauge.set(2100); // 5100 - 3000

    res = await request(app).get('/metrics').expect(200);
    expect(res.text).toContain('indexer_latest_ledger_processed 3000');
    expect(res.text).toContain('indexer_network_latest_ledger 5100');
    expect(res.text).toContain('indexer_sync_latency_ledgers 2100');

    // Cycle 3: fully synced
    latestLedgerProcessedGauge.set(5200);
    networkLatestLedgerGauge.set(5200);
    syncLatencyGauge.set(0);

    res = await request(app).get('/metrics').expect(200);
    expect(res.text).toContain('indexer_latest_ledger_processed 5200');
    expect(res.text).toContain('indexer_network_latest_ledger 5200');
    expect(res.text).toContain('indexer_sync_latency_ledgers 0');
  });
});
