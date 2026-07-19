/**
 * CLI script: generates openapi.json from the Zod-based registry and writes it
 * to the project root.
 *
 * Usage:
 *   npx tsx src/generate-openapi.ts
 *
 * The CI `check-openapi` job runs this script and diffs the output against the
 * committed openapi.json, failing if they differ.
 */
import { writeFileSync } from 'fs';
import { resolve } from 'path';
import { buildOpenApiDocument } from './api/openapi.js';

const doc = buildOpenApiDocument();
// Always write next to package.json (one level above src/)
const outPath = resolve(process.cwd(), 'openapi.json');

writeFileSync(outPath, JSON.stringify(doc, null, 2) + '\n', 'utf8');
console.log(`✓ OpenAPI spec written to ${outPath}`);
