import { Router, Request, Response, NextFunction } from 'express';
import prisma from '../db.js';
import redis from '../redis.js';
import { cacheMiddleware } from './cache-middleware.js';
import { etagMiddleware } from './etag-middleware.js';
import { strictRateLimiter } from './rate-limit-middleware.js';
import { badRequest, notFound, internalError } from './errors.js';
import { applyDecodedEvents } from '../poller.js';
import { collectMarketplaceEvents } from '../event-sync.js';
import {
  validateQuery,
  listingsQuerySchema,
  auctionsQuerySchema,
  offersQuerySchema,
  walletActivityQuerySchema,
  collectionsQuerySchema,
  statsQuerySchema,
  statsOverviewQuerySchema,
  statsDailyQuerySchema,
  statsTopQuerySchema,
} from './query-schemas.js';
import {
  getOverviewStats,
  getDailyStats,
  getTopCollections,
  getTopArtists,
} from '../stats.js';

// ── SSE registry ───────────────────────────────────────────────────────────────

const SSE_BUFFER_SIZE = 200;

interface SSEEvent {
  id: number;
  data: string;
}

let sseEventCounter = 0;
const sseBuffer: SSEEvent[] = [];
const sseClients: Map<Response, number> = new Map();

function nextSseId(): number {
  return ++sseEventCounter;
}

// Exposed for testing only
export function _getSseBuffer() { return sseBuffer; }
export function _getSseEventCounter() { return sseEventCounter; }
export function _resetSseState() {
  sseEventCounter = 0;
  sseBuffer.length = 0;
  sseClients.clear();
}

// SSE clients registry — keyed by Response, value is last-seen event ID

export function emitSSEEvent(event: any) {
  const id = nextSseId();
  const dataStr = JSON.stringify(event, (_k, v) => typeof v === 'bigint' ? v.toString() : v);
  const payload: SSEEvent = { id, data: dataStr };

  sseBuffer.push(payload);
  if (sseBuffer.length > SSE_BUFFER_SIZE) sseBuffer.shift();

  const frame = `id: ${id}\ndata: ${dataStr}\n\n`;
  for (const [client] of sseClients) {
    try {
      client.write(frame);
      sseClients.set(client, id);
    } catch {
      sseClients.delete(client);
    }
  }
}

export function closeSSEClients(): void {
    for (const [client] of sseClients) {
        try { client.end(); } catch { /* ignore */ }
    }
    sseClients.clear();
}

const router = Router();

router.use(etagMiddleware);

const CACHE_TTL_SECONDS = parseInt(process.env.REDIS_CACHE_TTL_SECONDS || '30');

async function getCached<T>(key: string, ttl: number, fetcher: () => Promise<T>): Promise<T> {
  try {
    const cached = await redis.get(key);
    if (cached) return JSON.parse(cached) as T;
  } catch {
    // Redis unavailable — fall through to DB
  }
  const result = await fetcher();
  try {
    await redis.set(key, JSON.stringify(result), { expiration: { type: 'EX', value: ttl } });
  } catch {
    // ignore cache write failures
  }
  return result;
}

const serialize = (obj: any) =>
  JSON.parse(JSON.stringify(obj, (key, value) =>
    typeof value === 'bigint' ? value.toString() : value
  ));

// ── GET /events (SSE) ─────────────────────────────────────────────────────────

router.get('/events', (req: Request, res: Response) => {
  res.setHeader('Content-Type', 'text/event-stream');
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Connection', 'keep-alive');
  res.flushHeaders();

  const lastEventId = req.headers['last-event-id'];
  const resumeFrom = lastEventId ? parseInt(String(lastEventId), 10) : null;

  sseClients.set(res, resumeFrom ?? sseEventCounter);

  if (resumeFrom !== null && !isNaN(resumeFrom)) {
    const missed = sseBuffer.filter(e => e.id > resumeFrom);
    for (const ev of missed) {
      try {
        res.write(`id: ${ev.id}\ndata: ${ev.data}\n\n`);
      } catch {
        break;
      }
    }
  }

  req.on('close', () => sseClients.delete(res));
});

// ── GET /listings ─────────────────────────────────────────────────────────────

