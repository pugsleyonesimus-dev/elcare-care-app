-- Add eventHash column to MarketplaceEvent for idempotent event processing.
-- The column is nullable initially so the backfill can run before we enforce NOT NULL.

ALTER TABLE "MarketplaceEvent" ADD COLUMN IF NOT EXISTS "eventHash" TEXT;

-- Backfill existing rows with a deterministic hash derived from available columns.
-- Uses MD5(listingId || eventType || ledgerSequence || id) as a stable surrogate
-- for rows that predate contractId/txHash tracking.
UPDATE "MarketplaceEvent"
SET "eventHash" = encode(
  digest(
    COALESCE("listingId"::TEXT, 'null') || ':' ||
    "eventType" || ':' ||
    "ledgerSequence"::TEXT || ':' ||
    "id"::TEXT,
    'sha256'
  ),
  'hex'
)
WHERE "eventHash" IS NULL;

-- Now enforce NOT NULL and add the unique constraint.
ALTER TABLE "MarketplaceEvent" ALTER COLUMN "eventHash" SET NOT NULL;
ALTER TABLE "MarketplaceEvent" ALTER COLUMN "eventHash" SET DEFAULT '';
CREATE UNIQUE INDEX IF NOT EXISTS "MarketplaceEvent_eventHash_key" ON "MarketplaceEvent"("eventHash");
