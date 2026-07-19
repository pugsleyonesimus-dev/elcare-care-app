-- CreateEnum
CREATE TYPE "KeeperActionStatus" AS ENUM ('Pending', 'Submitted', 'Succeeded', 'Failed', 'Skipped');

-- CreateEnum
CREATE TYPE "KeeperTargetType" AS ENUM ('ExpireListing', 'FinalizeAuction', 'ReclaimOffer');

-- CreateTable
CREATE TABLE "KeeperAction" (
    "id"         SERIAL          NOT NULL,
    "targetType" "KeeperTargetType" NOT NULL,
    "targetId"   BIGINT          NOT NULL,
    "txHash"     TEXT,
    "status"     "KeeperActionStatus" NOT NULL DEFAULT 'Pending',
    "attempts"   INTEGER         NOT NULL DEFAULT 0,
    "lastError"  TEXT,
    "feePaid"    BIGINT,
    "createdAt"  TIMESTAMP(3)    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt"  TIMESTAMP(3)    NOT NULL,

    CONSTRAINT "KeeperAction_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE UNIQUE INDEX "KeeperAction_targetType_targetId_key" ON "KeeperAction"("targetType", "targetId");

-- CreateIndex
CREATE INDEX "KeeperAction_status_idx" ON "KeeperAction"("status");

-- CreateIndex
CREATE INDEX "KeeperAction_targetType_status_idx" ON "KeeperAction"("targetType", "status");

-- CreateIndex
CREATE INDEX "KeeperAction_updatedAt_idx" ON "KeeperAction"("updatedAt");