router.get('/listings', validateQuery(listingsQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { artist, owner, status, limit, offset, minPrice, maxPrice, search } =
    (req as any).validatedQuery;
  try {
    const where: any = {};
    if (artist) where.artist = artist;
    if (owner) where.owner = owner;
    if (status) where.status = status;

    if (minPrice !== undefined || maxPrice !== undefined) {
      where.price = {};
      if (minPrice !== undefined) where.price.gte = String(minPrice);
      if (maxPrice !== undefined) where.price.lte = String(maxPrice);
    }

    if (search) {
      where.OR = [
        { artist: { contains: search, mode: 'insensitive' } },
        { collection: { contains: search, mode: 'insensitive' } },
      ];
    }

    const take = limit || undefined;
    const skip = offset || undefined;

    const results = await prisma.listing.findMany({
      where,
      orderBy: { updatedAtLedger: 'desc' },
      take,
      skip,
    });

    if (take !== undefined || skip !== undefined) {
      const total = await prisma.listing.count({ where });
      return res.json({ listings: serialize(results), total });
    }

    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch listings'));
  }
});

// ── GET /listings/:id ─────────────────────────────────────────────────────────

router.get('/listings/:id', async (req: Request, res: Response, next: NextFunction) => {
  const { id } = req.params;
  try {
    const listing = await prisma.listing.findUnique({
      where: { listingId: BigInt(id as string) },
    });
    if (!listing) return next(notFound('Listing not found'));
    return res.json(serialize(listing));
  } catch (err) {
    next(internalError('Failed to fetch listing details'));
  }
});

// ── GET /listings/:id/history ─────────────────────────────────────────────────

router.get('/listings/:id/history', async (req: Request, res: Response, next: NextFunction) => {
  const id = req.params.id as string;
  if (!/^\d+$/.test(id)) {
    return next(badRequest('Invalid ID format'));
  }

  const limit  = Math.min(parseInt(String(req.query.limit  ?? '100'), 10) || 100, 500);
  const offset = Math.min(parseInt(String(req.query.offset ?? '0'),   10) || 0,   10000);

  try {
    const where = { listingId: BigInt(id) };
    const [results, total] = await Promise.all([
      prisma.marketplaceEvent.findMany({
        where,
        orderBy: { ledgerSequence: 'asc' },
        take: limit,
        skip: offset,
      }),
      prisma.marketplaceEvent.count({ where }),
    ]);
    res.json({ events: serialize(results), total });
  } catch (err) {
    next(internalError('Failed to fetch listing history'));
  }
});

// ── GET /auctions ─────────────────────────────────────────────────────────────

router.get('/auctions', validateQuery(auctionsQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { creator, status } = (req as any).validatedQuery;
  try {
    const where: any = {};
    if (creator) where.creator = creator;
    if (status) where.status = status;

    const results = await prisma.auction.findMany({
      where,
      orderBy: { updatedAtLedger: 'desc' },
    });
    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch auctions'));
  }
});

// ── GET /auctions/:id ─────────────────────────────────────────────────────────

router.get('/auctions/:id', async (req: Request, res: Response, next: NextFunction) => {
  const id = req.params.id as string;
  if (!/^\d+$/.test(id)) {
    return next(badRequest('Invalid ID format'));
  }
  try {
    const result = await prisma.auction.findUnique({
      where: { auctionId: BigInt(id) },
    });
    if (!result) return next(notFound('Auction not found'));
    res.json(serialize(result));
  } catch (err) {
    next(internalError('Failed to fetch auction'));
  }
});

// ── GET /offers ───────────────────────────────────────────────────────────────

router.get('/offers', validateQuery(offersQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { listing_id } = (req as any).validatedQuery;
  try {
    const where: any = {};
    if (listing_id) {
      where.listingId = BigInt(listing_id);
    }

    const results = await prisma.offer.findMany({
      where,
      orderBy: { updatedAtLedger: 'desc' },
    });
    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch offers'));
  }
});

// ── GET /activity/recent ──────────────────────────────────────────────────────

router.get('/activity/recent', cacheMiddleware(30), async (req: Request, res: Response, next: NextFunction) => {
  try {
    const results = await getCached('activity:recent', CACHE_TTL_SECONDS, () =>
      prisma.marketplaceEvent.findMany({
        take: 20,
        orderBy: { ledgerSequence: 'desc' },
      })
    );
    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch recent activity'));
  }
});

// ── GET /collections ──────────────────────────────────────────────────────────

router.get('/collections', cacheMiddleware(60), validateQuery(collectionsQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { kind, creator } = (req as any).validatedQuery;
  try {
    const where: any = {};
    if (kind)    where.kind    = kind;
    if (creator) where.creator = creator;
    const cacheKey = `collections:${kind ?? ''}:${creator ?? ''}`;
    const results = await getCached(cacheKey, CACHE_TTL_SECONDS, () =>
      prisma.collection.findMany({
        where,
        orderBy: { deployedAtLedger: 'desc' },
      })
    );
    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch collections'));
  }
});

