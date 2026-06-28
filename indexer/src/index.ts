import express from 'express';
import cors from 'cors';
import dotenv from 'dotenv';
import routes from './api/routes.js';
import { startPolling } from './poller.js';
import { globalRateLimiter, rateLimiter } from './api/rate-limit-middleware.js';
import { metricsMiddleware, handleMetrics } from './metrics.js';
import prisma from './db.js';

dotenv.config();

// Fail fast — refuse to start if the contract ID is missing.
if (!process.env.MARKETPLACE_CONTRACT_ID) {
  console.error('[Startup] MARKETPLACE_CONTRACT_ID is not set. Exiting.');
  process.exit(1);
}

const app = express();
const PORT = process.env.PORT || 4000;

app.use(cors({
    origin: process.env.NODE_ENV === 'production'
        ? (process.env.CORS_ORIGIN || '').split(',').map(o => o.trim()).filter(Boolean)
        : true,
    credentials: true,
}));
app.use(express.json());

// Apply global baseline rate limiter to all public endpoints
app.use(globalRateLimiter);

// Track response time metrics for all routes
app.use(metricsMiddleware);

// Expose /metrics for Prometheus scrapers (bypass rate limit via skip in globalRateLimiter)
app.get('/metrics', handleMetrics);

// Apply standard rate limiting for fallback
app.use(rateLimiter);

// API Routes
app.use('/', routes);

// Health check
app.get('/health', (req: express.Request, res: express.Response) => {
    res.json({ status: 'ok' });
});

// Readiness probe — returns 503 until the indexer has processed at least one ledger.
// Use this for Kubernetes readinessProbe / Docker HEALTHCHECK so traffic is only
// routed once the indexer is actually synced and serving real data.
app.get('/readyz', async (req: express.Request, res: express.Response) => {
    const state = await prisma.syncState.findUnique({ where: { id: 1 } });
    if (state && state.lastLedger > 0) {
        res.json({ status: 'ready', lastLedger: state.lastLedger });
    } else {
        res.status(503).json({ status: 'not_ready', reason: 'No ledgers indexed yet' });
    }
});

// Start the server
app.listen(PORT, () => {
    console.log(`Indexer API listening on http://localhost:${PORT}`);
    
    // Start the background polling loop
    startPolling().catch((err) => {
        console.error('Fatal error in poller:', err);
        process.exit(1);
    });
});
