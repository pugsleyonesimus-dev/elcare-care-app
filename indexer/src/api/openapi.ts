import {
  OpenAPIRegistry,
  OpenApiGeneratorV3,
  extendZodWithOpenApi,
} from '@asteasolutions/zod-to-openapi';
import { z } from 'zod';

// Extend Zod with .openapi() so schemas can carry metadata
extendZodWithOpenApi(z);

export const registry = new OpenAPIRegistry();

// ── Reusable field types ──────────────────────────────────────────────────────

/** BigInt IDs are serialised as decimal strings by the route serialize() helper */
const bigIntString = z.string().openapi({ example: '1' });

const isoDateTime = z.string().openapi({ format: 'date-time', example: '2024-01-15T12:00:00.000Z' });

// ── Response schemas ──────────────────────────────────────────────────────────

export const ListingSchema = registry.register(
  'Listing',
  z.object({
    listingId:       bigIntString.openapi({ description: 'Unique listing ID' }),
    artist:          z.string().openapi({ example: 'GABC...XYZ', description: 'Seller / artist Stellar address' }),
    owner:           z.string().nullable().openapi({ example: 'GABC...XYZ', description: 'Current owner address, null before first sale' }),
    price:           z.string().openapi({ example: '10.0000000', description: 'Listing price as a decimal string (7 dp)' }),
    currency:        z.string().openapi({ example: 'XLM' }),
    collection:      z.string().openapi({ example: 'CABC...DEF', description: 'Collection contract address' }),
    nftTokenId:      bigIntString.openapi({ description: 'NFT token ID within the collection' }),
    token:           z.string().openapi({ example: 'CABC...DEF', description: 'Payment token contract address' }),
    status:          z.enum(['Active', 'Sold', 'Cancelled', 'Auction']).openapi({ example: 'Active' }),
    recipients:      z.unknown().nullable().openapi({ description: 'Royalty recipients (JSON)' }),
    createdAtLedger: z.number().int().openapi({ example: 50000000, description: 'Ledger sequence when the listing was created' }),
    updatedAtLedger: z.number().int().openapi({ example: 50000001, description: 'Ledger sequence of the last state change' }),
    createdAt:       isoDateTime.openapi({ description: 'Wall-clock creation time' }),
    updatedAt:       isoDateTime.openapi({ description: 'Wall-clock last-update time' }),
  }).openapi('Listing'),
);

export const AuctionSchema = registry.register(
  'Auction',
  z.object({
    auctionId:       bigIntString.openapi({ description: 'Unique auction ID' }),
    creator:         z.string().openapi({ example: 'GABC...XYZ', description: 'Auction creator Stellar address' }),
    collection:      z.string().openapi({ example: 'CABC...DEF' }),
    nftTokenId:      bigIntString,
    token:           z.string().openapi({ example: 'CABC...DEF', description: 'Payment token contract address' }),
    reservePrice:    z.string().openapi({ example: '5.0000000', description: 'Minimum acceptable bid' }),
    highestBid:      z.string().openapi({ example: '7.5000000', description: 'Current highest bid amount' }),
    highestBidder:   z.string().nullable().openapi({ example: 'GABC...XYZ', description: 'Address of the current highest bidder' }),
    endTime:         bigIntString.openapi({ description: 'Auction end time as Unix timestamp string' }),
    status:          z.enum(['Active', 'Finalized', 'Cancelled']).openapi({ example: 'Active' }),
    recipients:      z.unknown().nullable().openapi({ description: 'Royalty recipients (JSON)' }),
    createdAtLedger: z.number().int().openapi({ example: 50000000 }),
    updatedAtLedger: z.number().int().openapi({ example: 50000001 }),
    createdAt:       isoDateTime,
    updatedAt:       isoDateTime,
  }).openapi('Auction'),
);

export const OfferSchema = registry.register(
  'Offer',
  z.object({
    offerId:         bigIntString.openapi({ description: 'Unique offer ID' }),
    listingId:       bigIntString.openapi({ description: 'Listing this offer targets' }),
    offerer:         z.string().openapi({ example: 'GABC...XYZ', description: 'Address that placed the offer' }),
    amount:          z.string().openapi({ example: '8.0000000', description: 'Offered amount as a decimal string' }),
    token:           z.string().openapi({ example: 'CABC...DEF', description: 'Payment token contract address' }),
    status:          z.enum(['Pending', 'Accepted', 'Rejected', 'Withdrawn']).openapi({ example: 'Pending' }),
    createdAtLedger: z.number().int().openapi({ example: 50000000 }),
    updatedAtLedger: z.number().int().openapi({ example: 50000001 }),
    createdAt:       isoDateTime,
    updatedAt:       isoDateTime,
  }).openapi('Offer'),
);