// ── GET /creators/:address/collections ───────────────────────────────────────

router.get('/creators/:address/collections', async (req: Request, res: Response, next: NextFunction) => {
  const { address } = req.params;
  try {
    const results = await prisma.collection.findMany({
      where: { creator: address as string },
      orderBy: { deployedAtLedger: 'desc' },
    });
    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch creator collections'));
  }
});

// ── GET /wallets/:address/activity ────────────────────────────────────────────

router.get('/wallets/:address/activity', strictRateLimiter, validateQuery(walletActivityQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const address = req.params.address as string;
  const { limit } = (req as any).validatedQuery;
  const take = Math.min(limit ?? 50, 200);
  try {
    const jsonKeys = ['buyer', 'artist', 'offerer', 'bidder', 'winner', 'creator'];
    const fromJson = jsonKeys.map((path) => ({
      data: { path: [path], equals: address },
    }));

    const events = await prisma.marketplaceEvent.findMany({
      where: {
        OR: [{ actor: address }, ...fromJson],
      },
      orderBy: { ledgerSequence: 'desc' },
      take,
    });

    res.json(serialize(events));
  } catch (err) {
    next(internalError('Failed to fetch wallet activity'));
  }
});

// ── GET /wallets/:address/royalty-stats ───────────────────────────────────────

router.get('/wallets/:address/royalty-stats', strictRateLimiter, async (req: Request, res: Response, next: NextFunction) => {
  const { address } = req.params;
  try {
    const sold = await prisma.listing.findMany({
      where: {
        originalCreator: address as string,
        status: 'Sold',
        NOT: { artist: address as string },
      },
      select: {
        listingId: true,
        price: true,
        royaltyBps: true,
        updatedAtLedger: true,
      },
    });

    let totalEarned = 0;
    for (const row of sold) {
      const p = Number(row.price);
      totalEarned += (p * row.royaltyBps) / 10000;
    }

    const lastSale = sold.reduce<(typeof sold)[0] | null>((latest, row) => {
      if (!latest || row.updatedAtLedger > latest.updatedAtLedger) return row;
      return latest;
    }, null);

    res.json({
      totalEarned: totalEarned.toFixed(7),
      payoutCount: sold.length,
      lastPayout: lastSale ? lastSale.updatedAtLedger * 1000 : 0,
    });
  } catch (err) {
    next(internalError('Failed to fetch royalty stats'));
  }
});

// ── GET /stats ────────────────────────────────────────────────────────────────

router.get('/stats', validateQuery(statsQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { from, to, range } = (req as any).validatedQuery;
  try {
    let dateFrom: Date | undefined;
    let dateTo: Date | undefined;

    if (range) {
      const now = new Date();
      dateTo = now;
      if (range === 'day') {
        dateFrom = new Date(now.getTime() - 24 * 60 * 60 * 1000);
      } else if (range === 'week') {
        dateFrom = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
      } else {
        dateFrom = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);
      }
    } else {
      if (from) {
        dateFrom = new Date(from as string);
        if (isNaN(dateFrom.getTime())) {
          return next(badRequest('Invalid from date format. Use ISO 8601.'));
        }
      }
      if (to) {
        dateTo = new Date(to as string);
        if (isNaN(dateTo.getTime())) {
          return next(badRequest('Invalid to date format. Use ISO 8601.'));
        }
      }
    }

    const eventTimeFilter: any = {};
    if (dateFrom) eventTimeFilter.gte = dateFrom;
    if (dateTo)   eventTimeFilter.lte = dateTo;
    const hasTimeFilter = Object.keys(eventTimeFilter).length > 0;

    const totalListings = await prisma.listing.count();
    const activeListings = await prisma.listing.count({ where: { status: 'Active' } });

    const volumeResult = await prisma.listing.aggregate({
      _sum: { price: true },
      where: { status: 'Sold' },
    });
    const totalVolume = volumeResult._sum.price?.toString() ?? '0';

    const userFilter: any = hasTimeFilter ? { ledgerTimestamp: eventTimeFilter } : {};
    const distinctActors = await prisma.marketplaceEvent.findMany({
      where: userFilter,
      select: { actor: true },
      distinct: ['actor'],
    });
    const activeUsers = distinctActors.length;

    const totalEvents = await prisma.marketplaceEvent.count({ where: userFilter });

    const salesFilter: any = { eventType: 'ARTWORK_SOLD' };
    if (hasTimeFilter) salesFilter.ledgerTimestamp = eventTimeFilter;
    const totalSales = await prisma.marketplaceEvent.count({ where: salesFilter });

    res.json({
      totalListings,
      activeListings,
      totalVolume,
      activeUsers,
      totalEvents,
      totalSales,
      ...(hasTimeFilter && {
        timeRange: {
          from: dateFrom?.toISOString() ?? null,
          to: dateTo?.toISOString() ?? null,
        },
      }),
    });
  } catch (err) {
    next(internalError('Failed to fetch stats'));
  }
});

