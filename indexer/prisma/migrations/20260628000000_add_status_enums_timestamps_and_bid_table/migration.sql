-- CreateEnum for ListingStatus
CREATE TYPE "ListingStatus" AS ENUM ('Active', 'Sold', 'Cancelled', 'Auction');

-- CreateEnum for AuctionStatus
CREATE TYPE "AuctionStatus" AS ENUM ('Active', 'Finalized', 'Cancelled');

-- CreateEnum for OfferStatus
CREATE TYPE "OfferStatus" AS ENUM ('Pending', 'Accepted', 'Rejected', 'Withdrawn');

-- Add createdAt and updatedAt to Listing, set defaults for existing rows
ALTER TABLE "Listing" ADD COLUMN "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "Listing" ADD COLUMN "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Convert Listing.status to enum
ALTER TABLE "Listing" ALTER COLUMN "status" TYPE "ListingStatus" USING "status"::"ListingStatus";
ALTER TABLE "Listing" ALTER COLUMN "status" SET DEFAULT 'Active'::"ListingStatus";

-- Add createdAt and updatedAt to Auction, set defaults for existing rows
ALTER TABLE "Auction" ADD COLUMN "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "Auction" ADD COLUMN "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Convert Auction.status to enum
ALTER TABLE "Auction" ALTER COLUMN "status" TYPE "AuctionStatus" USING "status"::"AuctionStatus";
ALTER TABLE "Auction" ALTER COLUMN "status" SET DEFAULT 'Active'::"AuctionStatus";

-- Add createdAt and updatedAt to Offer, set defaults for existing rows
ALTER TABLE "Offer" ADD COLUMN "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "Offer" ADD COLUMN "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Convert Offer.status to enum
ALTER TABLE "Offer" ALTER COLUMN "status" TYPE "OfferStatus" USING "status"::"OfferStatus";
ALTER TABLE "Offer" ALTER COLUMN "status" SET DEFAULT 'Pending'::"OfferStatus";

-- CreateTable Bid
CREATE TABLE "Bid" (
    "id" SERIAL NOT NULL,
    "auctionId" BIGINT NOT NULL,
    "bidder" TEXT NOT NULL,
    "amount" DECIMAL(32,7) NOT NULL,
    "ledgerSequence" INTEGER NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "Bid_pkey" PRIMARY KEY ("id")
);

-- CreateIndex for Bid
CREATE INDEX "Bid_auctionId_idx" ON "Bid"("auctionId");
CREATE INDEX "Bid_bidder_idx" ON "Bid"("bidder");
CREATE INDEX "Bid_ledgerSequence_idx" ON "Bid"("ledgerSequence");
CREATE UNIQUE INDEX "Bid_auctionId_ledgerSequence_bidder_key" ON "Bid"("auctionId", "ledgerSequence", "bidder");