export const MarketplaceEventSchema = registry.register(
  'MarketplaceEvent',
  z.object({
    id:               z.number().int().openapi({ example: 1, description: 'Auto-increment PK' }),
    listingId:        bigIntString.nullable().openapi({ description: 'Related listing ID, if applicable' }),
    eventType:        z.string().openapi({ example: 'ARTWORK_SOLD', description: 'Contract event type (e.g. LISTING_CREATED, ARTWORK_SOLD, BID_PLACED)' }),
    actor:            z.string().openapi({ example: 'GABC...XYZ', description: 'Primary address that triggered the event' }),
    data:             z.record(z.string(), z.unknown()).openapi({ description: 'Raw decoded event payload (varies by eventType)' }),
    ledgerSequence:   z.number().int().openapi({ example: 50000001 }),
    ledgerTimestamp:  isoDateTime.openapi({ description: 'Ledger close time' }),
  }).openapi('MarketplaceEvent'),
);

export const CollectionSchema = registry.register(
  'Collection',
  z.object({
    id:               z.number().int().openapi({ example: 1 }),
    contractAddress:  z.string().openapi({ example: 'CABC...DEF', description: 'Deployed collection contract address' }),
    kind:             z.string().openapi({
      example: 'normal_721',
      description: 'Collection type: normal_721 | normal_1155 | lazy_721 | lazy_1155',
    }),
    creator:          z.string().openapi({ example: 'GABC...XYZ' }),
    name:             z.string().nullable().openapi({ example: 'My NFT Collection' }),
    symbol:           z.string().nullable().openapi({ example: 'MNC' }),
    deployedAtLedger: z.number().int().openapi({ example: 50000000 }),
    createdAt:        isoDateTime,
  }).openapi('Collection'),
);

export const RoyaltyStatsSchema = registry.register(
  'RoyaltyStats',
  z.object({
    totalEarned: z.string().openapi({ example: '12.3456789', description: 'Total royalty earnings as a decimal string (7 dp)' }),
    payoutCount: z.number().int().openapi({ example: 5, description: 'Number of secondary sales that generated royalties' }),
    lastPayout:  z.number().int().openapi({ example: 1705320000000, description: 'Unix timestamp (ms) of the most recent royalty payout, 0 if none' }),
  }).openapi('RoyaltyStats'),
);

export const StatsSchema = registry.register(
  'Stats',
  z.object({
    totalListings:  z.number().int().openapi({ example: 1000 }),
    activeListings: z.number().int().openapi({ example: 250 }),
    totalVolume:    z.string().openapi({ example: '50000.0000000', description: 'Cumulative sold volume as a decimal string' }),
    activeUsers:    z.number().int().openapi({ example: 100, description: 'Distinct actors in the requested time window' }),
    totalEvents:    z.number().int().openapi({ example: 5000 }),
    totalSales:     z.number().int().openapi({ example: 300 }),
    timeRange: z
      .object({
        from: z.string().nullable().openapi({ format: 'date-time' }),
        to:   z.string().nullable().openapi({ format: 'date-time' }),
      })
      .optional()
      .openapi({ description: 'Echoed back when a time filter was applied' }),
  }).openapi('Stats'),
);

export const ArtistMetricsSchema = registry.register(
  'ArtistMetrics',
  z.object({
    address:        z.string().openapi({ example: 'GABC...XYZ' }),
    range:          z.string().openapi({ example: 'week', description: 'Time window used ("all" when no range param was given)' }),
    totalListings:  z.number().int().openapi({ example: 50 }),
    totalSales:     z.number().int().openapi({ example: 20 }),
    totalVolume:    z.string().openapi({ example: '200.0000000' }),
    uniqueBuyers:   z.number().int().openapi({ example: 15 }),
    conversionRate: z.number().openapi({ example: 0.4, description: 'Sales / listings ratio (0 – 1)' }),
    salesTimeline: z.array(
      z.object({
        date:  z.string().openapi({ example: '2024-01-15' }),
        count: z.number().int().openapi({ example: 3 }),
      }),
    ).openapi({ description: 'Daily sales counts for the time window' }),
  }).openapi('ArtistMetrics'),
);