// GET /events — Server-Sent Events stream with keep-alive heartbeats
router.get('/events', (req: Request, res: Response) => {
    // Check connection limit
    if (sseClients.size >= MAX_SSE_CONNECTIONS) {
        return res.status(503).json({ error: 'Too many SSE connections' });
    }

    // Setup SSE headers
    res.setHeader('Content-Type', 'text/event-stream');
    res.setHeader('Cache-Control', 'no-cache');
    res.setHeader('Connection', 'keep-alive');
    res.setHeader('Access-Control-Allow-Origin', '*');

    // Setup heartbeat
    setupSSEHeartbeat(res);

    // Send initial connection message
    res.write(`data: ${JSON.stringify({ type: 'CONNECTED' })}\n\n`);

    // Cleanup on disconnect
    res.on('close', () => {
        cleanupSSEClient(res);
    });

    res.on('error', () => {
        cleanupSSEClient(res);
    });
});

// ── GET /artists/:address/metrics ─────────────────────────────────────────────
// Returns mints-over-time, volume-over-time, and conversion rate aggregates
// for a given artist, scoped by an optional ?range=day|week|month query param.

router.get('/artists/:address/metrics', cacheMiddleware(60), async (req: Request, res: Response, next: NextFunction) => {
  const address = req.params.address as string;
  const range = req.query.range as string | undefined;

  const now = new Date();
  let dateFrom: Date | undefined;
  if (range === 'day')   dateFrom = new Date(now.getTime() - 24 * 60 * 60 * 1000);
  else if (range === 'week')  dateFrom = new Date(now.getTime() - 7  * 24 * 60 * 60 * 1000);
  else if (range === 'month') dateFrom = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);

  try {
    const timeWhere = dateFrom ? { createdAt: { gte: dateFrom } } : {};

    // Total listings created by artist in range (proxy for mints)
    const totalListings = await prisma.listing.count({
      where: { artist: address, ...timeWhere },
    });

    // Sales (sold listings)
    const totalSales = await prisma.listing.count({
      where: { artist: address, status: 'Sold', ...timeWhere },
    });

    // Volume (sum of sold listing prices in range)
    const volumeResult = await prisma.listing.aggregate({
      _sum: { price: true },
      where: { artist: address, status: 'Sold', ...timeWhere },
    });
    const totalVolume = volumeResult._sum.price?.toString() ?? '0';

    // Unique buyers
    const soldListings = await prisma.listing.findMany({
      where: { artist: address, status: 'Sold', owner: { not: null }, ...timeWhere },
      select: { owner: true },
    });
    const uniqueBuyers = new Set(soldListings.map((l) => l.owner)).size;

    // Conversion rate: sales / listings (0 if no listings)
    const conversionRate = totalListings > 0
      ? Number((totalSales / totalListings).toFixed(4))
      : 0;

    // Mints over time: group sold ARTWORK_SOLD events by day
    const soldEvents = await prisma.marketplaceEvent.findMany({
      where: {
        actor: address,
        eventType: 'ARTWORK_SOLD',
        ...(dateFrom ? { ledgerTimestamp: { gte: dateFrom } } : {}),
      },
      select: { ledgerTimestamp: true },
      orderBy: { ledgerTimestamp: 'asc' },
    });

    // Bucket events by ISO date (YYYY-MM-DD)
    const salesByDay: Record<string, number> = {};
    for (const ev of soldEvents) {
      const day = ev.ledgerTimestamp.toISOString().slice(0, 10);
      salesByDay[day] = (salesByDay[day] ?? 0) + 1;
    }
    const salesTimeline = Object.entries(salesByDay).map(([date, count]) => ({ date, count }));

    res.json({
      address,
      range: range ?? 'all',
      totalListings,
      totalSales,
      totalVolume,
      uniqueBuyers,
      conversionRate,
      salesTimeline,
    });
  } catch (err) {
    next(internalError('Failed to fetch artist metrics'));
  }
});

