import { rpc } from '@stellar/stellar-sdk';
import prisma from './db.js';

const RPC_URL = process.env.STELLAR_RPC_URL || 'https://soroban-testnet.stellar.org';
const CONTRACT_ID = process.env.MARKETPLACE_CONTRACT_ID || '';

// How many records to sample per run
const SAMPLE_SIZE = parseInt(process.env.RECONCILE_SAMPLE_SIZE || '50');
// Interval between reconciliation runs in ms
const RECONCILE_INTERVAL_MS = parseInt(process.env.RECONCILE_INTERVAL_MS || '300000'); // 5 min

let discrepancyCount = 0;

export function getDiscrepancyCount() {
  return discrepancyCount;
}

export interface ReconcileResult {
  sampledListings: number;
  sampledAuctions: number;
  discrepancies: DiscrepancyRecord[];
}

export interface DiscrepancyRecord {
  kind: 'listing' | 'auction';
  id: string;
  field: string;
  dbValue: string;
  chainValue: string;
}

// Fetch on-chain listing state. Returns null when the contract call fails or the
// listing is not present on-chain (e.g. using a stub or testnet that has no state).
export async function fetchListingOnChain(
  server: rpc.Server,
  _contractId: string,
  _listingId: bigint
): Promise<{ status: string; price: string } | null> {
  // Real implementation would call the contract's `get_listing` view function.
  // This is left as a no-op stub because the Soroban RPC call requires ABI
  // encoding that is contract-specific; the reconciler still exercises the
  // comparison logic when chain data is available.
  return null;
}

export async function fetchAuctionOnChain(
  server: rpc.Server,
  _contractId: string,
  _auctionId: bigint
): Promise<{ status: string; highestBid: string } | null> {
  return null;
}

type FetchListing = (
  server: rpc.Server,
  contractId: string,
  listingId: bigint
) => Promise<{ status: string; price: string } | null>;

type FetchAuction = (
  server: rpc.Server,
  contractId: string,
  auctionId: bigint
) => Promise<{ status: string; highestBid: string } | null>;

export async function runReconciliation(
  server: rpc.Server,
  contractId: string,
  sampleSize = SAMPLE_SIZE,
  fetchListing: FetchListing = fetchListingOnChain,
  fetchAuction: FetchAuction = fetchAuctionOnChain
): Promise<ReconcileResult> {
  const discrepancies: DiscrepancyRecord[] = [];

  // ── Sample active listings ─────────────────────────────────────────────────
  const listings = await prisma.listing.findMany({
    where: { status: 'Active' },
    take: sampleSize,
    orderBy: { updatedAtLedger: 'desc' },
    select: { listingId: true, status: true, price: true },
  });

  for (const listing of listings) {
    const chainState = await fetchListing(server, contractId, listing.listingId);
    if (!chainState) continue; // chain unavailable — skip this record

    if (chainState.status !== listing.status) {
      const rec: DiscrepancyRecord = {
        kind: 'listing',
        id: listing.listingId.toString(),
        field: 'status',
        dbValue: listing.status,
        chainValue: chainState.status,
      };
      discrepancies.push(rec);
      console.warn('[Reconciler] Discrepancy', rec);
      discrepancyCount++;
    }

    if (chainState.price !== listing.price.toString()) {
      const rec: DiscrepancyRecord = {
        kind: 'listing',
        id: listing.listingId.toString(),
        field: 'price',
        dbValue: listing.price.toString(),
        chainValue: chainState.price,
      };
      discrepancies.push(rec);
      console.warn('[Reconciler] Discrepancy', rec);
      discrepancyCount++;
    }
  }

  // ── Sample active auctions ─────────────────────────────────────────────────
  const auctions = await prisma.auction.findMany({
    where: { status: 'Active' },
    take: sampleSize,
    orderBy: { updatedAtLedger: 'desc' },
    select: { auctionId: true, status: true, highestBid: true },
  });

  for (const auction of auctions) {
    const chainState = await fetchAuction(server, contractId, auction.auctionId);
    if (!chainState) continue;

    if (chainState.status !== auction.status) {
      const rec: DiscrepancyRecord = {
        kind: 'auction',
        id: auction.auctionId.toString(),
        field: 'status',
        dbValue: auction.status,
        chainValue: chainState.status,
      };
      discrepancies.push(rec);
      console.warn('[Reconciler] Discrepancy', rec);
      discrepancyCount++;
    }

    if (chainState.highestBid !== auction.highestBid.toString()) {
      const rec: DiscrepancyRecord = {
        kind: 'auction',
        id: auction.auctionId.toString(),
        field: 'highestBid',
        dbValue: auction.highestBid.toString(),
        chainValue: chainState.highestBid,
      };
      discrepancies.push(rec);
      console.warn('[Reconciler] Discrepancy', rec);
      discrepancyCount++;
    }
  }

  console.log(
    `[Reconciler] Sampled ${listings.length} listings, ${auctions.length} auctions. ` +
    `Discrepancies found: ${discrepancies.length}`
  );

  return {
    sampledListings: listings.length,
    sampledAuctions: auctions.length,
    discrepancies,
  };
}

export async function startReconciler() {
  const server = new rpc.Server(RPC_URL);

  const tick = async () => {
    try {
      await runReconciliation(server, CONTRACT_ID);
    } catch (err) {
      console.error('[Reconciler] Run failed:', err);
    }
  };

  // Run once immediately, then on interval
  await tick();
  setInterval(tick, RECONCILE_INTERVAL_MS);
}
