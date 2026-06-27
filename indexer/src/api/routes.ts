import { Router, Request, Response, NextFunction } from 'express';
import prisma from '../db.js';
import redis from '../redis.js';
import { cacheMiddleware } from './cache-middleware.js';
import { strictRateLimiter } from './rate-limit-middleware.js';
import { badRequest, notFound, internalError } from './errors.js';

// ── SSE registry ───────────────────────────────────────────────────────────────

const SSE_BUFFER_SIZE = 200;

interface SSEEvent {
  id: number;
  data: string;
}

let sseEventCounter = 0;
const sseBuffer: SSEEvent[] = [];
const sseClients: Map<Response, number> = new Map(); // client → last-sent id

function nextSseId(): number {
  return ++sseEventCounter;
}

export function emitSSEEvent(event: any) {
  const id = nextSseId();
  const dataStr = JSON.stringify(event, (_k, v) => typeof v === 'bigint' ? v.toString() : v);
  const payload: SSEEvent = { id, data: dataStr };

  // Append to ring buffer, drop oldest when full
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

const router = Router();

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

function normaliseGateway(gateway: string): string {
  return gateway.endsWith('/') ? gateway : `${gateway}/`;
}

// ── GET /events (SSE) ─────────────────────────────────────────────────────────

router.get('/events', (req: Request, res: Response) => {
  res.setHeader('Content-Type', 'text/event-stream');
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Connection', 'keep-alive');
  res.flushHeaders();

  const lastEventId = req.headers['last-event-id'];
  const resumeFrom = lastEventId ? parseInt(String(lastEventId), 10) : null;

  sseClients.set(res, resumeFrom ?? sseEventCounter);

  // Replay missed events
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

router.get('/listings', async (req: Request, res: Response, next: NextFunction) => {
  const { artist, owner, status, limit, offset, minPrice, maxPrice, search } = req.query;
  try {
    const where: any = {};
    if (artist) where.artist = artist as string;
    if (owner) where.owner = owner as string;
    if (status) where.status = status as string;

    if (minPrice || maxPrice) {
      where.price = {};
      if (minPrice) where.price.gte = minPrice as string;
      if (maxPrice) where.price.lte = maxPrice as string;
    }

    if (search) {
      const q = search as string;
      where.OR = [
        { artist: { contains: q, mode: 'insensitive' } },
        { collection: { contains: q, mode: 'insensitive' } },
      ];
    }

    const take = Math.max(0, Math.min(Number(limit || 0), 1000)) || undefined;
    const rawOffset = Number(offset || 0);
    const skip = Number.isFinite(rawOffset) && rawOffset > 0
      ? Math.min(rawOffset, 10_000)
      : undefined;

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
  try {
    const results = await prisma.marketplaceEvent.findMany({
      where: { listingId: BigInt(id) },
      orderBy: { ledgerSequence: 'asc' },
    });
    res.json(serialize(results));
  } catch (err) {
    next(internalError('Failed to fetch listing history'));
  }
});

// ── GET /auctions ─────────────────────────────────────────────────────────────

router.get('/auctions', async (req: Request, res: Response, next: NextFunction) => {
  const { creator, status } = req.query;
  try {
    const where: any = {};
    if (creator) where.creator = creator as string;
    if (status) where.status = status as string;

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

router.get('/offers', async (req: Request, res: Response, next: NextFunction) => {
  const { listing_id } = req.query;
  try {
    const where: any = {};
    if (listing_id) {
      if (!/^\d+$/.test(listing_id as string)) {
        return next(badRequest('Invalid listing_id format'));
      }
      where.listingId = BigInt(listing_id as string);
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

router.get('/collections', cacheMiddleware(60), async (req: Request, res: Response, next: NextFunction) => {
  const { kind, creator } = req.query;
  try {
    const where: any = {};
    if (kind)    where.kind    = kind as string;
    if (creator) where.creator = creator as string;
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

router.get('/wallets/:address/activity', strictRateLimiter, async (req: Request, res: Response, next: NextFunction) => {
  const address = req.params.address as string;
  const take = Math.min(parseInt(String(req.query.limit || '50'), 10) || 50, 200);
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

router.get('/stats', async (req: Request, res: Response, next: NextFunction) => {
  try {
    const { from, to, range } = req.query;

    let dateFrom: Date | undefined;
    let dateTo: Date | undefined;

    if (range) {
      const now = new Date();
      dateTo = now;
      if (range === 'day') {
        dateFrom = new Date(now.getTime() - 24 * 60 * 60 * 1000);
      } else if (range === 'week') {
        dateFrom = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
      } else if (range === 'month') {
        dateFrom = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);
      } else {
        return next(badRequest('Invalid range value. Use day, week, or month.'));
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

export default router;
