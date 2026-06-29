import { describe, it, expect, vi, beforeEach } from 'vitest';

// ── Mocks ────────────────────────────────────────────────────────────────────

const mockPrisma = vi.hoisted(() => ({
  listing: { findMany: vi.fn() },
  auction: { findMany: vi.fn() },
}));

vi.mock('../db', () => ({ default: mockPrisma }));

import { runReconciliation, fetchListingOnChain, fetchAuctionOnChain } from '../reconciler';
import type { rpc } from '@stellar/stellar-sdk';

const mockServer = {} as rpc.Server;

const sampleListing = (overrides = {}) => ({
  listingId: BigInt(1),
  status: 'Active',
  price: { toString: () => '100.0000000' },
  ...overrides,
});

const sampleAuction = (overrides = {}) => ({
  auctionId: BigInt(1),
  status: 'Active',
  highestBid: { toString: () => '0.0000000' },
  ...overrides,
});

beforeEach(() => {
  vi.clearAllMocks();
  mockPrisma.listing.findMany.mockResolvedValue([]);
  mockPrisma.auction.findMany.mockResolvedValue([]);
});

// ── runReconciliation ─────────────────────────────────────────────────────────

describe('runReconciliation', () => {
  it('returns zero discrepancies when chain responses are null (stub/unavailable)', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([sampleListing()]);
    mockPrisma.auction.findMany.mockResolvedValue([sampleAuction()]);

    const result = await runReconciliation(mockServer, 'CONTRACT');

    expect(result.sampledListings).toBe(1);
    expect(result.sampledAuctions).toBe(1);
    expect(result.discrepancies).toHaveLength(0);
  });

  it('detects a listing status mismatch', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([sampleListing()]);
    mockPrisma.auction.findMany.mockResolvedValue([]);

    // Override fetchListingOnChain to return a different status
    vi.spyOn({ fetchListingOnChain }, 'fetchListingOnChain');
    const fetchSpy = vi.fn().mockResolvedValue({ status: 'Sold', price: '100.0000000' });

    const result = await runReconciliation(mockServer, 'CONTRACT', 50, fetchSpy, undefined);

    expect(result.discrepancies).toHaveLength(1);
    expect(result.discrepancies[0]).toMatchObject({
      kind: 'listing',
      field: 'status',
      dbValue: 'Active',
      chainValue: 'Sold',
    });
  });

  it('detects a listing price mismatch', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([sampleListing()]);
    mockPrisma.auction.findMany.mockResolvedValue([]);

    const fetchSpy = vi.fn().mockResolvedValue({ status: 'Active', price: '999.0000000' });

    const result = await runReconciliation(mockServer, 'CONTRACT', 50, fetchSpy, undefined);

    expect(result.discrepancies).toHaveLength(1);
    expect(result.discrepancies[0]).toMatchObject({
      kind: 'listing',
      field: 'price',
      dbValue: '100.0000000',
      chainValue: '999.0000000',
    });
  });

  it('detects an auction status mismatch', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([]);
    mockPrisma.auction.findMany.mockResolvedValue([sampleAuction()]);

    const fetchAuctionSpy = vi.fn().mockResolvedValue({ status: 'Finalized', highestBid: '0.0000000' });

    const result = await runReconciliation(mockServer, 'CONTRACT', 50, undefined, fetchAuctionSpy);

    expect(result.discrepancies).toHaveLength(1);
    expect(result.discrepancies[0]).toMatchObject({
      kind: 'auction',
      field: 'status',
      dbValue: 'Active',
      chainValue: 'Finalized',
    });
  });

  it('detects an auction highestBid mismatch', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([]);
    mockPrisma.auction.findMany.mockResolvedValue([sampleAuction()]);

    const fetchAuctionSpy = vi.fn().mockResolvedValue({ status: 'Active', highestBid: '500.0000000' });

    const result = await runReconciliation(mockServer, 'CONTRACT', 50, undefined, fetchAuctionSpy);

    expect(result.discrepancies).toHaveLength(1);
    expect(result.discrepancies[0]).toMatchObject({
      kind: 'auction',
      field: 'highestBid',
      dbValue: '0.0000000',
      chainValue: '500.0000000',
    });
  });

  it('reports no discrepancies when DB and chain match', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([sampleListing()]);
    mockPrisma.auction.findMany.mockResolvedValue([sampleAuction()]);

    const fetchListingSpy = vi.fn().mockResolvedValue({ status: 'Active', price: '100.0000000' });
    const fetchAuctionSpy = vi.fn().mockResolvedValue({ status: 'Active', highestBid: '0.0000000' });

    const result = await runReconciliation(mockServer, 'CONTRACT', 50, fetchListingSpy, fetchAuctionSpy);

    expect(result.discrepancies).toHaveLength(0);
    expect(result.sampledListings).toBe(1);
    expect(result.sampledAuctions).toBe(1);
  });

  it('respects the sampleSize parameter', async () => {
    mockPrisma.listing.findMany.mockResolvedValue([]);
    mockPrisma.auction.findMany.mockResolvedValue([]);

    await runReconciliation(mockServer, 'CONTRACT', 25);

    expect(mockPrisma.listing.findMany).toHaveBeenCalledWith(
      expect.objectContaining({ take: 25 })
    );
    expect(mockPrisma.auction.findMany).toHaveBeenCalledWith(
      expect.objectContaining({ take: 25 })
    );
  });
});
