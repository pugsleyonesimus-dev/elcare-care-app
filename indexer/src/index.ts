import express from 'express';
import cors from 'cors';
import dotenv from 'dotenv';
import rateLimit from 'express-rate-limit';
import routes from './api/routes.js';
import { startPolling } from './poller.js';
import { isStalled } from './stall.js';
import { logger } from './logger.js';
import { rateLimiter } from './api/rate-limit-middleware.js';
import { metricsMiddleware, handleMetrics } from './metrics.js';
import prisma from './db.js';

dotenv.config();

// Fail fast — refuse to start if the contract ID is missing.
if (!process.env.MARKETPLACE_CONTRACT_ID) {
  logger.error('MARKETPLACE_CONTRACT_ID is not set — exiting');
  process.exit(1);
}

const app = express();
const PORT = process.env.PORT || 4000;

const limiter = rateLimit({
    windowMs: 60 * 1000,
    limit: 100,
    standardHeaders: 'draft-8',
    legacyHeaders: false,
    message: { error: 'Too many requests, please try again after a minute.' },
});

app.use(cors({
    origin: process.env.NODE_ENV === 'production'
        ? (process.env.CORS_ORIGIN || '').split(',').map(o => o.trim()).filter(Boolean)
        : true,
    credentials: true,
}));
app.use(express.json());
app.use(limiter);

// Track response time metrics for all routes
app.use(metricsMiddleware);

// Expose /metrics for Prometheus scrapers (bypass global rate limit)
app.get('/metrics', handleMetrics);

// Apply rate limiting to all other routes
app.use(rateLimiter);

// API Routes
app.use('/', routes);

// Health check
app.get('/health', (req: express.Request, res: express.Response) => {
    res.json({ status: 'ok' });
});

// Readiness probe — returns 503 until the indexer has processed at least one ledger,
// or if the indexer has stalled (no progress for STALL_THRESHOLD_MS).
app.get('/readyz', async (req: express.Request, res: express.Response) => {
    if (isStalled()) {
        return res.status(503).json({ status: 'stalled', reason: 'Indexer not advancing' });
    }
    const state = await prisma.syncState.findUnique({ where: { id: 1 } });
    if (state && state.lastLedger > 0) {
        res.json({ status: 'ready', lastLedger: state.lastLedger });
    } else {
        res.status(503).json({ status: 'not_ready', reason: 'No ledgers indexed yet' });
    }
});

// Start the server
app.listen(PORT, () => {
    logger.info('Indexer API listening', { port: PORT });
    
    // Start the background polling loop
    startPolling().catch((err) => {
        logger.error('Fatal error in poller', { err });
        process.exit(1);
    });
});