export const ErrorResponseSchema = registry.register(
  'ErrorResponse',
  z.object({
    error: z.object({
      code:    z.string().openapi({ example: 'NOT_FOUND' }),
      message: z.string().openapi({ example: 'Listing not found' }),
    }),
  }).openapi('ErrorResponse'),
);

// ── Shared parameter helpers ──────────────────────────────────────────────────

function pathParam(name: string, description: string) {
  return {
    name,
    in: 'path' as const,
    required: true,
    schema: { type: 'string' as const },
    description,
  };
}

function queryParam(name: string, schema: z.ZodTypeAny, description?: string) {
  return registry.registerParameter(name, schema.openapi({ param: { name, in: 'query' }, description }));
}

// ── Route registrations ───────────────────────────────────────────────────────

// GET /listings
registry.registerPath({
  method: 'get',
  path: '/listings',
  tags: ['Listings'],
  summary: 'List all listings',
  description: 'Returns listings with optional filters. When `limit` or `offset` are supplied the response wraps results in a pagination envelope.',
  request: {
    query: z.object({
      artist:   z.string().optional().openapi({ description: 'Filter by artist Stellar address' }),
      owner:    z.string().optional().openapi({ description: 'Filter by current owner address' }),
      status:   z.enum(['Active', 'Sold', 'Cancelled', 'Auction']).optional().openapi({ description: 'Filter by listing status' }),
      search:   z.string().optional().openapi({ description: 'Full-text search on artist address or collection' }),
      minPrice: z.coerce.number().nonnegative().optional().openapi({ description: 'Minimum price (inclusive)' }),
      maxPrice: z.coerce.number().nonnegative().optional().openapi({ description: 'Maximum price (inclusive)' }),
      limit:    z.coerce.number().int().nonnegative().max(1000).optional().openapi({ description: 'Max results to return (max 1000)' }),
      offset:   z.coerce.number().int().nonnegative().max(10000).optional().openapi({ description: 'Number of results to skip' }),
    }),
  },
  responses: {
    200: {
      description: 'Listing array, or paginated envelope when limit/offset are used',
      content: {
        'application/json': {
          schema: z.union([
            z.array(ListingSchema),
            z.object({ listings: z.array(ListingSchema), total: z.number().int() }),
          ]),
        },
      },
    },
  },
});

// GET /listings/:id
registry.registerPath({
  method: 'get',
  path: '/listings/{id}',
  tags: ['Listings'],
  summary: 'Get a single listing',
  request: { params: z.object({ id: z.string().openapi({ description: 'Listing ID' }) }) },
  responses: {
    200: { description: 'Listing details', content: { 'application/json': { schema: ListingSchema } } },
    404: { description: 'Listing not found', content: { 'application/json': { schema: ErrorResponseSchema } } },
  },
});

// GET /listings/:id/history
registry.registerPath({
  method: 'get',
  path: '/listings/{id}/history',
  tags: ['Listings'],
  summary: 'Get on-chain event history for a listing',
  request: { params: z.object({ id: z.string().openapi({ description: 'Listing ID' }) }) },
  responses: {
    200: {
      description: 'Ordered list of marketplace events for this listing',
      content: { 'application/json': { schema: z.array(MarketplaceEventSchema) } },
    },
    400: { description: 'Invalid ID format', content: { 'application/json': { schema: ErrorResponseSchema } } },
  },
});

// GET /auctions
registry.registerPath({
  method: 'get',
  path: '/auctions',
  tags: ['Auctions'],
  summary: 'List all auctions',
  request: {
    query: z.object({
      creator: z.string().optional().openapi({ description: 'Filter by creator address' }),
      status:  z.enum(['Active', 'Finalized', 'Cancelled']).optional().openapi({ description: 'Filter by auction status' }),
    }),
  },
  responses: {
    200: { description: 'Auction list', content: { 'application/json': { schema: z.array(AuctionSchema) } } },
  },
});