// ── GET /stats/overview ───────────────────────────────────────────────────────

const STATS_CACHE_TTL = 300; // 5 minutes

router.get('/stats/overview', validateQuery(statsOverviewQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  try {
    const result = await getCached('stats:overview', STATS_CACHE_TTL, getOverviewStats);
    res.json(serialize(result));
  } catch (err) {
    next(internalError('Failed to fetch overview stats'));
  }
});

// ── GET /stats/daily ──────────────────────────────────────────────────────────

const MAX_DAILY_RANGE_DAYS = 365;
const MAX_DAILY_QUERY_DAYS = 90;

router.get('/stats/daily', validateQuery(statsDailyQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { from, to } = (req as any).validatedQuery;

  const fromDate = new Date(from as string);
  const toDate = new Date(to as string);

  if (isNaN(fromDate.getTime())) {
    return next(badRequest('Invalid `from` date — use ISO 8601 format (e.g. 2024-01-01)'));
  }
  if (isNaN(toDate.getTime())) {
    return next(badRequest('Invalid `to` date — use ISO 8601 format (e.g. 2024-03-31)'));
  }
  if (fromDate > toDate) {
    return next(badRequest('`from` must be before `to`'));
  }

  const diffDays = Math.ceil((toDate.getTime() - fromDate.getTime()) / (1000 * 60 * 60 * 24));
  if (diffDays > MAX_DAILY_RANGE_DAYS) {
    return next(badRequest(`Date range exceeds maximum of ${MAX_DAILY_RANGE_DAYS} days`));
  }

  try {
    const cacheKey = `stats:daily:${from}:${to}`;
    const result = await getCached(cacheKey, STATS_CACHE_TTL, () =>
      getDailyStats(fromDate, toDate)
    );
    res.json(serialize(result));
  } catch (err) {
    next(internalError('Failed to fetch daily stats'));
  }
});

// ── GET /stats/top-collections ────────────────────────────────────────────────

router.get('/stats/top-collections', validateQuery(statsTopQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { limit } = (req as any).validatedQuery;
  const take = Math.min(limit ?? 10, 100);
  try {
    const cacheKey = `stats:top-collections:${take}`;
    const result = await getCached(cacheKey, STATS_CACHE_TTL, () => getTopCollections(take));
    res.json(serialize(result));
  } catch (err) {
    next(internalError('Failed to fetch top collections'));
  }
});

// ── GET /stats/top-artists ────────────────────────────────────────────────────

router.get('/stats/top-artists', validateQuery(statsTopQuerySchema), async (req: Request, res: Response, next: NextFunction) => {
  const { limit } = (req as any).validatedQuery;
  const take = Math.min(limit ?? 10, 100);
  try {
    const cacheKey = `stats:top-artists:${take}`;
    const result = await getCached(cacheKey, STATS_CACHE_TTL, () => getTopArtists(take));
    res.json(serialize(result));
  } catch (err) {
    next(internalError('Failed to fetch top artists'));
  }
});

// ── POST /admin/reprocess-ledger/:sequence ────────────────────────────────────
// Re-fetches and re-processes all contract events for a specific ledger.
// Safe to call multiple times — idempotent via eventHash deduplication.

const RPC_URL = process.env.STELLAR_RPC_URL || 'https://soroban-testnet.stellar.org';

router.post('/admin/reprocess-ledger/:sequence', strictRateLimiter, async (req: Request, res: Response, next: NextFunction) => {
  const seq = req.params.sequence;
  if (!/^\d+$/.test(seq)) {
    return next(badRequest('sequence must be a positive integer'));
  }
  const ledger = parseInt(seq, 10);

  try {
    const contractIds = [
      process.env.MARKETPLACE_CONTRACT_ID,
      process.env.LAUNCHPAD_CONTRACT_ID,
    ].filter(Boolean) as string[];

    if (contractIds.length === 0) {
      return next(badRequest('No contract IDs configured'));
    }

    const { rpc } = await import('@stellar/stellar-sdk');
    const server = new rpc.Server(RPC_URL);

    const events = await collectMarketplaceEvents(server, contractIds, ledger, ledger);

    const inserted = await prisma.$transaction((tx: Parameters<typeof applyDecodedEvents>[1]) =>
      applyDecodedEvents(events, tx)
    );

    res.json({
      ledger,
      fetched: events.length,
      inserted: inserted.length,
      duplicatesSkipped: events.length - inserted.length,
    });
  } catch (err) {
    next(internalError('Failed to reprocess ledger'));
  }
});

export default router;
