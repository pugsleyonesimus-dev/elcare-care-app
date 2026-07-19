-- Create materialized view: daily_marketplace_stats
-- Aggregates per-day stats from MarketplaceEvent and Listing tables.

CREATE MATERIALIZED VIEW IF NOT EXISTS daily_marketplace_stats AS
SELECT
  DATE(me."ledgerTimestamp") AS day,

  -- Total number of ARTWORK_SOLD events
  COUNT(*) FILTER (WHERE me."eventType" = 'ARTWORK_SOLD') AS sales_count,

  -- Total sales volume (sum of prices of sold listings for that day)
  COALESCE(
    (
      SELECT SUM(l.price)
      FROM "Listing" l
      WHERE l.status = 'Sold'
        AND DATE(l."updatedAt") = DATE(me."ledgerTimestamp")
    ),
    0
  )::NUMERIC AS sales_volume,

  -- Unique buyers (data->>'buyer' for ARTWORK_SOLD events)
  COUNT(DISTINCT me."data"->>'buyer') FILTER (WHERE me."eventType" = 'ARTWORK_SOLD') AS unique_buyers,

  -- Unique sellers (actor on ARTWORK_SOLD events = the artist/seller)
  COUNT(DISTINCT me."actor") FILTER (WHERE me."eventType" = 'ARTWORK_SOLD') AS unique_sellers,

  -- New listings created that day
  COUNT(*) FILTER (WHERE me."eventType" = 'LISTING_CREATED') AS new_listings,

  -- Average sale price: derived from listing prices for sold listings on that day
  COALESCE(
    (
      SELECT AVG(l.price)
      FROM "Listing" l
      WHERE l.status = 'Sold'
        AND DATE(l."updatedAt") = DATE(me."ledgerTimestamp")
    ),
    0
  )::NUMERIC AS avg_sale_price

FROM "MarketplaceEvent" me
GROUP BY DATE(me."ledgerTimestamp")
ORDER BY day;

-- Unique index required for concurrent refresh
CREATE UNIQUE INDEX IF NOT EXISTS daily_marketplace_stats_day_idx
  ON daily_marketplace_stats (day);
