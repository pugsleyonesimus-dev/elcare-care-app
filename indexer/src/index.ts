import { initSentry, Sentry } from './sentry.js';
import express from 'express';
import cors from 'cors';
import compression from 'compression';
import dotenv from 'dotenv';
import rateLimit from 'express-rate-limit';
import routes, { closeSSEClients } from './api/routes.js';
import { startPolling, registerShutdownHook } from './poller.js';
import { rateLimiter } from './api/rate-limit-middleware.js';
import { metricsMiddleware, handleMetrics } from './metrics.js';
import { errorHandler } from './api/errors.js';
import { startReconciler } from './reconciler.js';
import { validateRequiredEnv } from './config.js';
import { startStatsScheduler } from './stats-scheduler.js';
import prisma from './db.js';

dotenv.config();

// Initialise Sentry before the Express app is constructed so it can instrument
// framework integrations automatically. No-op when SENTRY_DSN is not set.
initSentry();

// Load OpenAPI spec
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const openapiPath = path.join(__dirname, '..', 'openapi.yaml');
const openapiFile = fs.readFileSync(openapiPath, 'utf8');
const swaggerDoc = yaml.parse(openapiFile);

// Fail fast — refuse to start if any required environment variable is missing.
try {
  validateRequiredEnv();
} catch (err) {
  console.error((err as Error).message);
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
app.use(compression());
app.use(express.json());

// Apply global baseline rate limiter to all public endpoints
app.use(globalRateLimiter);

// Request logging and metrics
app.use(requestLogger);
app.use(metricsMiddleware);

// Expose /metrics for Prometheus scrapers (bypass rate limit via skip in globalRateLimiter)
app.get('/metrics', handleMetrics);

// Apply standard rate limiting for fallback
app.use(rateLimiter);

// API Routes
app.use('/', routes);

// Sentry error handler must be registered before the custom error handler so
// that it receives the error object before it is serialised into an HTTP response.
Sentry.setupExpressErrorHandler(app);

// Central error handler — must be registered after all routes
app.use(errorHandler);

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

    // Check first ledger indexed
    try {
        const state = await prisma.syncState.findUnique({ where: { id: 1 } });
        if (!state || state.lastLedger === 0) {
            reasons.push('No ledgers indexed yet');
        }
    } catch (err) {
        reasons.push('Failed to check sync state');
    }

    if (reasons.length > 0) {
        return res.status(503).json({ status: 'not_ready', reasons });
    }

    const state = await prisma.syncState.findUnique({ where: { id: 1 } });
    res.json({ status: 'ready', lastLedger: state?.lastLedger });
});

// Start the server
const httpServer = app.listen(PORT, () => {
    console.log(`Indexer API listening on http://localhost:${PORT}`);

    // Start the background polling loop
    startPolling().catch((err) => {
        logger.error('Fatal error in poller', { err });
        process.exit(1);
    });

    // Start the periodic reconciliation job (non-fatal if it fails)
    startReconciler().catch((err) => {
        console.error('[Reconciler] Failed to start:', err);
    });

    // Start the hourly materialized-view refresh for analytics stats
    const stopStatsScheduler = startStatsScheduler();
    registerShutdownHook(async () => { stopStatsScheduler(); });
});

// Register HTTP server and SSE cleanup so gracefulShutdown() in poller closes them too.
registerShutdownHook(() => new Promise<void>((resolve) => {
    closeSSEClients();
    httpServer.close(() => resolve());
}));