// GET /auctions/:id
registry.registerPath({
  method: 'get',
  path: '/auctions/{id}',
  tags: ['Auctions'],
  summary: 'Get a single auction',
  request: { params: z.object({ id: z.string().openapi({ description: 'Auction ID' }) }) },
  responses: {
    200: { description: 'Auction details', content: { 'application/json': { schema: AuctionSchema } } },
    400: { description: 'Invalid ID format', content: { 'application/json': { schema: ErrorResponseSchema } } },
    404: { description: 'Auction not found', content: { 'application/json': { schema: ErrorResponseSchema } } },
  },
});

// GET /offers
registry.registerPath({
  method: 'get',
  path: '/offers',
  tags: ['Offers'],
  summary: 'List offers',
  description: 'Returns all offers. Use `listing_id` to filter to a specific listing.',
  request: {
    query: z.object({
      listing_id: z.string().regex(/^\d+$/).optional().openapi({ description: 'Filter by listing ID (numeric string)' }),
    }),
  },
  responses: {
    200: { description: 'Offer list', content: { 'application/json': { schema: z.array(OfferSchema) } } },
  },
});

// GET /activity/recent
registry.registerPath({
  method: 'get',
  path: '/activity/recent',
  tags: ['Activity'],
  summary: 'Get the 20 most recent marketplace events',
  description: 'Cached for 30 s. Returns the latest cross-marketplace activity feed.',
  responses: {
    200: { description: 'Recent events', content: { 'application/json': { schema: z.array(MarketplaceEventSchema) } } },
  },
});

// GET /collections
registry.registerPath({
  method: 'get',
  path: '/collections',
  tags: ['Collections'],
  summary: 'List all deployed collections',
  description: 'Cached for 60 s.',
  request: {
    query: z.object({
      kind:    z.string().optional().openapi({ description: 'Filter by collection type (normal_721, normal_1155, lazy_721, lazy_1155)' }),
      creator: z.string().optional().openapi({ description: 'Filter by creator address' }),
    }),
  },
  responses: {
    200: { description: 'Collection list', content: { 'application/json': { schema: z.array(CollectionSchema) } } },
  },
});

// GET /creators/:address/collections
registry.registerPath({
  method: 'get',
  path: '/creators/{address}/collections',
  tags: ['Collections'],
  summary: 'Get all collections deployed by a creator',
  request: { params: z.object({ address: z.string().openapi({ description: 'Creator Stellar address' }) }) },
  responses: {
    200: { description: 'Collections by creator', content: { 'application/json': { schema: z.array(CollectionSchema) } } },
  },
});

// GET /wallets/:address/activity
registry.registerPath({
  method: 'get',
  path: '/wallets/{address}/activity',
  tags: ['Wallets'],
  summary: 'Get activity feed for a wallet',
  description: 'Returns events where the address is the `actor` or appears in the event JSON payload (buyer, artist, offerer, bidder, winner, creator). Rate-limited to 20 req/min.',
  request: {
    params: z.object({ address: z.string().openapi({ description: 'Wallet Stellar address' }) }),
    query:  z.object({
      limit: z.coerce.number().int().nonnegative().max(200).optional().openapi({ description: 'Max results (default 50, max 200)' }),
    }),
  },
  responses: {
    200: { description: 'Wallet event feed', content: { 'application/json': { schema: z.array(MarketplaceEventSchema) } } },
  },
});

// GET /wallets/:address/royalty-stats
registry.registerPath({
  method: 'get',
  path: '/wallets/{address}/royalty-stats',
  tags: ['Wallets'],
  summary: 'Get royalty earnings summary for an artist',
  description: 'Calculates total royalties earned from secondary sales. Rate-limited to 20 req/min.',
  request: { params: z.object({ address: z.string().openapi({ description: 'Artist Stellar address' }) }) },
  responses: {
    200: { description: 'Royalty statistics', content: { 'application/json': { schema: RoyaltyStatsSchema } } },
  },
});

