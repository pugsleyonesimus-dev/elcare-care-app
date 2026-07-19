-- CreateTable: dynamic contract registry
CREATE TABLE "TrackedContract" (
    "id"             SERIAL       NOT NULL,
    "contractId"     TEXT         NOT NULL,
    "type"           TEXT         NOT NULL,
    "label"          TEXT         NOT NULL DEFAULT '',
    "startLedger"    INTEGER      NOT NULL DEFAULT 0,
    "lastLedger"     INTEGER      NOT NULL DEFAULT 0,
    "lastLedgerHash" TEXT,
    "active"         BOOLEAN      NOT NULL DEFAULT true,
    "createdAt"      TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt"      TIMESTAMP(3) NOT NULL,

    CONSTRAINT "TrackedContract_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE UNIQUE INDEX "TrackedContract_contractId_key" ON "TrackedContract"("contractId");
CREATE INDEX "TrackedContract_active_idx" ON "TrackedContract"("active");
CREATE INDEX "TrackedContract_type_idx" ON "TrackedContract"("type");

-- AddColumn: tag every marketplace event with its source contract
ALTER TABLE "MarketplaceEvent" ADD COLUMN "contractId" TEXT NOT NULL DEFAULT '';

-- CreateIndex on contractId
CREATE INDEX "MarketplaceEvent_contractId_idx" ON "MarketplaceEvent"("contractId");
