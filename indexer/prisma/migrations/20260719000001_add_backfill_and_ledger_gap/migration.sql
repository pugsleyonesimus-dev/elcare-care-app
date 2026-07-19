-- CreateEnum
CREATE TYPE "BackfillJobStatus" AS ENUM ('Pending', 'Running', 'Completed', 'Failed', 'Cancelled');

-- CreateEnum
CREATE TYPE "LedgerGapSource" AS ENUM ('rpc_window_skip', 'reorg', 'manual');

-- CreateEnum
CREATE TYPE "LedgerGapStatus" AS ENUM ('Open', 'Repairing', 'Repaired', 'Failed');

-- CreateTable
CREATE TABLE "LedgerGap" (
    "id"          SERIAL                  NOT NULL,
    "fromLedger"  INTEGER                 NOT NULL,
    "toLedger"    INTEGER                 NOT NULL,
    "source"      "LedgerGapSource"       NOT NULL,
    "status"      "LedgerGapStatus"       NOT NULL DEFAULT 'Open',
    "error"       TEXT,
    "createdAt"   TIMESTAMP(3)            NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt"   TIMESTAMP(3)            NOT NULL,

    CONSTRAINT "LedgerGap_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "BackfillJob" (
    "id"                SERIAL                  NOT NULL,
    "startLedger"       INTEGER                 NOT NULL,
    "endLedger"         INTEGER                 NOT NULL,
    "checkpointLedger"  INTEGER                 NOT NULL DEFAULT 0,
    "status"            "BackfillJobStatus"     NOT NULL DEFAULT 'Pending',
    "rpcUrl"            TEXT                    NOT NULL,
    "error"             TEXT,
    "totalInserted"     INTEGER                 NOT NULL DEFAULT 0,
    "gapId"             INTEGER,
    "createdAt"         TIMESTAMP(3)            NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt"         TIMESTAMP(3)            NOT NULL,

    CONSTRAINT "BackfillJob_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE INDEX "LedgerGap_status_idx" ON "LedgerGap"("status");
CREATE INDEX "LedgerGap_fromLedger_toLedger_idx" ON "LedgerGap"("fromLedger", "toLedger");
CREATE UNIQUE INDEX "LedgerGap_fromLedger_toLedger_source_key" ON "LedgerGap"("fromLedger", "toLedger", "source");

-- CreateIndex
CREATE INDEX "BackfillJob_status_idx" ON "BackfillJob"("status");
CREATE INDEX "BackfillJob_startLedger_endLedger_idx" ON "BackfillJob"("startLedger", "endLedger");
CREATE INDEX "BackfillJob_gapId_idx" ON "BackfillJob"("gapId");

-- AddForeignKey
ALTER TABLE "BackfillJob" ADD CONSTRAINT "BackfillJob_gapId_fkey"
    FOREIGN KEY ("gapId") REFERENCES "LedgerGap"("id") ON DELETE SET NULL ON UPDATE CASCADE;