// GET /stats
registry.registerPath({
  method: 'get',
  path: '/stats',
  tags: ['Stats'],
  summary: 'Get marketplace statistics',
  description: 'Aggregate counts and volumes. Supports an optional time window via `range` shorthand or explicit `from`/`to` ISO 8601 dates.',
  request: {
    query: z.object({
      range: z.enum(['day', 'week', 'month']).optional().openapi({ description: 'Shorthand time window (last 24 h / 7 d / 30 d)' }),
      from:  z.string().optional().openapi({ format: 'date-time', description: 'Window start (ISO 8601). Ignored when `range` is set.' }),
      to:    z.string().optional().openapi({ format: 'date-time', description: 'Window end (ISO 8601). Ignored when `range` is set.' }),
    }),
  },
  responses: {
    200: { description: 'Marketplace statistics', content: { 'application/json': { schema: StatsSchema } } },
    400: { description: 'Invalid date format', content: { 'application/json': { schema: ErrorResponseSchema } } },
  },
});

// GET /artists/:address/metrics
registry.registerPath({
  method: 'get',
  path: '/artists/{address}/metrics',
  tags: ['Artists'],
  summary: 'Get per-artist performance metrics',
  description: 'Returns sales volume, conversion rate, unique buyers, and a daily sales timeline. Cached for 60 s.',
  request: {
    params: z.object({ address: z.string().openapi({ description: 'Artist Stellar address' }) }),
    query:  z.object({
      range: z.enum(['day', 'week', 'month']).optional().openapi({ description: 'Time window (default: all time)' }),
    }),
  },
  responses: {
    200: { description: 'Artist metrics', content: { 'application/json': { schema: ArtistMetricsSchema } } },
  },
});

// GET /events (SSE)
registry.registerPath({
  method: 'get',
  path: '/events',
  tags: ['System'],
  summary: 'Server-Sent Events stream',
  description: 'Real-time event stream using SSE. Supports `Last-Event-Id` header for replay of up to the last 200 events. Returns 503 when the connection limit is reached.',
  responses: {
    200: {
      description: 'SSE stream (text/event-stream)',
      content: { 'text/event-stream': { schema: { type: 'string' } } },
    },
    503: { description: 'Too many SSE connections', content: { 'application/json': { schema: ErrorResponseSchema } } },
  },
});

// GET /health
registry.registerPath({
  method: 'get',
  path: '/health',
  tags: ['System'],
  summary: 'Liveness probe',
  responses: {
    200: {
      description: 'Service is alive',
      content: {
        'application/json': {
          schema: z.object({ status: z.literal('ok') }),
        },
      },
    },
  },
});

// GET /readyz
registry.registerPath({
  method: 'get',
  path: '/readyz',
  tags: ['System'],
  summary: 'Readiness probe',
  description: 'Returns 503 until at least one ledger has been indexed, or if the indexer has stalled.',
  responses: {
    200: {
      description: 'Service is ready',
      content: {
        'application/json': {
          schema: z.object({
            status:      z.literal('ready'),
            lastLedger:  z.number().int(),
          }),
        },
      },
    },
    503: {
      description: 'Service is not ready or stalled',
      content: {
        'application/json': {
          schema: z.object({
            status:  z.string().openapi({ example: 'not_ready' }),
            reasons: z.array(z.string()).optional(),
            reason:  z.string().optional(),
          }),
        },
      },
    },
  },
});

// GET /metrics
registry.registerPath({
  method: 'get',
  path: '/metrics',
  tags: ['System'],
  summary: 'Prometheus metrics',
  description: 'Exposes `http_request_duration_seconds`, `latest_ledger_processed`, `network_latest_ledger`, and `sync_latency_ledgers` metrics.',
  responses: {
    200: {
      description: 'Prometheus text format',
      content: { 'text/plain': { schema: { type: 'string' } } },
    },
  },
});

// ── Generator ─────────────────────────────────────────────────────────────────

/**
 * Build and return the complete OpenAPI 3.0 document.
 * Call once at startup; the result is stable and can be cached.
 */
export function buildOpenApiDocument() {
  const generator = new OpenApiGeneratorV3(registry.definitions);
  return generator.generateDocument({
    openapi: '3.0.0',
    info: {
      title: 'ElcareHub Indexer API',
      description:
        'Off-chain event indexer and REST API for the ElcareHub NFT marketplace on Stellar Soroban. ' +
        'All BigInt values (IDs, endTime) are serialised as decimal strings.',
      version: '1.0.0',
      contact: { name: 'ElcareHub', url: 'https://elcarehub.io' },
    },
    servers: [
      { url: 'http://localhost:4000', description: 'Local development' },
      { url: 'https://indexer.elcarehub.io', description: 'Production' },
    ],
  });
}
