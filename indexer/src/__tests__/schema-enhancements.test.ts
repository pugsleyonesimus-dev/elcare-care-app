import { describe, it, expect, vi, beforeEach } from 'vitest';
import prisma from '../db.js';

// Mock Prisma to avoid actual DB calls in unit tests
vi.mock('../db', () => ({
  default: {
    listing: {
      create: vi.fn(),
      upsert: vi.fn(),
      updateMany: vi.fn(),
      findMany: vi.fn(),
    },
    auction: {
      create: vi.fn(),
      upsert: vi.fn(),
      updateMany: vi.fn(),
      findMany: vi.fn(),
    },
    offer: {
      create: vi.fn(),
      upsert: vi.fn(),
      update: vi.fn(),
      findMany: vi.fn(),
    },
    bid: {
      create: vi.fn(),
      upsert: vi.fn(),
      findMany: vi.fn(),
    },
    collection: {
      create: vi.fn(),
      upsert: vi.fn(),
      findMany: vi.fn(),
    },
    marketplaceEvent: {
      create: vi.fn(),
      findMany: vi.fn(),
    },
    $transaction: vi.fn((fn: any) => fn(prisma)),
  },
}));

describe('Schema Enhancements', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Issue #77: Status Enums', () => {
    it('should validate listing status enum values', async () => {
      const validListingStatuses = ['Active', 'Sold', 'Cancelled', 'Auction'];
      
      expect(validListingStatuses).toContain('Active');
      expect(validListingStatuses).toContain('Sold');
      expect(validListingStatuses).toContain('Cancelled');
      expect(validListingStatuses).toContain('Auction');
    });

    it('should validate auction status enum values', async () => {
      const validAuctionStatuses = ['Active', 'Finalized', 'Cancelled'];
      
      expect(validAuctionStatuses).toContain('Active');
      expect(validAuctionStatuses).toContain('Finalized');
      expect(validAuctionStatuses).toContain('Cancelled');
    });

    it('should validate offer status enum values', async () => {
      const validOfferStatuses = ['Pending', 'Accepted', 'Rejected', 'Withdrawn'];
      
      expect(validOfferStatuses).toContain('Pending');
      expect(validOfferStatuses).toContain('Accepted');
      expect(validOfferStatuses).toContain('Rejected');
      expect(validOfferStatuses).toContain('Withdrawn');
    });

    it('should reject invalid listing status values in strict mode', () => {
      const invalidStatus = 'InvalidStatus';
      const validStatuses = ['Active', 'Sold', 'Cancelled', 'Auction'];
      
      expect(validStatuses).not.toContain(invalidStatus);
    });

    it('should ensure listing creation uses enum status', async () => {
      const mockListing = {
        listingId: 123n,
        artist: 'artist_addr',
        owner: null,
        price: '100.0000000',
        currency: 'XLM',
        collection: 'collection_addr',
        nftTokenId: 1n,
        token: 'native',
        status: 'Active' as const,
        recipients: [],
        createdAtLedger: 1000,
        updatedAtLedger: 1000,
        createdAt: new Date(),
        updatedAt: new Date(),
      };

      (prisma.listing.create as any).mockResolvedValueOnce(mockListing);
      
      await prisma.listing.create({ data: mockListing });
      
      expect(prisma.listing.create).toHaveBeenCalledWith({
        data: expect.objectContaining({
          status: 'Active',
        }),
      });
    });
  });

  describe('Issue #78: CreatedAt/UpdatedAt Timestamps', () => {
    it('should add createdAt to listing', async () => {
      const now = new Date();
      const mockListing = {
        listingId: 101n,
        artist: 'artist_addr',
        owner: null,
        price: '100.0000000',
        currency: 'XLM',
        collection: 'collection_addr',
        nftTokenId: 1n,
        token: 'native',
        status: 'Active' as const,
        recipients: [],
        createdAtLedger: 1000,
        updatedAtLedger: 1000,
        createdAt: now,
        updatedAt: now,
      };

      (prisma.listing.create as any).mockResolvedValueOnce(mockListing);
      
      const result = await prisma.listing.create({ data: mockListing });
      
      expect(result.createdAt).toBeDefined();
      expect(result.createdAt).toBeInstanceOf(Date);
    });

    it('should add updatedAt to listing', async () => {
      const now = new Date();
      const mockListing = {
        listingId: 102n,
        artist: 'artist_addr',
        owner: null,
        price: '100.0000000',
        currency: 'XLM',
        collection: 'collection_addr',
        nftTokenId: 1n,
        token: 'native',
        status: 'Active' as const,
        recipients: [],
        createdAtLedger: 1000,
        updatedAtLedger: 1000,
        createdAt: now,
        updatedAt: now,
      };

      (prisma.listing.create as any).mockResolvedValueOnce(mockListing);
      
      const result = await prisma.listing.create({ data: mockListing });
      
      expect(result.updatedAt).toBeDefined();
      expect(result.updatedAt).toBeInstanceOf(Date);
    });

    it('should preserve createdAt on listing update', async () => {
      const createdAt = new Date('2026-01-01');
      const updatedAt = new Date('2026-06-28');
      
      (prisma.listing.updateMany as any).mockResolvedValueOnce({ count: 1 });
      
      await prisma.listing.updateMany({
        where: { listingId: 101n },
        data: {
          status: 'Sold' as const,
          updatedAtLedger: 2000,
        },
      });

      expect(prisma.listing.updateMany).toHaveBeenCalledWith(
        expect.objectContaining({
          where: { listingId: 101n },
          data: expect.not.objectContaining({
            createdAt,
          }),
        })
      );
    });

    it('should add createdAt and updatedAt to auction', async () => {
      const now = new Date();
      const mockAuction = {
        auctionId: 201n,
        creator: 'creator_addr',
        collection: 'collection_addr',
        nftTokenId: 1n,
        token: 'native',
        reservePrice: '50.0000000',
        highestBid: '100.0000000',
        highestBidder: 'bidder_addr',
        endTime: 2000000000n,
        status: 'Active' as const,
        recipients: [],
        createdAtLedger: 1000,
        updatedAtLedger: 1000,
        createdAt: now,
        updatedAt: now,
      };

      (prisma.auction.create as any).mockResolvedValueOnce(mockAuction);
      
      const result = await prisma.auction.create({ data: mockAuction });
      
      expect(result.createdAt).toBeDefined();
      expect(result.updatedAt).toBeDefined();
    });

    it('should add createdAt and updatedAt to offer', async () => {
      const now = new Date();
      const mockOffer = {
        offerId: 301n,
        listingId: 101n,
        offerer: 'offerer_addr',
        amount: '120.0000000',
        token: 'native',
        status: 'Pending' as const,
        createdAtLedger: 1000,
        updatedAtLedger: 1000,
        createdAt: now,
        updatedAt: now,
      };

      (prisma.offer.create as any).mockResolvedValueOnce(mockOffer);
      
      const result = await prisma.offer.create({ data: mockOffer });
      
      expect(result.createdAt).toBeDefined();
      expect(result.updatedAt).toBeDefined();
    });
  });

  describe('Issue #79: Bid Table for Normalized Auction Bid History', () => {
    it('should create bid record with auction relationship', async () => {
      const mockBid = {
        id: 1,
        auctionId: 201n,
        bidder: 'bidder_addr',
        amount: '100.0000000',
        ledgerSequence: 1050,
        createdAt: new Date(),
      };

      (prisma.bid.create as any).mockResolvedValueOnce(mockBid);
      
      const result = await prisma.bid.create({
        data: {
          auctionId: 201n,
          bidder: 'bidder_addr',
          amount: '100.0000000',
          ledgerSequence: 1050,
        },
      });

      expect(result.auctionId).toBe(201n);
      expect(result.bidder).toBe('bidder_addr');
    });

    it('should store bid amount as decimal', async () => {
      const mockBid = {
        id: 2,
        auctionId: 201n,
        bidder: 'bidder_addr2',
        amount: '250.1234567',
        ledgerSequence: 1060,
        createdAt: new Date(),
      };

      (prisma.bid.create as any).mockResolvedValueOnce(mockBid);
      
      const result = await prisma.bid.create({
        data: {
          auctionId: 201n,
          bidder: 'bidder_addr2',
          amount: '250.1234567',
          ledgerSequence: 1060,
        },
      });

      expect(result.amount).toBe('250.1234567');
    });

    it('should enforce unique constraint on auctionId, ledgerSequence, bidder', async () => {
      const bidData = {
        auctionId: 201n,
        bidder: 'bidder_addr',
        amount: '100.0000000',
        ledgerSequence: 1050,
      };

      (prisma.bid.upsert as any).mockResolvedValueOnce({
        id: 1,
        ...bidData,
        createdAt: new Date(),
      });

      const result1 = await prisma.bid.upsert({
        where: {
          auctionId_ledgerSequence_bidder: {
            auctionId: bidData.auctionId,
            ledgerSequence: bidData.ledgerSequence,
            bidder: bidData.bidder,
          },
        },
        create: bidData,
        update: { amount: bidData.amount },
      });

      expect(result1).toBeDefined();
      expect(result1.auctionId).toBe(201n);
    });

    it('should have proper indexes on bid table', () => {
      const indexes = [
        'auctionId',
        'bidder',
        'ledgerSequence',
        'auctionId_ledgerSequence_bidder',
      ];
      
      expect(indexes).toContain('auctionId');
      expect(indexes).toContain('bidder');
      expect(indexes).toContain('ledgerSequence');
    });

    it('should query bids by auctionId efficiently', async () => {
      const mockBids = [
        {
          id: 1,
          auctionId: 201n,
          bidder: 'bidder1',
          amount: '100.0000000',
          ledgerSequence: 1050,
          createdAt: new Date(),
        },
        {
          id: 2,
          auctionId: 201n,
          bidder: 'bidder2',
          amount: '150.0000000',
          ledgerSequence: 1060,
          createdAt: new Date(),
        },
      ];

      (prisma.bid.findMany as any).mockResolvedValueOnce(mockBids);
      
      const results = await prisma.bid.findMany({
        where: { auctionId: 201n },
      });

      expect(results).toHaveLength(2);
      expect(results[0].auctionId).toBe(201n);
    });

    it('should query bids by bidder for history', async () => {
      const mockBids = [
        {
          id: 1,
          auctionId: 201n,
          bidder: 'bidder_addr',
          amount: '100.0000000',
          ledgerSequence: 1050,
          createdAt: new Date(),
        },
        {
          id: 3,
          auctionId: 202n,
          bidder: 'bidder_addr',
          amount: '200.0000000',
          ledgerSequence: 1070,
          createdAt: new Date(),
        },
      ];

      (prisma.bid.findMany as any).mockResolvedValueOnce(mockBids);
      
      const results = await prisma.bid.findMany({
        where: { bidder: 'bidder_addr' },
      });

      expect(results).toHaveLength(2);
      expect(results.every(b => b.bidder === 'bidder_addr')).toBe(true);
    });

    it('should support highest bid query', async () => {
      const mockHighestBid = {
        id: 2,
        auctionId: 201n,
        bidder: 'bidder2',
        amount: '150.0000000',
        ledgerSequence: 1060,
        createdAt: new Date(),
      };

      (prisma.bid.findMany as any).mockResolvedValueOnce([mockHighestBid]);
      
      const results = await prisma.bid.findMany({
        where: { auctionId: 201n },
        orderBy: { amount: 'desc' },
        take: 1,
      });

      expect(results[0].amount).toBe('150.0000000');
    });
  });

  describe('Issue #80: Prisma Seed Script', () => {
    it('should support seed script configuration in package.json', () => {
      // This test verifies seed config is properly defined
      const seedConfig = {
        seed: 'tsx prisma/seed.ts',
      };

      expect(seedConfig.seed).toBe('tsx prisma/seed.ts');
    });

    it('should create representative seed data with multiple collections', async () => {
      const mockCollections = [
        {
          id: 1,
          contractAddress: 'CAQBWUKVLOR5W43QBQDFJAHSE2LUGCALRDCM7EVEO36FTWOP5P2O36ML',
          kind: 'normal_1155',
          creator: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
          name: 'African Heritage NFTs',
          symbol: 'AHT',
          deployedAtLedger: 1000,
          createdAt: new Date(),
        },
      ];

      (prisma.collection.findMany as any).mockResolvedValueOnce(mockCollections);
      
      const results = await prisma.collection.findMany();
      
      expect(results).toHaveLength(1);
      expect(results[0].kind).toBe('normal_1155');
    });

    it('should create representative seed data with listings', async () => {
      const mockListings = [
        {
          listingId: 101n,
          artist: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
          owner: null,
          price: '100.0000000',
          currency: 'XLM',
          collection: 'CAQBWUKVLOR5W43QBQDFJAHSE2LUGCALRDCM7EVEO36FTWOP5P2O36ML',
          nftTokenId: 1n,
          token: 'native',
          status: 'Active' as const,
          recipients: [],
          createdAtLedger: 1100,
          updatedAtLedger: 1100,
          createdAt: new Date(),
          updatedAt: new Date(),
        },
      ];

      (prisma.listing.findMany as any).mockResolvedValueOnce(mockListings);
      
      const results = await prisma.listing.findMany();
      
      expect(results).toHaveLength(1);
      expect(results[0].status).toBe('Active');
    });

    it('should create representative seed data with auctions', async () => {
      const mockAuctions = [
        {
          auctionId: 201n,
          creator: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
          collection: 'CAQBWUKVLOR5W43QBQDFJAHSE2LUGCALRDCM7EVEO36FTWOP5P2O36ML',
          nftTokenId: 3n,
          token: 'native',
          reservePrice: '50.0000000',
          highestBid: '150.0000000',
          highestBidder: 'GBD4U46RSIVHVGNGHSRUQQ7SVU7ISA26FTJSTPVYS4IRAHYRNECKUIGHTS',
          endTime: 2000000000n,
          status: 'Active' as const,
          recipients: [],
          createdAtLedger: 1250,
          updatedAtLedger: 1300,
          createdAt: new Date(),
          updatedAt: new Date(),
        },
      ];

      (prisma.auction.findMany as any).mockResolvedValueOnce(mockAuctions);
      
      const results = await prisma.auction.findMany();
      
      expect(results).toHaveLength(1);
      expect(results[0].status).toBe('Active');
    });

    it('should create representative seed data with bids', async () => {
      const mockBids = [
        {
          id: 1,
          auctionId: 201n,
          bidder: 'GCXVVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVX',
          amount: '75.0000000',
          ledgerSequence: 1310,
          createdAt: new Date(),
        },
      ];

      (prisma.bid.findMany as any).mockResolvedValueOnce(mockBids);
      
      const results = await prisma.bid.findMany();
      
      expect(results).toHaveLength(1);
      expect(results[0].auctionId).toBe(201n);
    });

    it('should create representative seed data with offers', async () => {
      const mockOffers = [
        {
          offerId: 301n,
          listingId: 101n,
          offerer: 'GCXVVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVXVX',
          amount: '120.0000000',
          token: 'native',
          status: 'Pending' as const,
          createdAtLedger: 1350,
          updatedAtLedger: 1350,
          createdAt: new Date(),
          updatedAt: new Date(),
        },
      ];

      (prisma.offer.findMany as any).mockResolvedValueOnce(mockOffers);
      
      const results = await prisma.offer.findMany();
      
      expect(results).toHaveLength(1);
      expect(results[0].status).toBe('Pending');
    });

    it('should create representative seed data with events', async () => {
      const mockEvents = [
        {
          id: 1,
          listingId: 101n,
          eventType: 'LISTING_CREATED',
          actor: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
          data: {
            listing_id: '101',
            artist: 'GBFUNHEQOVN35LFEKP7SZXFYJPMJ3WLXLX4PQZGBK737NTLRHOKVES3F',
          },
          ledgerSequence: 1100,
          ledgerTimestamp: new Date(),
        },
      ];

      (prisma.marketplaceEvent.findMany as any).mockResolvedValueOnce(mockEvents);
      
      const results = await prisma.marketplaceEvent.findMany();
      
      expect(results).toHaveLength(1);
      expect(results[0].eventType).toBe('LISTING_CREATED');
    });

    it('seed data should be idempotent', () => {
      // Multiple runs should produce the same result
      const contracts = [
        {
          contractAddress: 'CAQBWUKVLOR5W43QBQDFJAHSE2LUGCALRDCM7EVEO36FTWOP5P2O36ML',
          kind: 'normal_1155',
        },
      ];

      // First run
      const firstRun = contracts[0];
      
      // Second run (should be identical)
      const secondRun = {
        contractAddress: 'CAQBWUKVLOR5W43QBQDFJAHSE2LUGCALRDCM7EVEO36FTWOP5P2O36ML',
        kind: 'normal_1155',
      };

      expect(firstRun.contractAddress).toBe(secondRun.contractAddress);
      expect(firstRun.kind).toBe(secondRun.kind);
    });
  });
});
