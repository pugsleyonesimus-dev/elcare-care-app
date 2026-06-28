import express from 'express';
import cors from 'cors';
import compression from 'compression';
import dotenv from 'dotenv';
import rateLimit from 'express-rate-limit';
import swaggerUi from 'swagger-ui-express';
import yaml from 'yaml';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import routes from './api/routes.js';
import { startPolling } from './poller.js';
import { rateLimiter } from './api/rate-limit-middleware.js';
import { metricsMiddleware, handleMetrics } from './metrics.js';
import prisma from './db.js';

dotenv.config();

// Load OpenAPI spec
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const openapiPath = path.join(__dirname, '..', 'openapi.yaml');
const openapiFile = fs.readFileSync(openapiPath, 'utf8');
const swaggerDoc = yaml.parse(openapiFile);

// Fail fast — refuse to start if the contract ID is missing.
if (!process.env.MARKETPLACE_CONTRACT_ID) {
  console.error('[Startup] MARKETPLACE_CONTRACT_ID is not set. Exiting.');
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
app.use(compression());
app.use(express.json());
app.use(limiter);

// Track response time metrics for all routes
app.use(metricsMiddleware);

// Expose /metrics for Prometheus scrapers (bypass global rate limit)
app.get('/metrics', handleMetrics);

// Serve OpenAPI docs
app.use('/docs', swaggerUi.serve, swaggerUi.setup(swaggerDoc, {
    swaggerOptions: {
        url: '/openapi.yaml',
    },
}));

// Serve raw OpenAPI spec
app.get('/openapi.yaml', (req: express.Request, res: express.Response) => {
    res.type('text/yaml').sendFile(openapiPath);
});

// Apply rate limiting to all other routes
app.use(rateLimiter);

// API Routes
app.use('/', routes);

// Health check
app.get('/health', (req: express.Request, res: express.Response) => {
    res.json({ status: 'ok' });
});

// Readiness probe — returns 503 until DB is reachable and at least one ledger is indexed.
app.get('/readyz', async (req: express.Request, res: express.Response) => {
    const reasons: string[] = [];

    // Check DB connectivity
    try {
        await prisma.syncState.findUnique({ where: { id: 1 } });
    } catch (err) {
        reasons.push('Database unreachable');
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
app.listen(PORT, () => {
    console.log(`Indexer API listening on http://localhost:${PORT}`);
    
    // Start the background polling loop
    startPolling().catch((err) => {
        console.error('Fatal error in poller:', err);
        process.exit(1);
    });
});
