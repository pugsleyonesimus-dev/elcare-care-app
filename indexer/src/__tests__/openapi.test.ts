import { describe, it, expect, vi, beforeEach } from 'vitest';
import express from 'express';
import request from 'supertest';
import swaggerUi from 'swagger-ui-express';
import yaml from 'yaml';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const mockPrisma = vi.hoisted(() => ({
  listing: {
    findMany: vi.fn(),
  },
}));

vi.mock('../db', () => ({ default: mockPrisma }));
vi.mock('../redis.js', () => ({ default: { get: vi.fn() } }));

describe('OpenAPI and Swagger Documentation', () => {
  let app: express.Application;
  let swaggerDoc: any;

  beforeEach(() => {
    const __filename = fileURLToPath(import.meta.url);
    const __dirname = path.dirname(__filename);
    const openapiPath = path.join(__dirname, '..', '..', 'openapi.yaml');
    
    const openapiFile = fs.readFileSync(openapiPath, 'utf8');
    swaggerDoc = yaml.parse(openapiFile);

    app = express();
    app.use(express.json());
    
    // Mock the routes endpoint
    app.get('/listings', (req, res) => {
      res.json([]);
    });

    // Swagger UI
    app.use('/docs', swaggerUi.serve, swaggerUi.setup(swaggerDoc));
    
    // OpenAPI spec endpoint
    app.get('/openapi.yaml', (req, res) => {
      res.type('text/yaml').send(openapiFile);
    });
  });

  it('should serve swagger UI docs', async () => {
    const res = await request(app).get('/docs/');

    expect(res.status).toBe(200);
    expect(res.text).toContain('swagger-ui');
  });

  it('should serve raw OpenAPI YAML', async () => {
    const res = await request(app).get('/openapi.yaml');

    expect(res.status).toBe(200);
    expect(res.type).toContain('text/yaml');
    expect(res.text).toContain('openapi: 3.0.0');
  });

  it('should have valid OpenAPI structure', () => {
    expect(swaggerDoc).toBeDefined();
    expect(swaggerDoc.openapi).toBe('3.0.0');
    expect(swaggerDoc.info).toBeDefined();
    expect(swaggerDoc.info.version).toBeDefined();
    expect(swaggerDoc.servers).toBeDefined();
    expect(swaggerDoc.paths).toBeDefined();
    expect(swaggerDoc.components).toBeDefined();
  });

  it('should document all listing endpoints', () => {
    expect(swaggerDoc.paths['/listings']).toBeDefined();
    expect(swaggerDoc.paths['/listings/{id}']).toBeDefined();
    expect(swaggerDoc.paths['/listings/{id}/history']).toBeDefined();
  });

  it('should document all auction endpoints', () => {
    expect(swaggerDoc.paths['/auctions']).toBeDefined();
    expect(swaggerDoc.paths['/auctions/{id}']).toBeDefined();
  });

  it('should document all collection endpoints', () => {
    expect(swaggerDoc.paths['/collections']).toBeDefined();
    expect(swaggerDoc.paths['/creators/{address}/collections']).toBeDefined();
  });

  it('should document wallet endpoints', () => {
    expect(swaggerDoc.paths['/wallets/{address}/activity']).toBeDefined();
    expect(swaggerDoc.paths['/wallets/{address}/royalty-stats']).toBeDefined();
  });

  it('should document system endpoints', () => {
    expect(swaggerDoc.paths['/health']).toBeDefined();
    expect(swaggerDoc.paths['/readyz']).toBeDefined();
    expect(swaggerDoc.paths['/metrics']).toBeDefined();
  });

  it('should have proper API info', () => {
    expect(swaggerDoc.info.title).toBeDefined();
    expect(swaggerDoc.info.description).toBeDefined();
    expect(swaggerDoc.info.version).toBe('1.0.0');
  });

  it('should define all response schemas', () => {
    expect(swaggerDoc.components.schemas.Listing).toBeDefined();
    expect(swaggerDoc.components.schemas.Auction).toBeDefined();
    expect(swaggerDoc.components.schemas.Offer).toBeDefined();
    expect(swaggerDoc.components.schemas.Collection).toBeDefined();
    expect(swaggerDoc.components.schemas.MarketplaceEvent).toBeDefined();
    expect(swaggerDoc.components.schemas.RoyaltyStats).toBeDefined();
    expect(swaggerDoc.components.schemas.Stats).toBeDefined();
  });

  it('should include parameter documentation for filters', () => {
    const listingsGet = swaggerDoc.paths['/listings'].get;
    expect(listingsGet.parameters).toBeDefined();
    
    const paramNames = listingsGet.parameters.map((p: any) => p.name);
    expect(paramNames).toContain('artist');
    expect(paramNames).toContain('status');
    expect(paramNames).toContain('minPrice');
    expect(paramNames).toContain('maxPrice');
  });

  it('should include proper response codes', () => {
    const listingsGet = swaggerDoc.paths['/listings'].get;
    expect(listingsGet.responses['200']).toBeDefined();
    
    const singleListing = swaggerDoc.paths['/listings/{id}'].get;
    expect(singleListing.responses['200']).toBeDefined();
    expect(singleListing.responses['404']).toBeDefined();
  });

  it('should tag endpoints for organization', () => {
    const listingsGet = swaggerDoc.paths['/listings'].get;
    expect(listingsGet.tags).toContain('Listings');
    
    const auctionsGet = swaggerDoc.paths['/auctions'].get;
    expect(auctionsGet.tags).toContain('Auctions');
  });

  it('should have proper server configurations', () => {
    expect(swaggerDoc.servers).toBeDefined();
    expect(swaggerDoc.servers.length).toBeGreaterThan(0);
    
    const serverUrls = swaggerDoc.servers.map((s: any) => s.url);
    expect(serverUrls).toContain('http://localhost:4000');
  });

  it('should document readyz readiness probe', () => {
    const readyzEndpoint = swaggerDoc.paths['/readyz'];
    expect(readyzEndpoint).toBeDefined();
    expect(readyzEndpoint.get).toBeDefined();
    expect(readyzEndpoint.get.responses['200']).toBeDefined();
    expect(readyzEndpoint.get.responses['503']).toBeDefined();
  });
});
