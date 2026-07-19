/**
 * stats.ts — server-side aggregation queries for the analytics dashboard.
 *
 * All queries read from PostgreSQL. The daily_marketplace_stats materialized
 * view is used for the /stats/daily endpoint; the other endpoints query the
 * primary tables directly.
 */

import prisma from './db.js';

export interface OverviewStats {
  totalListings: number;
  totalSales: number;
  totalVolume: string;
  totalCreators: number;
  totalCollections: number;
}

export interface DailyStatRow {
  day: string;             // ISO date string YYYY-MM-DD
  salesCount: number;
  salesVolume: string;
  uniqueBuyers: number;
  uniqueSellers: number;
  newListings: number;
  avgSalePrice: string;
}

export interface TopCollectionRow {
  collection: string;
  salesCount: number;
  salesVolume: string;
}

export interface TopArtistRow {
  artist: string;
  earnings: string;
  salesCount: number;
}

// ── Overview ──────────────────────────────────────────────────────────────────

export async function getOverviewStats(): Promise<OverviewStats> {
  const [
    totalListings,
    totalSalesRow,
    totalVolumeRow,
    totalCreatorsRow,
    totalCollections,
  ] = await Promise.all([
    prisma.listing.count(),

    prisma.marketplaceEvent.count({
      where: { eventType: 'ARTWORK_SOLD' },
    }),

    prisma.listing.aggregate({
      _sum: { price: true },
      where: { status: 'Sold' },
    }),

    // Distinct artists who have ever listed
    prisma.listing.groupBy({
      by: ['artist'],
    }),

    prisma.collection.count(),
  ]);

  return {
    totalListings,
    totalSales: totalSalesRow,
    totalVolume: totalVolumeRow._sum.price?.toString() ?? '0',
    totalCreators: totalCreatorsRow.length,
    totalCollections,
  };
}

// ── Daily stats (materialized view) ──────────────────────────────────────────

export async function getDailyStats(
  from: Date,
  to: Date
): Promise<DailyStatRow[]> {
  type ViewRow = {
    day: Date;
    sales_count: bigint;
    sales_volume: string;
    unique_buyers: bigint;
    unique_sellers: bigint;
    new_listings: bigint;
    avg_sale_price: string;
  };

  // Query the materialized view via raw SQL
  const rows: ViewRow[] = await prisma.$queryRaw<ViewRow[]>`
    SELECT
      day,
      sales_count,
      sales_volume::TEXT,
      unique_buyers,
      unique_sellers,
      new_listings,
      avg_sale_price::TEXT
    FROM daily_marketplace_stats
    WHERE day >= ${from}::DATE
      AND day <= ${to}::DATE
    ORDER BY day ASC
  `;

  return rows.map((r: ViewRow): DailyStatRow => ({
    day: r.day.toISOString().slice(0, 10),
    salesCount: Number(r.sales_count),
    salesVolume: r.sales_volume ?? '0',
    uniqueBuyers: Number(r.unique_buyers),
    uniqueSellers: Number(r.unique_sellers),
    newListings: Number(r.new_listings),
    avgSalePrice: r.avg_sale_price ?? '0',
  }));
}

// ── Top collections ───────────────────────────────────────────────────────────

export async function getTopCollections(limit: number): Promise<TopCollectionRow[]> {
  type DecimalLike = { toString(): string } | null;
  type CollectionGroupRow = { collection: string; _count: { listingId: number }; _sum: { price: DecimalLike } };
  const rows = (await prisma.listing.groupBy({
    by: ['collection'],
    where: { status: 'Sold' },
    _count: { listingId: true },
    _sum: { price: true },
    orderBy: { _sum: { price: 'desc' } },
    take: limit,
  })) as unknown as CollectionGroupRow[];

  return rows.map((r: CollectionGroupRow): TopCollectionRow => ({
    collection: r.collection,
    salesCount: r._count.listingId,
    salesVolume: r._sum.price?.toString() ?? '0',
  }));
}

// ── Top artists ───────────────────────────────────────────────────────────────

export async function getTopArtists(limit: number): Promise<TopArtistRow[]> {
  type DecimalLike = { toString(): string } | null;
  type ArtistGroupRow = { artist: string; _count: { listingId: number }; _sum: { price: DecimalLike } };
  const rows = (await prisma.listing.groupBy({
    by: ['artist'],
    where: { status: 'Sold' },
    _count: { listingId: true },
    _sum: { price: true },
    orderBy: { _sum: { price: 'desc' } },
    take: limit,
  })) as unknown as ArtistGroupRow[];

  return rows.map((r: ArtistGroupRow): TopArtistRow => ({
    artist: r.artist,
    earnings: r._sum.price?.toString() ?? '0',
    salesCount: r._count.listingId,
  }));
}

// ── Materialized view refresh ─────────────────────────────────────────────────

export async function refreshDailyStats(): Promise<void> {
  await prisma.$executeRaw`REFRESH MATERIALIZED VIEW CONCURRENTLY daily_marketplace_stats`;
}
