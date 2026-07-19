/**
 * Serves GET /openapi.json and GET /docs (Swagger UI).
 *
 * The spec is built once from the Zod-based registry in openapi.ts and cached
 * in memory. Swagger UI assets are served from the swagger-ui-dist package so
 * there is no external CDN dependency.
 */
import { Router, Request, Response } from 'express';
import swaggerUi from 'swagger-ui-express';
import { buildOpenApiDocument } from './openapi.js';

const router = Router();

// Build once at module load — safe because the registry is fully populated by
// the time this module is imported.
const spec = buildOpenApiDocument();

// ── GET /openapi.json ─────────────────────────────────────────────────────────

router.get('/openapi.json', (_req: Request, res: Response) => {
  res.setHeader('Content-Type', 'application/json');
  res.json(spec);
});

// ── GET /docs ─────────────────────────────────────────────────────────────────

router.use('/docs', swaggerUi.serve);
router.get('/docs', swaggerUi.setup(spec, {
  customSiteTitle: 'ElcareHub Indexer API Docs',
  swaggerOptions: {
    url: '/openapi.json',
    persistAuthorization: true,
  },
}));

export default router;
